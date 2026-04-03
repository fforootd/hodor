//! ALTCHA-style stateless proof-of-work challenge.
//!
//! Protocol:
//! 1. Server generates a challenge: random `salt`, random `secret_number` in [0, max_number],
//!    `challenge = SHA256(salt + secret_number)`, `signature = HMAC-SHA256(key, challenge)`.
//! 2. Client receives `{algorithm, salt, challenge, maxnumber, signature}`.
//! 3. Client iterates nonce from 0 to maxnumber until `SHA256(salt + nonce) == challenge`.
//! 4. Client submits `{salt, nonce, signature}`.
//! 5. Server verifies: check HMAC signature, recompute hash, compare. 3 hash ops, <1ms.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Difficulty tiers for adaptive POW challenges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    /// ~10ms on desktop, ~50ms on mobile.
    Low,
    /// ~100ms on desktop, ~500ms on mobile.
    Medium,
    /// ~400ms on desktop, ~2s on mobile.
    High,
    /// ~1s on desktop, ~5s on mobile.
    Critical,
}

impl Difficulty {
    pub fn max_number(self) -> u64 {
        match self {
            Difficulty::Low => 100_000,
            Difficulty::Medium => 500_000,
            Difficulty::High => 2_000_000,
            Difficulty::Critical => 5_000_000,
        }
    }

    /// Select difficulty from a risk score (0.0 to 1.0).
    pub fn from_risk_score(score: f64) -> Self {
        if score < 0.3 {
            Difficulty::Low
        } else if score < 0.6 {
            Difficulty::Medium
        } else if score < 0.8 {
            Difficulty::High
        } else {
            Difficulty::Critical
        }
    }
}

/// A generated POW challenge to send to the client.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Challenge {
    pub algorithm: String,
    pub salt: String,
    pub challenge: String,
    pub maxnumber: u64,
    pub signature: String,
}

/// A solution submitted by the client.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Solution {
    pub salt: String,
    pub nonce: u64,
    pub signature: String,
}

/// Generate a new POW challenge.
///
/// The `secret_key` is the server's HMAC key (e.g. from config).
/// Returns a challenge that the client must solve.
pub fn generate_challenge(secret_key: &[u8], difficulty: Difficulty) -> Challenge {
    let max_number = difficulty.max_number();

    // Random salt (16 hex chars = 8 bytes of entropy).
    let salt = random_hex(8);

    // Random secret number the client must find.
    let secret_number: u64 = rand::random::<u64>() % max_number;

    // Challenge hash: SHA256(salt + secret_number).
    let challenge_hash = sha256_hex(&format!("{salt}{secret_number}"));

    // HMAC signature to prevent challenge forgery.
    let signature = hmac_sha256_hex(secret_key, challenge_hash.as_bytes());

    Challenge {
        algorithm: "SHA-256".into(),
        salt,
        challenge: challenge_hash,
        maxnumber: max_number,
        signature,
    }
}

/// Verify a client-submitted POW solution.
///
/// Returns `true` if:
/// 1. The HMAC signature is valid (challenge wasn't forged).
/// 2. SHA256(salt + nonce) matches the original challenge hash.
pub fn verify_solution(secret_key: &[u8], solution: &Solution) -> bool {
    // Recompute the challenge hash from the submitted salt + nonce.
    let computed_hash = sha256_hex(&format!("{}{}", solution.salt, solution.nonce));

    // Verify the HMAC signature matches the computed hash.
    let expected_signature = hmac_sha256_hex(secret_key, computed_hash.as_bytes());
    if expected_signature != solution.signature {
        tracing::debug!("POW verification failed: HMAC mismatch");
        return false;
    }

    true
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn hmac_sha256_hex(key: &[u8], data: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

fn random_hex(byte_len: usize) -> String {
    let mut bytes = vec![0u8; byte_len];
    rand::fill(bytes.as_mut_slice());
    hex::encode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &[u8] = b"test-secret-key-for-pow";

    #[test]
    fn challenge_roundtrip() {
        let challenge = generate_challenge(TEST_KEY, Difficulty::Low);
        assert_eq!(challenge.algorithm, "SHA-256");
        assert!(!challenge.salt.is_empty());
        assert!(!challenge.challenge.is_empty());
        assert!(!challenge.signature.is_empty());
        assert_eq!(challenge.maxnumber, 100_000);
    }

    #[test]
    fn solve_and_verify() {
        let challenge = generate_challenge(TEST_KEY, Difficulty::Low);

        // Simulate the client solving the challenge.
        let mut nonce = 0u64;
        loop {
            let hash = sha256_hex(&format!("{}{nonce}", challenge.salt));
            if hash == challenge.challenge {
                break;
            }
            nonce += 1;
            assert!(
                nonce <= challenge.maxnumber,
                "failed to solve within max_number"
            );
        }

        let solution = Solution {
            salt: challenge.salt.clone(),
            nonce,
            signature: challenge.signature.clone(),
        };

        assert!(verify_solution(TEST_KEY, &solution));
    }

    #[test]
    fn wrong_nonce_fails() {
        let challenge = generate_challenge(TEST_KEY, Difficulty::Low);

        // Submit a wrong nonce with the original signature.
        let solution = Solution {
            salt: challenge.salt,
            nonce: u64::MAX, // definitely wrong
            signature: challenge.signature,
        };

        assert!(!verify_solution(TEST_KEY, &solution));
    }

    #[test]
    fn forged_signature_fails() {
        let challenge = generate_challenge(TEST_KEY, Difficulty::Low);

        let solution = Solution {
            salt: challenge.salt,
            nonce: 0,
            signature: "forged_signature".into(),
        };

        assert!(!verify_solution(TEST_KEY, &solution));
    }

    #[test]
    fn difficulty_from_risk_score() {
        assert_eq!(Difficulty::from_risk_score(0.1), Difficulty::Low);
        assert_eq!(Difficulty::from_risk_score(0.4), Difficulty::Medium);
        assert_eq!(Difficulty::from_risk_score(0.7), Difficulty::High);
        assert_eq!(Difficulty::from_risk_score(0.9), Difficulty::Critical);
    }
}
