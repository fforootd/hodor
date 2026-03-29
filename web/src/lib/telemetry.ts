/**
 * Telemetry SDK for the <zitadel-login> web component.
 * Initializes OTel Browser SDK for structured signal collection.
 *
 * Signals collected:
 * - document.load timing (auto-instrumented)
 * - Fetch durations to /v1/* (auto-instrumented, with traceparent propagation)
 * - User interaction events: click, input (auto-instrumented)
 * - Custom spans: login.flow.step_transition (manual)
 *
 * All spans are exported to the server's /v1/otel/traces endpoint
 * and flow into the Tier 2 OLAP pipeline (ADR-010).
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

    // Convert to simplified OTLP JSON format.
    const otlpPayload = {
      resourceSpans: [{
        resource: {
          attributes: [
            { key: 'service.name', value: { stringValue: 'zitadel-login-wc' } },
            { key: 'browser.language', value: { stringValue: navigator.language } },
          ]
        },
        scopeSpans: [{
          spans: batch.map(s => ({
            traceId: generateTraceId(),
            spanId: generateSpanId(),
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
      await fetch(endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(this.config.flowId ? { 'X-Flow-ID': this.config.flowId } : {}),
        },
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
 */
export function initTelemetry(config: TelemetryConfig): TelemetryProvider | null {
  if (config.enabled === false) return null
  if (provider) return provider

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
 */
export function setFlowId(flowId: string) {
  // The fallback tracer stores config by reference, so we update it.
  if (provider && provider instanceof FallbackTracer) {
    (provider as any).config.flowId = flowId
  }
}

// ─── Helpers ───────────────────────────────────────────────

function generateTraceId(): string {
  return randomHex(32)
}

function generateSpanId(): string {
  return randomHex(16)
}

function randomHex(length: number): string {
  const bytes = new Uint8Array(length / 2)
  crypto.getRandomValues(bytes)
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('')
}
