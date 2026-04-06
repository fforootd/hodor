use google_cloud_spanner::{
    client::Error as SpannerError, statement::Statement,
};

use crate::Db;
use super::{
    ActionRecord, FingerprintRecord, JobRecord, MembershipRow, OrgRecord, OrgRoleMembershipRecord,
    OrgUserLinkRecord, PatRecord, SavedQueryRecord, SearchRecord, SettingsRecord,
    UnshippedEventRecord,
    action_from_spanner_row, action_from_sql_row,
    spanner_query_all, spanner_query_optional,
};

// ─── Org CRUD ───

pub async fn create_org(
    db: &Db,
    instance_id: &str,
    id: &str,
    name: &str,
    metadata_json: &str,
) -> anyhow::Result<OrgRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO orgs (id, instance_id, name, state, metadata) \
                 VALUES ($1, $2, $3, 'active', {})",
                scoped.json_bind(4),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(name)
                .bind(metadata_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO orgs (id, instance_id, name, state, metadata) \
                 VALUES (@id, @instance_id, @name, 'active', @metadata)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("name", &name);
            stmt.add_param("metadata", &metadata_json);
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
    get_org(db, instance_id, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("created org but could not reload it"))
}

pub async fn get_org(db: &Db, instance_id: &str, id: &str) -> anyhow::Result<Option<OrgRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                 FROM orgs WHERE instance_id = $1 AND id = $2"
            );
            Ok(
                sqlx::query_as::<_, (String, String, String, String, String, String)>(&sql)
                    .bind(instance_id)
                    .bind(id)
                    .fetch_optional(scoped.pool())
                    .await?
                    .map(|row| OrgRecord {
                        id: row.0,
                        name: row.1,
                        state: row.2,
                        metadata_json: row.3,
                        created_at: row.4,
                        updated_at: row.5,
                    }),
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name, state, IFNULL(metadata, '{}') AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM orgs WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| OrgRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    metadata_json: row.column_by_name::<String>("metadata").unwrap_or_default(),
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

pub async fn list_org_records(
    db: &Db,
    instance_id: &str,
    after_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<OrgRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                 FROM orgs WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
            );
            let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(&sql)
                .bind(instance_id)
                .bind(after_id)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| OrgRecord {
                    id: row.0,
                    name: row.1,
                    state: row.2,
                    metadata_json: row.3,
                    created_at: row.4,
                    updated_at: row.5,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name, state, IFNULL(metadata, '{}') AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM orgs WHERE instance_id = @instance_id AND id > @after_id ORDER BY id LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| OrgRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    metadata_json: row.column_by_name::<String>("metadata").unwrap_or_default(),
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

pub async fn update_org(
    db: &Db,
    instance_id: &str,
    id: &str,
    name: Option<&str>,
    state: Option<&str>,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let mut sets = Vec::new();
            let mut binds: Vec<String> = Vec::new();
            let mut next = 1usize;
            if let Some(name) = name {
                sets.push(format!("name = {}", scoped.placeholder(next)));
                binds.push(name.to_string());
                next += 1;
            }
            if let Some(state) = state {
                sets.push(format!("state = {}", scoped.placeholder(next)));
                binds.push(state.to_string());
                next += 1;
            }
            if sets.is_empty() {
                return Ok(false);
            }
            sets.push("updated_at = CURRENT_TIMESTAMP".to_string());
            let sql = format!(
                "UPDATE orgs SET {} WHERE instance_id = {} AND id = {}",
                sets.join(", "),
                scoped.placeholder(next),
                scoped.placeholder(next + 1),
            );
            let mut query = sqlx::query(&sql);
            for bind in &binds {
                query = query.bind(bind);
            }
            Ok(query
                .bind(instance_id)
                .bind(id)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut sets = Vec::new();
            let mut stmt = Statement::new("");
            if let Some(name) = name {
                sets.push("name = @name");
                stmt.add_param("name", &name);
            }
            if let Some(state) = state {
                sets.push("state = @state");
                stmt.add_param("state", &state);
            }
            if sets.is_empty() {
                return Ok(false);
            }
            let sql = format!(
                "UPDATE orgs SET {}, updated_at = CURRENT_TIMESTAMP() WHERE instance_id = @instance_id AND id = @id",
                sets.join(", ")
            );
            stmt = Statement::new(sql);
            if let Some(name) = name {
                stmt.add_param("name", &name);
            }
            if let Some(state) = state {
                stmt.add_param("state", &state);
            }
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

pub async fn first_org_id(db: &Db, instance_id: &str) -> anyhow::Result<Option<String>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query_as::<_, (String,)>(
                "SELECT id FROM orgs WHERE instance_id = $1 ORDER BY created_at ASC LIMIT 1",
            )
            .bind(instance_id)
            .fetch_optional(scoped.pool())
            .await?
            .map(|row| row.0))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id FROM orgs WHERE instance_id = @instance_id ORDER BY created_at ASC LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| row.column_by_name::<String>("id").unwrap_or_default()))
        }
    }
}

// ─── Org users and role memberships (FGA-related) ───

