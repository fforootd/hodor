use anyhow::Context;
use google_cloud_spanner::statement::Statement;
use serde_json::{Map, Value};

use super::entities_impl::{
    SqlProviderRepository, SqlSchemaRepository, SqlSettingsRepository, json_string,
    limit_from_params, load_provider, load_schema, load_settings_exact, next_cursor, parse_scope,
    provider_definition_from_storage, provider_from_storage, provider_org_id,
    provider_payload_from_record, schema_from_retained, write_spanner_count, write_spanner_many,
    write_spanner_stmt,
};
use crate::{DEFAULT_ORG_ID, Db, delete_provider, first_org_id, list_schema_registry, provider};
use zitadel_app::repo::{
    BoxFuture, ListParams, ListResult, ProviderDefinitionRecord, ProviderRecord,
    ProviderRepository, SchemaRecord, SchemaRepository, SettingsRecord, SettingsRepository,
};

impl ProviderRepository for SqlProviderRepository {
    fn create(
        &self,
        instance_id: &str,
        provider_record: &ProviderRecord,
    ) -> BoxFuture<'_, anyhow::Result<ProviderRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let provider_record = provider_record.clone();
        Box::pin(async move {
            let org_id = if let Some(org_id) = provider_org_id(&provider_record.config) {
                org_id
            } else {
                first_org_id(&db, &instance_id)
                    .await?
                    .unwrap_or_else(|| DEFAULT_ORG_ID.to_string())
            };
            let payload = provider_payload_from_record(&provider_record)?;
            provider::insert_provider_for(
                &db,
                &instance_id,
                &provider_record.id,
                &org_id,
                &payload,
            )
            .await?;
            load_provider(&db, &instance_id, &provider_record.id)
                .await?
                .context("created provider but could not reload it")
        })
    }

    fn get(
        &self,
        instance_id: &str,
        provider_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<ProviderRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let provider_id = provider_id.to_string();
        Box::pin(async move { load_provider(&db, &instance_id, &provider_id).await })
    }

    fn get_definition(
        &self,
        instance_id: &str,
        provider_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<ProviderDefinitionRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let provider_id = provider_id.to_string();
        Box::pin(async move {
            provider::get_provider_for(&db, &instance_id, &provider_id)
                .await?
                .map(provider_definition_from_storage)
                .transpose()
        })
    }

    fn list(
        &self,
        instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<ProviderRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let params = params.clone();
        Box::pin(async move {
            let limit = limit_from_params(&params);
            let cursor = params.cursor.unwrap_or_default();
            let search = params.search.map(|value| value.to_lowercase());
            let mut items: Vec<ProviderRecord> = provider::list_providers_for(&db, &instance_id)
                .await?
                .into_iter()
                .filter(|item| item.id > cursor)
                .map(provider_from_storage)
                .collect::<anyhow::Result<_>>()?;
            if let Some(search) = search {
                items.retain(|item| {
                    item.name.to_lowercase().contains(&search)
                        || item.protocol.to_lowercase().contains(&search)
                });
            }
            items.truncate(limit as usize);
            Ok(ListResult {
                next_cursor: next_cursor(&items, limit, |item| item.id.as_str()),
                total_count: None,
                items,
            })
        })
    }

    fn update(
        &self,
        instance_id: &str,
        provider_record: &ProviderRecord,
    ) -> BoxFuture<'_, anyhow::Result<ProviderRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let provider_record = provider_record.clone();
        Box::pin(async move {
            let payload = provider_payload_from_record(&provider_record)?;
            let updated =
                provider::update_provider_for(&db, &instance_id, &provider_record.id, &payload)
                    .await?;
            if !updated {
                anyhow::bail!("provider not found");
            }
            load_provider(&db, &instance_id, &provider_record.id)
                .await?
                .context("updated provider but could not reload it")
        })
    }

    fn delete(&self, instance_id: &str, provider_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let provider_id = provider_id.to_string();
        Box::pin(async move {
            delete_provider(&db, &instance_id, &provider_id).await?;
            Ok(())
        })
    }
}

