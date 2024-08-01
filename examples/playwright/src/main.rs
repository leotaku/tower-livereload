use axum::{
    response::Html,
    routing::{get, post},
    Router,
};
use tower_livereload::LiveReloadLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let livereload = LiveReloadLayer::new();
    let reloader = livereload.reloader();
    let app = Router::new()
        .route("/", get(|| async { Html("<h1>Playwright!</h1>") }))
        .route(
            "/reload",
            post(|| async move {
                reloader.reload();
            }),
        )
        .layer(livereload);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
