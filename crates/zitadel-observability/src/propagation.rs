use opentelemetry::{global, propagation::Injector};

/// Collects injected header key-value pairs.
struct HeaderCollector(Vec<(String, String)>);

impl Injector for HeaderCollector {
    fn set(&mut self, key: &str, value: String) {
        self.0.push((key.to_string(), value));
    }
}

/// Returns the current OTel trace context as header key-value pairs (`traceparent`, `tracestate`).
///
/// Callers apply these to outbound HTTP requests for distributed trace propagation.
/// Returns an empty vec if no propagator is registered or no active span exists.
pub fn trace_context_headers() -> Vec<(String, String)> {
    let mut collector = HeaderCollector(Vec::new());
    global::get_text_map_propagator(|propagator| {
        let cx = opentelemetry::Context::current();
        propagator.inject_context(&cx, &mut collector);
    });
    collector.0
}
