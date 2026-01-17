use std::sync::Arc;

#[cfg(debug_assertions)]
use tokio::sync::mpsc::{Receiver, Sender};
#[cfg(not(debug_assertions))]
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{message::{BridgeNotificationType, MessageToBackend, MessageToFrontend}, serial::{AtomicOptionSerial, AtomicSerialProvider, AtomicSetSerial, Serial}};

pub fn create_pair() -> (BackendReceiver, BackendHandle, FrontendReceiver, FrontendHandle) {
    #[cfg(debug_assertions)]
    let (frontend_send, frontend_recv) = tokio::sync::mpsc::channel(4192);
    #[cfg(debug_assertions)]
    let (backend_send, backend_recv) = tokio::sync::mpsc::channel(4192);

    #[cfg(not(debug_assertions))]
    let (frontend_send, frontend_recv) = tokio::sync::mpsc::unbounded_channel();
    #[cfg(not(debug_assertions))]
    let (backend_send, backend_recv) = tokio::sync::mpsc::unbounded_channel();

    let backend_serial = AtomicSetSerial::default();
    let frontend_serial = AtomicSetSerial::default();

    (
        BackendReceiver {
            receiver: backend_recv,
            processed_serial: backend_serial.clone(),
        },
        BackendHandle {
            sender: backend_send,
            processed_serial: backend_serial.clone(),
            next_serial: Default::default(),
        },
        FrontendReceiver {
            receiver: frontend_recv,
            processed_serial: frontend_serial.clone(),
        },
        FrontendHandle {
            sender: frontend_send,
            processed_serial: frontend_serial.clone(),
            next_serial: Default::default(),
        }
    )
}

#[derive(Debug)]
pub struct BackendReceiver {
    #[cfg(debug_assertions)]
    receiver: Receiver<(MessageToBackend, Option<Serial>)>,
    #[cfg(not(debug_assertions))]
    receiver: UnboundedReceiver<(MessageToBackend, Option<Serial>)>,
    processed_serial: AtomicSetSerial,
}

impl BackendReceiver {
    pub async fn recv(&mut self) -> Option<MessageToBackend> {
        let (message, serial) = self.receiver.recv().await?;
        if let Some(serial) = serial {
            self.processed_serial.set(serial);
        }
        Some(message)
    }
}

#[derive(Debug)]
pub struct FrontendReceiver {
    #[cfg(debug_assertions)]
    receiver: Receiver<(MessageToFrontend, Option<Serial>)>,
    #[cfg(not(debug_assertions))]
    receiver: UnboundedReceiver<(MessageToFrontend, Option<Serial>)>,
    processed_serial: AtomicSetSerial,
}

impl FrontendReceiver {
    pub async fn recv(&mut self) -> Option<MessageToFrontend> {
        let (message, serial) = self.receiver.recv().await?;
        if let Some(serial) = serial {
            self.processed_serial.set(serial);
        }
        Some(message)
    }

    pub fn try_recv(&mut self) -> Option<MessageToFrontend> {
        let (message, serial) = self.receiver.try_recv().ok()?;
        if let Some(serial) = serial {
            self.processed_serial.set(serial);
        }
        Some(message)
    }
}

#[derive(Clone, Debug)]
pub struct BackendHandle {
    #[cfg(debug_assertions)]
    sender: Sender<(MessageToBackend, Option<Serial>)>,
    #[cfg(not(debug_assertions))]
    sender: UnboundedSender<(MessageToBackend, Option<Serial>)>,
    processed_serial: AtomicSetSerial,
    next_serial: AtomicSerialProvider,
}

unsafe impl Send for BackendHandle {}
unsafe impl Sync for BackendHandle {}

impl BackendHandle {
    pub fn send(&self, message: MessageToBackend) {
        #[cfg(debug_assertions)]
        self.sender.try_send((message, None)).unwrap();
        #[cfg(not(debug_assertions))]
        let _ = self.sender.send((message, None));
    }

    pub fn send_with_serial(&self, message: MessageToBackend, serial: &AtomicOptionSerial) {
        if let Some(serial) = serial.get() && self.processed_serial.get() < serial {
            return;
        }

        let next_serial = self.next_serial.next();
        serial.set(next_serial);

        #[cfg(debug_assertions)]
        self.sender.try_send((message, Some(next_serial))).unwrap();
        #[cfg(not(debug_assertions))]
        let _ = self.sender.send((message, Some(next_serial)));
    }

    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
}

#[derive(Clone, Debug)]
pub struct FrontendHandle {
    #[cfg(debug_assertions)]
    sender: Sender<(MessageToFrontend, Option<Serial>)>,
    #[cfg(not(debug_assertions))]
    sender: UnboundedSender<(MessageToFrontend, Option<Serial>)>,
    processed_serial: AtomicSetSerial,
    next_serial: AtomicSerialProvider,
}

unsafe impl Send for FrontendHandle {}
unsafe impl Sync for FrontendHandle {}

impl FrontendHandle {
    pub fn send(&self, message: MessageToFrontend) {
        #[cfg(debug_assertions)]
        let _ = self.sender.try_send((message, None));
        #[cfg(not(debug_assertions))]
        let _ = self.sender.send((message, None));
    }

    pub async fn send_async(&self, message: MessageToFrontend) {
        #[cfg(debug_assertions)]
        let _ = self.sender.send((message, None)).await;
        #[cfg(not(debug_assertions))]
        let _ = self.sender.send((message, None));
    }

    pub fn send_with_serial(&self, message: MessageToFrontend, serial: &AtomicOptionSerial) {
        if let Some(serial) = serial.get() && self.processed_serial.get() < serial {
            return;
        }

        let next_serial = self.next_serial.next();
        serial.set(next_serial);

        #[cfg(debug_assertions)]
        let _ = self.sender.try_send((message, Some(next_serial)));
        #[cfg(not(debug_assertions))]
        let _ = self.sender.send((message, Some(next_serial)));
    }

    pub async fn send_with_serial_async(&self, message: MessageToFrontend, serial: &AtomicOptionSerial) {
        if let Some(serial) = serial.get() && self.processed_serial.get() < serial {
            return;
        }

        let next_serial = self.next_serial.next();
        serial.set(next_serial);

        #[cfg(debug_assertions)]
        let _ = self.sender.send((message, Some(next_serial))).await;
        #[cfg(not(debug_assertions))]
        let _ = self.sender.send((message, Some(next_serial)));
    }

    pub fn send_info(&self, info: impl Into<Arc<str>>) {
        self.send(MessageToFrontend::AddNotification {
            notification_type: BridgeNotificationType::Info,
            message: info.into()
        })
    }

    pub fn send_success(&self, success: impl Into<Arc<str>>) {
        self.send(MessageToFrontend::AddNotification {
            notification_type: BridgeNotificationType::Success,
            message: success.into()
        })
    }

    pub fn send_warning(&self, warning: impl Into<Arc<str>>) {
        self.send(MessageToFrontend::AddNotification {
            notification_type: BridgeNotificationType::Warning,
            message: warning.into()
        })
    }

    pub fn send_error(&self, error: impl Into<Arc<str>>) {
        self.send(MessageToFrontend::AddNotification {
            notification_type: BridgeNotificationType::Error,
            message: error.into()
        })
    }

    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    pub fn last_serial(&self) -> Serial {
        self.processed_serial.get()
    }
}
