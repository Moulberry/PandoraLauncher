use std::{
    collections::HashMap,
    io::{self, ErrorKind},
    num::NonZeroU32,
    pin::Pin,
    sync::{Arc, Mutex as SyncMutex},
    task::{Context, Poll},
};

use async_broadcast::{Receiver, Sender, broadcast};
use futures_core::Future;
use futures_lite::Stream;
use ordered_stream::OrderedFuture;

use crate::{Error, Message, Result, message::Sequence};

#[derive(Clone, Debug)]
pub struct PendingMethodCalls {
    inner: Arc<SyncMutex<PendingMethodCallsState>>,
}

impl PendingMethodCalls {
    pub fn register_call(
        &self,
        serial: NonZeroU32,
    ) -> impl Future<Output = Result<Message>>
    + OrderedFuture<Output = Result<Message>, Ordering = Sequence> {
        use std::collections::hash_map::Entry;

        let (reply_sender, reply_receiver) = broadcast(1);
        let closed_error = {
            let mut state = self.inner.lock().unwrap();
            if let Some(error) = &state.closed_error {
                Some(error.clone())
            } else {
                match state.calls.entry(serial) {
                    Entry::Vacant(entry) => {
                        entry.insert(reply_sender.clone());
                    }
                    Entry::Occupied(_) => {
                        unreachable!(
                            "Serial number `{serial}` reused while a method call is still pending"
                        );
                    }
                }

                None
            }
        };

        if let Some(error) = closed_error {
            send_reply(reply_sender, Sequence::LAST, Err(error));
        }

        PendingMethodCall {
            serial,
            reply_receiver,
            pending_method_calls: self.clone(),
        }
    }

    pub fn complete_call(&self, serial: NonZeroU32, ordering: Sequence, reply: Result<Message>) {
        let reply_sender = self.inner.lock().unwrap().calls.remove(&serial);
        let Some(reply_sender) = reply_sender else {
            return;
        };

        send_reply(reply_sender, ordering, reply);
    }

    pub fn fail_all(&self, error: Error) {
        let reply_senders: Vec<_> = {
            let mut state = self.inner.lock().unwrap();
            state.closed_error.get_or_insert_with(|| error.clone());
            state
                .calls
                .drain()
                .map(|(_, reply_sender)| reply_sender)
                .collect()
        };

        for reply_sender in reply_senders {
            send_reply(reply_sender, Sequence::LAST, Err(error.clone()));
        }
    }

    fn remove_call(&self, serial: NonZeroU32) {
        self.inner.lock().unwrap().calls.remove(&serial);
    }
}

impl Default for PendingMethodCalls {
    fn default() -> Self {
        Self {
            inner: Arc::new(SyncMutex::new(PendingMethodCallsState::default())),
        }
    }
}

/// A method call whose completion can be awaited or joined with other streams.
///
/// This is useful for cache population method calls, where joining the call with an update signal
/// stream can be used to ensure that cache updates are not overwritten by a cache population whose
/// task is scheduled later.
#[derive(Debug)]
struct PendingMethodCall {
    serial: NonZeroU32,
    reply_receiver: Receiver<PendingMethodReply>,
    pending_method_calls: PendingMethodCalls,
}

impl Drop for PendingMethodCall {
    fn drop(&mut self) {
        self.pending_method_calls.remove_call(self.serial);
    }
}

impl Future for PendingMethodCall {
    type Output = Result<Message>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_before(cx, None).map(|ret| {
            ret.map(|(_, r)| r).unwrap_or_else(|| {
                Err(Error::InputOutput(
                    io::Error::new(ErrorKind::BrokenPipe, "socket closed").into(),
                ))
            })
        })
    }
}

impl OrderedFuture for PendingMethodCall {
    type Output = Result<Message>;
    type Ordering = Sequence;

    fn poll_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<Option<(Self::Ordering, Self::Output)>> {
        let this = self.get_mut();

        match Pin::new(&mut this.reply_receiver).poll_next(cx) {
            Poll::Ready(Some(reply)) => Poll::Ready(Some(reply)),
            Poll::Ready(None) => Poll::Ready(None),
            // `before` is only provided after another stream has produced that sequence. Since the
            // socket reader dispatches replies synchronously as it reads messages, any earlier
            // matching reply would already be queued above. For `OrderedFuture`, `Ready(None)` is
            // the `NoneBefore` equivalent; return it only after polling the receiver so the current
            // task is still woken when the reply arrives.
            Poll::Pending if before.is_some() => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Debug, Default)]
struct PendingMethodCallsState {
    calls: HashMap<NonZeroU32, PendingMethodReplySender>,
    closed_error: Option<Error>,
}

type PendingMethodReply = (Sequence, Result<Message>);
type PendingMethodReplySender = Sender<PendingMethodReply>;

fn send_reply(reply_sender: PendingMethodReplySender, ordering: Sequence, reply: Result<Message>) {
    let _ = reply_sender.try_broadcast((ordering, reply));
}
