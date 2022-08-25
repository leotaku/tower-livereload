use std::{convert::Infallible, task::Poll};

use pin_project::pin_project;

#[pin_project]
pub struct LongPollBody(());

impl LongPollBody {
    pub fn new() -> Self {
        LongPollBody(())
    }
}

impl http_body::Body for LongPollBody {
    type Data = bytes::Bytes;
    type Error = Infallible;

    fn poll_data(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        Poll::Pending
    }

    fn poll_trailers(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}
