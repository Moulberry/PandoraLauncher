use std::{
    convert::{TryFrom, TryInto},
    error::Error,
    ffi::{OsStr, OsString},
    fmt::Display,
    mem,
    os::unix::ffi::OsStrExt,
    sync::Weak,
};

use inotify_sys as ffi;

use crate::fd_guard::FdGuard;
use crate::watches::WatchDescriptor;

/// Iterator over inotify events
///
/// Allows for iteration over the events returned by
/// [`Inotify::read_events_blocking`] or [`Inotify::read_events`].
///
/// [`Inotify::read_events_blocking`]: crate::Inotify::read_events_blocking
/// [`Inotify::read_events`]: crate::Inotify::read_events
#[derive(Debug)]
pub struct Events<'a> {
    fd: Weak<FdGuard>,
    buffer: &'a [u8],
    num_bytes: usize,
    pos: usize,
}

impl<'a> Events<'a> {
    pub(crate) fn new(fd: Weak<FdGuard>, buffer: &'a [u8], num_bytes: usize) -> Self {
        Events {
            fd,
            buffer,
            num_bytes,
            pos: 0,
        }
    }
}

impl<'a> Iterator for Events<'a> {
    type Item = Event<&'a OsStr>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.num_bytes {
            let (step, event) = Event::from_buffer(self.fd.clone(), &self.buffer[self.pos..]);
            self.pos += step;

            Some(event)
        } else {
            None
        }
    }
}

/// An inotify event
///
/// A file system event that describes a change that the user previously
/// registered interest in. To watch for events, call [`Watches::add`]. To
/// retrieve events, call [`Inotify::read_events_blocking`] or
/// [`Inotify::read_events`].
///
/// [`Watches::add`]: crate::Watches::add
/// [`Inotify::read_events_blocking`]: crate::Inotify::read_events_blocking
/// [`Inotify::read_events`]: crate::Inotify::read_events
#[derive(Clone, Debug)]
pub struct Event<S> {
    /// Identifies the watch this event originates from
    ///
    /// This [`WatchDescriptor`] is equal to the one that [`Watches::add`]
    /// returned when interest for this event was registered. The
    /// [`WatchDescriptor`] can be used to remove the watch using
    /// [`Watches::remove`], thereby preventing future events of this type
    /// from being created.
    ///
    /// [`Watches::add`]: crate::Watches::add
    /// [`Watches::remove`]: crate::Watches::remove
    pub wd: WatchDescriptor,

    /// Indicates what kind of event this is
    pub mask: EventMask,

    /// Connects related events to each other
    ///
    /// When a file is renamed, this results two events: [`MOVED_FROM`] and
    /// [`MOVED_TO`]. The `cookie` field will be the same for both of them,
    /// thereby making is possible to connect the event pair.
    ///
    /// [`MOVED_FROM`]: EventMask::MOVED_FROM
    /// [`MOVED_TO`]: EventMask::MOVED_TO
    pub cookie: u32,

    /// The name of the file the event originates from
    ///
    /// This field is set only if the subject of the event is a file or directory in a
    /// watched directory. If the event concerns a file or directory that is
    /// watched directly, `name` will be `None`.
    pub name: Option<S>,
}

impl<'a> Event<&'a OsStr> {
    fn new(fd: Weak<FdGuard>, event: &ffi::inotify_event, name: &'a OsStr) -> Self {
        let mask = EventMask::from_bits(event.mask)
            .expect("Failed to convert event mask. This indicates a bug.");

        let wd = crate::WatchDescriptor { id: event.wd, fd };

        let name = if name.is_empty() { None } else { Some(name) };

        Event {
            wd,
            mask,
            cookie: event.cookie,
            name,
        }
    }

