//! A LiveReload middleware built on top of [tower].
//!
//! # Example
//!
//! Note that [axum] is only used as an example here, pretty much any Rust HTTP
//! library or framework will be compatible!
//!
//! ```
//! use axum::{response::Html, routing::get, Router};
//! use tower_livereload::LiveReloadLayer;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let app = Router::new()
//!         .route("/", get(|| async { Html("<h1>Wow, such webdev</h1>") }))
//!         .layer(LiveReloadLayer::new());
//!
//!     axum::Server::bind(&"0.0.0.0:3030".parse()?)
//!         .serve(app.into_make_service())
//!         .await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! More examples can be found on GitHub under [examples].
//!
//! [axum]: https://docs.rs/axum
//! [tower]: https://docs.rs/tower
//! [examples]: https://github.com/leotaku/tower-livereload/tree/master/examples
//!
//! # Ecosystem compatibility
//!
//! `tower-livereload` has been built from the ground up to provide the highest
//! amount of ecosystem compatibility.
//!
//! The provided middleware uses the [`http`] and [`http_body`] crates as its
//! HTTP abstractions. That means it is compatible with any library or framework
//! that also uses those crates, such as [`hyper`], [`axum`], [`tonic`], and
//! [`warp`].
//!
//! Moreover, we do not depend on any async runtime, keeping your dependency
//! graph small and simplifying debugging.
//!
//! [`http`]: https://docs.rs/http
//! [`http_body`]: https://docs.rs/http_body
//! [`hyper`]: https://docs.rs/hyper
//! [`axum`]: https://docs.rs/axum
//! [`tonic`]: https://docs.rs/tonic
//! [`warp`]: https://docs.rs/warp
//!
//! # Heuristics
//!
//! To provide LiveReload functionality, we have to inject code into HTML web
//! pages. To determine whether a page is injectable, some header-based
//! heuristics are used. In particular, [`Content-Type`] has to start with
//! `text/html`, [`Content-Length`] must be set, and [`Content-Encoding`] must
//! not be set.
//!
//! If LiveReload is not working for some of your pages, ensure that these
//! heuristics apply to your responses. In particular, if you use middleware to
//! compress your HTML, ensure that the [`LiveReload`] middleware is
//! applied before your compression middleware.
//!
//! [`Content-Type`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type
//! [`Content-Length`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Length
//! [`Content-Encoding`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Encoding

#![warn(clippy::all, missing_docs, nonstandard_style, future_incompatible)]
#![forbid(unsafe_code)]

mod inject;
mod long_poll;
mod overlay;
mod predicate;
mod ready_polyfill;

use http::{header, Request, Response, StatusCode};
use inject::InjectService;
use long_poll::LongPollBody;
use overlay::OverlayService;
use predicate::ContentTypeStartsWithPredicate;
use tokio::sync::broadcast::{Receiver, Sender};
use tower::{Layer, Service};

/// Utility to send reload requests to clients.
#[derive(Debug)]
pub struct Reloader {
    sender: Sender<()>,
}

impl Reloader {
    /// Send a reload request to all open clients.
    pub fn reload(&self) {
        self.sender.send(()).ok();
    }
}

/// Layer to apply [`LiveReload`] middleware.
#[derive(Clone, Debug)]
pub struct LiveReloadLayer {
    custom_prefix: Option<String>,
    sender: Sender<()>,
}

impl LiveReloadLayer {
    /// Create a new [`LiveReloadLayer`] with the default prefix for our own
    /// assets.
    ///
    /// The default prefix is deliberately long and specific to avoid any
    /// accidental collisions with the wrapped service.
    pub fn new() -> LiveReloadLayer {
        let (sender, _) = tokio::sync::broadcast::channel(1);
        LiveReloadLayer {
            custom_prefix: None,
            sender,
        }
    }

    /// Create a new [`LiveReloadLayer`] with a custom prefix.
    pub fn with_custom_prefix<P: Into<String>>(prefix: P) -> LiveReloadLayer {
        let (sender, _) = tokio::sync::broadcast::channel(1);
        LiveReloadLayer {
            custom_prefix: Some(prefix.into()),
            sender,
        }
    }

    /// Create a manual [`Reloader`] trigger for the given [`LiveReloadLayer`].
    pub fn reloader(&self) -> Reloader {
        Reloader {
            sender: self.sender.clone(),
        }
    }
}

impl<S> Layer<S> for LiveReloadLayer {
    type Service = LiveReload<S>;

    fn layer(&self, inner: S) -> Self::Service {
        if let Some(ref custom_prefix) = self.custom_prefix {
            LiveReload::with_custom_prefix(inner, self.sender.subscribe(), custom_prefix.clone())
        } else {
            LiveReload::new(inner, self.sender.subscribe())
        }
    }
}

type InnerService<S> = OverlayService<
    String,
    http::Error,
    OverlayService<
        LongPollBody,
        http::Error,
        InjectService<S, ContentTypeStartsWithPredicate<&'static str>>,
    >,
>;

/// Middleware to enable LiveReload functionality.
#[derive(Clone, Debug)]
pub struct LiveReload<S> {
    service: InnerService<S>,
}

impl<S> LiveReload<S> {
    /// Create a new [`LiveReload`] middleware with the default prefix
    /// for our own assets.
    ///
    /// The default prefix is deliberately long and specific to avoid
    /// any accidental collisions with the wrapped service.
    fn new(service: S, receiver: Receiver<()>) -> Self {
        Self::with_custom_prefix(
            service,
            receiver,
            "/tower-livereload/long-name-to-avoid-collisions",
        )
    }

    /// Create a new [`LiveReload`] middleware with a custom prefix.
    fn with_custom_prefix<P: Into<String>>(
        service: S,
        receiver: Receiver<()>,
        prefix: P,
    ) -> Self {
        let prefix = prefix.into();
        let long_poll_path = format!("{}/long-poll", prefix);
        let back_up_path = format!("{}/back-up", prefix);
        let inject = InjectService::new(
            service,
            format!(
                include_str!("../assets/polling.html"),
                long_poll = long_poll_path,
                back_up = back_up_path,
            )
            .into(),
            ContentTypeStartsWithPredicate::new("text/html"),
        );
        let overlay_poll = OverlayService::new(inject).path(long_poll_path, move || {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream")
                .body(LongPollBody::new(receiver.resubscribe()))
        });
        let overlay_up = OverlayService::new(overlay_poll).path(back_up_path, || {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/plain")
                .body("Ok".to_owned())
        });

        LiveReload {
            service: overlay_up,
        }
    }
}

impl<ReqBody, RespBody, S> Service<Request<ReqBody>> for LiveReload<S>
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
