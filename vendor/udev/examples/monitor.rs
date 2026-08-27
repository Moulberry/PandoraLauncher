extern crate libc;
#[cfg(feature = "mio06")]
extern crate mio06;
#[cfg(feature = "mio07")]
extern crate mio07;
#[cfg(feature = "mio08")]
extern crate mio08;
#[cfg(feature = "mio10")]
extern crate mio10;
extern crate udev;

use std::io;

#[cfg(not(any(
    feature = "mio06",
    feature = "mio07",
    feature = "mio08",
    feature = "mio10"
)))]
mod poll {
    use std::io;
    use std::ptr;
    use std::thread;
    use std::time::Duration;

    use std::os::unix::io::AsRawFd;

    use libc::{c_int, c_short, c_ulong, c_void};

    #[repr(C)]
    #[allow(non_camel_case_types)]
    struct pollfd {
        fd: c_int,
        events: c_short,
        revents: c_short,
    }

    #[repr(C)]
    #[allow(non_camel_case_types)]
    struct sigset_t {
        __private: c_void,
    }

    #[allow(non_camel_case_types)]
    type nfds_t = c_ulong;

    const POLLIN: c_short = 0x0001;

    extern "C" {
        fn ppoll(
            fds: *mut pollfd,
            nfds: nfds_t,
            timeout_ts: *mut libc::timespec,
            sigmask: *const sigset_t,
        ) -> c_int;
    }

    pub fn poll(socket: udev::MonitorSocket) -> io::Result<()> {
        println!("Use syspoll");
        let mut fds = vec![pollfd {
            fd: socket.as_raw_fd(),
            events: POLLIN,
            revents: 0,
        }];

        loop {
            let result = unsafe {
                ppoll(
                    (&mut fds[..]).as_mut_ptr(),
                    fds.len() as nfds_t,
                    ptr::null_mut(),
                    ptr::null(),
                )
            };

            if result < 0 {
                return Err(io::Error::last_os_error());
            }

            let event = match socket.iter().next() {
                Some(evt) => evt,
                None => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
            };

            super::print_event(event);
        }
    }
}

#[cfg(feature = "mio06")]
mod poll {
    use std::io;

    use mio06::{Events, Poll, PollOpt, Ready, Token};

    pub fn poll(mut socket: udev::MonitorSocket) -> io::Result<()> {
        println!("Use mio06 poll");

        let poll = Poll::new()?;
        let mut events = Events::with_capacity(1024);

        poll.register(
            &mut socket,
            Token(0),
            Ready::readable() | Ready::writable(),
            PollOpt::edge(),
        )?;

        loop {
            poll.poll(&mut events, None)?;

            for event in &events {
                if event.token() == Token(0) && event.readiness().is_writable() {
                    socket.iter().for_each(|x| super::print_event(x));
                }
            }
        }
    }
}

#[cfg(any(feature = "mio07", feature = "mio08", feature = "mio10"))]
mod poll {
    use std::io;

    #[cfg(feature = "mio07")]
    use mio07::{Events, Interest, Poll, Token};
    #[cfg(feature = "mio08")]
    use mio08::{Events, Interest, Poll, Token};
    #[cfg(feature = "mio10")]
    use mio10::{Events, Interest, Poll, Token};

    pub fn poll(mut socket: udev::MonitorSocket) -> io::Result<()> {
        let version = if cfg!(feature = "mio07") {
            "mio07"
        } else if cfg!(feature = "mio08") {
            "mio08"
        } else if cfg!(feature = "mio10") {
            "mio10"
        } else {
            "mio-unknown"
        };
        println!("Use {} poll", version);

        let mut poll = Poll::new()?;
        let mut events = Events::with_capacity(1024);

        poll.registry().register(
            &mut socket,
            Token(0),
            Interest::READABLE | Interest::WRITABLE,
        )?;

        loop {
            poll.poll(&mut events, None)?;

            for event in &events {
                if event.token() == Token(0) && event.is_writable() {
                    socket.iter().for_each(|x| super::print_event(x));
                }
            }
        }
    }
}

fn print_event(event: udev::Event) {
    println!(
        "{}: {} {} (subsystem={}, sysname={}, devtype={})",
        event.sequence_number(),
        event.event_type(),
        event.syspath().to_str().unwrap_or("---"),
        event
            .subsystem()
            .map_or("", |s| { s.to_str().unwrap_or("") }),
        event.sysname().to_str().unwrap_or(""),
        event.devtype().map_or("", |s| { s.to_str().unwrap_or("") })
    );
}

// Use `mio::poll` as poller by compile with:
// `cargo run --example monitor --features "mio10"`
fn main() -> io::Result<()> {
    let socket = udev::MonitorBuilder::new()?
        // .match_subsystem_devtype("usb", "usb_device")?
        .match_subsystem_devtype("block", "disk")?
        .listen()?;

    poll::poll(socket)
}