pub async fn list_active_org_users(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Vec<OrgUserLinkRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let rows = sqlx::query_as::<_, (String, String)>(
                "SELECT COALESCE(org_id, ''), id FROM users \
                 WHERE instance_id = $1 AND state = 'active' \
                 ORDER BY org_id, id",
            )
            .bind(instance_id)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|(org_id, user_id)| OrgUserLinkRecord { org_id, user_id })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT IFNULL(org_id, '') AS org_id, id FROM users \
                 WHERE instance_id = @instance_id AND state = 'active' \
                 ORDER BY org_id, id",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| OrgUserLinkRecord {
                    org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
                    user_id: row.column_by_name::<String>("id").unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn list_active_org_role_memberships(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Vec<OrgRoleMembershipRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let rows = sqlx::query_as::<_, (String, String, String)>(
                "SELECT m.resource_id, m.user_id, m.role \
                 FROM memberships m \
                 JOIN users u ON u.instance_id = m.instance_id AND u.id = m.user_id \
                 WHERE m.instance_id = $1 AND m.resource_type = 'org' AND u.state = 'active' \
                 ORDER BY m.resource_id, m.user_id",
            )
            .bind(instance_id)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|(org_id, user_id, role)| OrgRoleMembershipRecord {
                    org_id,
                    user_id,
                    role,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT m.resource_id, m.user_id, m.role \
                 FROM memberships m \
                 JOIN users u ON u.instance_id = m.instance_id AND u.id = m.user_id \
                 WHERE m.instance_id = @instance_id AND m.resource_type = 'org' AND u.state = 'active' \
                 ORDER BY m.resource_id, m.user_id",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| OrgRoleMembershipRecord {
                    org_id: row
                        .column_by_name::<String>("resource_id")
                        .unwrap_or_default(),
                    user_id: row.column_by_name::<String>("user_id").unwrap_or_default(),
                    role: row.column_by_name::<String>("role").unwrap_or_default(),
                })
                .collect())
        }
    }
}

// ─── Membership helpers ───

pub async fn list_memberships(
    db: &Db,
    instance_id: &str,
    resource_type: &str,
    resource_id: &str,
) -> anyhow::Result<Vec<MembershipRow>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let added_at = scoped.as_text("m.added_at");
            let sql = format!(
                "SELECT m.user_id, u.display_name, m.role, {added_at} \
                 FROM memberships m \
                 LEFT JOIN users u ON u.instance_id = m.instance_id AND u.id = m.user_id \
                 WHERE m.instance_id = $1 AND m.resource_type = $2 AND m.resource_id = $3 \
                 ORDER BY m.added_at DESC"
            );
            let rows: Vec<(String, Option<String>, String, String)> = sqlx::query_as(&sql)
                .bind(instance_id)
                .bind(resource_type)
                .bind(resource_id)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|(user_id, display_name, role, added_at)| MembershipRow {
                    user_id,
                    display_name,
                    role,
                    added_at,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT m.user_id, u.display_name, m.role, CAST(m.added_at AS STRING) AS added_at \
                 FROM memberships m \
                 LEFT JOIN users u ON u.instance_id = m.instance_id AND u.id = m.user_id \
                 WHERE m.instance_id = @instance_id AND m.resource_type = @resource_type AND m.resource_id = @resource_id \
                 ORDER BY m.added_at DESC",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("resource_type", &resource_type);
            stmt.add_param("resource_id", &resource_id);
            let mut tx = spanner.client().single().await?;
            let mut rows = tx.query(stmt).await?;
            let mut result = Vec::new();
            while let Some(row) = rows.next().await? {
                result.push(MembershipRow {
                    user_id: row.column_by_name::<String>("user_id")?,
                    display_name: row.column_by_name::<Option<String>>("display_name")?,
                    role: row.column_by_name::<String>("role")?,
                    added_at: row.column_by_name::<String>("added_at")?,
                });
            }
            Ok(result)
        }
    }
}

pub async fn add_membership(
    db: &Db,
    instance_id: &str,
    resource_type: &str,
    resource_id: &str,
    user_id: &str,
    role: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = match scoped.dialect() {
                crate::Dialect::Postgres => {
                    "INSERT INTO memberships (instance_id, resource_type, resource_id, user_id, role) \
                     VALUES ($1, $2, $3, $4, $5) \
                     ON CONFLICT (instance_id, resource_type, resource_id, user_id) DO NOTHING"
                }
                crate::Dialect::Sqlite => {
                    "INSERT OR IGNORE INTO memberships (instance_id, resource_type, resource_id, user_id, role) \
                     VALUES ($1, $2, $3, $4, $5)"
                }
                crate::Dialect::Spanner => unreachable!(),
            };
            sqlx::query(sql)
                .bind(instance_id)
                .bind(resource_type)
                .bind(resource_id)
                .bind(user_id)
                .bind(role)
                .execute(scoped.pool())
                .await?;
            Ok(())
        }
        Db::Spanner(spanner) => {
            let mut exists = Statement::new(
                "SELECT user_id FROM memberships \
                 WHERE instance_id = @instance_id AND resource_type = @resource_type \
                   AND resource_id = @resource_id AND user_id = @user_id LIMIT 1",
            );
            exists.add_param("instance_id", &instance_id);
            exists.add_param("resource_type", &resource_type);
            exists.add_param("resource_id", &resource_id);
            exists.add_param("user_id", &user_id);
            let mut tx = spanner.client().single().await?;
            let mut rs = tx.query(exists).await?;
            if rs.next().await?.is_none() {
                let mut stmt = Statement::new(
                    "INSERT INTO memberships (instance_id, resource_type, resource_id, user_id, role) \
                     VALUES (@instance_id, @resource_type, @resource_id, @user_id, @role)",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("resource_type", &resource_type);
                stmt.add_param("resource_id", &resource_id);
                stmt.add_param("user_id", &user_id);
                stmt.add_param("role", &role);
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
            Ok(())
        }
    }
}

