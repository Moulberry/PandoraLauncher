use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use event_listener::Event;
use tracing::{debug, instrument, trace};

use crate::{
    Executor, Message, OwnedMatchRule, Task,
    async_lock::Mutex,
    connection::{MsgBroadcaster, PendingMethodCalls},
    message::Type,
};

use super::socket::ReadHalf;

#[derive(Debug)]
pub(crate) struct SocketReader {
    socket: Box<dyn ReadHalf>,
    senders: Arc<Mutex<HashMap<Option<OwnedMatchRule>, MsgBroadcaster>>>,
    pending_method_calls: PendingMethodCalls,
    already_received_bytes: Vec<u8>,
    #[cfg(unix)]
    already_received_fds: Vec<std::os::fd::OwnedFd>,
    prev_seq: u64,
    socket_status: Arc<SocketStatus>,
}

impl SocketReader {
    pub fn new(
        socket: Box<dyn ReadHalf>,
        senders: Arc<Mutex<HashMap<Option<OwnedMatchRule>, MsgBroadcaster>>>,
        pending_method_calls: PendingMethodCalls,
        already_received_bytes: Vec<u8>,
        #[cfg(unix)] already_received_fds: Vec<std::os::fd::OwnedFd>,
        socket_status: Arc<SocketStatus>,
    ) -> Self {
        Self {
            socket,
            senders,
            pending_method_calls,
            already_received_bytes,
            #[cfg(unix)]
            already_received_fds,
            prev_seq: 0,
            socket_status,
        }
    }

    pub fn spawn(self, executor: &Executor<'_>) -> Task<()> {
        executor.spawn(self.receive_msg(), "socket reader")
    }

    // Keep receiving messages and put them on the queue.
    #[instrument(name = "socket reader", skip(self), level = "trace")]
    async fn receive_msg(mut self) {
        loop {
            trace!("Waiting for message on the socket..");
            let msg = self.read_socket().await;
            match &msg {
                Ok(msg) => {
                    trace!("Message received on the socket: {:?}", msg);
                    if matches!(msg.message_type(), Type::MethodReturn | Type::Error) {
                        self.dispatch_pending_reply(msg);
                    }
                }
                Err(e) => {
                    trace!("Error reading from the socket: {:?}", e);
                    self.fail_pending_method_calls(e.clone());
                }
            };

            let mut senders = self.senders.lock().await;
            for (rule, sender) in &*senders {
                if let Ok(msg) = &msg {
                    if let Some(rule) = rule.as_ref() {
                        match rule.matches(msg) {
                            Ok(true) => (),
                            Ok(false) => continue,
                            Err(e) => {
                                debug!("Error matching message against rule: {:?}", e);

                                continue;
                            }
                        }
                    }
                }

                if let Err(e) = sender.broadcast_direct(msg.clone()).await {
                    // An error would be due to either of these:
                    //
                    // 1. the channel is closed.
                    // 2. No active receivers.
                    //
                    // In either case, just log it unless this is the channel for the generic
                    // unfiltered stream, where the channel is not created on-demand.
                    if rule.is_some() {
                        trace!(
                            "Error broadcasting message to stream for `{:?}`: {:?}",
                            rule, e
                        );
                    }
                }
            }
            trace!("Broadcasted to all streams: {:?}", msg);

            if msg.is_err() {
                senders.clear();
                self.socket_status.closed.store(true, Ordering::Release);
                self.socket_status.closed_event.notify(usize::MAX);
                trace!("Socket reading task stopped");

                return;
            }
        }
    }

    fn dispatch_pending_reply(&self, msg: &Message) {
        debug_assert!(matches!(
            msg.message_type(),
            Type::MethodReturn | Type::Error
        ));

        let reply_serial = match msg.header().reply_serial() {
            Some(serial) => serial,
            None => return,
        };

        let result = match msg.message_type() {
            Type::MethodReturn => Ok(msg.clone()),
            Type::Error => Err(msg.clone().into()),
            Type::MethodCall | Type::Signal => return,
        };
        self.pending_method_calls
            .complete_call(reply_serial, msg.recv_position(), result);
    }

    fn fail_pending_method_calls(&self, error: crate::Error) {
        self.pending_method_calls.fail_all(error);
    }

    #[instrument(skip(self), level = "trace")]
    async fn read_socket(&mut self) -> crate::Result<Message> {
        self.socket_status.activity_event.notify(usize::MAX);
        let seq = self.prev_seq + 1;
        let msg = self
            .socket
            .receive_message(
                seq,
                &mut self.already_received_bytes,
                #[cfg(unix)]
                &mut self.already_received_fds,
            )
            .await?;
        self.prev_seq = seq;

        Ok(msg)
    }
}

/// Socket-related state shared between [`super::ConnectionInner`] and the socket reader task.
#[derive(Debug)]
pub(super) struct SocketStatus {
    pub activity_event: Event,
    pub closed: AtomicBool,
    pub closed_event: Event,
}
