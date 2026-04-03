use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, anyhow, bail};
use reqwest::{Client, Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use uuid::Uuid;
use zitadel_oidc::oidc::{OpenIdConfiguration, TokenResponse, s256_challenge};

const CLI_SERVICE: &str = "zitadel-cli";
const DEFAULT_PROFILE: &str = "default";
const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:8787/callback";
const DEFAULT_SCOPES: &[&str] = &["openid", "profile", "email"];

#[derive(Clone, Debug, Default)]
pub struct RemoteOverrides {
    pub profile: Option<String>,
    pub profile_path: Option<PathBuf>,
    pub issuer: Option<String>,
    pub api_url: Option<String>,
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub access_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ClientProfilesFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, ClientProfile>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ClientProfile {
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default = "default_redirect_uri")]
    pub redirect_uri: String,
    #[serde(default = "default_auth_mode")]
    pub auth_mode: String,
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredSession {
    pub auth_mode: String,
    pub token_type: String,
    pub storage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_epoch: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub enum CommandOutput {
    Json(Value),
    Ndjson(Vec<Value>),
    Text(String),
}

pub struct RemoteContext {
    profile_name: String,
    profile_path: PathBuf,
    state_path: PathBuf,
    profile: ClientProfile,
    session: Option<StoredSession>,
    access_token_override: Option<String>,
    client: Client,
}

pub async fn auth_login(
    overrides: &RemoteOverrides,
    no_browser: bool,
) -> anyhow::Result<CommandOutput> {
    let mut ctx = RemoteContext::resolve(overrides, false)?;
    let discovery = ctx.discovery().await?;
    let redirect = Url::parse(&ctx.profile.redirect_uri)
        .with_context(|| format!("invalid redirect_uri {}", ctx.profile.redirect_uri))?;
    let state = Uuid::new_v4().simple().to_string();
    let nonce = Uuid::new_v4().simple().to_string();
    let verifier = format!(
        "{}{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let challenge = s256_challenge(&verifier);

    let mut authorize = Url::parse(&discovery.authorization_endpoint).with_context(|| {
        format!(
            "invalid authorization endpoint {}",
            discovery.authorization_endpoint
        )
    })?;
    {
        let mut query = authorize.query_pairs_mut();
        query.append_pair("response_type", "code");
        query.append_pair("client_id", &ctx.profile.client_id);
        query.append_pair("redirect_uri", redirect.as_str());
        query.append_pair("scope", &ctx.profile.scopes.join(" "));
        query.append_pair("state", &state);
        query.append_pair("nonce", &nonce);
        query.append_pair("code_challenge", &challenge);
        query.append_pair("code_challenge_method", "S256");
    }

    let callback = tokio::time::timeout(
        Duration::from_secs(300),
        wait_for_callback(&redirect, authorize.as_str(), no_browser),
    )
    .await
    .context("timed out waiting for login callback")??;

    if callback.state != state {
        bail!("OIDC login state mismatch");
    }

    let token = ctx
        .client
        .post(discovery.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", callback.code.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("client_id", ctx.profile.client_id.as_str()),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await
        .context("exchange authorization code")?;
    let status = token.status();
    let token_text = token.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("token exchange failed ({status}): {token_text}");
    }
    let token: TokenResponse =
        serde_json::from_str(&token_text).context("decode token response")?;

    ctx.profile.auth_mode = "oidc".into();
    ctx.persist_profile()?;
    ctx.save_session(StoredSession {
        auth_mode: "oidc".into(),
        token_type: token.token_type.clone(),
        storage: String::new(),
        access_token: Some(token.access_token.clone()),
        id_token: token.id_token.clone(),
        expires_at_epoch: Some(now_epoch() + token.expires_in),
    })?;

    Ok(CommandOutput::Json(json!({
        "profile": ctx.profile_name,
        "issuer": ctx.profile.issuer,
        "api_url": ctx.profile.api_url,
        "client_id": ctx.profile.client_id,
        "redirect_uri": ctx.profile.redirect_uri,
        "auth_mode": "oidc",
        "expires_at_epoch": now_epoch() + token.expires_in,
    })))
}

pub fn auth_token_set(overrides: &RemoteOverrides, token: String) -> anyhow::Result<CommandOutput> {
    let mut ctx = RemoteContext::resolve(overrides, false)?;
    if ctx.profile.api_url.is_empty() {
        if let Some(issuer) = overrides.issuer.as_ref() {
            ctx.profile.issuer = issuer.clone();
            ctx.profile.api_url = issuer.clone();
        } else {
            bail!("api_url or issuer is required to save a remote token");
        }
    }
    ctx.profile.auth_mode = "pat".into();
    ctx.persist_profile()?;
    ctx.save_session(StoredSession {
        auth_mode: "pat".into(),
        token_type: "Bearer".into(),
        storage: String::new(),
        access_token: Some(token),
        id_token: None,
        expires_at_epoch: None,
    })?;
    Ok(CommandOutput::Json(json!({
        "profile": ctx.profile_name,
        "api_url": ctx.profile.api_url,
        "auth_mode": "pat",
        "stored": true,
    })))
}

pub fn auth_logout(overrides: &RemoteOverrides) -> anyhow::Result<CommandOutput> {
    let ctx = RemoteContext::resolve(overrides, false)?;
    ctx.clear_session()?;
    Ok(CommandOutput::Json(json!({
        "profile": ctx.profile_name,
        "logged_out": true,
    })))
}

pub fn auth_status(overrides: &RemoteOverrides) -> anyhow::Result<CommandOutput> {
    let ctx = RemoteContext::resolve(overrides, false)?;
    let storage = ctx
        .session
        .as_ref()
        .map(|session| session.storage.as_str())
        .unwrap_or("none");
    Ok(CommandOutput::Json(json!({
        "profile": ctx.profile_name,
        "profile_path": ctx.profile_path,
        "state_path": ctx.state_path,
        "issuer": empty_as_none(&ctx.profile.issuer),
        "api_url": empty_as_none(&ctx.profile.api_url),
        "client_id": empty_as_none(&ctx.profile.client_id),
        "redirect_uri": empty_as_none(&ctx.profile.redirect_uri),
        "auth_mode": empty_as_none(&ctx.profile.auth_mode),
        "authenticated": ctx.access_token().is_ok(),
        "session_storage": storage,
        "expires_at_epoch": ctx
            .session
            .as_ref()
            .and_then(|session| session.expires_at_epoch),
    })))
}

pub async fn auth_whoami(overrides: &RemoteOverrides) -> anyhow::Result<CommandOutput> {
    let ctx = RemoteContext::resolve(overrides, true)?;
    ctx.request_json(Method::GET, "/v1/auth/whoami", &[], None, false)
        .await
}

pub async fn schema_inspect(
    overrides: &RemoteOverrides,
    id: Option<String>,
    meta: bool,
) -> anyhow::Result<CommandOutput> {
    let ctx = RemoteContext::resolve(overrides, true)?;
    let path = if meta {
        "/v1/schemas/$meta".to_string()
    } else if let Some(id) = id {
        validate_identifier(&id)?;
        format!("/v1/schemas/{id}")
    } else {
        "/v1/schemas".to_string()
    };
    ctx.request_json(Method::GET, &path, &[], None, false).await
}

pub async fn api_call(
    overrides: &RemoteOverrides,
    method: Method,
    path: &str,
    params: &[(String, String)],
    body: Option<Value>,
    dry_run: bool,
    require_auth: bool,
) -> anyhow::Result<CommandOutput> {
    let ctx = RemoteContext::resolve(overrides, require_auth)?;
    ctx.request_json(method, path, params, body, dry_run).await
}

impl RemoteContext {
    pub fn resolve(overrides: &RemoteOverrides, require_profile: bool) -> anyhow::Result<Self> {
        let profile_path = overrides
            .profile_path
            .clone()
            .unwrap_or_else(default_profile_path);
        let mut profiles = load_profiles_file(&profile_path)?;

        let profile_name = resolve_profile_name(overrides, &profiles);
        let mut profile =
            profiles
                .profiles
                .remove(&profile_name)
                .unwrap_or_else(|| ClientProfile {
                    redirect_uri: default_redirect_uri(),
                    auth_mode: default_auth_mode(),
                    scopes: default_scopes(),
                    ..Default::default()
                });

        if let Some(issuer) = overrides
            .issuer
            .clone()
            .or_else(|| env::var("ZITADEL_ISSUER").ok())
        {
            profile.issuer = issuer;
        }
        if let Some(api_url) = overrides
            .api_url
            .clone()
            .or_else(|| env::var("ZITADEL_API_URL").ok())
        {
            profile.api_url = api_url;
        } else if profile.api_url.is_empty() && !profile.issuer.is_empty() {
            profile.api_url = profile.issuer.clone();
        }
        if let Some(client_id) = overrides
            .client_id
            .clone()
            .or_else(|| env::var("ZITADEL_CLIENT_ID").ok())
        {
            profile.client_id = client_id;
        }
        if let Some(redirect_uri) = overrides
            .redirect_uri
            .clone()
            .or_else(|| env::var("ZITADEL_REDIRECT_URI").ok())
        {
            profile.redirect_uri = redirect_uri;
        }
        if profile.redirect_uri.is_empty() {
            profile.redirect_uri = default_redirect_uri();
        }
        if profile.scopes.is_empty() {
            profile.scopes = default_scopes();
        }

        if require_profile && profile.api_url.is_empty() {
            bail!("api_url or issuer is required; configure a client profile first");
        }

        profiles
            .profiles
            .insert(profile_name.clone(), profile.clone());
        if profiles.default_profile.is_none() {
            profiles.default_profile = Some(profile_name.clone());
        }
        write_profiles_file(&profile_path, &profiles)?;

        let state_path = default_state_path(&profile_name);
        let access_token_override = overrides
            .access_token
            .clone()
            .or_else(|| env::var("ZITADEL_ACCESS_TOKEN").ok());

        Ok(Self {
            profile_name,
            profile_path,
            state_path: state_path.clone(),
            profile,
            session: load_session(&state_path).unwrap_or(None),
            access_token_override,
            client: Client::builder().build().context("build HTTP client")?,
        })
    }

    pub async fn request_json(
        &self,
        method: Method,
        path: &str,
        params: &[(String, String)],
        body: Option<Value>,
        dry_run: bool,
    ) -> anyhow::Result<CommandOutput> {
        let url = self.request_url(path, params)?;
        if dry_run {
            return Ok(CommandOutput::Json(json!({
                "dry_run": true,
                "method": method.as_str(),
                "url": url.as_str(),
                "body": body,
            })));
        }

        let mut request = self.client.request(method.clone(), url.clone());
        if let Ok(token) = self.access_token() {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("send {} {}", method.as_str(), url))?;
        response_to_output(response).await
    }

    async fn discovery(&self) -> anyhow::Result<OpenIdConfiguration> {
        if self.profile.issuer.is_empty() {
            bail!("issuer is required for OIDC login");
        }
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            self.profile.issuer.trim_end_matches('/')
        );
        let response = self
            .client
            .get(&discovery_url)
            .send()
            .await
            .with_context(|| format!("fetch OIDC discovery from {discovery_url}"))?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("OIDC discovery failed ({status}): {text}");
        }
        serde_json::from_str(&text).context("decode OIDC discovery document")
    }

    fn request_url(&self, path: &str, params: &[(String, String)]) -> anyhow::Result<Url> {
        validate_request_path(path)?;
        let combined = format!(
            "{}/{}",
            self.profile.api_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        let mut url =
            Url::parse(&combined).with_context(|| format!("invalid request URL {combined}"))?;
        for (key, value) in params {
            validate_identifier(key)?;
            reject_control_chars(value)?;
            url.query_pairs_mut().append_pair(key, value);
        }
        Ok(url)
    }

    fn access_token(&self) -> anyhow::Result<String> {
        if let Some(token) = self.access_token_override.clone() {
            return Ok(token);
        }

        let Some(session) = self.session.as_ref() else {
            bail!("not authenticated; run `zitadel auth login` or `zitadel auth token set`");
        };
        if session.storage == "keyring"
            && let Ok(token) = keyring_get(&self.profile_name)
        {
            return Ok(token);
        }
        session.access_token.clone().ok_or_else(|| {
            anyhow!(
                "no access token available for profile {}",
                self.profile_name
            )
        })
    }

    fn persist_profile(&self) -> anyhow::Result<()> {
        let mut profiles = load_profiles_file(&self.profile_path)?;
        profiles
            .profiles
            .insert(self.profile_name.clone(), self.profile.clone());
        if profiles.default_profile.is_none() {
            profiles.default_profile = Some(self.profile_name.clone());
        }
        write_profiles_file(&self.profile_path, &profiles)
    }

    fn save_session(&self, mut session: StoredSession) -> anyhow::Result<()> {
        ensure_parent_dir(&self.state_path)?;
        match session.access_token.take() {
            Some(token) if keyring_enabled() => match keyring_set(&self.profile_name, &token) {
                Ok(()) => {
                    session.storage = "keyring".into();
                }
                Err(_) => {
                    session.storage = "file".into();
                    session.access_token = Some(token);
                }
            },
            Some(token) => {
                session.storage = "file".into();
                session.access_token = Some(token);
            }
            None => {}
        }
        write_json_file(&self.state_path, &session)
    }

    fn clear_session(&self) -> anyhow::Result<()> {
        let _ = keyring_delete(&self.profile_name);
        if self.state_path.exists() {
            fs::remove_file(&self.state_path)
                .with_context(|| format!("remove {}", self.state_path.display()))?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct CallbackPayload {
    code: String,
    state: String,
}

async fn wait_for_callback(
    redirect_uri: &Url,
    authorize_url: &str,
    no_browser: bool,
) -> anyhow::Result<CallbackPayload> {
    let host = redirect_uri.host_str().unwrap_or_default();
    if host != "127.0.0.1" && host != "localhost" {
        bail!("redirect_uri must use localhost or 127.0.0.1");
    }

    let listener = TcpListener::bind((
        "127.0.0.1",
        redirect_uri.port_or_known_default().unwrap_or(8787),
    ))
    .await
    .with_context(|| format!("bind login callback on {}", redirect_uri))?;
    eprintln!("Open this URL to authenticate:\n{authorize_url}");
    if !no_browser {
        let _ = webbrowser::open(authorize_url);
    }

    let expected_path = redirect_uri.path().to_string();
    let (mut stream, _) = listener.accept().await.context("accept login callback")?;
    let mut buffer = vec![0_u8; 8192];
    let read = stream
        .read(&mut buffer)
        .await
        .context("read login callback")?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let line = request
        .lines()
        .next()
        .ok_or_else(|| anyhow!("invalid HTTP callback request"))?;
    let target = line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("invalid callback request line"))?;
    let callback_url =
        Url::parse(&format!("http://127.0.0.1{target}")).context("parse callback URL")?;
    if callback_url.path() != expected_path {
        bail!("unexpected callback path {}", callback_url.path());
    }

    let code = callback_url
        .query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.to_string())
        .ok_or_else(|| anyhow!("authorization code missing in callback"))?;
    let state = callback_url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.to_string())
        .ok_or_else(|| anyhow!("state missing in callback"))?;

    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: 29\r\n\r\nAuthentication complete. Return.",
        )
        .await
        .ok();

    Ok(CallbackPayload { code, state })
}

async fn response_to_output(response: reqwest::Response) -> anyhow::Result<CommandOutput> {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let parsed = serde_json::from_str::<Value>(&text).ok();

    if !status.is_success() {
        if let Some(parsed) = parsed {
            bail!(
                "request failed ({status}): {}",
                serde_json::to_string_pretty(&parsed)?
            );
        }
        bail!("request failed ({status}): {text}");
    }

    if let Some(parsed) = parsed {
        return Ok(CommandOutput::Json(parsed));
    }
    Ok(CommandOutput::Text(text))
}

fn resolve_profile_name(overrides: &RemoteOverrides, profiles: &ClientProfilesFile) -> String {
    overrides
        .profile
        .clone()
        .or_else(|| env::var("ZITADEL_PROFILE").ok())
        .or_else(|| profiles.default_profile.clone())
        .unwrap_or_else(|| DEFAULT_PROFILE.to_string())
}

fn load_profiles_file(path: &Path) -> anyhow::Result<ClientProfilesFile> {
    if !path.exists() {
        return Ok(ClientProfilesFile::default());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn write_profiles_file(path: &Path, profiles: &ClientProfilesFile) -> anyhow::Result<()> {
    ensure_parent_dir(path)?;
    let raw = toml::to_string_pretty(profiles).context("serialize client profiles")?;
    write_text_file(path, &raw)
}

fn load_session(path: &Path) -> anyhow::Result<Option<StoredSession>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let session: StoredSession =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    Ok(Some(session))
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let raw = serde_json::to_string_pretty(value).context("serialize JSON file")?;
    write_text_file(path, &raw)
}

fn write_text_file(path: &Path, raw: &str) -> anyhow::Result<()> {
    ensure_parent_dir(path)?;
    fs::write(path, raw).with_context(|| format!("write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("chmod 600 {}", path.display()))?;
    }
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    Ok(())
}

fn validate_request_path(path: &str) -> anyhow::Result<()> {
    reject_control_chars(path)?;
    if path.is_empty() || !path.starts_with('/') {
        bail!("request path must start with '/'");
    }
    if path.contains("..") || path.contains('#') || path.contains('\\') || path.contains('%') {
        bail!("unsafe request path");
    }
    Ok(())
}

pub fn validate_identifier(value: &str) -> anyhow::Result<()> {
    reject_control_chars(value)?;
    if value.is_empty() {
        bail!("identifier must not be empty");
    }
    if value.contains('/')
        || value.contains('?')
        || value.contains('#')
        || value.contains('\\')
        || value.contains('%')
    {
        bail!("unsafe identifier {value}");
    }
    Ok(())
}

pub fn reject_control_chars(value: &str) -> anyhow::Result<()> {
    if value
        .chars()
        .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
    {
        bail!("control characters are not allowed");
    }
    Ok(())
}

pub fn parse_json_input(input: Option<&str>) -> anyhow::Result<Option<Value>> {
    let Some(input) = input else {
        return Ok(None);
    };
    let raw = if let Some(path) = input.strip_prefix('@') {
        fs::read_to_string(path).with_context(|| format!("read {path}"))?
    } else {
        input.to_string()
    };
    let parsed = serde_json::from_str(&raw).context("parse JSON input")?;
    Ok(Some(parsed))
}

pub fn parse_key_value_pairs(values: &[String]) -> anyhow::Result<Map<String, Value>> {
    let mut map = Map::new();
    for item in values {
        let (key, value) = item
            .split_once('=')
            .ok_or_else(|| anyhow!("expected key=value, got {item}"))?;
        validate_identifier(key)?;
        reject_control_chars(value)?;
        let parsed = serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.into()));
        map.insert(key.to_string(), parsed);
    }
    Ok(map)
}

pub fn parse_params_input(input: Option<&str>) -> anyhow::Result<Vec<(String, String)>> {
    let Some(value) = parse_json_input(input)? else {
        return Ok(Vec::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("--params must be a JSON object"))?;
    let mut params = Vec::with_capacity(object.len());
    for (key, value) in object {
        validate_identifier(key)?;
        let value = match value {
            Value::Null => "null".to_string(),
            Value::Bool(value) => value.to_string(),
            Value::Number(value) => value.to_string(),
            Value::String(value) => value.clone(),
            other => serde_json::to_string(other).context("serialize nested query param")?,
        };
        reject_control_chars(&value)?;
        params.push((key.clone(), value));
    }
    Ok(params)
}

fn empty_as_none(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn default_profile_path() -> PathBuf {
    xdg_config_home().join("zitadel").join("client.toml")
}

fn default_state_path(profile_name: &str) -> PathBuf {
    xdg_state_home()
        .join("zitadel")
        .join(format!("{profile_name}.json"))
}

fn xdg_config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn xdg_state_home() -> PathBuf {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn keyring_enabled() -> bool {
    env::var("ZITADEL_DISABLE_KEYRING").ok().as_deref() != Some("1")
}

fn keyring_set(profile_name: &str, token: &str) -> anyhow::Result<()> {
    let entry = keyring::Entry::new(CLI_SERVICE, profile_name)?;
    entry.set_password(token)?;
    Ok(())
}

fn keyring_get(profile_name: &str) -> anyhow::Result<String> {
    let entry = keyring::Entry::new(CLI_SERVICE, profile_name)?;
    Ok(entry.get_password()?)
}

fn keyring_delete(profile_name: &str) -> anyhow::Result<()> {
    let entry = keyring::Entry::new(CLI_SERVICE, profile_name)?;
    let _ = entry.delete_credential();
    Ok(())
}

fn default_auth_mode() -> String {
    "oidc".into()
}

fn default_redirect_uri() -> String {
    DEFAULT_REDIRECT_URI.into()
}

fn default_scopes() -> Vec<String> {
    DEFAULT_SCOPES
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_value_pairs() {
        let map = parse_key_value_pairs(&[
            "identifier=\"admin\"".into(),
            "enabled=true".into(),
            "count=2".into(),
        ])
        .unwrap();
        assert_eq!(map["identifier"], Value::String("admin".into()));
        assert_eq!(map["enabled"], Value::Bool(true));
        assert_eq!(map["count"], Value::Number(2.into()));
    }

    #[test]
    fn rejects_unsafe_identifiers() {
        assert!(validate_identifier("../admin").is_err());
        assert!(validate_identifier("abc/def").is_err());
        assert!(validate_identifier("abc%2fdef").is_err());
        assert!(validate_identifier("ok-id").is_ok());
    }

    #[test]
    fn parses_params_json() {
        let mut params = parse_params_input(Some(r#"{"limit": 10, "cursor": "abc"}"#)).unwrap();
        params.sort();
        assert_eq!(
            params,
            vec![
                ("cursor".to_string(), "abc".to_string()),
                ("limit".to_string(), "10".to_string()),
            ]
        );
    }
}
