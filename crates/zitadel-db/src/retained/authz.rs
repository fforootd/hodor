use anyhow::Context;
use google_cloud_spanner::{client::Error as SpannerError, row::Row, statement::Statement};
use zitadel_app::repo::{InstanceTrustLinkRecord, RoleAssignmentFilter, RoleAssignmentRecord};
use zitadel_authz::{RoleDefinition, builtin_role_definitions};

use super::ActiveRoleBindingRecord;
use crate::Db;

fn now_rfc3339ish() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let nanos = now.subsec_nanos();
    format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        1970 + secs / 31_536_000,
        (secs % 31_536_000) / 2_592_000 + 1,
        (secs % 2_592_000) / 86_400 + 1,
        (secs % 86_400) / 3600,
        (secs % 3600) / 60,
        secs % 60,
        nanos / 1_000_000
    )
}

fn parse_string_array(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn role_definition_from_sql_row(
    row: (String, String, String, String, bool, String),
) -> anyhow::Result<RoleDefinition> {
    Ok(RoleDefinition {
        role_key: row.0,
        relation_name: row.1,
        scope_kind: row.2,
        permissions: parse_string_array(&row.3),
        builtin: row.4,
        source_version: row.5,
    })
}

fn role_definition_from_spanner_row(row: Row) -> anyhow::Result<RoleDefinition> {
    Ok(RoleDefinition {
        role_key: row.column_by_name::<String>("role_key")?,
        relation_name: row.column_by_name::<String>("relation_name")?,
        scope_kind: row.column_by_name::<String>("scope_kind")?,
        permissions: parse_string_array(&row.column_by_name::<String>("permissions_json")?),
        builtin: row.column_by_name::<bool>("builtin")?,
        source_version: row.column_by_name::<String>("source_version")?,
    })
}

#[allow(clippy::type_complexity)]
fn role_assignment_from_sql_row(
    row: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
    ),
) -> RoleAssignmentRecord {
    RoleAssignmentRecord {
        assignment_id: row.0,
        enforcement_instance_id: row.1,
        scope_kind: row.2,
        scope_id: row.3,
        principal_ref: row.4,
        role_key: row.5,
        source_kind: row.6,
        origin_instance_id: row.7,
        approved_by: row.8,
        reason: row.9,
        expires_at: row.10,
        revoked_at: row.11,
        created_at: row.12,
        updated_at: row.13,
    }
}

fn role_assignment_from_spanner_row(row: Row) -> anyhow::Result<RoleAssignmentRecord> {
    Ok(RoleAssignmentRecord {
        assignment_id: row.column_by_name::<String>("assignment_id")?,
        enforcement_instance_id: row.column_by_name::<String>("enforcement_instance_id")?,
        scope_kind: row.column_by_name::<String>("scope_kind")?,
        scope_id: row.column_by_name::<String>("scope_id")?,
        principal_ref: row.column_by_name::<String>("principal_ref")?,
        role_key: row.column_by_name::<String>("role_key")?,
        source_kind: row.column_by_name::<String>("source_kind")?,
        origin_instance_id: row.column_by_name::<Option<String>>("origin_instance_id")?,
        approved_by: row.column_by_name::<Option<String>>("approved_by")?,
        reason: row.column_by_name::<Option<String>>("reason")?,
        expires_at: row.column_by_name::<Option<String>>("expires_at")?,
        revoked_at: row.column_by_name::<Option<String>>("revoked_at")?,
        created_at: row.column_by_name::<String>("created_at")?,
        updated_at: row.column_by_name::<String>("updated_at")?,
    })
}

fn trust_link_from_sql_row(
    row: (String, String, String, String, String, String, String),
) -> InstanceTrustLinkRecord {
    InstanceTrustLinkRecord {
        child_instance_id: row.0,
        issuer: row.1,
        audience: row.2,
        allowed_scopes: parse_string_array(&row.3),
        state: row.4,
        created_at: row.5,
        updated_at: row.6,
    }
}

fn trust_link_from_spanner_row(row: Row) -> anyhow::Result<InstanceTrustLinkRecord> {
    Ok(InstanceTrustLinkRecord {
        child_instance_id: row.column_by_name::<String>("child_instance_id")?,
        issuer: row.column_by_name::<String>("issuer")?,
        audience: row.column_by_name::<String>("audience")?,
        allowed_scopes: parse_string_array(&row.column_by_name::<String>("allowed_scopes")?),
        state: row.column_by_name::<String>("state")?,
        created_at: row.column_by_name::<String>("created_at")?,
        updated_at: row.column_by_name::<String>("updated_at")?,
    })
}

