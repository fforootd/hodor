/**
 * Shared date/time formatting utilities.
 * Replaces per-view formatDate/formatTime functions.
 */

/** Short date: "Mar 30, 2026" */
export function formatDate(ts?: string): string {
  if (!ts) return '—'
  try {
    return new Date(ts).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    })
  } catch {
    return ts
  }
}

/** Full date + time: "3/30/2026, 6:24:33 AM" */
export function formatDateTime(ts?: string): string {
  if (!ts) return '—'
  try {
    return new Date(ts).toLocaleString()
  } catch {
    return ts
  }
}

/** Time only: "06:24" */
export function formatTime(ts?: string): string {
  if (!ts) return '—'
  try {
    return new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  } catch {
    return ts
  }
}

/** "display_name" → "Display Name" */
export function formatKey(key: string): string {
  return key.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
}

/** Format any value for display */
export function formatValue(val: unknown): string {
  if (val === null || val === undefined) return '—'
  if (typeof val === 'object') return JSON.stringify(val)
  return String(val)
}
