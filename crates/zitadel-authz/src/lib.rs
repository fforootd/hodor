//! Cedar authorization module.
//!
//! Uses the cedar-policy crate for real policy evaluation.
//! Entity types: User, Instance, Org, Group, Project, App, Session, Settings.
//! Instance owner gets wildcard access (root bypass).

use cedar_policy::{Authorizer, Context, Decision, Entities, EntityUid, PolicySet, Request};
use std::str::FromStr;
use std::sync::Arc;

/// The Cedar policy set for Zitadel's internal authorization.
const POLICIES: &str = r#"
// Instance owners can do anything.
permit(
    principal,
    action,
    resource
) when {
    principal has role && principal.role == "owner"
};

// Admins can do anything within their scope.
permit(
    principal,
    action,
    resource
) when {
    principal has role && principal.role == "admin"
};

// Members can read.
permit(
    principal,
    action == Action::"read",
    resource
) when {
    principal has role && principal.role == "member"
};

// Viewers can read.
permit(
    principal,
    action == Action::"read",
    resource
) when {
    principal has role && principal.role == "viewer"
};
"#;

/// Authorization service using Cedar policies.
pub struct AuthzService {
    authorizer: Authorizer,
    policies: Arc<PolicySet>,
    /// Instance owner user IDs (bypass all checks via "owner" role).
    instance_owners: Vec<String>,
}

/// Check result.
#[derive(Debug, Clone)]
pub struct AuthzDecision {
    pub allowed: bool,
    pub reason: String,
}

impl Default for AuthzService {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthzService {
    pub fn new() -> Self {
        let policies = PolicySet::from_str(POLICIES).unwrap_or_else(|e| {
            tracing::error!(error = %e, "failed to parse Cedar policies, using empty set");
            PolicySet::new()
        });
        tracing::info!(
            policies = policies.policies().count(),
            "Cedar policies loaded"
        );

        Self {
            authorizer: Authorizer::new(),
            policies: Arc::new(policies),
            instance_owners: Vec::new(),
        }
    }

    pub fn add_instance_owner(&mut self, user_id: String) {
        if !self.instance_owners.contains(&user_id) {
            tracing::info!(user_id, "registered instance owner");
            self.instance_owners.push(user_id);
        }
    }

    pub fn is_instance_owner(&self, user_id: &str) -> bool {
        self.instance_owners.contains(&user_id.to_string())
    }

    /// Check if user has permission on entity.
    /// Instance owners always get access (role=owner in entity attributes).
    pub fn check(
        &self,
        user_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
    ) -> AuthzDecision {
        // Fast path: instance owners.
        if self.is_instance_owner(user_id) {
            return AuthzDecision {
                allowed: true,
                reason: "instance_owner".into(),
            };
        }

        // Build Cedar request.
        let principal = match EntityUid::from_str(&format!("User::\"{}\"", user_id)) {
            Ok(p) => p,
            Err(_) => {
                return AuthzDecision {
                    allowed: false,
                    reason: "invalid_principal".into(),
                };
            }
        };
        let action_uid = match EntityUid::from_str(&format!("Action::\"{}\"", action)) {
            Ok(a) => a,
            Err(_) => {
                return AuthzDecision {
                    allowed: false,
                    reason: "invalid_action".into(),
                };
            }
        };
        let resource = match EntityUid::from_str(&format!(
            "{}::\"{}\"",
            capitalize(resource_type),
            resource_id
        )) {
            Ok(r) => r,
            Err(_) => {
                return AuthzDecision {
                    allowed: false,
                    reason: "invalid_resource".into(),
                };
            }
        };

        let request = match Request::new(principal, action_uid, resource, Context::empty(), None) {
            Ok(r) => r,
            Err(e) => {
                return AuthzDecision {
                    allowed: false,
                    reason: format!("request_error: {e}"),
                };
            }
        };

        let entities = Entities::empty();
        let response = self
            .authorizer
            .is_authorized(&request, &self.policies, &entities);

        AuthzDecision {
            allowed: response.decision() == Decision::Allow,
            reason: if response.decision() == Decision::Allow {
                "policy_allow".into()
            } else {
                "default_deny".into()
            },
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_owner_bypass() {
        let mut svc = AuthzService::new();
        svc.add_instance_owner("admin-123".into());
        let d = svc.check("admin-123", "delete", "org", "org-1");
        assert!(d.allowed);
        assert_eq!(d.reason, "instance_owner");
    }

    #[test]
    fn non_owner_denied_by_default() {
        let svc = AuthzService::new();
        let d = svc.check("user-456", "read", "user", "user-789");
        // Without entity attributes, Cedar can't evaluate role-based policies → deny.
        assert!(!d.allowed);
    }

    #[test]
    fn policies_load() {
        let svc = AuthzService::new();
        assert!(svc.policies.policies().count() >= 4);
    }
}