pub async fn remove_membership(
    db: &Db,
    instance_id: &str,
    resource_type: &str,
    resource_id: &str,
    user_id: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "DELETE FROM memberships \
                 WHERE instance_id = $1 AND resource_type = $2 AND resource_id = $3 AND user_id = $4",
            )
            .bind(instance_id)
            .bind(resource_type)
            .bind(resource_id)
            .bind(user_id)
            .execute(scoped.pool())
            .await?;
            Ok(())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "DELETE FROM memberships \
                 WHERE instance_id = @instance_id AND resource_type = @resource_type \
                   AND resource_id = @resource_id AND user_id = @user_id",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("resource_type", &resource_type);
            stmt.add_param("resource_id", &resource_id);
            stmt.add_param("user_id", &user_id);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
            Ok(())
        }
    }
}

// ─── Events ───

pub async fn append_event(
    db: &Db,
    instance_id: &str,
    id: &str,
    event_type: &str,
    category: &str,
    flow_id: &str,
    fingerprint: &str,
    payload_json: &str,
    metadata_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO events \
                 (id, instance_id, event_type, category, flow_id, fingerprint, payload, metadata) \
                 VALUES ($1, $2, $3, $4, $5, $6, {}, {})",
                scoped.json_bind(7),
                scoped.json_bind(8),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(event_type)
                .bind(category)
                .bind(flow_id)
                .bind(fingerprint)
                .bind(payload_json)
                .bind(metadata_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO events \
                 (id, instance_id, event_type, category, flow_id, fingerprint, payload, metadata) \
                 VALUES (@id, @instance_id, @event_type, @category, @flow_id, @fingerprint, @payload, @metadata)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("event_type", &event_type);
            stmt.add_param("category", &category);
            stmt.add_param("flow_id", &flow_id);
            stmt.add_param("fingerprint", &fingerprint);
            stmt.add_param("payload", &payload_json);
            stmt.add_param("metadata", &metadata_json);
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
    Ok(())
}

/// Fetch up to `limit` events that have not yet been shipped (shipped_at IS NULL),
/// ordered by created_at ASC, id ASC. Used by the event consumer job.
pub async fn fetch_unshipped_events(
    db: &Db,
    instance_id: &str,
    limit: u32,
) -> anyhow::Result<Vec<UnshippedEventRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let rows =
                sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
                    "SELECT id, instance_id, event_type, category, \
                        COALESCE(payload, '{}'), COALESCE(metadata, '{}'), created_at \
                 FROM events \
                 WHERE instance_id = $1 AND shipped_at IS NULL \
                 ORDER BY created_at ASC, id ASC \
                 LIMIT $2",
                )
                .bind(instance_id)
                .bind(limit as i64)
                .fetch_all(scoped.pool())
                .await?;

            Ok(rows
                .into_iter()
                .map(|r| UnshippedEventRecord {
                    id: r.0,
                    instance_id: r.1,
                    event_type: r.2,
                    category: r.3,
                    payload: r.4,
                    metadata: r.5,
                    created_at: r.6,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, instance_id, event_type, category, \
                        COALESCE(payload, '{}'), COALESCE(metadata, '{}'), \
                        CAST(created_at AS STRING) AS created_at \
                 FROM events \
                 WHERE instance_id = @instance_id AND shipped_at IS NULL \
                 ORDER BY created_at ASC, id ASC \
                 LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("limit", &(limit as i64));

            let mut tx = spanner.client().single().await?;
            let mut result_set = tx.query(stmt).await?;
            let mut records = Vec::new();
            while let Some(row) = result_set.next().await? {
                records.push(UnshippedEventRecord {
                    id: row.column_by_name::<String>("id")?,
                    instance_id: row.column_by_name::<String>("instance_id")?,
                    event_type: row.column_by_name::<String>("event_type")?,
                    category: row.column_by_name::<String>("category")?,
                    payload: row.column_by_name::<String>("payload").unwrap_or_default(),
                    metadata: row.column_by_name::<String>("metadata").unwrap_or_default(),
                    created_at: row.column_by_name::<String>("created_at")?,
                });
            }
            Ok(records)
        }
    }
}

