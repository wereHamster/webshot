use biscuit_auth::{
    macros::rule, AuthorizerBuilder, AuthorizerLimits, Biscuit, KeyPair, PrivateKey, PublicKey,
};
use dropshot::{ClientErrorStatusCode, HttpError, RequestContext};
use http::header;
use std::sync::Arc;
use std::time::Duration;

use crate::ServerContext;

pub struct Auth {
    pub public_key: PublicKey,
}

impl Auth {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let private_key_str = std::env::var("BISCUIT_PRIVATE_KEY")
            .map_err(|e| format!("BISCUIT_PRIVATE_KEY environment variable is not set: {}", e))?;
        let private_key: PrivateKey = private_key_str.parse()?;
        let keypair = KeyPair::from(&private_key);
        Ok(Self {
            public_key: keypair.public(),
        })
    }
}

pub(crate) fn extract_token(
    rqctx: &RequestContext<Arc<ServerContext>>,
) -> Result<Biscuit, HttpError> {
    let headers = rqctx.request.headers();
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .ok_or_else(|| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::UNAUTHORIZED,
                "Unauthorized".to_string(),
            )
        })?
        .to_str()
        .map_err(|_| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::BAD_REQUEST,
                "Bad Request".to_string(),
            )
        })?;

    if !auth_header.starts_with("Bearer ") {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::UNAUTHORIZED,
            "Bad Request".to_string(),
        ));
    }

    let token_base64 = &auth_header[7..];
    Biscuit::from_base64(token_base64, rqctx.context().auth.public_key).map_err(|e| {
        tracing::error!("Biscuit parse error: {:?}", e);
        HttpError::for_bad_request(None, "Bad Request".to_string())
    })
}

pub(crate) fn authorize_request(
    biscuit: Biscuit,
    builder: AuthorizerBuilder,
) -> Result<String, HttpError> {
    let mut authorizer = builder.build(&biscuit).map_err(|e| {
        tracing::error!("Biscuit add_token error: {:?}", e);
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::UNAUTHORIZED,
            "Unauthorized".to_string(),
        )
    })?;

    let limits = AuthorizerLimits {
        max_facts: 1000,
        max_iterations: 100,
        max_time: Duration::from_micros(100_000),
    };

    authorizer.authorize_with_limits(limits).map_err(|e| {
        tracing::error!("Biscuit authorization failed: {:?}", e);
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::UNAUTHORIZED,
            "Unauthorized".to_string(),
        )
    })?;

    let facts: Vec<(String,)> =
        authorizer
            .query(rule!("u($user) <- user($user)"))
            .map_err(|_| {
                HttpError::for_client_error(
                    None,
                    ClientErrorStatusCode::UNAUTHORIZED,
                    "Unauthorized".to_string(),
                )
            })?;

    let user = facts.into_iter().next().map(|(u,)| u).ok_or_else(|| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::UNAUTHORIZED,
            "Missing user fact".to_string(),
        )
    })?;

    Ok(user)
}
