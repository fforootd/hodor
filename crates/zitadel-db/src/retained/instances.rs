use std::collections::BTreeMap;

use google_cloud_spanner::{client::Error as SpannerError, mutation::insert, statement::Statement};

use super::{
    ChildInstanceOwnershipRecord, ConsoleBootstrapData, CreateManagedInstanceInput,
    DomainDeleteOutcome, DomainRecord, InstanceMetadata, ManagedInstancePatch,
    ManagedInstanceRecord, NamedResourceRecord, OrgSummary, RouteResolutionRecord,
    instance_from_spanner_row, instance_from_sql_row, spanner_query_all, spanner_query_optional,
    spanner_query_scalar_i64,
};
use crate::Db;

pub async fn load_instance_metadata(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Option<InstanceMetadata>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let feature_overrides = scoped.as_text("feature_overrides");
            let sql = format!(
                "SELECT instance_id, kind, parent_instance_id, COALESCE({feature_overrides}, '{{}}') \
                 FROM instances WHERE instance_id = $1"
            );
            let row: Option<(String, String, Option<String>, String)> = sqlx::query_as(&sql)
                .bind(scoped.instance_id())
                .fetch_optional(scoped.pool())
                .await?;
            Ok(row.map(
                |(instance_id, kind, parent_instance_id, feature_overrides_json)| {
                    InstanceMetadata {
                        instance_id,
                        kind,
                        parent_instance_id,
                        feature_overrides_json,
                    }
                },
            ))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT instance_id, kind, parent_instance_id, IFNULL(feature_overrides, '{}') AS feature_overrides \
                 FROM instances WHERE instance_id = @instance_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| InstanceMetadata {
                    instance_id: row
                        .column_by_name::<String>("instance_id")
                        .unwrap_or_default(),
                    kind: row.column_by_name::<String>("kind").unwrap_or_default(),
                    parent_instance_id: row
                        .column_by_name::<Option<String>>("parent_instance_id")
                        .unwrap_or(None),
                    feature_overrides_json: row
                        .column_by_name::<String>("feature_overrides")
                        .unwrap_or_else(|_| "{}".to_string()),
                }))
        }
    }
}

pub async fn load_console_bootstrap_data(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<ConsoleBootstrapData> {
    let mut counts = load_entity_counts(db, instance_id).await?;
    let child_count = count_child_instances(db, instance_id).await?;
    counts.insert("instance".to_string(), child_count);

    let orgs = list_orgs(db, instance_id, 50).await?;
    let instance = load_instance_metadata(db, instance_id)
        .await?
        .unwrap_or(InstanceMetadata {
            instance_id: instance_id.to_string(),
            kind: "managed".to_string(),
            parent_instance_id: None,
            feature_overrides_json: "{}".to_string(),
        });

    Ok(ConsoleBootstrapData {
        counts,
        orgs,
        instance,
    })
}

pub async fn load_entity_counts(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<BTreeMap<String, i64>> {
    let queries = [
        (
            "human_user",
            "SELECT COUNT(*) AS total FROM users WHERE instance_id = @instance_id AND user_type = 'human'",
            "SELECT COUNT(*) FROM users WHERE instance_id = $1 AND user_type = 'human'",
        ),
        (
            "service_user",
            "SELECT COUNT(*) AS total FROM users WHERE instance_id = @instance_id AND user_type = 'service'",
            "SELECT COUNT(*) FROM users WHERE instance_id = $1 AND user_type = 'service'",
        ),
        (
            "ai_agent",
            "SELECT COUNT(*) AS total FROM users WHERE instance_id = @instance_id AND user_type = 'ai_agent'",
            "SELECT COUNT(*) FROM users WHERE instance_id = $1 AND user_type = 'ai_agent'",
        ),
        (
            "org",
            "SELECT COUNT(*) AS total FROM orgs WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM orgs WHERE instance_id = $1",
        ),
        (
            "group",
            "SELECT COUNT(*) AS total FROM groups WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM groups WHERE instance_id = $1",
        ),
        (
            "project",
            "SELECT COUNT(*) AS total FROM projects WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM projects WHERE instance_id = $1",
        ),
        (
            "app",
            "SELECT COUNT(*) AS total FROM apps WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM apps WHERE instance_id = $1",
        ),
        (
            "provider",
            "SELECT COUNT(*) AS total FROM providers WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM providers WHERE instance_id = $1",
        ),
        (
            "login_flow",
            "SELECT COUNT(*) AS total FROM login_flows WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM login_flows WHERE instance_id = $1",
        ),
    ];

    let mut counts = BTreeMap::new();
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            for (name, _, sql) in queries {
                let count: i64 = sqlx::query_as::<_, (i64,)>(sql)
                    .bind(scoped.instance_id())
                    .fetch_one(scoped.pool())
                    .await
                    .map(|row| row.0)
                    .unwrap_or(0);
                counts.insert(name.to_string(), count);
            }
        }
        Db::Spanner(spanner) => {
            for (name, sql, _) in queries {
                let mut stmt = Statement::new(sql);
                stmt.add_param("instance_id", &instance_id);
                let count = spanner_query_scalar_i64(spanner, stmt).await.unwrap_or(0);
                counts.insert(name.to_string(), count);
            }
        }
    }
    Ok(counts)
}

