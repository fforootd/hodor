use std::collections::BTreeMap;
use std::time::Duration;

use google_cloud_spanner::{
    client::Error as SpannerError,
    mutation::insert,
    statement::{Statement, ToKind},
};
use serde::{Deserialize, Serialize};

use super::{
    ChildInstanceOwnershipRecord, ConsoleBootstrapData, CreateManagedInstanceInput,
    DomainDeleteOutcome, DomainRecord, InstanceMetadata, ManagedInstancePatch,
    ManagedInstanceRecord, NamedResourceRecord, OrgSummary, RouteResolutionRecord,
    instance_from_spanner_row, instance_from_sql_row, spanner_query_all, spanner_query_optional,
    spanner_query_scalar_i64,
};
use crate::{Db, InstanceContext, spanner_ident};

#[derive(sqlx::FromRow)]
struct DomainSqlRow {
    instance_id: String,
    org_id: Option<String>,
    domain: String,
    is_primary: i64,
    purpose: String,
    state: String,
    verified: i64,
    verification_token: String,
    dns_challenge_host: String,
    dns_authorization_id: String,
    certificate_dns_record_name: String,
    certificate_dns_record_type: String,
    certificate_dns_record_value: String,
    certificate_state: String,
    certificate_id: String,
    certificate_map_entry: String,
    origin_trust_state: String,
    provisioning_error: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct InstanceRoutingCacheEnvelope {
    value: Option<InstanceContext>,
}

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
            "SELECT COUNT(*) AS total FROM `groups` WHERE instance_id = @instance_id",
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
    list_instance_domains_filtered(db, instance_id, None, true).await
}

/// List domains for an instance, optionally filtered by org_id.
/// When `org_null_only` is true, only returns instance-level domains (org_id IS NULL).
pub async fn list_instance_domains_filtered(
    db: &Db,
    instance_id: &str,
    org_id: Option<&str>,
    org_null_only: bool,
) -> anyhow::Result<Vec<DomainRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let org_filter = if org_id.is_some() {
                "AND org_id = $2"
            } else if org_null_only {
                "AND org_id IS NULL"
            } else {
                ""
            };
            let sql = format!(
                "SELECT instance_id, org_id, domain, {} AS is_primary, purpose, state, {} AS verified, \
                 verification_token, dns_challenge_host, dns_authorization_id, \
                 certificate_dns_record_name, certificate_dns_record_type, certificate_dns_record_value, \
                 certificate_state, certificate_id, certificate_map_entry, origin_trust_state, provisioning_error, \
                 {created_at} AS created_at, {updated_at} AS updated_at \
                 FROM domains WHERE instance_id = $1 {org_filter} \
                 ORDER BY is_primary DESC, domain",
                scoped.bool_as_int("is_primary"),
                scoped.bool_as_int("verified"),
            );
            let query = sqlx::query_as(&sql).bind(instance_id);
            let query = if let Some(oid) = org_id {
                query.bind(oid)
            } else {
                query
            };
            let rows: Vec<DomainSqlRow> = query.fetch_all(scoped.pool()).await?;
            Ok(rows.into_iter().map(domain_from_sql_row).collect())
        }
        Db::Spanner(spanner) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = @org_id"
            } else if org_null_only {
                "AND org_id IS NULL"
            } else {
                ""
            };
            let sql = format!(
                "SELECT instance_id, org_id, domain, is_primary, purpose, state, verified, \
                 verification_token, dns_challenge_host, dns_authorization_id, \
                 certificate_dns_record_name, certificate_dns_record_type, certificate_dns_record_value, \
                 certificate_state, certificate_id, certificate_map_entry, origin_trust_state, provisioning_error, \
                 CAST(created_at AS STRING) AS created_at, \
                 CAST(updated_at AS STRING) AS updated_at \
                 FROM domains WHERE instance_id = @instance_id {org_filter} \
                 ORDER BY is_primary DESC, domain"
            );
            let mut stmt = Statement::new(&sql);
            stmt.add_param("instance_id", &instance_id);
            if let Some(oid) = org_id {
                stmt.add_param("org_id", &oid);
            }
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| domain_from_spanner_row(&row))
                .collect())
        }
    }
}