    /// Create an `Event` from a buffer
    ///
    /// Assumes that a full `inotify_event` plus its name is located at the
    /// beginning of `buffer`.
    ///
    /// Returns the number of bytes used from the buffer, and the event.
    ///
    /// # Panics
    ///
    /// Panics if the buffer does not contain a full event, including its name.
    pub(crate) fn from_buffer(fd: Weak<FdGuard>, buffer: &'a [u8]) -> (usize, Self) {
        let event_size = mem::size_of::<ffi::inotify_event>();

        // Make sure that the buffer is big enough to contain an event, without
        // the name. Otherwise we can't safely convert it to an `inotify_event`.
        assert!(buffer.len() >= event_size);

        let ffi_event_ptr = buffer.as_ptr() as *const ffi::inotify_event;

        // We have a pointer to an `inotify_event`, pointing to the beginning of
        // `buffer`. Since we know, as per the assertion above, that there are
        // enough bytes in the buffer for at least one event, we can safely
        // read that `inotify_event`.
        // We call `read_unaligned()` since the byte buffer has alignment 1
        // and `inotify_event` has a higher alignment, so `*` cannot be used to dereference
        // the unaligned pointer (undefined behavior).
        let ffi_event = unsafe { ffi_event_ptr.read_unaligned() };

        // The name's length is given by `event.len`. There should always be
        // enough bytes left in the buffer to fit the name. Let's make sure that
        // is the case.
        let bytes_left_in_buffer = buffer.len() - event_size;
        assert!(bytes_left_in_buffer >= ffi_event.len as usize);

        // Directly after the event struct should be a name, if there's one
        // associated with the event. Let's make a new slice that starts with
        // that name. If there's no name, this slice might have a length of `0`.
        let bytes_consumed = event_size + ffi_event.len as usize;
        let name = &buffer[event_size..bytes_consumed];

        // Remove trailing '\0' bytes
        //
        // The events in the buffer are aligned, and `name` is filled up
        // with '\0' up to the alignment boundary. Here we remove those
        // additional bytes.
        //
        // The `unwrap` here is safe, because `splitn` always returns at
        // least one result, even if the original slice contains no '\0'.
        let name = name.splitn(2, |b| b == &0u8).next().unwrap();

        let event = Event::new(fd, &ffi_event, OsStr::from_bytes(name));

        (bytes_consumed, event)
    }

    /// Returns an owned copy of the event.
    #[deprecated = "use `to_owned()` instead; methods named `into_owned()` usually take self by value"]
    #[allow(clippy::wrong_self_convention)]
    pub fn into_owned(&self) -> EventOwned {
        self.to_owned()
    }

    /// Returns an owned copy of the event.
    #[must_use = "cloning is often expensive and is not expected to have side effects"]
    pub fn to_owned(&self) -> EventOwned {
        Event {
            wd: self.wd.clone(),
            mask: self.mask,
            cookie: self.cookie,
            name: self.name.map(OsStr::to_os_string),
        }
    }
}

/// An owned version of `Event`
pub type EventOwned = Event<OsString>;

