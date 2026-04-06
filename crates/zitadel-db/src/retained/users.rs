use google_cloud_spanner::{client::Error as SpannerError, statement::Statement};

use super::{
    IdentityMetadata, LinkedIdentityRecord, UserClaimsRecord, UserRecord, metadata_has_capability,
    spanner_query_all, spanner_query_optional, spanner_query_scalar_i64,
};
use crate::Db;

pub async fn load_identity_metadata(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Option<IdentityMetadata>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let sql = format!(
                "SELECT COALESCE(org_id, ''), COALESCE({metadata}, '{{}}') FROM users WHERE instance_id = $1 AND id = $2"
            );
            let row: Option<(String, String)> = sqlx::query_as(&sql)
                .bind(scoped.instance_id())
                .bind(user_id)
                .fetch_optional(scoped.pool())
                .await?;
            Ok(row.map(|(org_id, metadata_json)| IdentityMetadata {
                org_id,
                metadata_json,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT org_id, IFNULL(metadata, '{}') AS metadata \
                 FROM users WHERE instance_id = @instance_id AND id = @user_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| IdentityMetadata {
                    org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
                    metadata_json: row.column_by_name::<String>("metadata").unwrap_or_default(),
                }))
        }
    }
}

pub async fn load_session_user_profile(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Option<(String, String)>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let row: Option<(String, String)> = sqlx::query_as(
                "SELECT identifier, display_name FROM users WHERE instance_id = $1 AND id = $2",
            )
            .bind(scoped.instance_id())
            .bind(user_id)
            .fetch_optional(scoped.pool())
            .await?;
            Ok(row)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT identifier, display_name \
                 FROM users WHERE instance_id = @instance_id AND id = @user_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            Ok(spanner_query_optional(spanner, stmt).await?.map(|row| {
                (
                    row.column_by_name::<String>("identifier")
                        .unwrap_or_default(),
                    row.column_by_name::<String>("display_name")
                        .unwrap_or_default(),
                )
            }))
        }
    }
}

pub async fn update_password_hash(
    db: &Db,
    instance_id: &str,
    user_id: &str,
    credential_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "UPDATE credentials SET data = {} WHERE instance_id = $1 AND user_id = $2 AND type = 'password'",
                scoped.json_bind(3),
            );
            sqlx::query(&sql)
                .bind(scoped.instance_id())
                .bind(user_id)
                .bind(credential_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE credentials SET data = @data \
                 WHERE instance_id = @instance_id AND user_id = @user_id AND type = 'password'",
            );
            stmt.add_param("data", &credential_json);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
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

pub async fn create_user(
    db: &Db,
    instance_id: &str,
    id: &str,
    org_id: &str,
    identifier: &str,
    display_name: &str,
    schema_id: &str,
    metadata_json: &str,
) -> anyhow::Result<UserRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, schema_id, metadata) \
                 VALUES ($1, $2, $3, $4, $5, 'human', 'active', $6, {})",
                scoped.json_bind(7),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(org_id)
                .bind(identifier)
                .bind(display_name)
                .bind(schema_id)
                .bind(metadata_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, schema_id, metadata) \
                 VALUES (@id, @instance_id, @org_id, @identifier, @display_name, 'human', 'active', @schema_id, @metadata)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("org_id", &org_id);
            stmt.add_param("identifier", &identifier);
            stmt.add_param("display_name", &display_name);
            stmt.add_param("schema_id", &schema_id);
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
    get_user(db, instance_id, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("created user but could not reload it"))
}

