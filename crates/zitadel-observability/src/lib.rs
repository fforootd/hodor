mod middleware;
pub mod propagation;

use std::{
    collections::{BTreeMap, HashSet, hash_map::DefaultHasher},
    fmt,
    hash::{Hash, Hasher},
    io::Write,
    path::Path,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Context as _;
use google_cloud_spanner::{client::Error as SpannerError, statement::Statement};
use middleware::REQUEST_ID_HEADER;
use opentelemetry::{
    KeyValue,
    logs::{AnyValue, LogRecord as _, Logger as _, LoggerProvider as _, Severity},
    trace::{SpanId, TraceFlags, TraceId, TracerProvider as _},
};
use opentelemetry_otlp::{LogExporter, MetricExporter, Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    logs::{SdkLogger, SdkLoggerProvider},
    metrics::SdkMeterProvider,
    trace::SdkTracerProvider,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{
    Row,
    sqlite::{SqlitePoolOptions, SqliteRow},
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time::MissedTickBehavior,
};
use tracing::{Event, Id, Subscriber, field::Visit};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{
    EnvFilter, Layer, Registry, layer::Context, prelude::*, registry::LookupSpan,
    util::SubscriberInitExt,
};
use uuid::Uuid;
use zitadel_config::{ObservabilityConfig, OtelSinkConfig, StreamConfig};
use zitadel_db::{DEFAULT_INSTANCE_ID, Db, Dialect};

pub use middleware::{
    RequestContext, classify_request, record_server_timing, request_context_middleware, time_async,
};

const SERVICE_NAME: &str = "zitadel";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const ANALYTICS_BREAKER_FAILURES: u32 = 5;
const ANALYTICS_BREAKER_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stream {
    Runtime,
    Request,
    Jobs,
    EventHandler,
    EventPusher,
}

impl Stream {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Request => "request",
            Self::Jobs => "jobs",
            Self::EventHandler => "event_handler",
            Self::EventPusher => "event_pusher",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "runtime" => Some(Self::Runtime),
            "request" => Some(Self::Request),
            "jobs" | "queue" => Some(Self::Jobs),
            "event_handler" => Some(Self::EventHandler),
            "event_pusher" => Some(Self::EventPusher),
            _ => None,
        }
    }
}

impl fmt::Display for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SinkKind {
    Stdout,
    Analytics,
    Otel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReliabilityMode {
    Buffered,
    Sampled,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, Debug)]
struct StreamRouting {
    mode: ReliabilityMode,
    sample_rate: f64,
    sinks: HashSet<SinkKind>,
}

#[derive(Clone, Debug)]
struct Redaction {
    keys: HashSet<String>,
    mask: String,
    ip_mode: IpMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IpMode {
    Keep,
    Redact,
    Hash,
    Mask,
}

#[derive(Clone)]
struct RuntimeState {
    format: OutputFormat,
    gcp_logging: Option<String>, // Some(project_id) when GCP Cloud Logging JSON is enabled
    redaction: Redaction,
    streams: BTreeMap<Stream, StreamRouting>,
    analytics_tx: Option<mpsc::UnboundedSender<StructuredRecord>>,
    otel_tx: Option<mpsc::UnboundedSender<StructuredRecord>>,
}

#[derive(Clone, Debug)]
struct StructuredRecord {
    id: String,
    created_at_ms: i64,
    level: String,
    severity: Severity,
    stream: Stream,
    message: String,
    event_type: String,
    category: String,
    org_id: String,
    actor_id: Option<String>,
    actor_type: Option<String>,
    aggregate_id: Option<String>,
    aggregate_type: Option<String>,
    resource_type: Option<String>,
    request_id: Option<String>,
    session_id: Option<String>,
    flow_id: Option<String>,
    fingerprint: Option<String>,
    client_id: Option<String>,
    token_id: Option<String>,
    delegation_type: Option<String>,
    sdk_name: Option<String>,
    sdk_version: Option<String>,
    payload: Value,
    metadata: Value,
}

#[derive(Clone, Default)]
struct SpanFields {
    fields: BTreeMap<String, Value>,
}

pub struct ObservabilityGuard {
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
}

impl Drop for ObservabilityGuard {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        for task in &self.tasks {
            task.abort();
        }
        if let Some(provider) = self.tracer_provider.take() {
            let _ = provider.shutdown();
        }
        if let Some(provider) = self.meter_provider.take() {
            let _ = provider.shutdown();
        }
    }
}

#[derive(Clone)]
struct ObservabilityLayer {
    state: Arc<RuntimeState>,
}