pub async fn list_admin_instances(
    db: &Db,
    root_instance_id: &str,
    after_instance_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<ManagedInstanceRecord>> {
    list_managed_instances(db, root_instance_id, None, after_instance_id, limit).await
}

pub async fn list_managed_instances(
    db: &Db,
    root_instance_id: &str,
    owner_filter: Option<&str>,
    after_instance_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<ManagedInstanceRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(root_instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let feature_overrides = scoped.as_text("i.feature_overrides");
            let base = format!(
                "SELECT i.instance_id, i.state, i.kind, i.placement_mode, i.region_key, \
                 i.owner_org_id, COALESCE({feature_overrides}, '{{}}'), \
                 {created_at}, {updated_at}, d.domain AS primary_domain \
                 FROM instances i \
                 LEFT JOIN domains d ON d.instance_id = i.instance_id AND d.org_id IS NULL AND d.is_primary = 1 \
                 WHERE i.parent_instance_id = $1",
                created_at = created_at.replace("created_at", "i.created_at"),
                updated_at = updated_at.replace("updated_at", "i.updated_at"),
            );
            let sql = if owner_filter.is_some() {
                format!(
                    "{base} AND i.owner_org_id = $2 AND i.instance_id > $3 ORDER BY i.instance_id LIMIT $4"
                )
            } else {
                format!("{base} AND i.instance_id > $2 ORDER BY i.instance_id LIMIT $3")
            };

            let rows = if let Some(owner_org_id) = owner_filter {
                sqlx::query_as::<
                    _,
                    (
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                    ),
                >(&sql)
                .bind(root_instance_id)
                .bind(owner_org_id)
                .bind(after_instance_id)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?
            } else {
                sqlx::query_as::<
                    _,
                    (
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                    ),
                >(&sql)
                .bind(root_instance_id)
                .bind(after_instance_id)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?
            };

            Ok(rows.into_iter().map(instance_from_sql_row).collect())
        }
        Db::Spanner(spanner) => {
            let sql = if owner_filter.is_some() {
                "SELECT i.instance_id, i.state, i.kind, i.placement_mode, i.region_key, \
                        i.owner_org_id, IFNULL(i.feature_overrides, '{}') AS feature_overrides, \
                        CAST(i.created_at AS STRING) AS created_at, CAST(i.updated_at AS STRING) AS updated_at, \
                        d.domain AS primary_domain \
                 FROM instances i \
                 LEFT JOIN domains d ON d.instance_id = i.instance_id AND d.org_id IS NULL AND d.is_primary = TRUE \
                 WHERE i.parent_instance_id = @root_instance_id AND i.owner_org_id = @owner_org_id AND i.instance_id > @after_instance_id \
                 ORDER BY i.instance_id LIMIT @limit"
            } else {
                "SELECT i.instance_id, i.state, i.kind, i.placement_mode, i.region_key, \
                        i.owner_org_id, IFNULL(i.feature_overrides, '{}') AS feature_overrides, \
                        CAST(i.created_at AS STRING) AS created_at, CAST(i.updated_at AS STRING) AS updated_at, \
                        d.domain AS primary_domain \
                 FROM instances i \
                 LEFT JOIN domains d ON d.instance_id = i.instance_id AND d.org_id IS NULL AND d.is_primary = TRUE \
                 WHERE i.parent_instance_id = @root_instance_id AND i.instance_id > @after_instance_id \
                 ORDER BY i.instance_id LIMIT @limit"
            };
            let mut stmt = Statement::new(sql);
            stmt.add_param("root_instance_id", &root_instance_id);
            stmt.add_param("after_instance_id", &after_instance_id);
            stmt.add_param("limit", &limit);
            if let Some(owner_org_id) = owner_filter {
                stmt.add_param("owner_org_id", &owner_org_id);
            }
            let rows = spanner_query_all(spanner, stmt).await?;
            Ok(rows.into_iter().map(instance_from_spanner_row).collect())
        }
    }
}