bitflags! {
    /// Indicates the type of an event
    ///
    /// This struct can be retrieved from an [`Event`] via its `mask` field.
    /// You can determine the [`Event`]'s type by comparing the `EventMask` to
    /// its associated constants.
    ///
    /// Please refer to the documentation of [`Event`] for a usage example.
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct EventMask: u32 {
        /// File was accessed
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_ACCESS`].
        const ACCESS = ffi::IN_ACCESS;

        /// Metadata (permissions, timestamps, ...) changed
        ///
        /// When watching a directory, this event can be triggered for the
        /// directory itself, as well as objects inside the directory.
        ///
        /// See [`inotify_sys::IN_ATTRIB`].
        const ATTRIB = ffi::IN_ATTRIB;

        /// File opened for writing was closed
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_CLOSE_WRITE`].
        const CLOSE_WRITE = ffi::IN_CLOSE_WRITE;

        /// File or directory not opened for writing was closed
        ///
        /// When watching a directory, this event can be triggered for the
        /// directory itself, as well as objects inside the directory.
        ///
        /// See [`inotify_sys::IN_CLOSE_NOWRITE`].
        const CLOSE_NOWRITE = ffi::IN_CLOSE_NOWRITE;

        /// File/directory created in watched directory
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_CREATE`].
        const CREATE = ffi::IN_CREATE;

        /// File/directory deleted from watched directory
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        const DELETE = ffi::IN_DELETE;

        /// Watched file/directory was deleted
        ///
        /// See [`inotify_sys::IN_DELETE_SELF`].
        const DELETE_SELF = ffi::IN_DELETE_SELF;

        /// File was modified
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_MODIFY`].
        const MODIFY = ffi::IN_MODIFY;

        /// Watched file/directory was moved
        ///
        /// See [`inotify_sys::IN_MOVE_SELF`].
        const MOVE_SELF = ffi::IN_MOVE_SELF;

        /// File was renamed/moved; watched directory contained old name
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_MOVED_FROM`].
        const MOVED_FROM = ffi::IN_MOVED_FROM;

        /// File was renamed/moved; watched directory contains new name
        ///
        /// When watching a directory, this event is only triggered for objects
        /// inside the directory, not the directory itself.
        ///
        /// See [`inotify_sys::IN_MOVED_TO`].
        const MOVED_TO = ffi::IN_MOVED_TO;

        /// File or directory was opened
        ///
        /// When watching a directory, this event can be triggered for the
        /// directory itself, as well as objects inside the directory.
        ///
        /// See [`inotify_sys::IN_OPEN`].
        const OPEN = ffi::IN_OPEN;

        /// Watch was removed
        ///
        /// This event will be generated, if the watch was removed explicitly
        /// (via [`Watches::remove`]), or automatically (because the file was
        /// deleted or the file system was unmounted).
        ///
        /// See [`inotify_sys::IN_IGNORED`].
        ///
        /// [`Watches::remove`]: crate::Watches::remove
        const IGNORED = ffi::IN_IGNORED;

        /// Event related to a directory
        ///
        /// The subject of the event is a directory.
        ///
        /// See [`inotify_sys::IN_ISDIR`].
        const ISDIR = ffi::IN_ISDIR;

        /// Event queue overflowed
        ///
        /// The event queue has overflowed and events have presumably been lost.
        ///
        /// See [`inotify_sys::IN_Q_OVERFLOW`].
        const Q_OVERFLOW = ffi::IN_Q_OVERFLOW;

        /// File system containing watched object was unmounted.
        /// File system was unmounted
        ///
        /// The file system that contained the watched object has been
        /// unmounted. An event with [`EventMask::IGNORED`] will subsequently be
        /// generated for the same watch descriptor.
        ///
        /// See [`inotify_sys::IN_UNMOUNT`].
        const UNMOUNT = ffi::IN_UNMOUNT;
    }
}

impl EventMask {
    /// Parse this event mask into a ParsedEventMask
    pub fn parse(self) -> Result<ParsedEventMask, EventMaskParseError> {
        self.try_into()
    }

    /// Wrapper around [`Self::from_bits_retain`] for backwards compatibility
    ///
    /// # Safety
    ///
    /// This function is not actually unsafe. It is just a wrapper around the
    /// safe [`Self::from_bits_retain`].
    #[deprecated = "Use the safe `from_bits_retain` method instead"]
    pub unsafe fn from_bits_unchecked(bits: u32) -> Self {
        Self::from_bits_retain(bits)
    }
}

/// A struct that provides structured access to event masks
/// returned from reading an event from an inotify fd
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParsedEventMask {
    /// The kind of event that occurred
    ///
    /// Inotify events that come from the kernel have
    /// exactly 0 or 1 of the flags associated with the
    /// event type set.
    pub kind: Option<EventKind>,
    /// The auxiliary flags about the event
    pub auxiliary_flags: EventAuxiliaryFlags,
}

impl ParsedEventMask {
    /// Construct a `ParsedEventMask` from its component parts
    pub fn from_parts(kind: Option<EventKind>, auxiliary_flags: EventAuxiliaryFlags) -> Self {
        ParsedEventMask {
            kind,
            auxiliary_flags,
        }
    }

    /// Parse a raw event mask
    pub fn from_raw_event_mask(mask: EventMask) -> Result<Self, EventMaskParseError> {
        if mask.contains(EventMask::Q_OVERFLOW) {
            return Err(EventMaskParseError::QueueOverflow);
        }

        let kind = Option::<EventKind>::try_from(mask)?;
        let auxiliary_flags = EventAuxiliaryFlags::from(mask);

        Ok(ParsedEventMask::from_parts(kind, auxiliary_flags))
    }
}

impl TryFrom<EventMask> for ParsedEventMask {
    type Error = EventMaskParseError;

    fn try_from(value: EventMask) -> Result<Self, Self::Error> {
        Self::from_raw_event_mask(value)
    }
}

/// Represents the type of inotify event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// File was accessed (e.g., [`read(2)`], [`execve(2)`])
    ///
    /// [`read(2)`]: https://man7.org/linux/man-pages/man2/read.2.html
    /// [`execve(2)`]: https://man7.org/linux/man-pages/man2/execve.2.html
    Access,

    /// Metadata changed—for example, permissions (e.g.,
    /// [`chmod(2)`]), timestamps (e.g., [`utimensat(2)`]), extended
    /// attributes ([`setxattr(2)`]), link count (since Linux
    /// 2.6.25; e.g., for the target of [`link(2)`] and for
    /// [`unlink(2)`]), and user/group ID (e.g., [`chown(2)`])
    ///
    /// [`chmod(2)`]: https://man7.org/linux/man-pages/man2/chmod.2.html
    /// [`utimensat(2)`]: https://man7.org/linux/man-pages/man2/utimensat.2.html
    /// [`setxattr(2)`]: https://man7.org/linux/man-pages/man2/setxattr.2.html
    /// [`link(2)`]: https://man7.org/linux/man-pages/man2/link.2.html
    /// [`unlink(2)`]: https://man7.org/linux/man-pages/man2/unlink.2.html
    /// [`chown(2)`]: https://man7.org/linux/man-pages/man2/chown.2.html
    Attrib,

    /// File opened for writing was closed
    CloseWrite,

    /// File or directory not opened for writing was closed
    CloseNowrite,

    /// File/directory created in watched directory (e.g.,
    /// [`open(2)`] **O_CREAT**, [`mkdir(2)`], [`link(2)`], [`symlink(2)`], [`bind(2)`]
    /// on a UNIX domain socket)
    ///
    /// [`open(2)`]: https://man7.org/linux/man-pages/man2/open.2.html
    /// [`mkdir(2)`]: https://man7.org/linux/man-pages/man2/mkdir.2.html
    /// [`link(2)`]: https://man7.org/linux/man-pages/man2/link.2.html
    /// [`symlink(2)`]: https://man7.org/linux/man-pages/man2/symlink.2.html
    /// [`bind(2)`]: https://man7.org/linux/man-pages/man2/bind.2.html
    Create,

    /// File/directory deleted from watched directory
    Delete,

    /// Watched file/directory was itself deleted. (This event
    /// also occurs if an object is moved to another
    /// filesystem, since [`mv(1)`] in effect copies the file to
    /// the other filesystem and then deletes it from the
    /// original filesystem.)
    ///
    /// [`mv(1)`]: https://man7.org/linux/man-pages/man1/mv.1.html
    DeleteSelf,

    /// File was modified (e.g., [`write(2)`], [`truncate(2)`])
    ///
    /// [`write(2)`]: https://man7.org/linux/man-pages/man2/write.2.html
    /// [`truncate(2)`]: https://man7.org/linux/man-pages/man2/truncate.2.html
    Modify,

    /// Watched file/directory was itself moved
    MoveSelf,

    /// Generated for the directory containing the old filename when a file is renamed
    MovedFrom,

    /// Generated for the directory containing the new filename when a file is renamed
    MovedTo,

    /// File or directory was opened
    Open,
}

impl EventKind {
    const BITFLAG_ENUM_MAP: &[(EventMask, EventKind)] = &[
        (EventMask::ACCESS, EventKind::Access),
        (EventMask::ATTRIB, EventKind::Attrib),
        (EventMask::CLOSE_WRITE, EventKind::CloseWrite),
        (EventMask::CLOSE_NOWRITE, EventKind::CloseNowrite),
        (EventMask::CREATE, EventKind::Create),
        (EventMask::DELETE, EventKind::Delete),
        (EventMask::DELETE_SELF, EventKind::DeleteSelf),
        (EventMask::MODIFY, EventKind::Modify),
        (EventMask::MOVE_SELF, EventKind::MoveSelf),
        (EventMask::MOVED_FROM, EventKind::MovedFrom),
        (EventMask::MOVED_TO, EventKind::MovedTo),
        (EventMask::OPEN, EventKind::Open),
    ];

    /// Parse the auxiliary flags from a raw event mask
    pub fn from_raw_event_mask(mask: EventMask) -> Result<Option<Self>, EventMaskParseError> {
        let mut kinds = Self::BITFLAG_ENUM_MAP.iter().filter_map(|bf_map| {
            if mask.contains(bf_map.0) {
                Some(bf_map.1)
            } else {
                None
            }
        });

        // Optionally take the first matching bitflag
        let kind = kinds.next();

        if kinds.next().is_some() {
            // The mask is invalid.
            //
            // More than one of the bitflags are set
            return Err(EventMaskParseError::TooManyBitsSet(mask));
        }

        Ok(kind)
    }
}

impl TryFrom<EventMask> for Option<EventKind> {
    type Error = EventMaskParseError;

    fn try_from(value: EventMask) -> Result<Self, Self::Error> {
        EventKind::from_raw_event_mask(value)
    }
}

/// Auxiliary flags for inotify events
///
/// The non-mutually-exclusive bitflags that may be set
/// in an event read from an inotify fd. 0 or more of these
/// bitflags may be set.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Default)]
pub struct EventAuxiliaryFlags {
    /// Watch was removed when explicitly removed via [`inotify_rm_watch(2)`]
    /// or automatically (because the file was deleted or the filesystem was unmounted)
    ///
    /// [`inotify_rm_watch(2)`]: https://man7.org/linux/man-pages/man2/inotify_rm_watch.2.html
    pub ignored: bool,

    /// Event subject is a directory rather than a regular file
    pub isdir: bool,

    /// File system containing watched object was unmounted
    ///
    /// An event with **IN_IGNORED** will subsequently be generated for the same watch descriptor.
    pub unmount: bool,
}

impl EventAuxiliaryFlags {
    /// Parse the auxiliary flags from a raw event mask
    pub fn from_raw_event_mask(mask: EventMask) -> Self {
        EventAuxiliaryFlags {
            ignored: mask.contains(EventMask::IGNORED),
            isdir: mask.contains(EventMask::ISDIR),
            unmount: mask.contains(EventMask::UNMOUNT),
        }
    }
}

impl From<EventMask> for EventAuxiliaryFlags {
    fn from(value: EventMask) -> Self {
        Self::from_raw_event_mask(value)
    }
}

/// An error that occured from parsing an raw event mask
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventMaskParseError {
    /// More than one bit repesenting the event type was set
    TooManyBitsSet(EventMask),
    /// The event is a signal that the kernels event queue overflowed
    QueueOverflow,
}

impl Display for EventMaskParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyBitsSet(mask) => {
                writeln!(
                    f,
                    "Error parsing event mask: too many event type bits set | {mask:?}"
                )
            }
            Self::QueueOverflow => writeln!(f, "Error: the kernel's event queue overflowed"),
        }
    }
}

impl Error for EventMaskParseError {}

#[cfg(test)]
mod tests {
    use std::{io::prelude::*, mem, slice, sync};

    use inotify_sys as ffi;

    use crate::{EventMask, EventMaskParseError};

    use super::{Event, EventAuxiliaryFlags, EventKind, ParsedEventMask};

    #[test]
    fn from_buffer_should_not_mistake_next_event_for_name_of_previous_event() {
        let mut buffer = [0u8; 1024];

        // First, put a normal event into the buffer
        let event = ffi::inotify_event {
            wd: 0,
            mask: 0,
            cookie: 0,
            len: 0, // no name following after event
        };
        let event = unsafe {
            slice::from_raw_parts(&event as *const _ as *const u8, mem::size_of_val(&event))
        };
        (&mut buffer[..])
            .write_all(event)
            .expect("Failed to write into buffer");

        // After that event, simulate an event that starts with a non-zero byte.
        buffer[mem::size_of_val(event)] = 1;

        // Now create the event and verify that the name is actually `None`, as
        // dictated by the value `len` above.
        let (_, event) = Event::from_buffer(sync::Weak::new(), &buffer);
        assert_eq!(event.name, None);
    }

    #[test]
    fn parse_event_kinds() {
        // Parse each event kind
        for bf_map in EventKind::BITFLAG_ENUM_MAP {
            assert_eq!(
                Ok(ParsedEventMask {
                    kind: Some(bf_map.1),
                    auxiliary_flags: Default::default()
                }),
                bf_map.0.parse()
            );
        }

        // Parse an event with no event kind
        assert_eq!(
            Ok(ParsedEventMask {
                kind: None,
                auxiliary_flags: Default::default()
            }),
            EventMask::from_bits_retain(0).parse()
        )
    }

    #[test]
    fn parse_event_auxiliary_flags() {
        assert_eq!(
            Ok(ParsedEventMask {
                kind: None,
                auxiliary_flags: EventAuxiliaryFlags {
                    ignored: true,
                    isdir: false,
                    unmount: false
                }
            }),
            EventMask::IGNORED.parse()
        );

        assert_eq!(
            Ok(ParsedEventMask {
                kind: None,
                auxiliary_flags: EventAuxiliaryFlags {
                    ignored: false,
                    isdir: true,
                    unmount: false
                }
            }),
            EventMask::ISDIR.parse()
        );

        assert_eq!(
            Ok(ParsedEventMask {
                kind: None,
                auxiliary_flags: EventAuxiliaryFlags {
                    ignored: false,
                    isdir: false,
                    unmount: true
                }
            }),
            EventMask::UNMOUNT.parse()
        );
    }

    #[test]
    fn parse_event_errors() {
        assert_eq!(
            Err(EventMaskParseError::QueueOverflow),
            EventMask::Q_OVERFLOW.parse()
        );

        let mask = EventMask::ATTRIB | EventMask::ACCESS;
        assert_eq!(Err(EventMaskParseError::TooManyBitsSet(mask)), mask.parse());
    }
}
