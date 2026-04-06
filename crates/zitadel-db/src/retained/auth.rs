use google_cloud_spanner::{client::Error as SpannerError, statement::Statement};

use super::{
    LoginFlowRecord, OidcAuthRequestRecord, OidcClientRecord, login_flow_from_spanner_row,
    login_flow_from_sql_row, spanner_query_all, spanner_query_optional,
};
use crate::Db;

pub async fn update_session_metadata(
    db: &Db,
    instance_id: &str,
    session_id: &str,
    metadata_json: &str,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "UPDATE sessions SET metadata = {} WHERE instance_id = $1 AND id = $2",
                scoped.json_bind(3),
            );
            Ok(sqlx::query(&sql)
                .bind(instance_id)
                .bind(session_id)
                .bind(metadata_json)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE sessions SET metadata = @metadata WHERE instance_id = @instance_id AND id = @id",
            );
            stmt.add_param("metadata", &metadata_json);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &session_id);
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

pub async fn get_oidc_client_record(
    db: &Db,
    instance_id: &str,
    client_id: &str,
) -> anyhow::Result<Option<OidcClientRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "SELECT id, COALESCE(client_secret, ''), COALESCE({}, '[]'), COALESCE({}, '[]'), \
                        COALESCE({}, '[]'), COALESCE({}, '[]'), COALESCE(state, 'active') \
                 FROM apps WHERE instance_id = $1 AND client_id = $2",
                scoped.as_text("redirect_uris"),
                scoped.as_text("post_logout_redirect_uris"),
                scoped.as_text("grant_types"),
                scoped.as_text("response_types"),
            );
            Ok(
                sqlx::query_as::<_, (String, String, String, String, String, String, String)>(&sql)
                    .bind(instance_id)
                    .bind(client_id)
                    .fetch_optional(scoped.pool())
                    .await?
                    .map(|row| OidcClientRecord {
                        app_id: row.0,
                        client_secret: row.1,
                        redirect_uris_json: row.2,
                        post_logout_redirect_uris_json: row.3,
                        grant_types_json: row.4,
                        response_types_json: row.5,
                        state: row.6,
                    }),
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, IFNULL(client_secret, '') AS client_secret, \
                        IFNULL(redirect_uris, '[]') AS redirect_uris, \
                        IFNULL(post_logout_redirect_uris, '[]') AS post_logout_redirect_uris, \
                        IFNULL(grant_types, '[]') AS grant_types, \
                        IFNULL(response_types, '[]') AS response_types, \
                        IFNULL(state, 'active') AS state \
                 FROM apps WHERE instance_id = @instance_id AND client_id = @client_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("client_id", &client_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| OidcClientRecord {
                    app_id: row.column_by_name::<String>("id").unwrap_or_default(),
                    client_secret: row
                        .column_by_name::<String>("client_secret")
                        .unwrap_or_default(),
                    redirect_uris_json: row
                        .column_by_name::<String>("redirect_uris")
                        .unwrap_or_default(),
                    post_logout_redirect_uris_json: row
                        .column_by_name::<String>("post_logout_redirect_uris")
                        .unwrap_or_default(),
                    grant_types_json: row
                        .column_by_name::<String>("grant_types")
                        .unwrap_or_default(),
                    response_types_json: row
                        .column_by_name::<String>("response_types")
                        .unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                }))
        }
    }
}

