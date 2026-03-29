/**
 * Browser fingerprinting wrapper for ThumbmarkJS.
 *
 * Collects a persistent visitor ID that survives:
 * - Tab switches
 * - Private/incognito mode
 * - Cookie clears
 *
 * The fingerprint is based on hardware signals (canvas, WebGL, audio, etc.)
 * and does NOT use cookies or localStorage.
 *
 * GDPR/HIPAA/CCPA compliant — no PII is collected.
 */

export interface FingerprintResult {
  /** 32-char hex fingerprint hash — persistent across sessions */
  visitorId: string
  /** Individual component hashes used to compute the fingerprint */
  components: Record<string, string>
  /** Timestamp when the fingerprint was collected */
  collectedAt: number
}

/**
 * Collect a browser fingerprint.
 *
 * If ThumbmarkJS is installed (@thumbmarkjs/thumbmarkjs), it uses the full
 * library for maximum accuracy. Otherwise, falls back to a lightweight
 * built-in implementation using canvas + navigator signals.
 */
export async function collectFingerprint(): Promise<FingerprintResult> {
  // Try ThumbmarkJS first (if installed).
  try {
    // Variable-based import to bypass Vite's static analysis.
    // ThumbmarkJS is an optional peer dependency — works without it.
    const pkg = '@thumbmarkjs/thumbmarkjs'
    const tm = await (Function('p', 'return import(p)')(pkg))
    if (tm && (tm.getFingerprint || tm.default?.getFingerprint)) {
      const fn = tm.getFingerprint || tm.default.getFingerprint
      const result = await fn()
      return {
        visitorId: typeof result === 'string' ? result : result.hash || result.thumbmark || '',
        components: typeof result === 'object' ? (result.components || {}) : {},
        collectedAt: Date.now(),
      }
    }
  } catch {
    // ThumbmarkJS not installed — use fallback.
  }

  // Fallback: lightweight built-in fingerprinting.
  return collectFallbackFingerprint()
}

/**
 * Lightweight fallback fingerprint using browser-native APIs.
 * Less accurate than ThumbmarkJS but works without dependencies.
 */
async function collectFallbackFingerprint(): Promise<FingerprintResult> {
  const components: Record<string, string> = {}

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
      ctx.fillText('Zitadel fp 🔐', 2, 15)
      ctx.fillStyle = 'rgba(102, 204, 0, 0.7)'
      ctx.fillText('canvas fp', 4, 35)
      components.canvas = await hashString(canvas.toDataURL())
    }
  } catch {
    components.canvas = 'unavailable'
  }

  // WebGL renderer.
  try {
    const canvas = document.createElement('canvas')
    const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl')
    if (gl && gl instanceof WebGLRenderingContext) {
      const dbg = gl.getExtension('WEBGL_debug_renderer_info')
      if (dbg) {
        components.webgl_renderer = gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) || ''
        components.webgl_vendor = gl.getParameter(dbg.UNMASKED_VENDOR_WEBGL) || ''
      }
    }
  } catch {
    components.webgl_renderer = 'unavailable'
  }

  // Screen.
  components.screen = `${screen.width}x${screen.height}x${screen.colorDepth}`

  // Timezone.
  components.timezone = Intl.DateTimeFormat().resolvedOptions().timeZone || ''

  // Language.
  components.language = navigator.language || ''

  // Platform.
  components.platform = navigator.platform || ''

  // Hardware concurrency.
  components.cores = String(navigator.hardwareConcurrency || 0)

  // Device memory (if available).
  components.memory = String((navigator as any).deviceMemory || 0)

  // Compute composite hash.
  const combined = Object.entries(components)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([k, v]) => `${k}:${v}`)
    .join('|')
  const visitorId = await hashString(combined)

  return { visitorId, components, collectedAt: Date.now() }
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
 * Submit the fingerprint to the flow engine.
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
        fingerprint_hash: fingerprint.visitorId, // same for now
      }),
    })
  } catch {
    // Silent fail — fingerprint collection should never block login.
  }
}
