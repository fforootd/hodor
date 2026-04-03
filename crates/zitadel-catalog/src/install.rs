//! Database-aware install methods for the template catalog.

use std::collections::HashMap;

use zitadel_db::provider::{
    ProviderCatalogRef, ProviderPayload, insert_provider, list_providers, update_provider,
};

use crate::Catalog;

impl Catalog {
    /// Install a provider template into the database.
    pub async fn install_provider(
        &self,
        id: &str,
        variables: &HashMap<String, serde_json::Value>,
        db: &zitadel_db::Db,
    ) -> anyhow::Result<String> {
        let (entry, payload) = self.resolve_payload(id, variables)?;
        let scoped = db.scoped_default();

        let provider_id = uuid::Uuid::new_v4().to_string();
        let mut provider: ProviderPayload = serde_json::from_value(payload)?;
        let display_name = provider.display_name.clone();
        provider.catalog_ref = ProviderCatalogRef {
            template_id: id.to_string(),
            template_version: entry.version.clone(),
            official: entry.official,
            capabilities: entry.capabilities.clone(),
            logo_url: entry.logo_url.clone(),
            docs_url: entry.docs_url.clone(),
        };

        let org_id: String =
            sqlx::query_as::<_, (String,)>("SELECT id FROM orgs WHERE instance_id = $1 LIMIT 1")
                .bind(scoped.instance_id())
                .fetch_optional(scoped.pool())
                .await?
                .map(|r| r.0)
                .unwrap_or_default();

        let existing = list_providers(&scoped)
            .await?
            .into_iter()
            .find(|candidate| {
                candidate.org_id == org_id && candidate.payload.display_name == display_name
            });

        let provider_id = if let Some(existing) = existing {
            update_provider(&scoped, &existing.id, &provider).await?;
            existing.id
        } else {
            insert_provider(&scoped, &provider_id, &org_id, &provider).await?;
            provider_id
        };

        tracing::info!(provider_id = %provider_id, template = id, name = display_name, "installed provider from catalog");
        Ok(provider_id)
    }

    /// Install an action template into the database.
    pub async fn install_action(
        &self,
        id: &str,
        variables: &HashMap<String, serde_json::Value>,
        db: &zitadel_db::Db,
    ) -> anyhow::Result<String> {
        let (entry, payload) = self.resolve_payload(id, variables)?;
        let scoped = db.scoped_default();

        let action_id = uuid::Uuid::new_v4().to_string();
        let display_name = payload["display_name"].as_str().unwrap_or(&entry.name);
        let hook = payload["hook"].as_str().unwrap_or("on_event");
        let action_type = payload["action_type"].as_str().unwrap_or("expr");
        let trigger_expr = payload["trigger"].as_str().unwrap_or("true");
        let config = payload.get("config").cloned().unwrap_or_default();
        let priority = payload["priority"].as_i64().unwrap_or(0);
        let enabled = payload["enabled"].as_bool().unwrap_or(true);
        let fail_open = payload["fail_open"].as_bool().unwrap_or(false);

        let org_id: String =
            sqlx::query_as::<_, (String,)>("SELECT id FROM orgs WHERE instance_id = $1 LIMIT 1")
                .bind(scoped.instance_id())
                .fetch_optional(scoped.pool())
                .await?
                .map(|r| r.0)
                .unwrap_or_default();

        let config_json = serde_json::to_string(&config)?;
        let metadata = serde_json::json!({
            "_catalog": {
                "template_id": id,
                "template_version": entry.version,
            }
        });

        // Upsert: update if action with same name exists.
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM actions WHERE instance_id = $1 AND org_id = $2 AND name = $3",
        )
        .bind(scoped.instance_id())
        .bind(&org_id)
        .bind(display_name)
        .fetch_optional(scoped.pool())
        .await?;

        let action_id = if let Some((existing_id,)) = existing {
            let sql = format!(
                "UPDATE actions SET hook = $1, action_type = $2, trigger_expr = $3, config = {}, \
                 priority = $4, enabled = $5, fail_open = $6, metadata = {}, updated_at = CURRENT_TIMESTAMP \
                 WHERE id = $7",
                scoped.json_bind(8),
                scoped.json_bind(9),
            );
            sqlx::query(&sql)
                .bind(hook)
                .bind(action_type)
                .bind(trigger_expr)
                .bind(priority)
                .bind(enabled)
                .bind(fail_open)
                .bind(&existing_id)
                .bind(&config_json)
                .bind(serde_json::to_string(&metadata)?)
                .execute(scoped.pool())
                .await?;
            existing_id
        } else {
            let sql = format!(
                "INSERT INTO actions (id, instance_id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, metadata) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, {}, $8, $9, $10, {})",
                scoped.json_bind(11),
                scoped.json_bind(12),
            );
            sqlx::query(&sql)
                .bind(&action_id)
                .bind(scoped.instance_id())
                .bind(&org_id)
                .bind(display_name)
                .bind(hook)
                .bind(action_type)
                .bind(trigger_expr)
                .bind(priority)
                .bind(enabled)
                .bind(fail_open)
                .bind(&config_json)
                .bind(serde_json::to_string(&metadata)?)
                .execute(scoped.pool())
                .await?;
            action_id
        };

        tracing::info!(action_id = %action_id, template = id, name = display_name, "installed action from catalog");
        Ok(action_id)
    }
}
