//! Database-aware install methods for the template catalog.

use std::collections::HashMap;

use zitadel_db::{first_org_id, upsert_catalog_action};
use zitadel_db::provider::{
    ProviderCatalogRef, ProviderPayload, insert_provider_for, list_providers_for,
    update_provider_for,
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
        let instance_id = zitadel_db::current_instance_id_or(zitadel_db::DEFAULT_INSTANCE_ID)
            .into_owned();

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

        let org_id = first_org_id(db, &instance_id).await?.unwrap_or_default();

        let existing = list_providers_for(db, &instance_id)
            .await?
            .into_iter()
            .find(|candidate| {
                candidate.org_id == org_id && candidate.payload.display_name == display_name
            });

        let provider_id = if let Some(existing) = existing {
            update_provider_for(db, &instance_id, &existing.id, &provider).await?;
            existing.id
        } else {
            insert_provider_for(db, &instance_id, &provider_id, &org_id, &provider).await?;
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
        let instance_id = zitadel_db::current_instance_id_or(zitadel_db::DEFAULT_INSTANCE_ID)
            .into_owned();

        let action_id = uuid::Uuid::new_v4().to_string();
        let display_name = payload["display_name"].as_str().unwrap_or(&entry.name);
        let hook = payload["hook"].as_str().unwrap_or("on_event");
        let action_type = payload["action_type"].as_str().unwrap_or("expr");
        let trigger_expr = payload["trigger"].as_str().unwrap_or("true");
        let config = payload.get("config").cloned().unwrap_or_default();
        let priority = payload["priority"].as_i64().unwrap_or(0);
        let enabled = payload["enabled"].as_bool().unwrap_or(true);
        let fail_open = payload["fail_open"].as_bool().unwrap_or(false);

        let org_id = first_org_id(db, &instance_id).await?.unwrap_or_default();

        let config_json = serde_json::to_string(&config)?;
        let metadata = serde_json::json!({
            "_catalog": {
                "template_id": id,
                "template_version": entry.version,
            }
        });
        let metadata_json = serde_json::to_string(&metadata)?;
        let action_id = upsert_catalog_action(
            db,
            &instance_id,
            &action_id,
            &org_id,
            display_name,
            hook,
            action_type,
            trigger_expr,
            &config_json,
            priority,
            enabled,
            fail_open,
            &metadata_json,
        )
        .await?;

        tracing::info!(action_id = %action_id, template = id, name = display_name, "installed action from catalog");
        Ok(action_id)
    }
}
