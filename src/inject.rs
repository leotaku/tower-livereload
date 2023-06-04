use std::{future::Future, task::Poll};

use bytes::{Buf, Bytes};
use http::{header, Request, Response};
use tower::Service;

use crate::{predicate::Predicate, ready_polyfill::ready};

#[derive(Clone, Debug)]
pub struct InjectService<S, ReqPred, ResPred> {
    service: S,
    data: Bytes,
    req_predicate: ReqPred,
    res_predicate: ResPred,
}

impl<S, ReqPred, ResPred> InjectService<S, ReqPred, ResPred> {
    pub fn new(
        service: S,
        data: Bytes,
        request_predicate: ReqPred,
        response_predicate: ResPred,
    ) -> Self {
        Self {
            service,
            data,
            req_predicate: request_predicate,
            res_predicate: response_predicate,
        }
    }
}

impl<S, ReqPred, ResPred, ReqBody, ResBody> Service<Request<ReqBody>>
    for InjectService<S, ReqPred, ResPred>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    ReqPred: Predicate<Request<ReqBody>>,
    ResPred: Predicate<Response<ResBody>>,
    ResBody: http_body::Body,
{
    type Response = Response<InjectBody<ResBody>>;
    type Error = S::Error;
    type Future = InjectResponseFuture<S::Future, ResPred>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ReqBody>) -> Self::Future {
        let should_inject = self.req_predicate.check(&request);
        InjectResponseFuture {
            inner: self.service.call(request),
            data: should_inject.then(|| self.data.clone()),
            predicate: self.res_predicate,
        }
    }
}

pin_project_lite::pin_project! {
    pub struct InjectResponseFuture<F, Pred> {
        #[pin]
        inner: F,
        data: Option<Bytes>,
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

        let data = match this.data {
            Some(data) => data,
            None => {
                let (parts, body) = response.into_parts();
                return Poll::Ready(Ok(Response::from_parts(
                    parts,
                    InjectBody { body, inject: None },
                )));
            }
        };

        let content_length: Option<usize> = this
            .predicate
            .check(&response)
            .then(|| {
                response
                    .headers()
                    .get(header::CONTENT_ENCODING)
                    .map_or_else(|| Some(()), |_| None)
            })
            .and_then(|_| {
                response
                    .headers()
                    .get(header::CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok().and_then(|s| s.parse().ok()))
            });

        let (mut parts, body) = response.into_parts();
        let inject = if let Some(length) = content_length {
            parts
                .headers
                .insert(header::CONTENT_LENGTH, (length + data.remaining()).into());
            this.data.take()
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
