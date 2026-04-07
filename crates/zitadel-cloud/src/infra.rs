// SPDX-License-Identifier: LicenseRef-ZITADEL-Cloud
//
// GCP infrastructure integration:
//   - Cloud Load Balancer host rule and backend management
//   - TLS certificate provisioning (with DNS verification)
//   - Domain-to-instance routing at the LB layer

use std::sync::Arc;

use zitadel_app::effect::{Effect, EffectDispatcher, EffectType};
use zitadel_db::Db;

use crate::gcp::GcpClient;

/// Durable effect dispatcher for domain provisioning on GCP.
///
/// When a domain transitions to `verified`, a `DomainProvisioning` effect
/// is enqueued. This dispatcher picks it up and:
/// 1. Creates a DNS-authorized certificate via Certificate Manager
/// 2. Adds the cert to the GLB certificate map
/// 3. Adds a host rule to the GLB URL map
/// 4. Updates the domain state to `active`
pub struct DomainProvisioningDispatcher {
    gcp: Arc<GcpClient>,
    db: Db,
}

impl DomainProvisioningDispatcher {
    pub fn new(gcp: Arc<GcpClient>, db: Db) -> Self {
        Self { gcp, db }
    }

    pub fn kind(&self) -> EffectType {
        EffectType::DomainProvisioning
    }