pub async fn create_oidc_auth_request_record(
    db: &Db,
    instance_id: &str,
    auth_request_id: &str,
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state_value: &str,
    nonce: &str,
    response_type: &str,
    code_challenge: &str,
    code_challenge_method: &str,
    prompt_json: &str,
    login_hint: &str,
    max_age: Option<i64>,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO oidc_auth_requests \
                 (id, instance_id, client_id, redirect_uri, scope, state, nonce, response_type, code_challenge, code_challenge_method, prompt, login_hint, max_age) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, {}, $12, $13)",
                scoped.json_bind(11),
            );
            sqlx::query(&sql)
                .bind(auth_request_id)
                .bind(instance_id)
                .bind(client_id)
                .bind(redirect_uri)
                .bind(scope)
                .bind(state_value)
                .bind(nonce)
                .bind(response_type)
                .bind(code_challenge)
                .bind(code_challenge_method)
                .bind(prompt_json)
                .bind(login_hint)
                .bind(max_age)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO oidc_auth_requests \
                 (id, instance_id, client_id, redirect_uri, scope, state, nonce, response_type, code_challenge, code_challenge_method, prompt, login_hint, max_age) \
                 VALUES \
                 (@id, @instance_id, @client_id, @redirect_uri, @scope, @state, @nonce, @response_type, @code_challenge, @code_challenge_method, @prompt, @login_hint, @max_age)",
            );
            stmt.add_param("id", &auth_request_id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("client_id", &client_id);
            stmt.add_param("redirect_uri", &redirect_uri);
            stmt.add_param("scope", &scope);
            stmt.add_param("state", &state_value);
            stmt.add_param("nonce", &nonce);
            stmt.add_param("response_type", &response_type);
            stmt.add_param("code_challenge", &code_challenge);
            stmt.add_param("code_challenge_method", &code_challenge_method);
            stmt.add_param("prompt", &prompt_json);
            stmt.add_param("login_hint", &login_hint);
            stmt.add_param("max_age", &max_age);
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

pub async fn consume_oidc_auth_code_record(
    db: &Db,
    instance_id: &str,
    code: &str,
) -> anyhow::Result<Option<OidcAuthRequestRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let mut tx = scoped.pool().begin().await?;
            let auth_time = scoped.epoch_seconds("auth_time");
            let prompt = scoped.as_text("prompt");
            let row: Option<(
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
                String,
                String,
                String,
                Option<i64>,
                Option<i64>,
            )> = sqlx::query_as(&format!(
                "SELECT id, user_id, COALESCE(session_id, ''), client_id, redirect_uri, scope, COALESCE(state, ''), nonce, \
                        response_type, code_challenge, code_challenge_method, COALESCE({prompt}, '[]'), \
                        COALESCE(login_hint, ''), max_age, {auth_time} \
                 FROM oidc_auth_requests WHERE instance_id = $1 AND code = $2 AND done = 1"
            ))
                .bind(instance_id)
                .bind(code)
                .fetch_optional(&mut *tx)
                .await?;
            let Some(row) = row else {
                tx.rollback().await?;
                return Ok(None);
            };
            sqlx::query("DELETE FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2")
                .bind(instance_id)
                .bind(&row.0)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            Ok(Some(OidcAuthRequestRecord {
                auth_request_id: row.0,
                user_id: row.1,
                session_id: row.2,
                client_id: row.3,
                redirect_uri: row.4,
                scope: row.5,
                state: row.6,
                nonce: row.7,
                response_type: row.8,
                code_challenge: row.9,
                code_challenge_method: row.10,
                prompt_json: row.11,
                login_hint: row.12,
                max_age: row.13,
                auth_time: row.14,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, user_id, IFNULL(session_id, '') AS session_id, client_id, redirect_uri, scope, IFNULL(state, '') AS state, nonce, \
                        response_type, code_challenge, code_challenge_method, IFNULL(prompt, '[]') AS prompt, \
                        IFNULL(login_hint, '') AS login_hint, max_age, \
                        UNIX_SECONDS(auth_time) AS auth_time \
                 FROM oidc_auth_requests \
                 WHERE instance_id = @instance_id AND code = @code AND done = TRUE LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("code", &code);
            let Some(row) = spanner_query_optional(spanner, stmt).await? else {
                return Ok(None);
            };
            let auth_request_id = row.column_by_name::<String>("id")?;
            let record = OidcAuthRequestRecord {
                auth_request_id: auth_request_id.clone(),
                user_id: row.column_by_name::<String>("user_id")?,
                session_id: row.column_by_name::<String>("session_id")?,
                client_id: row.column_by_name::<String>("client_id")?,
                redirect_uri: row.column_by_name::<String>("redirect_uri")?,
                scope: row.column_by_name::<String>("scope")?,
                state: row.column_by_name::<String>("state")?,
                nonce: row.column_by_name::<String>("nonce")?,
                response_type: row.column_by_name::<String>("response_type")?,
                code_challenge: row.column_by_name::<String>("code_challenge")?,
                code_challenge_method: row.column_by_name::<String>("code_challenge_method")?,
                prompt_json: row.column_by_name::<String>("prompt")?,
                login_hint: row.column_by_name::<String>("login_hint")?,
                max_age: row.column_by_name::<Option<i64>>("max_age")?,
                auth_time: row.column_by_name::<Option<i64>>("auth_time")?,
            };
            let mut delete_stmt = Statement::new(
                "DELETE FROM oidc_auth_requests WHERE instance_id = @instance_id AND id = @id",
            );
            delete_stmt.add_param("instance_id", &instance_id);
            delete_stmt.add_param("id", &auth_request_id);
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
            Ok(Some(record))
        }
    }
}

pub async fn create_login_flow(
    db: &Db,
    instance_id: &str,
    id: &str,
    name: &str,
    strategy: &str,
    config_json: &str,
    audience_json: &str,
    auth_methods_json: &str,
    is_default: bool,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO login_flows (id, instance_id, name, strategy, config, audience, auth_methods, is_default) \
                 VALUES ($1, $2, $3, $4, {}, {}, {}, $8)",
                scoped.json_bind(5),
                scoped.json_bind(6),
                scoped.json_bind(7),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(name)
                .bind(strategy)
                .bind(config_json)
                .bind(audience_json)
                .bind(auth_methods_json)
                .bind(is_default)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO login_flows (id, instance_id, name, strategy, config, audience, auth_methods, is_default) \
                 VALUES (@id, @instance_id, @name, @strategy, @config, @audience, @auth_methods, @is_default)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("name", &name);
            stmt.add_param("strategy", &strategy);
            stmt.add_param("config", &config_json);
            stmt.add_param("audience", &audience_json);
            stmt.add_param("auth_methods", &auth_methods_json);
            stmt.add_param("is_default", &is_default);
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

pub async fn list_login_flow_records(
    db: &Db,
    instance_id: &str,
    after_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<LoginFlowRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let is_default = scoped.bool_as_int("is_default");
            let enabled = scoped.bool_as_int("enabled");
            let config = scoped.as_text("config");
            let audience = scoped.as_text("audience");
            let auth_methods = scoped.as_text("auth_methods");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, name, strategy, state, {is_default}, {enabled}, priority, \
                        COALESCE({config}, '{{}}'), COALESCE({audience}, '{{}}'), COALESCE({auth_methods}, '{{}}'), \
                        {created_at}, {updated_at} \
                 FROM login_flows WHERE instance_id = $1 AND id > $2 ORDER BY priority DESC, name LIMIT $3"
            );
            let rows = sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    i64,
                    i64,
                    i64,
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
            Ok(rows.into_iter().map(login_flow_from_sql_row).collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name, strategy, state, is_default, enabled, priority, \
                        IFNULL(config, '{}') AS config, IFNULL(audience, '{}') AS audience, \
                        IFNULL(auth_methods, '{}') AS auth_methods, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM login_flows WHERE instance_id = @instance_id AND id > @after_id \
                 ORDER BY priority DESC, name LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(login_flow_from_spanner_row)
                .collect())
        }
    }
}

