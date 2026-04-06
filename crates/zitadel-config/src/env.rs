use figment::providers::Serialized;
use serde_json::{Map, Value};
use std::env;

/// Map flat Go-style env vars (ZITADEL_PORT) to nested config paths.
pub(crate) fn flat_env_overrides() -> Serialized<Value> {
    use serde_json::json;

    let mut overrides = Map::new();

    // Server
    if let Ok(v) = env::var("ZITADEL_PORT")
        && let Ok(port) = v.parse::<u16>()
    {
        merge_path(&mut overrides, &["server", "port"], json!(port));
    }
    if let Ok(v) = env::var("ZITADEL_EXTERNAL_DOMAIN") {
        merge_path(
            &mut overrides,
            &["server", "external_domain"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_PUBLIC_ORIGIN") {
        merge_path(
            &mut overrides,
            &["server", "public_origin"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_MANAGEMENT_SECRET") {
        merge_path(
            &mut overrides,
            &["server", "management_secret"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_COOKIE_SECRETS") {
        let secrets: Vec<Value> = v
            .split(',')
            .map(|s| Value::String(s.trim().to_string()))
            .collect();
        merge_path(
            &mut overrides,
            &["server", "cookie_secrets"],
            Value::Array(secrets),
        );
    }

    // Storage
    if let Ok(v) = env::var("ZITADEL_STORAGE_STATEFUL_URL") {
        merge_path(
            &mut overrides,
            &["storage", "stateful", "url"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_STORAGE_STATEFUL_DATABASE") {
        merge_path(
            &mut overrides,
            &["storage", "stateful", "database"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_STORAGE_STATEFUL_EMULATOR_HOST") {
        merge_path(
            &mut overrides,
            &["storage", "stateful", "emulator_host"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_STORAGE_STATEFUL_CREDENTIALS_FILE") {
        merge_path(
            &mut overrides,
            &["storage", "stateful", "credentials_file"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_STORAGE_STATEFUL_CREDENTIALS_JSON") {
        merge_path(
            &mut overrides,
            &["storage", "stateful", "credentials_json"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_STORAGE_STATEFUL_BACKEND") {
        merge_path(
            &mut overrides,
            &["storage", "stateful", "backend"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_STORAGE_STATEFUL_MIGRATE") {
        merge_path(
            &mut overrides,
            &["storage", "stateful", "migrate"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_STORAGE_STATEFUL_BOOTSTRAP") {
        merge_path(
            &mut overrides,
            &["storage", "stateful", "bootstrap"],
            Value::String(v),
        );
    }

    // Cloud
    if let Ok(v) = env::var("ZITADEL_CLOUD_ENABLED")
        && let Some(value) = parse_boolish(&v)
    {
        merge_path(&mut overrides, &["cloud", "enabled"], Value::Bool(value));
    }
    if let Ok(v) = env::var("ZITADEL_CLOUD_CONTROL_PLANE_URL") {
        merge_path(
            &mut overrides,
            &["cloud", "control_plane", "url"],
            Value::String(v),
        );
    }

    // Encryption
    if let Ok(v) = env::var("ZITADEL_ENCRYPTION_ACTIVE_KEY_ID") {
        merge_path(
            &mut overrides,
            &["encryption", "active_key_id"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_ENCRYPTION_KEYS")
        && let Ok(keys) = serde_json::from_str::<Value>(&v)
    {
        merge_path(&mut overrides, &["encryption", "keys"], keys);
    }

    // Observability
    if let Ok(v) = env::var("ZITADEL_LOG_LEVEL") {
        merge_path(
            &mut overrides,
            &["observability", "log_level"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_LOG_FORMAT") {
        merge_path(
            &mut overrides,
            &["observability", "log_format"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_CACHE_PATH") {
        merge_path(
            &mut overrides,
            &["observability", "cache_path"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_CACHE_MAX")
        && let Ok(value) = v.parse::<u64>()
    {
        merge_path(
            &mut overrides,
            &["observability", "cache_max"],
            json!(value),
        );
    }
    if let Ok(v) = env::var("ZITADEL_OTEL_ENDPOINT") {
        merge_path(
            &mut overrides,
            &["observability", "sinks", "otel", "endpoint"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_OTEL_PROTOCOL") {
        merge_path(
            &mut overrides,
            &["observability", "sinks", "otel", "protocol"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_ANALYTICS_ENABLED")
        && let Some(value) = parse_boolish(&v)
    {
        merge_path(
            &mut overrides,
            &["observability", "sinks", "analytics", "enabled"],
            Value::Bool(value),
        );
    }
    if let Ok(v) = env::var("ZITADEL_ANALYTICS_DRAIN_INTERVAL") {
        merge_path(
            &mut overrides,
            &["observability", "sinks", "analytics", "drain_interval"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_ANALYTICS_DRAIN_BATCH")
        && let Ok(value) = v.parse::<u32>()
    {
        merge_path(
            &mut overrides,
            &["observability", "sinks", "analytics", "drain_batch"],
            json!(value),
        );
    }
    if let Ok(v) = env::var("ZITADEL_REDACTION_KEYS") {
        let keys = v
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| Value::String(value.to_string()))
            .collect();
        merge_path(
            &mut overrides,
            &["observability", "redaction", "keys"],
            Value::Array(keys),
        );
    }
    if let Ok(v) = env::var("ZITADEL_REDACTION_MASK") {
        merge_path(
            &mut overrides,
            &["observability", "redaction", "mask"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_IP_MODE") {
        merge_path(
            &mut overrides,
            &["observability", "redaction", "ip_mode"],
            Value::String(v),
        );
    }
    merge_stream_env(&mut overrides, "runtime", "ZITADEL_STREAM_RUNTIME");
    merge_stream_env(&mut overrides, "request", "ZITADEL_STREAM_REQUEST");
    merge_stream_env(&mut overrides, "jobs", "ZITADEL_STREAM_JOBS");
    merge_stream_env(&mut overrides, "queue", "ZITADEL_STREAM_QUEUE");
    merge_stream_env(
        &mut overrides,
        "event_handler",
        "ZITADEL_STREAM_EVENT_HANDLER",
    );
    merge_stream_env(
        &mut overrides,
        "event_pusher",
        "ZITADEL_STREAM_EVENT_PUSHER",
    );

    // Dev
    if let Ok(v) = env::var("ZITADEL_MOCK_OIDC")
        && let Some(value) = parse_boolish(&v)
    {
        merge_path(&mut overrides, &["dev", "mock_oidc"], Value::Bool(value));
    }
    if let Ok(v) = env::var("ZITADEL_SEED_FILE") {
        merge_path(&mut overrides, &["dev", "seed_file"], Value::String(v));
    }

    // TLS
    if let Ok(v) = env::var("ZITADEL_TLS_MODE") {
        merge_path(&mut overrides, &["tls", "mode"], Value::String(v));
    }

    // Password hasher
    if let Ok(v) = env::var("ZITADEL_PASSWORD_HASHER_ALGORITHM") {
        merge_path(
            &mut overrides,
            &["password_hasher", "algorithm"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_PASSWORD_HASHER_MEMORY_COST_KB")
        && let Ok(value) = v.parse::<u32>()
    {
        merge_path(
            &mut overrides,
            &["password_hasher", "memory_cost_kb"],
            json!(value),
        );
    }

    // OIDC
    if let Ok(v) = env::var("ZITADEL_OIDC_ACCESS_TOKEN_LIFETIME_SECS")
        && let Ok(value) = v.parse::<u64>()
    {
        merge_path(
            &mut overrides,
            &["oidc", "access_token_lifetime_secs"],
            json!(value),
        );
    }
    if let Ok(v) = env::var("ZITADEL_OIDC_ID_TOKEN_LIFETIME_SECS")
        && let Ok(value) = v.parse::<u64>()
    {
        merge_path(
            &mut overrides,
            &["oidc", "id_token_lifetime_secs"],
            json!(value),
        );
    }

    // Session
    if let Ok(v) = env::var("ZITADEL_SESSION_MAX_AGE_SECS")
        && let Ok(value) = v.parse::<u64>()
    {
        merge_path(&mut overrides, &["session", "max_age_secs"], json!(value));
    }

    Serialized::defaults(Value::Object(overrides))
}

fn merge_stream_env(overrides: &mut Map<String, Value>, stream_name: &str, prefix: &str) {
    if let Ok(v) = env::var(format!("{prefix}_MODE")) {
        merge_path(
            overrides,
            &["observability", "streams", stream_name, "mode"],
            Value::String(v),
        );
    }
    if let Ok(v) = env::var(format!("{prefix}_SINKS")) {
        let sinks = v
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| Value::String(value.to_string()))
            .collect();
        merge_path(
            overrides,
            &["observability", "streams", stream_name, "sinks"],
            Value::Array(sinks),
        );
    }
    if let Ok(v) = env::var(format!("{prefix}_SAMPLE_RATE"))
        && let Ok(value) = v.parse::<f64>()
    {
        merge_path(
            overrides,
            &["observability", "streams", stream_name, "sample_rate"],
            Value::from(value),
        );
    }
}

fn parse_boolish(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub(crate) fn merge_path(overrides: &mut Map<String, Value>, path: &[&str], value: Value) {
    let Some((head, tail)) = path.split_first() else {
        return;
    };
    if tail.is_empty() {
        overrides.insert((*head).to_string(), value);
        return;
    }

    let section = overrides
        .entry((*head).to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(map) = section {
        merge_path(map, tail, value);
    }
}
