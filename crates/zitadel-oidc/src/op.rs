#![allow(async_fn_in_trait)]

use crate::oidc::{
    AccessTokenClaims, ClientMetadata, ConsumedAuthRequest, IdTokenClaims, JsonWebKeySet,
    NewAuthRequest, OpenIdConfiguration, ProtocolError, RefreshTokenClaims, SigningKeys,
    TokenResponse, UserClaims, UserInfoResponse, now_epoch_seconds, s256_challenge,
};
use base64::Engine;
use jsonwebtoken::{Algorithm, Header, Validation};
use std::sync::Arc;

pub trait ClientStore: Clone + Send + Sync + 'static {
    async fn find_client(
        &self,
        instance_id: &str,
        client_id: &str,
    ) -> anyhow::Result<Option<ClientMetadata>>;

    async fn authenticate_client_secret(
        &self,
        instance_id: &str,
        client_id: &str,
        client_secret: &str,
    ) -> anyhow::Result<bool>;
}

pub trait AuthRequestStore: Clone + Send + Sync + 'static {
    async fn create_auth_request(
        &self,
        instance_id: &str,
        request: &NewAuthRequest,
    ) -> anyhow::Result<String>;

    async fn consume_auth_code(
        &self,
        instance_id: &str,
        code: &str,
    ) -> anyhow::Result<Option<ConsumedAuthRequest>>;
}

pub trait ClaimSource: Clone + Send + Sync + 'static {
    async fn load_user_claims(
        &self,
        instance_id: &str,
        subject: &str,
    ) -> anyhow::Result<Option<UserClaims>>;
}

