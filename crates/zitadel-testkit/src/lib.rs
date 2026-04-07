use std::sync::Arc;

use anyhow::Context;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{
        HeaderMap, Method, Request, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE, COOKIE},
    },
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower::util::ServiceExt;
use uuid::Uuid;
use zitadel_api::ApiState;
use zitadel_app::{ApplicationServices, HookPipeline};
use zitadel_authn::{
    cookie::{CookieConfig, sign},
    password::{Swapper, encode_credential_json},
    session::hash_token,
};
use zitadel_config::{Config, password::PasswordHasherConfig};
use zitadel_crypto::SecretBox;
use zitadel_db::{DEFAULT_INSTANCE_ID, Db};
use zitadel_fga::{FgaService, StoreResolver};
use zitadel_login::LoginState;
use zitadel_oidc::{
    OidcState,
    op::{ClientAuthMethod, ClientAuthentication, TokenExchangeRequest},
    rp::{InMemoryIssuerMetadataCache, ReqwestHttpClient, RpService},
};
use zitadel_storage::StorageRuntime;

#[derive(Clone)]
pub struct TestDb {
    pub db: Db,
}

impl TestDb {
    pub async fn new() -> anyhow::Result<Self> {
        let config = zitadel_config::StatefulStorageConfig {
            url: "sqlite://:memory:".into(),
            ..Default::default()
        };
        Self::open_with_config(&config).await
    }

    pub async fn open_with_config(
        config: &zitadel_config::StatefulStorageConfig,
    ) -> anyhow::Result<Self> {
        let db = Db::open_with_config(&config.url, config)
            .await
            .with_context(|| format!("open test db {}", config.url))?;
        zitadel_db::migrate::migrate(&db)
            .await
            .context("run test migrations")?;
        zitadel_db::bootstrap::bootstrap(&db, None)
            .await
            .context("bootstrap default org/admin")?;
        Ok(Self { db })
    }

    pub fn scoped_default(&self) -> zitadel_db::scoped::ScopedDb {
        self.db.scoped_default()
    }

    pub async fn default_org_id(&self) -> anyhow::Result<String> {
        let scoped = self.scoped_default();
        let row: (String,) = sqlx::query_as(
            "SELECT id FROM orgs WHERE instance_id = $1 ORDER BY created_at ASC LIMIT 1",
        )
        .bind(scoped.instance_id())
        .fetch_one(scoped.pool())
        .await
        .context("load default org")?;
        Ok(row.0)
    }
}

#[derive(Clone)]
pub enum AuthActor {
    Anonymous,
    Bearer(String),
    Cookie(String),
}

impl AuthActor {
    pub fn bearer(token: impl Into<String>) -> Self {
        Self::Bearer(token.into())
    }

    pub fn cookie(cookie_header: impl Into<String>) -> Self {
        Self::Cookie(cookie_header.into())
    }

    fn apply(&self, builder: axum::http::request::Builder) -> axum::http::request::Builder {
        match self {
            Self::Anonymous => builder,
            Self::Bearer(token) => builder.header(AUTHORIZATION, format!("Bearer {token}")),
            Self::Cookie(cookie) => builder.header(COOKIE, cookie),
        }
    }
}

#[derive(Clone)]
pub struct UserFixture {
    pub user_id: String,
    pub org_id: String,
    pub identifier: String,
}

#[derive(Clone)]
pub struct SessionFixture {
    pub session_id: String,
    pub token: String,
}

impl SessionFixture {
    pub fn bearer_actor(&self) -> AuthActor {
        AuthActor::bearer(self.token.clone())
    }
}

#[derive(Clone)]
pub struct PatFixture {
    pub pat_id: String,
    pub token: String,
}

impl PatFixture {
    pub fn actor(&self) -> AuthActor {
        AuthActor::bearer(self.token.clone())
    }
}

#[derive(Clone)]
pub struct OidcClientFixture {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

pub struct TestContext {
    pub db: TestDb,
    pub config: Config,
    pub api_state: ApiState,
    pub login_state: LoginState,
    pub oidc_state: OidcState,
    pub cookie_config: Arc<CookieConfig>,
}

impl TestContext {
    pub async fn new() -> anyhow::Result<Self> {
        let mut config = Config::default();
        config.storage.stateful.url = "sqlite://:memory:".into();
        Self::with_config(config).await
    }

