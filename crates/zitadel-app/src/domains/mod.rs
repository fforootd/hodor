//! Customer-facing custom domain management use cases.
//!
//! These operate within instance context (no parent management required).
//! Distinct from the staff-level domain CRUD in `instances::AddDomain`.

pub mod dns;

use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{DomainRecord, DomainRemoveResult, Repositories};
use std::sync::Arc;

// ── Commands ──

pub struct AddCustomDomainCommand {
    pub domain: String,
    pub purpose: String,
    pub org_id: Option<String>,
}

// ── List Custom Domains ──

pub struct ListCustomDomains {
    repos: Arc<Repositories>,
}

impl ListCustomDomains {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    pub async fn execute(
        &self,
        ctx: &ActorContext,
        instance_id: &str,
        org_id: Option<&str>,
    ) -> Result<Vec<DomainRecord>, AppError> {
        require_domain_scope_permission(&self.repos, ctx, instance_id, org_id, "viewer").await?;
        self.repos
            .instances
            .list_domains_for_instance(instance_id, org_id)
            .await
            .map_err(AppError::Internal)
    }
}

// ── Get Custom Domain ──

pub struct GetCustomDomain {
    repos: Arc<Repositories>,
}

impl GetCustomDomain {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    pub async fn execute(
        &self,
        ctx: &ActorContext,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
    ) -> Result<Option<DomainRecord>, AppError> {
        require_domain_scope_permission(&self.repos, ctx, instance_id, org_id, "viewer").await?;
        self.repos
            .instances
            .get_domain(instance_id, org_id, domain)
            .await
            .map_err(AppError::Internal)
    }
}

// ── Add Custom Domain ──

pub struct AddCustomDomain {
    repos: Arc<Repositories>,
}

