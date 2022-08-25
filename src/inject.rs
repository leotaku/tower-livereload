use std::{
    future::Future,
    task::{ready, Poll},
};

use bytes::{Buf, Bytes};
use http::{header, Request, Response};
use tower::Service;

use crate::predicate::Predicate;

#[derive(Clone)]
pub struct InjectService<S, Pred> {
    service: S,
    data: Bytes,
    predicate: Pred,
}

impl<S, Pred> InjectService<S, Pred> {
    pub fn new(service: S, data: Bytes, predicate: Pred) -> Self {
        Self {
            service,
            data,
            predicate,
        }
    }
}

impl<S, Pred, ReqBody, ResBody> Service<Request<ReqBody>> for InjectService<S, Pred>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    Pred: Predicate<Response<ResBody>>,
    ResBody: http_body::Body,
{
    type Response = Response<InjectBody<ResBody>>;
    type Error = S::Error;
    type Future = InjectResponseFuture<S::Future, Pred>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ReqBody>) -> Self::Future {
        InjectResponseFuture {
            inner: self.service.call(request),
            data: self.data.clone(),
            predicate: self.predicate,
        }
    }
}

pin_project_lite::pin_project! {
    pub struct InjectResponseFuture<F, Pred> {
        #[pin]
        inner: F,
        data: Bytes,
        predicate: Pred,
    }
}

impl<F, Pred, B, E> Future for InjectResponseFuture<F, Pred>
where
    F: Future<Output = Result<Response<B>, E>>,
    Pred: Predicate<Response<B>>,
    B: http_body::Body,
{
    type Output = Result<Response<InjectBody<B>>, E>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let response = ready!(this.inner.poll(cx)?);

        let content_length: Option<usize> = response
            .headers()
            .get(header::CONTENT_ENCODING)
            .map_or_else(|| Some(()), |_| None)
            .and_then(|_| {
                response
                    .headers()
                    .get(header::CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok().and_then(|s| s.parse().ok()))
            });

        let (mut parts, body) = response.into_parts();
        let inject = if let Some(length) = content_length {
            parts.headers.insert(
                header::CONTENT_LENGTH,
                (length + this.data.remaining()).into(),
            );
            Some(this.data.clone())
        } else {
            None
        };

        Poll::Ready(Ok(Response::from_parts(parts, InjectBody { body, inject })))
    }
}

pin_project_lite::pin_project! {
    pub struct InjectBody<B> {
        #[pin]
        body: B,
        inject: Option<Bytes>,
    }
}

impl<B: http_body::Body> http_body::Body for InjectBody<B> {
    type Data = Bytes;
    type Error = B::Error;

    fn poll_data(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let this = self.project();
        let poll = ready!(this
            .body
            .poll_data(cx)
            .map_ok(|mut chunk| chunk.copy_to_bytes(chunk.remaining()))?);
        if let Some(chunk) = poll {
            Poll::Ready(Some(Ok(chunk)))
        } else if let Some(trail) = this.inject.take() {
            Poll::Ready(Some(Ok(trail)))
        } else {
            Poll::Ready(None)
        }
    }

    fn poll_trailers(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        self.project().body.poll_trailers(cx)
    }
}
