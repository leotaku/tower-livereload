use axum::{response::Html, routing::get, Router};

#[cfg(debug_assertions)]
use tower_livereload::LiveReloadLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new().route("/", get(|| async { Html("<h1>Wow, such webdev</h1>") }));

    #[cfg(debug_assertions)]
    let app = app.layer(LiveReloadLayer::new());

    axum::Server::bind(&"0.0.0.0:3030".parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
