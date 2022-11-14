use std::{convert::Infallible, task::Poll};
use tokio::sync::broadcast::Receiver;

pub struct LongPollBody {
    receiver: Option<Receiver<()>>,
}

impl LongPollBody {
    pub fn new(receiver: Receiver<()>) -> Self {
        LongPollBody {
            receiver: Some(receiver),
        }
    }
}

impl http_body::Body for LongPollBody {
    type Data = bytes::Bytes;
    type Error = Infallible;

    fn poll_data(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        match self.receiver.take() {
            Some(mut receiver) => {
                let waker = cx.waker().clone();
                tokio::spawn(async move {
                    receiver.recv().await.ok();
                    waker.wake();
                });
                Poll::Pending
            }
            None => Poll::Ready(None),
        }
    }

    fn poll_trailers(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}