    /// Dispatch a domain provisioning effect.
    ///
    /// Called by the effects worker when a `DomainProvisioning` effect is claimed.
    /// On failure, the effects system retries automatically with exponential backoff.
    pub async fn deliver(&self, effect: &Effect) -> anyhow::Result<()> {
        let domain = effect
            .config
            .get("domain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("DomainProvisioning effect missing 'domain'"))?;

        let instance_id = effect
            .config
            .get("instance_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let org_id = effect.config.get("org_id").and_then(|v| v.as_str());

        tracing::info!(
            effect_id = %effect.id,
            domain = %domain,
            instance_id = %instance_id,
            "reconciling TLS certificate and GLB host rule"
        );

        // Step 1: Create or fetch DNS authorization and persist the CNAME challenge
        // that Certificate Manager expects the customer to publish.
        let dns_auth = self
            .gcp
            .ensure_dns_authorization(domain)
            .await
            .map_err(|e| {
                tracing::error!(domain = %domain, error = %e, "DNS authorization creation failed");
                e
            })?;

        zitadel_db::update_domain_certificate_state_for_scope(
            &self.db,
            instance_id,
            org_id,
            domain,
            "awaiting_dns_authorization",
            "",
            None,
            Some(&dns_auth.name),
            Some(&dns_auth.record_name),
            Some(&dns_auth.record_type),
            Some(&dns_auth.record_value),
            None,
        )
        .await?;
        zitadel_db::update_domain_state_for_scope(
            &self.db,
            instance_id,
            org_id,
            domain,
            "awaiting_gcp_dns",
            true,
            None,
        )
        .await?;

        tracing::info!(domain = %domain, dns_auth = %dns_auth.name, "DNS authorization ready");

        // Step 2: Create or fetch the managed certificate.
        let cert_name = self
            .gcp
            .create_certificate(domain, &dns_auth.name)
            .await
            .map_err(|e| {
                tracing::error!(domain = %domain, error = %e, "certificate creation failed");
                e
            })?;

        tracing::info!(domain = %domain, cert = %cert_name, "managed certificate created");

        let cert_state = self.gcp.certificate_state(domain).await.map_err(|e| {
            tracing::error!(domain = %domain, error = %e, "certificate state lookup failed");
            e
        })?;
        let cert_id = cert_name.rsplit('/').next().unwrap_or("");

        if cert_state != "ACTIVE" {
            let error_message = format!("certificate not ready: {cert_state}");
            zitadel_db::update_domain_certificate_state_for_scope(
                &self.db,
                instance_id,
                org_id,
                domain,
                &cert_state.to_ascii_lowercase(),
                cert_id,
                None,
                Some(&dns_auth.name),
                Some(&dns_auth.record_name),
                Some(&dns_auth.record_type),
                Some(&dns_auth.record_value),
                Some(&error_message),
            )
            .await?;
            zitadel_db::update_domain_state_for_scope(
                &self.db,
                instance_id,
                org_id,
                domain,
                "awaiting_gcp_dns",
                true,
                Some(&error_message),
            )
            .await?;
            anyhow::bail!(error_message);
        }

        // Step 3: Add certificate to certificate map once the certificate is active.
        let cert_map_entry = self
            .gcp
            .add_certificate_to_map(domain, &cert_name)
            .await
            .map_err(|e| {
                tracing::error!(domain = %domain, error = %e, "certificate map entry creation failed");
                e
            })?;

        tracing::info!(domain = %domain, "certificate map entry added");

        // Step 4: Add host rule to GLB URL map.
        self.gcp.add_host_rule(domain).await.map_err(|e| {
            tracing::error!(domain = %domain, error = %e, "GLB host rule creation failed");
            e
        })?;

        tracing::info!(domain = %domain, "GLB host rule added");

        // Step 5: Update domain state to active.
        zitadel_db::update_domain_certificate_state_for_scope(
            &self.db,
            instance_id,
            org_id,
            domain,
            "active",
            cert_id,
            Some(&cert_map_entry),
            Some(&dns_auth.name),
            Some(&dns_auth.record_name),
            Some(&dns_auth.record_type),
            Some(&dns_auth.record_value),
            None,
        )
        .await?;
        zitadel_db::update_domain_state_for_scope(
            &self.db,
            instance_id,
            org_id,
            domain,
            "active",
            true,
            None,
        )
        .await?;

        tracing::info!(
            domain = %domain,
            instance_id = %instance_id,
            "domain provisioning complete — TLS and GLB active"
        );

        Ok(())
    }
}

impl EffectDispatcher for DomainProvisioningDispatcher {
    fn effect_type(&self) -> EffectType {
        self.kind()
    }

    fn dispatch<'a>(
        &'a self,
        effect: &'a Effect,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send + 'a>>
    {
        Box::pin(async move { self.deliver(effect).await })
    }
}

/// Durable effect dispatcher for domain teardown on GCP.
pub struct DomainDeprovisioningDispatcher {
    gcp: Arc<GcpClient>,
    db: Db,
}

impl DomainDeprovisioningDispatcher {
    pub fn new(gcp: Arc<GcpClient>, db: Db) -> Self {
        Self { gcp, db }
    }

    pub fn kind(&self) -> EffectType {
        EffectType::DomainDeprovisioning
    }

    pub async fn deliver(&self, effect: &Effect) -> anyhow::Result<()> {
        let domain = effect
            .config
            .get("domain")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow::anyhow!("DomainDeprovisioning effect missing 'domain'"))?;

        let instance_id = effect
            .config
            .get("instance_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let org_id = effect.config.get("org_id").and_then(|value| value.as_str());

        tracing::info!(
            effect_id = %effect.id,
            domain = %domain,
            instance_id = %instance_id,
            "removing TLS certificate and GLB host rule"
        );

        self.gcp.remove_domain_resources(domain).await?;
        let _ = zitadel_db::delete_domain_for_scope(&self.db, instance_id, org_id, domain).await?;

        tracing::info!(
            domain = %domain,
            instance_id = %instance_id,
            "domain deprovisioning complete"
        );

        Ok(())
    }
}

impl EffectDispatcher for DomainDeprovisioningDispatcher {
    fn effect_type(&self) -> EffectType {
        self.kind()
    }

    fn dispatch<'a>(
        &'a self,
        effect: &'a Effect,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send + 'a>>
    {
        Box::pin(async move { self.deliver(effect).await })
    }
}
