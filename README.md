# tower-livereload

[![Build Status][build-badge]][build-url]
[![Crates.io][crates-badge]][crates-url]
[![Lib.rs][librs-badge]][librs-url]
[![Documentation][docs-badge]][docs-url]

[build-url]: https://github.com/leotaku/tower-livereload/actions
[crates-url]: https://crates.io/crates/tower-livereload
[librs-url]: https://lib.rs/crates/tower-livereload
[docs-url]: https://docs.rs/tower-livereload

[build-badge]: https://img.shields.io/github/actions/workflow/status/leotaku/tower-livereload/build.yml?branch=master
[crates-badge]: https://img.shields.io/crates/v/tower-livereload.svg
[librs-badge]: https://img.shields.io/badge/lib.rs-linked-informational
[docs-badge]: https://img.shields.io/docsrs/tower-livereload

A middleware for browser reloading, built on top of [`tower`].

## Example

Note that [`axum`] is only used as an example here, pretty much any Rust
HTTP library or framework will be compatible!

```rust
use axum::{response::Html, routing::get, Router};
use tower_livereload::LiveReloadLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/", get(|| async { Html("<h1>Wow, such webdev</h1>") }))
        .layer(LiveReloadLayer::new());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

If you continuously rebuild and rerun this example e.g. using [`watchexec`],
you should see your browser reload whenever the code is changed.

More examples can be found on GitHub under [examples].

[`axum`]: https://docs.rs/axum
[`tower`]: https://docs.rs/tower
[`watchexec`]: https://watchexec.github.io/
[examples]: https://github.com/leotaku/tower-livereload/tree/master/examples

## Manual reload

With the [`Reloader`] utility, it is possible to reload your web browser
entirely using hooks from Rust code. See this [example] on GitHub for
pointers on how to implement a self-contained live-reloading static server.

[example]: https://github.com/leotaku/tower-livereload/blob/master/examples/axum-file-watch/

## Ecosystem compatibility

`tower-livereload` has been built from the ground up to provide the highest
amount of ecosystem compatibility.

The provided middleware uses the [`http`] and [`http_body`] crates as its
HTTP abstractions. That means it is compatible with any library or framework
that also uses those crates, such as [`hyper`], [`axum`], [`tonic`], and
[`warp`].

[`http`]: https://docs.rs/http
[`http_body`]: https://docs.rs/http_body
[`hyper`]: https://docs.rs/hyper
[`axum`]: https://docs.rs/axum
[`tonic`]: https://docs.rs/tonic
[`warp`]: https://docs.rs/warp

## Heuristics

To provide LiveReload functionality, we have to inject code into HTML web
pages. To determine whether a page is injectable, some header-based
heuristics are used. In particular, [`Content-Type`] has to start with
`text/html` and [`Content-Encoding`] must not be set.

If LiveReload is not working for some of your pages, ensure that these
heuristics apply to your responses. In particular, if you use middleware to
compress your HTML, ensure that the [`LiveReload`] middleware is
applied before your compression middleware.

[`Content-Type`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type
[`Content-Encoding`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Encoding

<!-- Override internal links from README generation: -->

[`LiveReload`]: https://docs.rs/tower-livereload/latest/tower_livereload/struct.LiveReload.html
[`Reloader`]: https://docs.rs/tower-livereload/latest/tower_livereload/struct.Reloader.html

## License

`tower-livereload` is free and open source software distributed under the terms of either the MIT or the Apache 2.0 license, at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