/// Mark events as shipped by setting shipped_at to the current timestamp.
pub async fn mark_events_shipped(
    db: &Db,
    instance_id: &str,
    event_ids: &[String],
) -> anyhow::Result<u64> {
    if event_ids.is_empty() {
        return Ok(0);
    }
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let current_ts = match db.dialect() {
                crate::Dialect::Sqlite => "datetime('now')",
                crate::Dialect::Postgres => "NOW()",
                crate::Dialect::Spanner => unreachable!(),
            };
            // Build IN list with numbered params starting at $2
            let placeholders: Vec<String> = (0..event_ids.len())
                .map(|i| format!("${}", i + 2))
                .collect();
            let sql = format!(
                "UPDATE events SET shipped_at = {current_ts} \
                 WHERE instance_id = $1 AND id IN ({}) AND shipped_at IS NULL",
                placeholders.join(", ")
            );
            let mut query = sqlx::query(&sql).bind(instance_id);
            for eid in event_ids {
                query = query.bind(eid);
            }
            let result = query.execute(scoped.pool()).await?;
            Ok(result.rows_affected())
        }
        Db::Spanner(spanner) => {
            // Spanner: batch update in a read-write transaction
            let ids = event_ids.to_vec();
            let iid = instance_id.to_string();
            let (_, affected) = spanner
                .client()
                .read_write_transaction(|tx| {
                    let ids = ids.clone();
                    let iid = iid.clone();
                    Box::pin(async move {
                        let mut total = 0i64;
                        for chunk in ids.chunks(100) {
                            let placeholders: Vec<String> = chunk
                                .iter()
                                .enumerate()
                                .map(|(i, _)| format!("@id{i}"))
                                .collect();
                            let sql = format!(
                                "UPDATE events SET shipped_at = CURRENT_TIMESTAMP() \
                                 WHERE instance_id = @iid AND id IN ({}) AND shipped_at IS NULL",
                                placeholders.join(", ")
                            );
                            let mut stmt = Statement::new(&sql);
                            stmt.add_param("iid", &iid);
                            for (i, id) in chunk.iter().enumerate() {
                                stmt.add_param(&format!("id{i}"), id);
                            }
                            total += tx.update(stmt).await?;
                        }
                        Ok::<i64, SpannerError>(total)
                    })
                })
                .await?;
            Ok(affected as u64)
        }
    }
}

// ─── Settings ───

pub async fn get_settings_record(
    db: &Db,
    instance_id: &str,
    type_: &str,
) -> anyhow::Result<Option<SettingsRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "SELECT type, scope, {} FROM settings \
                 WHERE instance_id = $1 AND type = $2 \
                 ORDER BY CASE scope WHEN 'app' THEN 1 WHEN 'org' THEN 2 ELSE 3 END LIMIT 1",
                scoped.as_text("data"),
            );
            Ok(sqlx::query_as::<_, (String, String, String)>(&sql)
                .bind(instance_id)
                .bind(type_)
                .fetch_optional(scoped.pool())
                .await?
                .map(|row| SettingsRecord {
                    type_: row.0,
                    scope: row.1,
                    data_json: row.2,
                }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT type, scope, IFNULL(data, '{}') AS data \
                 FROM settings WHERE instance_id = @instance_id AND type = @type \
                 ORDER BY CASE scope WHEN 'app' THEN 1 WHEN 'org' THEN 2 ELSE 3 END LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("type", &type_);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| SettingsRecord {
                    type_: row.column_by_name::<String>("type").unwrap_or_default(),
                    scope: row.column_by_name::<String>("scope").unwrap_or_default(),
                    data_json: row.column_by_name::<String>("data").unwrap_or_default(),
                }))
        }
    }
}

pub async fn put_instance_settings(
    db: &Db,
    instance_id: &str,
    id: &str,
    type_: &str,
    data_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) \
                 VALUES ($1, $2, $3, 'instance', '', {}) \
                 ON CONFLICT(instance_id, type, scope, scope_id) DO UPDATE SET data = {}, updated_at = CURRENT_TIMESTAMP",
                scoped.json_bind(4),
                scoped.json_bind(4),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(type_)
                .bind(data_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut exists_stmt = Statement::new(
                "SELECT id FROM settings WHERE instance_id = @instance_id AND type = @type AND scope = 'instance' AND scope_id = '' LIMIT 1",
            );
            exists_stmt.add_param("instance_id", &instance_id);
            exists_stmt.add_param("type", &type_);
            let existing = spanner_query_optional(spanner, exists_stmt).await?;
            if let Some(row) = existing {
                let existing_id = row.column_by_name::<String>("id").unwrap_or_default();
                let mut update_stmt = Statement::new(
                    "UPDATE settings SET data = @data, updated_at = CURRENT_TIMESTAMP() \
                     WHERE instance_id = @instance_id AND id = @id",
                );
                update_stmt.add_param("data", &data_json);
                update_stmt.add_param("instance_id", &instance_id);
                update_stmt.add_param("id", &existing_id);
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = update_stmt.clone();
                        Box::pin(async move {
                            tx.update(stmt).await?;
                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await?;
            } else {
                let mut insert_stmt = Statement::new(
                    "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) \
                     VALUES (@id, @instance_id, @type, 'instance', '', @data)",
                );
                insert_stmt.add_param("id", &id);
                insert_stmt.add_param("instance_id", &instance_id);
                insert_stmt.add_param("type", &type_);
                insert_stmt.add_param("data", &data_json);
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = insert_stmt.clone();
                        Box::pin(async move {
                            tx.update(stmt).await?;
                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await?;
            }
        }
    }
    Ok(())
}

pub async fn delete_settings_record(db: &Db, instance_id: &str, type_: &str) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query("DELETE FROM settings WHERE instance_id = $1 AND type = $2")
                .bind(instance_id)
                .bind(type_)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "DELETE FROM settings WHERE instance_id = @instance_id AND type = @type",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("type", &type_);
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
    Ok(())
}

// ─── PATs ───

pub async fn create_pat(
    db: &Db,
    instance_id: &str,
    id: &str,
    user_id: &str,
    name: &str,
    token_hash: &str,
    scopes_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) \
                 VALUES ($1, $2, 'pat', $3, $4, $5, {})",
                scoped.json_bind(6),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(token_hash)
                .bind(user_id)
                .bind(name)
                .bind(scopes_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) \
                 VALUES (@id, @instance_id, 'pat', @token_hash, @user_id, @name, @scopes)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("token_hash", &token_hash);
            stmt.add_param("user_id", &user_id);
            stmt.add_param("name", &name);
            stmt.add_param("scopes", &scopes_json);
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
    Ok(())
}

