/**
 * Proof-of-Work solver for ALTCHA-style challenges.
 *
 * Uses the WebCrypto API (crypto.subtle.digest) for SHA-256 hashing.
 * Runs in the main thread with yielding to keep the UI responsive.
 *
 * Protocol:
 * 1. Receive {salt, challenge, maxnumber} from server
 * 2. Iterate nonce from 0 to maxnumber
 * 3. Compute SHA256(salt + nonce) and compare to challenge
 * 4. Return {nonce, took_ms} when found
 */

export interface PowChallenge {
  algorithm: string
  salt: string
  challenge: string
  maxnumber: number
  signature: string
}

export interface PowSolution {
  salt: string
  nonce: number
  signature: string
}

/**
 * Solve a POW challenge by finding the nonce.
 *
 * Returns the solution with the original signature for server verification.
 * Yields to the event loop every 10K iterations to keep UI responsive.
 */
export async function solveChallenge(challenge: PowChallenge): Promise<PowSolution & { took_ms: number }> {
  const start = performance.now()
  const encoder = new TextEncoder()

  for (let nonce = 0; nonce <= challenge.maxnumber; nonce++) {
    const input = `${challenge.salt}${nonce}`
    const data = encoder.encode(input)
    const hashBuffer = await crypto.subtle.digest('SHA-256', data)
    const hashHex = bufferToHex(hashBuffer)

    if (hashHex === challenge.challenge) {
      return {
        salt: challenge.salt,
        nonce,
        signature: challenge.signature,
        took_ms: Math.round(performance.now() - start),
      }
    }

    // Yield to event loop every 10K iterations to keep UI responsive.
    if (nonce % 10_000 === 0 && nonce > 0) {
      await new Promise(resolve => setTimeout(resolve, 0))
    }
  }

  throw new Error('POW challenge: no solution found within maxnumber')
}

function bufferToHex(buffer: ArrayBuffer): string {
  return Array.from(new Uint8Array(buffer))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('')
}
