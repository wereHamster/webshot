pub mod auth;
pub mod browser;
pub mod v1;

use chromiumoxide::Browser;
use dropshot::{endpoint, HttpError, HttpResponseOk, RequestContext};
use std::sync::Arc;

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
