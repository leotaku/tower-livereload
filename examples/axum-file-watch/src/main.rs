use axum::Router;
use notify::Watcher;
use std::path::Path;
use tower_http::services::ServeDir;
use tower_livereload::LiveReloadLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let livereload = LiveReloadLayer::new();
    let reloader = livereload.reloader();
    let app = Router::new()
        .fallback_service(ServeDir::new(Path::new("assets")))
        .layer(livereload);

    let mut watcher = notify::recommended_watcher(move |event: Result<_, _>| {
        if event.is_ok_and(|it: notify::Event| !it.kind.is_access()) {
            reloader.reload();
        }
    })?;
    watcher.watch(Path::new("assets"), notify::RecursiveMode::Recursive)?;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
