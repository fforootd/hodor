//! Embedded template catalog for the Zitadel marketplace.
//!
//! Templates are compiled into the binary from the `embedded/` directory.
//! Each template has an index entry (catalog.json) and a payload file with
//! variables that are substituted on install.

pub mod install;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Embedded files ───────────────────────────────────────

const CATALOG_INDEX: &str = include_str!("../embedded/catalog.json");

// Provider templates
const TPL_GOOGLE_OIDC: &str = include_str!("../embedded/templates/providers/google-oidc.json");
const TPL_ENTRA_ID: &str = include_str!("../embedded/templates/providers/entra-id.json");
const TPL_CUSTOM_OIDC: &str = include_str!("../embedded/templates/providers/custom-oidc.json");
const TPL_GITHUB: &str = include_str!("../embedded/templates/providers/github.json");
const TPL_GITLAB: &str = include_str!("../embedded/templates/providers/gitlab.json");

// Action templates
const TPL_RATE_LIMIT: &str = include_str!("../embedded/templates/actions/rate-limit-by-path.json");
const TPL_WEBHOOK: &str =
    include_str!("../embedded/templates/actions/webhook-on-user-created.json");
const TPL_BLOCK_DISPOSABLE: &str =
    include_str!("../embedded/templates/actions/block-disposable-emails.json");

// Authorization templates
const TPL_RBAC_BASIC: &str = include_str!("../embedded/templates/authorization/rbac-basic.json");

// Login flow templates
const TPL_PASSKEY_FIRST: &str =
    include_str!("../embedded/templates/login_flows/passkey-first.json");
const TPL_SSO_ENTERPRISE: &str =
    include_str!("../embedded/templates/login_flows/sso-enterprise.json");

fn template_content(id: &str) -> Option<&'static str> {
    match id {
        "google-oidc" => Some(TPL_GOOGLE_OIDC),
        "entra-id" => Some(TPL_ENTRA_ID),
        "custom-oidc" => Some(TPL_CUSTOM_OIDC),
        "github" => Some(TPL_GITHUB),
        "gitlab" => Some(TPL_GITLAB),
        "rate-limit-by-path" => Some(TPL_RATE_LIMIT),
        "webhook-on-user-created" => Some(TPL_WEBHOOK),
        "block-disposable-emails" => Some(TPL_BLOCK_DISPOSABLE),
        "rbac-basic" => Some(TPL_RBAC_BASIC),
        "passkey-first" => Some(TPL_PASSKEY_FIRST),
        "sso-enterprise" => Some(TPL_SSO_ENTERPRISE),
        _ => None,
    }
}

// ─── Types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogIndex {
    pub version: String,
    pub templates: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub official: bool,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub logo_url: String,
    #[serde(default)]
    pub docs_url: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDetail {
    #[serde(rename = "type")]
    pub template_type: String,
    pub version: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub variables: HashMap<String, VariableDef>,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDef {
    #[serde(rename = "type", default)]
    pub var_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub sensitive: bool,
}

// ─── Catalog Service ──────────────────────────────────────

pub struct Catalog {
    index: CatalogIndex,
}

impl Catalog {
    /// Create catalog from embedded templates.
    pub fn embedded() -> Self {
        let index: CatalogIndex =
            serde_json::from_str(CATALOG_INDEX).expect("embedded catalog.json is valid");
        Self { index }
    }

    /// List templates, optionally filtered by type and tag.
    pub fn list(&self, type_filter: Option<&str>, tag_filter: Option<&str>) -> Vec<&CatalogEntry> {
        self.index
            .templates
            .iter()
            .filter(|t| {
                if let Some(tf) = type_filter {
                    if t.entry_type != tf {
                        return false;
                    }
                }
                if let Some(tag) = tag_filter {
                    if !t.tags.iter().any(|t| t == tag) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Get full template detail including variables and payload.
    pub fn get(&self, id: &str) -> Option<(CatalogEntry, TemplateDetail)> {
        let entry = self.index.templates.iter().find(|t| t.id == id)?;
        let content = template_content(id)?;
        let detail: TemplateDetail = serde_json::from_str(content).ok()?;
        Some((entry.clone(), detail))
    }

    /// Resolve a template payload by substituting variables.
    pub fn resolve_payload(
        &self,
        id: &str,
        variables: &HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<(CatalogEntry, serde_json::Value)> {
        let (entry, detail) = self
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("template not found: {id}"))?;

        // Build substitution map: user values + defaults.
        let mut subs: HashMap<String, String> = HashMap::new();
        for (key, def) in &detail.variables {
            if let Some(user_val) = variables.get(key) {
                subs.insert(key.clone(), value_to_string(user_val));
            } else if let Some(default_val) = &def.default {
                subs.insert(key.clone(), value_to_string(default_val));
            }
        }

        // Substitute {{variable}} placeholders in the payload JSON string.
        let mut payload_str = serde_json::to_string(&detail.payload)?;
        for (key, val) in &subs {
            payload_str = payload_str.replace(&format!("{{{{{key}}}}}"), val);
        }

        let resolved: serde_json::Value = serde_json::from_str(&payload_str)?;
        Ok((entry, resolved))
    }
}

fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => v.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_embedded_catalog() {
        let catalog = Catalog::embedded();
        assert_eq!(catalog.index.templates.len(), 11);
    }

    #[test]
    fn list_all() {
        let catalog = Catalog::embedded();
        assert_eq!(catalog.list(None, None).len(), 11);
    }

    #[test]
    fn list_providers_only() {
        let catalog = Catalog::embedded();
        let providers = catalog.list(Some("provider"), None);
        assert_eq!(providers.len(), 5);
    }

    #[test]
    fn list_actions_only() {
        let catalog = Catalog::embedded();
        assert_eq!(catalog.list(Some("action"), None).len(), 3);
    }

    #[test]
    fn get_google_template() {
        let catalog = Catalog::embedded();
        let (entry, detail) = catalog.get("google-oidc").unwrap();
        assert_eq!(entry.name, "Google OIDC");
        assert_eq!(detail.template_type, "provider");
        assert!(detail.variables.contains_key("client_id"));
        assert!(detail.variables["client_secret"].sensitive);
    }

    #[test]
    fn resolve_google_payload() {
        let catalog = Catalog::embedded();
        let mut vars = HashMap::new();
        vars.insert("client_id".into(), serde_json::json!("my-google-id"));
        vars.insert(
            "client_secret".into(),
            serde_json::json!("my-google-secret"),
        );

        let (_entry, payload) = catalog.resolve_payload("google-oidc", &vars).unwrap();
        assert_eq!(payload["connection"]["client_id"], "my-google-id");
        assert_eq!(payload["connection"]["client_secret"], "my-google-secret");
        assert_eq!(
            payload["connection"]["issuer"],
            "https://accounts.google.com"
        );
        assert_eq!(payload["display_name"], "Google");
    }

    #[test]
    fn resolve_with_custom_name() {
        let catalog = Catalog::embedded();
        let mut vars = HashMap::new();
        vars.insert(
            "provider_name".into(),
            serde_json::json!("Corporate Google"),
        );
        vars.insert("client_id".into(), serde_json::json!("id"));
        vars.insert("client_secret".into(), serde_json::json!("secret"));

        let (_entry, payload) = catalog.resolve_payload("google-oidc", &vars).unwrap();
        assert_eq!(payload["display_name"], "Corporate Google");
    }

    #[test]
    fn unknown_template_returns_none() {
        let catalog = Catalog::embedded();
        assert!(catalog.get("nonexistent").is_none());
    }
}