pub async fn get_managed_instance(
    db: &Db,
    instance_id: &str,
    root_instance_id: &str,
    owner_filter: Option<&str>,
) -> anyhow::Result<Option<ManagedInstanceRecord>> {
    let mut rows = list_managed_instances(db, root_instance_id, owner_filter, "", i64::MAX).await?;
    Ok(rows
        .drain(..)
        .find(|record| record.instance_id == instance_id))
}

pub async fn create_managed_instance(
    db: &Db,
    input: &CreateManagedInstanceInput,
) -> anyhow::Result<ManagedInstanceRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(input.root_instance_id.clone());
            let mut tx = scoped.pool().begin().await?;
            sqlx::query(
                "INSERT INTO instances (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, region_key, feature_overrides) \
                 VALUES ($1, $2, $3, $4, 'active', $5, $6, '{}')",
            )
            .bind(&input.instance_id)
            .bind(&input.root_instance_id)
            .bind(&input.owner_org_id)
            .bind(&input.kind)
            .bind(&input.placement_mode)
            .bind(&input.region_key)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                 VALUES ($1, $2, 1, 'active', 0)",
            )
            .bind(&input.primary_domain)
            .bind(&input.instance_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }
        Db::Spanner(spanner) => {
            let region_key = input.region_key.clone();
            let mutations = vec![
                insert(
                    "instances",
                    &[
                        "instance_id",
                        "parent_instance_id",
                        "owner_org_id",
                        "kind",
                        "state",
                        "placement_mode",
                        "region_key",
                        "feature_overrides",
                    ],
                    &[
                        &input.instance_id,
                        &input.root_instance_id,
                        &input.owner_org_id,
                        &input.kind,
                        &"active",
                        &input.placement_mode,
                        &region_key,
                        &"{}",
                    ],
                ),
                insert(
                    "domains",
                    &["domain", "instance_id", "is_primary", "state", "verified"],
                    &[
                        &input.primary_domain,
                        &input.instance_id,
                        &true,
                        &"active",
                        &false,
                    ],
                ),
            ];
            spanner.client().apply(mutations).await?;
        }
    }

    get_managed_instance(
        db,
        &input.instance_id,
        &input.root_instance_id,
        Some(&input.owner_org_id),
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("created instance but could not reload it"))
}