fn domain_from_sql_row(row: DomainSqlRow) -> DomainRecord {
    DomainRecord {
        instance_id: row.instance_id,
        org_id: row.org_id,
        domain: row.domain,
        is_primary: row.is_primary != 0,
        purpose: row.purpose,
        state: row.state,
        verified: row.verified != 0,
        verification_token: row.verification_token,
        dns_challenge_host: row.dns_challenge_host,
        dns_authorization_id: row.dns_authorization_id,
        certificate_dns_record_name: row.certificate_dns_record_name,
        certificate_dns_record_type: row.certificate_dns_record_type,
        certificate_dns_record_value: row.certificate_dns_record_value,
        certificate_state: row.certificate_state,
        certificate_id: row.certificate_id,
        certificate_map_entry: row.certificate_map_entry,
        origin_trust_state: row.origin_trust_state,
        provisioning_error: row.provisioning_error,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn domain_from_spanner_row(row: &google_cloud_spanner::row::Row) -> DomainRecord {
    DomainRecord {
        instance_id: row
            .column_by_name::<String>("instance_id")
            .unwrap_or_default(),
        org_id: row
            .column_by_name::<Option<String>>("org_id")
            .unwrap_or(None),
        domain: row.column_by_name::<String>("domain").unwrap_or_default(),
        is_primary: row.column_by_name::<bool>("is_primary").unwrap_or(false),
        purpose: row
            .column_by_name::<String>("purpose")
            .unwrap_or_else(|_| "served".to_string()),
        state: row.column_by_name::<String>("state").unwrap_or_default(),
        verified: row.column_by_name::<bool>("verified").unwrap_or(false),
        verification_token: row
            .column_by_name::<String>("verification_token")
            .unwrap_or_default(),
        dns_challenge_host: row
            .column_by_name::<String>("dns_challenge_host")
            .unwrap_or_default(),
        dns_authorization_id: row
            .column_by_name::<String>("dns_authorization_id")
            .unwrap_or_default(),
        certificate_dns_record_name: row
            .column_by_name::<String>("certificate_dns_record_name")
            .unwrap_or_default(),
        certificate_dns_record_type: row
            .column_by_name::<String>("certificate_dns_record_type")
            .unwrap_or_default(),
        certificate_dns_record_value: row
            .column_by_name::<String>("certificate_dns_record_value")
            .unwrap_or_default(),
        certificate_state: row
            .column_by_name::<String>("certificate_state")
            .unwrap_or_default(),
        certificate_id: row
            .column_by_name::<String>("certificate_id")
            .unwrap_or_default(),
        certificate_map_entry: row
            .column_by_name::<String>("certificate_map_entry")
            .unwrap_or_default(),
        origin_trust_state: row
            .column_by_name::<String>("origin_trust_state")
            .unwrap_or_default(),
        provisioning_error: row
            .column_by_name::<String>("provisioning_error")
            .unwrap_or_default(),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
        updated_at: row
            .column_by_name::<String>("updated_at")
            .unwrap_or_default(),
    }
}

/// Find a single domain globally by name.
pub async fn find_domain(db: &Db, domain: &str) -> anyhow::Result<Option<DomainRecord>> {
    match db {
        Db::Sql(_) => {
            let sql = "SELECT instance_id, org_id, domain, \
                CASE WHEN is_primary THEN 1 ELSE 0 END AS is_primary, \
                purpose, state, \
                CASE WHEN verified THEN 1 ELSE 0 END AS verified, \
                verification_token, dns_challenge_host, dns_authorization_id, \
                certificate_dns_record_name, certificate_dns_record_type, certificate_dns_record_value, \
                certificate_state, certificate_id, certificate_map_entry, origin_trust_state, provisioning_error, \
                CAST(created_at AS TEXT) AS created_at, \
                CAST(updated_at AS TEXT) AS updated_at \
                FROM domains WHERE domain = $1 LIMIT 1";
            let row: Option<DomainSqlRow> = sqlx::query_as(sql)
                .bind(domain)
                .fetch_optional(db.pool())
                .await?;
            Ok(row.map(domain_from_sql_row))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT instance_id, org_id, domain, is_primary, purpose, state, verified, \
                 verification_token, dns_challenge_host, dns_authorization_id, \
                 certificate_dns_record_name, certificate_dns_record_type, certificate_dns_record_value, \
                 certificate_state, certificate_id, certificate_map_entry, origin_trust_state, provisioning_error, \
                 CAST(created_at AS STRING) AS created_at, \
                 CAST(updated_at AS STRING) AS updated_at \
                 FROM domains WHERE domain = @domain LIMIT 1",
            );
            stmt.add_param("domain", &domain);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| domain_from_spanner_row(&row)))
        }
    }
}

