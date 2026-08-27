use std::fmt;
use std::ptr;

use std::ffi::OsStr;
use std::io::Result;
use std::ops::Deref;
use std::os::unix::io::{AsRawFd, RawFd};

use io_lifetimes::{AsFd, BorrowedFd};
#[cfg(feature = "mio06")]
use mio::{event::Evented, unix::EventedFd, Poll, PollOpt, Ready, Token};
#[cfg(any(feature = "mio07", feature = "mio08", feature = "mio10"))]
use mio::{event::Source, unix::SourceFd, Interest, Registry, Token};

use Udev;
use {ffi, util};

use {AsRaw, AsRawWithContext, Device, FromRaw};

/// Monitors for device events.
///
/// A monitor communicates with the kernel over a socket. Filtering events is performed efficiently
/// in the kernel, and only events that match the filters are received by the socket. Filters must
/// be set up before listening for events.
pub struct Builder {
    udev: Udev,
    monitor: *mut ffi::udev_monitor,
}

impl Clone for Builder {
    fn clone(&self) -> Self {
        Self {
            udev: self.udev.clone(),
            monitor: unsafe { ffi::udev_monitor_ref(self.monitor) },
        }
    }
}

impl Drop for Builder {
    fn drop(&mut self) {
        unsafe {
            ffi::udev_monitor_unref(self.monitor);
        }
    }
}

#[cfg(feature = "send")]
unsafe impl Send for Builder {}
#[cfg(feature = "sync")]
unsafe impl Sync for Builder {}

as_ffi_with_context!(Builder, monitor, ffi::udev_monitor, ffi::udev_monitor_ref);

/// The event source to monitor.
pub(crate) enum EventSource {
    Udev,
    Kernel,
}

impl Builder {
    /// Creates a new udev event `Monitor`.
    pub fn new() -> Result<Self> {
        // Create a new Udev context for this monitor
        // It would be more efficient to allow callers to create just one context and use multiple
        // monitors, however that would be an API-breaking change.
        Self::with_udev(Udev::new()?)
    }

    /// Creates a new kernel event `Monitor`.
    pub fn new_kernel() -> Result<Self> {
        Self::with_kernel(Udev::new()?)
    }

    /// Creates a new `Monitor` using an existing `Udev` instance
    pub(crate) fn with_udev(udev: Udev) -> Result<Self> {
        Self::with_source(udev, EventSource::Udev)
    }

    /// Creates a new kernel event `Monitor` using an existing `Udev` instance.
    pub(crate) fn with_kernel(udev: Udev) -> Result<Self> {
        Self::with_source(udev, EventSource::Kernel)
    }

    /// Creates a `Monitor` for the given source, using an existing `Udev` instance.
    pub(crate) fn with_source(udev: Udev, source: EventSource) -> Result<Self> {
        let name = match source {
            EventSource::Udev => b"udev\0".as_ptr() as *const libc::c_char,
            EventSource::Kernel => b"kernel\0".as_ptr() as *const libc::c_char,
        };

        let ptr = try_alloc!(unsafe { ffi::udev_monitor_new_from_netlink(udev.as_raw(), name) });

        Ok(Self { udev, monitor: ptr })
    }

    /// Adds a filter that matches events for devices with the given subsystem.
    pub fn match_subsystem<T: AsRef<OsStr>>(self, subsystem: T) -> Result<Self> {
        let subsystem = util::os_str_to_cstring(subsystem)?;

        util::errno_to_result(unsafe {
            ffi::udev_monitor_filter_add_match_subsystem_devtype(
                self.monitor,
                subsystem.as_ptr(),
                ptr::null(),
            )
        })
        .and(Ok(self))
    }

    /// Adds a filter that matches events for devices with the given subsystem and device type.
    pub fn match_subsystem_devtype<T: AsRef<OsStr>, U: AsRef<OsStr>>(
        self,
        subsystem: T,
        devtype: U,
    ) -> Result<Self> {
        let subsystem = util::os_str_to_cstring(subsystem)?;
        let devtype = util::os_str_to_cstring(devtype)?;

        util::errno_to_result(unsafe {
            ffi::udev_monitor_filter_add_match_subsystem_devtype(
                self.monitor,
                subsystem.as_ptr(),
                devtype.as_ptr(),
            )
        })
        .and(Ok(self))
    }

    /// Adds a filter that matches events for devices with the given tag.
    pub fn match_tag<T: AsRef<OsStr>>(self, tag: T) -> Result<Self> {
        let tag = util::os_str_to_cstring(tag)?;

        util::errno_to_result(unsafe {
            ffi::udev_monitor_filter_add_match_tag(self.monitor, tag.as_ptr())
        })
        .and(Ok(self))
    }

    /// Removes all filters currently set on the monitor.
    pub fn clear_filters(self) -> Result<Self> {
        util::errno_to_result(unsafe { ffi::udev_monitor_filter_remove(self.monitor) })
            .and(Ok(self))
    }

