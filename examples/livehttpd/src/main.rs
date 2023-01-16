use axum::{http, routing::get_service, Router};
use clap::Parser;
use notify::Watcher;
use tower::layer::util::Stack;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_livereload::LiveReloadLayer;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(color=clap::ColorChoice::Never)]
struct Command {
    #[arg(short = 'a', long = "addr", default_value = "0.0.0.0")]
    #[arg(help = "Address to listen on", hide_default_value = true)]
    addr: std::net::IpAddr,

    #[arg(short = 'p', long = "port", default_value = "8080")]
    #[arg(help = "Port to listen on", hide_default_value = true)]
    port: u16,

    #[arg(help = "Path to serve as HTTP root")]
    directory: std::path::PathBuf,
}

fn serve_dir(path: &std::path::Path) -> axum::routing::MethodRouter {
    get_service(ServeDir::new(path)).handle_error(|error| async move {
        (
            http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unhandled internal error: {}", error),
        )
    })
}

type Srhl = SetResponseHeaderLayer<http::HeaderValue>;

fn no_cache_layer() -> Stack<Srhl, Stack<Srhl, Srhl>> {
    Stack::new(
        SetResponseHeaderLayer::overriding(
            http::header::CACHE_CONTROL,
            http::HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        ),
        Stack::new(
            SetResponseHeaderLayer::overriding(
                http::header::PRAGMA,
                http::HeaderValue::from_static("no-cache"),
            ),
            SetResponseHeaderLayer::overriding(
                http::header::EXPIRES,
                http::HeaderValue::from_static("0"),
            ),
        ),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Command::parse();

    let livereload = LiveReloadLayer::new();
    let reloader = livereload.reloader();
    let app = Router::new()
        .nest_service("/", serve_dir(&args.directory))
        .layer(livereload)
        .layer(no_cache_layer());

    let mut watcher = notify::recommended_watcher(move |_| reloader.reload())?;
    watcher.watch(&args.directory, notify::RecursiveMode::Recursive)?;

    let addr = (args.addr, args.port).into();
    eprintln!("listening on: http://{}/", addr);

    tracing_subscriber::fmt::init();
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