impl AddCustomDomain {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.add_custom_domain",
        skip_all,
        fields(event_type = "domain.added", category = "domain")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        instance_id: &str,
        cmd: AddCustomDomainCommand,
    ) -> Result<DomainRecord, AppError> {
        require_domain_scope_permission(
            &self.repos,
            ctx,
            instance_id,
            cmd.org_id.as_deref(),
            "admin",
        )
        .await?;

        let normalized_domain = normalize_domain(&cmd.domain);

        // Validate domain format.
        if normalized_domain.is_empty() {
            return Err(AppError::validation("domain is required"));
        }
        if !is_valid_domain(&normalized_domain) {
            return Err(AppError::validation("invalid domain name"));
        }

        // Validate purpose.
        if cmd.purpose != "allowed" && cmd.purpose != "served" {
            return Err(AppError::validation(
                "purpose must be 'allowed' or 'served'",
            ));
        }

        // Check domain doesn't already exist.
        if let Some(_existing) = self
            .repos
            .instances
            .find_domain(&normalized_domain)
            .await
            .map_err(AppError::Internal)?
        {
            return Err(AppError::already_exists("domain", &normalized_domain));
        }

        // Generate verification token and challenge.
        let verification_token = uuid::Uuid::now_v7().to_string();
        let dns_challenge_host = format!("_zitadel-challenge.{}", normalized_domain);

        let now = crate::users::chrono_now();
        let record = DomainRecord {
            instance_id: instance_id.to_string(),
            org_id: cmd.org_id.clone(),
            domain: normalized_domain.clone(),
            is_primary: false,
            purpose: cmd.purpose.clone(),
            state: "pending_verification".to_string(),
            verified: false,
            verification_token: verification_token.clone(),
            dns_challenge_host: dns_challenge_host.clone(),
            dns_authorization_id: String::new(),
            certificate_dns_record_name: String::new(),
            certificate_dns_record_type: String::new(),
            certificate_dns_record_value: String::new(),
            certificate_state: String::new(),
            certificate_id: String::new(),
            certificate_map_entry: String::new(),
            origin_trust_state: String::new(),
            provisioning_error: String::new(),
            created_at: now.clone(),
            updated_at: now,
        };

        self.repos
            .instances
            .set_domain(instance_id, &record)
            .await
            .map_err(AppError::Internal)?;

        // Emit domain.added event.
        self.repos
            .events
            .append(
                instance_id,
                &DomainEvent::DomainAdded {
                    domain: normalized_domain,
                    instance_id: instance_id.to_string(),
                    org_id: cmd.org_id,
                    purpose: cmd.purpose,
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(record)
    }
}

// ── Verify Custom Domain ──

pub struct VerifyCustomDomain {
    repos: Arc<Repositories>,
    cloud_enabled: bool,
}

impl VerifyCustomDomain {
    pub fn new(repos: Arc<Repositories>, cloud_enabled: bool) -> Self {
        Self {
            repos,
            cloud_enabled,
        }
    }

    #[tracing::instrument(
        name = "use_case.verify_custom_domain",
        skip_all,
        fields(event_type = "domain.verified", category = "domain")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        instance_id: &str,
        org_id: Option<&str>,
        domain_name: &str,
    ) -> Result<DomainRecord, AppError> {
        require_domain_scope_permission(&self.repos, ctx, instance_id, org_id, "admin").await?;

        let normalized_domain = normalize_domain(domain_name);
        let record = self
            .repos
            .instances
            .get_domain(instance_id, org_id, &normalized_domain)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("domain", &normalized_domain))?;

        // Must be in a verifiable state.
        if record.state != "pending_verification" && record.state != "verification_failed" {
            return Err(AppError::validation(format!(
                "domain is in state '{}', cannot verify",
                record.state
            )));
        }

        // Perform DNS TXT lookup.
        let expected_value = format!("zitadel-verify={}", record.verification_token);
        let found = dns::verify_txt_record(&record.dns_challenge_host, &expected_value)
            .await
            .map_err(AppError::Internal)?;

        if !found {
            // Update state to verification_failed.
            self.repos
                .instances
                .update_domain_state(
                    instance_id,
                    org_id,
                    &normalized_domain,
                    "verification_failed",
                    false,
                    Some("ownership TXT record not found"),
                )
                .await
                .map_err(AppError::Internal)?;

            self.repos
                .events
                .append(
                    instance_id,
                    &DomainEvent::DomainVerificationFailed {
                        domain: domain_name.to_string(),
                        instance_id: instance_id.to_string(),
                        reason: format!(
                            "TXT record not found at {}. Expected value: {}",
                            record.dns_challenge_host, expected_value
                        ),
                    },
                    None,
                    None,
                    None,
                )
                .await
                .map_err(AppError::Internal)?;

            return Err(AppError::validation(format!(
                "DNS verification failed. Add a TXT record at '{}' with value '{}'",
                record.dns_challenge_host, expected_value
            )));
        }

        // Verification succeeded.
        self.repos
            .events
            .append(
                instance_id,
                &DomainEvent::DomainVerified {
                    domain: domain_name.to_string(),
                    instance_id: instance_id.to_string(),
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        // Determine next state based on purpose and cloud mode.
        let next_state = if record.purpose == "served" && self.cloud_enabled {
            // Queue a DomainProvisioning effect for GCP cert + GLB.
            let source_key = format!("domain-provision:{domain_name}");
            let effect = crate::effect::Effect::new(
                normalized_domain.clone(),
                source_key,
                crate::effect::EffectType::DomainProvisioning,
                serde_json::json!({
                    "domain": normalized_domain,
                    "instance_id": instance_id,
                    "org_id": org_id,
                }),
                serde_json::json!({
                    "domain": normalized_domain,
                    "instance_id": instance_id,
                    "org_id": org_id,
                    "purpose": "served",
                }),
            );
            self.repos
                .effects
                .enqueue_batch(instance_id, &[effect])
                .await
                .map_err(AppError::Internal)?;
            "provisioning"
        } else if record.purpose == "allowed" {
            // For allowed domains, activate immediately (origin trust setup
            // will be handled by the CORS/CSP layers reading from the DB).
            self.repos
                .instances
                .update_domain_origin_trust_state(instance_id, org_id, &normalized_domain, "active")
                .await
                .map_err(AppError::Internal)?;
            "active"
        } else {
            // Self-hosted served domains: activate directly.
            "active"
        };

        self.repos
            .instances
            .update_domain_state(
                instance_id,
                org_id,
                &normalized_domain,
                next_state,
                true,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        // Reload and return updated record.
        self.repos
            .instances
            .get_domain(instance_id, org_id, &normalized_domain)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("domain disappeared after update")))
    }
}