    /// Listens for events matching the current filters.
    ///
    /// This method consumes the `Monitor`.
    pub fn listen(self) -> Result<Socket> {
        util::errno_to_result(unsafe { ffi::udev_monitor_enable_receiving(self.monitor) })?;

        Ok(Socket { inner: self })
    }
}

/// Provides raw access to the monitor's socket.
impl AsRawFd for Builder {
    /// Returns the file descriptor of the monitor's socket.
    fn as_raw_fd(&self) -> RawFd {
        unsafe { ffi::udev_monitor_get_fd(self.monitor) }
    }
}

/// Provides raw access to the monitor's socket.
impl AsFd for Builder {
    /// Returns the file descriptor of the monitor's socket.
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

/// An active monitor that can receive events.
///
/// The events received by a `Socket` match the filters set up by the `Monitor` that created
/// the socket.
///
/// Monitors are initially set up to receive events from the kernel via a nonblocking socket. A
/// variant of `poll()` should be used on the file descriptor returned by the `AsRawFd` trait to
/// wait for new events.
pub struct Socket {
    inner: Builder,
}

impl Socket {
    /// Create an iterator of socket event messages
    pub fn iter(&self) -> SocketIter {
        SocketIter::new(&self)
    }
}

impl AsRaw<ffi::udev_monitor> for Socket {
    fn as_raw(&self) -> *mut ffi::udev_monitor {
        self.inner.monitor
    }

    fn into_raw(self) -> *mut ffi::udev_monitor {
        self.inner.monitor
    }
}

/// Provides raw access to the monitor's socket.
impl AsRawFd for Socket {
    /// Returns the file descriptor of the monitor's socket.
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

/// Provides raw access to the monitor's socket.
impl AsFd for Socket {
    /// Returns the file descriptor of the monitor's socket.
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

/// Iterator of socket events
pub struct SocketIter<'a> {
    socket: &'a Socket,
}

impl<'a> SocketIter<'a> {
    /// Create a socket by cloning the underlying udev instance
    fn new(socket: &'a Socket) -> SocketIter<'a> {
        SocketIter { socket }
    }
}

impl<'a> Iterator for SocketIter<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        let ptr = unsafe { ffi::udev_monitor_receive_device(self.socket.inner.monitor) };

        if ptr.is_null() {
            None
        } else {
            let device = Device::from_raw(self.socket.inner.udev.clone(), ptr);
            Some(Event { device })
        }
    }
}

/// Types of events that can be received from udev.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// A device was added.
    Add,

    /// A device changed.
    Change,

    /// A device was removed.
    Remove,

    /// A device was bound to a driver.
    Bind,

    /// A device was unbound from a driver.
    Unbind,

    /// An unknown event occurred.
    Unknown,
}

impl Default for EventType {
    fn default() -> Self {
        EventType::Unknown
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            EventType::Add => "add",
            EventType::Change => "change",
            EventType::Remove => "remove",
            EventType::Bind => "bind",
            EventType::Unbind => "unbind",
            EventType::Unknown => "unknown",
        })
    }
}

/// An event that indicates a change in device state.
pub struct Event {
    device: Device,
}

impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Event")
            .field("device", &self.device())
            .field("event_type", &self.event_type())
            .field("sequence_number", &self.sequence_number())
            .finish()
    }
}

/// Provides access to the device associated with the event.
impl Deref for Event {
    type Target = Device;

    fn deref(&self) -> &Device {
        &self.device
    }
}

impl Event {
    /// Returns the `EventType` corresponding to this event.
    pub fn event_type(&self) -> EventType {
        let value = match self.device.property_value("ACTION") {
            Some(s) => s.to_str(),
            None => None,
        };

        match value {
            Some("add") => EventType::Add,
            Some("change") => EventType::Change,
            Some("remove") => EventType::Remove,
            Some("bind") => EventType::Bind,
            Some("unbind") => EventType::Unbind,
            _ => EventType::Unknown,
        }
    }

    /// Returns the event's sequence number.
    pub fn sequence_number(&self) -> u64 {
        unsafe { ffi::udev_device_get_seqnum(self.device.as_raw()) as u64 }
    }

    /// Returns the device associated with this event.
    pub fn device(&self) -> Device {
        self.device.clone()
    }
}

#[cfg(feature = "mio06")]
impl Evented for Socket {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> std::io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> std::io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}

#[cfg(any(feature = "mio07", feature = "mio08", feature = "mio10"))]
impl Source for Socket {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> std::io::Result<()> {
        SourceFd(&self.as_raw_fd()).register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> std::io::Result<()> {
        SourceFd(&self.as_raw_fd()).reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &Registry) -> std::io::Result<()> {
        SourceFd(&self.as_raw_fd()).deregister(registry)
    }
}