pub async fn install(
    config: &ObservabilityConfig,
    analytics_db: Option<Db>,
) -> anyhow::Result<ObservabilityGuard> {
    let redaction = build_redaction(&config.redaction);
    let streams = build_streams(config);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut tasks = Vec::new();

    let (analytics_tx, analytics_tasks) = if config.sinks.analytics.enabled {
        if let Some(db) = analytics_db.clone() {
            let (tx, handles) = start_analytics_pipeline(config, db, shutdown_rx.clone()).await?;
            (Some(tx), handles)
        } else {
            (None, Vec::new())
        }
    } else {
        (None, Vec::new())
    };
    tasks.extend(analytics_tasks);

    let (otel_tx, otel_tasks) =
        if !config.sinks.otel.endpoint.is_empty() && config.sinks.otel.logs_enabled {
            let (tx, handles) = start_otel_log_pipeline(config, shutdown_rx.clone()).await?;
            (Some(tx), handles)
        } else {
            (None, Vec::new())
        };
    tasks.extend(otel_tasks);

    let gcp_project_id = resolve_gcp_project_id(&config.gcp_project_id);

    let format = match config.log_format.as_str() {
        "json" => OutputFormat::Json,
        _ => OutputFormat::Text,
    };
    let gcp_logging = if config.gcp_cloud_logging && format == OutputFormat::Json {
        gcp_project_id.clone()
    } else {
        None
    };
    let state = Arc::new(RuntimeState {
        format,
        gcp_logging,
        redaction,
        streams,
        analytics_tx,
        otel_tx,
    });

    // Build OTEL TracerProvider for distributed trace export (Tier 3)
    let tracer_provider =
        if !config.sinks.otel.endpoint.is_empty() && config.sinks.otel.traces_enabled {
            let provider = build_tracer_provider(&config.sinks.otel).await?;
            // Register W3C TraceContext propagator for inbound/outbound trace correlation
            opentelemetry::global::set_text_map_propagator(
                opentelemetry_sdk::propagation::TraceContextPropagator::new(),
            );
            Some(provider)
        } else {
            None
        };

    // Build OTEL MeterProvider for metrics export (Tier 3)
    let meter_provider =
        if !config.sinks.otel.endpoint.is_empty() && config.sinks.otel.metrics_enabled {
            let provider = build_meter_provider(&config.sinks.otel).await?;
            opentelemetry::global::set_meter_provider(provider.clone());
            Some(provider)
        } else {
            None
        };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.log_level.clone()));

    // Subscriber stack: EnvFilter + ObservabilityLayer (wide events) + OpenTelemetryLayer (trace export)
    let otel_layer = tracer_provider
        .as_ref()
        .map(|provider| OpenTelemetryLayer::new(provider.tracer(SERVICE_NAME)));

    Registry::default()
        .with(filter)
        .with(ObservabilityLayer { state })
        .with(otel_layer)
        .try_init()?;

    Ok(ObservabilityGuard {
        shutdown: shutdown_tx,
        tasks,
        tracer_provider,
        meter_provider,
    })
}

impl<S> Layer<S> for ObservabilityLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut visitor = JsonVisitor::default();
        attrs.record(&mut visitor);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(SpanFields {
                fields: visitor.fields,
            });
        }
    }

    fn on_record(&self, id: &Id, values: &tracing::span::Record<'_>, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let mut visitor = JsonVisitor::default();
            values.record(&mut visitor);
            let mut extensions = span.extensions_mut();
            let span_fields = extensions.get_mut::<SpanFields>();
            if let Some(span_fields) = span_fields {
                span_fields.fields.extend(visitor.fields);
            } else {
                extensions.insert(SpanFields {
                    fields: visitor.fields,
                });
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut merged = BTreeMap::new();
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope.from_root() {
                if let Some(fields) = span.extensions().get::<SpanFields>() {
                    merged.extend(fields.fields.clone());
                }
            }
        }

        let mut visitor = JsonVisitor::default();
        event.record(&mut visitor);
        merged.extend(visitor.fields);

        let Some(mut record) = StructuredRecord::from_event(event.metadata(), merged) else {
            return;
        };
        redact_record(&self.state.redaction, &mut record);

        let Some(route) = route_record(&self.state, &record) else {
            return;
        };

        if route.stdout {
            write_stdout(
                self.state.format,
                self.state.gcp_logging.as_deref(),
                &record,
            );
        }
        if route.analytics
            && let Some(tx) = &self.state.analytics_tx
        {
            let _ = tx.send(record.clone());
        }
        if route.otel
            && let Some(tx) = &self.state.otel_tx
        {
            let _ = tx.send(record);
        }
    }
}

#[derive(Clone, Copy)]
struct Route {
    stdout: bool,
    analytics: bool,
    otel: bool,
}

fn route_record(state: &RuntimeState, record: &StructuredRecord) -> Option<Route> {
    let routing = state.streams.get(&record.stream)?;
    if routing.mode == ReliabilityMode::Off {
        return None;
    }
    if routing.mode == ReliabilityMode::Sampled && !should_sample(routing.sample_rate, &record.id) {
        return None;
    }
    Some(Route {
        stdout: routing.sinks.contains(&SinkKind::Stdout),
        analytics: routing.sinks.contains(&SinkKind::Analytics),
        otel: routing.sinks.contains(&SinkKind::Otel),
    })
}