/// Get a single domain by name within a specific instance/org scope.
pub async fn get_domain_for_scope(
    db: &Db,
    instance_id: &str,
    org_id: Option<&str>,
    domain: &str,
) -> anyhow::Result<Option<DomainRecord>> {
    match db {
        Db::Sql(_) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = $3"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "SELECT instance_id, org_id, domain, \
                 CASE WHEN is_primary THEN 1 ELSE 0 END AS is_primary, \
                 purpose, state, CASE WHEN verified THEN 1 ELSE 0 END AS verified, \
                 verification_token, dns_challenge_host, dns_authorization_id, \
                 certificate_dns_record_name, certificate_dns_record_type, certificate_dns_record_value, \
                 certificate_state, certificate_id, certificate_map_entry, origin_trust_state, provisioning_error, \
                 CAST(created_at AS TEXT) AS created_at, CAST(updated_at AS TEXT) AS updated_at \
                 FROM domains WHERE instance_id = $1 AND domain = $2 {org_filter} LIMIT 1"
            );
            let query = sqlx::query_as::<_, DomainSqlRow>(&sql)
                .bind(instance_id)
                .bind(domain);
            let query = if let Some(org_id) = org_id {
                query.bind(org_id)
            } else {
                query
            };
            Ok(query
                .fetch_optional(db.pool())
                .await?
                .map(domain_from_sql_row))
        }
        Db::Spanner(spanner) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = @org_id"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "SELECT instance_id, org_id, domain, is_primary, purpose, state, verified, \
                 verification_token, dns_challenge_host, dns_authorization_id, \
                 certificate_dns_record_name, certificate_dns_record_type, certificate_dns_record_value, \
                 certificate_state, certificate_id, certificate_map_entry, origin_trust_state, provisioning_error, \
                 CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM domains WHERE instance_id = @instance_id AND domain = @domain {org_filter} LIMIT 1"
            );
            let mut stmt = Statement::new(&sql);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("domain", &domain);
            if let Some(org_id) = org_id {
                stmt.add_param("org_id", &org_id);
            }
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| domain_from_spanner_row(&row)))
        }
    }
}

/// Update domain state and verified flag.
pub async fn update_domain_state_for_scope(
    db: &Db,
    instance_id: &str,
    org_id: Option<&str>,
    domain: &str,
    new_state: &str,
    verified: bool,
    provisioning_error: Option<&str>,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = $6"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "UPDATE domains SET state = $1, verified = $2, provisioning_error = $3, \
                 updated_at = CURRENT_TIMESTAMP WHERE instance_id = $4 AND domain = $5 {org_filter}"
            );
            let query = sqlx::query(&sql)
                .bind(new_state)
                .bind(verified)
                .bind(provisioning_error.unwrap_or(""))
                .bind(instance_id)
                .bind(domain);
            let query = if let Some(org_id) = org_id {
                query.bind(org_id)
            } else {
                query
            };
            query.execute(db.pool()).await?;
        }
        Db::Spanner(spanner) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = @org_id"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "UPDATE domains SET state = @state, verified = @verified, provisioning_error = @provisioning_error, \
                 updated_at = CURRENT_TIMESTAMP() WHERE instance_id = @instance_id AND domain = @domain {org_filter}"
            );
            let mut stmt = Statement::new(&sql);
            stmt.add_param("state", &new_state);
            stmt.add_param("verified", &verified);
            stmt.add_param("provisioning_error", &provisioning_error.unwrap_or(""));
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("domain", &domain);
            if let Some(org_id) = org_id {
                stmt.add_param("org_id", &org_id);
            }
            spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
        }
    }
    Ok(())
}

