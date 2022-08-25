use std::{
    collections::HashMap,
    error::Error,
    fmt::Display,
    future::Future,
    sync::Arc,
    task::{ready, Poll},
};

use bytes::Buf;
use http::{Request, Response};
use http_body::Body;
use tower::Service;

pub struct OverlayService<B, E, S> {
    map: HashMap<String, Arc<dyn Fn() -> Result<Response<B>, E> + Send + Sync>>,
    prefix: String,
    service: S,
}

impl<B, E, S> OverlayService<B, E, S> {
    pub fn new(service: S, prefix: impl Into<String>) -> Self {
        Self {
            map: HashMap::new(),
            prefix: prefix.into(),
            service,
        }
    }

    pub fn path(
        self,
        path: impl Into<String>,
        resp: impl Fn() -> Result<Response<B>, E> + Send + Sync + 'static,
    ) -> Self {
        let mut full_path = self.prefix.clone();
        full_path.push_str(&path.into());

        let mut result = self;
        result.map.insert(full_path, Arc::new(resp));
        result
    }
}

impl<B, E, S: Clone> Clone for OverlayService<B, E, S> {
    fn clone(&self) -> Self {
        OverlayService {
            map: self.map.clone(),
            prefix: self.prefix.clone(),
            service: self.service.clone(),
        }
    }
}

impl<S, E, ReqBody, ResBody, ResBodyNew> Service<Request<ReqBody>>
    for OverlayService<ResBodyNew, E, S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = Response<OverlayBody<ResBodyNew, ResBody>>;
    type Error = S::Error;
    type Future = OverlayFuture<ResBodyNew, E, S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
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
    type Output = Result<Response<OverlayBody<B, PB>>, PE>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if let Some(resp) = this.response.take() {
            Poll::Ready(
                resp.map(|ok| ok.map(|a| OverlayBody::A { a }))
                    .or_else(|_| panic!()),
            )
        } else {
            let polled = ready!(this.inner.poll(cx));
            Poll::Ready(polled.map(|resp| resp.map(|b| OverlayBody::B { b })))
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

    fn poll_data(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        match self.project() {
            OverlayBodyProj::A { a } => match ready!(a.poll_data(cx)) {
                Some(Ok(ok)) => Poll::Ready(Some(Ok(ok))),
                Some(Err(err)) => Poll::Ready(Some(Err(OverlayError::A(err)))),
                None => Poll::Ready(None),
            },
            OverlayBodyProj::B { b } => match ready!(b.poll_data(cx)) {
                Some(Ok(ok)) => Poll::Ready(Some(Ok(ok))),
                Some(Err(err)) => Poll::Ready(Some(Err(OverlayError::B(err)))),
                None => Poll::Ready(None),
            },
        }
    }

    fn poll_trailers(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        match self.project() {
            OverlayBodyProj::A { a } => {
                Poll::Ready(Ok(ready!(a.poll_trailers(cx)).map_err(OverlayError::A)?))
            }
            OverlayBodyProj::B { b } => {
                Poll::Ready(Ok(ready!(b.poll_trailers(cx)).map_err(OverlayError::B)?))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum OverlayError<A, B> {
    A(A),
    B(B),
}

impl<A: Error, B: Error> Error for OverlayError<A, B> {}

impl<A: Display, B: Display> Display for OverlayError<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverlayError::A(a) => a.fmt(f),
            OverlayError::B(b) => b.fmt(f),
        }
    }
}
