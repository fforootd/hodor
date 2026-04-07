//! DNS TXT record verification for custom domain ownership.

use hickory_resolver::Resolver;

/// Verify that a DNS TXT record exists at `host` containing `expected_value`.
///
/// Returns `Ok(true)` if the record is found, `Ok(false)` if not,
/// and `Err` on DNS resolution failures.
pub async fn verify_txt_record(host: &str, expected_value: &str) -> Result<bool, anyhow::Error> {
    let resolver = Resolver::builder_tokio()
        .map_err(|e| anyhow::anyhow!("failed to create DNS resolver: {e}"))?
        .build();

    let lookup = match resolver.txt_lookup(host).await {
        Ok(lookup) => lookup,
        Err(e) => {
            // NXDOMAIN or SERVFAIL — no record found, not a hard error.
            tracing::debug!(host = %host, error = %e, "DNS TXT lookup failed");
            return Ok(false);
        }
    };

    for record in lookup.iter() {
        let txt = record.to_string();
        if txt.contains(expected_value) {
            return Ok(true);
        }
    }

    Ok(false)
}