/// Update cloud certificate provisioning state.
#[allow(clippy::too_many_arguments)]
pub async fn update_domain_certificate_state_for_scope(
    db: &Db,
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
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = $11"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "UPDATE domains SET certificate_state = $1, certificate_id = $2, certificate_map_entry = $3, \
                 dns_authorization_id = $4, certificate_dns_record_name = $5, certificate_dns_record_type = $6, \
                 certificate_dns_record_value = $7, provisioning_error = $8, updated_at = CURRENT_TIMESTAMP \
                 WHERE instance_id = $9 AND domain = $10 {org_filter}"
            );
            let query = sqlx::query(&sql)
                .bind(cert_state)
                .bind(cert_id)
                .bind(cert_map_entry.unwrap_or(""))
                .bind(dns_authorization_id.unwrap_or(""))
                .bind(dns_record_name.unwrap_or(""))
                .bind(dns_record_type.unwrap_or(""))
                .bind(dns_record_value.unwrap_or(""))
                .bind(provisioning_error.unwrap_or(""))
                .bind(instance_id)
                .bind(domain);
            let query = if let Some(org_id) = org_id {
                query.bind(org_id)
            } else {
                query
            };
            query.execute(db.pool()).await?;
        }
        Db::Spanner(spanner) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = @org_id"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "UPDATE domains SET certificate_state = @cert_state, certificate_id = @cert_id, \
                 certificate_map_entry = @certificate_map_entry, dns_authorization_id = @dns_authorization_id, \
                 certificate_dns_record_name = @dns_record_name, certificate_dns_record_type = @dns_record_type, \
                 certificate_dns_record_value = @dns_record_value, provisioning_error = @provisioning_error, \
                 updated_at = CURRENT_TIMESTAMP() WHERE instance_id = @instance_id AND domain = @domain {org_filter}"
            );
            let mut stmt = Statement::new(&sql);
            stmt.add_param("cert_state", &cert_state);
            stmt.add_param("cert_id", &cert_id);
            stmt.add_param("certificate_map_entry", &cert_map_entry.unwrap_or(""));
            stmt.add_param("dns_authorization_id", &dns_authorization_id.unwrap_or(""));
            stmt.add_param("dns_record_name", &dns_record_name.unwrap_or(""));
            stmt.add_param("dns_record_type", &dns_record_type.unwrap_or(""));
            stmt.add_param("dns_record_value", &dns_record_value.unwrap_or(""));
            stmt.add_param("provisioning_error", &provisioning_error.unwrap_or(""));
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("domain", &domain);
            if let Some(org_id) = org_id {
                stmt.add_param("org_id", &org_id);
            }
            spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
        }
    }
    Ok(())
}

/// Update origin trust state (for allowed domains).
pub async fn update_domain_origin_trust_state_for_scope(
    db: &Db,
    instance_id: &str,
    org_id: Option<&str>,
    domain: &str,
    state: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = $4"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "UPDATE domains SET origin_trust_state = $1, updated_at = CURRENT_TIMESTAMP \
                 WHERE instance_id = $2 AND domain = $3 {org_filter}"
            );
            let query = sqlx::query(&sql).bind(state).bind(instance_id).bind(domain);
            let query = if let Some(org_id) = org_id {
                query.bind(org_id)
            } else {
                query
            };
            query.execute(db.pool()).await?;
        }
        Db::Spanner(spanner) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = @org_id"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "UPDATE domains SET origin_trust_state = @state, updated_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND domain = @domain {org_filter}"
            );
            let mut stmt = Statement::new(&sql);
            stmt.add_param("state", &state);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("domain", &domain);
            if let Some(org_id) = org_id {
                stmt.add_param("org_id", &org_id);
            }
            spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
        }
    }
    Ok(())
}

