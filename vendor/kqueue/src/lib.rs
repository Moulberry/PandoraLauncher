use kqueue_sys::{kevent, kqueue};
use libc::{close, pid_t, uintptr_t};
use std::fmt::Debug;
use std::fs::File;
use std::io::{self, Error, Result};
use std::os::unix::io::{AsRawFd, IntoRawFd, RawFd};
use std::path::Path;
use std::ptr;
use std::time::Duration;

pub use kqueue_sys::constants::*;

mod os;
use crate::os::vnode;

mod time;
use crate::time::duration_to_timespec;

/// The watched object that fired the event
#[derive(Debug, Eq, Clone)]
pub enum Ident {
    Filename(RawFd, String),
    Fd(RawFd),
    Pid(pid_t),
    Signal(i32),
    Timer(i32),
}

#[doc(hidden)]
#[derive(Debug, PartialEq, Clone)]
pub struct Watched {
    filter: EventFilter,
    flags: FilterFlag,
    ident: Ident,
}

/// Watches one or more resources
///
/// These can be created with `Watcher::new()`. You can create as many
/// `Watcher`s as you want, and they can watch as many objects as you wish.
/// The objects do not need to be the same type.
///
/// Each `Watcher` is backed by a `kqueue(2)` queue. These resources are freed
/// on the `Watcher`s destruction. If the destructor cannot run for whatever
/// reason, the underlying kernel object will be leaked.
///
/// Files and file descriptors given to the `Watcher` are presumed to be owned
/// by the `Watcher`, and will be closed when they're removed from the `Watcher`
/// or on `Drop`. In a future version, the API will make this explicit via
/// `OwnedFd`s
#[derive(Debug)]
pub struct Watcher {
    watched: Vec<Watched>,
    queue: RawFd,
    started: bool,
    opts: KqueueOpts,
}

/// Vnode events
///
/// These are OS-specific, and may not all be supported on your platform. Check
/// `kqueue(2)` for more information.
#[derive(Debug)]
#[non_exhaustive]
pub enum Vnode {
    /// The file was deleted
    Delete,

    /// The file received a write
    Write,

    /// The file was extended with `truncate(2)`
    Extend,

    /// The file was shrunk with `truncate(2)`
    Truncate,

    /// The attributes of the file were changed
    Attrib,

    /// The link count of the file was changed
    Link,

    /// The file was renamed
    Rename,

    /// Access to the file was revoked with `revoke(2)` or the fs was unmounted
    Revoke,

    /// File was opened by a process (FreeBSD-specific)
    Open,

    /// File was closed and the descriptor had write access (FreeBSD-specific)
    CloseWrite,

    /// File was closed and the descriptor had read access (FreeBSD-specific)
    Close,
}

/// Process events
///
/// These are OS-specific, and may not all be supported on your platform. Check
/// `kqueue(2)` for more information.
#[derive(Debug)]
pub enum Proc {
    /// The watched process exited with the returned exit code
    Exit(usize),

    /// The process called `fork(2)`
    Fork,

    /// The process called `exec(2)`
    Exec,

    /// The process called `fork(2)`, and returned the child pid.
    Track(libc::pid_t),

    /// The process called `fork(2)`, but we were not able to track the child
    Trackerr,

    /// The process called `fork(2)`, and returned the child pid.
    // TODO: this is FreeBSD-specific. We can probably convert this to `Track`.
    Child(libc::pid_t),
}

/// Event-specific data returned with the event.
///
/// Like much of this library, this is OS-specific. Check `kqueue(2)` for more
/// details on your target OS.
#[derive(Debug)]
pub enum EventData {
    /// Data relating to `Vnode` events
    Vnode(Vnode),

    /// Data relating to process events
    Proc(Proc),

    /// The returned number of bytes are ready for reading from the watched
    /// descriptor
    ReadReady(usize),

    /// The file is ready for writing. On some files (like sockets, pipes, etc),
    /// the number of bytes in the write buffer will be returned.
    WriteReady(usize),

    /// One of the watched signals fired. The number of times this signal was received
    /// is returned.
    Signal(usize),