pub async fn list_pats_for_instance(db: &Db, instance_id: &str) -> anyhow::Result<Vec<PatRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let created_at = scoped.as_text("created_at");
            let sql = format!(
                "SELECT id, user_id, COALESCE(name,''), {created_at} \
                 FROM tokens WHERE instance_id = $1 AND type = 'pat' AND revoked_at IS NULL \
                 ORDER BY created_at DESC"
            );
            let rows = sqlx::query_as::<_, (String, String, String, String)>(&sql)
                .bind(instance_id)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| PatRecord {
                    id: row.0,
                    user_id: row.1,
                    name: row.2,
                    created_at: row.3,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, user_id, IFNULL(name, '') AS name, CAST(created_at AS STRING) AS created_at \
                 FROM tokens WHERE instance_id = @instance_id AND type = 'pat' AND revoked_at IS NULL \
                 ORDER BY created_at DESC",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| PatRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    user_id: row.column_by_name::<String>("user_id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn revoke_pat(db: &Db, instance_id: &str, id: &str) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query(
                "UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND id = $2 AND type = 'pat'",
            )
            .bind(instance_id)
            .bind(id)
            .execute(scoped.pool())
            .await?
            .rows_affected()
            > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND id = @id AND type = 'pat'",
            );
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

// ─── Actions ───

pub async fn list_actions(db: &Db, instance_id: &str) -> anyhow::Result<Vec<ActionRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let config = scoped.as_text("config");
            let metadata = scoped.as_text("metadata");
            let created_at = scoped.as_text("created_at");
            let enabled = scoped.bool_as_int("enabled");
            let fail_open = scoped.bool_as_int("fail_open");
            let sql = format!(
                "SELECT id, COALESCE(org_id, ''), name, hook, action_type, COALESCE(trigger_expr, 'true'), \
                        COALESCE({config}, '{{}}'), priority, {enabled}, {fail_open}, \
                        COALESCE({metadata}, '{{}}'), {created_at} \
                 FROM actions WHERE instance_id = $1 ORDER BY priority, name"
            );
            let rows = sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    i64,
                    i64,
                    i64,
                    String,
                    String,
                ),
            >(&sql)
            .bind(instance_id)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows.into_iter().map(action_from_sql_row).collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, IFNULL(org_id, '') AS org_id, name, hook, action_type, IFNULL(trigger_expr, 'true') AS trigger_expr, \
                        IFNULL(config, '{}') AS config, priority, enabled, fail_open, \
                        IFNULL(metadata, '{}') AS metadata, CAST(created_at AS STRING) AS created_at \
                 FROM actions WHERE instance_id = @instance_id ORDER BY priority, name",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(action_from_spanner_row)
                .collect())
        }
    }
}

pub async fn get_action(
    db: &Db,
    instance_id: &str,
    id: &str,
) -> anyhow::Result<Option<ActionRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let config = scoped.as_text("config");
            let metadata = scoped.as_text("metadata");
            let created_at = scoped.as_text("created_at");
            let enabled = scoped.bool_as_int("enabled");
            let fail_open = scoped.bool_as_int("fail_open");
            let sql = format!(
                "SELECT id, COALESCE(org_id, ''), name, hook, action_type, COALESCE(trigger_expr, 'true'), \
                        COALESCE({config}, '{{}}'), priority, {enabled}, {fail_open}, \
                        COALESCE({metadata}, '{{}}'), {created_at} \
                 FROM actions WHERE instance_id = $1 AND id = $2"
            );
            Ok(sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    i64,
                    i64,
                    i64,
                    String,
                    String,
                ),
            >(&sql)
            .bind(instance_id)
            .bind(id)
            .fetch_optional(scoped.pool())
            .await?
            .map(action_from_sql_row))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, IFNULL(org_id, '') AS org_id, name, hook, action_type, IFNULL(trigger_expr, 'true') AS trigger_expr, \
                        IFNULL(config, '{}') AS config, priority, enabled, fail_open, \
                        IFNULL(metadata, '{}') AS metadata, CAST(created_at AS STRING) AS created_at \
                 FROM actions WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(action_from_spanner_row))
        }
    }
}

