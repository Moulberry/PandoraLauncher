use kqueue_sys::{constants::FilterFlag, kevent};

use crate::{Event, EventData, Ident, Vnode, Watcher};

type VnodeFlagsTableEntry = (FilterFlag, fn() -> Vnode);

static VNODE_FLAGS: &[VnodeFlagsTableEntry] = &[
    (FilterFlag::NOTE_WRITE, || Vnode::Write),
    (FilterFlag::NOTE_EXTEND, || Vnode::Extend),
    (FilterFlag::NOTE_ATTRIB, || Vnode::Attrib),
    (FilterFlag::NOTE_LINK, || Vnode::Link),
    (FilterFlag::NOTE_RENAME, || Vnode::Rename),
    (FilterFlag::NOTE_REVOKE, || Vnode::Revoke),
    #[cfg(target_os = "freebsd")]
    (FilterFlag::NOTE_CLOSE_WRITE, || Vnode::CloseWrite),
    #[cfg(target_os = "freebsd")]
    (FilterFlag::NOTE_CLOSE, || Vnode::Close),
    #[cfg(target_os = "freebsd")]
    (FilterFlag::NOTE_OPEN, || Vnode::Open),
    #[cfg(target_os = "freebsd")]
    (FilterFlag::NOTE_READ, || Vnode::Read),
    #[cfg(target_os = "openbsd")]
    (FilterFlag::NOTE_TRUNCATE, || Vnode::Truncate),
    (FilterFlag::NOTE_DELETE, || Vnode::Delete),
];

pub(crate) fn handle_event(ev: kevent, ident: &Ident, watcher: &Watcher) -> Option<Event> {
    let mut ret = None;

    // We have to do it this way, since FilterFlag bitflags values
    // are not unique, so bitflags.iter() could return values that
    // don't make any sense
    let mut vecdeque = watcher.decoalesced_events.lock().unwrap();
    for (fflag, ctor) in VNODE_FLAGS {
        if ev.fflags.contains(*fflag) {
            let inner = ctor();

            if ret.is_none() {
                ret = Some(Event {
                    ident: ident.clone(),
                    data: EventData::Vnode(inner),
                });
            } else {
                vecdeque.push_back(Event {
                    ident: ident.clone(),
                    data: EventData::Vnode(inner),
                });
            }
        }
    }

    ret
}
