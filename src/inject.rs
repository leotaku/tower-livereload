use std::{future::Future, task::{ready, Poll}};

use bytes::{Buf, Bytes};
use http::{header, Request, Response};
use http_body::Frame;
use tower::Service;

use crate::predicate::Predicate;

#[derive(Clone, Debug)]
pub struct InjectService<S, ReqPred, ResPred> {
    service: S,
    data: Bytes,
    req_predicate: ReqPred,
    res_predicate: ResPred,
}

impl<S, ReqPred, ResPred> InjectService<S, ReqPred, ResPred> {
    pub fn new(service: S, data: Bytes, req_predicate: ReqPred, res_predicate: ResPred) -> Self {
        Self {
            service,
            data,
            req_predicate,
            res_predicate,
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
            Some(data)
                if response.headers().get(header::CONTENT_ENCODING).is_none()
                    && this.predicate.check(&response) =>
            {
                data
            }
            Some(_) | None => {
                let (parts, body) = response.into_parts();
                return Poll::Ready(Ok(Response::from_parts(
                    parts,
                    InjectBody { body, inject: None },
                )));
            }
        };

        let content_length: Option<usize> = response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok().and_then(|s| s.parse().ok()));

        let (mut parts, body) = response.into_parts();
        if let Some(length) = content_length {
            parts
                .headers
                .insert(header::CONTENT_LENGTH, (length + data.remaining()).into());
        };

        Poll::Ready(Ok(Response::from_parts(
            parts,
            InjectBody {
                body,
                inject: this.data.take(),
            },
        )))
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

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        let poll = ready!(this
            .body
            .poll_frame(cx)
            .map_ok(|frame| frame.map_data(|mut chunk| chunk.copy_to_bytes(chunk.remaining())))?);
        if let Some(chunk) = poll {
            Poll::Ready(Some(Ok(chunk)))
        } else if let Some(trail) = this.inject.take() {
            Poll::Ready(Some(Ok(Frame::data(trail))))
        } else {
            Poll::Ready(None)
        }
    }
}
