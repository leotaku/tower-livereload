use std::{convert::Infallible, future::Future, pin::Pin, task::Poll};
use tokio::sync::broadcast::Receiver;

pub struct LongPollBody {
    receiver: Receiver<()>,
}

impl LongPollBody {
    pub fn new(receiver: Receiver<()>) -> Self {
        LongPollBody { receiver }
    }
}

impl http_body::Body for LongPollBody {
    type Data = bytes::Bytes;
    type Error = Infallible;

    fn poll_data(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let polled = {
            let mut boxed = Box::pin(self.receiver.recv());
            Pin::new(&mut boxed).poll(cx)
        };

        match polled {
            Poll::Ready(Ok(_)) => Poll::Ready(None),
            Poll::Ready(Err(_)) | Poll::Pending => {
                let waker = cx.waker().clone();
                let mut receiver = self.receiver.resubscribe();
                tokio::spawn(async move {
                    receiver.recv().await.ok();
                    waker.wake();
                });
                Poll::Pending
            }
        }
    }

    fn poll_trailers(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}