impl StructuredRecord {
    fn from_event(
        metadata: &tracing::Metadata<'_>,
        mut fields: BTreeMap<String, Value>,
    ) -> Option<Self> {
        let level = metadata.level().to_string().to_ascii_lowercase();
        let stream = take_string(&mut fields, "stream")
            .as_deref()
            .and_then(Stream::from_name)
            .unwrap_or(Stream::Runtime);
        let message = take_string(&mut fields, "message").unwrap_or_default();
        let event_type =
            take_string(&mut fields, "event_type").unwrap_or_else(|| format!("log.{level}"));
        let category =
            take_string(&mut fields, "category").unwrap_or_else(|| category_for(&event_type));
        let org_id = take_string(&mut fields, "org_id").unwrap_or_else(|| "0".into());
        let actor_id = take_string(&mut fields, "actor_id");
        let actor_type = take_string(&mut fields, "actor_type");
        let aggregate_id = take_string(&mut fields, "aggregate_id");
        let aggregate_type = take_string(&mut fields, "aggregate_type");
        let resource_type = take_string(&mut fields, "resource_type");
        let parent_span_id = take_string(&mut fields, "parent_span_id");

        let request_id = take_string(&mut fields, REQUEST_ID_HEADER)
            .or_else(|| take_string(&mut fields, "request_id"));
        let session_id = take_string(&mut fields, "session_id");
        let flow_id = take_string(&mut fields, "flow_id");
        let fingerprint = take_string(&mut fields, "fingerprint");
        let client_id = take_string(&mut fields, "client_id");
        let token_id = take_string(&mut fields, "token_id");
        let delegation_type = take_string(&mut fields, "delegation_type");
        let sdk_name = take_string(&mut fields, "sdk_name");
        let sdk_version = take_string(&mut fields, "sdk_version");

        let mut payload = Map::new();
        if !message.is_empty() {
            payload.insert("message".into(), Value::String(message.clone()));
        }
        for (key, value) in fields {
            if is_reserved_key(&key) {
                continue;
            }
            payload.insert(key, value);
        }

        let mut metadata_fields = Map::new();
        metadata_fields.insert("stream".into(), Value::String(stream.to_string()));
        metadata_fields.insert("level".into(), Value::String(level.clone()));
        metadata_fields.insert(
            "target".into(),
            Value::String(metadata.target().to_string()),
        );
        metadata_fields.insert("version".into(), Value::String(VERSION.to_string()));
        if let Some(file) = metadata.file() {
            metadata_fields.insert("file".into(), Value::String(file.to_string()));
        }
        if let Some(line) = metadata.line() {
            metadata_fields.insert("line".into(), Value::Number(line.into()));
        }
        if let Some(parent_span_id) = parent_span_id {
            metadata_fields.insert("parent_span_id".into(), Value::String(parent_span_id));
        }

        Some(Self {
            id: Uuid::now_v7().to_string(),
            created_at_ms: now_ms(),
            level: level.clone(),
            severity: severity_for(metadata.level()),
            stream,
            message,
            event_type,
            category,
            org_id,
            actor_id,
            actor_type,
            aggregate_id,
            aggregate_type,
            resource_type,
            request_id,
            session_id,
            flow_id,
            fingerprint,
            client_id,
            token_id,
            delegation_type,
            sdk_name,
            sdk_version,
            payload: Value::Object(payload),
            metadata: Value::Object(metadata_fields),
        })
    }
}

fn is_reserved_key(key: &str) -> bool {
    matches!(
        key,
        "stream" | "event_type" | "category" | "request_id" | REQUEST_ID_HEADER | "parent_span_id"
    )
}

fn build_streams(config: &ObservabilityConfig) -> BTreeMap<Stream, StreamRouting> {
    [
        Stream::Runtime,
        Stream::Request,
        Stream::Jobs,
        Stream::EventHandler,
        Stream::EventPusher,
    ]
    .into_iter()
    .filter_map(|stream| {
        config
            .streams
            .by_name(stream.as_str())
            .map(|cfg| (stream, parse_stream_config(cfg)))
    })
    .collect()
}

fn parse_stream_config(config: &StreamConfig) -> StreamRouting {
    let mode = match config.mode.as_str() {
        "sampled" => ReliabilityMode::Sampled,
        "off" => ReliabilityMode::Off,
        _ => ReliabilityMode::Buffered,
    };
    let sinks = config
        .sinks
        .iter()
        .filter_map(|sink| match sink.as_str() {
            "stdout" => Some(SinkKind::Stdout),
            "analytics" => Some(SinkKind::Analytics),
            "otel" => Some(SinkKind::Otel),
            _ => None,
        })
        .collect();
    StreamRouting {
        mode,
        sample_rate: config.sample_rate.clamp(0.0, 1.0),
        sinks,
    }
}

fn build_redaction(config: &zitadel_config::RedactionConfig) -> Redaction {
    let ip_mode = match config.ip_mode.as_str() {
        "redact" => IpMode::Redact,
        "hash" => IpMode::Hash,
        "mask" => IpMode::Mask,
        _ => IpMode::Keep,
    };
    Redaction {
        keys: config
            .keys
            .iter()
            .map(|key| key.to_ascii_lowercase())
            .collect(),
        mask: config.mask.clone(),
        ip_mode,
    }
}

fn redact_record(redaction: &Redaction, record: &mut StructuredRecord) {
    redact_value(redaction, None, &mut record.payload);
    redact_value(redaction, None, &mut record.metadata);

    if let Some(value) = record.fingerprint.as_mut() {
        *value = redact_scalar(redaction, "fingerprint", value.clone());
    }
}

fn redact_value(redaction: &Redaction, key: Option<&str>, value: &mut Value) {
    match value {
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for child_key in keys {
                if let Some(child_value) = map.get_mut(&child_key) {
                    let lower = child_key.to_ascii_lowercase();
                    if redaction.keys.contains(&lower) {
                        *child_value = Value::String(redaction.mask.clone());
                        continue;
                    }
                    if is_ip_key(&lower) {
                        *child_value = Value::String(redact_ip(redaction, child_value));
                        continue;
                    }
                    redact_value(redaction, Some(&lower), child_value);
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                redact_value(redaction, key, child);
            }
        }
        Value::String(string) => {
            if let Some(key) = key
                && is_ip_key(key)
            {
                *string = redact_ip(redaction, &Value::String(string.clone()));
            }
        }
        _ => {}
    }
}

fn redact_scalar(redaction: &Redaction, key: &str, value: String) -> String {
    if redaction.keys.contains(&key.to_ascii_lowercase()) {
        return redaction.mask.clone();
    }
    value
}

fn is_ip_key(key: &str) -> bool {
    matches!(key, "ip" | "ip_address" | "client_ip")
}

fn redact_ip(redaction: &Redaction, value: &Value) -> String {
    match redaction.ip_mode {
        IpMode::Keep => value_as_string(value).unwrap_or_default(),
        IpMode::Redact => "[redacted-ip]".into(),
        IpMode::Mask => redaction.mask.clone(),
        IpMode::Hash => {
            let mut hasher = Sha256::new();
            hasher.update(value_as_string(value).unwrap_or_default());
            format!("{:x}", hasher.finalize())
        }
    }
}

