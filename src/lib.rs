pub mod auth;
pub mod browser;
pub mod v1;

use chromiumoxide::Browser;
use dropshot::{endpoint, HttpError, HttpResponseOk, RequestContext};
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

#[endpoint {
    method = GET,
    path = "/",
}]
pub async fn ping(
    _rqctx: RequestContext<Arc<ServerContext>>,
) -> Result<HttpResponseOk<()>, HttpError> {
    Ok(HttpResponseOk(()))
}
