use std::time::Instant;

use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use tracing::{Instrument, info_span};
use uuid::Uuid;

use crate::Stream;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

pub async fn request_context_middleware(mut req: Request<Body>, next: Next) -> Response {
    let Some(event_type) = classify_request(req.uri().path()) else {
        return next.run(req).await;
    };

    let start = Instant::now();
    let (request_id, parent_span_id) = extract_request_context(&req);
    let fingerprint = header_value(&req, "x-fingerprint");
    let flow_id = header_value(&req, "x-flow-id");
    let session_id = header_value(&req, "x-session-id");
    let sdk_name = header_value(&req, "x-sdk-name");
    let sdk_version = header_value(&req, "x-sdk-version");
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let protocol = if req.version() == axum::http::Version::HTTP_2 {
        "http2"
    } else {
        "http"
    };

    req.extensions_mut().insert(RequestContext {
        request_id: request_id.clone(),
    });

    let span = info_span!(
        "request",
        stream = Stream::Request.as_str(),
        request_id = request_id.as_str(),
        parent_span_id = parent_span_id.as_deref().unwrap_or(""),
        fingerprint = fingerprint.as_deref().unwrap_or(""),
        flow_id = flow_id.as_deref().unwrap_or(""),
        session_id = session_id.as_deref().unwrap_or(""),
        sdk_name = sdk_name.as_deref().unwrap_or(""),
        sdk_version = sdk_version.as_deref().unwrap_or(""),
        http_method = method.as_str(),
        path = path.as_str(),
        protocol,
        actor_id = tracing::field::Empty,
        org_id = tracing::field::Empty,
        client_id = tracing::field::Empty,
        token_id = tracing::field::Empty,
        delegation_type = tracing::field::Empty,
        status = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
    );

    let mut response = next.run(req).instrument(span.clone()).await;
    span.record("status", response.status().as_u16());
    span.record("duration_ms", start.elapsed().as_millis() as u64);
    tracing::info!(
        parent: &span,
        event_type = event_type,
        "request served"
    );

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }
    response
}

#[derive(Clone, Debug)]
pub struct RequestContext {
    pub request_id: String,
}

pub fn classify_request(path: &str) -> Option<&'static str> {
    if path == "/healthz" || path == "/readyz" {
        return None;
    }
    if path.starts_with("/v1/auth/sso/")
        || path.starts_with("/login")
        || path.starts_with("/conformance/login")
    {
        return Some("request.login");
    }
    if path.starts_with("/v1/") {
        return Some("request.api");
    }
    if path.starts_with("/.well-known/")
        || path.starts_with("/oauth/")
        || path.starts_with("/oidc/")
        || path == "/authorize"
        || path == "/token"
        || path == "/userinfo"
        || path == "/jwks"
    {
        return Some("request.oidc");
    }
    None
}

fn extract_request_context(req: &Request<Body>) -> (String, Option<String>) {
    if let Some(traceparent) = req
        .headers()
        .get("traceparent")
        .and_then(|v| v.to_str().ok())
    {
        if let Some((trace_id, span_id)) = parse_traceparent(traceparent) {
            return (trace_id, Some(span_id));
        }
    }
    (Uuid::now_v7().simple().to_string(), None)
}

fn parse_traceparent(value: &str) -> Option<(String, String)> {
    let mut parts = value.split('-');
    let _version = parts.next()?;
    let trace_id = parts.next()?;
    let parent_span_id = parts.next()?;
    let _flags = parts.next()?;
    if parts.next().is_some() || trace_id.len() != 32 || parent_span_id.len() != 16 {
        return None;
    }
    Some((trace_id.to_string(), parent_span_id.to_string()))
}

fn header_value(req: &Request<Body>, name: &str) -> Option<String> {
    req.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_request_families() {
        assert_eq!(classify_request("/v1/users"), Some("request.api"));
        assert_eq!(classify_request("/login"), Some("request.login"));
        assert_eq!(
            classify_request("/.well-known/openid-configuration"),
            Some("request.oidc")
        );
        assert_eq!(classify_request("/healthz"), None);
    }

    #[test]
    fn parses_w3c_traceparent() {
        let parsed =
            parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01").unwrap();
        assert_eq!(parsed.0, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(parsed.1, "00f067aa0ba902b7");
    }
}
