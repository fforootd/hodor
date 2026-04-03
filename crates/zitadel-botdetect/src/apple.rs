//! Apple device attestation and iCloud Private Relay detection.
//!
//! ## Private Access Tokens (RFC 9576/9577)
//!
//! Apple devices with Safari 16+/iOS 16+ support Private Access Tokens
//! which prove the device is real via Secure Enclave attestation without
//! revealing the user's identity. The protocol:
//!
//! 1. Server sends `HTTP 401` with `WWW-Authenticate: PrivateToken challenge=<b64>, token-key=<b64>`
//! 2. Client contacts Apple attester → verifies device via Secure Enclave
//! 3. Token issuer generates blind-signed RSA token
//! 4. Client returns `Authorization: PrivateToken token=<b64>`
//! 5. Server verifies token against issuer's public key
//!
//! Full verification requires RSA blind signatures (RFC 9474) which is complex.
//! For now we detect presence of the PrivateToken header as a positive signal.
//!
//! ## iCloud Private Relay IP Detection
//!
//! Apple publishes the IP ranges used by Private Relay egress nodes.
//! Traffic from these IPs indicates a real Apple device with iCloud+.

use std::net::IpAddr;

/// Check if an HTTP Authorization header contains a Private Access Token.
///
/// This is a lightweight presence check — full token verification requires
/// fetching the issuer's public key and verifying the RSA blind signature.
pub fn has_private_access_token(authorization: Option<&str>) -> bool {
    authorization
        .map(|v| v.starts_with("PrivateToken "))
        .unwrap_or(false)
}

/// Known iCloud Private Relay egress IP ranges (IPv4 CIDRs).
///
/// Apple publishes these at:
/// <https://mask-api.icloud.com/egress-ip-ranges.csv>
///
/// This is a static subset for initial implementation. In production,
/// this list should be fetched and cached periodically.
const PRIVATE_RELAY_IPV4_RANGES: &[(&str, u8)] = &[
    // Apple Private Relay egress ranges (subset — commonly seen ranges)
    ("104.28.0.0", 15),
    ("172.224.0.0", 13),
    ("162.158.0.0", 15),
    ("198.41.128.0", 17),
];

/// Check if an IP address belongs to a known iCloud Private Relay egress range.
///
/// Returns `true` if the IP is in a known Apple relay range, indicating
/// the client is likely a real Apple device with iCloud+ subscription.
pub fn is_private_relay_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let ip_bits = u32::from(*v4);
            for (base_str, prefix_len) in PRIVATE_RELAY_IPV4_RANGES {
                if let Ok(base) = base_str.parse::<std::net::Ipv4Addr>() {
                    let base_bits = u32::from(base);
                    let mask = if *prefix_len >= 32 {
                        u32::MAX
                    } else {
                        u32::MAX << (32 - prefix_len)
                    };
                    if (ip_bits & mask) == (base_bits & mask) {
                        return true;
                    }
                }
            }
            false
        }
        IpAddr::V6(_) => {
            // IPv6 Private Relay ranges not yet implemented.
            // Apple also uses IPv6 relay egress — add ranges when needed.
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_private_token_header() {
        assert!(has_private_access_token(Some(
            "PrivateToken token=abc123base64..."
        )));
        assert!(!has_private_access_token(Some("Bearer abc123")));
        assert!(!has_private_access_token(None));
    }

    #[test]
    fn detects_known_relay_ip() {
        let relay_ip: IpAddr = "104.28.1.50".parse().unwrap();
        assert!(is_private_relay_ip(&relay_ip));
    }

    #[test]
    fn rejects_non_relay_ip() {
        let normal_ip: IpAddr = "8.8.8.8".parse().unwrap();
        assert!(!is_private_relay_ip(&normal_ip));
    }

    #[test]
    fn handles_ipv6() {
        let v6: IpAddr = "::1".parse().unwrap();
        assert!(!is_private_relay_ip(&v6)); // Not in our ranges yet
    }
}