    /// One of the watched timers fired. The number of times this timer fired
    /// is returned.
    Timer(usize),

    /// Some error was received
    Error(Error),
}

/// An event from a `Watcher` object.
///
/// An event contains both the a signifier of the watched object that triggered
/// the event, as well as any event-specific. See the `EventData` enum for info
/// on what event-specific data is returned for each event.
#[derive(Debug)]
pub struct Event {
    /// The watched resource that triggered the event
    pub ident: Ident,

    /// Any event-specific data returned with the event.
    pub data: EventData,
}

pub struct EventIter<'a> {
    watcher: &'a Watcher,
}

/// Options for a `Watcher`
#[derive(Debug)]
pub struct KqueueOpts {
    /// Clear state on watched objects
    clear: bool,
}

impl Default for KqueueOpts {
    /// Returns the default options for a `Watcher`
    ///
    /// `clear` is set to `true`
    fn default() -> KqueueOpts {
        KqueueOpts { clear: true }
    }
}

// We don't have enough information to turn a `usize` into
// an `Ident`, so we only implement `Into<usize>` here.
#[allow(clippy::from_over_into)]
impl Into<usize> for Ident {
    fn into(self) -> usize {
        match self {
            Ident::Filename(fd, _) => fd as usize,
            Ident::Fd(fd) => fd as usize,
            Ident::Pid(pid) => pid as usize,
            Ident::Signal(sig) => sig as usize,
            Ident::Timer(timer) => timer as usize,
        }
    }
}

impl PartialEq<Ident> for Ident {
    fn eq(&self, other: &Ident) -> bool {
        match *self {
            Ident::Filename(_, ref name) => {
                if let Ident::Filename(_, ref othername) = *other {
                    name == othername
                } else {
                    false
                }
            }
            _ => self.as_usize() == other.as_usize(),
        }
    }
}

impl Ident {
    fn as_usize(&self) -> usize {
        match *self {
            Ident::Filename(fd, _) => fd as usize,
            Ident::Fd(fd) => fd as usize,
            Ident::Pid(pid) => pid as usize,
            Ident::Signal(sig) => sig as usize,
            Ident::Timer(timer) => timer as usize,
        }
    }
}

impl Watcher {
    /// Creates a new `Watcher`
    ///
    /// Creates a brand new `Watcher` with `KqueueOpts::default()`. Will return
    /// an `io::Error` if creation fails.
    pub fn new() -> Result<Watcher> {
        let queue = unsafe { kqueue() };

        if queue == -1 {
            Err(Error::last_os_error())
        } else {
            Ok(Watcher {
                watched: Vec::new(),
                queue,
                started: false,
                opts: Default::default(),
            })
        }
    }

    /// Disables the `clear` flag on a `Watcher`. New events will no longer
    /// be added with the `EV_CLEAR` flag on `watch`.
    pub fn disable_clears(&mut self) -> &mut Self {
        self.opts.clear = false;
        self
    }

    /// Adds a `pid` to the `Watcher` to be watched
    pub fn add_pid(
        &mut self,
        pid: libc::pid_t,
        filter: EventFilter,
        flags: FilterFlag,
    ) -> Result<()> {
        let watch = Watched {
            filter,
            flags,
            ident: Ident::Pid(pid),
        };

        if !self.watched.contains(&watch) {
            self.watched.push(watch);
        }

        Ok(())
    }

    /// Adds a file by filename to be watched
    ///
    /// **NB**: `kqueue(2)` is an `fd`-based API. If you add a filename with
    /// `add_filename`, internally we open it and pass the file descriptor to
    /// `kqueue(2)`. If the file is moved or deleted, and a new file is created
    /// with the same name, you will not receive new events for it without
    /// calling `add_filename` again.
    ///
    /// TODO: Adding new files requires calling `Watcher.watch` again
    pub fn add_filename<P: AsRef<Path>>(
        &mut self,
        filename: P,
        filter: EventFilter,
        flags: FilterFlag,
    ) -> Result<()> {
        let file = File::open(filename.as_ref())?;
        let watch = Watched {
            filter,
            flags,
            ident: Ident::Filename(
                file.into_raw_fd(),
                filename.as_ref().to_string_lossy().into_owned(),
            ),
        };

        if !self.watched.contains(&watch) {
            self.watched.push(watch);
        }

        Ok(())
    }

