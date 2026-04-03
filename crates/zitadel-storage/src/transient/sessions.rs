use serde_json::Value;
use uuid::Uuid;
use zitadel_crypto::token_hash;
use zitadel_db::Dialect;

use super::{CreatedSession, SessionRecord, SqlTransientCompatKv};

pub(crate) fn map_session_row(
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
    ),
) -> SessionRecord {
    SessionRecord {
        id: row.0,
        user_id: row.1,
        org_id: row.2,
        token_hash: row.3,
        user_agent: row.4,
        ip_address: row.5,
        metadata: Value::Object(Default::default()),
        created_at: row.6,
        expires_at: row.7,
        revoked_at: row.8,
    }
}

pub(crate) async fn create_session_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    user_id: &str,
    org_id: &str,
    user_agent: &str,
    ip_address: &str,
) -> anyhow::Result<CreatedSession> {
    let scoped = kv.scoped(instance_id);
    let session_id = Uuid::new_v4().to_string();
    let token = Uuid::new_v4().to_string();
    let hashed_token = token_hash(&token);
    let org = if org_id.is_empty() { "_global" } else { org_id };
    let expires_expr = match scoped.dialect() {
        Dialect::Postgres => "CURRENT_TIMESTAMP + INTERVAL '24 hours'",
        Dialect::Sqlite => "datetime(CURRENT_TIMESTAMP, '+24 hours')",
    };
    let sql = format!(
        "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, {}, {})",
        scoped.timestamp_now(),
        expires_expr,
    );

    sqlx::query(&sql)
        .bind(&session_id)
        .bind(scoped.instance_id())
        .bind(user_id)
        .bind(org)
        .bind(&hashed_token)
        .bind(user_agent)
        .bind(ip_address)
        .execute(scoped.pool())
        .await?;

    Ok(CreatedSession { session_id, token })
}

pub(crate) async fn find_session_by_token_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    raw_token: &str,
) -> anyhow::Result<Option<SessionRecord>> {
    let scoped = kv.scoped(instance_id);
    let created_at = scoped.as_text("created_at");
    let expires_at = scoped.as_text("expires_at");
    let revoked_at = scoped.as_text("revoked_at");
    let sql = format!(
        "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, {created_at}, {expires_at}, {revoked_at} \
         FROM sessions \
         WHERE instance_id = $1 AND token_hash = $2 AND revoked_at IS NULL \
         AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)"
    );
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(token_hash(raw_token))
        .fetch_optional(scoped.pool())
        .await?;

    Ok(row.map(map_session_row))
}

pub(crate) async fn list_sessions_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
) -> anyhow::Result<Vec<SessionRecord>> {
    let scoped = kv.scoped(instance_id);
    let created_at = scoped.as_text("created_at");
    let expires_at = scoped.as_text("expires_at");
    let revoked_at = scoped.as_text("revoked_at");
    let sql = format!(
        "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, {created_at}, {expires_at}, {revoked_at} \
         FROM sessions WHERE instance_id = $1 ORDER BY created_at DESC LIMIT 50"
    );
    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .fetch_all(scoped.pool())
        .await?;

    Ok(rows.into_iter().map(map_session_row).collect())
}

pub(crate) async fn get_session_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    session_id: &str,
) -> anyhow::Result<Option<SessionRecord>> {
    let scoped = kv.scoped(instance_id);
    let created_at = scoped.as_text("created_at");
    let expires_at = scoped.as_text("expires_at");
    let revoked_at = scoped.as_text("revoked_at");
    let sql = format!(
        "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, {created_at}, {expires_at}, {revoked_at} \
         FROM sessions WHERE instance_id = $1 AND id = $2"
    );
    let row: Option<(
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(session_id)
        .fetch_optional(scoped.pool())
        .await?;

    Ok(row.map(map_session_row))
}

pub(crate) async fn revoke_session_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    session_id: &str,
) -> anyhow::Result<bool> {
    let scoped = kv.scoped(instance_id);
    let result = sqlx::query(
        "UPDATE sessions SET revoked_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND id = $2",
    )
    .bind(scoped.instance_id())
    .bind(session_id)
    .execute(scoped.pool())
    .await?;

    Ok(result.rows_affected() > 0)
}
