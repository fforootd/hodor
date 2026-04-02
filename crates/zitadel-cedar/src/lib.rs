//! Cedar authorization module (POC).
//!
//! This is a lightweight stub that defines the entity types and permission model.
//! The full cedar-policy crate integration comes later.
//! For now, the instance owner (root) bypasses all checks (wildcard access).

use serde::{Deserialize, Serialize};

/// Entity types matching the OpenFGA model from the Go version.
/// These will become Cedar entity types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Instance,
    Org,
    Group,
    Project,
    App,
    Settings,
    Session,
    Schema,
    User,
}

/// Relation/permission types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Owner,
    Admin,
    Member,
    Viewer,
    Create,
    Read,
    Update,
    Delete,
}

/// Authorization check result.
#[derive(Debug, Clone)]
pub struct AuthzDecision {
    pub allowed: bool,
    pub reason: String,
}

/// Authorization service (POC: simple role-based with instance owner bypass).
pub struct AuthzService {
    /// Instance owner user IDs (bypass all checks).
    instance_owners: Vec<String>,
}

impl AuthzService {
    pub fn new() -> Self {
        Self {
            instance_owners: Vec::new(),
        }
    }

    /// Register a user as an instance owner (root access).
    pub fn add_instance_owner(&mut self, user_id: String) {
        if !self.instance_owners.contains(&user_id) {
            self.instance_owners.push(user_id);
        }
    }

    /// Check if a user has a permission on an entity.
    /// For POC: instance owners get wildcard access.
    pub fn check(&self, user_id: &str, permission: Permission, entity_type: EntityType, _entity_id: &str) -> AuthzDecision {
        // Instance owner bypass (root gets *).
        if self.instance_owners.contains(&user_id.to_string()) {
            return AuthzDecision {
                allowed: true,
                reason: "instance_owner_bypass".into(),
            };
        }

        // Default: deny (proper Cedar policy evaluation comes later).
        tracing::debug!(
            user = user_id,
            permission = ?permission,
            entity_type = ?entity_type,
            "authorization check — default deny (Cedar not yet integrated)"
        );
        AuthzDecision {
            allowed: false,
            reason: "no_matching_policy".into(),
        }
    }
}

/// Cedar policy template (for documentation / future use).
/// This is what the policies will look like when cedar-policy is integrated.
pub const CEDAR_POLICY_TEMPLATE: &str = r#"
// Instance owner gets wildcard access.
permit(
    principal is User,
    action,
    resource
) when {
    principal in Instance::"self".owners
};

// Org admins can manage org resources.
permit(
    principal is User,
    action in [Action::"create", Action::"read", Action::"update", Action::"delete"],
    resource
) when {
    principal in resource.org.admins
};

// Org members can read.
permit(
    principal is User,
    action == Action::"read",
    resource
) when {
    principal in resource.org.members
};
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_owner_bypass() {
        let mut svc = AuthzService::new();
        svc.add_instance_owner("admin-123".into());
        let decision = svc.check("admin-123", Permission::Delete, EntityType::Org, "org-1");
        assert!(decision.allowed);
        assert_eq!(decision.reason, "instance_owner_bypass");
    }

    #[test]
    fn non_owner_denied() {
        let svc = AuthzService::new();
        let decision = svc.check("user-456", Permission::Read, EntityType::User, "user-789");
        assert!(!decision.allowed);
    }
}
