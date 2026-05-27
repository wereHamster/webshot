pub mod auth;
pub mod browser;
pub mod v1;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chromiumoxide::Browser;
use std::sync::Arc;
use tracing_subscriber::prelude::*;

pub fn init_tracing_subscriber() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info,chromiumoxide::handler=error")
    });

    // If we detect that stdout/stderr is connected to journald, use the
    // journald-specific layer.
    //
    // If connecting to journald fails, fall through to the fmt subscriber.
    if std::env::var("JOURNAL_STREAM").is_ok() {
        if let Ok(journald_layer) = tracing_journald::layer() {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(journald_layer)
                .init();
            return;
        }
    }

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}

pub struct ServerContext {
    pub auth: auth::Auth,
    pub browser: Browser,
}

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::Internal(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error".to_string(),
                )
            }
        };

        let body = serde_json::json!({
            "error": message
        });

        (status, axum::Json(body)).into_response()
    }
}

pub async fn ping(_state: axum::extract::State<Arc<ServerContext>>) -> impl IntoResponse {
    StatusCode::OK
}
