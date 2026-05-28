use axum::{extract::FromRequestParts, http::request::Parts};
use biscuit_auth::{
    macros::rule, AuthorizerBuilder, AuthorizerLimits, Biscuit, KeyPair, PrivateKey, PublicKey,
};
use std::sync::Arc;
use std::time::Duration;

use crate::{AppError, ServerContext};

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

pub(crate) trait HasAuth {
    fn auth(&self) -> &Auth;
}

impl HasAuth for Arc<ServerContext> {
    fn auth(&self) -> &Auth {
        &self.auth
    }
}

pub struct ValidBiscuit(pub Biscuit);

impl<S> FromRequestParts<S> for ValidBiscuit
where
    S: Send + Sync + HasAuth,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth = state.auth();
        let auth_header = parts
            .headers
            .get(http::header::AUTHORIZATION)
            .ok_or_else(|| AppError::Unauthorized("Unauthorized".to_string()))?
            .to_str()
            .map_err(|_| AppError::BadRequest("Bad Request".to_string()))?;

        if !auth_header.starts_with("Bearer ") {
            return Err(AppError::Unauthorized("Unauthorized".to_string()));
        }

        let token_base64 = &auth_header[7..];
        match Biscuit::from_base64(token_base64, auth.public_key) {
            Ok(biscuit) => Ok(ValidBiscuit(biscuit)),
            Err(e) => {
                tracing::debug!("Biscuit parse error: {:?}", e);
                Err(AppError::Unauthorized("Unauthorized".to_string()))
            }
        }
    }
}

pub(crate) fn authorize_request(
    biscuit: Biscuit,
    builder: AuthorizerBuilder,
) -> Result<String, AppError> {
    let mut authorizer = builder.build(&biscuit).map_err(|e| {
        tracing::debug!("Biscuit add_token error: {:?}", e);
        AppError::Unauthorized("Unauthorized".to_string())
    })?;

    let limits = AuthorizerLimits {
        max_facts: 1000,
        max_iterations: 100,
        max_time: Duration::from_micros(100_000),
    };

    authorizer.authorize_with_limits(limits).map_err(|e| {
        tracing::debug!("Biscuit authorization failed: {:?}", e);
        AppError::Unauthorized("Unauthorized".to_string())
    })?;

    let facts: Vec<(String,)> = authorizer
        .query(rule!("u($user) <- user($user)"))
        .map_err(|_| AppError::Unauthorized("Unauthorized".to_string()))?;

    let user = facts
        .into_iter()
        .next()
        .map(|(u,)| u)
        .ok_or_else(|| AppError::Unauthorized("Missing user fact".to_string()))?;

    Ok(user)
}
