use std::{convert::Infallible, future::Future, sync::Arc, task::Poll};

use bytes::Buf;
use http::{request::Parts, Request, Response};
use http_body::{Body, Frame};
use tower::Service;

pub struct OverlayService<B, E, S> {
    alternative: Arc<dyn Fn(&Parts) -> Option<Result<Response<B>, E>> + Send + Sync>,
    service: S,
}

impl<B, E, S> OverlayService<B, E, S> {
    pub fn new(
        service: S,
        alternative_fn: impl Fn(&Parts) -> Option<Result<Response<B>, E>> + Send + Sync + 'static,
    ) -> Self {
        Self {
            alternative: Arc::new(alternative_fn),
            service,
        }
    }
}

impl<B, E, S: Clone> Clone for OverlayService<B, E, S> {
    fn clone(&self) -> Self {
        OverlayService {
            alternative: self.alternative.clone(),
            service: self.service.clone(),
        }
    }
}

impl<B, E, S: std::fmt::Debug> std::fmt::Debug for OverlayService<B, E, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayService")
            .field("alternative", &"...")
            .field("service", &self.service)
            .finish()
    }
}

impl<S, E, ReqBody, ResBody, ResBodyNew> Service<Request<ReqBody>>
    for OverlayService<ResBodyNew, E, S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = Response<OverlayBody<ResBodyNew, ResBody>>;
    type Error = OverlayError<E, S::Error>;
    type Future = OverlayFuture<ResBodyNew, E, S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx).map_err(OverlayError::Right)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let (parts, body) = req.into_parts();
        if let Some(result) = self.alternative.clone()(&parts) {
            OverlayFuture::Alternative {
                alternative: Some(result),
            }
        } else {
            OverlayFuture::Inner {
                inner: self.service.call(Request::from_parts(parts, body)),
            }
        }
    }
}

pin_project_lite::pin_project! {
    #[project = OverlayFutureProj]
    pub enum OverlayFuture<B, E, F> {
        Inner {
            #[pin]
            inner: F
        },
        Alternative {
            alternative: Option<Result<Response<B>, E>>
        },
    }
}

impl<B, E, PB, PE, F> Future for OverlayFuture<B, E, F>
where
    F: Future<Output = Result<Response<PB>, PE>>,
{
    type Output = Result<Response<OverlayBody<B, PB>>, OverlayError<E, PE>>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this {
            OverlayFutureProj::Inner { inner } => inner
                .poll(cx)
                .map_ok(|resp| resp.map(|right| OverlayBody::Right { right }))
                .map_err(OverlayError::Right),
            OverlayFutureProj::Alternative { alternative } => Poll::Ready(
                alternative
                    .take()
                    .map(|some| {
                        some.map(|ok| ok.map(|left| OverlayBody::Left { left }))
                            .map_err(OverlayError::Left)
                    })
                    .unwrap_or_else(|| unreachable!()),
            ),
        }
    }
}

pin_project_lite::pin_project! {
    #[project = OverlayBodyProj]
    pub enum OverlayBody<L, R> {
        Left {
            #[pin]
            left: L
        },
        Right{
            #[pin]
            right: R
        },
    }
}

impl<Data: Buf, A: Body<Data = Data>, B: Body<Data = Data>> Body for OverlayBody<A, B> {
    type Data = Data;
    type Error = OverlayError<A::Error, B::Error>;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match self.project() {
            OverlayBodyProj::Left { left } => left.poll_frame(cx).map_err(OverlayError::Left),
            OverlayBodyProj::Right { right } => right.poll_frame(cx).map_err(OverlayError::Right),
        }
    }
}

#[derive(Debug, Clone)]
pub enum OverlayError<L, R> {
    Left(L),
    Right(R),
}

impl<L: std::error::Error, R: std::error::Error> std::error::Error for OverlayError<L, R> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OverlayError::Left(left) => left.source(),
            OverlayError::Right(right) => right.source(),
        }
    }
}

impl<L: std::fmt::Display, R: std::fmt::Display> std::fmt::Display for OverlayError<L, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverlayError::Left(left) => left.fmt(f),
            OverlayError::Right(right) => right.fmt(f),
        }
    }
}

impl<L, R> From<OverlayError<L, R>> for Infallible
where
    L: Into<Infallible>,
    R: Into<Infallible>,
{
    fn from(value: OverlayError<L, R>) -> Self {
        match value {
            OverlayError::Left(left) => left.into(),
            OverlayError::Right(right) => right.into(),
        }
    }
}
