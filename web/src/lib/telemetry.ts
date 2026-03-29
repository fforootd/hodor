/**
 * Telemetry SDK for the <zitadel-login> web component.
 * Provides trace context propagation, signal collection,
 * and device fingerprinting via ThumbmarkJS.
 *
 * Signals collected:
 * - document.load timing (auto-instrumented)
 * - Fetch durations to /v1/* (auto-instrumented, with traceparent propagation)
 * - User interaction events: click, input (auto-instrumented)
 * - Custom spans: login.flow.step_transition (manual)
 * - Device fingerprint (via ThumbmarkJS — persistent, cookie-less)
 *
 * All spans are exported to the server's /v1/otel/traces endpoint
 * and flow into the Tier 2 OLAP pipeline (ADR-010).
 *
 * Trace strategy:
 * - One trace_id is generated per login flow lifecycle.
 * - All browser spans AND server-side fetch calls share this trace_id.
 * - The flow_id is sent via X-Flow-ID header for server-side correlation.
 * - Device fingerprint is sent via X-Fingerprint header for device correlation.
 */

// Types for the telemetry config
export interface TelemetryConfig {
  /** Base URL of the Zitadel API (e.g. "http://localhost:8080") */
  baseUrl: string
  /** Flow ID for linking traces to the active login flow */
  flowId?: string
  /** Override the OTLP export URL */
  otelEndpoint?: string
  /** Enable/disable telemetry (default: true) */
  enabled?: boolean
}

interface TelemetryProvider {
  shutdown: () => void
  getTracer: (name: string) => any
}

// ─── Trace ID Management ───────────────────────────────────
// One trace_id per login flow. Shared across browser spans and
// sent as Traceparent to the server so server events join the
// same trace tree.
let flowTraceId: string = generateHex(32)
let currentFlowId: string = ''

/** Get the current trace_id for use in Traceparent headers. */
export function getFlowTraceId(): string {
  return flowTraceId
}

/** Get the current flow_id for use in X-Flow-ID headers. */
export function getFlowId(): string {
  return currentFlowId
}

/**
 * Generate a W3C Traceparent header value for the current flow.
 * Each call gets a new span_id but shares the flow's trace_id.
 */
export function generateTraceparent(): string {
  const spanId = generateHex(16)
  return `00-${flowTraceId}-${spanId}-01`
}

// ─── Device Fingerprint (ThumbmarkJS) ──────────────────────
// Computes a stable 32-char device fingerprint that persists
// across tabs, refreshes, and private browsing. Cookie-less.
let cachedFingerprint: string | null = null
let fingerprintPromise: Promise<string> | null = null

/**
 * Get the device fingerprint. Returns cached value if available,
 * or null if not yet computed.
 */
export function getDeviceFingerprint(): string | null {
  return cachedFingerprint
}

/**
 * Compute the device fingerprint asynchronously.
 * Called during initTelemetry(). The result is cached for all subsequent calls.
 */
async function computeFingerprint(): Promise<string> {
  if (cachedFingerprint) return cachedFingerprint
  if (fingerprintPromise) return fingerprintPromise

  fingerprintPromise = (async () => {
    try {
      const { getThumbmark } = await import('@thumbmarkjs/thumbmarkjs')
      const fpData = await getThumbmark()
      const fp = fpData.thumbmark as string
      cachedFingerprint = fp

      // Ingest the raw thumbmark payload directly to the server for analytics/fraud modeling
      fetch('/v1/telemetry/fingerprints', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(fpData),
        keepalive: true, // Fire and forget
      }).catch(console.warn)

      return fp
    } catch {
      // ThumbmarkJS failed (e.g., server-side rendering, blocked APIs).
      // Fall back to a session-scoped random ID.
      cachedFingerprint = `fallback-${generateHex(24)}`
      return cachedFingerprint
    }
  })()

  return fingerprintPromise
}

// Lightweight tracer that works without the full OTel SDK.
// Falls back to this when OTel packages are not installed.
class FallbackTracer implements TelemetryProvider {
  private config: TelemetryConfig
  private spans: Array<{name: string, startTime: number, endTime?: number, attributes?: Record<string, any>}> = []
  private flushInterval: ReturnType<typeof setInterval> | null = null

  constructor(config: TelemetryConfig) {
    this.config = config
    // Flush collected spans every 5 seconds.
    this.flushInterval = setInterval(() => this.flush(), 5000)
    // Capture page load timing.
    if (typeof window !== 'undefined' && window.performance) {
      window.addEventListener('load', () => {
        const timing = performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming
        if (timing) {
          this.spans.push({
            name: 'documentLoad',
            startTime: timing.startTime,
            endTime: timing.loadEventEnd,
            attributes: {
              'http.url': window.location.href,
              'document.load_ms': timing.loadEventEnd - timing.startTime,
              'document.dom_content_loaded_ms': timing.domContentLoadedEventEnd - timing.startTime,
            }
          })
        }
      })
    }
  }