    /// Adds a descriptor to a `Watcher`. This or `add_file` is the preferred
    /// way to watch a file
    ///
    /// TODO: Adding new files requires calling `Watcher.watch` again
    pub fn add_fd(&mut self, fd: RawFd, filter: EventFilter, flags: FilterFlag) -> Result<()> {
        let watch = Watched {
            filter,
            flags,
            ident: Ident::Fd(fd),
        };

        if !self.watched.contains(&watch) {
            self.watched.push(watch);
        }

        Ok(())
    }

    /// Adds a `File` to a `Watcher`. This, or `add_fd` is the preferred way
    /// to watch a file
    ///
    /// TODO: Adding new files requires calling `Watcher.watch` again
    pub fn add_file(&mut self, file: &File, filter: EventFilter, flags: FilterFlag) -> Result<()> {
        self.add_fd(file.as_raw_fd(), filter, flags)
    }

    fn delete_kevents(&self, ident: Ident, filter: EventFilter) -> Result<()> {
        let kev = &[kevent::new(
            ident.as_usize(),
            filter,
            EventFlag::EV_DELETE,
            FilterFlag::empty(),
        )];

        let ret = unsafe {
            kevent(
                self.queue,
                kev.as_ptr(),
                // On NetBSD, this is passed as a usize, not i32
                #[allow(clippy::useless_conversion)]
                i32::try_from(kev.len()).unwrap().try_into().unwrap(),
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        };

        match ret {
            -1 => Err(Error::last_os_error()),
            _ => Ok(()),
        }
    }

    /// Removes a pid from a `Watcher`
    pub fn remove_pid(&mut self, pid: libc::pid_t, filter: EventFilter) -> Result<()> {
        let new_watched = self
            .watched
            .drain(..)
            .filter(|x| {
                if let Ident::Pid(iterpid) = x.ident {
                    iterpid != pid
                } else {
                    true
                }
            })
            .collect();

        self.watched = new_watched;
        self.delete_kevents(Ident::Pid(pid), filter)
    }

    /// Removes a filename from a `Watcher`.
    ///
    /// *NB*: This matches the `filename` that this item was initially added under.
    /// If a file has been moved, it will not be removable by the new name.
    pub fn remove_filename<P: AsRef<Path> + Debug>(
        &mut self,
        filename: P,
        filter: EventFilter,
    ) -> Result<()> {
        let mut fd: RawFd = 0;
        let new_watched = self
            .watched
            .drain(..)
            .filter(|x| {
                if let Ident::Filename(iterfd, ref iterfile) = x.ident {
                    if iterfile == filename.as_ref().to_str().unwrap() {
                        fd = iterfd;
                        false
                    } else {
                        true
                    }
                } else {
                    true
                }
            })
            .collect();

        if fd == 0 {
            return Err(Error::new(
                io::ErrorKind::NotFound,
                format!("{filename:?} was not being watched"),
            ));
        }

        self.watched = new_watched;
        let ret = self.delete_kevents(Ident::Fd(fd), filter);
        let close_err = unsafe { close(fd) };
        if close_err != 0 {
            Err(Error::from_raw_os_error(close_err))
        } else {
            ret
        }
    }

    /// Removes an fd from a `Watcher`. This closes the fd.
    pub fn remove_fd(&mut self, fd: RawFd, filter: EventFilter) -> Result<()> {
        let new_watched = self
            .watched
            .drain(..)
            .filter(|x| {
                if let Ident::Fd(iterfd) = x.ident {
                    iterfd != fd
                } else {
                    true
                }
            })
            .collect();

        self.watched = new_watched;
        let ret = self.delete_kevents(Ident::Fd(fd), filter);
        let close_err = unsafe { close(fd) };
        if close_err != 0 {
            Err(Error::from_raw_os_error(close_err))
        } else {
            ret
        }
    }

    /// Removes a `File` from a `Watcher`
    pub fn remove_file(&mut self, file: &File, filter: EventFilter) -> Result<()> {
        self.remove_fd(file.as_raw_fd(), filter)
    }

    /// Starts watching for events from `kqueue(2)`. This function needs to
    /// be called before `Watcher.iter()` or `Watcher.poll()` to actually
    /// start listening for events.
    pub fn watch(&mut self) -> Result<()> {
        let kevs: Vec<kevent> = self
            .watched
            .iter()
            .map(|watched| {
                let raw_ident = match watched.ident {
                    Ident::Fd(fd) => fd as uintptr_t,
                    Ident::Filename(fd, _) => fd as uintptr_t,
                    Ident::Pid(pid) => pid as uintptr_t,
                    Ident::Signal(sig) => sig as uintptr_t,
                    Ident::Timer(ident) => ident as uintptr_t,
                };

                kevent::new(
                    raw_ident,
                    watched.filter,
                    if self.opts.clear {
                        EventFlag::EV_ADD | EventFlag::EV_CLEAR
                    } else {
                        EventFlag::EV_ADD
                    },
                    watched.flags,
                )
            })
            .collect();

        let ret = unsafe {
            kevent(
                self.queue,
                kevs.as_ptr(),
                // On NetBSD, this is passed as a usize, not i32
                #[allow(clippy::useless_conversion)]
                i32::try_from(kevs.len()).unwrap().try_into().unwrap(),
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        };

        self.started = true;
        match ret {
            -1 => Err(Error::last_os_error()),
            _ => Ok(()),
        }
    }

    /// Polls for a new event, with an optional timeout. If no `timeout`
    /// is passed, then it will return immediately.
    pub fn poll(&self, timeout: Option<Duration>) -> Option<Event> {
        // poll will not block indefinitely
        // None -> return immediately
        match timeout {
            Some(timeout) => get_event(self, Some(timeout)),
            None => get_event(self, Some(Duration::new(0, 0))),
        }
    }

    /// Polls for a new event, with an optional timeout. If no `timeout`
    /// is passed, then it will block until an event is received.
    pub fn poll_forever(&self, timeout: Option<Duration>) -> Option<Event> {
        if timeout.is_some() {
            self.poll(timeout)
        } else {
            get_event(self, None)
        }
    }

    /// Creates an iterator that iterates over the queue. This iterator will block
    /// until a new event is received.
    pub fn iter(&self) -> EventIter<'_> {
        EventIter { watcher: self }
    }
}

impl AsRawFd for Watcher {
    fn as_raw_fd(&self) -> RawFd {
        self.queue
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        unsafe { libc::close(self.queue) };
        for watched in &self.watched {
            match watched.ident {
                Ident::Fd(fd) => unsafe { libc::close(fd) },
                Ident::Filename(fd, _) => unsafe { libc::close(fd) },
                _ => continue,
            };
        }
    }
}

fn find_file_ident(watcher: &Watcher, fd: RawFd) -> Option<Ident> {
    for watched in &watcher.watched {
        match watched.ident.clone() {
            Ident::Fd(ident_fd) => {
                if fd == ident_fd {
                    return Some(Ident::Fd(fd));
                } else {
                    continue;
                }
            }
            Ident::Filename(ident_fd, ident_str) => {
                if fd == ident_fd {
                    return Some(Ident::Filename(ident_fd, ident_str));
                } else {
                    continue;
                }
            }
            _ => continue,
        }
    }

    None
}

fn get_event(watcher: &Watcher, timeout: Option<Duration>) -> Option<Event> {
    let mut kev = kevent::new(
        0,
        EventFilter::EVFILT_SYSCOUNT,
        EventFlag::empty(),
        FilterFlag::empty(),
    );
    let ret = if let Some(ts) = timeout {
        unsafe {
            kevent(
                watcher.queue,
                ptr::null(),
                0,
                &mut kev,
                1,
                &duration_to_timespec(ts),
            )
        }
    } else {
        unsafe { kevent(watcher.queue, ptr::null(), 0, &mut kev, 1, ptr::null()) }
    };

    match ret {
        -1 => Some(Event::from_error(kev, watcher)),
        0 => None, // timeout expired
        _ => Some(Event::new(kev, watcher)),
    }
}

// OS specific
// TODO: Events can have more than one filter flag
impl Event {
    #[doc(hidden)]
    pub fn new(ev: kevent, watcher: &Watcher) -> Event {
        let data = match ev.filter {
            EventFilter::EVFILT_READ => EventData::ReadReady(ev.data as usize),
            EventFilter::EVFILT_WRITE => EventData::WriteReady(ev.data as usize),
            EventFilter::EVFILT_SIGNAL => EventData::Signal(ev.data as usize),
            EventFilter::EVFILT_TIMER => EventData::Timer(ev.data as usize),
            EventFilter::EVFILT_PROC => {
                let inner = if ev.fflags.contains(FilterFlag::NOTE_EXIT) {
                    Proc::Exit(ev.data as usize)
                } else if ev.fflags.contains(FilterFlag::NOTE_FORK) {
                    Proc::Fork
                } else if ev.fflags.contains(FilterFlag::NOTE_EXEC) {
                    Proc::Exec
                } else if ev.fflags.contains(FilterFlag::NOTE_TRACK) {
                    Proc::Track(ev.data as libc::pid_t)
                } else if ev.fflags.contains(FilterFlag::NOTE_CHILD) {
                    Proc::Child(ev.data as libc::pid_t)
                } else {
                    panic!("proc filterflag not supported: {0:?}", ev.fflags)
                };

                EventData::Proc(inner)
            }
            EventFilter::EVFILT_VNODE => {
                let inner = if ev.fflags.contains(FilterFlag::NOTE_DELETE) {
                    Vnode::Delete
                } else if ev.fflags.contains(FilterFlag::NOTE_WRITE) {
                    Vnode::Write
                } else if ev.fflags.contains(FilterFlag::NOTE_EXTEND) {
                    Vnode::Extend
                } else if ev.fflags.contains(FilterFlag::NOTE_ATTRIB) {
                    Vnode::Attrib
                } else if ev.fflags.contains(FilterFlag::NOTE_LINK) {
                    Vnode::Link
                } else if ev.fflags.contains(FilterFlag::NOTE_RENAME) {
                    Vnode::Rename
                } else if ev.fflags.contains(FilterFlag::NOTE_REVOKE) {
                    Vnode::Revoke
                } else {
                    // This handles any filter flags that are OS-specific
                    vnode::handle_vnode_extras(ev.fflags)
                };

                EventData::Vnode(inner)
            }
            _ => panic!("eventfilter not supported: {0:?}", ev.filter),
        };

        let ident = match ev.filter {
            EventFilter::EVFILT_READ => find_file_ident(watcher, ev.ident as RawFd).unwrap(),
            EventFilter::EVFILT_WRITE => find_file_ident(watcher, ev.ident as RawFd).unwrap(),
            EventFilter::EVFILT_VNODE => find_file_ident(watcher, ev.ident as RawFd).unwrap(),
            EventFilter::EVFILT_SIGNAL => Ident::Signal(ev.ident as i32),
            EventFilter::EVFILT_TIMER => Ident::Timer(ev.ident as i32),
            EventFilter::EVFILT_PROC => Ident::Pid(ev.ident as pid_t),
            _ => panic!("not supported"),
        };

        Event { ident, data }
    }

