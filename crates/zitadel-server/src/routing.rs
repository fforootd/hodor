use std::convert::Infallible;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use ipnet::IpNet;
use lru::LruCache;
use serde::Serialize;
use tower::{Layer, Service};
use zitadel_config::Config;
use zitadel_db::{
    DEFAULT_INSTANCE_ID, Db, InstanceContext, resolve_domain_route, resolve_instance_route,
    with_instance_context,
};
use zitadel_fga::PLATFORM_STORE_ID;

#[derive(Clone)]
pub struct InstanceResolver {
    routing_db: Db,
    cloud_enabled: bool,
    trusted_proxies: Vec<IpNet>,
    cache: Arc<Mutex<LruCache<String, CacheEntry>>>,
    positive_ttl: Duration,
    negative_ttl: Duration,
}

#[derive(Clone)]
pub struct InstanceContextLayer {
    resolver: Arc<InstanceResolver>,
}

#[derive(Clone)]
pub struct InstanceContextService<S> {
    inner: S,
    resolver: Arc<InstanceResolver>,
}

#[derive(Clone, Debug)]
struct CacheEntry {
    expires_at: Instant,
    value: Option<InstanceContext>,
}

#[derive(Clone, Debug)]
struct RequestRoutingInput {
    scheme: String,
    host: String,
    trusted_instance_id: Option<String>,
    path_instance_id: Option<String>,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    code: u16,
}

impl InstanceResolver {
    pub fn new(config: &Config, db: Db) -> Self {
        Self::from_parts(config, db)
    }

    pub async fn from_config(config: &Config, stateful_db: Db) -> anyhow::Result<Self> {
        Ok(Self::from_parts(config, stateful_db))
    }

    fn from_parts(config: &Config, routing_db: Db) -> Self {
        let capacity = NonZeroUsize::new(config.cloud.resolve_cache_capacity()).unwrap();
        Self {
            routing_db,
            cloud_enabled: config.cloud.enabled,
            trusted_proxies: config
                .server
                .trusted_proxies
                .iter()
                .filter_map(|value| value.parse().ok())
                .collect(),
            cache: Arc::new(Mutex::new(LruCache::new(capacity))),
            positive_ttl: Duration::from_secs(config.cloud.resolve_positive_cache_ttl_secs()),
            negative_ttl: Duration::from_secs(config.cloud.resolve_negative_cache_ttl_secs()),
        }
    }

    fn request_input<B>(&self, req: &Request<B>) -> RequestRoutingInput {
        RequestRoutingInput {
            scheme: forwarded_scheme(req).unwrap_or_default(),
            host: normalized_host(req),
            trusted_instance_id: trusted_instance_override(req, &self.trusted_proxies),
            path_instance_id: extract_path_instance_id(req.uri().path()),
        }
    }

    async fn resolve(&self, input: RequestRoutingInput) -> anyhow::Result<InstanceContext> {
        let RequestRoutingInput {
            scheme,
            host,
            trusted_instance_id,
            path_instance_id,
        } = input;

        // Path param takes highest priority — works regardless of cloud.enabled.
        // This is the primary mechanism for the console frontend.
        if let Some(instance_id) = path_instance_id {
            if is_reserved_instance_id(&instance_id) {
                anyhow::bail!("instance not found");
            }
            let cache_key = format!("instance:{instance_id}");
            if let Some(cached) = self.cached(&cache_key) {
                return cached
                    .map(|ctx| hydrate_request_context(ctx, &scheme, &host, "path_param"));
            }

            let resolved = self
                .load_by_instance_id(&instance_id)
                .await?
                .map(|ctx| hydrate_request_context(ctx, &scheme, &host, "path_param"));
            self.remember(cache_key, resolved.clone());
            return resolved.ok_or_else(|| anyhow::anyhow!("instance not found"));
        }

        if !self.cloud_enabled {
            let mut ctx = InstanceContext::new(DEFAULT_INSTANCE_ID);
            ctx.scheme = scheme;
            ctx.host = host;
            ctx.source = "self_host_default".into();
            return Ok(ctx);
        }

        if let Some(instance_id) = trusted_instance_id {
            if is_reserved_instance_id(&instance_id) {
                anyhow::bail!("instance not found");
            }
            let cache_key = format!("instance:{instance_id}");
            if let Some(cached) = self.cached(&cache_key) {
                return cached
                    .map(|ctx| hydrate_request_context(ctx, &scheme, &host, "trusted_header"));
            }

            let resolved = self
                .load_by_instance_id(&instance_id)
                .await?
                .map(|ctx| hydrate_request_context(ctx, &scheme, &host, "trusted_header"));
            self.remember(cache_key, resolved.clone());
            return resolved.ok_or_else(|| anyhow::anyhow!("instance not found"));
        }

        if host.is_empty() {
            anyhow::bail!("host header required");
        }

        let cache_key = format!("host:{host}");
        if let Some(cached) = self.cached(&cache_key) {
            return cached.map(|ctx| hydrate_request_context(ctx, &scheme, &host, "host"));
        }

        let resolved = self
            .load_by_host(&host)
            .await?
            .map(|ctx| hydrate_request_context(ctx, &scheme, &host, "host"));
        self.remember(cache_key, resolved.clone());
        resolved.ok_or_else(|| anyhow::anyhow!("instance not found"))
    }