pub async fn seed_builtin_role_definitions(db: &Db) -> anyhow::Result<usize> {
    let definitions = builtin_role_definitions();
    match db {
        Db::Sql(_) => {
            let pool = db.pool();
            for definition in definitions {
                let permissions_json = serde_json::to_string(&definition.permissions)?;
                sqlx::query(
                    "INSERT INTO role_definitions (role_key, relation_name, scope_kind, permissions_json, builtin, source_version) \
                     VALUES ($1, $2, $3, $4, $5, $6) \
                     ON CONFLICT (role_key) DO UPDATE SET \
                       relation_name = EXCLUDED.relation_name, \
                       scope_kind = EXCLUDED.scope_kind, \
                       permissions_json = EXCLUDED.permissions_json, \
                       builtin = EXCLUDED.builtin, \
                       source_version = EXCLUDED.source_version, \
                       updated_at = CURRENT_TIMESTAMP",
                )
                .bind(&definition.role_key)
                .bind(&definition.relation_name)
                .bind(&definition.scope_kind)
                .bind(&permissions_json)
                .bind(definition.builtin)
                .bind(&definition.source_version)
                .execute(pool)
                .await?;
            }
        }
        Db::Spanner(spanner) => {
            let existing = list_role_definitions(db)
                .await?
                .into_iter()
                .map(|definition| definition.role_key)
                .collect::<std::collections::HashSet<_>>();

            let statements = definitions
                .iter()
                .map(|definition| {
                    let permissions_json =
                        serde_json::to_string(&definition.permissions).unwrap_or_else(|_| "[]".into());
                    if existing.contains(&definition.role_key) {
                        let mut stmt = Statement::new(
                            "UPDATE role_definitions \
                             SET relation_name = @relation_name, scope_kind = @scope_kind, \
                                 permissions_json = @permissions_json, builtin = @builtin, \
                                 source_version = @source_version, updated_at = CURRENT_TIMESTAMP() \
                             WHERE role_key = @role_key",
                        );
                        stmt.add_param("role_key", &definition.role_key);
                        stmt.add_param("relation_name", &definition.relation_name);
                        stmt.add_param("scope_kind", &definition.scope_kind);
                        stmt.add_param("permissions_json", &permissions_json);
                        stmt.add_param("builtin", &definition.builtin);
                        stmt.add_param("source_version", &definition.source_version);
                        stmt
                    } else {
                        let mut stmt = Statement::new(
                            "INSERT INTO role_definitions \
                             (role_key, relation_name, scope_kind, permissions_json, builtin, source_version) \
                             VALUES (@role_key, @relation_name, @scope_kind, @permissions_json, @builtin, @source_version)",
                        );
                        stmt.add_param("role_key", &definition.role_key);
                        stmt.add_param("relation_name", &definition.relation_name);
                        stmt.add_param("scope_kind", &definition.scope_kind);
                        stmt.add_param("permissions_json", &permissions_json);
                        stmt.add_param("builtin", &definition.builtin);
                        stmt.add_param("source_version", &definition.source_version);
                        stmt
                    }
                })
                .collect::<Vec<_>>();
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let statements = statements.clone();
                    Box::pin(async move {
                        for stmt in statements {
                            tx.update(stmt).await?;
                        }
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
        }
    }
    Ok(definitions.len())
}

pub async fn list_role_definitions(db: &Db) -> anyhow::Result<Vec<RoleDefinition>> {
    match db {
        Db::Sql(_) => {
            let rows = sqlx::query_as::<_, (String, String, String, String, bool, String)>(
                "SELECT role_key, relation_name, scope_kind, permissions_json, builtin, source_version \
                 FROM role_definitions ORDER BY role_key",
            )
            .fetch_all(db.pool())
            .await?;
            rows.into_iter().map(role_definition_from_sql_row).collect()
        }
        Db::Spanner(spanner) => {
            let stmt = Statement::new(
                "SELECT role_key, relation_name, scope_kind, permissions_json, builtin, source_version \
                 FROM role_definitions ORDER BY role_key",
            );
            let mut tx = spanner.client().single().await?;
            let mut rows = tx.query(stmt).await?;
            let mut out = Vec::new();
            while let Some(row) = rows.next().await? {
                out.push(role_definition_from_spanner_row(row)?);
            }
            Ok(out)
        }
    }
}

pub async fn create_role_assignment(
    db: &Db,
    assignment: &RoleAssignmentRecord,
) -> anyhow::Result<RoleAssignmentRecord> {
    match db {
        Db::Sql(_) => {
            sqlx::query(
                "INSERT INTO role_assignments \
                 (assignment_id, enforcement_instance_id, scope_kind, scope_id, principal_ref, role_key, source_kind, \
                  origin_instance_id, approved_by, reason, expires_at, revoked_at, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
            )
            .bind(&assignment.assignment_id)
            .bind(&assignment.enforcement_instance_id)
            .bind(&assignment.scope_kind)
            .bind(&assignment.scope_id)
            .bind(&assignment.principal_ref)
            .bind(&assignment.role_key)
            .bind(&assignment.source_kind)
            .bind(&assignment.origin_instance_id)
            .bind(&assignment.approved_by)
            .bind(&assignment.reason)
            .bind(&assignment.expires_at)
            .bind(&assignment.revoked_at)
            .bind(&assignment.created_at)
            .bind(&assignment.updated_at)
            .execute(db.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO role_assignments \
                 (assignment_id, enforcement_instance_id, scope_kind, scope_id, principal_ref, role_key, source_kind, \
                  origin_instance_id, approved_by, reason, expires_at, revoked_at, created_at, updated_at) \
                 VALUES (@assignment_id, @enforcement_instance_id, @scope_kind, @scope_id, @principal_ref, @role_key, \
                         @source_kind, @origin_instance_id, @approved_by, @reason, @expires_at, @revoked_at, @created_at, @updated_at)",
            );
            stmt.add_param("assignment_id", &assignment.assignment_id);
            stmt.add_param(
                "enforcement_instance_id",
                &assignment.enforcement_instance_id,
            );
            stmt.add_param("scope_kind", &assignment.scope_kind);
            stmt.add_param("scope_id", &assignment.scope_id);
            stmt.add_param("principal_ref", &assignment.principal_ref);
            stmt.add_param("role_key", &assignment.role_key);
            stmt.add_param("source_kind", &assignment.source_kind);
            stmt.add_param("origin_instance_id", &assignment.origin_instance_id);
            stmt.add_param("approved_by", &assignment.approved_by);
            stmt.add_param("reason", &assignment.reason);
            stmt.add_param("expires_at", &assignment.expires_at);
            stmt.add_param("revoked_at", &assignment.revoked_at);
            stmt.add_param("created_at", &assignment.created_at);
            stmt.add_param("updated_at", &assignment.updated_at);
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

    get_role_assignment(db, &assignment.assignment_id)
        .await?
        .context("created role assignment but could not reload it")
}

pub async fn get_role_assignment(
    db: &Db,
    assignment_id: &str,
) -> anyhow::Result<Option<RoleAssignmentRecord>> {
    match db {
        Db::Sql(_) => {
            let row = sqlx::query_as::<_, (
                String,
                String,
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                String,
                String,
            )>(
                "SELECT assignment_id, enforcement_instance_id, scope_kind, scope_id, principal_ref, role_key, source_kind, \
                        origin_instance_id, approved_by, reason, expires_at, revoked_at, created_at, updated_at \
                 FROM role_assignments WHERE assignment_id = $1",
            )
            .bind(assignment_id)
            .fetch_optional(db.pool())
            .await?;
            Ok(row.map(role_assignment_from_sql_row))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT assignment_id, enforcement_instance_id, scope_kind, scope_id, principal_ref, role_key, source_kind, \
                        origin_instance_id, approved_by, reason, expires_at, revoked_at, created_at, updated_at \
                 FROM role_assignments WHERE assignment_id = @assignment_id LIMIT 1",
            );
            stmt.add_param("assignment_id", &assignment_id);
            let mut tx = spanner.client().single().await?;
            let mut rows = tx.query(stmt).await?;
            Ok(match rows.next().await? {
                Some(row) => Some(role_assignment_from_spanner_row(row)?),
                None => None,
            })
        }
    }
}

pub async fn list_role_assignments(
    db: &Db,
    filter: &RoleAssignmentFilter,
) -> anyhow::Result<Vec<RoleAssignmentRecord>> {
    let mut rows = match db {
        Db::Sql(_) => {
            let rows = sqlx::query_as::<_, (
                String,
                String,
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                String,
                String,
            )>(
                "SELECT assignment_id, enforcement_instance_id, scope_kind, scope_id, principal_ref, role_key, source_kind, \
                        origin_instance_id, approved_by, reason, expires_at, revoked_at, created_at, updated_at \
                 FROM role_assignments ORDER BY created_at DESC, assignment_id DESC",
            )
            .fetch_all(db.pool())
            .await?;
            rows.into_iter()
                .map(role_assignment_from_sql_row)
                .collect::<Vec<_>>()
        }
        Db::Spanner(spanner) => {
            let stmt = Statement::new(
                "SELECT assignment_id, enforcement_instance_id, scope_kind, scope_id, principal_ref, role_key, source_kind, \
                        origin_instance_id, approved_by, reason, expires_at, revoked_at, created_at, updated_at \
                 FROM role_assignments ORDER BY created_at DESC, assignment_id DESC",
            );
            let mut tx = spanner.client().single().await?;
            let mut result = Vec::new();
            let mut stream = tx.query(stmt).await?;
            while let Some(row) = stream.next().await? {
                result.push(role_assignment_from_spanner_row(row)?);
            }
            result
        }
    };

    rows.retain(|assignment| {
        (filter.enforcement_instance_id.is_none()
            || filter.enforcement_instance_id.as_deref()
                == Some(assignment.enforcement_instance_id.as_str()))
            && (filter.scope_kind.is_none()
                || filter.scope_kind.as_deref() == Some(assignment.scope_kind.as_str()))
            && (filter.scope_id.is_none()
                || filter.scope_id.as_deref() == Some(assignment.scope_id.as_str()))
            && (filter.principal_ref.is_none()
                || filter.principal_ref.as_deref() == Some(assignment.principal_ref.as_str()))
            && (filter.role_key.is_none()
                || filter.role_key.as_deref() == Some(assignment.role_key.as_str()))
            && (filter.source_kind.is_none()
                || filter.source_kind.as_deref() == Some(assignment.source_kind.as_str()))
            && (filter.include_revoked || assignment.revoked_at.is_none())
    });

    Ok(rows)
}

pub async fn revoke_role_assignment(
    db: &Db,
    assignment_id: &str,
    revoked_at: &str,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => Ok(sqlx::query(
            "UPDATE role_assignments \
             SET revoked_at = $2, updated_at = CURRENT_TIMESTAMP \
             WHERE assignment_id = $1 AND revoked_at IS NULL",
        )
        .bind(assignment_id)
        .bind(revoked_at)
        .execute(db.pool())
        .await?
        .rows_affected()
            > 0),
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE role_assignments \
                 SET revoked_at = @revoked_at, updated_at = CURRENT_TIMESTAMP() \
                 WHERE assignment_id = @assignment_id AND revoked_at IS NULL",
            );
            stmt.add_param("assignment_id", &assignment_id);
            stmt.add_param("revoked_at", &revoked_at);
            let (_, updated) = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
            Ok(updated > 0)
        }
    }
}

