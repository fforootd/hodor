/**
 * Browser fingerprinting via FingerprintJS OSS v5.
 *
 * Collects a persistent visitor ID that survives:
 * - Tab switches
 * - Private/incognito mode (most browsers)
 * - Cookie clears
 *
 * The fingerprint is based on 42 hardware signals (canvas, WebGL, audio, etc.)
 * and does NOT use cookies or localStorage.
 *
 * GDPR/HIPAA/CCPA compliant — no PII is collected.
 */

export interface FingerprintResult {
  /** Stable hex fingerprint hash — persistent across sessions */
  visitorId: string
  /** Individual component signals used to compute the fingerprint */
  components: Record<string, unknown>
  /** Confidence score (0-1, FingerprintJS only) */
  confidence: number
  /** Timestamp when the fingerprint was collected */
  collectedAt: number
}

/**
 * Collect a browser fingerprint using FingerprintJS OSS v5.
 *
 * Falls back to a lightweight built-in implementation if FingerprintJS
 * fails to load (e.g. blocked by content policy, SSR).
 */
export async function collectFingerprint(): Promise<FingerprintResult> {
  try {
    const FingerprintJS = await import('@fingerprintjs/fingerprintjs')
    const fp = await FingerprintJS.load({ monitoring: false })
    const result = await fp.get()

    return {
      visitorId: result.visitorId,
      components: result.components as Record<string, unknown>,
      confidence: result.confidence.score,
      collectedAt: Date.now(),
    }
  } catch {
    // FingerprintJS failed — use fallback.
    return collectFallbackFingerprint()
  }
}

/**
 * Lightweight fallback fingerprint using browser-native APIs.
 * Less accurate than FingerprintJS but works without the library.
 */
async function collectFallbackFingerprint(): Promise<FingerprintResult> {
  const components: Record<string, unknown> = {}

  // Canvas fingerprint.
  try {
    const canvas = document.createElement('canvas')
    canvas.width = 200
    canvas.height = 50
    const ctx = canvas.getContext('2d')
    if (ctx) {
      ctx.textBaseline = 'top'
      ctx.font = '14px Arial'
      ctx.fillStyle = '#f60'
      ctx.fillRect(100, 1, 62, 20)
      ctx.fillStyle = '#069'
      ctx.fillText('Zitadel fp', 2, 15)
      ctx.fillStyle = 'rgba(102, 204, 0, 0.7)'
      ctx.fillText('canvas fp', 4, 35)
      components.canvas = { value: await hashString(canvas.toDataURL()) }
    }
  } catch {
    components.canvas = { error: 'unavailable' }
  }

  // WebGL renderer.
  try {
    const canvas = document.createElement('canvas')
    const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl')
    if (gl && gl instanceof WebGLRenderingContext) {
      const dbg = gl.getExtension('WEBGL_debug_renderer_info')
      if (dbg) {
        components.webGlBasics = {
          value: {
            vendor: gl.getParameter(dbg.UNMASKED_VENDOR_WEBGL) || '',
            renderer: gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) || '',
          },
        }
      }
    }
  } catch {
    components.webGlBasics = { error: 'unavailable' }
  }

  // Screen.
  components.screenResolution = { value: [screen.width, screen.height] }
  components.colorDepth = { value: screen.colorDepth }

  // Timezone.
  components.timezone = { value: Intl.DateTimeFormat().resolvedOptions().timeZone || '' }

  // Language.
  components.languages = { value: [[navigator.language]] }

  // Platform.
  components.platform = { value: navigator.platform || '' }

  // Hardware concurrency.
  components.hardwareConcurrency = { value: navigator.hardwareConcurrency || 0 }

  // Device memory (if available).
  components.deviceMemory = { value: (navigator as any).deviceMemory || 0 }

  // Compute composite hash.
  const combined = Object.entries(components)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([k, v]) => `${k}:${JSON.stringify(v)}`)
    .join('|')
  const visitorId = await hashString(combined)

  return { visitorId, components, confidence: 0.4, collectedAt: Date.now() }
}

/**
 * SHA-256 hash a string and return a hex-encoded result.
 */
async function hashString(input: string): Promise<string> {
  const encoder = new TextEncoder()
  const data = encoder.encode(input)
  const hash = await crypto.subtle.digest('SHA-256', data)
  return Array.from(new Uint8Array(hash))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('')
}

/**
 * Submit the fingerprint to the login flow engine.
 */
export async function submitFingerprint(
  baseUrl: string,
  flowId: string,
  fingerprint: FingerprintResult,
): Promise<void> {
  try {
    await fetch(`${baseUrl}/v1/login/flows/${flowId}/submit`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'include',
      body: JSON.stringify({
        action: 'fingerprint_submit',
        visitor_id: fingerprint.visitorId,
        fingerprint_hash: fingerprint.visitorId,
      }),
    })
  } catch {
    // Silent fail — fingerprint collection should never block login.
  }
}

/**
 * Upload the full fingerprint context to the telemetry endpoint.
 * Called after login flow fingerprint submission succeeds.
 */
export function uploadFingerprintContext(
  baseUrl: string,
  fingerprint: FingerprintResult,
): void {
  try {
    fetch(`${baseUrl}/v1/telemetry/fingerprints`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      credentials: 'include',
      body: JSON.stringify({
        id: fingerprint.visitorId,
        type: 'fingerprintjs',
        raw_data: {
          visitorId: fingerprint.visitorId,
          components: fingerprint.components,
          confidence: { score: fingerprint.confidence },
          collectedAt: fingerprint.collectedAt,
        },
      }),
      keepalive: true,
    }).catch(() => {})
  } catch {
    // Silent fail — telemetry should never block login.
  }
}
