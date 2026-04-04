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
use zitadel_db::{DEFAULT_INSTANCE_ID, Db, InstanceContext, with_instance_context};

#[derive(Clone)]
pub struct InstanceResolver {
    control_plane_db: Db,
    cloud_enabled: bool,
    trusted_proxies: Vec<IpNet>,
    cache: Arc<Mutex<LruCache<String, CacheEntry>>>,
    positive_ttl: Duration,
    negative_ttl: Duration,
    default_backend_kind: String,
    default_backend_url: String,
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
    host: String,
    trusted_instance_id: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct InstanceRouteRow {
    instance_id: String,
    customer_id: String,
    placement_mode: String,
    region_key: Option<String>,
    backend_key: String,
    backend_kind: String,
    backend_url: String,
    backend_secret_ref: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    code: u16,
}

impl InstanceResolver {
    pub fn new(config: &Config, db: Db) -> Self {
        Self::from_parts(
            config,
            db.clone(),
            db.dialect().to_string(),
            config.storage.stateful.url.clone(),
        )
    }

    pub async fn from_config(config: &Config, stateful_db: Db) -> anyhow::Result<Self> {
        let control_plane_url = config
            .cloud
            .resolve_control_plane_url(&config.storage.stateful.url);
        let control_plane_db =
            if control_plane_url.is_empty() || control_plane_url == config.storage.stateful.url {
                stateful_db.clone()
            } else {
                Db::open(control_plane_url).await?
            };

        Ok(Self::from_parts(
            config,
            control_plane_db,
            stateful_db.dialect().to_string(),
            config.storage.stateful.url.clone(),
        ))
    }

    fn from_parts(
        config: &Config,
        control_plane_db: Db,
        default_backend_kind: String,
        default_backend_url: String,
    ) -> Self {
        let capacity = NonZeroUsize::new(config.cloud.resolve_cache_capacity()).unwrap();
        Self {
            control_plane_db,
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
            default_backend_kind,
            default_backend_url,
        }
    }

    fn request_input<B>(&self, req: &Request<B>) -> RequestRoutingInput {
        RequestRoutingInput {
            host: normalized_host(req),
            trusted_instance_id: trusted_instance_override(req, &self.trusted_proxies),
        }
    }

    async fn resolve(&self, input: RequestRoutingInput) -> anyhow::Result<InstanceContext> {
        let RequestRoutingInput {
            host,
            trusted_instance_id,
        } = input;

        if !self.cloud_enabled {
            return Ok(InstanceContext {
                instance_id: DEFAULT_INSTANCE_ID.into(),
                customer_id: String::new(),
                placement_mode: "global".into(),
                region_key: None,
                backend_key: "default".into(),
                backend_kind: self.default_backend_kind.clone(),
                backend_url: self.default_backend_url.clone(),
                backend_secret_ref: String::new(),
                host,
                source: "self_host_default".into(),
            });
        }

        if let Some(instance_id) = trusted_instance_id {
            let cache_key = format!("instance:{instance_id}");
            if let Some(cached) = self.cached(&cache_key) {
                return cached;
            }

            let resolved = self
                .load_by_instance_id(&instance_id)
                .await?
                .map(|mut ctx| {
                    ctx.source = "trusted_header".into();
                    ctx.host = host.clone();
                    ctx
                });
            self.remember(cache_key, resolved.clone());
            return resolved.ok_or_else(|| anyhow::anyhow!("instance not found"));
        }

        if host.is_empty() {
            anyhow::bail!("host header required");
        }

        let cache_key = format!("host:{host}");
        if let Some(cached) = self.cached(&cache_key) {
            return cached;
        }

        let resolved = self.load_by_host(&host).await?.map(|mut ctx| {
            ctx.host = host.clone();
            ctx.source = "host".into();
            ctx
        });
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
        let row = sqlx::query_as::<_, InstanceRouteRow>(
            "SELECT i.instance_id, i.customer_id, i.placement_mode, \
                    COALESCE(NULLIF(i.region_key, ''), NULLIF(b.region_key, '')) AS region_key, \
                    i.backend_key, b.kind AS backend_kind, b.url AS backend_url, \
                    b.secret_ref AS backend_secret_ref \
             FROM instance_domains d \
             JOIN instances i ON i.instance_id = d.instance_id \
             JOIN cloud_backends b ON b.backend_key = i.backend_key \
             WHERE d.domain = $1 AND d.state = 'active' AND i.state = 'active' AND b.state = 'active' \
             ORDER BY d.is_primary DESC LIMIT 1",
        )
        .bind(host)
        .fetch_optional(self.control_plane_db.pool())
        .await?;

        Ok(row.map(|row| InstanceContext {
            instance_id: row.instance_id,
            customer_id: row.customer_id,
            placement_mode: row.placement_mode,
            region_key: row.region_key,
            backend_key: row.backend_key,
            backend_kind: row.backend_kind,
            backend_url: row.backend_url,
            backend_secret_ref: row.backend_secret_ref,
            host: host.into(),
            source: "host".into(),
        }))
    }

