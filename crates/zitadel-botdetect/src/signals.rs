//! Server-side request signal analysis for bot detection.
//!
//! Extracts and scores signals from HTTP requests to estimate
//! whether the client is a real browser or an automated tool.

use sha2::{Digest, Sha256};

/// Signals extracted from an HTTP request.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RequestSignals {
    /// Hash of HTTP header key ordering (browsers have predictable patterns).
    pub header_order_hash: String,
    /// Accept-Language header value.
    pub accept_language: String,
    /// Accept-Encoding header value.
    pub accept_encoding: String,
    /// User-Agent header value.
    pub user_agent: String,
    /// Client IP address.
    pub ip_address: String,
    /// HTTP version (1.0, 1.1, 2.0).
    pub http_version: String,
    /// Whether the client presented a valid Apple Private Access Token.
    pub has_private_access_token: bool,
    /// Whether the client IP is in the iCloud Private Relay range.
    pub is_private_relay_ip: bool,
    /// FingerprintJS confidence score (0.0-1.0, if available).
    pub fingerprint_confidence: Option<f64>,
}

/// Risk assessment result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RiskScore {
    /// Overall risk score (0.0 = trusted, 1.0 = definitely a bot).
    pub score: f64,
    /// Names of signals that contributed to the score.
    pub signals: Vec<String>,
    /// Recommended action based on the score.
    pub recommendation: Recommendation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    /// Allow without challenge.
    Allow,
    /// Require POW challenge.
    Challenge,
    /// Block the request.
    Block,
}

/// Score a request based on its signals.
pub fn score_request(signals: &RequestSignals) -> RiskScore {
    let mut score = 0.5_f64; // Start neutral.
    let mut fired: Vec<String> = Vec::new();

    // Positive signals (reduce risk).
    if signals.has_private_access_token {
        score -= 0.5;
        fired.push("apple_pat_verified".into());
    }
    if signals.is_private_relay_ip {
        score -= 0.2;
        fired.push("private_relay_ip".into());
    }
    if let Some(conf) = signals.fingerprint_confidence {
        if conf > 0.9 {
            score -= 0.1;
            fired.push("high_fp_confidence".into());
        } else if conf < 0.5 {
            score += 0.15;
            fired.push("low_fp_confidence".into());
        }
    }

    // Negative signals (increase risk).
    if signals.accept_language.is_empty() {
        score += 0.2;
        fired.push("missing_accept_language".into());
    }
    if signals.accept_encoding.is_empty() {
        score += 0.1;
        fired.push("missing_accept_encoding".into());
    }
    if is_bot_user_agent(&signals.user_agent) {
        score += 0.3;
        fired.push("bot_user_agent".into());
    }
    if signals.http_version == "1.0" {
        score += 0.1;
        fired.push("http_1_0".into());
    }
    // Private Relay IP but non-Apple UA = suspicious spoofing.
    if signals.is_private_relay_ip && !is_apple_user_agent(&signals.user_agent) {
        score += 0.3;
        fired.push("relay_ua_mismatch".into());
    }

    // Clamp to [0, 1].
    score = score.clamp(0.0, 1.0);

    let recommendation = if score < 0.3 {
        Recommendation::Allow
    } else if score < 0.8 {
        Recommendation::Challenge
    } else {
        Recommendation::Block
    };

    RiskScore {
        score,
        signals: fired,
        recommendation,
    }
}

/// Compute a hash of HTTP header key ordering.
/// Different browser engines produce headers in predictable orders.
pub fn hash_header_order(header_keys: &[&str]) -> String {
    let combined = header_keys.join(",");
    let mut hasher = Sha256::new();
    hasher.update(combined.as_bytes());
    hex::encode(&hasher.finalize()[..8]) // 16 hex chars
}

fn is_bot_user_agent(ua: &str) -> bool {
    let ua_lower = ua.to_lowercase();
    let bot_patterns = [
        "bot",
        "crawl",
        "spider",
        "scrape",
        "curl",
        "wget",
        "python-requests",
        "go-http-client",
        "httpie",
        "postman",
        "insomnia",
        "apache-httpclient",
    ];
    bot_patterns.iter().any(|p| ua_lower.contains(p))
}

fn is_apple_user_agent(ua: &str) -> bool {
    let ua_lower = ua.to_lowercase();
    ua_lower.contains("safari")
        || ua_lower.contains("iphone")
        || ua_lower.contains("ipad")
        || ua_lower.contains("mac os")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_browser_gets_neutral_score() {
        let signals = RequestSignals {
            accept_language: "en-US,en;q=0.9".into(),
            accept_encoding: "gzip, deflate, br".into(),
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/125".into(),
            http_version: "2.0".into(),
            ..Default::default()
        };
        let result = score_request(&signals);
        assert!(
            result.score < 0.6,
            "normal browser should score low: {}",
            result.score
        );
        assert_eq!(result.recommendation, Recommendation::Challenge); // neutral gets challenged
    }

    #[test]
    fn apple_device_with_pat_gets_low_score() {
        let signals = RequestSignals {
            accept_language: "en-US".into(),
            accept_encoding: "gzip, deflate, br".into(),
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) Safari/605.1.15".into(),
            http_version: "2.0".into(),
            has_private_access_token: true,
            is_private_relay_ip: true,
            ..Default::default()
        };
        let result = score_request(&signals);
        assert!(
            result.score < 0.3,
            "Apple device with PAT should score very low: {}",
            result.score
        );
        assert_eq!(result.recommendation, Recommendation::Allow);
    }

    #[test]
    fn curl_gets_high_score() {
        let signals = RequestSignals {
            user_agent: "curl/8.5.0".into(),
            http_version: "1.1".into(),
            ..Default::default()
        };
        let result = score_request(&signals);
        assert!(
            result.score > 0.7,
            "curl should score high: {}",
            result.score
        );
    }

    #[test]
    fn relay_ip_with_non_apple_ua_is_suspicious() {
        let signals = RequestSignals {
            accept_language: "en-US".into(),
            accept_encoding: "gzip".into(),
            user_agent: "Mozilla/5.0 (Windows NT 10.0) Chrome/125".into(),
            http_version: "2.0".into(),
            is_private_relay_ip: true,
            ..Default::default()
        };
        let result = score_request(&signals);
        assert!(
            result.signals.contains(&"relay_ua_mismatch".to_string()),
            "should flag relay+non-Apple UA mismatch"
        );
    }

    #[test]
    fn header_order_hash_is_deterministic() {
        let h1 = hash_header_order(&["Host", "Accept", "User-Agent"]);
        let h2 = hash_header_order(&["Host", "Accept", "User-Agent"]);
        assert_eq!(h1, h2);

        let h3 = hash_header_order(&["Accept", "Host", "User-Agent"]);
        assert_ne!(h1, h3, "different order should produce different hash");
    }
}