pub async fn get_instance_trust_link(
    db: &Db,
    child_instance_id: &str,
    issuer: &str,
    audience: &str,
) -> anyhow::Result<Option<InstanceTrustLinkRecord>> {
    match db {
        Db::Sql(_) => {
            let row = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
                "SELECT child_instance_id, issuer, audience, allowed_scopes, state, created_at, updated_at \
                 FROM instance_trust_links \
                 WHERE child_instance_id = $1 AND issuer = $2 AND audience = $3",
            )
            .bind(child_instance_id)
            .bind(issuer)
            .bind(audience)
            .fetch_optional(db.pool())
            .await?;
            Ok(row.map(trust_link_from_sql_row))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT child_instance_id, issuer, audience, allowed_scopes, state, created_at, updated_at \
                 FROM instance_trust_links \
                 WHERE child_instance_id = @child_instance_id AND issuer = @issuer AND audience = @audience LIMIT 1",
            );
            stmt.add_param("child_instance_id", &child_instance_id);
            stmt.add_param("issuer", &issuer);
            stmt.add_param("audience", &audience);
            let mut tx = spanner.client().single().await?;
            let mut rows = tx.query(stmt).await?;
            Ok(match rows.next().await? {
                Some(row) => Some(trust_link_from_spanner_row(row)?),
                None => None,
            })
        }
    }
}

pub async fn list_active_role_bindings_for_scope(
    db: &Db,
    enforcement_instance_id: &str,
    scope_kind: &str,
    scope_id: &str,
    source_kind_prefix: Option<&str>,
) -> anyhow::Result<Vec<ActiveRoleBindingRecord>> {
    let now = now_rfc3339ish();
    let mut assignments = list_role_assignments(
        db,
        &RoleAssignmentFilter {
            enforcement_instance_id: Some(enforcement_instance_id.to_string()),
            scope_kind: Some(scope_kind.to_string()),
            scope_id: Some(scope_id.to_string()),
            include_revoked: false,
            ..Default::default()
        },
    )
    .await?;
    assignments.retain(|assignment| {
        source_kind_prefix.is_none_or(|prefix| assignment.source_kind.starts_with(prefix))
            && assignment
                .expires_at
                .as_deref()
                .is_none_or(|expires_at| expires_at > now.as_str())
    });
    Ok(assignments
        .into_iter()
        .map(|assignment| ActiveRoleBindingRecord {
            principal_ref: assignment.principal_ref,
            role_key: assignment.role_key,
        })
        .collect())
}