    fn cached(&self, key: &str) -> Option<anyhow::Result<InstanceContext>> {
        let mut cache = self.cache.lock().expect("resolver cache poisoned");
        let entry = cache.get(key)?.clone();
        if Instant::now() > entry.expires_at {
            let _ = cache.pop(key);
            return None;
        }
        Some(
            entry
                .value
                .ok_or_else(|| anyhow::anyhow!("instance not found")),
        )
    }

    fn remember(&self, key: String, value: Option<InstanceContext>) {
        let ttl = if value.is_some() {
            self.positive_ttl
        } else {
            self.negative_ttl
        };
        let mut cache = self.cache.lock().expect("resolver cache poisoned");
        cache.put(
            key,
            CacheEntry {
                expires_at: Instant::now() + ttl,
                value,
            },
        );
    }

    async fn load_by_host(&self, host: &str) -> anyhow::Result<Option<InstanceContext>> {
        let row = resolve_domain_route(&self.routing_db, host).await?;
        Ok(row.map(|row| InstanceContext {
            instance_id: row.instance_id,
            resolved_org_id: row.resolved_org_id,
            placement_mode: row.placement_mode,
            region_key: row.region_key,
            scheme: String::new(),
            host: host.into(),
            source: "host".into(),
        }))
    }

    async fn load_by_instance_id(
        &self,
        instance_id: &str,
    ) -> anyhow::Result<Option<InstanceContext>> {
        if is_reserved_instance_id(instance_id) {
            return Ok(None);
        }
        let row = resolve_instance_route(&self.routing_db, instance_id).await?;
        Ok(row.map(|row| InstanceContext {
            instance_id: row.instance_id,
            resolved_org_id: None,
            placement_mode: row.placement_mode,
            region_key: row.region_key,
            scheme: String::new(),
            host: String::new(),
            source: "trusted_header".into(),
        }))
    }
}

fn is_reserved_instance_id(instance_id: &str) -> bool {
    instance_id == PLATFORM_STORE_ID
}

impl InstanceContextLayer {
    pub fn new(resolver: Arc<InstanceResolver>) -> Self {
        Self { resolver }
    }
}

impl<S> Layer<S> for InstanceContextLayer {
    type Service = InstanceContextService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InstanceContextService {
            inner,
            resolver: self.resolver.clone(),
        }
    }
}

impl<S, B> Service<Request<B>> for InstanceContextService<S>
where
    S: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        let mut inner = self.inner.clone();
        let resolver = self.resolver.clone();
        let input = resolver.request_input(&req);

        Box::pin(async move {
            let context = match resolver.resolve(input).await {
                Ok(context) => context,
                Err(err) => return Ok(instance_resolution_error(err)),
            };

            req.extensions_mut().insert(context.clone());
            with_instance_context(context, async move { inner.call(req).await }).await
        })
    }
}

fn instance_resolution_error(err: anyhow::Error) -> Response {
    let status = if err.to_string().contains("not found") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::BAD_REQUEST
    };
    (
        status,
        axum::Json(ErrorBody {
            error: err.to_string(),
            code: status.as_u16(),
        }),
    )
        .into_response()
}

fn normalized_host<B>(req: &Request<B>) -> String {
    req.headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(normalize_host)
        .unwrap_or_default()
}

fn forwarded_scheme<B>(req: &Request<B>) -> Option<String> {
    req.headers()
        .get(header::FORWARDED)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_forwarded_proto)
        .or_else(|| {
            req.headers()
                .get("X-Forwarded-Proto")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(',').next())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase())
        })
}

fn parse_forwarded_proto(value: &str) -> Option<String> {
    value.split(',').next().and_then(|entry| {
        entry.split(';').find_map(|part| {
            let (key, raw_value) = part.trim().split_once('=')?;
            if !key.eq_ignore_ascii_case("proto") {
                return None;
            }
            let proto = raw_value.trim().trim_matches('"');
            (!proto.is_empty()).then(|| proto.to_ascii_lowercase())
        })
    })
}

