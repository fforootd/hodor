#![allow(async_fn_in_trait)]

use crate::oidc::{JsonWebKeySet, OpenIdConfiguration, s256_challenge};
use anyhow::Context;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpProviderSpec {
    pub provider_id: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub token_endpoint_auth_method: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpAuthState {
    pub provider_id: String,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
    pub flow_id: String,
    pub redirect_uri: String,
    pub expected_issuer: String,
    pub callback_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpStartRequest {
    pub provider: RpProviderSpec,
    pub flow_id: String,
    pub redirect_uri: String,
    pub callback_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpStartResult {
    pub authorization_url: String,
    pub state: RpAuthState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpCallbackRequest {
    pub provider: RpProviderSpec,
    pub stored_state: RpAuthState,
    pub returned_state: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerifiedExternalIdentity {
    pub issuer: String,
    pub subject: String,
    pub email: String,
    pub email_verified: bool,
    pub claims: Value,
    pub id_token: Option<String>,
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedIssuerMetadata {
    pub config: OpenIdConfiguration,
    pub jwks: JsonWebKeySet,
}

pub trait StateStore: Clone + Send + Sync + 'static {
    async fn put_state(&self, instance_id: &str, state: &RpAuthState) -> anyhow::Result<()>;
    async fn take_state(
        &self,
        instance_id: &str,
        state: &str,
    ) -> anyhow::Result<Option<RpAuthState>>;
}

pub trait HttpClient: Clone + Send + Sync + 'static {
    async fn get_json<T: DeserializeOwned + Send>(&self, url: &str) -> anyhow::Result<T>;
    async fn get_json_with_bearer<T: DeserializeOwned + Send>(
        &self,
        url: &str,
        access_token: &str,
    ) -> anyhow::Result<T>;
    async fn post_form<T: DeserializeOwned + Send>(
        &self,
        url: &str,
        params: &[(String, String)],
        basic_auth: Option<(&str, &str)>,
    ) -> anyhow::Result<T>;
}

pub trait IssuerMetadataCache: Clone + Send + Sync + 'static {
    async fn get(&self, issuer: &str) -> anyhow::Result<Option<CachedIssuerMetadata>>;
    async fn put(&self, issuer: &str, metadata: &CachedIssuerMetadata) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct ReqwestHttpClient {
    client: reqwest::Client,
}

impl ReqwestHttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for ReqwestHttpClient {
    async fn get_json<T: DeserializeOwned + Send>(&self, url: &str) -> anyhow::Result<T> {
        let response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            anyhow::bail!("GET {url} failed with {}", response.status());
        }
        Ok(response.json().await?)
    }

    async fn get_json_with_bearer<T: DeserializeOwned + Send>(
        &self,
        url: &str,
        access_token: &str,
    ) -> anyhow::Result<T> {
        let response = self
            .client
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await?;
        if !response.status().is_success() {
            anyhow::bail!("GET {url} failed with {}", response.status());
        }
        Ok(response.json().await?)
    }

    async fn post_form<T: DeserializeOwned + Send>(
        &self,
        url: &str,
        params: &[(String, String)],
        basic_auth: Option<(&str, &str)>,
    ) -> anyhow::Result<T> {
        let mut request = self.client.post(url).form(params);
        if let Some((username, password)) = basic_auth {
            request = request.basic_auth(username, Some(password));
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("POST {url} failed with {status}: {body}");
        }
        Ok(response.json().await?)
    }
}

#[derive(Clone, Default)]
pub struct InMemoryIssuerMetadataCache {
    entries: Arc<RwLock<HashMap<String, CachedIssuerMetadata>>>,
}

impl IssuerMetadataCache for InMemoryIssuerMetadataCache {
    async fn get(&self, issuer: &str) -> anyhow::Result<Option<CachedIssuerMetadata>> {
        Ok(self.entries.read().await.get(issuer).cloned())
    }

    async fn put(&self, issuer: &str, metadata: &CachedIssuerMetadata) -> anyhow::Result<()> {
        self.entries
            .write()
            .await
            .insert(issuer.to_string(), metadata.clone());
        Ok(())
    }
}

#[derive(Clone)]
pub struct RpService<H, C> {
    http: H,
    cache: C,
}

impl<H, C> RpService<H, C> {
    pub fn new(http: H, cache: C) -> Self {
        Self { http, cache }
    }
}

impl<H, C> RpService<H, C>
where
    H: HttpClient,
    C: IssuerMetadataCache,
{
    pub async fn start(&self, request: &RpStartRequest) -> anyhow::Result<RpStartResult> {
        let metadata = self.issuer_metadata(&request.provider.issuer).await?;

        let state = RpAuthState {
            provider_id: request.provider.provider_id.clone(),
            state: Uuid::new_v4().to_string(),
            nonce: Uuid::new_v4().to_string(),
            pkce_verifier: format!("{}{}", Uuid::new_v4(), Uuid::new_v4()),
            flow_id: request.flow_id.clone(),
            redirect_uri: request.redirect_uri.clone(),
            expected_issuer: request.provider.issuer.clone(),
            callback_uri: request.callback_uri.clone(),
        };

        let challenge = s256_challenge(&state.pkce_verifier);
        let scopes = if request.provider.scopes.is_empty() {
            vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ]
        } else {
            request.provider.scopes.clone()
        };

        let mut authorization_url = Url::parse(&metadata.config.authorization_endpoint)
            .with_context(|| "parse authorization endpoint")?;
        authorization_url
            .query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &request.provider.client_id)
            .append_pair("redirect_uri", &request.callback_uri)
            .append_pair("scope", &scopes.join(" "))
            .append_pair("state", &state.state)
            .append_pair("nonce", &state.nonce)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");

        Ok(RpStartResult {
            authorization_url: authorization_url.to_string(),
            state,
        })
    }

    pub async fn start_with_store<S: StateStore>(
        &self,
        instance_id: &str,
        request: &RpStartRequest,
        store: &S,
    ) -> anyhow::Result<RpStartResult> {
        let result = self.start(request).await?;
        store.put_state(instance_id, &result.state).await?;
        Ok(result)
    }

    pub async fn finish(
        &self,
        request: &RpCallbackRequest,
    ) -> anyhow::Result<VerifiedExternalIdentity> {
        if request.code.is_empty() {
            anyhow::bail!("missing authorization code");
        }
        if request.returned_state != request.stored_state.state {
            anyhow::bail!("invalid or expired state");
        }
        if request.provider.issuer != request.stored_state.expected_issuer {
            anyhow::bail!("issuer mismatch");
        }

        let metadata = self
            .issuer_metadata(&request.stored_state.expected_issuer)
            .await?;
        let token_response = self
            .exchange_code(
                &metadata,
                &request.provider,
                &request.stored_state,
                &request.code,
            )
            .await?;

        let mut claims = if let Some(id_token) = token_response.id_token.as_ref() {
            verify_id_token(
                id_token,
                &metadata.jwks,
                &request.provider.client_id,
                &request.stored_state.expected_issuer,
                &request.stored_state.nonce,
            )?
        } else if !metadata.config.userinfo_endpoint.is_empty() {
            self.http
                .get_json_with_bearer::<Value>(
                    &metadata.config.userinfo_endpoint,
                    &token_response.access_token,
                )
                .await?
        } else {
            anyhow::bail!("provider did not return id_token or userinfo endpoint");
        };

        if !metadata.config.userinfo_endpoint.is_empty() && should_enrich_from_userinfo(&claims) {
            let userinfo = self
                .http
                .get_json_with_bearer::<Value>(
                    &metadata.config.userinfo_endpoint,
                    &token_response.access_token,
                )
                .await?;
            claims = merge_claims(claims, userinfo);
        }

        let subject = claims
            .get("sub")
            .or_else(|| claims.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if subject.is_empty() {
            anyhow::bail!("provider response did not include sub");
        }

        Ok(VerifiedExternalIdentity {
            issuer: request.stored_state.expected_issuer.clone(),
            subject,
            email: claims
                .get("email")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            email_verified: claims
                .get("email_verified")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            claims,
            id_token: token_response.id_token,
            access_token: token_response.access_token,
        })
    }

    pub async fn finish_with_store<S: StateStore>(
        &self,
        instance_id: &str,
        provider: &RpProviderSpec,
        returned_state: &str,
        code: &str,
        store: &S,
    ) -> anyhow::Result<VerifiedExternalIdentity> {
        let stored_state = store
            .take_state(instance_id, returned_state)
            .await?
            .ok_or_else(|| anyhow::anyhow!("invalid or expired state"))?;

        self.finish(&RpCallbackRequest {
            provider: provider.clone(),
            stored_state,
            returned_state: returned_state.to_string(),
            code: code.to_string(),
        })
        .await
    }

    async fn issuer_metadata(&self, issuer: &str) -> anyhow::Result<CachedIssuerMetadata> {
        if let Some(metadata) = self.cache.get(issuer).await? {
            return Ok(metadata);
        }

        let metadata_url = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        let config: OpenIdConfiguration = self.http.get_json(&metadata_url).await?;
        let jwks: JsonWebKeySet = self.http.get_json(&config.jwks_uri).await?;
        let cached = CachedIssuerMetadata { config, jwks };
        self.cache.put(issuer, &cached).await?;
        Ok(cached)
    }

    async fn exchange_code(
        &self,
        metadata: &CachedIssuerMetadata,
        provider: &RpProviderSpec,
        state: &RpAuthState,
        code: &str,
    ) -> anyhow::Result<TokenEndpointResponse> {
        let mut params = vec![
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("code".to_string(), code.to_string()),
            ("client_id".to_string(), provider.client_id.clone()),
            ("redirect_uri".to_string(), state.callback_uri.clone()),
            ("code_verifier".to_string(), state.pkce_verifier.clone()),
        ];
        let mut basic_auth = None;

        match provider.token_endpoint_auth_method.as_str() {
            "client_secret_basic" => {
                basic_auth = Some((provider.client_id.as_str(), provider.client_secret.as_str()));
            }
            _ => {
                if !provider.client_secret.is_empty() {
                    params.push(("client_secret".to_string(), provider.client_secret.clone()));
                }
            }
        }

        self.http
            .post_form(&metadata.config.token_endpoint, &params, basic_auth)
            .await
    }
}

fn should_enrich_from_userinfo(claims: &Value) -> bool {
    claims
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
        || claims
            .get("email_verified")
            .and_then(Value::as_bool)
            .is_none()
}

fn merge_claims(primary: Value, fallback: Value) -> Value {
    match (primary, fallback) {
        (Value::Object(mut primary), Value::Object(fallback)) => {
            for (key, value) in fallback {
                primary.entry(key).or_insert(value);
            }
            Value::Object(primary)
        }
        (primary, _) => primary,
    }
}

#[derive(Debug, Deserialize)]
struct TokenEndpointResponse {
    access_token: String,
    #[serde(default)]
    id_token: Option<String>,
}

fn verify_id_token(
    token: &str,
    jwks: &JsonWebKeySet,
    audience: &str,
    issuer: &str,
    expected_nonce: &str,
) -> anyhow::Result<Value> {
    let header = decode_header(token)?;
    let key = resolve_decoding_key(jwks, header.kid.as_deref())?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[audience]);
    validation.set_issuer(&[issuer]);
    let decoded = decode::<Value>(token, &key, &validation)?;
    let claims = decoded.claims;

    let nonce = claims
        .get("nonce")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !expected_nonce.is_empty() && nonce != expected_nonce {
        anyhow::bail!("nonce mismatch");
    }

    Ok(claims)
}

fn resolve_decoding_key(jwks: &JsonWebKeySet, kid: Option<&str>) -> anyhow::Result<DecodingKey> {
    let jwk = if let Some(kid) = kid {
        jwks.keys
            .iter()
            .find(|candidate| candidate.kid == kid)
            .or_else(|| jwks.keys.first())
    } else {
        jwks.keys.first()
    }
    .ok_or_else(|| anyhow::anyhow!("no signing keys available"))?;

    Ok(DecodingKey::from_rsa_components(&jwk.n, &jwk.e)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oidc::{JsonWebKey, SigningKeys, now_epoch_seconds};
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

    #[derive(Clone, Default)]
    struct MemoryStateStore {
        states: Arc<RwLock<HashMap<String, RpAuthState>>>,
    }

    impl StateStore for MemoryStateStore {
        async fn put_state(&self, _instance_id: &str, state: &RpAuthState) -> anyhow::Result<()> {
            self.states
                .write()
                .await
                .insert(state.state.clone(), state.clone());
            Ok(())
        }

        async fn take_state(
            &self,
            _instance_id: &str,
            state: &str,
        ) -> anyhow::Result<Option<RpAuthState>> {
            Ok(self.states.write().await.remove(state))
        }
    }

    #[derive(Clone, Default)]
    struct MockHttpClient {
        metadata: Option<OpenIdConfiguration>,
        jwks: Option<JsonWebKeySet>,
        token: Option<Value>,
        userinfo: Option<Value>,
    }

    impl HttpClient for MockHttpClient {
        async fn get_json<T: DeserializeOwned + Send>(&self, url: &str) -> anyhow::Result<T> {
            if url.ends_with("/.well-known/openid-configuration") {
                return Ok(serde_json::from_value(
                    serde_json::to_value(self.metadata.clone().unwrap()).unwrap(),
                )?);
            }
            Ok(serde_json::from_value(
                serde_json::to_value(self.jwks.clone().unwrap()).unwrap(),
            )?)
        }

        async fn get_json_with_bearer<T: DeserializeOwned + Send>(
            &self,
            _url: &str,
            _access_token: &str,
        ) -> anyhow::Result<T> {
            Ok(serde_json::from_value(self.userinfo.clone().unwrap())?)
        }

        async fn post_form<T: DeserializeOwned + Send>(
            &self,
            _url: &str,
            _params: &[(String, String)],
            _basic_auth: Option<(&str, &str)>,
        ) -> anyhow::Result<T> {
            Ok(serde_json::from_value(self.token.clone().unwrap())?)
        }
    }

    #[tokio::test]
    async fn start_builds_authorization_url_and_persists_state() {
        let service = RpService::new(
            MockHttpClient {
                metadata: Some(test_metadata()),
                jwks: Some(JsonWebKeySet { keys: vec![] }),
                ..MockHttpClient::default()
            },
            InMemoryIssuerMetadataCache::default(),
        );
        let store = MemoryStateStore::default();
        let provider = test_provider();

        let result = service
            .start_with_store(
                "default",
                &RpStartRequest {
                    provider,
                    flow_id: "flow-1".into(),
                    redirect_uri: "/console".into(),
                    callback_uri: "http://localhost:8080/v1/auth/sso/callback".into(),
                },
                &store,
            )
            .await
            .unwrap();

        let url = Url::parse(&result.authorization_url).unwrap();
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "response_type")
                .unwrap()
                .1,
            "code"
        );
        assert!(
            store
                .take_state("default", &result.state.state)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn finish_verifies_id_token_and_returns_identity() {
        let signing = SigningKeys::generate().unwrap();
        let state = RpAuthState {
            provider_id: "provider-1".into(),
            state: "state-1".into(),
            nonce: "nonce-1".into(),
            pkce_verifier: "verifier-1".into(),
            flow_id: "flow-1".into(),
            redirect_uri: "/console".into(),
            expected_issuer: "https://issuer.example".into(),
            callback_uri: "http://localhost:8080/v1/auth/sso/callback".into(),
        };
        let id_token = signed_id_token(
            &signing.encoding,
            &signing.kid,
            &state.expected_issuer,
            "client-1",
            &state.nonce,
        );
        let service = RpService::new(
            MockHttpClient {
                metadata: Some(test_metadata()),
                jwks: Some(JsonWebKeySet {
                    keys: vec![JsonWebKey {
                        kty: "RSA".into(),
                        use_: "sig".into(),
                        kid: signing.kid.clone(),
                        alg: "RS256".into(),
                        n: signing.n.clone(),
                        e: signing.e.clone(),
                    }],
                }),
                token: Some(serde_json::json!({
                    "access_token": "access-1",
                    "id_token": id_token,
                })),
                ..MockHttpClient::default()
            },
            InMemoryIssuerMetadataCache::default(),
        );

        let identity = service
            .finish(&RpCallbackRequest {
                provider: test_provider(),
                stored_state: state,
                returned_state: "state-1".into(),
                code: "code-1".into(),
            })
            .await
            .unwrap();

        assert_eq!(identity.subject, "user-1");
        assert_eq!(identity.email, "alice@example.com");
        assert!(identity.email_verified);
    }

    #[tokio::test]
    async fn finish_falls_back_to_userinfo_when_no_id_token_exists() {
        let state = RpAuthState {
            provider_id: "provider-1".into(),
            state: "state-1".into(),
            nonce: "nonce-1".into(),
            pkce_verifier: "verifier-1".into(),
            flow_id: "flow-1".into(),
            redirect_uri: "/console".into(),
            expected_issuer: "https://issuer.example".into(),
            callback_uri: "http://localhost:8080/v1/auth/sso/callback".into(),
        };
        let service = RpService::new(
            MockHttpClient {
                metadata: Some(test_metadata()),
                jwks: Some(JsonWebKeySet { keys: vec![] }),
                token: Some(serde_json::json!({
                    "access_token": "access-1"
                })),
                userinfo: Some(serde_json::json!({
                    "sub": "userinfo-1",
                    "email": "userinfo@example.com",
                    "email_verified": true
                })),
            },
            InMemoryIssuerMetadataCache::default(),
        );

        let identity = service
            .finish(&RpCallbackRequest {
                provider: test_provider(),
                stored_state: state,
                returned_state: "state-1".into(),
                code: "code-1".into(),
            })
            .await
            .unwrap();

        assert_eq!(identity.subject, "userinfo-1");
        assert_eq!(identity.email, "userinfo@example.com");
    }

    #[tokio::test]
    async fn finish_enriches_sparse_id_token_with_userinfo_claims() {
        let signing = SigningKeys::generate().unwrap();
        let state = RpAuthState {
            provider_id: "provider-1".into(),
            state: "state-1".into(),
            nonce: "nonce-1".into(),
            pkce_verifier: "verifier-1".into(),
            flow_id: "flow-1".into(),
            redirect_uri: "/console".into(),
            expected_issuer: "https://issuer.example".into(),
            callback_uri: "http://localhost:8080/v1/auth/sso/callback".into(),
        };
        let now = now_epoch_seconds();
        let header = Header {
            alg: Algorithm::RS256,
            kid: Some(signing.kid.clone()),
            ..Header::default()
        };
        let sparse_id_token = encode(
            &header,
            &serde_json::json!({
                "iss": state.expected_issuer,
                "sub": "user-1",
                "aud": "client-1",
                "exp": now + 300,
                "iat": now,
                "nonce": state.nonce,
            }),
            &signing.encoding,
        )
        .unwrap();

        let service = RpService::new(
            MockHttpClient {
                metadata: Some(test_metadata()),
                jwks: Some(JsonWebKeySet {
                    keys: vec![JsonWebKey {
                        kty: "RSA".into(),
                        use_: "sig".into(),
                        kid: signing.kid.clone(),
                        alg: "RS256".into(),
                        n: signing.n.clone(),
                        e: signing.e.clone(),
                    }],
                }),
                token: Some(serde_json::json!({
                    "access_token": "access-1",
                    "id_token": sparse_id_token,
                })),
                userinfo: Some(serde_json::json!({
                    "sub": "user-1",
                    "email": "userinfo@example.com",
                    "email_verified": true,
                    "name": "User Info"
                })),
                ..MockHttpClient::default()
            },
            InMemoryIssuerMetadataCache::default(),
        );

        let identity = service
            .finish(&RpCallbackRequest {
                provider: test_provider(),
                stored_state: state,
                returned_state: "state-1".into(),
                code: "code-1".into(),
            })
            .await
            .unwrap();

        assert_eq!(identity.subject, "user-1");
        assert_eq!(identity.email, "userinfo@example.com");
        assert!(identity.email_verified);
        assert_eq!(
            identity.claims.get("name").and_then(Value::as_str),
            Some("User Info")
        );
    }

    #[tokio::test]
    async fn finish_rejects_nonce_mismatch() {
        let signing = SigningKeys::generate().unwrap();
        let state = RpAuthState {
            provider_id: "provider-1".into(),
            state: "state-1".into(),
            nonce: "expected-nonce".into(),
            pkce_verifier: "verifier-1".into(),
            flow_id: "flow-1".into(),
            redirect_uri: "/console".into(),
            expected_issuer: "https://issuer.example".into(),
            callback_uri: "http://localhost:8080/v1/auth/sso/callback".into(),
        };
        let service = RpService::new(
            MockHttpClient {
                metadata: Some(test_metadata()),
                jwks: Some(JsonWebKeySet {
                    keys: vec![JsonWebKey {
                        kty: "RSA".into(),
                        use_: "sig".into(),
                        kid: signing.kid.clone(),
                        alg: "RS256".into(),
                        n: signing.n.clone(),
                        e: signing.e.clone(),
                    }],
                }),
                token: Some(serde_json::json!({
                    "access_token": "access-1",
                    "id_token": signed_id_token(
                        &signing.encoding,
                        &signing.kid,
                        &state.expected_issuer,
                        "client-1",
                        "wrong-nonce",
                    ),
                })),
                ..MockHttpClient::default()
            },
            InMemoryIssuerMetadataCache::default(),
        );

        let error = service
            .finish(&RpCallbackRequest {
                provider: test_provider(),
                stored_state: state,
                returned_state: "state-1".into(),
                code: "code-1".into(),
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("nonce mismatch"));
    }

    fn test_provider() -> RpProviderSpec {
        RpProviderSpec {
            provider_id: "provider-1".into(),
            issuer: "https://issuer.example".into(),
            client_id: "client-1".into(),
            client_secret: "secret-1".into(),
            scopes: vec!["openid".into(), "profile".into(), "email".into()],
            token_endpoint_auth_method: "client_secret_post".into(),
        }
    }

    fn test_metadata() -> OpenIdConfiguration {
        OpenIdConfiguration {
            issuer: "https://issuer.example".into(),
            authorization_endpoint: "https://issuer.example/authorize".into(),
            token_endpoint: "https://issuer.example/oauth/token".into(),
            userinfo_endpoint: "https://issuer.example/userinfo".into(),
            jwks_uri: "https://issuer.example/keys".into(),
            revocation_endpoint: "https://issuer.example/revoke".into(),
            end_session_endpoint: "https://issuer.example/end_session".into(),
            response_types_supported: vec!["code".into()],
            grant_types_supported: vec!["authorization_code".into()],
            subject_types_supported: vec!["public".into()],
            id_token_signing_alg_values_supported: vec!["RS256".into()],
            scopes_supported: vec!["openid".into(), "profile".into(), "email".into()],
            token_endpoint_auth_methods_supported: vec![
                "client_secret_post".into(),
                "client_secret_basic".into(),
            ],
            code_challenge_methods_supported: vec!["S256".into()],
            claims_supported: vec!["sub".into(), "email".into()],
        }
    }

    fn signed_id_token(
        key: &EncodingKey,
        kid: &str,
        issuer: &str,
        audience: &str,
        nonce: &str,
    ) -> String {
        let header = Header {
            alg: Algorithm::RS256,
            kid: Some(kid.into()),
            ..Header::default()
        };
        let now = now_epoch_seconds();
        encode(
            &header,
            &serde_json::json!({
                "iss": issuer,
                "sub": "user-1",
                "aud": audience,
                "exp": now + 300,
                "iat": now,
                "nonce": nonce,
                "email": "alice@example.com",
                "email_verified": true
            }),
            key,
        )
        .unwrap()
    }
}
