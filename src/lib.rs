//! A middleware for browser reloading, built on top of [`tower`].
//!
//! # Example
//!
//! Note that [`axum`] is only used as an example here, pretty much any Rust
//! HTTP library or framework will be compatible!
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
//!     let listener = tokio::net::TcpListener::bind("0.0.0.0:3030").await?;
//!     axum::serve(listener, app).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! If you continuously rebuild and rerun this example e.g. using [`watchexec`],
//! you should see your browser reload whenever the code is changed.
//!
//! More examples can be found on GitHub under [examples].
//!
//! [`axum`]: https://docs.rs/axum
//! [`tower`]: https://docs.rs/tower
//! [`watchexec`]: https://watchexec.github.io/
//! [examples]: https://github.com/leotaku/tower-livereload/tree/master/examples
//!
//! # Manual reload
//!
//! With the [`Reloader`] utility, it is possible to reload your web browser
//! entirely using hooks from Rust code. See this [example] on GitHub for
//! pointers on how to implement a self-contained live-reloading static server.
//!
//! [example]: https://github.com/leotaku/tower-livereload/blob/master/examples/axum-file-watch/
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
//! `text/html` and [`Content-Encoding`] must not be set.
//!
//! If LiveReload is not working for some of your pages, ensure that these
//! heuristics apply to your responses. In particular, if you use middleware to
//! compress your HTML, ensure that the [`LiveReload`] middleware is
//! applied before your compression middleware.
//!
//! [`Content-Type`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type
//! [`Content-Encoding`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Encoding

#![forbid(unsafe_code, unused_unsafe)]
#![warn(clippy::all, missing_docs, nonstandard_style, future_incompatible)]
#![allow(clippy::type_complexity)]

mod inject;
mod long_poll;
mod overlay;
pub mod predicate;
mod ready_polyfill;

use std::{convert::Infallible, time::Duration};

use http::{header, Request, Response, StatusCode};
use inject::InjectService;
use long_poll::LongPollBody;
use overlay::OverlayService;
use predicate::{Always, ContentTypeStartsWith, Predicate};
use tokio::sync::broadcast::Sender;
use tower::{Layer, Service};

const DEFAULT_PREFIX: &str = "/tower-livereload/long-name-to-avoid-collisions";

/// Utility to send reload requests to clients.
#[derive(Clone, Debug)]
pub struct Reloader {
    sender: Sender<()>,
}

impl Reloader {
    /// Create a new [`Reloader`].
    ///
    /// This can be manually passed to the [`LiveReload`] constructor, but in
    /// most cases the [`LiveReloadLayer`] and [`LiveReloadLayer::reloader`]
    /// utilities are preferred.
    pub fn new() -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(1);
        Self { sender }
    }

    /// Send a reload request to all open clients.
    pub fn reload(&self) {
        self.sender.send(()).ok();
    }
}

impl Default for Reloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Layer to apply [`LiveReload`] middleware.
#[derive(Clone, Debug)]
pub struct LiveReloadLayer<ReqPred = Always, ResPred = ContentTypeStartsWith<&'static str>> {
    custom_prefix: Option<String>,
    reloader: Reloader,
    req_predicate: ReqPred,
    res_predicate: ResPred,
    reload_interval: Duration,
}

impl LiveReloadLayer {
    /// Create a new [`LiveReloadLayer`] with the default prefix for internal
    /// routes.
    ///
    /// The default prefix is deliberately long and specific to avoid any
    /// accidental collisions with the wrapped service.
    pub fn new() -> Self {
        Self {
            custom_prefix: None,
            reloader: Reloader::new(),
            req_predicate: Always,
            res_predicate: ContentTypeStartsWith::new("text/html"),
            reload_interval: Duration::from_secs(1),
        }
    }

    /// Create a new [`LiveReloadLayer`] with a custom prefix.
    #[deprecated(
        since = "0.8.0",
        note = "please use `LiveReloadLayer::new` and `custom_prefix` instead"
    )]
    pub fn with_custom_prefix<P: Into<String>>(prefix: P) -> Self {
        Self::new().custom_prefix(prefix)
    }
}

impl<ReqPred, ResPred> LiveReloadLayer<ReqPred, ResPred> {
    /// Set a custom prefix for internal routes for the given
    /// [`LiveReloadLayer`].
    pub fn custom_prefix<P: Into<String>>(self, prefix: P) -> Self {
        Self {
            custom_prefix: Some(prefix.into()),
            ..self
        }
    }

    /// Set a custom predicate for requests that should have their response HTML
    /// injected with live-reload logic.
    ///
    /// Note that this predicate is applied in addition to the default response
    /// predicate, which makes sure that only HTML responses are injected.
    ///
    /// Also see [`predicate`] for pre-defined predicates and
    /// [`predicate::Predicate`] for how to implement your own predicates.
    pub fn request_predicate<Body, P: Predicate<Request<Body>>>(
        self,
        predicate: P,
    ) -> LiveReloadLayer<P, ResPred> {
        LiveReloadLayer {
            custom_prefix: self.custom_prefix,
            reloader: self.reloader,
            req_predicate: predicate,
            res_predicate: self.res_predicate,
            reload_interval: self.reload_interval,
        }
    }