    async fn load_by_instance_id(
        &self,
        instance_id: &str,
    ) -> anyhow::Result<Option<InstanceContext>> {
        let row = sqlx::query_as::<_, InstanceRouteRow>(
            "SELECT i.instance_id, i.customer_id, i.placement_mode, \
                    COALESCE(NULLIF(i.region_key, ''), NULLIF(b.region_key, '')) AS region_key, \
                    i.backend_key, b.kind AS backend_kind, b.url AS backend_url, \
                    b.secret_ref AS backend_secret_ref \
             FROM instances i \
             JOIN cloud_backends b ON b.backend_key = i.backend_key \
             WHERE i.instance_id = $1 AND i.state = 'active' AND b.state = 'active' LIMIT 1",
        )
        .bind(instance_id)
        .fetch_optional(self.control_plane_db.pool())
        .await?;

        Ok(row.map(|row| InstanceContext {
            instance_id: row.instance_id,
            customer_id: row.customer_id,
            placement_mode: row.placement_mode,
            region_key: row.region_key,
            backend_key: row.backend_key,
            backend_kind: row.backend_kind,
            backend_url: row.backend_url,
            backend_secret_ref: row.backend_secret_ref,
            host: String::new(),
            source: "trusted_header".into(),
        }))
    }
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
        sqlx::query(
            "INSERT INTO cloud_backends (backend_key, kind, url, secret_ref, region_key, state, global_default) \
             VALUES ('eu-primary', 'spanner', 'spanner://projects/example/instances/eu/databases/identity', \
                     'projects/example/secrets/eu-primary', 'europe-west1', 'active', 0)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO instances (instance_id, customer_id, state, primary_domain, placement_mode, region_key, backend_key) \
             VALUES ('inst_eu', 'cust_1', 'active', 'login.example.com', 'regional', 'europe-west1', 'eu-primary')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO instance_domains (domain, instance_id, is_primary, state) VALUES ('login.example.com', 'inst_eu', 1, 'active')",
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
            .header(header::HOST, "login.example.com")
            .body(Body::empty())
            .unwrap();

        let ctx = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap();
        assert_eq!(ctx.instance_id, "inst_eu");
        assert_eq!(ctx.backend_key, "eu-primary");
        assert_eq!(ctx.backend_kind, "spanner");
        assert_eq!(
            ctx.backend_url,
            "spanner://projects/example/instances/eu/databases/identity"
        );
        assert_eq!(
            ctx.backend_secret_ref,
            "projects/example/secrets/eu-primary"
        );
        assert_eq!(ctx.region_key.as_deref(), Some("europe-west1"));
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
        assert_eq!(first.backend_key, "eu-primary");

        sqlx::query(
            "INSERT INTO cloud_backends (backend_key, kind, url, secret_ref, region_key, state, global_default) \
             VALUES ('eu-secondary', 'spanner', 'spanner://projects/example/instances/eu2/databases/identity', \
                     'projects/example/secrets/eu-secondary', 'europe-west2', 'active', 0)",
        )
        .execute(resolver.control_plane_db.pool())
        .await
        .unwrap();
        sqlx::query(
            "UPDATE instances SET backend_key = 'eu-secondary', region_key = 'europe-west2' WHERE instance_id = 'inst_eu'",
        )
        .execute(resolver.control_plane_db.pool())
        .await
        .unwrap();

        let second = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap();
        assert_eq!(second.backend_key, "eu-primary");
    }

    #[tokio::test]
    async fn falls_back_to_backend_region_when_instance_region_is_empty() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO cloud_backends (backend_key, kind, url, secret_ref, region_key, state, global_default) \
             VALUES ('us-primary', 'spanner', 'spanner://projects/example/instances/us/databases/identity', \
                     'projects/example/secrets/us-primary', 'us-central1', 'active', 1)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO instances (instance_id, customer_id, state, primary_domain, placement_mode, region_key, backend_key) \
             VALUES ('inst_us', 'cust_2', 'active', 'us.example.com', 'regional', NULL, 'us-primary')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO instance_domains (domain, instance_id, is_primary, state) VALUES ('us.example.com', 'inst_us', 1, 'active')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut config = Config::default();
        config.cloud.enabled = true;
        let resolver = InstanceResolver::new(&config, db);
        let req = Request::builder()
            .header(header::HOST, "us.example.com")
            .body(Body::empty())
            .unwrap();

        let ctx = resolver
            .resolve(resolver.request_input(&req))
            .await
            .unwrap();
        assert_eq!(ctx.region_key.as_deref(), Some("us-central1"));
    }

    #[tokio::test]
    async fn resolves_routes_from_a_separate_control_plane_database() {
        let (stateful_path, stateful_url) = temp_sqlite_url("stateful");
        let (control_plane_path, control_plane_url) = temp_sqlite_url("control-plane");

        let stateful_db = Db::open(&stateful_url).await.unwrap();
        zitadel_db::migrate::migrate(&stateful_db).await.unwrap();

        let control_plane_db = Db::open(&control_plane_url).await.unwrap();
        zitadel_db::migrate::migrate(&control_plane_db)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO cloud_backends (backend_key, kind, url, secret_ref, region_key, state, global_default) \
             VALUES ('apac-primary', 'spanner', 'spanner://projects/example/instances/apac/databases/identity', \
                     'projects/example/secrets/apac-primary', 'australia-southeast1', 'active', 0)",
        )
        .execute(control_plane_db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO instances (instance_id, customer_id, state, primary_domain, placement_mode, region_key, backend_key) \
             VALUES ('inst_apac', 'cust_3', 'active', 'apac.example.com', 'regional', 'australia-southeast1', 'apac-primary')",
        )
        .execute(control_plane_db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO instance_domains (domain, instance_id, is_primary, state) VALUES ('apac.example.com', 'inst_apac', 1, 'active')",
        )
        .execute(control_plane_db.pool())
        .await
        .unwrap();

        let stateful_row: Option<(String,)> =
            sqlx::query_as("SELECT instance_id FROM instance_domains WHERE domain = $1")
                .bind("apac.example.com")
                .fetch_optional(stateful_db.pool())
                .await
                .unwrap();
        assert!(stateful_row.is_none());

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
        assert_eq!(ctx.backend_key, "apac-primary");
        assert_eq!(ctx.backend_kind, "spanner");
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
