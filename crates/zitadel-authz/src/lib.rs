//! Built-in authorization role catalog and permission-resolution helpers.
//!
//! The authoritative built-in role definitions are vendored from ZITADEL's
//! `InternalAuthZ.RolePermissionMappings` snapshot in `cmd/defaults.yaml`,
//! then normalized into a catalog that the app and storage layers can seed and
//! query offline.

mod builtin;

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

pub use builtin::{BUILTIN_ROLE_SOURCE_URL, BUILTIN_ROLE_SOURCE_VERSION};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleDefinition {
    pub role_key: String,
    pub relation_name: String,
    pub scope_kind: String,
    pub permissions: Vec<String>,
    pub builtin: bool,
    pub source_version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionGrant {
    pub scope_kind: String,
    pub role_key: String,
    pub relation_name: String,
}

static BUILTIN_DEFINITIONS: OnceLock<Vec<RoleDefinition>> = OnceLock::new();
static PERMISSION_INDEX: OnceLock<HashMap<(String, String), Vec<PermissionGrant>>> =
    OnceLock::new();

pub fn builtin_role_definitions() -> &'static [RoleDefinition] {
    BUILTIN_DEFINITIONS.get_or_init(|| {
        builtin::BUILTIN_ROLES
            .iter()
            .map(|role| RoleDefinition {
                role_key: role.role_key.to_string(),
                relation_name: relation_name_for_role(role.role_key),
                scope_kind: role.scope_kind.to_string(),
                permissions: role.permissions.iter().map(|item| (*item).to_string()).collect(),
                builtin: true,
                source_version: BUILTIN_ROLE_SOURCE_VERSION.to_string(),
            })
            .collect()
    })
}

pub fn builtin_role_definition(role_key: &str) -> Option<&'static RoleDefinition> {
    builtin_role_definitions()
        .iter()
        .find(|definition| definition.role_key == role_key)
}

pub fn relation_name_for_role(role_key: &str) -> String {
    role_key.to_ascii_lowercase()
}

pub fn role_key_for_relation(relation_name: &str) -> Option<&'static str> {
    builtin_role_definitions()
        .iter()
        .find(|definition| definition.relation_name == relation_name)
        .map(|definition| definition.role_key.as_str())
}

pub fn grants_for_permission(scope_kind: &str, permission: &str) -> Vec<PermissionGrant> {
    permission_index()
        .get(&(scope_kind.to_string(), permission.to_string()))
        .cloned()
        .unwrap_or_default()
}

pub fn role_grants_permission(role_key: &str, permission: &str) -> bool {
    builtin_role_definition(role_key)
        .is_some_and(|definition| definition.permissions.iter().any(|item| item == permission))
}

fn permission_index() -> &'static HashMap<(String, String), Vec<PermissionGrant>> {
    PERMISSION_INDEX.get_or_init(|| {
        let mut index: HashMap<(String, String), Vec<PermissionGrant>> = HashMap::new();
        for definition in builtin_role_definitions() {
            for permission in &definition.permissions {
                index
                    .entry((definition.scope_kind.clone(), permission.clone()))
                    .or_default()
                    .push(PermissionGrant {
                        scope_kind: definition.scope_kind.clone(),
                        role_key: definition.role_key.clone(),
                        relation_name: definition.relation_name.clone(),
                    });
            }
        }
        index
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_contains_upstream_roles() {
        let keys = builtin_role_definitions()
            .iter()
            .map(|role| role.role_key.as_str())
            .collect::<Vec<_>>();
        assert!(keys.contains(&"IAM_OWNER"));
        assert!(keys.contains(&"ORG_USER_MANAGER"));
        assert!(keys.contains(&"PROJECT_GRANT_OWNER"));
    }

    #[test]
    fn relation_names_are_normalized_to_lower_snake_case() {
        assert_eq!(relation_name_for_role("IAM_OWNER"), "iam_owner");
        assert_eq!(relation_name_for_role("PROJECT_GRANT_OWNER"), "project_grant_owner");
    }

    #[test]
    fn permission_lookup_returns_expected_role() {
        let grants = grants_for_permission("instance", "iam.write");
        assert!(grants.iter().any(|grant| grant.role_key == "IAM_OWNER"));
        assert!(!grants.iter().any(|grant| grant.role_key == "IAM_OWNER_VIEWER"));
    }
}