fn should_sample(sample_rate: f64, seed: &str) -> bool {
    if sample_rate >= 1.0 {
        return true;
    }
    if sample_rate <= 0.0 {
        return false;
    }
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let bucket = hasher.finish() as f64 / u64::MAX as f64;
    bucket <= sample_rate
}

fn write_stdout(format: OutputFormat, gcp_project: Option<&str>, record: &StructuredRecord) {
    match format {
        OutputFormat::Json if gcp_project.is_some() => {
            write_stdout_gcp_json(gcp_project.unwrap(), record);
        }
        OutputFormat::Json => {
            let line = json!({
                "timestamp_ms": record.created_at_ms,
                "level": record.level,
                "stream": record.stream.as_str(),
                "event_type": record.event_type,
                "request_id": record.request_id,
                "message": record.message,
                "payload": record.payload,
                "metadata": record.metadata,
            });
            let _ = writeln!(std::io::stdout(), "{}", line);
        }
        OutputFormat::Text => {
            let mut line = format!(
                "{} level={} stream={} event_type={} msg={}",
                record.created_at_ms,
                record.level,
                record.stream,
                record.event_type,
                serde_json::to_string(&record.message).unwrap_or_else(|_| "\"\"".into())
            );
            if let Some(request_id) = &record.request_id {
                line.push_str(&format!(" request_id={request_id}"));
            }
            if let Value::Object(payload) = &record.payload {
                for (key, value) in payload {
                    if key == "message" {
                        continue;
                    }
                    line.push(' ');
                    line.push_str(key);
                    line.push('=');
                    line.push_str(&compact_value(value));
                }
            }
            let _ = writeln!(std::io::stdout(), "{line}");
        }
    }
}

fn compact_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn write_stdout_gcp_json(project_id: &str, record: &StructuredRecord) {
    let severity = match record.level.as_str() {
        "error" => "ERROR",
        "warn" => "WARNING",
        "debug" => "DEBUG",
        "trace" => "DEBUG",
        _ => "INFO",
    };
    let secs = record.created_at_ms / 1000;
    let nanos = (record.created_at_ms % 1000) * 1_000_000;
    let time = format!("{}.{:09}Z", chrono_format_utc(secs), nanos,);

    let mut line = serde_json::Map::new();
    line.insert("severity".into(), Value::String(severity.into()));
    line.insert("time".into(), Value::String(time));
    line.insert("message".into(), Value::String(record.message.clone()));
    line.insert(
        "stream".into(),
        Value::String(record.stream.as_str().into()),
    );
    line.insert(
        "event_type".into(),
        Value::String(record.event_type.clone()),
    );

    // Trace correlation: request_id maps to trace_id per ADR-023
    if let Some(request_id) = &record.request_id {
        line.insert(
            "logging.googleapis.com/trace".into(),
            Value::String(format!("projects/{project_id}/traces/{request_id}")),
        );
    }
    if let Some(span_id) = record
        .metadata
        .get("parent_span_id")
        .and_then(value_as_string)
    {
        line.insert(
            "logging.googleapis.com/spanId".into(),
            Value::String(span_id),
        );
    }

    // Source location from tracing metadata
    let mut source_location = serde_json::Map::new();
    if let Some(file) = record.metadata.get("file").and_then(value_as_string) {
        source_location.insert("file".into(), Value::String(file));
    }
    if let Some(line_num) = record.metadata.get("line") {
        source_location.insert("line".into(), line_num.clone());
    }
    if !source_location.is_empty() {
        line.insert(
            "logging.googleapis.com/sourceLocation".into(),
            Value::Object(source_location),
        );
    }

    if let Some(request_id) = &record.request_id {
        line.insert("request_id".into(), Value::String(request_id.clone()));
    }
    line.insert("payload".into(), record.payload.clone());
    line.insert("metadata".into(), record.metadata.clone());

    let _ = writeln!(std::io::stdout(), "{}", Value::Object(line));
}

/// Formats a unix timestamp in seconds as "YYYY-MM-DDTHH:MM:SS" UTC.
fn chrono_format_utc(secs: i64) -> String {
    const SECS_PER_DAY: i64 = 86400;
    let days = secs.div_euclid(SECS_PER_DAY);
    let day_secs = secs.rem_euclid(SECS_PER_DAY);
    let h = day_secs / 3600;
    let m = (day_secs % 3600) / 60;
    let s = day_secs % 60;

    // Civil date from days since epoch (algorithm from Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!("{year:04}-{month:02}-{d:02}T{h:02}:{m:02}:{s:02}")
}

fn category_for(event_type: &str) -> String {
    event_type
        .split('.')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("log")
        .to_string()
}

fn severity_for(level: &tracing::Level) -> Severity {
    match *level {
        tracing::Level::ERROR => Severity::Error,
        tracing::Level::WARN => Severity::Warn,
        tracing::Level::INFO => Severity::Info,
        tracing::Level::DEBUG => Severity::Debug,
        tracing::Level::TRACE => Severity::Trace,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => None,
        _ => Some(value.to_string()),
    }
}

fn take_string(fields: &mut BTreeMap<String, Value>, key: &str) -> Option<String> {
    fields.remove(key).and_then(|value| value_as_string(&value))
}

#[derive(Default)]
struct JsonVisitor {
    fields: BTreeMap<String, Value>,
}