    pub async fn with_config(mut config: Config) -> anyhow::Result<Self> {
        let db = TestDb::open_with_config(&config.storage.stateful).await?;

        config.server.external_domain = "localhost".into();
        config.server.public_origin = "http://localhost:18080".into();
        config.server.force_insecure_cookies = true;
        config.password_hasher = PasswordHasherConfig::dev_defaults();
        let secret_box = Arc::new(SecretBox::new("", &std::collections::HashMap::new())?);

        let cookie_config = Arc::new(CookieConfig::new_with_max_age(
            vec!["test-secret".into()],
            &config.server.external_domain,
            config.server.force_insecure_cookies,
            config.session.max_age_secs as i64,
        ));

        let passwords = Arc::new(Swapper::from_config(&config.password_hasher));
        let storage = StorageRuntime::from_config(
            &config.storage,
            db.db.clone(),
            config.session.max_age_secs,
        )
        .await?;
        let fga = Arc::new(FgaService::new(db.db.clone()));
        zitadel_db::seed_builtin_role_definitions(&db.db)
            .await
            .context("seed builtin role definitions")?;
        fga.initialize_platform_store()
            .await
            .context("initialize platform fga store")?;
        fga.initialize_instance(DEFAULT_INSTANCE_ID).await?;
        fga.rebuild_platform_store()
            .await
            .context("rebuild platform fga tuples")?;

        // Build application services (ADR-032).
        let repos = Arc::new(zitadel_server::repo_bridge::build_repositories(
            db.db.clone(),
            storage.transient.clone(),
            fga.clone(),
            storage.analytics.clone(),
        ));
        let hooks = Arc::new(HookPipeline::empty());
        let app = Arc::new(ApplicationServices::new(repos.clone(), hooks, false));

        let oidc_state = OidcState::new_runtime_with_config(
            repos.oidc.clone(),
            config.server.public_origin.clone(),
            "/login".into(),
            DEFAULT_INSTANCE_ID.to_string(),
            &config.oidc,
            repos.oidc_tokens.clone(),
            repos.oidc_keys.clone(),
            secret_box,
            storage.transient.clone(),
            cookie_config.clone(),
        )
        .with_public_origin_override(&config.server.public_origin);

        let api_state = ApiState {
            db: db.db.clone(),
            fga,
            stateful: storage.stateful.clone(),
            transient: storage.transient.clone(),
            analytics: storage.analytics.clone(),
            oidc: oidc_state.clone(),
            passwords: passwords.clone(),
            cookie_config: cookie_config.clone(),
            support_grant_secret: Arc::new(if config.server.management_secret.is_empty() {
                cookie_config
                    .secrets
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "test-support-grant-secret".to_string())
            } else {
                config.server.management_secret.clone()
            }),
            is_dev: true,
            app: app.clone(),
        };

        let login_state = LoginState {
            stateful: storage.stateful.clone(),
            transient: storage.transient.clone(),
            passwords,
            cookie_config: cookie_config.clone(),
            public_origin: Arc::new(config.server.public_origin.clone()),
            public_origin_override: Some(Arc::new(config.server.public_origin.clone())),
            conformance_login_html: false,
            rp: Arc::new(RpService::new(
                ReqwestHttpClient::new(),
                InMemoryIssuerMetadataCache::default(),
            )),
            pow_secret: config.server.management_secret.clone(),
            app,
        };

        Ok(Self {
            db,
            config,
            api_state,
            login_state,
            oidc_state,
            cookie_config,
        })
    }

    pub fn api_router(&self) -> Router {
        zitadel_api::routes(self.api_state.clone())
    }

    pub fn cookie_header_for_token(&self, token: &str) -> String {
        let signed = sign(token, &self.cookie_config.secrets[0]);
        format!("{}={signed}", self.cookie_config.cookie_name())
    }

    pub fn cookie_actor_for_token(&self, token: &str) -> AuthActor {
        AuthActor::cookie(self.cookie_header_for_token(token))
    }

    pub async fn admin_user(&self) -> anyhow::Result<UserFixture> {
        let scoped = self.db.scoped_default();
        let row: (String, String) = sqlx::query_as(
            "SELECT id, org_id FROM users WHERE instance_id = $1 AND identifier = 'admin' LIMIT 1",
        )
        .bind(scoped.instance_id())
        .fetch_one(scoped.pool())
        .await
        .context("load admin user")?;
        Ok(UserFixture {
            user_id: row.0,
            org_id: row.1,
            identifier: "admin".into(),
        })
    }