pub async fn update_managed_instance(
    db: &Db,
    instance_id: &str,
    root_instance_id: &str,
    owner_filter: Option<&str>,
    patch: &ManagedInstancePatch,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(root_instance_id.to_string());
            let mut sets = Vec::new();
            let mut binds: Vec<String> = Vec::new();
            let mut next = 1usize;

            if let Some(state) = &patch.state {
                sets.push(format!("state = ${next}"));
                binds.push(state.clone());
                next += 1;
            }
            if let Some(placement_mode) = &patch.placement_mode {
                sets.push(format!("placement_mode = ${next}"));
                binds.push(placement_mode.clone());
                next += 1;
            }
            if let Some(region_key) = &patch.region_key {
                sets.push(format!("region_key = ${next}"));
                binds.push(region_key.clone());
                next += 1;
            }
            if let Some(feature_overrides_json) = &patch.feature_overrides_json {
                sets.push(format!("feature_overrides = {}", scoped.json_bind(next)));
                binds.push(feature_overrides_json.clone());
                next += 1;
            }
            if sets.is_empty() {
                return Ok(false);
            }
            sets.push("updated_at = CURRENT_TIMESTAMP".to_string());

            let sql = if owner_filter.is_some() {
                format!(
                    "UPDATE instances SET {} WHERE instance_id = ${} AND parent_instance_id = ${} AND owner_org_id = ${}",
                    sets.join(", "),
                    next,
                    next + 1,
                    next + 2,
                )
            } else {
                format!(
                    "UPDATE instances SET {} WHERE instance_id = ${} AND parent_instance_id = ${}",
                    sets.join(", "),
                    next,
                    next + 1,
                )
            };

            let mut query = sqlx::query(&sql);
            for bind in &binds {
                query = query.bind(bind);
            }
            query = query.bind(instance_id).bind(root_instance_id);
            if let Some(owner_org_id) = owner_filter {
                query = query.bind(owner_org_id);
            }
            Ok(query.execute(scoped.pool()).await?.rows_affected() > 0)
        }
        Db::Spanner(spanner) => {
            let mut sets = Vec::new();
            let mut stmt = Statement::new("");
            if let Some(state) = &patch.state {
                sets.push("state = @state");
                stmt.add_param("state", state);
            }
            if let Some(placement_mode) = &patch.placement_mode {
                sets.push("placement_mode = @placement_mode");
                stmt.add_param("placement_mode", placement_mode);
            }
            if let Some(region_key) = &patch.region_key {
                sets.push("region_key = @region_key");
                stmt.add_param("region_key", region_key);
            }
            if let Some(feature_overrides_json) = &patch.feature_overrides_json {
                sets.push("feature_overrides = @feature_overrides");
                stmt.add_param("feature_overrides", feature_overrides_json);
            }
            if sets.is_empty() {
                return Ok(false);
            }
            sets.push("updated_at = CURRENT_TIMESTAMP()");
            let mut sql = format!(
                "UPDATE instances SET {} WHERE instance_id = @instance_id AND parent_instance_id = @root_instance_id",
                sets.join(", ")
            );
            if owner_filter.is_some() {
                sql.push_str(" AND owner_org_id = @owner_org_id");
            }
            stmt = Statement::new(sql);
            if let Some(state) = &patch.state {
                stmt.add_param("state", state);
            }
            if let Some(placement_mode) = &patch.placement_mode {
                stmt.add_param("placement_mode", placement_mode);
            }
            if let Some(region_key) = &patch.region_key {
                stmt.add_param("region_key", region_key);
            }
            if let Some(feature_overrides_json) = &patch.feature_overrides_json {
                stmt.add_param("feature_overrides", feature_overrides_json);
            }
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("root_instance_id", &root_instance_id);
            if let Some(owner_org_id) = owner_filter {
                stmt.add_param("owner_org_id", &owner_org_id);
            }

            let (_, affected) = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
            Ok(affected > 0)
        }
    }
}

pub async fn deprovision_managed_instance(
    db: &Db,
    instance_id: &str,
    root_instance_id: &str,
    owner_filter: Option<&str>,
) -> anyhow::Result<bool> {
    let patch = ManagedInstancePatch {
        state: Some("deprovisioning".to_string()),
        ..ManagedInstancePatch::default()
    };
    update_managed_instance(db, instance_id, root_instance_id, owner_filter, &patch).await
}

pub async fn instance_visible(
    db: &Db,
    instance_id: &str,
    root_instance_id: &str,
    owner_filter: Option<&str>,
) -> anyhow::Result<bool> {
    Ok(
        get_managed_instance(db, instance_id, root_instance_id, owner_filter)
            .await?
            .is_some(),
    )
}

