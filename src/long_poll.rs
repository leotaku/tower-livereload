use std::{convert::Infallible, task::Poll};

pin_project_lite::pin_project! {
    pub struct LongPollBody { _private: () }
}

impl LongPollBody {
    pub fn new() -> Self {
        LongPollBody { _private: () }
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