pub trait KeyStore: Clone + Send + Sync + 'static {
    async fn active_signing_key(&self, instance_id: &str) -> anyhow::Result<Arc<SigningKeys>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizeRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub scope: String,
    pub state: String,
    pub nonce: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub prompt: Vec<String>,
    pub login_hint: String,
    pub max_age: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizeRedirect {
    pub location: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientAuthMethod {
    ClientSecretBasic,
    ClientSecretPost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientAuthentication {
    pub client_id: String,
    pub client_secret: String,
    pub method: ClientAuthMethod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenExchangeRequest {
    pub grant_type: String,
    pub code: String,
    pub redirect_uri: String,
    pub client_auth: Option<ClientAuthentication>,
    pub code_verifier: String,
    pub refresh_token: String,
}

/// Token lifetime configuration, sourced from `OidcConfig`.
#[derive(Debug, Clone)]
pub struct TokenLifetimes {
    pub access_token_secs: u64,
    pub id_token_secs: u64,
    pub refresh_token_secs: u64,
}

impl Default for TokenLifetimes {
    fn default() -> Self {
        Self {
            access_token_secs: 12 * 3600,
            id_token_secs: 12 * 3600,
            refresh_token_secs: 30 * 24 * 3600,
        }
    }
}

impl From<&zitadel_config::oidc::OidcConfig> for TokenLifetimes {
    fn from(cfg: &zitadel_config::oidc::OidcConfig) -> Self {
        Self {
            access_token_secs: cfg.access_token_lifetime_secs,
            id_token_secs: cfg.id_token_lifetime_secs,
            refresh_token_secs: cfg.refresh_token_max_secs,
        }
    }
}

#[derive(Clone)]
pub struct Provider<C, A, K, U> {
    instance_id: String,
    issuer: String,
    login_path: String,
    clients: C,
    auth_requests: A,
    keys: K,
    claims: U,
    lifetimes: TokenLifetimes,
}

impl<C, A, K, U> Provider<C, A, K, U> {
    pub fn new(
        instance_id: String,
        issuer: String,
        login_path: String,
        clients: C,
        auth_requests: A,
        keys: K,
        claims: U,
    ) -> Self {
        Self {
            instance_id,
            issuer,
            login_path,
            clients,
            auth_requests,
            keys,
            claims,
            lifetimes: TokenLifetimes::default(),
        }
    }

    pub fn with_lifetimes(mut self, lifetimes: TokenLifetimes) -> Self {
        self.lifetimes = lifetimes;
        self
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    pub fn discovery_document(&self) -> OpenIdConfiguration {
        let issuer = self.issuer.clone();
        OpenIdConfiguration {
            issuer: issuer.clone(),
            authorization_endpoint: format!("{issuer}/authorize"),
            token_endpoint: format!("{issuer}/oauth/token"),
            userinfo_endpoint: format!("{issuer}/userinfo"),
            jwks_uri: format!("{issuer}/keys"),
            revocation_endpoint: format!("{issuer}/revoke"),
            end_session_endpoint: format!("{issuer}/end_session"),
            response_types_supported: vec!["code".into()],
            grant_types_supported: vec![
                "authorization_code".into(),
                "client_credentials".into(),
                "refresh_token".into(),
            ],
            subject_types_supported: vec!["public".into()],
            id_token_signing_alg_values_supported: vec!["RS256".into()],
            scopes_supported: vec![
                "openid".into(),
                "profile".into(),
                "email".into(),
                "offline_access".into(),
            ],
            token_endpoint_auth_methods_supported: vec![
                "client_secret_post".into(),
                "client_secret_basic".into(),
                "none".into(),
            ],
            code_challenge_methods_supported: vec!["S256".into()],
            claims_supported: vec![
                "sub".into(),
                "iss".into(),
                "aud".into(),
                "exp".into(),
                "iat".into(),
                "auth_time".into(),
                "name".into(),
                "email".into(),
                "locale".into(),
            ],
        }
    }
}

impl<C, A, K, U> Provider<C, A, K, U>
where
    C: ClientStore,
    A: AuthRequestStore,
    K: KeyStore,
    U: ClaimSource,
{
    pub async fn jwks(&self) -> Result<JsonWebKeySet, ProtocolError> {
        let key = self
            .keys
            .active_signing_key(&self.instance_id)
            .await
            .map_err(|error| {
                ProtocolError::temporarily_unavailable(format!("signing keys: {error}"))
            })?;
        Ok(JsonWebKeySet {
            keys: vec![key.jwk()],
        })
    }

    pub async fn authorize(
        &self,
        request: &AuthorizeRequest,
    ) -> Result<AuthorizeRedirect, ProtocolError> {
        if request.client_id.is_empty() {
            return Err(ProtocolError::invalid_request("client_id required"));
        }
        if request.redirect_uri.is_empty() {
            return Err(ProtocolError::invalid_request("redirect_uri required"));
        }
        if request.response_type != "code" {
            return Err(ProtocolError::unsupported_response_type(
                "only response_type=code is supported",
            ));
        }
        if !request.code_challenge_method.is_empty() && request.code_challenge_method != "S256" {
            return Err(ProtocolError::invalid_request(
                "only code_challenge_method=S256 is supported",
            ));
        }

        let client = self
            .clients
            .find_client(&self.instance_id, &request.client_id)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load client: {error}")))?;

        let client = match client {
            Some(client) if client.state == "active" => client,
            _ => return Err(ProtocolError::invalid_client("unknown client_id")),
        };

        if !client
            .redirect_uris
            .iter()
            .any(|uri| uri == &request.redirect_uri)
        {
            return Err(ProtocolError::invalid_request(
                "redirect_uri is not registered",
            ));
        }

        let auth_request_id = self
            .auth_requests
            .create_auth_request(
                &self.instance_id,
                &NewAuthRequest {
                    client_id: request.client_id.clone(),
                    redirect_uri: request.redirect_uri.clone(),
                    response_type: request.response_type.clone(),
                    scope: if request.scope.is_empty() {
                        "openid".to_string()
                    } else {
                        request.scope.clone()
                    },
                    state: request.state.clone(),
                    nonce: request.nonce.clone(),
                    code_challenge: request.code_challenge.clone(),
                    code_challenge_method: request.code_challenge_method.clone(),
                    prompt: request.prompt.clone(),
                    login_hint: request.login_hint.clone(),
                    max_age: request.max_age,
                },
            )
            .await
            .map_err(|error| {
                ProtocolError::server_error(format!("create auth request: {error}"))
            })?;

        Ok(AuthorizeRedirect {
            location: format!("{}?auth_request_id={auth_request_id}", self.login_path),
        })
    }

    pub async fn allows_authorization_error_redirect(
        &self,
        client_id: &str,
        redirect_uri: &str,
    ) -> bool {
        if client_id.is_empty() || redirect_uri.is_empty() {
            return false;
        }

        let Ok(Some(client)) = self.clients.find_client(&self.instance_id, client_id).await else {
            return false;
        };

        client.state == "active" && client.redirect_uris.iter().any(|uri| uri == redirect_uri)
    }

    pub async fn token(
        &self,
        request: &TokenExchangeRequest,
    ) -> Result<TokenResponse, ProtocolError> {
        match request.grant_type.as_str() {
            "authorization_code" => self.exchange_authorization_code(request).await,
            "client_credentials" => self.exchange_client_credentials(request).await,
            "refresh_token" => self.exchange_refresh_token(request).await,
            _ => Err(ProtocolError::unsupported_grant_type(
                "unsupported grant_type",
            )),
        }
    }

    pub async fn validate_access_token(
        &self,
        access_token: &str,
    ) -> Result<AccessTokenClaims, ProtocolError> {
        if access_token.is_empty() {
            return Err(ProtocolError::invalid_request("Bearer token required"));
        }

        let key = self
            .keys
            .active_signing_key(&self.instance_id)
            .await
            .map_err(|error| {
                ProtocolError::temporarily_unavailable(format!("signing keys: {error}"))
            })?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_aud = false;
        validation.set_issuer(&[&self.issuer]);

        jsonwebtoken::decode::<AccessTokenClaims>(access_token, &key.decoding, &validation)
            .map(|token| token.claims)
            .map_err(|_| ProtocolError::invalid_grant("invalid access token"))
    }

    async fn validate_refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<RefreshTokenClaims, ProtocolError> {
        if refresh_token.is_empty() {
            return Err(ProtocolError::invalid_request("refresh_token required"));
        }

        let key = self
            .keys
            .active_signing_key(&self.instance_id)
            .await
            .map_err(|error| {
                ProtocolError::temporarily_unavailable(format!("signing keys: {error}"))
            })?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_aud = false;
        validation.set_issuer(&[&self.issuer]);

        jsonwebtoken::decode::<RefreshTokenClaims>(refresh_token, &key.decoding, &validation)
            .map(|token| token.claims)
            .map_err(|_| ProtocolError::invalid_grant("invalid refresh token"))
    }

    pub async fn userinfo(&self, access_token: &str) -> Result<UserInfoResponse, ProtocolError> {
        let token = self.validate_access_token(access_token).await?;

        let claims = self
            .claims
            .load_user_claims(&self.instance_id, &token.sub)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load user claims: {error}")))?;

        let claims = claims.ok_or_else(|| ProtocolError::invalid_grant("subject not found"))?;

        Ok(UserInfoResponse {
            sub: claims.subject,
            name: claims.name,
            email: claims.email,
            email_verified: claims.email_verified,
        })
    }

    async fn issue_id_token(
        &self,
        key: &SigningKeys,
        user: &UserClaims,
        client_id: &str,
        nonce: &str,
        auth_time: Option<u64>,
    ) -> Result<String, ProtocolError> {
        let now = now_epoch_seconds();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(key.kid.clone());

        jsonwebtoken::encode(
            &header,
            &IdTokenClaims {
                iss: self.issuer.clone(),
                sub: user.subject.clone(),
                aud: client_id.to_string(),
                exp: now + self.lifetimes.id_token_secs,
                iat: now,
                auth_time,
                nonce: nonce.to_string(),
                name: user.name.clone(),
                email: user.email.clone(),
            },
            &key.encoding,
        )
        .map_err(|error| ProtocolError::server_error(format!("id_token: {error}")))
    }

    async fn issue_access_token(
        &self,
        key: &SigningKeys,
        subject: &str,
        client_id: &str,
        scope: &str,
    ) -> Result<String, ProtocolError> {
        let now = now_epoch_seconds();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(key.kid.clone());

        jsonwebtoken::encode(
            &header,
            &AccessTokenClaims {
                iss: self.issuer.clone(),
                sub: subject.to_string(),
                aud: client_id.to_string(),
                exp: now + self.lifetimes.access_token_secs,
                iat: now,
                scope: scope.to_string(),
                client_id: client_id.to_string(),
            },
            &key.encoding,
        )
        .map_err(|error| ProtocolError::server_error(format!("access_token: {error}")))
    }

    async fn issue_refresh_token(
        &self,
        key: &SigningKeys,
        subject: &str,
        client_id: &str,
        scope: &str,
    ) -> Result<String, ProtocolError> {
        let now = now_epoch_seconds();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(key.kid.clone());

        jsonwebtoken::encode(
            &header,
            &RefreshTokenClaims {
                iss: self.issuer.clone(),
                sub: subject.to_string(),
                aud: client_id.to_string(),
                exp: now + self.lifetimes.refresh_token_secs,
                iat: now,
                scope: scope.to_string(),
                client_id: client_id.to_string(),
            },
            &key.encoding,
        )
        .map_err(|error| ProtocolError::server_error(format!("refresh_token: {error}")))
    }

    async fn exchange_authorization_code(
        &self,
        request: &TokenExchangeRequest,
    ) -> Result<TokenResponse, ProtocolError> {
        let auth = request
            .client_auth
            .as_ref()
            .ok_or_else(|| ProtocolError::invalid_client("client_id required"))?;

        let client = self
            .clients
            .find_client(&self.instance_id, &auth.client_id)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load client: {error}")))?;
        let client = client.ok_or_else(|| ProtocolError::invalid_client("unknown client_id"))?;

        let authenticated = if client.client_secret.is_empty() {
            auth.client_secret.is_empty()
        } else {
            self.clients
                .authenticate_client_secret(&self.instance_id, &auth.client_id, &auth.client_secret)
                .await
                .map_err(|error| {
                    ProtocolError::server_error(format!("authenticate client: {error}"))
                })?
        };

        if !authenticated {
            return Err(ProtocolError::invalid_client("invalid client credentials"));
        }
        if !client
            .grant_types
            .iter()
            .any(|grant| grant == "authorization_code")
        {
            return Err(ProtocolError::unauthorized_client(
                "client is not allowed to use authorization_code",
            ));
        }

        let granted = self
            .auth_requests
            .consume_auth_code(&self.instance_id, &request.code)
            .await
            .map_err(|error| ProtocolError::server_error(format!("consume auth code: {error}")))?;

        let granted =
            granted.ok_or_else(|| ProtocolError::invalid_grant("authorization code not found"))?;

        if granted.client_id != auth.client_id {
            return Err(ProtocolError::invalid_grant(
                "authorization code client mismatch",
            ));
        }
        if !request.redirect_uri.is_empty() && request.redirect_uri != granted.redirect_uri {
            return Err(ProtocolError::invalid_grant("redirect_uri mismatch"));
        }
        if !client
            .redirect_uris
            .iter()
            .any(|uri| uri == &granted.redirect_uri)
        {
            return Err(ProtocolError::invalid_grant(
                "redirect_uri is not registered",
            ));
        }
        if !granted.code_challenge.is_empty() {
            if request.code_verifier.is_empty() {
                return Err(ProtocolError::invalid_grant("code_verifier required"));
            }
            if s256_challenge(&request.code_verifier) != granted.code_challenge {
                return Err(ProtocolError::invalid_grant("PKCE verification failed"));
            }
        }

        let user = self
            .claims
            .load_user_claims(&self.instance_id, &granted.user_id)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load user claims: {error}")))?;

        let user = user.ok_or_else(|| ProtocolError::invalid_grant("subject not found"))?;
        let key = self
            .keys
            .active_signing_key(&self.instance_id)
            .await
            .map_err(|error| {
                ProtocolError::temporarily_unavailable(format!("signing keys: {error}"))
            })?;

        let id_token = self
            .issue_id_token(
                &key,
                &user,
                &auth.client_id,
                &granted.nonce,
                granted.auth_time,
            )
            .await?;
        let access_token = self
            .issue_access_token(&key, &user.subject, &auth.client_id, &granted.scope)
            .await?;
        let refresh_token = if client
            .grant_types
            .iter()
            .any(|grant| grant == "refresh_token")
            && scope_contains(&granted.scope, "offline_access")
        {
            Some(
                self.issue_refresh_token(&key, &user.subject, &auth.client_id, &granted.scope)
                    .await?,
            )
        } else {
            None
        };

        Ok(TokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.lifetimes.access_token_secs,
            id_token: Some(id_token),
            refresh_token,
            scope: granted.scope,
        })
    }

    async fn exchange_client_credentials(
        &self,
        request: &TokenExchangeRequest,
    ) -> Result<TokenResponse, ProtocolError> {
        let auth = request
            .client_auth
            .as_ref()
            .ok_or_else(|| ProtocolError::invalid_client("client authentication required"))?;

        let client = self
            .clients
            .find_client(&self.instance_id, &auth.client_id)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load client: {error}")))?;
        let client = client.ok_or_else(|| ProtocolError::invalid_client("unknown client_id"))?;

        if !client
            .grant_types
            .iter()
            .any(|grant| grant == "client_credentials")
        {
            return Err(ProtocolError::unauthorized_client(
                "client is not allowed to use client_credentials",
            ));
        }

        let authenticated = self
            .clients
            .authenticate_client_secret(&self.instance_id, &auth.client_id, &auth.client_secret)
            .await
            .map_err(|error| {
                ProtocolError::server_error(format!("authenticate client: {error}"))
            })?;
        if !authenticated {
            return Err(ProtocolError::invalid_client("invalid client credentials"));
        }

        let key = self
            .keys
            .active_signing_key(&self.instance_id)
            .await
            .map_err(|error| {
                ProtocolError::temporarily_unavailable(format!("signing keys: {error}"))
            })?;

        let access_token = self
            .issue_access_token(&key, &auth.client_id, &auth.client_id, "openid")
            .await?;

        Ok(TokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.lifetimes.access_token_secs,
            id_token: None,
            refresh_token: None,
            scope: "openid".to_string(),
        })
    }

    async fn exchange_refresh_token(
        &self,
        request: &TokenExchangeRequest,
    ) -> Result<TokenResponse, ProtocolError> {
        let auth = request
            .client_auth
            .as_ref()
            .ok_or_else(|| ProtocolError::invalid_client("client authentication required"))?;

        let client = self
            .clients
            .find_client(&self.instance_id, &auth.client_id)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load client: {error}")))?;
        let client = client.ok_or_else(|| ProtocolError::invalid_client("unknown client_id"))?;

        if !client
            .grant_types
            .iter()
            .any(|grant| grant == "refresh_token")
        {
            return Err(ProtocolError::unauthorized_client(
                "client is not allowed to use refresh_token",
            ));
        }

        let authenticated = if client.client_secret.is_empty() {
            auth.client_secret.is_empty()
        } else {
            self.clients
                .authenticate_client_secret(&self.instance_id, &auth.client_id, &auth.client_secret)
                .await
                .map_err(|error| {
                    ProtocolError::server_error(format!("authenticate client: {error}"))
                })?
        };
        if !authenticated {
            return Err(ProtocolError::invalid_client("invalid client credentials"));
        }

        let refresh = self.validate_refresh_token(&request.refresh_token).await?;
        if refresh.client_id != auth.client_id {
            return Err(ProtocolError::invalid_grant(
                "refresh token client mismatch",
            ));
        }

        let user = self
            .claims
            .load_user_claims(&self.instance_id, &refresh.sub)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load user claims: {error}")))?;
        let user = user.ok_or_else(|| ProtocolError::invalid_grant("subject not found"))?;

        let key = self
            .keys
            .active_signing_key(&self.instance_id)
            .await
            .map_err(|error| {
                ProtocolError::temporarily_unavailable(format!("signing keys: {error}"))
            })?;

        let access_token = self
            .issue_access_token(&key, &user.subject, &auth.client_id, &refresh.scope)
            .await?;
        let refresh_token = if scope_contains(&refresh.scope, "offline_access") {
            Some(
                self.issue_refresh_token(&key, &user.subject, &auth.client_id, &refresh.scope)
                    .await?,
            )
        } else {
            None
        };

        Ok(TokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.lifetimes.access_token_secs,
            id_token: None,
            refresh_token,
            scope: refresh.scope,
        })
    }
}

fn scope_contains(scope: &str, needle: &str) -> bool {
    scope.split_whitespace().any(|part| part == needle)
}

pub fn resolve_client_auth(
    authorization_header: Option<&str>,
    form_client_id: &str,
    form_client_secret: &str,
) -> Result<Option<ClientAuthentication>, ProtocolError> {
    if let Some(header) = authorization_header
        && let Some(value) = header.strip_prefix("Basic ")
    {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(value)
            .map_err(|_| ProtocolError::invalid_client("invalid basic authorization encoding"))?;
        let decoded = String::from_utf8(decoded)
            .map_err(|_| ProtocolError::invalid_client("invalid basic authorization payload"))?;
        let mut parts = decoded.splitn(2, ':');
        let client_id = parts.next().unwrap_or_default().to_string();
        let client_secret = parts.next().unwrap_or_default().to_string();
        if client_id.is_empty() {
            return Err(ProtocolError::invalid_client("client_id required"));
        }
        return Ok(Some(ClientAuthentication {
            client_id,
            client_secret,
            method: ClientAuthMethod::ClientSecretBasic,
        }));
    }

    if form_client_id.is_empty() && form_client_secret.is_empty() {
        return Ok(None);
    }

    if form_client_id.is_empty() {
        return Err(ProtocolError::invalid_client("client_id required"));
    }

    Ok(Some(ClientAuthentication {
        client_id: form_client_id.to_string(),
        client_secret: form_client_secret.to_string(),
        method: ClientAuthMethod::ClientSecretPost,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oidc::{
        ClientMetadata, ConsumedAuthRequest, IdTokenClaims, SigningKeys, UserClaims,
    };
    use base64::Engine;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct FakeClientStore {
        client: Option<ClientMetadata>,
    }

    impl ClientStore for FakeClientStore {
        async fn find_client(
            &self,
            _instance_id: &str,
            _client_id: &str,
        ) -> anyhow::Result<Option<ClientMetadata>> {
            Ok(self.client.clone())
        }

        async fn authenticate_client_secret(
            &self,
            _instance_id: &str,
            client_id: &str,
            client_secret: &str,
        ) -> anyhow::Result<bool> {
            Ok(self
                .client
                .as_ref()
                .map(|client| {
                    client.client_id == client_id && client.client_secret == client_secret
                })
                .unwrap_or(false))
        }
    }

    #[derive(Clone, Default)]
    struct FakeAuthRequestStore {
        created_ids: Arc<Mutex<Vec<String>>>,
        consumed: Arc<Mutex<VecDeque<ConsumedAuthRequest>>>,
    }

    impl AuthRequestStore for FakeAuthRequestStore {
        async fn create_auth_request(
            &self,
            _instance_id: &str,
            _request: &NewAuthRequest,
        ) -> anyhow::Result<String> {
            let id = "auth-1".to_string();
            self.created_ids.lock().unwrap().push(id.clone());
            Ok(id)
        }

        async fn consume_auth_code(
            &self,
            _instance_id: &str,
            _code: &str,
        ) -> anyhow::Result<Option<ConsumedAuthRequest>> {
            Ok(self.consumed.lock().unwrap().pop_front())
        }
    }

    #[derive(Clone)]
    struct StaticKeyStore {
        key: Arc<SigningKeys>,
    }

    impl KeyStore for StaticKeyStore {
        async fn active_signing_key(&self, _instance_id: &str) -> anyhow::Result<Arc<SigningKeys>> {
            Ok(self.key.clone())
        }
    }

    #[derive(Clone, Default)]
    struct FakeClaimSource {
        claims: Option<UserClaims>,
    }

    impl ClaimSource for FakeClaimSource {
        async fn load_user_claims(
            &self,
            _instance_id: &str,
            _subject: &str,
        ) -> anyhow::Result<Option<UserClaims>> {
            Ok(self.claims.clone())
        }
    }

    fn test_provider(
        consumed: Option<ConsumedAuthRequest>,
    ) -> Provider<FakeClientStore, FakeAuthRequestStore, StaticKeyStore, FakeClaimSource> {
        Provider::new(
            "default".to_string(),
            "http://issuer.example".to_string(),
            "/login".to_string(),
            FakeClientStore {
                client: Some(ClientMetadata {
                    client_id: "client".to_string(),
                    client_secret: "secret".to_string(),
                    redirect_uris: vec!["https://app.example/callback".to_string()],
                    grant_types: vec![
                        "authorization_code".to_string(),
                        "client_credentials".to_string(),
                        "refresh_token".to_string(),
                    ],
                    response_types: vec!["code".to_string()],
                    state: "active".to_string(),
                }),
            },
            FakeAuthRequestStore {
                created_ids: Arc::default(),
                consumed: Arc::new(Mutex::new(consumed.into_iter().collect::<VecDeque<_>>())),
            },
            StaticKeyStore {
                key: SigningKeys::generate().unwrap().shared(),
            },
            FakeClaimSource {
                claims: Some(UserClaims {
                    subject: "user-1".to_string(),
                    name: "Alice".to_string(),
                    email: "alice@example.com".to_string(),
                    email_verified: true,
                }),
            },
        )
    }

    #[tokio::test]
    async fn authorize_rejects_unregistered_redirect_uri() {
        let provider = test_provider(None);
        let err = provider
            .authorize(&AuthorizeRequest {
                client_id: "client".to_string(),
                redirect_uri: "https://evil.example/callback".to_string(),
                response_type: "code".to_string(),
                scope: "openid".to_string(),
                state: "state".to_string(),
                nonce: "nonce".to_string(),
                code_challenge: String::new(),
                code_challenge_method: String::new(),
                prompt: Vec::new(),
                login_hint: String::new(),
                max_age: None,
            })
            .await
            .unwrap_err();

        assert_eq!(err.body.error, "invalid_request");
    }

    #[tokio::test]
    async fn token_rejects_wrong_pkce_verifier() {
        let provider = test_provider(Some(ConsumedAuthRequest {
            auth_request_id: "auth-1".to_string(),
            user_id: "user-1".to_string(),
            client_id: "client".to_string(),
            redirect_uri: "https://app.example/callback".to_string(),
            scope: "openid profile".to_string(),
            nonce: "nonce".to_string(),
            code_challenge: s256_challenge("expected"),
            auth_time: Some(1_700_000_000),
        }));

        let err = provider
            .token(&TokenExchangeRequest {
                grant_type: "authorization_code".to_string(),
                code: "code".to_string(),
                redirect_uri: "https://app.example/callback".to_string(),
                client_auth: Some(ClientAuthentication {
                    client_id: "client".to_string(),
                    client_secret: "secret".to_string(),
                    method: ClientAuthMethod::ClientSecretPost,
                }),
                code_verifier: "wrong".to_string(),
                refresh_token: String::new(),
            })
            .await
            .unwrap_err();

        assert_eq!(err.body.error, "invalid_grant");
    }

    #[tokio::test]
    async fn token_uses_consumed_auth_time_in_id_token() {
        let provider = test_provider(Some(ConsumedAuthRequest {
            auth_request_id: "auth-1".to_string(),
            user_id: "user-1".to_string(),
            client_id: "client".to_string(),
            redirect_uri: "https://app.example/callback".to_string(),
            scope: "openid".to_string(),
            nonce: "nonce".to_string(),
            code_challenge: String::new(),
            auth_time: Some(1_700_000_123),
        }));

        let response = provider
            .token(&TokenExchangeRequest {
                grant_type: "authorization_code".to_string(),
                code: "code".to_string(),
                redirect_uri: "https://app.example/callback".to_string(),
                client_auth: Some(ClientAuthentication {
                    client_id: "client".to_string(),
                    client_secret: "secret".to_string(),
                    method: ClientAuthMethod::ClientSecretPost,
                }),
                code_verifier: String::new(),
                refresh_token: String::new(),
            })
            .await
            .unwrap();

        let id_token = response.id_token.expect("id token");
        let key = provider
            .keys
            .active_signing_key(&provider.instance_id)
            .await
            .unwrap();
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_aud = false;
        validation.set_issuer(&[provider.issuer.as_str()]);
        let claims = jsonwebtoken::decode::<IdTokenClaims>(&id_token, &key.decoding, &validation)
            .unwrap()
            .claims;

        assert_eq!(claims.auth_time, Some(1_700_000_123));
    }

    #[test]
    fn resolve_client_auth_from_basic_header() {
        let header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("client:secret")
        );
        let auth = resolve_client_auth(Some(&header), "", "").unwrap().unwrap();
        assert_eq!(auth.client_id, "client");
        assert_eq!(auth.client_secret, "secret");
        assert_eq!(auth.method, ClientAuthMethod::ClientSecretBasic);
    }
}