pub async fn list_active_child_instance_ownerships(
    db: &Db,
    root_instance_id: &str,
) -> anyhow::Result<Vec<ChildInstanceOwnershipRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(root_instance_id.to_string());
            let rows = sqlx::query_as::<_, (String, String)>(
                "SELECT instance_id, owner_org_id FROM instances \
                 WHERE parent_instance_id = $1 AND state = 'active' \
                 ORDER BY instance_id",
            )
            .bind(root_instance_id)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|(instance_id, owner_org_id)| ChildInstanceOwnershipRecord {
                    instance_id,
                    owner_org_id,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT instance_id, owner_org_id FROM instances \
                 WHERE parent_instance_id = @root_instance_id AND state = 'active' \
                 ORDER BY instance_id",
            );
            stmt.add_param("root_instance_id", &root_instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| ChildInstanceOwnershipRecord {
                    instance_id: row
                        .column_by_name::<String>("instance_id")
                        .unwrap_or_default(),
                    owner_org_id: row
                        .column_by_name::<String>("owner_org_id")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn list_instance_domains(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Vec<DomainRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT domain, {}, state, {}, {created_at}, {updated_at} \
                 FROM domains WHERE instance_id = $1 AND org_id IS NULL \
                 ORDER BY is_primary DESC, domain",
                scoped.bool_as_int("is_primary"),
                scoped.bool_as_int("verified"),
            );
            let rows: Vec<(String, i32, String, i32, String, String)> = sqlx::query_as(&sql)
                .bind(instance_id)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| DomainRecord {
                    domain: row.0,
                    is_primary: row.1 != 0,
                    state: row.2,
                    verified: row.3 != 0,
                    created_at: row.4,
                    updated_at: row.5,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT domain, is_primary, state, verified, CAST(created_at AS STRING) AS created_at, \
                        CAST(updated_at AS STRING) AS updated_at \
                 FROM domains WHERE instance_id = @instance_id AND org_id IS NULL \
                 ORDER BY is_primary DESC, domain",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| DomainRecord {
                    domain: row.column_by_name::<String>("domain").unwrap_or_default(),
                    is_primary: row.column_by_name::<bool>("is_primary").unwrap_or(false),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    verified: row.column_by_name::<bool>("verified").unwrap_or(false),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                    updated_at: row
                        .column_by_name::<String>("updated_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn add_instance_domain(
    db: &Db,
    instance_id: &str,
    domain: &str,
) -> anyhow::Result<DomainRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                 VALUES ($1, $2, 0, 'active', 0)",
            )
            .bind(domain)
            .bind(instance_id)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mutation = insert(
                "domains",
                &["domain", "instance_id", "is_primary", "state", "verified"],
                &[&domain, &instance_id, &false, &"active", &false],
            );
            spanner.client().apply(vec![mutation]).await?;
        }
    }

    let items = list_instance_domains(db, instance_id).await?;
    items
        .into_iter()
        .find(|item| item.domain == domain)
        .ok_or_else(|| anyhow::anyhow!("created domain but could not reload it"))
}

pub async fn delete_instance_domain(
    db: &Db,
    instance_id: &str,
    domain: &str,
) -> anyhow::Result<DomainDeleteOutcome> {
    let current = match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "SELECT {}, state FROM domains WHERE domain = $1 AND instance_id = $2 AND org_id IS NULL",
                scoped.bool_as_int("is_primary"),
            );
            let row: Option<(i32, String)> = sqlx::query_as(&sql)
                .bind(domain)
                .bind(instance_id)
                .fetch_optional(scoped.pool())
                .await?;
            row.map(|(is_primary, state)| (is_primary != 0, state))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT is_primary, state FROM domains \
                 WHERE domain = @domain AND instance_id = @instance_id AND org_id IS NULL LIMIT 1",
            );
            stmt.add_param("domain", &domain);
            stmt.add_param("instance_id", &instance_id);
            spanner_query_optional(spanner, stmt).await?.map(|row| {
                (
                    row.column_by_name::<bool>("is_primary").unwrap_or(false),
                    row.column_by_name::<String>("state").unwrap_or_default(),
                )
            })
        }
    };

    let Some((is_primary, _)) = current else {
        return Ok(DomainDeleteOutcome::NotFound);
    };
    if is_primary {
        return Ok(DomainDeleteOutcome::PrimaryDomain);
    }

    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "DELETE FROM domains WHERE domain = $1 AND instance_id = $2 AND org_id IS NULL",
            )
            .bind(domain)
            .bind(instance_id)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "DELETE FROM domains WHERE domain = @domain AND instance_id = @instance_id AND org_id IS NULL",
            );
            stmt.add_param("domain", &domain);
            stmt.add_param("instance_id", &instance_id);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move {
                        tx.update(stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
        }
    }
    Ok(DomainDeleteOutcome::Deleted)
}