pub async fn get_user(db: &Db, instance_id: &str, id: &str) -> anyhow::Result<Option<UserRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, COALESCE(org_id, ''), identifier, display_name, user_type, state, schema_id, \
                        COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                 FROM users WHERE instance_id = $1 AND id = $2"
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
                    String,
                    String,
                    String,
                ),
            >(&sql)
            .bind(instance_id)
            .bind(id)
            .fetch_optional(scoped.pool())
            .await?
            .map(|row| UserRecord {
                id: row.0,
                org_id: row.1,
                identifier: row.2,
                display_name: row.3,
                user_type: row.4,
                state: row.5,
                schema_id: row.6,
                metadata_json: row.7,
                created_at: row.8,
                updated_at: row.9,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, IFNULL(org_id, '') AS org_id, identifier, display_name, user_type, state, schema_id, \
                        IFNULL(metadata, '{}') AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM users WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| UserRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
                    identifier: row
                        .column_by_name::<String>("identifier")
                        .unwrap_or_default(),
                    display_name: row
                        .column_by_name::<String>("display_name")
                        .unwrap_or_default(),
                    user_type: row
                        .column_by_name::<String>("user_type")
                        .unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    schema_id: row
                        .column_by_name::<String>("schema_id")
                        .unwrap_or_default(),
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

pub async fn list_users(
    db: &Db,
    instance_id: &str,
    after_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<UserRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, '{{}}', {created_at}, {updated_at} \
                 FROM users WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
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
                    String,
                    String,
                    String,
                ),
            >(&sql)
            .bind(instance_id)
            .bind(after_id)
            .bind(limit)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|row| UserRecord {
                    id: row.0,
                    org_id: row.1,
                    identifier: row.2,
                    display_name: row.3,
                    user_type: row.4,
                    state: row.5,
                    schema_id: row.6,
                    metadata_json: row.7,
                    created_at: row.8,
                    updated_at: row.9,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, '{}' AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM users WHERE instance_id = @instance_id AND id > @after_id ORDER BY id LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| UserRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
                    identifier: row
                        .column_by_name::<String>("identifier")
                        .unwrap_or_default(),
                    display_name: row
                        .column_by_name::<String>("display_name")
                        .unwrap_or_default(),
                    user_type: row
                        .column_by_name::<String>("user_type")
                        .unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    schema_id: row
                        .column_by_name::<String>("schema_id")
                        .unwrap_or_default(),
                    metadata_json: "{}".to_string(),
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

pub async fn update_user(
    db: &Db,
    instance_id: &str,
    id: &str,
    display_name: Option<&str>,
    state: Option<&str>,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let mut sets = Vec::new();
            let mut binds: Vec<String> = Vec::new();
            let mut next = 1usize;
            if let Some(display_name) = display_name {
                sets.push(format!("display_name = {}", scoped.placeholder(next)));
                binds.push(display_name.to_string());
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
                "UPDATE users SET {} WHERE instance_id = {} AND id = {}",
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
            if let Some(display_name) = display_name {
                sets.push("display_name = @display_name");
                stmt.add_param("display_name", &display_name);
            }
            if let Some(state) = state {
                sets.push("state = @state");
                stmt.add_param("state", &state);
            }
            if sets.is_empty() {
                return Ok(false);
            }
            let sql = format!(
                "UPDATE users SET {}, updated_at = CURRENT_TIMESTAMP() WHERE instance_id = @instance_id AND id = @id",
                sets.join(", ")
            );
            stmt = Statement::new(sql);
            if let Some(display_name) = display_name {
                stmt.add_param("display_name", &display_name);
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

pub async fn replace_password_credential(
    db: &Db,
    instance_id: &str,
    user_id: &str,
    credential_id: &str,
    credential_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "DELETE FROM credentials WHERE instance_id = $1 AND user_id = $2 AND type = 'password'",
            )
            .bind(instance_id)
            .bind(user_id)
            .execute(scoped.pool())
            .await?;
            let sql = format!(
                "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, $2, $3, 'password', {})",
                scoped.json_bind(4),
            );
            sqlx::query(&sql)
                .bind(credential_id)
                .bind(instance_id)
                .bind(user_id)
                .bind(credential_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut delete_stmt = Statement::new(
                "DELETE FROM credentials WHERE instance_id = @instance_id AND user_id = @user_id AND type = 'password'",
            );
            delete_stmt.add_param("instance_id", &instance_id);
            delete_stmt.add_param("user_id", &user_id);
            let mut insert_stmt = Statement::new(
                "INSERT INTO credentials (id, instance_id, user_id, type, data) \
                 VALUES (@id, @instance_id, @user_id, 'password', @data)",
            );
            insert_stmt.add_param("id", &credential_id);
            insert_stmt.add_param("instance_id", &instance_id);
            insert_stmt.add_param("user_id", &user_id);
            insert_stmt.add_param("data", &credential_json);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let delete_stmt = delete_stmt.clone();
                    let insert_stmt = insert_stmt.clone();
                    Box::pin(async move {
                        tx.update(delete_stmt).await?;
                        tx.update(insert_stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
        }
    }
    Ok(())
}

pub async fn user_has_capability(
    db: &Db,
    instance_id: &str,
    user_id: &str,
    capability: &str,
) -> anyhow::Result<bool> {
    Ok(load_identity_metadata(db, instance_id, user_id)
        .await?
        .map(|identity| metadata_has_capability(&identity.metadata_json, capability))
        .unwrap_or(false))
}

pub async fn load_user_claims_record(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Option<UserClaimsRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query_as::<_, (String, String)>(
                "SELECT identifier, display_name FROM users WHERE instance_id = $1 AND id = $2",
            )
            .bind(instance_id)
            .bind(user_id)
            .fetch_optional(scoped.pool())
            .await?
            .map(|row| UserClaimsRecord {
                identifier: row.0,
                display_name: row.1,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT identifier, display_name FROM users \
                 WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &user_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| UserClaimsRecord {
                    identifier: row
                        .column_by_name::<String>("identifier")
                        .unwrap_or_default(),
                    display_name: row
                        .column_by_name::<String>("display_name")
                        .unwrap_or_default(),
                }))
        }
    }
}

pub async fn find_active_user_by_identifier(
    db: &Db,
    instance_id: &str,
    identifier: &str,
) -> anyhow::Result<Option<UserRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, \
                        COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                 FROM users \
                 WHERE instance_id = $1 AND identifier = $2 AND state = 'active' \
                 LIMIT 1"
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
                    String,
                    String,
                    String,
                ),
            >(&sql)
            .bind(instance_id)
            .bind(identifier)
            .fetch_optional(scoped.pool())
            .await?
            .map(|row| UserRecord {
                id: row.0,
                org_id: row.1,
                identifier: row.2,
                display_name: row.3,
                user_type: row.4,
                state: row.5,
                schema_id: row.6,
                metadata_json: row.7,
                created_at: row.8,
                updated_at: row.9,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, \
                        IFNULL(metadata, '{}') AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM users \
                 WHERE instance_id = @instance_id AND identifier = @identifier AND state = 'active' \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("identifier", &identifier);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| UserRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
                    identifier: row
                        .column_by_name::<String>("identifier")
                        .unwrap_or_default(),
                    display_name: row
                        .column_by_name::<String>("display_name")
                        .unwrap_or_default(),
                    user_type: row
                        .column_by_name::<String>("user_type")
                        .unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    schema_id: row
                        .column_by_name::<String>("schema_id")
                        .unwrap_or_default(),
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

pub async fn find_linked_identity(
    db: &Db,
    instance_id: &str,
    provider_id: &str,
    external_sub: &str,
) -> anyhow::Result<Option<LinkedIdentityRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let raw_claims = scoped.as_text("raw_claims");
            let sql = format!(
                "SELECT id, user_id, provider_id, external_sub, COALESCE(external_email, ''), \
                        COALESCE({raw_claims}, '{{}}') \
                 FROM linked_identities \
                 WHERE instance_id = $1 AND provider_id = $2 AND external_sub = $3 \
                 LIMIT 1"
            );
            Ok(
                sqlx::query_as::<_, (String, String, String, String, String, String)>(&sql)
                    .bind(instance_id)
                    .bind(provider_id)
                    .bind(external_sub)
                    .fetch_optional(scoped.pool())
                    .await?
                    .map(|row| LinkedIdentityRecord {
                        id: row.0,
                        user_id: row.1,
                        provider_id: row.2,
                        external_sub: row.3,
                        external_email: row.4,
                        raw_claims_json: row.5,
                    }),
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, user_id, provider_id, external_sub, IFNULL(external_email, '') AS external_email, \
                        IFNULL(raw_claims, '{}') AS raw_claims \
                 FROM linked_identities \
                 WHERE instance_id = @instance_id AND provider_id = @provider_id AND external_sub = @external_sub \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("provider_id", &provider_id);
            stmt.add_param("external_sub", &external_sub);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| LinkedIdentityRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    user_id: row.column_by_name::<String>("user_id").unwrap_or_default(),
                    provider_id: row
                        .column_by_name::<String>("provider_id")
                        .unwrap_or_default(),
                    external_sub: row
                        .column_by_name::<String>("external_sub")
                        .unwrap_or_default(),
                    external_email: row
                        .column_by_name::<String>("external_email")
                        .unwrap_or_default(),
                    raw_claims_json: row
                        .column_by_name::<String>("raw_claims")
                        .unwrap_or_default(),
                }))
        }
    }
}

pub async fn touch_linked_identity(
    db: &Db,
    instance_id: &str,
    provider_id: &str,
    external_sub: &str,
    external_email: &str,
    raw_claims_json: &str,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "UPDATE linked_identities \
                 SET last_used_at = CURRENT_TIMESTAMP, external_email = $1, raw_claims = {} \
                 WHERE instance_id = $2 AND provider_id = $3 AND external_sub = $4",
                scoped.json_bind(5),
            );
            Ok(sqlx::query(&sql)
                .bind(external_email)
                .bind(instance_id)
                .bind(provider_id)
                .bind(external_sub)
                .bind(raw_claims_json)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE linked_identities \
                 SET last_used_at = CURRENT_TIMESTAMP(), external_email = @external_email, raw_claims = @raw_claims \
                 WHERE instance_id = @instance_id AND provider_id = @provider_id AND external_sub = @external_sub",
            );
            stmt.add_param("external_email", &external_email);
            stmt.add_param("raw_claims", &raw_claims_json);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("provider_id", &provider_id);
            stmt.add_param("external_sub", &external_sub);
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

pub async fn create_linked_identity_record(
    db: &Db,
    instance_id: &str,
    id: &str,
    user_id: &str,
    provider_id: &str,
    external_sub: &str,
    external_email: &str,
    raw_claims_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO linked_identities \
                 (id, instance_id, user_id, provider_id, external_sub, external_email, raw_claims) \
                 VALUES ($1, $2, $3, $4, $5, $6, {})",
                scoped.json_bind(7),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(user_id)
                .bind(provider_id)
                .bind(external_sub)
                .bind(external_email)
                .bind(raw_claims_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO linked_identities \
                 (id, instance_id, user_id, provider_id, external_sub, external_email, raw_claims) \
                 VALUES (@id, @instance_id, @user_id, @provider_id, @external_sub, @external_email, @raw_claims)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            stmt.add_param("provider_id", &provider_id);
            stmt.add_param("external_sub", &external_sub);
            stmt.add_param("external_email", &external_email);
            stmt.add_param("raw_claims", &raw_claims_json);
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

pub async fn count_users_for_schema(
    db: &Db,
    instance_id: &str,
    schema_id: &str,
) -> anyhow::Result<i64> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM users WHERE instance_id = $1 AND schema_id = $2",
            )
            .bind(instance_id)
            .bind(schema_id)
            .fetch_one(scoped.pool())
            .await?
            .0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT COUNT(*) AS total FROM users WHERE instance_id = @instance_id AND schema_id = @schema_id",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("schema_id", &schema_id);
            spanner_query_scalar_i64(spanner, stmt).await
        }
    }
}