    pub async fn create_user(
        &self,
        identifier: &str,
        password: &str,
    ) -> anyhow::Result<UserFixture> {
        let scoped = self.db.scoped_default();
        let org_id = self.db.default_org_id().await?;
        let user_id = Uuid::new_v4().to_string();
        let credential_id = format!("cred-{user_id}");
        let password_hash = self
            .login_state
            .passwords
            .hash(password)
            .context("hash test password")?;
        let credential_json = encode_credential_json(&password_hash);
        let sql = format!(
            "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, $2, $3, 'password', {})",
            scoped.json_bind(4),
        );

        sqlx::query(
            "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state) \
             VALUES ($1, $2, $3, $4, $5, 'human', 'active')",
        )
        .bind(&user_id)
        .bind(scoped.instance_id())
        .bind(&org_id)
        .bind(identifier)
        .bind(identifier)
        .execute(scoped.pool())
        .await
        .context("insert test user")?;

        sqlx::query(&sql)
            .bind(&credential_id)
            .bind(scoped.instance_id())
            .bind(&user_id)
            .bind(&credential_json)
            .execute(scoped.pool())
            .await
            .context("insert test password credential")?;

        Ok(UserFixture {
            user_id,
            org_id,
            identifier: identifier.into(),
        })
    }

    pub async fn grant_operator_admin(&self, user: &UserFixture) -> anyhow::Result<()> {
        let scoped = self.db.scoped_default();
        let metadata = r#"{"capabilities":["operator_admin"]}"#;
        let metadata_bind = scoped.json_bind(1);
        let sql = format!(
            "UPDATE users SET metadata = {metadata_bind} WHERE instance_id = $2 AND id = $3"
        );
        sqlx::query(&sql)
            .bind(metadata)
            .bind(scoped.instance_id())
            .bind(&user.user_id)
            .execute(scoped.pool())
            .await
            .context("grant operator_admin")?;
        Ok(())
    }

    pub async fn create_session(&self, user: &UserFixture) -> anyhow::Result<SessionFixture> {
        let session = self
            .login_state
            .transient
            .create_session(
                DEFAULT_INSTANCE_ID,
                &user.user_id,
                &user.org_id,
                "zitadel-testkit",
                "127.0.0.1",
                "",
            )
            .await
            .context("create test session")?;
        Ok(SessionFixture {
            session_id: session.session_id,
            token: session.token,
        })
    }

    pub async fn create_pat(&self, user: &UserFixture, name: &str) -> anyhow::Result<PatFixture> {
        let scoped = self.db.scoped_default();
        let pat_id = Uuid::new_v4().to_string();
        let token = format!("zit_pat_{}", zitadel_crypto::random_hex(24));
        let token_hash = hash_token(&token);
        let sql = format!(
            "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) VALUES ($1, $2, 'pat', $3, $4, $5, {})",
            scoped.json_bind(6),
        );
        sqlx::query(&sql)
            .bind(&pat_id)
            .bind(scoped.instance_id())
            .bind(&token_hash)
            .bind(&user.user_id)
            .bind(name)
            .bind("[\"admin\"]")
            .execute(scoped.pool())
            .await
            .context("insert test pat")?;
        Ok(PatFixture { pat_id, token })
    }

    pub async fn create_oidc_client(
        &self,
        grant_types: &[&str],
    ) -> anyhow::Result<OidcClientFixture> {
        let scoped = self.db.scoped_default();
        let org_id = self.db.default_org_id().await?;
        let app_id = Uuid::new_v4().to_string();
        let client_id = format!("client-{}", &app_id[..8]);
        let client_secret = format!("secret-{}", &Uuid::new_v4().simple());
        let redirect_uri = "http://127.0.0.1:9876/callback".to_string();
        let grant_types_json =
            serde_json::to_string(grant_types).context("serialize oidc grant types")?;
        let response_types_json =
            serde_json::to_string(&["code"]).context("serialize oidc response types")?;
        let redirect_uris_json = serde_json::to_string(&vec![redirect_uri.clone()])
            .context("serialize oidc redirect uris")?;
        let sql = format!(
            "INSERT INTO apps (id, instance_id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, {}, {}, $11)",
            scoped.json_bind(8),
            scoped.json_bind(9),
            scoped.json_bind(10),
        );

        sqlx::query(&sql)
            .bind(&app_id)
            .bind(scoped.instance_id())
            .bind(&org_id)
            .bind("Test Client")
            .bind("web")
            .bind(&client_id)
            .bind(&client_secret)
            .bind(&redirect_uris_json)
            .bind(&grant_types_json)
            .bind(&response_types_json)
            .bind("active")
            .execute(scoped.pool())
            .await
            .context("insert oidc client")?;

        Ok(OidcClientFixture {
            client_id,
            client_secret,
            redirect_uri,
        })
    }

