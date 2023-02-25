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
        .nest_service("/", ServeDir::new(Path::new("assets")))
        .layer(livereload);

    let mut watcher = notify::recommended_watcher(move |_| reloader.reload())?;
    watcher.watch(Path::new("assets"), notify::RecursiveMode::Recursive)?;

    axum::Server::bind(&"0.0.0.0:3030".parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