pub async fn delete_instance_row(
    db: &Db,
    instance_id: &str,
    table: &'static str,
    id: &str,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!("DELETE FROM {table} WHERE instance_id = $1 AND id = $2");
            Ok(sqlx::query(&sql)
                .bind(instance_id)
                .bind(id)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut exists_stmt = Statement::new(&format!(
                "SELECT id FROM {table} WHERE instance_id = @instance_id AND id = @id LIMIT 1"
            ));
            exists_stmt.add_param("instance_id", &instance_id);
            exists_stmt.add_param("id", &id);
            if spanner_query_optional(spanner, exists_stmt)
                .await?
                .is_none()
            {
                return Ok(false);
            }
            let mut delete_stmt = Statement::new(&format!(
                "DELETE FROM {table} WHERE instance_id = @instance_id AND id = @id"
            ));
            delete_stmt.add_param("instance_id", &instance_id);
            delete_stmt.add_param("id", &id);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = delete_stmt.clone();
                    Box::pin(async move {
                        tx.update(stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
            Ok(true)
        }
    }
}

pub async fn create_named_resource(
    db: &Db,
    instance_id: &str,
    table: &'static str,
    id: &str,
    name: &str,
    org_id: &str,
) -> anyhow::Result<NamedResourceRecord> {
    fn app_client_id(name: &str) -> String {
        let mut slug = name
            .trim()
            .to_lowercase()
            .chars()
            .map(|char| {
                if char.is_ascii_alphanumeric() {
                    char
                } else {
                    '-'
                }
            })
            .collect::<String>();
        while slug.contains("--") {
            slug = slug.replace("--", "-");
        }
        let slug = slug.trim_matches('-').to_string();
        if slug.is_empty() {
            "app".to_string()
        } else {
            slug
        }
    }

    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            if table == "apps" {
                let client_id = app_client_id(name);
                let sql = format!(
                    "INSERT INTO apps \
                     (id, instance_id, org_id, name, app_type, client_id, client_secret, redirect_uris, post_logout_redirect_uris, grant_types, response_types, state) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, {}, {}, {}, {}, 'active')",
                    scoped.json_bind(8),
                    scoped.json_bind(9),
                    scoped.json_bind(10),
                    scoped.json_bind(11),
                );
                sqlx::query(&sql)
                    .bind(id)
                    .bind(instance_id)
                    .bind(org_id)
                    .bind(name)
                    .bind("web")
                    .bind(client_id)
                    .bind("")
                    .bind("[]")
                    .bind("[]")
                    .bind("[\"authorization_code\",\"refresh_token\"]")
                    .bind("[\"code\"]")
                    .execute(scoped.pool())
                    .await?;
            } else {
                let sql = format!(
                    "INSERT INTO {table} (id, instance_id, org_id, name, state) VALUES ($1, $2, $3, $4, 'active')"
                );
                sqlx::query(&sql)
                    .bind(id)
                    .bind(instance_id)
                    .bind(org_id)
                    .bind(name)
                    .execute(scoped.pool())
                    .await?;
            }
        }
        Db::Spanner(spanner) => {
            let mut stmt = if table == "apps" {
                let client_id = app_client_id(name);
                let mut stmt = Statement::new(
                    "INSERT INTO apps \
                     (id, instance_id, org_id, name, app_type, client_id, client_secret, redirect_uris, post_logout_redirect_uris, grant_types, response_types, state) \
                     VALUES (@id, @instance_id, @org_id, @name, @app_type, @client_id, @client_secret, @redirect_uris, @post_logout_redirect_uris, @grant_types, @response_types, 'active')",
                );
                stmt.add_param("app_type", &"web");
                stmt.add_param("client_id", &client_id);
                stmt.add_param("client_secret", &"");
                stmt.add_param("redirect_uris", &"[]");
                stmt.add_param("post_logout_redirect_uris", &"[]");
                stmt.add_param("grant_types", &"[\"authorization_code\",\"refresh_token\"]");
                stmt.add_param("response_types", &"[\"code\"]");
                stmt
            } else {
                Statement::new(&format!(
                    "INSERT INTO {table} (id, instance_id, org_id, name, state) \
                     VALUES (@id, @instance_id, @org_id, @name, 'active')"
                ))
            };
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("org_id", &org_id);
            stmt.add_param("name", &name);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move {
                        tx.update(stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
        }
    }

    get_named_resource(db, instance_id, table, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("created {table} row but could not reload it"))
}

pub async fn get_named_resource(
    db: &Db,
    instance_id: &str,
    table: &'static str,
    id: &str,
) -> anyhow::Result<Option<NamedResourceRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, name, state, {created_at}, {updated_at} \
                 FROM {table} WHERE instance_id = $1 AND id = $2"
            );
            Ok(
                sqlx::query_as::<_, (String, String, String, String, String)>(&sql)
                    .bind(instance_id)
                    .bind(id)
                    .fetch_optional(scoped.pool())
                    .await?
                    .map(|row| NamedResourceRecord {
                        id: row.0,
                        name: row.1,
                        state: row.2,
                        created_at: row.3,
                        updated_at: row.4,
                    }),
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(&format!(
                "SELECT id, name, state, CAST(created_at AS STRING) AS created_at, \
                        CAST(updated_at AS STRING) AS updated_at \
                 FROM {table} WHERE instance_id = @instance_id AND id = @id LIMIT 1"
            ));
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| NamedResourceRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                    updated_at: row
                        .column_by_name::<String>("updated_at")
                        .unwrap_or_default(),
                }))
        }
    }
}