    #[doc(hidden)]
    pub fn from_error(ev: kevent, watcher: &Watcher) -> Event {
        let ident = match ev.filter {
            EventFilter::EVFILT_READ => find_file_ident(watcher, ev.ident as RawFd).unwrap(),
            EventFilter::EVFILT_WRITE => find_file_ident(watcher, ev.ident as RawFd).unwrap(),
            EventFilter::EVFILT_VNODE => find_file_ident(watcher, ev.ident as RawFd).unwrap(),
            EventFilter::EVFILT_SIGNAL => Ident::Signal(ev.ident as i32),
            EventFilter::EVFILT_TIMER => Ident::Timer(ev.ident as i32),
            EventFilter::EVFILT_PROC => Ident::Pid(ev.ident as pid_t),
            _ => panic!("not supported"),
        };

        Event {
            data: EventData::Error(io::Error::last_os_error()),
            ident,
        }
    }

    #[doc(hidden)]
    pub fn is_err(&self) -> bool {
        matches!(self.data, EventData::Error(_))
    }
}

impl Iterator for EventIter<'_> {
    type Item = Event;

    // rather than call kevent(2) each time, we can likely optimize and
    // call it once for like 100 items
    fn next(&mut self) -> Option<Self::Item> {
        if !self.watcher.started {
            return None;
        }

        get_event(self.watcher, None)
    }
}