impl Visit for JsonVisitor {
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields.insert(field.name().into(), Value::Bool(value));
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.fields.insert(field.name().into(), json!(value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.insert(field.name().into(), json!(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.insert(field.name().into(), json!(value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields
            .insert(field.name().into(), Value::String(value.to_string()));
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.fields
            .insert(field.name().into(), Value::String(value.to_string()));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        self.fields
            .insert(field.name().into(), Value::String(format!("{value:?}")));
    }
}

async fn start_analytics_pipeline(
    config: &ObservabilityConfig,
    analytics_db: Db,
    shutdown: watch::Receiver<bool>,
) -> anyhow::Result<(mpsc::UnboundedSender<StructuredRecord>, Vec<JoinHandle<()>>)> {
    ensure_parent_dir(&config.cache_path)?;
    let cache_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", config.cache_path))
        .await
        .with_context(|| format!("open analytics cache at {}", config.cache_path))?;
    init_cache_schema(&cache_pool).await?;

    let (tx, rx) = mpsc::unbounded_channel();
    let writer_shutdown = shutdown.clone();
    let writer_pool = cache_pool.clone();
    let cache_max = config.cache_max as i64;
    let writer = tokio::spawn(async move {
        analytics_writer_loop(writer_pool, rx, writer_shutdown, cache_max).await;
    });

    let drain_interval = parse_duration(&config.sinks.analytics.drain_interval);
    let drain_batch = config.sinks.analytics.drain_batch.max(1) as i64;
    let drainer = tokio::spawn(async move {
        analytics_drainer_loop(
            cache_pool,
            analytics_db,
            shutdown,
            drain_interval,
            drain_batch,
        )
        .await;
    });

    Ok((tx, vec![writer, drainer]))
}

fn ensure_parent_dir(path: &str) -> anyhow::Result<()> {
    if let Some(parent) = Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

async fn init_cache_schema(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(pool)
        .await?;
    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(pool)
        .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS log_buffer (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            instance_id TEXT NOT NULL,
            event_id TEXT NOT NULL,
            stream TEXT NOT NULL,
            event_type TEXT NOT NULL,
            category TEXT NOT NULL,
            level TEXT NOT NULL,
            message TEXT NOT NULL,
            payload TEXT NOT NULL DEFAULT '{}',
            metadata TEXT NOT NULL DEFAULT '{}',
            org_id TEXT NOT NULL DEFAULT '0',
            actor_id TEXT,
            actor_type TEXT,
            aggregate_id TEXT,
            aggregate_type TEXT,
            resource_type TEXT,
            request_id TEXT,
            session_id TEXT,
            flow_id TEXT,
            fingerprint TEXT DEFAULT '',
            client_id TEXT DEFAULT '',
            token_id TEXT DEFAULT '',
            delegation_type TEXT DEFAULT '',
            sdk_name TEXT DEFAULT '',
            sdk_version TEXT DEFAULT '',
            created_at_ms INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_log_buffer_id ON log_buffer(id)")
        .execute(pool)
        .await?;
    Ok(())
}

async fn analytics_writer_loop(
    pool: sqlx::SqlitePool,
    mut rx: mpsc::UnboundedReceiver<StructuredRecord>,
    mut shutdown: watch::Receiver<bool>,
    cache_max: i64,
) {
    let mut writes = 0_i64;
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            maybe_record = rx.recv() => {
                let Some(record) = maybe_record else { break; };
                if insert_buffered_record(&pool, &record).await.is_ok() {
                    writes += 1;
                    if writes % 64 == 0 {
                        let _ = trim_buffer(&pool, cache_max).await;
                    }
                }
            }
        }
    }
}

async fn insert_buffered_record(
    pool: &sqlx::SqlitePool,
    record: &StructuredRecord,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO log_buffer (
            instance_id, event_id, stream, event_type, category, level, message, payload, metadata,
            org_id, actor_id, actor_type, aggregate_id, aggregate_type, resource_type,
            request_id, session_id, flow_id, fingerprint, client_id, token_id,
            delegation_type, sdk_name, sdk_version, created_at_ms
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(DEFAULT_INSTANCE_ID)
    .bind(&record.id)
    .bind(record.stream.as_str())
    .bind(&record.event_type)
    .bind(&record.category)
    .bind(&record.level)
    .bind(&record.message)
    .bind(record.payload.to_string())
    .bind(record.metadata.to_string())
    .bind(&record.org_id)
    .bind(record.actor_id.clone())
    .bind(record.actor_type.clone())
    .bind(record.aggregate_id.clone())
    .bind(record.aggregate_type.clone())
    .bind(record.resource_type.clone())
    .bind(record.request_id.clone())
    .bind(record.session_id.clone())
    .bind(record.flow_id.clone())
    .bind(record.fingerprint.clone().unwrap_or_default())
    .bind(record.client_id.clone().unwrap_or_default())
    .bind(record.token_id.clone().unwrap_or_default())
    .bind(record.delegation_type.clone().unwrap_or_default())
    .bind(record.sdk_name.clone().unwrap_or_default())
    .bind(record.sdk_version.clone().unwrap_or_default())
    .bind(record.created_at_ms)
    .execute(pool)
    .await?;
    Ok(())
}

async fn trim_buffer(pool: &sqlx::SqlitePool, cache_max: i64) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM log_buffer
         WHERE id IN (
            SELECT id FROM log_buffer
            ORDER BY id DESC
            LIMIT -1 OFFSET ?
         )",
    )
    .bind(cache_max)
    .execute(pool)
    .await?;
    Ok(())
}

async fn analytics_drainer_loop(
    cache_pool: sqlx::SqlitePool,
    analytics_db: Db,
    mut shutdown: watch::Receiver<bool>,
    drain_interval: Duration,
    drain_batch: i64,
) {
    let mut breaker = CircuitBreaker::default();
    let mut ticker = tokio::time::interval(drain_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            _ = ticker.tick() => {
                if !breaker.allows() {
                    continue;
                }
                match drain_once(&cache_pool, &analytics_db, drain_batch).await {
                    Ok(drained) => {
                        if drained > 0 {
                            breaker.on_success();
                        }
                    }
                    Err(_) => breaker.on_failure(),
                }
            }
        }
    }
}

#[derive(Default)]
struct CircuitBreaker {
    failures: u32,
    open_until: Option<Instant>,
}

impl CircuitBreaker {
    fn allows(&self) -> bool {
        match self.open_until {
            Some(deadline) => Instant::now() >= deadline,
            None => true,
        }
    }

    fn on_success(&mut self) {
        self.failures = 0;
        self.open_until = None;
    }

    fn on_failure(&mut self) {
        self.failures += 1;
        if self.failures >= ANALYTICS_BREAKER_FAILURES {
            self.open_until = Some(Instant::now() + ANALYTICS_BREAKER_COOLDOWN);
        }
    }
}

#[derive(Clone, Debug)]
struct BufferedEventRow {
    buffer_id: i64,
    instance_id: String,
    id: String,
    event_type: String,
    category: String,
    org_id: String,
    actor_id: Option<String>,
    actor_type: Option<String>,
    aggregate_id: Option<String>,
    aggregate_type: Option<String>,
    resource_type: Option<String>,
    payload: String,
    metadata: String,
    request_id: Option<String>,
    session_id: Option<String>,
    flow_id: Option<String>,
    fingerprint: String,
    client_id: String,
    token_id: String,
    delegation_type: String,
    sdk_name: String,
    sdk_version: String,
    created_at_ms: i64,
}

async fn drain_once(
    cache_pool: &sqlx::SqlitePool,
    analytics_db: &Db,
    drain_batch: i64,
) -> anyhow::Result<usize> {
    let rows = sqlx::query(
        "SELECT id, instance_id, event_id, event_type, category, level, message, payload, metadata,
                org_id, actor_id, actor_type, aggregate_id, aggregate_type, resource_type,
                request_id, session_id, flow_id, fingerprint, client_id, token_id,
                delegation_type, sdk_name, sdk_version, created_at_ms
         FROM log_buffer
         ORDER BY id
         LIMIT ?",
    )
    .bind(drain_batch)
    .fetch_all(cache_pool)
    .await?;

    if rows.is_empty() {
        return Ok(0);
    }

    let events: Vec<BufferedEventRow> = rows.iter().map(buffered_event_row).collect();

    match analytics_db {
        Db::Sql(_) => {
            let mut tx = analytics_db.pool().begin().await?;
            for event in &events {
                insert_event_row_sql(&mut tx, analytics_db.dialect(), event).await?;
            }
            tx.commit().await?;
        }
        Db::Spanner(spanner) => {
            let events = events.clone();
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let events = events.clone();
                    Box::pin(async move {
                        for event in &events {
                            tx.update(spanner_insert_event_stmt(event)).await?;
                        }
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
        }
    }

    for event in &events {
        sqlx::query("DELETE FROM log_buffer WHERE id = ?")
            .bind(event.buffer_id)
            .execute(cache_pool)
            .await?;
    }

    Ok(events.len())
}

fn buffered_event_row(row: &SqliteRow) -> BufferedEventRow {
    BufferedEventRow {
        buffer_id: row.get(0),
        instance_id: row.get(1),
        id: row.get(2),
        event_type: row.get(3),
        category: row.get(4),
        payload: row.get(7),
        metadata: row.get(8),
        org_id: row.get(9),
        actor_id: row.try_get::<Option<String>, _>(10).ok().flatten(),
        actor_type: row.try_get::<Option<String>, _>(11).ok().flatten(),
        aggregate_id: row.try_get::<Option<String>, _>(12).ok().flatten(),
        aggregate_type: row.try_get::<Option<String>, _>(13).ok().flatten(),
        resource_type: row.try_get::<Option<String>, _>(14).ok().flatten(),
        request_id: row.try_get::<Option<String>, _>(15).ok().flatten(),
        session_id: row.try_get::<Option<String>, _>(16).ok().flatten(),
        flow_id: row.try_get::<Option<String>, _>(17).ok().flatten(),
        fingerprint: row.get(18),
        client_id: row.get(19),
        token_id: row.get(20),
        delegation_type: row.get(21),
        sdk_name: row.get(22),
        sdk_version: row.get(23),
        created_at_ms: row.get(24),
    }
}

async fn insert_event_row_sql(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    dialect: Dialect,
    event: &BufferedEventRow,
) -> anyhow::Result<()> {
    let insert_sql = match dialect {
        Dialect::Sqlite => {
            "INSERT INTO events (
                id, instance_id, event_type, category, org_id, actor_id, actor_type, aggregate_id,
                aggregate_type, resource_type, payload, metadata, request_id, session_id, flow_id,
                fingerprint, client_id, token_id, delegation_type, sdk_name, sdk_version, created_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8,
                $9, $10, $11, $12, $13, $14, $15,
                $16, $17, $18, $19, $20, $21, datetime($22 / 1000, 'unixepoch')
            )"
        }
        Dialect::Postgres => {
            "INSERT INTO events (
                id, instance_id, event_type, category, org_id, actor_id, actor_type, aggregate_id,
                aggregate_type, resource_type, payload, metadata, request_id, session_id, flow_id,
                fingerprint, client_id, token_id, delegation_type, sdk_name, sdk_version, created_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8,
                $9, $10, $11, $12, $13, $14, $15,
                $16, $17, $18, $19, $20, $21, to_timestamp($22::double precision / 1000.0)
            )"
        }
        Dialect::Spanner => unreachable!("native Spanner uses spanner_insert_event_stmt"),
    };

    sqlx::query(insert_sql)
        .bind(&event.id)
        .bind(&event.instance_id)
        .bind(&event.event_type)
        .bind(&event.category)
        .bind(&event.org_id)
        .bind(event.actor_id.clone())
        .bind(event.actor_type.clone())
        .bind(event.aggregate_id.clone())
        .bind(event.aggregate_type.clone())
        .bind(event.resource_type.clone())
        .bind(&event.payload)
        .bind(&event.metadata)
        .bind(event.request_id.clone())
        .bind(event.session_id.clone())
        .bind(event.flow_id.clone())
        .bind(&event.fingerprint)
        .bind(&event.client_id)
        .bind(&event.token_id)
        .bind(&event.delegation_type)
        .bind(&event.sdk_name)
        .bind(&event.sdk_version)
        .bind(event.created_at_ms)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn spanner_insert_event_stmt(event: &BufferedEventRow) -> Statement {
    let mut stmt = Statement::new(
        "INSERT INTO events (
            id, instance_id, event_type, category, org_id, actor_id, actor_type, aggregate_id,
            aggregate_type, resource_type, payload, metadata, request_id, session_id, flow_id,
            fingerprint, client_id, token_id, delegation_type, sdk_name, sdk_version, created_at
        ) VALUES (
            @id, @instance_id, @event_type, @category, @org_id, @actor_id, @actor_type, @aggregate_id,
            @aggregate_type, @resource_type, @payload, @metadata, @request_id, @session_id, @flow_id,
            @fingerprint, @client_id, @token_id, @delegation_type, @sdk_name, @sdk_version, TIMESTAMP_MILLIS(@created_at_ms)
        )",
    );
    stmt.add_param("id", &event.id);
    stmt.add_param("instance_id", &event.instance_id);
    stmt.add_param("event_type", &event.event_type);
    stmt.add_param("category", &event.category);
    stmt.add_param("org_id", &event.org_id);
    stmt.add_param("actor_id", &event.actor_id.clone().unwrap_or_default());
    stmt.add_param("actor_type", &event.actor_type.clone().unwrap_or_default());
    stmt.add_param(
        "aggregate_id",
        &event.aggregate_id.clone().unwrap_or_default(),
    );
    stmt.add_param(
        "aggregate_type",
        &event.aggregate_type.clone().unwrap_or_default(),
    );
    stmt.add_param(
        "resource_type",
        &event.resource_type.clone().unwrap_or_default(),
    );
    stmt.add_param("payload", &event.payload);
    stmt.add_param("metadata", &event.metadata);
    stmt.add_param("request_id", &event.request_id.clone().unwrap_or_default());
    stmt.add_param("session_id", &event.session_id.clone().unwrap_or_default());
    stmt.add_param("flow_id", &event.flow_id.clone().unwrap_or_default());
    stmt.add_param("fingerprint", &event.fingerprint);
    stmt.add_param("client_id", &event.client_id);
    stmt.add_param("token_id", &event.token_id);
    stmt.add_param("delegation_type", &event.delegation_type);
    stmt.add_param("sdk_name", &event.sdk_name);
    stmt.add_param("sdk_version", &event.sdk_version);
    stmt.add_param("created_at_ms", &event.created_at_ms);
    stmt
}

async fn start_otel_log_pipeline(
    config: &ObservabilityConfig,
    shutdown: watch::Receiver<bool>,
) -> anyhow::Result<(mpsc::UnboundedSender<StructuredRecord>, Vec<JoinHandle<()>>)> {
    let exporter = match config.sinks.otel.protocol.as_str() {
        "grpc" | "tonic" => LogExporter::builder()
            .with_tonic()
            .with_endpoint(config.sinks.otel.endpoint.clone())
            .build()?,
        _ => LogExporter::builder()
            .with_http()
            .with_endpoint(config.sinks.otel.endpoint.clone())
            .with_protocol(Protocol::HttpBinary)
            .build()?,
    };

    let provider = SdkLoggerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(otel_resource())
        .build();
    let logger = provider.logger("zitadel");
    let (tx, mut rx) = mpsc::unbounded_channel();

    let task = tokio::spawn(async move {
        let mut shutdown = shutdown;
        loop {
            tokio::select! {
                _ = shutdown.changed() => break,
                maybe_record = rx.recv() => {
                    let Some(record) = maybe_record else { break; };
                    emit_otel(&logger, &record);
                }
            }
        }
        let _ = provider.shutdown();
    });

    Ok((tx, vec![task]))
}

fn otel_resource() -> Resource {
    Resource::builder()
        .with_attributes([
            KeyValue::new("service.name", SERVICE_NAME),
            KeyValue::new("service.version", VERSION),
        ])
        .build()
}

async fn build_tracer_provider(otel_config: &OtelSinkConfig) -> anyhow::Result<SdkTracerProvider> {
    let endpoint = otel_config.traces_endpoint();
    let exporter = match otel_config.protocol.as_str() {
        "grpc" | "tonic" => SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()?,
        _ => SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_protocol(Protocol::HttpBinary)
            .build()?,
    };
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(otel_resource())
        .build();
    Ok(provider)
}

async fn build_meter_provider(otel_config: &OtelSinkConfig) -> anyhow::Result<SdkMeterProvider> {
    let endpoint = otel_config.metrics_endpoint();
    let interval = parse_duration(&otel_config.metrics_interval);
    let exporter = match otel_config.protocol.as_str() {
        "grpc" | "tonic" => MetricExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()?,
        _ => MetricExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_protocol(Protocol::HttpBinary)
            .build()?,
    };
    let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
        .with_interval(interval)
        .build();
    let provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(otel_resource())
        .build();
    Ok(provider)
}

fn resolve_gcp_project_id(configured: &str) -> Option<String> {
    if !configured.is_empty() {
        return Some(configured.to_string());
    }
    std::env::var("GCP_PROJECT")
        .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT"))
        .or_else(|_| std::env::var("GCLOUD_PROJECT"))
        .ok()
}

fn emit_otel(logger: &SdkLogger, record: &StructuredRecord) {
    let mut otel_record = logger.create_log_record();
    otel_record.set_body(AnyValue::from(record.message.clone()));
    otel_record.set_severity_text(record.severity.name());
    otel_record.set_severity_number(record.severity);
    otel_record.set_target("zitadel");
    otel_record.set_observed_timestamp(SystemTime::now());
    otel_record
        .set_timestamp(UNIX_EPOCH + Duration::from_millis(record.created_at_ms.max(0) as u64));
    otel_record.add_attribute("stream", AnyValue::from(record.stream.to_string()));
    otel_record.add_attribute("event_type", AnyValue::from(record.event_type.clone()));
    otel_record.add_attribute("category", AnyValue::from(record.category.clone()));
    if let Some(request_id) = &record.request_id {
        otel_record.add_attribute("request_id", AnyValue::from(request_id.clone()));
    }
    if let Some(session_id) = &record.session_id {
        otel_record.add_attribute("session_id", AnyValue::from(session_id.clone()));
    }
    if let Some(flow_id) = &record.flow_id {
        otel_record.add_attribute("flow_id", AnyValue::from(flow_id.clone()));
    }
    if let Some(client_id) = &record.client_id {
        otel_record.add_attribute("client_id", AnyValue::from(client_id.clone()));
    }
    if let Some(parent_span_id) = record
        .metadata
        .get("parent_span_id")
        .and_then(value_as_string)
        .and_then(|span_id| SpanId::from_hex(&span_id).ok())
    {
        if let Some(request_id) = &record.request_id
            && let Ok(trace_id) = TraceId::from_hex(request_id)
        {
            otel_record.set_trace_context(trace_id, parent_span_id, Some(TraceFlags::SAMPLED));
        }
    } else if let Some(request_id) = &record.request_id
        && let Ok(trace_id) = TraceId::from_hex(request_id)
    {
        let mut hasher = DefaultHasher::new();
        record.id.hash(&mut hasher);
        otel_record.set_trace_context(
            trace_id,
            SpanId::from(hasher.finish()),
            Some(TraceFlags::SAMPLED),
        );
    }
    otel_record.add_attribute("payload", AnyValue::from(record.payload.to_string()));
    otel_record.add_attribute("metadata", AnyValue::from(record.metadata.to_string()));
    logger.emit(otel_record);
}

fn parse_duration(raw: &str) -> Duration {
    let trimmed = raw.trim();
    if let Some(value) = trimmed.strip_suffix("ms") {
        return Duration::from_millis(value.parse().unwrap_or(100));
    }
    if let Some(value) = trimmed.strip_suffix('s') {
        return Duration::from_secs(value.parse().unwrap_or(5));
    }
    if let Some(value) = trimmed.strip_suffix('m') {
        return Duration::from_secs(value.parse::<u64>().unwrap_or(1) * 60);
    }
    Duration::from_secs(5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stream_aliases() {
        assert_eq!(Stream::from_name("queue"), Some(Stream::Jobs));
        assert_eq!(Stream::from_name("jobs"), Some(Stream::Jobs));
        assert_eq!(
            Stream::from_name("event_handler"),
            Some(Stream::EventHandler)
        );
    }

    #[test]
    fn redacts_ip_values() {
        let redaction = Redaction {
            keys: HashSet::new(),
            mask: "***".into(),
            ip_mode: IpMode::Hash,
        };
        let mut value = json!({ "ip": "203.0.113.5" });
        redact_value(&redaction, None, &mut value);
        assert_ne!(value["ip"], Value::String("203.0.113.5".into()));
    }

    #[tokio::test]
    async fn drains_buffered_records_into_events() {
        let analytics_db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&analytics_db).await.unwrap();
        let cache_path =
            std::env::temp_dir().join(format!("zitadel-observability-{}.db", Uuid::now_v7()));
        let cache_url = cache_path.to_string_lossy().to_string();
        let cache_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}?mode=rwc", cache_url))
            .await
            .unwrap();
        init_cache_schema(&cache_pool).await.unwrap();

        let record = StructuredRecord {
            id: Uuid::now_v7().to_string(),
            created_at_ms: now_ms(),
            level: "info".into(),
            severity: severity_for(&tracing::Level::INFO),
            stream: Stream::Request,
            message: "request served".into(),
            event_type: "request.api".into(),
            category: "request".into(),
            org_id: "0".into(),
            actor_id: None,
            actor_type: None,
            aggregate_id: None,
            aggregate_type: None,
            resource_type: None,
            request_id: Some(Uuid::now_v7().simple().to_string()),
            session_id: None,
            flow_id: None,
            fingerprint: None,
            client_id: None,
            token_id: None,
            delegation_type: None,
            sdk_name: None,
            sdk_version: None,
            payload: json!({"message": "request served", "status": 200}),
            metadata: json!({"stream": "request", "level": "info"}),
        };
        insert_buffered_record(&cache_pool, &record).await.unwrap();
        let drained = drain_once(&cache_pool, &analytics_db, 100).await.unwrap();
        assert_eq!(drained, 1);

        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM events WHERE instance_id = $1 AND event_type = $2",
        )
        .bind(DEFAULT_INSTANCE_ID)
        .bind("request.api")
        .fetch_one(analytics_db.pool())
        .await
        .unwrap();
        assert_eq!(count.0, 1);
        let _ = std::fs::remove_file(cache_path);
    }
}
