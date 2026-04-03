//! Bot detection and proof-of-work challenge system.
//!
//! Multi-layered approach inspired by Cloudflare Turnstile:
//! - Client-side fingerprinting (FingerprintJS OSS v5)
//! - Server-side request signal analysis (header patterns, UA, IP)
//! - Stateless ALTCHA-style proof-of-work challenges
//! - Apple Private Access Tokens for device attestation (future)
//! - iCloud Private Relay IP detection (future)

pub mod pow;
pub mod signals;

pub use pow::{Challenge, Difficulty, Solution, generate_challenge, verify_solution};
pub use signals::{Recommendation, RequestSignals, RiskScore, score_request};