#[cfg(test)]
mod tests {
    use super::{EventData, EventFilter, FilterFlag, Ident, Vnode, Watcher};
    use std::fs;
    use std::io::{ErrorKind, Write};
    use std::os::unix::io::{AsRawFd, FromRawFd};
    use std::path::Path;
    use std::thread;
    use std::time;

    #[cfg(target_os = "freebsd")]
    use std::process;

    #[test]
    fn test_new_watcher() {
        let mut watcher = Watcher::new().expect("new failed");
        let file = tempfile::tempfile().expect("Couldn't create tempfile");

        watcher
            .add_file(&file, EventFilter::EVFILT_VNODE, FilterFlag::NOTE_WRITE)
            .expect("add failed");
        watcher.watch().expect("watch failed");
    }

    #[test]
    fn test_filename() {
        let mut watcher = Watcher::new().expect("new failed");
        let file = tempfile::NamedTempFile::new().expect("Couldn't create tempfile");

        watcher
            .add_filename(
                file.path(),
                EventFilter::EVFILT_VNODE,
                FilterFlag::NOTE_WRITE,
            )
            .expect("add failed");
        watcher.watch().expect("watch failed");

        let mut new_file = fs::OpenOptions::new()
            .write(true)
            .open(file.path())
            .expect("open failed");

        new_file.write_all(b"foo").expect("write failed");

        thread::sleep(time::Duration::from_secs(1));

        let ev = watcher.iter().next().expect("Could not get a watch");
        assert!(matches!(ev.data, EventData::Vnode(Vnode::Write)));

        match ev.ident {
            Ident::Filename(_, name) => assert!(Path::new(&name) == file.path()),
            _ => panic!(),
        };
    }

