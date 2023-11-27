use std::{
    collections::HashMap, convert::Infallible, fmt::Display, future::Future, sync::Arc, task::Poll,
};

use bytes::Buf;
use http::{Request, Response};
use http_body::{Body, Frame};
use tower::Service;

pub struct OverlayService<B, E, S> {
    map: HashMap<String, Arc<dyn Fn() -> Result<Response<B>, E> + Send + Sync>>,
    service: S,
}

impl<B, E, S> OverlayService<B, E, S> {
    pub fn new(service: S) -> Self {
        Self {
            map: HashMap::new(),
            service,
        }
    }

    pub fn path(
        self,
        path: impl Into<String>,
        resp: impl Fn() -> Result<Response<B>, E> + Send + Sync + 'static,
    ) -> Self {
        let mut result = self;
        result.map.insert(path.into(), Arc::new(resp));
        result
    }
}

impl<B, E, S: Clone> Clone for OverlayService<B, E, S> {
    fn clone(&self) -> Self {
        OverlayService {
            map: self.map.clone(),
            service: self.service.clone(),
        }
    }
}

impl<B, E, S: std::fmt::Debug> std::fmt::Debug for OverlayService<B, E, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            write!(
                f,
                "OverlayService: {{
    map.keys: {:#?},
    service: {:#?}
}}",
                self.map.keys(),
                self.service,
            )
        } else {
            write!(
                f,
                "OverlayService: {{ map.keys: {:?}, service: {:?} }}",
                self.map.keys(),
                self.service,
            )
        }
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
        self.service.poll_ready(cx).map_err(OverlayError::B)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let path = req.uri().path();
        if let Some(fun) = self.map.get(path) {
            OverlayFuture {
                inner: self.service.call(req),
                response: Some(fun()),
            }
        } else {
            OverlayFuture {
                inner: self.service.call(req),
                response: None,
            }
        }
    }
}

pin_project_lite::pin_project! {
    pub struct OverlayFuture<B, E, F> {
        #[pin]
        inner: F,
        response: Option<Result<Response<B>, E>>,
    }
}

impl<B, E, PB, PE, F> Future for OverlayFuture<B, E, F>
where
    F: Future<Output = Result<Response<PB>, PE>>,
{
    type Output = Result<Response<OverlayBody<B, PB>>, OverlayError<E, PE>>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if let Some(resp) = this.response.take() {
            Poll::Ready(
                resp.map(|ok| ok.map(|a| OverlayBody::A { a }))
                    .map_err(OverlayError::A),
            )
        } else {
            this.inner
                .poll(cx)
                .map_ok(|resp| resp.map(|b| OverlayBody::B { b }))
                .map_err(OverlayError::B)
        }
    }
}

pin_project_lite::pin_project! {
    #[project = OverlayBodyProj]
    pub enum OverlayBody<A, B> {
        A{#[pin] a: A},
        B{#[pin] b: B},
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
            OverlayBodyProj::A { a } => a.poll_frame(cx).map_err(OverlayError::A),
            OverlayBodyProj::B { b } => b.poll_frame(cx).map_err(OverlayError::B),
        }
    }
}

#[derive(Debug, Clone)]
pub enum OverlayError<A, B> {
    A(A),
    B(B),
}

impl<A: std::error::Error, B: std::error::Error> std::error::Error for OverlayError<A, B> {}

impl<A: Display, B: Display> Display for OverlayError<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverlayError::A(a) => a.fmt(f),
            OverlayError::B(b) => b.fmt(f),
        }
    }
}

impl<A, B> From<OverlayError<A, B>> for Infallible
where
    A: Into<Infallible>,
    B: Into<Infallible>,
{
    fn from(value: OverlayError<A, B>) -> Self {
        match value {
            OverlayError::A(a) => a.into(),
            OverlayError::B(b) => b.into(),
        }
    }
}
