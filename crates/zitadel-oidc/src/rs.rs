#![allow(async_fn_in_trait)]

use crate::oidc::JsonWebKeySet;
use serde_json::Value;

pub trait KeySource: Clone + Send + Sync + 'static {
    async fn fetch_jwks(&self, issuer: &str) -> anyhow::Result<JsonWebKeySet>;
}

pub trait IntrospectionClient: Clone + Send + Sync + 'static {
    async fn introspect(&self, issuer: &str, token: &str) -> anyhow::Result<Value>;
}