pub async fn upsert_catalog_action(
    db: &Db,
    instance_id: &str,
    action_id: &str,
    org_id: &str,
    name: &str,
    hook: &str,
    action_type: &str,
    trigger_expr: &str,
    config_json: &str,
    priority: i64,
    enabled: bool,
    fail_open: bool,
    metadata_json: &str,
) -> anyhow::Result<String> {
    let org_id_opt: Option<&str> = if org_id.is_empty() { None } else { Some(org_id) };
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let existing: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM actions WHERE instance_id = $1 AND COALESCE(org_id, '') = $2 AND name = $3",
            )
            .bind(instance_id)
            .bind(org_id)
            .bind(name)
            .fetch_optional(scoped.pool())
            .await?;
            let id = if let Some((existing_id,)) = existing {
                let sql = format!(
                    "UPDATE actions SET hook = $1, action_type = $2, trigger_expr = $3, config = {}, \
                     priority = $4, enabled = $5, fail_open = $6, metadata = {}, updated_at = CURRENT_TIMESTAMP \
                     WHERE id = $7 AND instance_id = $8",
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
                    .bind(instance_id)
                    .bind(config_json)
                    .bind(metadata_json)
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
                    .bind(action_id)
                    .bind(instance_id)
                    .bind(org_id_opt)
                    .bind(name)
                    .bind(hook)
                    .bind(action_type)
                    .bind(trigger_expr)
                    .bind(priority)
                    .bind(enabled)
                    .bind(fail_open)
                    .bind(config_json)
                    .bind(metadata_json)
                    .execute(scoped.pool())
                    .await?;
                action_id.to_string()
            };
            Ok(id)
        }
        Db::Spanner(spanner) => {
            let mut find_stmt = Statement::new(
                "SELECT id FROM actions WHERE instance_id = @instance_id AND IFNULL(org_id, '') = @org_id AND name = @name LIMIT 1",
            );
            find_stmt.add_param("instance_id", &instance_id);
            find_stmt.add_param("org_id", &org_id);
            find_stmt.add_param("name", &name);
            if let Some(row) = spanner_query_optional(spanner, find_stmt).await? {
                let existing_id = row.column_by_name::<String>("id").unwrap_or_default();
                let mut stmt = Statement::new(
                    "UPDATE actions SET hook = @hook, action_type = @action_type, trigger_expr = @trigger_expr, \
                     config = @config, priority = @priority, enabled = @enabled, fail_open = @fail_open, \
                     metadata = @metadata, updated_at = CURRENT_TIMESTAMP() \
                     WHERE instance_id = @instance_id AND id = @id",
                );
                stmt.add_param("hook", &hook);
                stmt.add_param("action_type", &action_type);
                stmt.add_param("trigger_expr", &trigger_expr);
                stmt.add_param("config", &config_json);
                stmt.add_param("priority", &priority);
                stmt.add_param("enabled", &enabled);
                stmt.add_param("fail_open", &fail_open);
                stmt.add_param("metadata", &metadata_json);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("id", &existing_id);
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
                Ok(existing_id)
            } else {
                let mut stmt = Statement::new(
                    "INSERT INTO actions (id, instance_id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, metadata) \
                     VALUES (@id, @instance_id, @org_id, @name, @hook, @action_type, @trigger_expr, @config, @priority, @enabled, @fail_open, @metadata)",
                );
                stmt.add_param("id", &action_id);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("org_id", &org_id);
                stmt.add_param("name", &name);
                stmt.add_param("hook", &hook);
                stmt.add_param("action_type", &action_type);
                stmt.add_param("trigger_expr", &trigger_expr);
                stmt.add_param("config", &config_json);
                stmt.add_param("priority", &priority);
                stmt.add_param("enabled", &enabled);
                stmt.add_param("fail_open", &fail_open);
                stmt.add_param("metadata", &metadata_json);
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
                Ok(action_id.to_string())
            }
        }
    }
}

// ─── Fingerprints ───