pub async fn get_login_flow_record(
    db: &Db,
    instance_id: &str,
    id: &str,
) -> anyhow::Result<Option<LoginFlowRecord>> {
    let rows = list_login_flow_records(db, instance_id, "", i64::MAX).await?;
    Ok(rows.into_iter().find(|row| row.id == id))
}

pub async fn update_login_flow(
    db: &Db,
    instance_id: &str,
    id: &str,
    name: &str,
    strategy: &str,
    config_json: &str,
    auth_methods_json: &str,
    is_default: bool,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "UPDATE login_flows SET name = $1, strategy = $2, config = {}, auth_methods = {}, is_default = $5, updated_at = CURRENT_TIMESTAMP \
                 WHERE instance_id = $6 AND id = $7",
                scoped.json_bind(3),
                scoped.json_bind(4),
            );
            Ok(sqlx::query(&sql)
                .bind(name)
                .bind(strategy)
                .bind(config_json)
                .bind(auth_methods_json)
                .bind(is_default)
                .bind(instance_id)
                .bind(id)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE login_flows SET name = @name, strategy = @strategy, config = @config, \
                 auth_methods = @auth_methods, is_default = @is_default, updated_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND id = @id",
            );
            stmt.add_param("name", &name);
            stmt.add_param("strategy", &strategy);
            stmt.add_param("config", &config_json);
            stmt.add_param("auth_methods", &auth_methods_json);
            stmt.add_param("is_default", &is_default);
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

pub async fn set_login_flow_state(
    db: &Db,
    instance_id: &str,
    id: &str,
    state: &str,
    enabled: bool,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query(
                "UPDATE login_flows SET state = $1, enabled = $2, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $3 AND id = $4",
            )
            .bind(state)
            .bind(enabled)
            .bind(instance_id)
            .bind(id)
            .execute(scoped.pool())
            .await?
            .rows_affected()
            > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE login_flows SET state = @state, enabled = @enabled, updated_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND id = @id",
            );
            stmt.add_param("state", &state);
            stmt.add_param("enabled", &enabled);
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

pub async fn resolve_login_flow(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Option<(String, String)>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query_as::<_, (String, String)>(
                "SELECT id, name FROM login_flows WHERE instance_id = $1 AND enabled = TRUE ORDER BY is_default DESC, priority DESC LIMIT 1",
            )
            .bind(instance_id)
            .fetch_optional(scoped.pool())
            .await?)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name FROM login_flows WHERE instance_id = @instance_id AND enabled = TRUE ORDER BY is_default DESC, priority DESC LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt).await?.map(|row| {
                (
                    row.column_by_name::<String>("id").unwrap_or_default(),
                    row.column_by_name::<String>("name").unwrap_or_default(),
                )
            }))
        }
    }
}