impl SchemaRepository for SqlSchemaRepository {
    fn register(
        &self,
        _instance_id: &str,
        schema: &SchemaRecord,
    ) -> BoxFuture<'_, anyhow::Result<SchemaRecord>> {
        let db = self.db.clone();
        let schema = schema.clone();
        Box::pin(async move {
            let schema_json = json_string(&schema.schema_json)?;
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped_default();
                    let sql = format!(
                        "INSERT INTO schemas (id, type, schema, version, is_default, visibility) \
                         VALUES ($1, $2, {}, $3, $4, $5)",
                        scoped.json_bind(6),
                    );
                    sqlx::query(&sql)
                        .bind(&schema.id)
                        .bind(&schema.schema_type)
                        .bind(schema.version)
                        .bind(schema.is_default)
                        .bind(&schema.visibility)
                        .bind(&schema_json)
                        .execute(scoped.pool())
                        .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "INSERT INTO schemas (id, type, schema, version, is_default, visibility) \
                         VALUES (@id, @type, @schema, @version, @is_default, @visibility)",
                    );
                    stmt.add_param("id", &schema.id);
                    stmt.add_param("type", &schema.schema_type);
                    stmt.add_param("schema", &schema_json);
                    stmt.add_param("version", &schema.version);
                    stmt.add_param("is_default", &schema.is_default);
                    stmt.add_param("visibility", &schema.visibility);
                    write_spanner_stmt(spanner, stmt).await?;
                }
            }
            load_schema(&db, &schema.id)
                .await?
                .context("created schema but could not reload it")
        })
    }

    fn get(
        &self,
        _instance_id: &str,
        schema_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SchemaRecord>>> {
        let db = self.db.clone();
        let schema_id = schema_id.to_string();
        Box::pin(async move { load_schema(&db, &schema_id).await })
    }

    fn get_by_type(
        &self,
        _instance_id: &str,
        schema_type: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SchemaRecord>>> {
        let db = self.db.clone();
        let schema_type = schema_type.to_string();
        Box::pin(async move {
            let items = list_schema_registry(&db, "", Some(&schema_type), 1).await?;
            Ok(items.into_iter().next().map(schema_from_retained))
        })
    }

    fn list(
        &self,
        _instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<SchemaRecord>>> {
        let db = self.db.clone();
        let params = params.clone();
        Box::pin(async move {
            let limit = limit_from_params(&params);
            let cursor = params.cursor.unwrap_or_default();
            let search = params.search.map(|value| value.to_lowercase());
            let mut items: Vec<SchemaRecord> = list_schema_registry(&db, &cursor, None, limit)
                .await?
                .into_iter()
                .map(schema_from_retained)
                .collect();
            if let Some(search) = search {
                items.retain(|item| {
                    item.schema_type.to_lowercase().contains(&search)
                        || item.id.to_lowercase().contains(&search)
                });
            }
            items.truncate(limit as usize);
            Ok(ListResult {
                next_cursor: next_cursor(&items, limit, |item| item.id.as_str()),
                total_count: None,
                items,
            })
        })
    }

    fn update(
        &self,
        _instance_id: &str,
        schema: &SchemaRecord,
    ) -> BoxFuture<'_, anyhow::Result<SchemaRecord>> {
        let db = self.db.clone();
        let schema = schema.clone();
        Box::pin(async move {
            let schema_json = json_string(&schema.schema_json)?;
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped_default();
                    let sql = format!(
                        "UPDATE schemas SET type = $1, schema = {}, version = $2, is_default = $3, visibility = $4 WHERE id = $5",
                        scoped.json_bind(6),
                    );
                    sqlx::query(&sql)
                        .bind(&schema.schema_type)
                        .bind(schema.version)
                        .bind(schema.is_default)
                        .bind(&schema.visibility)
                        .bind(&schema.id)
                        .bind(&schema_json)
                        .execute(scoped.pool())
                        .await?;
                    if schema.is_default {
                        sqlx::query(
                            "UPDATE schemas SET is_default = FALSE WHERE type = $1 AND id != $2",
                        )
                        .bind(&schema.schema_type)
                        .bind(&schema.id)
                        .execute(scoped.pool())
                        .await?;
                    }
                }
                Db::Spanner(spanner) => {
                    let mut stmts = vec![{
                        let mut stmt = Statement::new(
                            "UPDATE schemas SET type = @type, schema = @schema, version = @version, \
                                 is_default = @is_default, visibility = @visibility WHERE id = @id",
                        );
                        stmt.add_param("type", &schema.schema_type);
                        stmt.add_param("schema", &schema_json);
                        stmt.add_param("version", &schema.version);
                        stmt.add_param("is_default", &schema.is_default);
                        stmt.add_param("visibility", &schema.visibility);
                        stmt.add_param("id", &schema.id);
                        stmt
                    }];
                    if schema.is_default {
                        let mut stmt = Statement::new(
                            "UPDATE schemas SET is_default = FALSE WHERE type = @type AND id != @id",
                        );
                        stmt.add_param("type", &schema.schema_type);
                        stmt.add_param("id", &schema.id);
                        stmts.push(stmt);
                    }
                    write_spanner_many(spanner, stmts).await?;
                }
            }
            load_schema(&db, &schema.id)
                .await?
                .context("updated schema but could not reload it")
        })
    }

    fn promote(&self, _instance_id: &str, schema_id: &str) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.db.clone();
        let schema_id = schema_id.to_string();
        Box::pin(async move { crate::promote_schema_record(&db, &schema_id).await })
    }

    fn count_by_schema(
        &self,
        instance_id: &str,
        schema_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<i64>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let schema_id = schema_id.to_string();
        Box::pin(async move { crate::count_users_for_schema(&db, &instance_id, &schema_id).await })
    }
}