    #[test]
    fn test_file() {
        let mut watcher = Watcher::new().expect("new failed");
        let mut file = tempfile::tempfile().expect("Could not create tempfile");

        watcher
            .add_file(&file, EventFilter::EVFILT_VNODE, FilterFlag::NOTE_WRITE)
            .expect("add failed");
        watcher.watch().expect("watch failed");
        file.write_all(b"foo").expect("write failed");

        thread::sleep(time::Duration::from_secs(1));

        let ev = watcher.iter().next().expect("Didn't get an event");

        assert!(matches!(ev.data, EventData::Vnode(Vnode::Write)));
        assert!(matches!(ev.ident, Ident::Fd(_)));
    }

    #[test]
    fn test_delete_filename() {
        let mut watcher = Watcher::new().expect("new failed");

        let file = tempfile::NamedTempFile::new().expect("Could not create tempfile");
        let filename = file.path();

        watcher
            .add_filename(filename, EventFilter::EVFILT_VNODE, FilterFlag::NOTE_WRITE)
            .expect("add failed");
        watcher.watch().expect("watch failed");
        watcher
            .remove_filename(filename, EventFilter::EVFILT_VNODE)
            .expect("delete failed");
    }

    #[test]
    fn test_dupe() {
        let mut watcher = Watcher::new().expect("new failed");
        let file = tempfile::NamedTempFile::new().expect("Couldn't create tempfile");
        let filename = file.path();

        watcher
            .add_filename(filename, EventFilter::EVFILT_VNODE, FilterFlag::NOTE_WRITE)
            .expect("add failed");
        watcher
            .add_filename(filename, EventFilter::EVFILT_VNODE, FilterFlag::NOTE_WRITE)
            .expect("second add failed");

        assert_eq!(
            watcher.watched.len(),
            1,
            "Did not get an expected number of events"
        );
    }

