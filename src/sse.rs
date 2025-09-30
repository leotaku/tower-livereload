use std::{convert::Infallible, task::Poll, time::Duration};

use http_body::Frame;
use tokio::sync::broadcast::Receiver;

pub struct ReloadEventsBody {
    receiver: Option<Receiver<()>>,
    initial: Option<()>,
    retry_duration: Duration,
}

impl ReloadEventsBody {
    pub fn new(receiver: Receiver<()>, retry_duration: Duration) -> Self {
        Self {
            receiver: Some(receiver),
            initial: Some(()),
            retry_duration,
        }
    }
}

impl http_body::Body for ReloadEventsBody {
    type Data = bytes::Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.initial.take().is_some() {
            return Poll::Ready(Some(Ok(Frame::data(bytes::Bytes::from_owner(format!(
                "event: init\ndata:\nretry: {}\n\n",
                self.retry_duration.as_millis()
            ))))));
        }

        match self.receiver.take() {
            Some(mut receiver) => {
                let waker = cx.waker().clone();
                tokio::spawn(async move {
                    receiver.recv().await.ok();
                    waker.wake();
                });
                Poll::Pending
            }
            None => Poll::Ready(Some(Ok(Frame::data(bytes::Bytes::from_static(
                b"event: reload\ndata:\n\n",
            ))))),
        }
    }
}
