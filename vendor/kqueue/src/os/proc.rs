use kqueue_sys::{kevent, FilterFlag};

use crate::{Event, EventData, Ident, Proc, Watcher};

type ProcFlagsTableEntry = (FilterFlag, fn(i64) -> Proc);

static PROC_FLAGS: &[ProcFlagsTableEntry] = &[
    (FilterFlag::NOTE_FORK, |_| Proc::Fork),
    (FilterFlag::NOTE_EXEC, |_| Proc::Exec),
    (FilterFlag::NOTE_TRACK, |data| {
        Proc::Track(data as libc::pid_t)
    }),
    (FilterFlag::NOTE_TRACKERR, |_| Proc::Trackerr),
    (FilterFlag::NOTE_CHILD, |data| {
        Proc::Child(data as libc::pid_t)
    }),
    (FilterFlag::NOTE_EXIT, |data| Proc::Exit(data as usize)),
];

pub(crate) fn handle_event(ev: kevent, ident: &Ident, watcher: &Watcher) -> Option<Event> {
    let mut ret = None;

    // We have to do it this way, since FilterFlag bitflags values
    // are not unique, so bitflags.iter() could return values that
    // don't make any sense
    let mut vecdeque = watcher.decoalesced_events.lock().unwrap();
    for (fflag, ctor) in PROC_FLAGS {
        if ev.fflags.contains(*fflag) {
            let inner = ctor(ev.data);
            if ret.is_none() {
                ret = Some(Event {
                    ident: ident.clone(),
                    data: EventData::Proc(inner),
                });
            } else {
                vecdeque.push_back(Event {
                    ident: ident.clone(),
                    data: EventData::Proc(inner),
                });
            }
        }
    }

    ret
}