impl SettingsRepository for SqlSettingsRepository {
    fn get(
        &self,
        instance_id: &str,
        settings_type: &str,
        scope: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SettingsRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let settings_type = settings_type.to_string();
        let scope = scope.to_string();
        Box::pin(async move {
            let (scope_kind, scope_id) = parse_scope(&scope);
            load_settings_exact(&db, &instance_id, &settings_type, &scope_kind, &scope_id).await
        })
    }

    fn set(
        &self,
        instance_id: &str,
        settings: &SettingsRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let settings = settings.clone();
        Box::pin(async move {
            let (scope, scope_id) = parse_scope(&settings.scope);
            let data_json = json_string(&settings.data)?;
            let id = uuid::Uuid::now_v7().to_string();
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) \
                         VALUES ($1, $2, $3, $4, $5, {}) \
                         ON CONFLICT(instance_id, type, scope, scope_id) DO UPDATE SET \
                            data = excluded.data, updated_at = CURRENT_TIMESTAMP",
                        scoped.json_bind(6),
                    );
                    sqlx::query(&sql)
                        .bind(&id)
                        .bind(&instance_id)
                        .bind(&settings.settings_type)
                        .bind(&scope)
                        .bind(&scope_id)
                        .bind(&data_json)
                        .execute(scoped.pool())
                        .await?;
                }
                Db::Spanner(spanner) => {
                    let existing = load_settings_exact(
                        &db,
                        &instance_id,
                        &settings.settings_type,
                        &scope,
                        &scope_id,
                    )
                    .await?;
                    if existing.is_some() {
                        let mut stmt = Statement::new(
                            "UPDATE settings SET data = @data, updated_at = CURRENT_TIMESTAMP() \
                             WHERE instance_id = @instance_id AND type = @type AND scope = @scope AND scope_id = @scope_id",
                        );
                        stmt.add_param("data", &data_json);
                        stmt.add_param("instance_id", &instance_id);
                        stmt.add_param("type", &settings.settings_type);
                        stmt.add_param("scope", &scope);
                        stmt.add_param("scope_id", &scope_id);
                        let _ = write_spanner_count(spanner, stmt).await?;
                    } else {
                        let mut stmt = Statement::new(
                            "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) \
                             VALUES (@id, @instance_id, @type, @scope, @scope_id, @data)",
                        );
                        stmt.add_param("id", &id);
                        stmt.add_param("instance_id", &instance_id);
                        stmt.add_param("type", &settings.settings_type);
                        stmt.add_param("scope", &scope);
                        stmt.add_param("scope_id", &scope_id);
                        stmt.add_param("data", &data_json);
                        write_spanner_stmt(spanner, stmt).await?;
                    }
                }
            }
            Ok(())
        })
    }

    fn delete(&self, instance_id: &str, settings_type: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let settings_type = settings_type.to_string();
        Box::pin(
            async move { crate::delete_settings_record(&db, &instance_id, &settings_type).await },
        )
    }

    fn resolve(
        &self,
        instance_id: &str,
        settings_type: &str,
        org_id: Option<&str>,
        app_id: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<SettingsRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let settings_type = settings_type.to_string();
        let org_id = org_id.map(str::to_string);
        let app_id = app_id.map(str::to_string);
        Box::pin(async move {
            if let Some(app_id) = app_id.as_deref()
                && let Some(record) =
                    load_settings_exact(&db, &instance_id, &settings_type, "app", app_id).await?
            {
                return Ok(record);
            }
            if let Some(org_id) = org_id.as_deref()
                && let Some(record) =
                    load_settings_exact(&db, &instance_id, &settings_type, "org", org_id).await?
            {
                return Ok(record);
            }
            if let Some(record) =
                load_settings_exact(&db, &instance_id, &settings_type, "instance", "").await?
            {
                return Ok(record);
            }
            Ok(SettingsRecord {
                settings_type,
                scope: "instance".to_string(),
                data: Value::Object(Map::new()),
            })
        })
    }
}