pub async fn list_named_resources(
    db: &Db,
    instance_id: &str,
    table: &'static str,
    after_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<NamedResourceRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, name, state, {created_at}, {updated_at} \
                 FROM {table} WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
            );
            let rows = sqlx::query_as::<_, (String, String, String, String, String)>(&sql)
                .bind(instance_id)
                .bind(after_id)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| NamedResourceRecord {
                    id: row.0,
                    name: row.1,
                    state: row.2,
                    created_at: row.3,
                    updated_at: row.4,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(&format!(
                "SELECT id, name, state, CAST(created_at AS STRING) AS created_at, \
                        CAST(updated_at AS STRING) AS updated_at \
                 FROM {table} WHERE instance_id = @instance_id AND id > @after_id \
                 ORDER BY id LIMIT @limit"
            ));
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| NamedResourceRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                    updated_at: row
                        .column_by_name::<String>("updated_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn update_named_resource_name(
    db: &Db,
    instance_id: &str,
    table: &'static str,
    id: &str,
    name: &str,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "UPDATE {table} SET name = $1, updated_at = CURRENT_TIMESTAMP \
                 WHERE instance_id = $2 AND id = $3"
            );
            Ok(sqlx::query(&sql)
                .bind(name)
                .bind(instance_id)
                .bind(id)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(&format!(
                "UPDATE {table} SET name = @name, updated_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND id = @id"
            ));
            stmt.add_param("name", &name);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            let (_, affected) = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
            Ok(affected > 0)
        }
    }
}