    pub async fn mint_oidc_access_token_for_user(
        &self,
        user: &UserFixture,
    ) -> anyhow::Result<String> {
        let scoped = self.db.scoped_default();
        let client = self.create_oidc_client(&["authorization_code"]).await?;
        let auth_request_id = Uuid::new_v4().to_string();
        let auth_code = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO oidc_auth_requests (id, instance_id, user_id, client_id, redirect_uri, scope, nonce, code_challenge, code, done) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1)",
        )
        .bind(&auth_request_id)
        .bind(scoped.instance_id())
        .bind(&user.user_id)
        .bind(&client.client_id)
        .bind(&client.redirect_uri)
        .bind("openid email profile")
        .bind("nonce-testkit")
        .bind("")
        .bind(&auth_code)
        .execute(scoped.pool())
        .await
        .context("insert oidc auth request")?;

        let token = self
            .oidc_state
            .provider
            .token(&TokenExchangeRequest {
                grant_type: "authorization_code".into(),
                code: auth_code,
                redirect_uri: client.redirect_uri,
                client_auth: Some(ClientAuthentication {
                    client_id: client.client_id,
                    client_secret: client.client_secret,
                    method: ClientAuthMethod::ClientSecretPost,
                }),
                code_verifier: String::new(),
                refresh_token: String::new(),
            })
            .await
            .map_err(|error| anyhow::anyhow!("mint oidc access token: {}", error.body.error))?;

        Ok(token.access_token)
    }
}

pub struct TestApp {
    pub ctx: TestContext,
    router: Router,
}

impl TestApp {
    pub fn new(ctx: TestContext, router: Router) -> Self {
        Self { ctx, router }
    }

    pub async fn request(
        &self,
        method: Method,
        path: &str,
        actor: AuthActor,
        headers: HeaderMap,
        body: Body,
    ) -> anyhow::Result<TestResponse> {
        let mut builder = Request::builder().method(method).uri(path);
        builder = actor.apply(builder);

        {
            let request_headers = builder
                .headers_mut()
                .context("request builder missing headers")?;
            request_headers.extend(headers);
        }

        let request = builder.body(body).context("build test request")?;
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .context("execute test request")?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .context("read test response body")?
            .to_vec();
        Ok(TestResponse {
            status,
            headers,
            body,
        })
    }

    pub async fn get(&self, path: &str, actor: AuthActor) -> anyhow::Result<TestResponse> {
        self.request(method("GET"), path, actor, HeaderMap::new(), Body::empty())
            .await
    }

    pub async fn delete(&self, path: &str, actor: AuthActor) -> anyhow::Result<TestResponse> {
        self.request(
            method("DELETE"),
            path,
            actor,
            HeaderMap::new(),
            Body::empty(),
        )
        .await
    }

    pub async fn post_json(
        &self,
        path: &str,
        actor: AuthActor,
        body: &Value,
    ) -> anyhow::Result<TestResponse> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        self.request(
            method("POST"),
            path,
            actor,
            headers,
            Body::from(body.to_string()),
        )
        .await
    }

    pub async fn patch_json(
        &self,
        path: &str,
        actor: AuthActor,
        body: &Value,
    ) -> anyhow::Result<TestResponse> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        self.request(
            method("PATCH"),
            path,
            actor,
            headers,
            Body::from(body.to_string()),
        )
        .await
    }

    pub async fn post_form(
        &self,
        path: &str,
        actor: AuthActor,
        body: &str,
    ) -> anyhow::Result<TestResponse> {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        self.request(
            method("POST"),
            path,
            actor,
            headers,
            Body::from(body.to_string()),
        )
        .await
    }
}

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl TestResponse {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json_value(&self) -> Value {
        serde_json::from_slice(&self.body).expect("response should contain valid json")
    }

    pub fn json<T: DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&self.body).expect("response should deserialize from json")
    }

    pub fn set_cookie(&self) -> Option<String> {
        self.headers
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    }
}

fn method(value: &str) -> Method {
    Method::from_bytes(value.as_bytes()).expect("valid http method")
}