// ── Remove Custom Domain ──

pub struct RemoveCustomDomain {
    repos: Arc<Repositories>,
    cloud_enabled: bool,
}

impl RemoveCustomDomain {
    pub fn new(repos: Arc<Repositories>, cloud_enabled: bool) -> Self {
        Self {
            repos,
            cloud_enabled,
        }
    }

    #[tracing::instrument(
        name = "use_case.remove_custom_domain",
        skip_all,
        fields(event_type = "domain.removed", category = "domain")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        instance_id: &str,
        org_id: Option<&str>,
        domain_name: &str,
    ) -> Result<DomainRemoveResult, AppError> {
        require_domain_scope_permission(&self.repos, ctx, instance_id, org_id, "admin").await?;
        let normalized_domain = normalize_domain(domain_name);
        let Some(record) = self
            .repos
            .instances
            .get_domain(instance_id, org_id, &normalized_domain)
            .await
            .map_err(AppError::Internal)?
        else {
            return Ok(DomainRemoveResult::NotFound);
        };

        let cloud_managed = !record.certificate_id.is_empty()
            || !record.dns_authorization_id.is_empty()
            || !record.certificate_map_entry.is_empty();
        let needs_async_deprovision =
            record.purpose == "served" && (self.cloud_enabled || cloud_managed);

        let result = if needs_async_deprovision {
            if record.is_primary {
                DomainRemoveResult::PrimaryDomain
            } else {
                self.repos
                    .instances
                    .update_domain_state(
                        instance_id,
                        org_id,
                        &normalized_domain,
                        "deprovisioning",
                        record.verified,
                        None,
                    )
                    .await
                    .map_err(AppError::Internal)?;

                let source_key = format!(
                    "domain-deprovision:{}:{}:{}",
                    instance_id,
                    org_id.unwrap_or_default(),
                    normalized_domain
                );
                let effect = crate::effect::Effect::new(
                    normalized_domain.clone(),
                    source_key,
                    crate::effect::EffectType::DomainDeprovisioning,
                    serde_json::json!({
                        "domain": normalized_domain,
                        "instance_id": instance_id,
                        "org_id": org_id,
                    }),
                    serde_json::json!({
                        "domain": normalized_domain,
                        "instance_id": instance_id,
                        "org_id": org_id,
                        "purpose": record.purpose,
                        "action": "deprovision",
                    }),
                );
                self.repos
                    .effects
                    .enqueue_batch(instance_id, &[effect])
                    .await
                    .map_err(AppError::Internal)?;
                DomainRemoveResult::Deleted
            }
        } else {
            self.repos
                .instances
                .remove_domain(instance_id, org_id, &normalized_domain)
                .await
                .map_err(AppError::Internal)?
        };

        if result == DomainRemoveResult::Deleted {
            self.repos
                .events
                .append(
                    instance_id,
                    &DomainEvent::DomainRemoved {
                        domain: normalized_domain,
                        instance_id: instance_id.to_string(),
                        actor_id: ctx.user_id().to_string(),
                    },
                    None,
                    None,
                    None,
                )
                .await
                .map_err(AppError::Internal)?;
        }

        Ok(result)
    }
}

// ── Helpers ──

/// Basic domain name validation.
fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 253 {
        return false;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    for label in &labels {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
    }
    true
}

fn normalize_domain(domain: &str) -> String {
    domain.trim().trim_end_matches('.').to_ascii_lowercase()
}

async fn require_domain_scope_permission(
    repos: &Repositories,
    ctx: &ActorContext,
    instance_id: &str,
    org_id: Option<&str>,
    relation: &str,
) -> Result<(), AppError> {
    let object = if let Some(org_id) = org_id {
        format!("org:{org_id}")
    } else {
        format!("instance:{instance_id}")
    };
    crate::authz::require_permission(repos, ctx, relation, &object).await
}
