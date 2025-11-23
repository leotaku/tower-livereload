use std::{convert::Infallible, future::Future, pin::Pin, sync::Arc, task::Poll, time::Duration};

use http_body::Frame;
use tokio::sync::{futures::OwnedNotified, Notify};

pub struct ReloadEventsBody {
    state: State,
    retry_duration: Duration,
}

enum State {
    Initial(Arc<Notify>),
    Pending(Pin<Box<OwnedNotified>>),
    Final,
}

impl ReloadEventsBody {
    pub fn new(receiver: Arc<Notify>, retry_duration: Duration) -> Self {
        Self {
            state: State::Initial(receiver),
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
        match std::mem::replace(&mut self.state, State::Final) {
            State::Initial(notify) => {
                self.state = State::Pending(Box::pin(notify.notified_owned()));

                Poll::Ready(Some(Ok(Frame::data(bytes::Bytes::from_owner(format!(
                    "event: init\ndata:\nretry: {}\n\n",
                    self.retry_duration.as_millis()
                ))))))
            }
            State::Pending(mut notified) => {
                if notified.as_mut().poll(cx) == Poll::Pending {
                    self.state = State::Pending(notified);
                    return Poll::Pending;
                }

                Poll::Ready(Some(Ok(Frame::data(bytes::Bytes::from_static(
                    b"event: reload\ndata:\n\n",
                )))))
            }
            State::Final => Poll::Ready(None),
        }
    }
}
