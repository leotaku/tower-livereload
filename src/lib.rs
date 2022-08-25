mod inject;
mod long_poll;
mod overlay;
mod predicate;

use http::{header, Request, Response, StatusCode};
use inject::InjectService;
use long_poll::LongPollBody;
use overlay::OverlayService;
use predicate::ContentTypeStartsWithPredicate;
use tower::{Layer, Service};

#[derive(Clone, Debug)]
pub struct LiveReloadLayer {
    custom_prefix: Option<String>,
}

impl LiveReloadLayer {
    pub fn new() -> LiveReloadLayer {
        LiveReloadLayer {
            custom_prefix: None,
        }
    }

    pub fn with_custom_prefix(prefix: impl Into<String>) -> LiveReloadLayer {
        LiveReloadLayer {
            custom_prefix: Some(prefix.into()),
        }
    }
}

impl<S> Layer<S> for LiveReloadLayer {
    type Service = LiveReloadService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        if let Some(ref custom_prefix) = self.custom_prefix {
            LiveReloadService::with_custom_prefix(inner, custom_prefix.clone())
        } else {
            LiveReloadService::new(inner)
        }
    }
}

type InnerService<S> = OverlayService<
    LongPollBody,
    http::Error,
    InjectService<S, ContentTypeStartsWithPredicate<&'static str>>,
>;

#[derive(Clone, Debug)]
pub struct LiveReloadService<S> {
    service: InnerService<S>,
}

impl<S> LiveReloadService<S> {
    pub fn new(service: S) -> Self {
        Self::with_custom_prefix(service, "/tower-livereload/long-name-to-avoid-collisions")
    }

    pub fn with_custom_prefix(service: S, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        let inject = InjectService::new(
            service,
            format!(
                include_str!("../assets/polling.html"),
                long_poll = format!("{}/long-poll", prefix),
                back_up = "/",
            )
            .into(),
            ContentTypeStartsWithPredicate::new("text/html"),
        );
        let overlay = OverlayService::new(inject, prefix).path("/long-poll", || {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream")
                .body(LongPollBody::new())
        });

        LiveReloadService { service: overlay }
    }
}

impl<ReqBody, RespBody, S> Service<Request<ReqBody>> for LiveReloadService<S>
where
    S: Service<Request<ReqBody>, Response = Response<RespBody>>,
    RespBody: http_body::Body,
{
    type Response = <InnerService<S> as Service<Request<ReqBody>>>::Response;
    type Error = <InnerService<S> as Service<Request<ReqBody>>>::Error;
    type Future = <InnerService<S> as Service<Request<ReqBody>>>::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        self.service.call(req)
    }
}