fn normalize_host(value: &str) -> String {
    value
        .trim()
        .trim_end_matches('.')
        .rsplit_once(':')
        .map(|(host, port)| {
            if port.chars().all(|ch| ch.is_ascii_digit()) {
                host
            } else {
                value
            }
        })
        .unwrap_or(value)
        .to_ascii_lowercase()
}

fn hydrate_request_context(
    mut ctx: InstanceContext,
    scheme: &str,
    host: &str,
    source: &str,
) -> InstanceContext {
    ctx.scheme = scheme.to_string();
    ctx.host = host.to_string();
    ctx.source = source.to_string();
    ctx
}

/// Extract instance ID from URL path: `/v1/instances/{id}/...` → `Some(id)`.
/// Only matches when there's a sub-path after the ID (not bare `/v1/instances` or `/v1/instances/{id}`).
fn extract_path_instance_id(path: &str) -> Option<String> {
    let stripped = path.strip_prefix("/v1/instances/")?;
    let id = stripped.split('/').next().filter(|s| !s.is_empty())?;
    // Only match when id is followed by a sub-resource path
    if !stripped[id.len()..].starts_with('/') {
        return None;
    }
    Some(id.to_string())
}

fn trusted_instance_override<B>(req: &Request<B>, trusted_proxies: &[IpNet]) -> Option<String> {
    if trusted_proxies.is_empty() || !request_from_trusted_proxy(req, trusted_proxies) {
        return None;
    }

    req.headers()
        .get("X-Zitadel-Instance")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn request_from_trusted_proxy<B>(req: &Request<B>, trusted_proxies: &[IpNet]) -> bool {
    req.extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|addr| trusted_proxies.iter().any(|net| net.contains(&addr.ip())))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::{Path, PathBuf};

    use axum::body::Body;
    use uuid::Uuid;
    use zitadel_config::Config;

    async fn seeded_resolver() -> InstanceResolver {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db, None).await.unwrap();
        sqlx::query(
            "INSERT INTO instances (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, region_key, feature_overrides) \
             VALUES ('inst_eu', 'default', '1', 'managed', 'active', 'regional', 'europe-west1', '{}')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO domains (domain, instance_id, org_id, is_primary, state, verified) \
             VALUES ('login.example.com', 'inst_eu', NULL, 1, 'active', 0)",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut config = Config::default();
        config.cloud.enabled = true;
        config.cloud.resolver_cache_capacity = 8;
        config.cloud.positive_cache_ttl_secs = 60;
        config.cloud.negative_cache_ttl_secs = 1;
        InstanceResolver::new(&config, db)
    }

    #[tokio::test]
    async fn resolves_cloud_instance_by_host() {
        let resolver = seeded_resolver().await;
        let req = Request::builder()
            .header("X-Forwarded-Proto", "https")
            .header(header::HOST, "login.example.com")
            .body(Body::empty())
            .unwrap();

        let ctx = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap();
        assert_eq!(ctx.instance_id, "inst_eu");
        assert_eq!(ctx.resolved_org_id, None);
        assert_eq!(ctx.region_key.as_deref(), Some("europe-west1"));
        assert_eq!(ctx.placement_mode, "regional");
        assert_eq!(ctx.scheme, "https");
    }

    #[tokio::test]
    async fn resolves_org_bound_domain_and_sets_resolved_org_id() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db, None).await.unwrap();
        sqlx::query(
            "INSERT INTO instances (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, region_key, feature_overrides) \
             VALUES ('inst_child', 'default', '1', 'managed', 'active', 'regional', 'us-central1', '{}')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO orgs (instance_id, id, name, state) VALUES ('inst_child', 'org_child', 'Child', 'active')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO domains (domain, instance_id, org_id, is_primary, state, verified) \
             VALUES ('team.example.com', 'inst_child', 'org_child', 1, 'active', 1)",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut config = Config::default();
        config.cloud.enabled = true;
        let resolver = InstanceResolver::new(&config, db);
        let req = Request::builder()
            .header(header::HOST, "team.example.com")
            .body(Body::empty())
            .unwrap();

        let ctx = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap();
        assert_eq!(ctx.instance_id, "inst_child");
        assert_eq!(ctx.resolved_org_id.as_deref(), Some("org_child"));
    }

    #[tokio::test]
    async fn caches_host_lookup_until_ttl_expires() {
        let resolver = seeded_resolver().await;
        let req = Request::builder()
            .header(header::HOST, "login.example.com")
            .body(Body::empty())
            .unwrap();

        let first = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap();
        assert_eq!(first.region_key.as_deref(), Some("europe-west1"));

        sqlx::query(
            "UPDATE instances SET region_key = 'europe-west2' WHERE instance_id = 'inst_eu'",
        )
        .execute(resolver.routing_db.pool())
        .await
        .unwrap();

        let second = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap();
        assert_eq!(second.region_key.as_deref(), Some("europe-west1"));
    }

    #[tokio::test]
    async fn resolves_default_instance_when_root_domain_mapping_exists() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db, None).await.unwrap();
        sqlx::query(
            "INSERT INTO domains (domain, instance_id, org_id, is_primary, state, verified) \
             VALUES ('root.example.com', 'default', NULL, 1, 'active', 1)",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut config = Config::default();
        config.cloud.enabled = true;
        let resolver = InstanceResolver::new(&config, db);
        let req = Request::builder()
            .header(header::HOST, "root.example.com")
            .header("X-Forwarded-Proto", "https")
            .body(Body::empty())
            .unwrap();

        let ctx = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap();
        assert_eq!(ctx.instance_id, DEFAULT_INSTANCE_ID);
        assert_eq!(ctx.host, "root.example.com");
        assert_eq!(ctx.scheme, "https");
    }

    #[tokio::test]
    async fn rejects_reserved_platform_path_instance_id() {
        let resolver = seeded_resolver().await;
        let req = Request::builder()
            .uri("/v1/instances/_platform/console/bootstrap")
            .header(header::HOST, "root.example.com")
            .body(Body::empty())
            .unwrap();

        let err = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("instance not found"));
    }

    #[tokio::test]
    async fn rejects_reserved_platform_trusted_header_override() {
        let mut config = Config::default();
        config.cloud.enabled = true;
        config.server.trusted_proxies = vec!["127.0.0.1/32".into()];
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db, None).await.unwrap();
        let resolver = InstanceResolver::new(&config, db);

        let mut req = Request::builder()
            .header(header::HOST, "root.example.com")
            .header("X-Zitadel-Instance", "_platform")
            .body(Body::empty())
            .unwrap();
        req.extensions_mut().insert(ConnectInfo(
            "127.0.0.1:8080".parse::<std::net::SocketAddr>().unwrap(),
        ));

        let err = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("instance not found"));
    }

    #[test]
    fn forwarded_header_proto_takes_precedence() {
        let req = Request::builder()
            .header(
                header::FORWARDED,
                "for=192.0.2.60;proto=https;host=demo.example.com",
            )
            .header("X-Forwarded-Proto", "http")
            .body(Body::empty())
            .unwrap();

        assert_eq!(forwarded_scheme(&req).as_deref(), Some("https"));
    }

    #[tokio::test]
    async fn from_config_uses_stateful_database_for_routing() {
        let (stateful_path, stateful_url) = temp_sqlite_url("stateful");
        let (control_plane_path, control_plane_url) = temp_sqlite_url("control-plane");

        let stateful_db = Db::open(&stateful_url).await.unwrap();
        zitadel_db::migrate::migrate(&stateful_db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&stateful_db, None)
            .await
            .unwrap();

        let control_plane_db = Db::open(&control_plane_url).await.unwrap();
        zitadel_db::migrate::migrate(&control_plane_db)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO instances (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, region_key, feature_overrides) \
             VALUES ('inst_apac', 'default', '1', 'managed', 'active', 'regional', 'australia-southeast1', '{}')",
        )
        .execute(stateful_db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO domains (domain, instance_id, org_id, is_primary, state, verified) \
             VALUES ('apac.example.com', 'inst_apac', NULL, 1, 'active', 0)",
        )
        .execute(stateful_db.pool())
        .await
        .unwrap();

        let mut config = Config::default();
        config.cloud.enabled = true;
        config.storage.stateful.url = stateful_url.clone();
        config.cloud.control_plane.url = control_plane_url.clone();
        let resolver = InstanceResolver::from_config(&config, stateful_db.clone())
            .await
            .unwrap();

        let req = Request::builder()
            .header(header::HOST, "apac.example.com")
            .body(Body::empty())
            .unwrap();

        let ctx = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap();
        assert_eq!(ctx.instance_id, "inst_apac");
        assert_eq!(ctx.region_key.as_deref(), Some("australia-southeast1"));

        drop(resolver);
        control_plane_db.close().await;
        stateful_db.close().await;
        cleanup_sqlite_path(&control_plane_path);
        cleanup_sqlite_path(&stateful_path);
    }

    fn temp_sqlite_url(label: &str) -> (PathBuf, String) {
        let path = std::env::temp_dir().join(format!("zitadel-{label}-{}.db", Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        (path, url)
    }

    fn cleanup_sqlite_path(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }
}
