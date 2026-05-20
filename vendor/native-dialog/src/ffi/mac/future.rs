use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_channel::oneshot::Receiver;
use futures_lite::FutureExt;
use objc2::ClassType;
use objc2::rc::Retained as Id;
use objc2::runtime::AnyObject;

use super::{OpenPanelDelegate, SavePanelDelegate};

pub struct DispatchResponse<T> {
    receiver: Receiver<T>,
    delegates: Vec<Id<AnyObject>>,
}

unsafe impl<T: Send> Send for DispatchResponse<T> {}
unsafe impl<T: Sync> Sync for DispatchResponse<T> {}

impl<T: Default> Future for DispatchResponse<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.receiver.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(x) => Poll::Ready(x.unwrap_or_default()),
        }
    }
}

impl<T: Default> DispatchResponse<T> {
    pub fn new(receiver: Receiver<T>) -> Self {
        Self {
            receiver,
            delegates: vec![],
        }
    }

    pub fn retain(mut self, delegate: Id<impl AsyncDelegate>) -> Self {
        self.delegates.push(delegate.into());
        self
    }
}

pub trait AsyncDelegate: ClassType + 'static {}

impl AsyncDelegate for OpenPanelDelegate {}
impl AsyncDelegate for SavePanelDelegate {}
