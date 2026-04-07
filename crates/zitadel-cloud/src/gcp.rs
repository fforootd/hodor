//! Lightweight GCP REST client for Certificate Manager and Compute Engine APIs.
//!
//! Auth: Workload Identity Federation first (GKE/Cloud Run metadata server),
//! falls back to service account JSON if `credentials_path` is configured.

use serde::Deserialize;

// Config type re-exported from zitadel-config.
pub use zitadel_config::GcpInfraConfig;

/// GCP API client for domain provisioning.
pub struct GcpClient {
    http: reqwest::Client,
    config: GcpInfraConfig,
    access_token: tokio::sync::RwLock<Option<CachedToken>>,
}

struct CachedToken {
    token: String,
    expires_at: std::time::Instant,
}

#[derive(Deserialize)]
struct MetadataTokenResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Clone)]
pub struct DnsAuthorizationDetails {
    pub name: String,
    pub record_name: String,
    pub record_type: String,
    pub record_value: String,
}

impl GcpClient {
    pub fn new(config: GcpInfraConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
            access_token: tokio::sync::RwLock::new(None),
        }
    }

    /// Get a valid access token, refreshing from metadata server or SA key if needed.
    async fn token(&self) -> anyhow::Result<String> {
        // Check cache.
        {
            let cached = self.access_token.read().await;
            if let Some(ref t) = *cached {
                if t.expires_at > std::time::Instant::now() {
                    return Ok(t.token.clone());
                }
            }
        }

        // Refresh.
        let (token, expires_in) = if self.config.credentials_path.is_empty() {
            self.token_from_metadata().await?
        } else {
            self.token_from_service_account().await?
        };

        let cached = CachedToken {
            token: token.clone(),
            expires_at: std::time::Instant::now()
                + std::time::Duration::from_secs(expires_in.saturating_sub(60)),
        };
        *self.access_token.write().await = Some(cached);
        Ok(token)
    }

    async fn token_from_metadata(&self) -> anyhow::Result<(String, u64)> {
        let url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";
        let resp: MetadataTokenResponse = self
            .http
            .get(url)
            .header("Metadata-Flavor", "Google")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok((resp.access_token, resp.expires_in))
    }

    async fn token_from_service_account(&self) -> anyhow::Result<(String, u64)> {
        // For the POC: shell out to gcloud or use a JWT grant flow.
        // Full SA key flow requires JWT signing — defer to a crate like `google-cloud-auth`.
        anyhow::bail!(
            "service account JSON auth not yet implemented; use workload identity instead"
        )
    }

    /// Create or fetch a DNS authorization for a custom domain.
    pub async fn ensure_dns_authorization(
        &self,
        domain: &str,
    ) -> anyhow::Result<DnsAuthorizationDetails> {
        let token = self.token().await?;
        let url = format!(
            "https://certificatemanager.googleapis.com/v1/projects/{}/locations/global/dnsAuthorizations?dnsAuthorizationId={}",
            self.config.project_id,
            dns_auth_id(domain)
        );

        let body = serde_json::json!({
            "domain": domain,
            "type": "PER_PROJECT_RECORD",
        });

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() && resp.status().as_u16() != 409 {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GCP create DNS authorization failed: HTTP {status}: {body}")
        }

        self.get_dns_authorization(domain).await
    }

    async fn get_dns_authorization(&self, domain: &str) -> anyhow::Result<DnsAuthorizationDetails> {
        let token = self.token().await?;
        let url = format!(
            "https://certificatemanager.googleapis.com/v1/projects/{}/locations/global/dnsAuthorizations/{}",
            self.config.project_id,
            dns_auth_id(domain)
        );
        let body: serde_json::Value = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let name = body
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let record = body
            .get("dnsResourceRecord")
            .and_then(|value| value.as_object())
            .ok_or_else(|| {
                anyhow::anyhow!("DNS authorization response missing dnsResourceRecord")
            })?;
        Ok(DnsAuthorizationDetails {
            name,
            record_name: record
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            record_type: record
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("CNAME")
                .to_string(),
            record_value: record
                .get("data")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        })
    }

    /// Create a managed certificate using DNS authorization.
    pub async fn create_certificate(
        &self,
        domain: &str,
        dns_auth_name: &str,
    ) -> anyhow::Result<String> {
        let token = self.token().await?;
        let cert_id = cert_id(domain);
        let url = format!(
            "https://certificatemanager.googleapis.com/v1/projects/{}/locations/global/certificates?certificateId={}",
            self.config.project_id, cert_id
        );

        let body = serde_json::json!({
            "managed": {
                "domains": [domain],
                "dnsAuthorizations": [dns_auth_name],
            },
        });

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        if resp.status().is_success() || resp.status().as_u16() == 409 {
            let cert_name = format!(
                "projects/{}/locations/global/certificates/{}",
                self.config.project_id, cert_id
            );
            return Ok(cert_name);
        }

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GCP create certificate failed: HTTP {status}: {body}")
    }

    pub async fn certificate_state(&self, domain: &str) -> anyhow::Result<String> {
        let token = self.token().await?;
        let url = format!(
            "https://certificatemanager.googleapis.com/v1/projects/{}/locations/global/certificates/{}",
            self.config.project_id,
            cert_id(domain)
        );
        let body: serde_json::Value = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(body
            .pointer("/managed/state")
            .or_else(|| body.get("state"))
            .and_then(|value| value.as_str())
            .unwrap_or("UNKNOWN")
            .to_string())
    }

    /// Add a certificate map entry for the domain.
    pub async fn add_certificate_to_map(
        &self,
        domain: &str,
        cert_name: &str,
    ) -> anyhow::Result<String> {
        let token = self.token().await?;
        let entry_id = cert_map_entry_id(domain);
        let url = format!(
            "https://certificatemanager.googleapis.com/v1/projects/{}/locations/global/certificateMaps/{}/certificateMapEntries?certificateMapEntryId={}",
            self.config.project_id, self.config.certificate_map, entry_id
        );

        let body = serde_json::json!({
            "hostname": domain,
            "certificates": [cert_name],
        });

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        if resp.status().is_success() || resp.status().as_u16() == 409 {
            return Ok(format!(
                "projects/{}/locations/global/certificateMaps/{}/certificateMapEntries/{}",
                self.config.project_id, self.config.certificate_map, entry_id
            ));
        }

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GCP add certificate map entry failed: HTTP {status}: {body}")
    }

    /// Add a host rule to the GLB URL map for the domain.
    pub async fn add_host_rule(&self, domain: &str) -> anyhow::Result<()> {
        let token = self.token().await?;

        // First, get the current URL map to add the host rule.
        let get_url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/global/urlMaps/{}",
            self.config.project_id, self.config.url_map
        );

        let resp = self
            .http
            .get(&get_url)
            .bearer_auth(&token)
            .send()
            .await?
            .error_for_status()?;

        let mut url_map: serde_json::Value = resp.json().await?;

        // Check if host rule already exists.
        if let Some(host_rules) = url_map.get("hostRules").and_then(|v| v.as_array()) {
            for rule in host_rules {
                if let Some(hosts) = rule.get("hosts").and_then(|v| v.as_array()) {
                    if hosts.iter().any(|h| h.as_str() == Some(domain)) {
                        tracing::info!(domain = %domain, "host rule already exists in URL map");
                        return Ok(());
                    }
                }
            }
        }

        // Add the host rule.
        let backend_service_url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/global/backendServices/{}",
            self.config.project_id, self.config.backend_service
        );

        // Add a path matcher for this domain.
        let path_matcher_name = format!("pm-{}", sanitize_for_gcp(domain));
        let new_path_matcher = serde_json::json!({
            "name": path_matcher_name,
            "defaultService": backend_service_url,
        });

        let new_host_rule = serde_json::json!({
            "hosts": [domain],
            "pathMatcher": path_matcher_name,
        });

        // Append to existing arrays.
        if let Some(arr) = url_map.get_mut("hostRules").and_then(|v| v.as_array_mut()) {
            arr.push(new_host_rule);
        }
        if let Some(arr) = url_map
            .get_mut("pathMatchers")
            .and_then(|v| v.as_array_mut())
        {
            arr.push(new_path_matcher);
        }

        // Patch the URL map.
        let patch_url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/global/urlMaps/{}",
            self.config.project_id, self.config.url_map
        );

        let resp = self
            .http
            .put(&patch_url)
            .bearer_auth(&token)
            .json(&url_map)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GCP update URL map failed: HTTP {status}: {body}");
        }

        Ok(())
    }

    /// Remove certificate and host rule for a domain (cleanup on domain removal).
    pub async fn remove_domain_resources(&self, domain: &str) -> anyhow::Result<()> {
        let token = self.token().await?;

        self.remove_host_rule(domain, &token).await?;

        // Remove certificate map entry.
        let entry_id = cert_map_entry_id(domain);
        let url = format!(
            "https://certificatemanager.googleapis.com/v1/projects/{}/locations/global/certificateMaps/{}/certificateMapEntries/{}",
            self.config.project_id, self.config.certificate_map, entry_id
        );
        let resp = self.http.delete(&url).bearer_auth(&token).send().await?;
        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            tracing::warn!(domain = %domain, status = %resp.status(), "failed to delete cert map entry");
        }

        // Remove certificate.
        let cert = cert_id(domain);
        let url = format!(
            "https://certificatemanager.googleapis.com/v1/projects/{}/locations/global/certificates/{}",
            self.config.project_id, cert
        );
        let resp = self.http.delete(&url).bearer_auth(&token).send().await?;
        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            tracing::warn!(domain = %domain, status = %resp.status(), "failed to delete certificate");
        }

        // Remove DNS authorization.
        let auth = dns_auth_id(domain);
        let url = format!(
            "https://certificatemanager.googleapis.com/v1/projects/{}/locations/global/dnsAuthorizations/{}",
            self.config.project_id, auth
        );
        let resp = self.http.delete(&url).bearer_auth(&token).send().await?;
        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            tracing::warn!(domain = %domain, status = %resp.status(), "failed to delete DNS authorization");
        }

        Ok(())
    }

    async fn remove_host_rule(&self, domain: &str, token: &str) -> anyhow::Result<()> {
        let get_url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/global/urlMaps/{}",
            self.config.project_id, self.config.url_map
        );
        let resp = self
            .http
            .get(&get_url)
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?;
        let mut url_map: serde_json::Value = resp.json().await?;
        let path_matcher_name = format!("pm-{}", sanitize_for_gcp(domain));

        if let Some(host_rules) = url_map
            .get_mut("hostRules")
            .and_then(|value| value.as_array_mut())
        {
            host_rules.retain(|rule| {
                !rule
                    .get("hosts")
                    .and_then(|value| value.as_array())
                    .map(|hosts| hosts.iter().any(|host| host.as_str() == Some(domain)))
                    .unwrap_or(false)
            });
        }
        if let Some(path_matchers) = url_map
            .get_mut("pathMatchers")
            .and_then(|value| value.as_array_mut())
        {
            path_matchers.retain(|matcher| {
                matcher
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    != path_matcher_name
            });
        }

        let patch_url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/global/urlMaps/{}",
            self.config.project_id, self.config.url_map
        );
        let resp = self
            .http
            .put(&patch_url)
            .bearer_auth(token)
            .json(&url_map)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GCP update URL map during cleanup failed: HTTP {status}: {body}");
        }
        Ok(())
    }
}

// ── Helpers ──

pub(crate) fn sanitize_for_gcp(domain: &str) -> String {
    domain.replace('.', "-").replace('_', "-")
}

pub(crate) fn dns_auth_id(domain: &str) -> String {
    format!("zitadel-auth-{}", sanitize_for_gcp(domain))
}

pub(crate) fn cert_id(domain: &str) -> String {
    format!("zitadel-cert-{}", sanitize_for_gcp(domain))
}

pub(crate) fn cert_map_entry_id(domain: &str) -> String {
    format!("zitadel-entry-{}", sanitize_for_gcp(domain))
}