    /// Set a custom predicate for responses that should be injected with
    /// live-reload logic.
    ///
    /// Note that this predicate is applied instead of the default response
    /// predicate, which would make sure that only HTML responses are injected.
    /// However, even with a custom predicate only responses without a custom
    /// encoding i.e. no [`Content-Encoding`] header can and will be injected.
    ///
    /// Also see [`predicate`] for pre-defined predicates and
    /// [`predicate::Predicate`] for how to implement your own predicates.
    ///
    /// [`Content-Encoding`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Encoding
    pub fn response_predicate<Body, P: Predicate<Response<Body>>>(
        self,
        predicate: P,
    ) -> LiveReloadLayer<ReqPred, P> {
        LiveReloadLayer {
            custom_prefix: self.custom_prefix,
            reloader: self.reloader,
            req_predicate: self.req_predicate,
            res_predicate: predicate,
            reload_interval: self.reload_interval,
        }
    }

    /// Set a custom retry interval for the live-reload logic.
    pub fn reload_interval(self, interval: Duration) -> Self {
        Self {
            reload_interval: interval,
            ..self
        }
    }

    /// Return a manual [`Reloader`] trigger for the given [`LiveReloadLayer`].
    pub fn reloader(&self) -> Reloader {
        self.reloader.clone()
    }
}

impl Default for LiveReloadLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, ReqPred: Copy, ResPred: Copy> Layer<S> for LiveReloadLayer<ReqPred, ResPred> {
    type Service = LiveReload<S, ReqPred, ResPred>;

    fn layer(&self, inner: S) -> Self::Service {
        #[allow(deprecated)]
        LiveReload::new(
            inner,
            self.reloader.clone(),
            self.req_predicate,
            self.res_predicate,
            self.reload_interval,
            self.custom_prefix
                .as_ref()
                .cloned()
                .unwrap_or_else(|| DEFAULT_PREFIX.to_owned()),
        )
    }
}

type InnerService<S, ReqPred, ResPred> = OverlayService<
    String,
    Infallible,
    OverlayService<LongPollBody, Infallible, InjectService<S, ReqPred, ResPred>>,
>;

/// Middleware to enable LiveReload functionality.
#[derive(Clone, Debug)]
pub struct LiveReload<S, ReqPred = Always, ResPred = ContentTypeStartsWith<&'static str>> {
    service: InnerService<S, ReqPred, ResPred>,
}

impl<S, ReqPred, ResPred> LiveReload<S, ReqPred, ResPred> {
    #[deprecated(
        since = "0.9.0",
        note = "please use `LiveReloadLayer::new().layer(service)` instead"
    )]
    /// Create a new [`LiveReload`] middleware.
    pub fn new<P: Into<String>>(
        service: S,
        reloader: Reloader,
        req_predicate: ReqPred,
        res_predicate: ResPred,
        reload_interval: Duration,
        prefix: P,
    ) -> Self {
        let prefix = prefix.into();
        let long_poll_path = format!("{}/long-poll", prefix);
        let back_up_path = format!("{}/back-up", prefix);
        let inject = InjectService::new(
            service,
            format!(
                r#"<script data-long-poll="{long_poll}" data-back-up="{back_up}" data-reload-interval="{reload_interval}">{code}</script>"#,
                code = include_str!("../assets/polling.js"),
                long_poll = long_poll_path,
                back_up = back_up_path,
                reload_interval = reload_interval.as_millis(),
            )
            .into(),
            req_predicate,
            res_predicate,
        );
        let overlay_poll = OverlayService::new(inject).path(long_poll_path, move || {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream")
                .body(LongPollBody::new(reloader.sender.subscribe()))
                .map_err(|_| unreachable!())
        });
        let overlay_up = OverlayService::new(overlay_poll).path(back_up_path, || {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/plain")
                .body("Ok".to_owned())
                .map_err(|_| unreachable!())
        });

        LiveReload {
            service: overlay_up,
        }
    }
}

impl<ReqBody, ResBody, S, ReqPred, ResPred> Service<Request<ReqBody>>
    for LiveReload<S, ReqPred, ResPred>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    ResBody: http_body::Body,
    ReqPred: Predicate<Request<ReqBody>>,
    ResPred: Predicate<Response<ResBody>>,
{
    type Response = <InnerService<S, ReqPred, ResPred> as Service<Request<ReqBody>>>::Response;
    type Error = <InnerService<S, ReqPred, ResPred> as Service<Request<ReqBody>>>::Error;
    type Future = <InnerService<S, ReqPred, ResPred> as Service<Request<ReqBody>>>::Future;

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