pub async fn add_instance_domain(
    db: &Db,
    instance_id: &str,
    domain: &str,
) -> anyhow::Result<DomainRecord> {
    add_instance_domain_full(
        db,
        instance_id,
        domain,
        None,
        "served",
        "active",
        false,
        "",
        "",
    )
    .await
}

/// Add a domain with full control over all fields.
#[allow(clippy::too_many_arguments)]
pub async fn add_instance_domain_full(
    db: &Db,
    instance_id: &str,
    domain: &str,
    org_id: Option<&str>,
    purpose: &str,
    state: &str,
    verified: bool,
    verification_token: &str,
    dns_challenge_host: &str,
) -> anyhow::Result<DomainRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "INSERT INTO domains (domain, instance_id, org_id, is_primary, purpose, state, verified, \
                 verification_token, dns_challenge_host, dns_authorization_id, certificate_dns_record_name, \
                 certificate_dns_record_type, certificate_dns_record_value, certificate_state, certificate_id, \
                 certificate_map_entry, origin_trust_state, provisioning_error) \
                 VALUES ($1, $2, $3, 0, $4, $5, $6, $7, $8, '', '', '', '', '', '', '', '')",
            )
            .bind(domain)
            .bind(instance_id)
            .bind(org_id)
            .bind(purpose)
            .bind(state)
            .bind(verified)
            .bind(verification_token)
            .bind(dns_challenge_host)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let org_id = org_id.map(|value| value.to_string());
            let cols = &[
                "domain",
                "instance_id",
                "org_id",
                "is_primary",
                "purpose",
                "state",
                "verified",
                "verification_token",
                "dns_challenge_host",
                "dns_authorization_id",
                "certificate_dns_record_name",
                "certificate_dns_record_type",
                "certificate_dns_record_value",
                "certificate_state",
                "certificate_id",
                "certificate_map_entry",
                "origin_trust_state",
                "provisioning_error",
            ];
            let mutation = insert(
                "domains",
                cols,
                &[
                    &domain,
                    &instance_id,
                    &org_id as &dyn ToKind,
                    &false,
                    &purpose,
                    &state,
                    &verified,
                    &verification_token,
                    &dns_challenge_host,
                    &"",
                    &"",
                    &"",
                    &"",
                    &"",
                    &"",
                    &"",
                    &"",
                    &"",
                ],
            );
            spanner.client().apply(vec![mutation]).await?;
        }
    }

    get_domain_for_scope(db, instance_id, org_id, domain)
        .await?
        .ok_or_else(|| anyhow::anyhow!("created domain but could not reload it"))
}