    #[test]
    fn test_two_files() {
        let mut watcher = Watcher::new().expect("new failed");

        let mut first_file = tempfile::tempfile().expect("Unable to create first temporary file");
        let mut second_file = tempfile::tempfile().expect("Unable to create second temporary file");

        watcher
            .add_file(
                &first_file,
                EventFilter::EVFILT_VNODE,
                FilterFlag::NOTE_WRITE,
            )
            .expect("add failed");

        watcher
            .add_file(
                &second_file,
                EventFilter::EVFILT_VNODE,
                FilterFlag::NOTE_WRITE,
            )
            .expect("add failed");

        watcher.watch().expect("watch failed");
        first_file.write_all(b"foo").expect("first write failed");
        second_file.write_all(b"foo").expect("second write failed");

        thread::sleep(time::Duration::from_secs(1));

        watcher.iter().next().expect("didn't get any events");
        watcher.iter().next().expect("didn't get any events");
    }

    #[test]
    fn test_nested_kqueue() {
        let mut watcher = Watcher::new().expect("Failed to create main watcher");
        let mut nested_watcher = Watcher::new().expect("Failed to create nested watcher");

        let kqueue_file = unsafe { fs::File::from_raw_fd(nested_watcher.as_raw_fd()) };
        watcher
            .add_file(&kqueue_file, EventFilter::EVFILT_READ, FilterFlag::empty())
            .expect("add_file failed for main watcher");

        let mut file = tempfile::tempfile().expect("Couldn't create tempfile");
        nested_watcher
            .add_file(&file, EventFilter::EVFILT_VNODE, FilterFlag::NOTE_WRITE)
            .expect("add_file failed for nested watcher");

        watcher.watch().expect("watch failed on main watcher");
        nested_watcher
            .watch()
            .expect("watch failed on nested watcher");

        file.write_all(b"foo").expect("write failed");

        thread::sleep(time::Duration::from_secs(1));

        watcher.iter().next().expect("didn't get any events");
        nested_watcher.iter().next().expect("didn't get any events");
    }

    #[test]
    #[cfg(target_os = "freebsd")]
    fn test_close_read() {
        let mut watcher = Watcher::new().expect("new failed");

        {
            let file = tempfile::NamedTempFile::new().expect("temporary file failed to create");
            watcher
                .add_filename(
                    file.path(),
                    EventFilter::EVFILT_VNODE,
                    FilterFlag::NOTE_CLOSE,
                )
                .expect("add failed");
            watcher.watch().expect("watch failed");

            // we launch this in a separate process since it appears that FreeBSD does not fire
            // off a NOTE_CLOSE(_WRITE)? event for the same process closing a file descriptor.
            process::Command::new("cat")
                .arg(file.path())
                .spawn()
                .expect("should spawn a file");
            thread::sleep(time::Duration::from_secs(1));
        }
        let ev = watcher.iter().next().expect("did not receive event");
        assert!(matches!(ev.data, EventData::Vnode(Vnode::Close)));
    }

    #[test]
    #[cfg(target_os = "freebsd")]
    fn test_close_write() {
        let mut watcher = match Watcher::new() {
            Ok(wat) => wat,
            Err(_) => panic!("new failed"),
        };

        {
            let file = tempfile::NamedTempFile::new().expect("couldn't create tempfile");
            watcher
                .add_filename(
                    file.path(),
                    EventFilter::EVFILT_VNODE,
                    FilterFlag::NOTE_CLOSE_WRITE,
                )
                .expect("add failed");
            watcher.watch().expect("watch failed");

            // See above for rationale as to why we use a separate process here
            process::Command::new("cat")
                .arg(file.path())
                .spawn()
                .expect("should spawn a file");
            thread::sleep(time::Duration::from_secs(1));
        }
        let ev = watcher.iter().next().expect("didn't get an event");
        assert!(matches!(ev.data, EventData::Vnode(Vnode::CloseWrite)));
    }

    #[test]
    fn test_not_found_remove_watch() {
        let mut watcher = Watcher::new().unwrap();

        let ret = watcher.remove_filename("foo", EventFilter::EVFILT_VNODE);
        assert!(ret.is_err());

        let err = ret.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.to_string(), "\"foo\" was not being watched");
    }
}
