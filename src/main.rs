use axum::http::Request;
use axum::response::Response;
use axum::{
    middleware::from_fn,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::Instrument;
use webshot::{auth::Auth, browser, init_tracing_subscriber, ping, v1, ServerContext};

async fn logging_middleware(
    req: Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    let start = std::time::Instant::now();

    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    let span = tracing::info_span!(
        "request",
        http.method = %method,
        http.path = %path,
        http.status = tracing::field::Empty,
        latency = tracing::field::Empty,
    );

    let response = async move { next.run(req).await }
        .instrument(span.clone())
        .await;

    let latency = start.elapsed().as_millis();
    let status = response.status().as_u16();

    span.record("http.status", status);
    span.record("latency", latency);

    span.in_scope(|| {
        tracing::info!("request completed");
    });

    response
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing_subscriber();

    let auth = Auth::from_env()?;
    let browser = browser::launch_browser().await?;

    let context = Arc::new(ServerContext { auth, browser });

    let app = Router::new()
        .route("/", get(ping))
        .route("/v1/render", post(v1::render))
        .route("/v1/capture", post(v1::capture))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024))
        .layer(from_fn(logging_middleware))
        .with_state(context);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()?;

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}