pub async fn delete_domain_for_scope(
    db: &Db,
    instance_id: &str,
    org_id: Option<&str>,
    domain: &str,
) -> anyhow::Result<DomainDeleteOutcome> {
    let current = match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let org_filter = if org_id.is_some() {
                "AND org_id = $3"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "SELECT {}, state FROM domains WHERE domain = $1 AND instance_id = $2 {org_filter}",
                scoped.bool_as_int("is_primary"),
            );
            let query = sqlx::query_as::<_, (i32, String)>(&sql)
                .bind(domain)
                .bind(instance_id);
            let query = if let Some(org_id) = org_id {
                query.bind(org_id)
            } else {
                query
            };
            let row = query.fetch_optional(scoped.pool()).await?;
            row.map(|(is_primary, state)| (is_primary != 0, state))
        }
        Db::Spanner(spanner) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = @org_id"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "SELECT is_primary, state FROM domains \
                 WHERE domain = @domain AND instance_id = @instance_id {org_filter} LIMIT 1"
            );
            let mut stmt = Statement::new(&sql);
            stmt.add_param("domain", &domain);
            stmt.add_param("instance_id", &instance_id);
            if let Some(org_id) = org_id {
                stmt.add_param("org_id", &org_id);
            }
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
            let org_filter = if org_id.is_some() {
                "AND org_id = $3"
            } else {
                "AND org_id IS NULL"
            };
            let sql =
                format!("DELETE FROM domains WHERE domain = $1 AND instance_id = $2 {org_filter}");
            let query = sqlx::query(&sql).bind(domain).bind(instance_id);
            let query = if let Some(org_id) = org_id {
                query.bind(org_id)
            } else {
                query
            };
            query.execute(scoped.pool()).await?;
        }
        Db::Spanner(spanner) => {
            let org_filter = if org_id.is_some() {
                "AND org_id = @org_id"
            } else {
                "AND org_id IS NULL"
            };
            let sql = format!(
                "DELETE FROM domains WHERE domain = @domain AND instance_id = @instance_id {org_filter}"
            );
            let mut stmt = Statement::new(&sql);
            stmt.add_param("domain", &domain);
            stmt.add_param("instance_id", &instance_id);
            if let Some(org_id) = org_id {
                stmt.add_param("org_id", &org_id);
            }
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
            let table = spanner_ident(table);
            let mut exists_stmt = Statement::new(format!(
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
            let mut delete_stmt = Statement::new(format!(
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
                let table = spanner_ident(table);
                Statement::new(format!(
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
            let table = spanner_ident(table);
            let mut stmt = Statement::new(format!(
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
            let table = spanner_ident(table);
            let mut stmt = Statement::new(format!(
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
            let table = spanner_ident(table);
            let mut stmt = Statement::new(format!(
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

pub async fn load_instance_routing_cache_entry(
    db: &Db,
    key: &str,
) -> anyhow::Result<Option<Option<InstanceContext>>> {
    let Db::Sql(_) = db else {
        return Ok(None);
    };

    let scoped = db.scoped_default();
    let data = scoped.as_text("data");
    let sql = format!(
        "SELECT {data} FROM cache \
         WHERE instance_id = $1 AND namespace = $2 AND key = $3 \
           AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)"
    );
    let row: Option<(String,)> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind("instance_routing")
        .bind(key)
        .fetch_optional(scoped.pool())
        .await?;

    match row {
        Some((payload,)) => {
            let envelope: InstanceRoutingCacheEnvelope = serde_json::from_str(&payload)?;
            Ok(Some(envelope.value))
        }
        None => Ok(None),
    }
}

pub async fn store_instance_routing_cache_entry(
    db: &Db,
    key: &str,
    value: Option<InstanceContext>,
    ttl: Duration,
) -> anyhow::Result<()> {
    let Db::Sql(_) = db else {
        return Ok(());
    };

    let scoped = db.scoped_default();
    let expires_expr = match scoped.dialect() {
        crate::Dialect::Postgres => {
            format!("CURRENT_TIMESTAMP + INTERVAL '{} seconds'", ttl.as_secs())
        }
        crate::Dialect::Sqlite => {
            format!("datetime(CURRENT_TIMESTAMP, '+{} seconds')", ttl.as_secs())
        }
        crate::Dialect::Spanner => return Ok(()),
    };
    let payload = serde_json::to_string(&InstanceRoutingCacheEnvelope { value })?;
    let sql = format!(
        "INSERT INTO cache (instance_id, namespace, key, data, fetched_at, expires_at) \
         VALUES ($1, $2, $3, {}, {}, {}) \
         ON CONFLICT(instance_id, namespace, key) DO UPDATE SET \
           data = excluded.data, \
           fetched_at = excluded.fetched_at, \
           expires_at = excluded.expires_at",
        scoped.json_bind(4),
        scoped.timestamp_now(),
        expires_expr,
    );
    sqlx::query(&sql)
        .bind(scoped.instance_id())
        .bind("instance_routing")
        .bind(key)
        .bind(payload)
        .execute(scoped.pool())
        .await?;
    Ok(())
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
