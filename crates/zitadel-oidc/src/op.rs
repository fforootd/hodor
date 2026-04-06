#![allow(async_fn_in_trait)]

use crate::oidc::{
    AccessTokenClaims, ClientMetadata, ConsumedAuthRequest, IdTokenClaims, JsonWebKeySet,
    NewAuthRequest, OpenIdConfiguration, ProtocolError, RefreshTokenClaims, SigningKeys,
    TokenResponse, UserClaims, UserInfoResponse, now_epoch_seconds, s256_challenge,
};
use base64::Engine;
use jsonwebtoken::{Algorithm, Header, Validation, decode, decode_header};
use std::{borrow::Cow, sync::Arc};
use url::Url;
use uuid::Uuid;
use zitadel_db::{current_instance_id_or, current_request_origin_or};

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
    async fn signing_keys(&self, instance_id: &str) -> anyhow::Result<Vec<Arc<SigningKeys>>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredToken {
    pub token_id: String,
    pub token_type: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub client_id: String,
    pub application_id: String,
    pub scope: String,
    pub refresh_family_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewStoredToken {
    pub token_id: String,
    pub token_type: String,
    pub raw_token: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub client_id: String,
    pub application_id: String,
    pub scope: String,
    pub refresh_family_id: Option<String>,
    pub auth_method: String,
    pub expires_in_secs: u64,
}

pub trait TokenStore: Clone + Send + Sync + 'static {
    fn enforces_storage(&self) -> bool {
        true
    }

    async fn store_token(&self, instance_id: &str, token: &NewStoredToken) -> anyhow::Result<()>;

    async fn lookup_active_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<StoredToken>>;

    async fn revoke_token_by_id(&self, instance_id: &str, token_id: &str) -> anyhow::Result<()>;

    async fn revoke_refresh_family(
        &self,
        instance_id: &str,
        refresh_family_id: &str,
    ) -> anyhow::Result<()>;

    async fn revoke_session_tokens(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<()>;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndSessionRequest {
    pub client_id: String,
    pub id_token_hint: String,
    pub post_logout_redirect_uri: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndSessionOutcome {
    pub session_id: Option<String>,
    pub redirect_uri: String,
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
pub struct Provider<C, A, K, U, T> {
    instance_id: String,
    issuer: String,
    issuer_override: Option<String>,
    login_path: String,
    clients: C,
    auth_requests: A,
    keys: K,
    claims: U,
    tokens: T,
    lifetimes: TokenLifetimes,
}

impl<C, A, K, U, T> Provider<C, A, K, U, T> {
    pub fn new(
        instance_id: String,
        issuer: String,
        login_path: String,
        clients: C,
        auth_requests: A,
        keys: K,
        claims: U,
        tokens: T,
    ) -> Self {
        Self {
            instance_id,
            issuer,
            issuer_override: None,
            login_path,
            clients,
            auth_requests,
            keys,
            claims,
            tokens,
            lifetimes: TokenLifetimes::default(),
        }
    }

    pub fn with_lifetimes(mut self, lifetimes: TokenLifetimes) -> Self {
        self.lifetimes = lifetimes;
        self
    }

    pub fn with_issuer_override(mut self, issuer_override: Option<String>) -> Self {
        self.issuer_override =
            issuer_override.map(|origin| origin.trim_end_matches('/').to_string());
        self
    }

    pub fn issuer(&self) -> Cow<'_, str> {
        self.effective_issuer()
    }

    fn effective_instance_id(&self) -> Cow<'_, str> {
        current_instance_id_or(&self.instance_id)
    }

    fn effective_issuer(&self) -> Cow<'_, str> {
        if let Some(issuer_override) = self.issuer_override.as_deref() {
            Cow::Borrowed(issuer_override)
        } else {
            current_request_origin_or(&self.issuer)
        }
    }

    pub fn discovery_document(&self) -> OpenIdConfiguration {
        let issuer = self.effective_issuer().into_owned();
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

impl<C, A, K, U, T> Provider<C, A, K, U, T>
where
    C: ClientStore,
    A: AuthRequestStore,
    K: KeyStore,
    U: ClaimSource,
    T: TokenStore,
{
    pub async fn jwks(&self) -> Result<JsonWebKeySet, ProtocolError> {
        let instance_id = self.effective_instance_id();
        let keys = self
            .keys
            .signing_keys(instance_id.as_ref())
            .await
            .map_err(|error| {
                ProtocolError::temporarily_unavailable(format!("signing keys: {error}"))
            })?;
        Ok(JsonWebKeySet {
            keys: keys.into_iter().map(|key| key.jwk()).collect(),
        })
    }

    pub async fn authorize(
        &self,
        request: &AuthorizeRequest,
    ) -> Result<AuthorizeRedirect, ProtocolError> {
        let instance_id = self.effective_instance_id();
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
            .find_client(instance_id.as_ref(), &request.client_id)
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
                instance_id.as_ref(),
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
        let instance_id = self.effective_instance_id();
        if client_id.is_empty() || redirect_uri.is_empty() {
            return false;
        }

        let Ok(Some(client)) = self
            .clients
            .find_client(instance_id.as_ref(), client_id)
            .await
        else {
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

    pub async fn revoke(
        &self,
        token: &str,
        client_auth: Option<&ClientAuthentication>,
    ) -> Result<(), ProtocolError> {
        let instance_id = self.effective_instance_id();
        let auth = client_auth
            .ok_or_else(|| ProtocolError::invalid_client("client authentication required"))?;
        let client = self
            .clients
            .find_client(instance_id.as_ref(), &auth.client_id)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load client: {error}")))?;
        let Some(client) = client else {
            return Err(ProtocolError::invalid_client("unknown client_id"));
        };
        let authenticated = if client.client_secret.is_empty() {
            auth.client_secret.is_empty()
        } else {
            self.clients
                .authenticate_client_secret(
                    instance_id.as_ref(),
                    &auth.client_id,
                    &auth.client_secret,
                )
                .await
                .map_err(|error| {
                    ProtocolError::server_error(format!("authenticate client: {error}"))
                })?
        };
        if !authenticated {
            return Err(ProtocolError::invalid_client("invalid client credentials"));
        }
        if token.is_empty() {
            return Err(ProtocolError::invalid_request("token required"));
        }

        let Some(stored) = self
            .tokens
            .lookup_active_token(instance_id.as_ref(), token)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load token record: {error}")))?
        else {
            return Ok(());
        };
        if stored.client_id != auth.client_id {
            return Ok(());
        }

        match stored.token_type.as_str() {
            "oidc_refresh" => {
                let family_id = stored
                    .refresh_family_id
                    .as_deref()
                    .unwrap_or(&stored.token_id);
                self.tokens
                    .revoke_refresh_family(instance_id.as_ref(), family_id)
                    .await
                    .map_err(|error| {
                        ProtocolError::server_error(format!("revoke refresh family: {error}"))
                    })?;
            }
            "oidc_access" => {
                self.tokens
                    .revoke_token_by_id(instance_id.as_ref(), &stored.token_id)
                    .await
                    .map_err(|error| {
                        ProtocolError::server_error(format!("revoke access token: {error}"))
                    })?;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn end_session(
        &self,
        request: &EndSessionRequest,
        current_session_id: Option<&str>,
    ) -> Result<EndSessionOutcome, ProtocolError> {
        let instance_id = self.effective_instance_id();
        let mut client_id = non_empty(request.client_id.clone());
        let mut session_id = current_session_id.map(ToString::to_string);

        if !request.id_token_hint.is_empty() {
            let claims = self.validate_id_token_hint(&request.id_token_hint).await?;
            client_id = Some(claims.aud);
            session_id = claims.sid;
        }

        let redirect_uri = if request.post_logout_redirect_uri.is_empty() {
            "/login?logged_out=1".to_string()
        } else {
            let client_id = client_id
                .clone()
                .ok_or_else(|| ProtocolError::invalid_request("client_id required"))?;
            let client = self
                .clients
                .find_client(instance_id.as_ref(), &client_id)
                .await
                .map_err(|error| ProtocolError::server_error(format!("load client: {error}")))?;
            let Some(client) = client else {
                return Err(ProtocolError::invalid_client("unknown client_id"));
            };
            if !client
                .post_logout_redirect_uris
                .iter()
                .any(|uri| uri == &request.post_logout_redirect_uri)
            {
                return Err(ProtocolError::invalid_request(
                    "post_logout_redirect_uri is not registered",
                ));
            }
            append_query_param(
                &request.post_logout_redirect_uri,
                "state",
                request.state.as_str(),
            )
        };

        Ok(EndSessionOutcome {
            session_id,
            redirect_uri,
        })
    }

    pub async fn revoke_session_tokens(&self, session_id: &str) -> Result<(), ProtocolError> {
        let instance_id = self.effective_instance_id();
        self.tokens
            .revoke_session_tokens(instance_id.as_ref(), session_id)
            .await
            .map_err(|error| ProtocolError::server_error(format!("revoke session tokens: {error}")))
    }

    pub async fn validate_access_token(
        &self,
        access_token: &str,
    ) -> Result<AccessTokenClaims, ProtocolError> {
        let instance_id = self.effective_instance_id();
        if access_token.is_empty() {
            return Err(ProtocolError::invalid_request("Bearer token required"));
        }

        let claims = self
            .decode_token::<AccessTokenClaims>(instance_id.as_ref(), access_token)
            .await
            .map_err(|_| ProtocolError::invalid_grant("invalid access token"))?;
        self.enforce_token_storage(
            instance_id.as_ref(),
            access_token,
            "oidc_access",
            &claims.jti,
        )
        .await?;
        Ok(claims)
    }

    async fn validate_refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<RefreshTokenClaims, ProtocolError> {
        let instance_id = self.effective_instance_id();
        if refresh_token.is_empty() {
            return Err(ProtocolError::invalid_request("refresh_token required"));
        }

        let claims = self
            .decode_token::<RefreshTokenClaims>(instance_id.as_ref(), refresh_token)
            .await
            .map_err(|_| ProtocolError::invalid_grant("invalid refresh token"))?;
        self.enforce_token_storage(
            instance_id.as_ref(),
            refresh_token,
            "oidc_refresh",
            &claims.jti,
        )
        .await?;
        Ok(claims)
    }

    pub async fn validate_id_token_hint(
        &self,
        id_token_hint: &str,
    ) -> Result<IdTokenClaims, ProtocolError> {
        let instance_id = self.effective_instance_id();
        if id_token_hint.is_empty() {
            return Err(ProtocolError::invalid_request("id_token_hint required"));
        }

        self.decode_token::<IdTokenClaims>(instance_id.as_ref(), id_token_hint)
            .await
            .map_err(|_| ProtocolError::invalid_request("invalid id_token_hint"))
    }

    pub async fn userinfo(&self, access_token: &str) -> Result<UserInfoResponse, ProtocolError> {
        let instance_id = self.effective_instance_id();
        let token = self.validate_access_token(access_token).await?;

        let claims = self
            .claims
            .load_user_claims(instance_id.as_ref(), &token.sub)
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

    async fn decode_token<Claims>(&self, instance_id: &str, token: &str) -> anyhow::Result<Claims>
    where
        Claims: serde::de::DeserializeOwned,
    {
        let key = self.find_signing_key(instance_id, token).await?;
        let issuer = self.effective_issuer();
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_aud = false;
        validation.set_issuer(&[issuer.as_ref()]);
        Ok(decode::<Claims>(token, &key.decoding, &validation)?.claims)
    }

    async fn find_signing_key(
        &self,
        instance_id: &str,
        token: &str,
    ) -> anyhow::Result<Arc<SigningKeys>> {
        let header = decode_header(token)?;
        let keys = self.keys.signing_keys(instance_id).await?;
        if let Some(kid) = header.kid.as_deref() {
            if let Some(key) = keys.into_iter().find(|candidate| candidate.kid == kid) {
                return Ok(key);
            }
            anyhow::bail!("unknown signing key");
        }
        self.keys.active_signing_key(instance_id).await
    }

    async fn enforce_token_storage(
        &self,
        instance_id: &str,
        raw_token: &str,
        expected_type: &str,
        expected_token_id: &str,
    ) -> Result<(), ProtocolError> {
        if !self.tokens.enforces_storage() {
            return Ok(());
        }

        let stored = self
            .tokens
            .lookup_active_token(instance_id, raw_token)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load token record: {error}")))?;
        let Some(stored) = stored else {
            return Err(ProtocolError::invalid_grant("invalid token"));
        };
        if stored.token_type != expected_type || stored.token_id != expected_token_id {
            return Err(ProtocolError::invalid_grant("invalid token"));
        }
        Ok(())
    }

    async fn issue_id_token(
        &self,
        key: &SigningKeys,
        user: &UserClaims,
        client_id: &str,
        nonce: &str,
        auth_time: Option<u64>,
        sid: Option<&str>,
    ) -> Result<(String, IdTokenClaims), ProtocolError> {
        let now = now_epoch_seconds();
        let issuer = self.effective_issuer().into_owned();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(key.kid.clone());

        let claims = IdTokenClaims {
            iss: issuer,
            sub: user.subject.clone(),
            aud: client_id.to_string(),
            exp: now + self.lifetimes.id_token_secs,
            iat: now,
            jti: Uuid::new_v4().to_string(),
            auth_time,
            sid: sid.map(str::to_string),
            nonce: nonce.to_string(),
            name: user.name.clone(),
            email: user.email.clone(),
        };

        let token = jsonwebtoken::encode(&header, &claims, &key.encoding)
            .map_err(|error| ProtocolError::server_error(format!("id_token: {error}")))?;
        Ok((token, claims))
    }

    async fn issue_access_token(
        &self,
        key: &SigningKeys,
        subject: &str,
        client_id: &str,
        scope: &str,
        sid: Option<&str>,
    ) -> Result<(String, AccessTokenClaims), ProtocolError> {
        let now = now_epoch_seconds();
        let issuer = self.effective_issuer().into_owned();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(key.kid.clone());

        let claims = AccessTokenClaims {
            iss: issuer,
            sub: subject.to_string(),
            aud: client_id.to_string(),
            exp: now + self.lifetimes.access_token_secs,
            iat: now,
            jti: Uuid::new_v4().to_string(),
            sid: sid.map(str::to_string),
            scope: scope.to_string(),
            client_id: client_id.to_string(),
        };
        let token = jsonwebtoken::encode(&header, &claims, &key.encoding)
            .map_err(|error| ProtocolError::server_error(format!("access_token: {error}")))?;
        Ok((token, claims))
    }

    async fn issue_refresh_token(
        &self,
        key: &SigningKeys,
        subject: &str,
        client_id: &str,
        scope: &str,
        sid: Option<&str>,
    ) -> Result<(String, RefreshTokenClaims), ProtocolError> {
        let now = now_epoch_seconds();
        let issuer = self.effective_issuer().into_owned();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(key.kid.clone());

        let claims = RefreshTokenClaims {
            iss: issuer,
            sub: subject.to_string(),
            aud: client_id.to_string(),
            exp: now + self.lifetimes.refresh_token_secs,
            iat: now,
            jti: Uuid::new_v4().to_string(),
            sid: sid.map(str::to_string),
            scope: scope.to_string(),
            client_id: client_id.to_string(),
        };
        let token = jsonwebtoken::encode(&header, &claims, &key.encoding)
            .map_err(|error| ProtocolError::server_error(format!("refresh_token: {error}")))?;
        Ok((token, claims))
    }

    async fn exchange_authorization_code(
        &self,
        request: &TokenExchangeRequest,
    ) -> Result<TokenResponse, ProtocolError> {
        let instance_id = self.effective_instance_id();
        let auth = request
            .client_auth
            .as_ref()
            .ok_or_else(|| ProtocolError::invalid_client("client_id required"))?;

        let client = self
            .clients
            .find_client(instance_id.as_ref(), &auth.client_id)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load client: {error}")))?;
        let client = client.ok_or_else(|| ProtocolError::invalid_client("unknown client_id"))?;

        let authenticated = if client.client_secret.is_empty() {
            auth.client_secret.is_empty()
        } else {
            self.clients
                .authenticate_client_secret(
                    instance_id.as_ref(),
                    &auth.client_id,
                    &auth.client_secret,
                )
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
            .consume_auth_code(instance_id.as_ref(), &request.code)
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
            .load_user_claims(instance_id.as_ref(), &granted.user_id)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load user claims: {error}")))?;

        let user = user.ok_or_else(|| ProtocolError::invalid_grant("subject not found"))?;
        let key = self
            .keys
            .active_signing_key(instance_id.as_ref())
            .await
            .map_err(|error| {
                ProtocolError::temporarily_unavailable(format!("signing keys: {error}"))
            })?;

        let session_id = (!granted.session_id.is_empty()).then_some(granted.session_id.clone());
        let (id_token, _) = self
            .issue_id_token(
                &key,
                &user,
                &auth.client_id,
                &granted.nonce,
                granted.auth_time,
                session_id.as_deref(),
            )
            .await?;
        let refresh_bundle = if client
            .grant_types
            .iter()
            .any(|grant| grant == "refresh_token")
            && scope_contains(&granted.scope, "offline_access")
        {
            Some(
                self.issue_refresh_token(
                    &key,
                    &user.subject,
                    &auth.client_id,
                    &granted.scope,
                    session_id.as_deref(),
                )
                .await?,
            )
        } else {
            None
        };
        let refresh_family_id = refresh_bundle
            .as_ref()
            .map(|(_, claims)| claims.jti.clone());
        let (access_token, access_claims) = self
            .issue_access_token(
                &key,
                &user.subject,
                &auth.client_id,
                &granted.scope,
                session_id.as_deref(),
            )
            .await?;
        self.tokens
            .store_token(
                instance_id.as_ref(),
                &NewStoredToken {
                    token_id: access_claims.jti.clone(),
                    token_type: "oidc_access".to_string(),
                    raw_token: access_token.clone(),
                    user_id: Some(user.subject.clone()),
                    session_id: session_id.clone(),
                    client_id: auth.client_id.clone(),
                    application_id: client.app_id.clone(),
                    scope: granted.scope.clone(),
                    refresh_family_id: refresh_family_id.clone(),
                    auth_method: "authorization_code".to_string(),
                    expires_in_secs: self.lifetimes.access_token_secs,
                },
            )
            .await
            .map_err(|error| ProtocolError::server_error(format!("store access token: {error}")))?;
        if let Some((refresh_token, refresh_claims)) = refresh_bundle.as_ref() {
            self.tokens
                .store_token(
                    instance_id.as_ref(),
                    &NewStoredToken {
                        token_id: refresh_claims.jti.clone(),
                        token_type: "oidc_refresh".to_string(),
                        raw_token: refresh_token.clone(),
                        user_id: Some(user.subject.clone()),
                        session_id: session_id.clone(),
                        client_id: auth.client_id.clone(),
                        application_id: client.app_id.clone(),
                        scope: granted.scope.clone(),
                        refresh_family_id: Some(refresh_claims.jti.clone()),
                        auth_method: "authorization_code".to_string(),
                        expires_in_secs: self.lifetimes.refresh_token_secs,
                    },
                )
                .await
                .map_err(|error| {
                    ProtocolError::server_error(format!("store refresh token: {error}"))
                })?;
        }

        Ok(TokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.lifetimes.access_token_secs,
            id_token: Some(id_token),
            refresh_token: refresh_bundle.map(|(token, _)| token),
            scope: granted.scope,
        })
    }

    async fn exchange_client_credentials(
        &self,
        request: &TokenExchangeRequest,
    ) -> Result<TokenResponse, ProtocolError> {
        let instance_id = self.effective_instance_id();
        let auth = request
            .client_auth
            .as_ref()
            .ok_or_else(|| ProtocolError::invalid_client("client authentication required"))?;

        let client = self
            .clients
            .find_client(instance_id.as_ref(), &auth.client_id)
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
            .authenticate_client_secret(instance_id.as_ref(), &auth.client_id, &auth.client_secret)
            .await
            .map_err(|error| {
                ProtocolError::server_error(format!("authenticate client: {error}"))
            })?;
        if !authenticated {
            return Err(ProtocolError::invalid_client("invalid client credentials"));
        }

        let key = self
            .keys
            .active_signing_key(instance_id.as_ref())
            .await
            .map_err(|error| {
                ProtocolError::temporarily_unavailable(format!("signing keys: {error}"))
            })?;

        let (access_token, access_claims) = self
            .issue_access_token(&key, &auth.client_id, &auth.client_id, "openid", None)
            .await?;
        self.tokens
            .store_token(
                instance_id.as_ref(),
                &NewStoredToken {
                    token_id: access_claims.jti.clone(),
                    token_type: "oidc_access".to_string(),
                    raw_token: access_token.clone(),
                    user_id: None,
                    session_id: None,
                    client_id: auth.client_id.clone(),
                    application_id: client.app_id.clone(),
                    scope: "openid".to_string(),
                    refresh_family_id: None,
                    auth_method: "client_credentials".to_string(),
                    expires_in_secs: self.lifetimes.access_token_secs,
                },
            )
            .await
            .map_err(|error| ProtocolError::server_error(format!("store access token: {error}")))?;

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
        let instance_id = self.effective_instance_id();
        let auth = request
            .client_auth
            .as_ref()
            .ok_or_else(|| ProtocolError::invalid_client("client authentication required"))?;

        let client = self
            .clients
            .find_client(instance_id.as_ref(), &auth.client_id)
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
                .authenticate_client_secret(
                    instance_id.as_ref(),
                    &auth.client_id,
                    &auth.client_secret,
                )
                .await
                .map_err(|error| {
                    ProtocolError::server_error(format!("authenticate client: {error}"))
                })?
        };
        if !authenticated {
            return Err(ProtocolError::invalid_client("invalid client credentials"));
        }

        let refresh = self.validate_refresh_token(&request.refresh_token).await?;
        let stored_refresh = self
            .tokens
            .lookup_active_token(instance_id.as_ref(), &request.refresh_token)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load refresh token: {error}")))?
            .ok_or_else(|| ProtocolError::invalid_grant("refresh token not found"))?;
        if refresh.client_id != auth.client_id {
            return Err(ProtocolError::invalid_grant(
                "refresh token client mismatch",
            ));
        }

        let user = self
            .claims
            .load_user_claims(instance_id.as_ref(), &refresh.sub)
            .await
            .map_err(|error| ProtocolError::server_error(format!("load user claims: {error}")))?;
        let user = user.ok_or_else(|| ProtocolError::invalid_grant("subject not found"))?;

        let key = self
            .keys
            .active_signing_key(instance_id.as_ref())
            .await
            .map_err(|error| {
                ProtocolError::temporarily_unavailable(format!("signing keys: {error}"))
            })?;

        let refresh_family_id = stored_refresh
            .refresh_family_id
            .clone()
            .or_else(|| Some(stored_refresh.token_id.clone()));
        let session_id = stored_refresh.session_id.clone();
        let refresh_bundle = if scope_contains(&refresh.scope, "offline_access") {
            Some(
                self.issue_refresh_token(
                    &key,
                    &user.subject,
                    &auth.client_id,
                    &refresh.scope,
                    session_id.as_deref(),
                )
                .await?,
            )
        } else {
            None
        };
        let (access_token, access_claims) = self
            .issue_access_token(
                &key,
                &user.subject,
                &auth.client_id,
                &refresh.scope,
                session_id.as_deref(),
            )
            .await?;
        self.tokens
            .store_token(
                instance_id.as_ref(),
                &NewStoredToken {
                    token_id: access_claims.jti.clone(),
                    token_type: "oidc_access".to_string(),
                    raw_token: access_token.clone(),
                    user_id: Some(user.subject.clone()),
                    session_id: session_id.clone(),
                    client_id: auth.client_id.clone(),
                    application_id: client.app_id.clone(),
                    scope: refresh.scope.clone(),
                    refresh_family_id: refresh_family_id.clone(),
                    auth_method: "refresh_token".to_string(),
                    expires_in_secs: self.lifetimes.access_token_secs,
                },
            )
            .await
            .map_err(|error| ProtocolError::server_error(format!("store access token: {error}")))?;
        if let Some((refresh_token, refresh_claims)) = refresh_bundle.as_ref() {
            self.tokens
                .store_token(
                    instance_id.as_ref(),
                    &NewStoredToken {
                        token_id: refresh_claims.jti.clone(),
                        token_type: "oidc_refresh".to_string(),
                        raw_token: refresh_token.clone(),
                        user_id: Some(user.subject.clone()),
                        session_id: session_id.clone(),
                        client_id: auth.client_id.clone(),
                        application_id: client.app_id.clone(),
                        scope: refresh.scope.clone(),
                        refresh_family_id: refresh_family_id.clone(),
                        auth_method: "refresh_token".to_string(),
                        expires_in_secs: self.lifetimes.refresh_token_secs,
                    },
                )
                .await
                .map_err(|error| {
                    ProtocolError::server_error(format!("store refresh token: {error}"))
                })?;
        }
        self.tokens
            .revoke_token_by_id(instance_id.as_ref(), &stored_refresh.token_id)
            .await
            .map_err(|error| {
                ProtocolError::server_error(format!("revoke refresh token: {error}"))
            })?;

        Ok(TokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.lifetimes.access_token_secs,
            id_token: None,
            refresh_token: refresh_bundle.map(|(token, _)| token),
            scope: refresh.scope,
        })
    }
}

fn scope_contains(scope: &str, needle: &str) -> bool {
    scope.split_whitespace().any(|part| part == needle)
}

fn append_query_param(base: &str, key: &str, value: &str) -> String {
    if value.is_empty() {
        return base.to_string();
    }
    if let Ok(mut url) = Url::parse(base) {
        url.query_pairs_mut().append_pair(key, value);
        return url.to_string();
    }
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}{key}={value}")
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
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
    use crate::stores::NoopTokenStore;
    use base64::Engine;
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex};
    use zitadel_db::{InstanceContext, with_instance_context};

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

        async fn signing_keys(&self, _instance_id: &str) -> anyhow::Result<Vec<Arc<SigningKeys>>> {
            Ok(vec![self.key.clone()])
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

    #[derive(Clone, Default)]
    struct RecordingTokenStore {
        stored_by_raw: Arc<Mutex<HashMap<String, StoredToken>>>,
        stored_specs: Arc<Mutex<Vec<NewStoredToken>>>,
        revoked_ids: Arc<Mutex<Vec<String>>>,
        revoked_families: Arc<Mutex<Vec<String>>>,
        revoked_sessions: Arc<Mutex<Vec<String>>>,
    }

    impl TokenStore for RecordingTokenStore {
        async fn store_token(
            &self,
            _instance_id: &str,
            token: &NewStoredToken,
        ) -> anyhow::Result<()> {
            self.stored_specs.lock().unwrap().push(token.clone());
            self.stored_by_raw.lock().unwrap().insert(
                token.raw_token.clone(),
                StoredToken {
                    token_id: token.token_id.clone(),
                    token_type: token.token_type.clone(),
                    user_id: token.user_id.clone(),
                    session_id: token.session_id.clone(),
                    client_id: token.client_id.clone(),
                    application_id: token.application_id.clone(),
                    scope: token.scope.clone(),
                    refresh_family_id: token.refresh_family_id.clone(),
                },
            );
            Ok(())
        }

        async fn lookup_active_token(
            &self,
            _instance_id: &str,
            raw_token: &str,
        ) -> anyhow::Result<Option<StoredToken>> {
            Ok(self.stored_by_raw.lock().unwrap().get(raw_token).cloned())
        }

        async fn revoke_token_by_id(
            &self,
            _instance_id: &str,
            token_id: &str,
        ) -> anyhow::Result<()> {
            self.revoked_ids.lock().unwrap().push(token_id.to_string());
            Ok(())
        }

        async fn revoke_refresh_family(
            &self,
            _instance_id: &str,
            refresh_family_id: &str,
        ) -> anyhow::Result<()> {
            self.revoked_families
                .lock()
                .unwrap()
                .push(refresh_family_id.to_string());
            Ok(())
        }

        async fn revoke_session_tokens(
            &self,
            _instance_id: &str,
            session_id: &str,
        ) -> anyhow::Result<()> {
            self.revoked_sessions
                .lock()
                .unwrap()
                .push(session_id.to_string());
            Ok(())
        }
    }

    fn test_provider(
        consumed: Option<ConsumedAuthRequest>,
    ) -> Provider<
        FakeClientStore,
        FakeAuthRequestStore,
        StaticKeyStore,
        FakeClaimSource,
        NoopTokenStore,
    > {
        Provider::new(
            "default".to_string(),
            "http://issuer.example".to_string(),
            "/login".to_string(),
            FakeClientStore {
                client: Some(ClientMetadata {
                    app_id: "app-1".to_string(),
                    client_id: "client".to_string(),
                    client_secret: "secret".to_string(),
                    redirect_uris: vec!["https://app.example/callback".to_string()],
                    post_logout_redirect_uris: vec!["https://app.example/logout".to_string()],
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
            NoopTokenStore,
        )
    }

    fn test_provider_with_tokens(
        consumed: Option<ConsumedAuthRequest>,
        tokens: RecordingTokenStore,
    ) -> Provider<
        FakeClientStore,
        FakeAuthRequestStore,
        StaticKeyStore,
        FakeClaimSource,
        RecordingTokenStore,
    > {
        Provider::new(
            "default".to_string(),
            "http://issuer.example".to_string(),
            "/login".to_string(),
            FakeClientStore {
                client: Some(ClientMetadata {
                    app_id: "app-1".to_string(),
                    client_id: "client".to_string(),
                    client_secret: "secret".to_string(),
                    redirect_uris: vec!["https://app.example/callback".to_string()],
                    post_logout_redirect_uris: vec!["https://app.example/logout".to_string()],
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
            tokens,
        )
    }

    fn request_context(host: &str) -> InstanceContext {
        InstanceContext {
            instance_id: "default".to_string(),
            resolved_org_id: None,
            placement_mode: "global".to_string(),
            region_key: None,
            scheme: "https".to_string(),
            host: host.to_string(),
            source: "host".to_string(),
        }
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
            session_id: "session-1".to_string(),
            client_id: "client".to_string(),
            redirect_uri: "https://app.example/callback".to_string(),
            scope: "openid profile".to_string(),
            state: "state".to_string(),
            nonce: "nonce".to_string(),
            response_type: "code".to_string(),
            code_challenge: s256_challenge("expected"),
            code_challenge_method: "S256".to_string(),
            prompt: Vec::new(),
            login_hint: String::new(),
            max_age: None,
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
            session_id: "session-1".to_string(),
            client_id: "client".to_string(),
            redirect_uri: "https://app.example/callback".to_string(),
            scope: "openid".to_string(),
            state: "state".to_string(),
            nonce: "nonce".to_string(),
            response_type: "code".to_string(),
            code_challenge: String::new(),
            code_challenge_method: String::new(),
            prompt: Vec::new(),
            login_hint: String::new(),
            max_age: None,
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

    #[tokio::test]
    async fn discovery_document_uses_request_origin_when_unpinned() {
        let provider = test_provider(None);

        let discovery = with_instance_context(request_context("demo.example.com"), async {
            provider.discovery_document()
        })
        .await;

        assert_eq!(discovery.issuer, "https://demo.example.com");
        assert_eq!(
            discovery.authorization_endpoint,
            "https://demo.example.com/authorize"
        );
    }

    #[tokio::test]
    async fn token_claims_use_request_origin_when_unpinned() {
        let provider = test_provider(Some(ConsumedAuthRequest {
            auth_request_id: "auth-1".to_string(),
            user_id: "user-1".to_string(),
            session_id: "session-1".to_string(),
            client_id: "client".to_string(),
            redirect_uri: "https://app.example/callback".to_string(),
            scope: "openid".to_string(),
            state: "state".to_string(),
            nonce: "nonce".to_string(),
            response_type: "code".to_string(),
            code_challenge: String::new(),
            code_challenge_method: String::new(),
            prompt: Vec::new(),
            login_hint: String::new(),
            max_age: None,
            auth_time: Some(1_700_000_123),
        }));

        let response = with_instance_context(request_context("demo.example.com"), async {
            provider
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
                .unwrap()
        })
        .await;

        let id_token = response.id_token.expect("id token");
        let key = provider
            .keys
            .active_signing_key(&provider.instance_id)
            .await
            .unwrap();
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_aud = false;
        validation.set_issuer(&["https://demo.example.com"]);
        let claims = jsonwebtoken::decode::<IdTokenClaims>(&id_token, &key.decoding, &validation)
            .unwrap()
            .claims;

        assert_eq!(claims.iss, "https://demo.example.com");
    }

    #[tokio::test]
    async fn explicit_issuer_override_beats_request_origin() {
        let provider =
            test_provider(None).with_issuer_override(Some("https://login.example.com".into()));

        let discovery = with_instance_context(request_context("demo.example.com"), async {
            provider.discovery_document()
        })
        .await;

        assert_eq!(discovery.issuer, "https://login.example.com");
    }

    #[tokio::test]
    async fn token_exchange_persists_session_linked_tokens() {
        let tokens = RecordingTokenStore::default();
        let provider = test_provider_with_tokens(
            Some(ConsumedAuthRequest {
                auth_request_id: "auth-1".to_string(),
                user_id: "user-1".to_string(),
                session_id: "session-1".to_string(),
                client_id: "client".to_string(),
                redirect_uri: "https://app.example/callback".to_string(),
                scope: "openid offline_access".to_string(),
                state: "state".to_string(),
                nonce: "nonce".to_string(),
                response_type: "code".to_string(),
                code_challenge: String::new(),
                code_challenge_method: String::new(),
                prompt: Vec::new(),
                login_hint: String::new(),
                max_age: None,
                auth_time: Some(1_700_000_123),
            }),
            tokens.clone(),
        );

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

        assert!(!response.access_token.is_empty());
        assert!(response.refresh_token.is_some());

        let stored = tokens.stored_specs.lock().unwrap().clone();
        assert_eq!(stored.len(), 2);
        let access = stored
            .iter()
            .find(|token| token.token_type == "oidc_access")
            .unwrap();
        let refresh = stored
            .iter()
            .find(|token| token.token_type == "oidc_refresh")
            .unwrap();
        assert_eq!(access.session_id.as_deref(), Some("session-1"));
        assert_eq!(refresh.session_id.as_deref(), Some("session-1"));
        assert_eq!(
            access.refresh_family_id.as_deref(),
            Some(refresh.token_id.as_str())
        );
    }

    #[tokio::test]
    async fn revoke_refresh_token_revokes_entire_family() {
        let tokens = RecordingTokenStore::default();
        let provider = test_provider_with_tokens(
            Some(ConsumedAuthRequest {
                auth_request_id: "auth-1".to_string(),
                user_id: "user-1".to_string(),
                session_id: "session-1".to_string(),
                client_id: "client".to_string(),
                redirect_uri: "https://app.example/callback".to_string(),
                scope: "openid offline_access".to_string(),
                state: "state".to_string(),
                nonce: "nonce".to_string(),
                response_type: "code".to_string(),
                code_challenge: String::new(),
                code_challenge_method: String::new(),
                prompt: Vec::new(),
                login_hint: String::new(),
                max_age: None,
                auth_time: Some(1_700_000_123),
            }),
            tokens.clone(),
        );

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

        provider
            .revoke(
                response.refresh_token.as_deref().unwrap(),
                Some(&ClientAuthentication {
                    client_id: "client".to_string(),
                    client_secret: "secret".to_string(),
                    method: ClientAuthMethod::ClientSecretPost,
                }),
            )
            .await
            .unwrap();

        let stored = tokens.stored_specs.lock().unwrap().clone();
        let refresh = stored
            .iter()
            .find(|token| token.token_type == "oidc_refresh")
            .unwrap();
        assert_eq!(
            tokens.revoked_families.lock().unwrap().as_slice(),
            &[refresh.token_id.clone()]
        );
    }

    #[tokio::test]
    async fn end_session_uses_id_token_hint_for_redirect_validation() {
        let tokens = RecordingTokenStore::default();
        let provider = test_provider_with_tokens(
            Some(ConsumedAuthRequest {
                auth_request_id: "auth-1".to_string(),
                user_id: "user-1".to_string(),
                session_id: "session-1".to_string(),
                client_id: "client".to_string(),
                redirect_uri: "https://app.example/callback".to_string(),
                scope: "openid".to_string(),
                state: "state".to_string(),
                nonce: "nonce".to_string(),
                response_type: "code".to_string(),
                code_challenge: String::new(),
                code_challenge_method: String::new(),
                prompt: Vec::new(),
                login_hint: String::new(),
                max_age: None,
                auth_time: Some(1_700_000_123),
            }),
            tokens,
        );

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

        let outcome = provider
            .end_session(
                &EndSessionRequest {
                    client_id: String::new(),
                    id_token_hint: response.id_token.unwrap(),
                    post_logout_redirect_uri: "https://app.example/logout".to_string(),
                    state: "logout-state".to_string(),
                },
                None,
            )
            .await
            .unwrap();

        assert_eq!(outcome.session_id.as_deref(), Some("session-1"));
        assert_eq!(
            outcome.redirect_uri,
            "https://app.example/logout?state=logout-state"
        );
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