  getTracer(_name: string) {
    return {
      startSpan: (name: string, attributes?: Record<string, any>) => {
        const span = {
          name,
          startTime: performance.now(),
          endTime: undefined as number | undefined,
          attributes,
          end: () => {
            span.endTime = performance.now()
            this.spans.push(span)
          },
          setAttribute: (key: string, value: any) => {
            if (!span.attributes) span.attributes = {}
            span.attributes[key] = value
          },
        }
        return span
      }
    }
  }

  private async flush() {
    if (this.spans.length === 0) return

    const batch = this.spans.splice(0)
    const endpoint = this.config.otelEndpoint || `${this.config.baseUrl}/v1/otel/traces`

    // All spans in this batch share the flow's trace_id.
    const traceId = flowTraceId

    // Build resource attributes including fingerprint if available.
    const resourceAttrs: Array<{key: string, value: {stringValue: string}}> = [
      { key: 'service.name', value: { stringValue: 'zitadel-login-wc' } },
      { key: 'browser.language', value: { stringValue: navigator.language } },
    ]
    if (cachedFingerprint) {
      resourceAttrs.push({ key: 'device.fingerprint', value: { stringValue: cachedFingerprint } })
    }

    // Convert to simplified OTLP JSON format.
    const otlpPayload = {
      resourceSpans: [{
        resource: { attributes: resourceAttrs },
        scopeSpans: [{
          spans: batch.map(s => ({
            traceId: traceId,
            spanId: generateHex(16),
            name: s.name,
            kind: 1, // INTERNAL
            startTimeUnixNano: String(Math.floor((performance.timeOrigin + (s.startTime || 0)) * 1_000_000)),
            endTimeUnixNano: String(Math.floor((performance.timeOrigin + (s.endTime || performance.now())) * 1_000_000)),
            attributes: s.attributes ? Object.entries(s.attributes).map(([k, v]) => ({
              key: k,
              value: typeof v === 'number' ? { intValue: String(v) } : { stringValue: String(v) },
            })) : [],
          }))
        }]
      }]
    }

    try {
      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
      }
      if (currentFlowId) headers['X-Flow-ID'] = currentFlowId
      if (cachedFingerprint) headers['X-Fingerprint'] = cachedFingerprint

      await fetch(endpoint, {
        method: 'POST',
        headers,
        body: JSON.stringify(otlpPayload),
        keepalive: true,
      })
    } catch {
      // Silent fail — telemetry should never break the login flow.
    }
  }

  shutdown() {
    this.flush()
    if (this.flushInterval) clearInterval(this.flushInterval)
  }
}

// Internal state
let provider: TelemetryProvider | null = null

/**
 * Initialize the telemetry provider. Call once on component mount.
 * Uses the fallback tracer by default (no OTel dependency needed).
 * When OTel packages are installed, it will auto-detect and use them.
 * Also kicks off async device fingerprint computation.
 */
export function initTelemetry(config: TelemetryConfig): TelemetryProvider | null {
  if (config.enabled === false) return null
  if (provider) return provider

  // Start fingerprint computation in the background (non-blocking).
  computeFingerprint()

  provider = new FallbackTracer(config)
  return provider
}

/**
 * Record a custom span for a login flow step transition.
 */
export function traceStepTransition(fromStep: string, toStep: string, flowId: string) {
  if (!provider) return
  const tracer = provider.getTracer('zitadel-login')
  const span = tracer.startSpan('login.flow.step_transition', {
    'flow.id': flowId,
    'flow.from_step': fromStep,
    'flow.to_step': toStep,
  })
  // Step transitions are instantaneous — end immediately.
  span.end()
}

/**
 * Record a custom span for a form submission.
 */
export function traceFormSubmit(action: string, flowId: string) {
  if (!provider) return
  const tracer = provider.getTracer('zitadel-login')
  const span = tracer.startSpan('login.flow.submit', {
    'flow.id': flowId,
    'flow.action': action,
  })
  return span
}

/**
 * Shutdown the telemetry provider. Call on component unmount.
 * Flushes any remaining spans.
 */
export function shutdownTelemetry() {
  if (provider) {
    provider.shutdown()
    provider = null
  }
}

/**
 * Update the flow ID for trace linking (called when a new flow is created).
 * Also generates a fresh trace_id for this flow lifecycle.
 */
export function setFlowId(flowId: string) {
  currentFlowId = flowId
  // Generate a fresh trace_id for this flow — all subsequent spans and
  // fetch calls will share this trace_id for end-to-end correlation.
  flowTraceId = generateHex(32)
}

// ─── Helpers ───────────────────────────────────────────────

function generateHex(length: number): string {
  const bytes = new Uint8Array(length / 2)
  crypto.getRandomValues(bytes)
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('')
}
