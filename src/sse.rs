use std::{convert::Infallible, task::Poll, time::Duration};

use http_body::Frame;
use tokio::sync::broadcast::Receiver;

pub struct ReloadEventsBody {
    state: State,
    retry_duration: Duration,
}

enum State {
    Initial(Receiver<()>),
    BeforePending(Receiver<()>),
    Pending,
    Final,
}

impl ReloadEventsBody {
    pub fn new(receiver: Receiver<()>, retry_duration: Duration) -> Self {
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
            State::Initial(receiver) => {
                self.state = State::BeforePending(receiver);

                Poll::Ready(Some(Ok(Frame::data(bytes::Bytes::from_owner(format!(
                    "event: init\ndata:\nretry: {}\n\n",
                    self.retry_duration.as_millis()
                ))))))
            }
            State::BeforePending(mut receiver) => {
                self.state = State::Pending;

                let waker = cx.waker().clone();
                tokio::spawn(async move {
                    receiver.recv().await.ok();
                    waker.wake();
                });
                Poll::Pending
            }
            State::Pending => {
                self.state = State::Final;

                Poll::Ready(Some(Ok(Frame::data(bytes::Bytes::from_static(
                    b"event: reload\ndata:\n\n",
                )))))
            }
            State::Final => Poll::Ready(None),
        }
    }
}