pub async fn resolve_domain_route(
    db: &Db,
    host: &str,
) -> anyhow::Result<Option<RouteResolutionRecord>> {
    match db {
        Db::Sql(_) => {
            let row = sqlx::query_as::<_, (String, Option<String>, String, Option<String>)>(
                "SELECT d.instance_id, d.org_id AS resolved_org_id, i.placement_mode, \
                        NULLIF(i.region_key, '') AS region_key \
                 FROM domains d \
                 JOIN instances i ON i.instance_id = d.instance_id \
                 WHERE d.domain = $1 AND d.state = 'active' AND i.state = 'active' \
                 ORDER BY d.is_primary DESC, d.updated_at DESC LIMIT 1",
            )
            .bind(host)
            .fetch_optional(db.pool())
            .await?;
            Ok(row.map(|row| RouteResolutionRecord {
                instance_id: row.0,
                resolved_org_id: row.1,
                placement_mode: row.2,
                region_key: row.3,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT d.instance_id, d.org_id AS resolved_org_id, i.placement_mode, i.region_key \
                 FROM domains d \
                 JOIN instances i ON i.instance_id = d.instance_id \
                 WHERE d.domain = @host AND d.state = 'active' AND i.state = 'active' \
                 ORDER BY d.is_primary DESC, d.updated_at DESC LIMIT 1",
            );
            stmt.add_param("host", &host);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| RouteResolutionRecord {
                    instance_id: row
                        .column_by_name::<String>("instance_id")
                        .unwrap_or_default(),
                    resolved_org_id: row
                        .column_by_name::<Option<String>>("resolved_org_id")
                        .unwrap_or(None),
                    placement_mode: row
                        .column_by_name::<String>("placement_mode")
                        .unwrap_or_default(),
                    region_key: row
                        .column_by_name::<Option<String>>("region_key")
                        .unwrap_or(None)
                        .filter(|value| !value.is_empty()),
                }))
        }
    }
}

pub async fn resolve_instance_route(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Option<RouteResolutionRecord>> {
    match db {
        Db::Sql(_) => {
            let row = sqlx::query_as::<_, (String, String, Option<String>)>(
                "SELECT i.instance_id, i.placement_mode, NULLIF(i.region_key, '') AS region_key \
                 FROM instances i \
                 WHERE i.instance_id = $1 AND i.state = 'active' LIMIT 1",
            )
            .bind(instance_id)
            .fetch_optional(db.pool())
            .await?;
            Ok(row.map(|row| RouteResolutionRecord {
                instance_id: row.0,
                resolved_org_id: None,
                placement_mode: row.1,
                region_key: row.2,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT i.instance_id, i.placement_mode, i.region_key \
                 FROM instances i \
                 WHERE i.instance_id = @instance_id AND i.state = 'active' LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| RouteResolutionRecord {
                    instance_id: row
                        .column_by_name::<String>("instance_id")
                        .unwrap_or_default(),
                    resolved_org_id: None,
                    placement_mode: row
                        .column_by_name::<String>("placement_mode")
                        .unwrap_or_default(),
                    region_key: row
                        .column_by_name::<Option<String>>("region_key")
                        .unwrap_or(None)
                        .filter(|value| !value.is_empty()),
                }))
        }
    }
}

// ─── Private helpers ───

async fn count_child_instances(db: &Db, instance_id: &str) -> anyhow::Result<i64> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM instances WHERE parent_instance_id = $1",
            )
            .bind(instance_id)
            .fetch_one(scoped.pool())
            .await?
            .0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT COUNT(*) AS total FROM instances WHERE parent_instance_id = @instance_id",
            );
            stmt.add_param("instance_id", &instance_id);
            spanner_query_scalar_i64(spanner, stmt).await
        }
    }
}

async fn list_orgs(db: &Db, instance_id: &str, limit: i64) -> anyhow::Result<Vec<OrgSummary>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let rows: Vec<(String, String, String)> = sqlx::query_as(
                "SELECT id, name, state FROM orgs WHERE instance_id = $1 ORDER BY name LIMIT $2",
            )
            .bind(instance_id)
            .bind(limit)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|row| OrgSummary {
                    id: row.0,
                    name: row.1,
                    state: row.2,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name, state FROM orgs \
                 WHERE instance_id = @instance_id ORDER BY name LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| OrgSummary {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                })
                .collect())
        }
    }
}
