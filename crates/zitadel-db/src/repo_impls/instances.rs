use anyhow::Context;
use google_cloud_spanner::statement::Statement;

use super::entities::{
    SqlInstanceRepository, domain_from_retained, instance_from_retained, json_string,
    limit_from_params, load_instance, next_cursor, upsert_domain, write_spanner_count,
    write_spanner_many,
};
use crate::{
    Db, list_instance_domains, list_instance_domains_filtered, list_managed_instances,
    resolve_domain_route,
};
use zitadel_app::repo::{
    BoxFuture, DomainRecord, DomainRemoveResult, InstanceRecord, InstanceRepository, ListParams,
    ListResult, RouteResolution,
};

impl InstanceRepository for SqlInstanceRepository {
    fn create(
        &self,
        root_instance_id: &str,
        instance: &InstanceRecord,
    ) -> BoxFuture<'_, anyhow::Result<InstanceRecord>> {
        let db = self.db.clone();
        let root_instance_id = root_instance_id.to_string();
        let instance = instance.clone();
        Box::pin(async move {
            let feature_overrides_json = json_string(&instance.feature_overrides)?;
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(root_instance_id.clone());
                    let mut tx = scoped.pool().begin().await?;
                    let sql = format!(
                        "INSERT INTO instances \
                         (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, region_key, feature_overrides) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, {})",
                        scoped.json_bind(8),
                    );
                    sqlx::query(&sql)
                        .bind(&instance.instance_id)
                        .bind(&root_instance_id)
                        .bind(&instance.owner_org_id)
                        .bind(&instance.kind)
                        .bind(&instance.state)
                        .bind(&instance.placement_mode)
                        .bind(&instance.region_key)
                        .bind(&feature_overrides_json)
                        .execute(&mut *tx)
                        .await?;
                    if let Some(primary_domain) = &instance.primary_domain {
                        sqlx::query(
                            "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                             VALUES ($1, $2, TRUE, 'active', FALSE)",
                        )
                        .bind(primary_domain)
                        .bind(&instance.instance_id)
                        .execute(&mut *tx)
                        .await?;
                    }
                    tx.commit().await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmts = vec![{
                        let mut stmt = Statement::new(
                            "INSERT INTO instances \
                             (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, region_key, feature_overrides) \
                             VALUES (@instance_id, @parent_instance_id, @owner_org_id, @kind, @state, @placement_mode, @region_key, @feature_overrides)",
                        );
                        stmt.add_param("instance_id", &instance.instance_id);
                        stmt.add_param("parent_instance_id", &root_instance_id);
                        stmt.add_param("owner_org_id", &instance.owner_org_id);
                        stmt.add_param("kind", &instance.kind);
                        stmt.add_param("state", &instance.state);
                        stmt.add_param("placement_mode", &instance.placement_mode);
                        stmt.add_param("region_key", &instance.region_key);
                        stmt.add_param("feature_overrides", &feature_overrides_json);
                        stmt
                    }];
                    if let Some(primary_domain) = &instance.primary_domain {
                        let mut stmt = Statement::new(
                            "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                             VALUES (@domain, @instance_id, TRUE, 'active', FALSE)",
                        );
                        stmt.add_param("domain", primary_domain);
                        stmt.add_param("instance_id", &instance.instance_id);
                        stmts.push(stmt);
                    }
                    write_spanner_many(spanner, stmts).await?;
                }
            }
            load_instance(&db, &instance.instance_id)
                .await?
                .context("created instance but could not reload it")
        })
    }

    fn get(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Option<InstanceRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move { load_instance(&db, &instance_id).await })
    }

    fn list(
        &self,
        root_instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<InstanceRecord>>> {
        let db = self.db.clone();
        let root_instance_id = root_instance_id.to_string();
        let params = params.clone();
        Box::pin(async move {
            let limit = limit_from_params(&params);
            let cursor = params.cursor.unwrap_or_default();
            let search = params.search.map(|value| value.to_lowercase());
            let mut items: Vec<InstanceRecord> =
                list_managed_instances(&db, &root_instance_id, None, &cursor, limit)
                    .await?
                    .into_iter()
                    .map(instance_from_retained)
                    .collect();

            if let Some(search) = search {
                items.retain(|item| {
                    item.instance_id.to_lowercase().contains(&search)
                        || item
                            .owner_org_id
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&search)
                        || item
                            .primary_domain
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&search)
                });
            }

            Ok(ListResult {
                next_cursor: next_cursor(&items, limit, |item| item.instance_id.as_str()),
                total_count: None,
                items,
            })
        })
    }

    fn update(&self, instance: &InstanceRecord) -> BoxFuture<'_, anyhow::Result<InstanceRecord>> {
        let db = self.db.clone();
        let instance = instance.clone();
        Box::pin(async move {
            let feature_overrides_json = json_string(&instance.feature_overrides)?;
            let updated = match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance.instance_id.clone());
                    let sql = format!(
                        "UPDATE instances \
                         SET owner_org_id = $1, kind = $2, state = $3, placement_mode = $4, \
                             region_key = $5, feature_overrides = {}, updated_at = CURRENT_TIMESTAMP \
                         WHERE instance_id = $7",
                        scoped.json_bind(6),
                    );
                    sqlx::query(&sql)
                        .bind(&instance.owner_org_id)
                        .bind(&instance.kind)
                        .bind(&instance.state)
                        .bind(&instance.placement_mode)
                        .bind(&instance.region_key)
                        .bind(&feature_overrides_json)
                        .bind(&instance.instance_id)
                        .execute(scoped.pool())
                        .await?
                        .rows_affected()
                        > 0
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "UPDATE instances \
                         SET owner_org_id = @owner_org_id, kind = @kind, state = @state, \
                             placement_mode = @placement_mode, region_key = @region_key, \
                             feature_overrides = @feature_overrides, updated_at = CURRENT_TIMESTAMP() \
                         WHERE instance_id = @instance_id",
                    );
                    stmt.add_param("owner_org_id", &instance.owner_org_id);
                    stmt.add_param("kind", &instance.kind);
                    stmt.add_param("state", &instance.state);
                    stmt.add_param("placement_mode", &instance.placement_mode);
                    stmt.add_param("region_key", &instance.region_key);
                    stmt.add_param("feature_overrides", &feature_overrides_json);
                    stmt.add_param("instance_id", &instance.instance_id);
                    write_spanner_count(spanner, stmt).await? > 0
                }
            };
            if !updated {
                anyhow::bail!("instance not found");
            }
            if let Some(primary_domain) = &instance.primary_domain {
                let domain = DomainRecord {
                    instance_id: instance.instance_id.clone(),
                    org_id: None,
                    domain: primary_domain.clone(),
                    is_primary: true,
                    purpose: "served".to_string(),
                    state: "active".to_string(),
                    verified: false,
                    verification_token: String::new(),
                    dns_challenge_host: String::new(),
                    dns_authorization_id: String::new(),
                    certificate_dns_record_name: String::new(),
                    certificate_dns_record_type: String::new(),
                    certificate_dns_record_value: String::new(),
                    certificate_state: String::new(),
                    certificate_id: String::new(),
                    certificate_map_entry: String::new(),
                    origin_trust_state: String::new(),
                    provisioning_error: String::new(),
                    created_at: String::new(),
                    updated_at: String::new(),
                };
                upsert_domain(&db, &instance.instance_id, &domain).await?;
            }
            load_instance(&db, &instance.instance_id)
                .await?
                .context("updated instance but could not reload it")
        })
    }

    fn deprovision(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    sqlx::query(
                        "UPDATE instances \
                         SET state = 'deprovisioning', updated_at = CURRENT_TIMESTAMP \
                         WHERE instance_id = $1",
                    )
                    .bind(&instance_id)
                    .execute(scoped.pool())
                    .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "UPDATE instances \
                         SET state = 'deprovisioning', updated_at = CURRENT_TIMESTAMP() \
                         WHERE instance_id = @instance_id",
                    );
                    stmt.add_param("instance_id", &instance_id);
                    let _ = write_spanner_count(spanner, stmt).await?;
                }
            }
            Ok(())
        })
    }

    fn resolve_domain(
        &self,
        domain: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<RouteResolution>>> {
        let db = self.db.clone();
        let domain = domain.to_string();
        Box::pin(async move {
            Ok(resolve_domain_route(&db, &domain)
                .await?
                .map(|row| RouteResolution {
                    instance_id: row.instance_id,
                    resolved_org_id: row.resolved_org_id,
                    placement_mode: row.placement_mode,
                    region_key: row.region_key,
                }))
        })
    }

    fn list_domains(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Vec<DomainRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            Ok(list_instance_domains(&db, &instance_id)
                .await?
                .into_iter()
                .map(domain_from_retained)
                .collect())
        })
    }

    fn set_domain(
        &self,
        instance_id: &str,
        domain: &DomainRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let domain = domain.clone();
        Box::pin(async move { upsert_domain(&db, &instance_id, &domain).await })
    }

    fn remove_domain(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
    ) -> BoxFuture<'_, anyhow::Result<DomainRemoveResult>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org_id = org_id.map(|value| value.to_string());
        let domain = domain.to_string();
        Box::pin(async move {
            match crate::delete_domain_for_scope(&db, &instance_id, org_id.as_deref(), &domain)
                .await?
            {
                crate::DomainDeleteOutcome::Deleted => Ok(DomainRemoveResult::Deleted),
                crate::DomainDeleteOutcome::NotFound => Ok(DomainRemoveResult::NotFound),
                crate::DomainDeleteOutcome::PrimaryDomain => Ok(DomainRemoveResult::PrimaryDomain),
            }
        })
    }

    fn find_domain(&self, domain: &str) -> BoxFuture<'_, anyhow::Result<Option<DomainRecord>>> {
        let db = self.db.clone();
        let domain = domain.to_string();
        Box::pin(async move {
            Ok(crate::find_domain(&db, &domain)
                .await?
                .map(domain_from_retained))
        })
    }

    fn get_domain(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<DomainRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org_id = org_id.map(|value| value.to_string());
        let domain = domain.to_string();
        Box::pin(async move {
            Ok(
                crate::get_domain_for_scope(&db, &instance_id, org_id.as_deref(), &domain)
                    .await?
                    .map(domain_from_retained),
            )
        })
    }

    fn update_domain_state(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
        new_state: &str,
        verified: bool,
        provisioning_error: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org_id = org_id.map(|value| value.to_string());
        let domain = domain.to_string();
        let new_state = new_state.to_string();
        let provisioning_error = provisioning_error.map(|value| value.to_string());
        Box::pin(async move {
            crate::update_domain_state_for_scope(
                &db,
                &instance_id,
                org_id.as_deref(),
                &domain,
                &new_state,
                verified,
                provisioning_error.as_deref(),
            )
            .await
        })
    }

    fn update_domain_certificate_state(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
        cert_state: &str,
        cert_id: &str,
        cert_map_entry: Option<&str>,
        dns_authorization_id: Option<&str>,
        dns_record_name: Option<&str>,
        dns_record_type: Option<&str>,
        dns_record_value: Option<&str>,
        provisioning_error: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org_id = org_id.map(|value| value.to_string());
        let domain = domain.to_string();
        let cert_state = cert_state.to_string();
        let cert_id = cert_id.to_string();
        let cert_map_entry = cert_map_entry.map(|value| value.to_string());
        let dns_authorization_id = dns_authorization_id.map(|value| value.to_string());
        let dns_record_name = dns_record_name.map(|value| value.to_string());
        let dns_record_type = dns_record_type.map(|value| value.to_string());
        let dns_record_value = dns_record_value.map(|value| value.to_string());
        let provisioning_error = provisioning_error.map(|value| value.to_string());
        Box::pin(async move {
            crate::update_domain_certificate_state_for_scope(
                &db,
                &instance_id,
                org_id.as_deref(),
                &domain,
                &cert_state,
                &cert_id,
                cert_map_entry.as_deref(),
                dns_authorization_id.as_deref(),
                dns_record_name.as_deref(),
                dns_record_type.as_deref(),
                dns_record_value.as_deref(),
                provisioning_error.as_deref(),
            )
            .await
        })
    }

    fn update_domain_origin_trust_state(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
        state: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org_id = org_id.map(|value| value.to_string());
        let domain = domain.to_string();
        let state = state.to_string();
        Box::pin(async move {
            crate::update_domain_origin_trust_state_for_scope(
                &db,
                &instance_id,
                org_id.as_deref(),
                &domain,
                &state,
            )
            .await
        })
    }

    fn list_domains_for_instance(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<Vec<DomainRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org_id = org_id.map(|s| s.to_string());
        Box::pin(async move {
            Ok(list_instance_domains_filtered(
                &db,
                &instance_id,
                org_id.as_deref(),
                org_id.is_none(),
            )
            .await?
            .into_iter()
            .map(domain_from_retained)
            .collect())
        })
    }
}