pub async fn list_fingerprints(
    db: &Db,
    instance_id: &str,
    after_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<FingerprintRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let created_at = scoped.as_text("created_at");
            let sql = format!(
                "SELECT id, type, raw_data, {created_at} \
                 FROM fingerprints WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
            );
            let rows = sqlx::query_as::<_, (String, String, String, String)>(&sql)
                .bind(instance_id)
                .bind(after_id)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| FingerprintRecord {
                    id: row.0,
                    type_: row.1,
                    raw_data_json: row.2,
                    created_at: row.3,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, type, raw_data, CAST(created_at AS STRING) AS created_at \
                 FROM fingerprints WHERE instance_id = @instance_id AND id > @after_id ORDER BY id LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| FingerprintRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    type_: row.column_by_name::<String>("type").unwrap_or_default(),
                    raw_data_json: row.column_by_name::<String>("raw_data").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn upsert_fingerprint(
    db: &Db,
    instance_id: &str,
    id: &str,
    type_: &str,
    raw_data_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO fingerprints (id, instance_id, type, raw_data, created_at) \
                 VALUES ($1, $2, $3, {}, {}) \
                 ON CONFLICT (id) DO UPDATE SET raw_data = excluded.raw_data, type = excluded.type",
                scoped.json_bind(4),
                scoped.timestamp_now(),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(type_)
                .bind(raw_data_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut exists_stmt =
                Statement::new("SELECT id FROM fingerprints WHERE id = @id LIMIT 1");
            exists_stmt.add_param("id", &id);
            if spanner_query_optional(spanner, exists_stmt)
                .await?
                .is_some()
            {
                let mut stmt = Statement::new(
                    "UPDATE fingerprints SET raw_data = @raw_data, type = @type WHERE id = @id",
                );
                stmt.add_param("raw_data", &raw_data_json);
                stmt.add_param("type", &type_);
                stmt.add_param("id", &id);
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
            } else {
                let mut stmt = Statement::new(
                    "INSERT INTO fingerprints (id, instance_id, type, raw_data, created_at) \
                     VALUES (@id, @instance_id, @type, @raw_data, CURRENT_TIMESTAMP())",
                );
                stmt.add_param("id", &id);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("type", &type_);
                stmt.add_param("raw_data", &raw_data_json);
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
    }
    Ok(())
}

// ─── Saved queries ───

pub async fn list_saved_queries(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Vec<SavedQueryRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let created_at = scoped.as_text("created_at");
            let sql = format!(
                "SELECT id, name, COALESCE(description, ''), sql_text, {created_at} \
                 FROM saved_queries WHERE instance_id = $1 ORDER BY name"
            );
            let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(&sql)
                .bind(instance_id)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| SavedQueryRecord {
                    id: row.0,
                    name: row.1,
                    description: row.2,
                    sql: row.3,
                    created_at: row.4,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name, IFNULL(description, '') AS description, sql_text, \
                        CAST(created_at AS STRING) AS created_at \
                 FROM saved_queries WHERE instance_id = @instance_id ORDER BY name",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| SavedQueryRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    description: row
                        .column_by_name::<String>("description")
                        .unwrap_or_default(),
                    sql: row.column_by_name::<String>("sql_text").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn create_saved_query(
    db: &Db,
    instance_id: &str,
    id: &str,
    name: &str,
    description: &str,
    sql_text: &str,
) -> anyhow::Result<SavedQueryRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "INSERT INTO saved_queries (id, instance_id, name, description, sql_text) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(instance_id)
            .bind(name)
            .bind(description)
            .bind(sql_text)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mutation = google_cloud_spanner::mutation::insert(
                "saved_queries",
                &["id", "instance_id", "name", "description", "sql_text"],
                &[&id, &instance_id, &name, &description, &sql_text],
            );
            spanner.client().apply(vec![mutation]).await?;
        }
    }

    list_saved_queries(db, instance_id)
        .await?
        .into_iter()
        .find(|query| query.id == id)
        .ok_or_else(|| anyhow::anyhow!("saved query created but could not be reloaded"))
}

pub async fn delete_saved_query(db: &Db, instance_id: &str, id: &str) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(
                sqlx::query("DELETE FROM saved_queries WHERE instance_id = $1 AND id = $2")
                    .bind(instance_id)
                    .bind(id)
                    .execute(scoped.pool())
                    .await?
                    .rows_affected()
                    > 0,
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id FROM saved_queries WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            if spanner_query_optional(spanner, stmt).await?.is_none() {
                return Ok(false);
            }
            let mut delete_stmt = Statement::new(
                "DELETE FROM saved_queries WHERE instance_id = @instance_id AND id = @id",
            );
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

// ─── Jobs ───

pub async fn list_jobs_for_instance(db: &Db, instance_id: &str) -> anyhow::Result<Vec<JobRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let config_json = scoped.as_text("config_json");
            let last_run_at = scoped.as_text("last_run_at");
            let next_run_at = scoped.as_text("next_run_at");
            let lease_expires_at = scoped.as_text("lease_expires_at");
            let created_at = scoped.as_text("created_at");
            let updated_at = scoped.as_text("updated_at");
            let enabled = scoped.bool_as_int("enabled");
            let sql = format!(
                "SELECT name, display_name, description, cron, {enabled}, last_status, last_error, \
                        run_count, last_rows_removed, {config_json}, {last_run_at}, {next_run_at}, \
                        {lease_expires_at}, {created_at}, {updated_at} \
                 FROM jobs WHERE instance_id = $1 ORDER BY name ASC"
            );
            let rows = sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    i64,
                    String,
                    String,
                    i64,
                    i64,
                    String,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    String,
                    String,
                ),
            >(&sql)
            .bind(instance_id)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|row| JobRecord {
                    name: row.0,
                    display_name: row.1,
                    description: row.2,
                    cron: row.3,
                    enabled: row.4 != 0,
                    last_status: row.5,
                    last_error: row.6,
                    run_count: row.7,
                    last_rows_removed: row.8,
                    config_json: row.9,
                    last_run_at: row.10,
                    next_run_at: row.11,
                    lease_expires_at: row.12,
                    created_at: row.13,
                    updated_at: row.14,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT name, display_name, description, cron, enabled, last_status, last_error, run_count, \
                        last_rows_removed, IFNULL(config_json, '{}') AS config_json, \
                        CAST(last_run_at AS STRING) AS last_run_at, CAST(next_run_at AS STRING) AS next_run_at, \
                        CAST(lease_expires_at AS STRING) AS lease_expires_at, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM jobs WHERE instance_id = @instance_id ORDER BY name ASC",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| JobRecord {
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    display_name: row
                        .column_by_name::<String>("display_name")
                        .unwrap_or_default(),
                    description: row
                        .column_by_name::<String>("description")
                        .unwrap_or_default(),
                    cron: row.column_by_name::<String>("cron").unwrap_or_default(),
                    enabled: row.column_by_name::<bool>("enabled").unwrap_or(false),
                    last_status: row
                        .column_by_name::<String>("last_status")
                        .unwrap_or_default(),
                    last_error: row
                        .column_by_name::<String>("last_error")
                        .unwrap_or_default(),
                    run_count: row.column_by_name::<i64>("run_count").unwrap_or(0),
                    last_rows_removed: row.column_by_name::<i64>("last_rows_removed").unwrap_or(0),
                    config_json: row
                        .column_by_name::<String>("config_json")
                        .unwrap_or_else(|_| "{}".to_string()),
                    last_run_at: row
                        .column_by_name::<Option<String>>("last_run_at")
                        .unwrap_or(None),
                    next_run_at: row
                        .column_by_name::<Option<String>>("next_run_at")
                        .unwrap_or(None),
                    lease_expires_at: row
                        .column_by_name::<Option<String>>("lease_expires_at")
                        .unwrap_or(None),
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

// ─── Search ───

pub async fn search_records(
    db: &Db,
    instance_id: &str,
    q: &str,
    limit: i64,
) -> anyhow::Result<Vec<SearchRecord>> {
    let pattern = format!("%{q}%");
    let mut results = Vec::new();
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let user_rows = sqlx::query_as::<_, (String, String, String)>(
                "SELECT id, identifier, display_name FROM users \
                 WHERE instance_id = $1 AND (identifier LIKE $2 OR display_name LIKE $3) LIMIT $4",
            )
            .bind(instance_id)
            .bind(&pattern)
            .bind(&pattern)
            .bind(limit)
            .fetch_all(scoped.pool())
            .await?;
            for row in user_rows {
                results.push(SearchRecord {
                    resource_type: "user".to_string(),
                    id: row.0,
                    title: row.2.clone(),
                    subtitle: row.1,
                });
            }
            let org_rows = sqlx::query_as::<_, (String, String)>(
                "SELECT id, name FROM orgs WHERE instance_id = $1 AND name LIKE $2 LIMIT $3",
            )
            .bind(instance_id)
            .bind(&pattern)
            .bind(limit)
            .fetch_all(scoped.pool())
            .await?;
            for row in org_rows {
                results.push(SearchRecord {
                    resource_type: "org".to_string(),
                    id: row.0.clone(),
                    title: row.1,
                    subtitle: format!("Organization {}", row.0),
                });
            }
        }
        Db::Spanner(spanner) => {
            let mut user_stmt = Statement::new(
                "SELECT id, identifier, display_name FROM users \
                 WHERE instance_id = @instance_id AND (identifier LIKE @pattern OR display_name LIKE @pattern) LIMIT @limit",
            );
            user_stmt.add_param("instance_id", &instance_id);
            user_stmt.add_param("pattern", &pattern);
            user_stmt.add_param("limit", &limit);
            for row in spanner_query_all(spanner, user_stmt).await? {
                let id = row.column_by_name::<String>("id").unwrap_or_default();
                let identifier = row
                    .column_by_name::<String>("identifier")
                    .unwrap_or_default();
                let display_name = row
                    .column_by_name::<String>("display_name")
                    .unwrap_or_default();
                results.push(SearchRecord {
                    resource_type: "user".to_string(),
                    id,
                    title: display_name.clone(),
                    subtitle: identifier,
                });
            }
            let mut org_stmt = Statement::new(
                "SELECT id, name FROM orgs WHERE instance_id = @instance_id AND name LIKE @pattern LIMIT @limit",
            );
            org_stmt.add_param("instance_id", &instance_id);
            org_stmt.add_param("pattern", &pattern);
            org_stmt.add_param("limit", &limit);
            for row in spanner_query_all(spanner, org_stmt).await? {
                let id = row.column_by_name::<String>("id").unwrap_or_default();
                results.push(SearchRecord {
                    resource_type: "org".to_string(),
                    id: id.clone(),
                    title: row.column_by_name::<String>("name").unwrap_or_default(),
                    subtitle: format!("Organization {id}"),
                });
            }
        }
    }
    Ok(results)
}

// ─── Provider ───

pub async fn delete_provider(db: &Db, instance_id: &str, id: &str) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(
                sqlx::query("DELETE FROM providers WHERE instance_id = $1 AND id = $2")
                    .bind(instance_id)
                    .bind(id)
                    .execute(scoped.pool())
                    .await?
                    .rows_affected()
                    > 0,
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id FROM providers WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            if spanner_query_optional(spanner, stmt).await?.is_none() {
                return Ok(false);
            }
            let mut delete_stmt = Statement::new(
                "DELETE FROM providers WHERE instance_id = @instance_id AND id = @id",
            );
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
