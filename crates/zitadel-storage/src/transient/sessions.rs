use serde_json::Value;
use uuid::Uuid;
use zitadel_crypto::token_hash;
use zitadel_db::Dialect;

use super::{
    CreatedSession, SessionRecord, SqlKvStore,
    semantics::{SessionLookupOutcome, session_lookup_outcome},
};

type SessionRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    Option<String>,
    Option<String>,
    Option<i64>,
);

pub(crate) fn map_session_row(row: SessionRow) -> SessionRecord {
    SessionRecord {
        id: row.0,
        user_id: row.1,
        org_id: row.2,
        token_hash: row.3,
        user_agent: row.4,
        ip_address: row.5,
        fingerprint: row.6,
        metadata: Value::Object(Default::default()),
        created_at: row.7,
        created_at_epoch: row.8 as u64,
        expires_at: row.9,
        revoked_at: row.10,
    }
}

fn classify_session_row(row: SessionRow) -> SessionLookupOutcome {
    let expires_at_epoch = row.11.map(|value| value as u64);
    session_lookup_outcome(map_session_row(row), expires_at_epoch)
}

pub(crate) async fn create_session_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    user_id: &str,
    org_id: &str,
    user_agent: &str,
    ip_address: &str,
    fingerprint: &str,
) -> anyhow::Result<CreatedSession> {
    let scoped = kv.scoped(instance_id);
    let session_id = Uuid::new_v4().to_string();
    let token = Uuid::new_v4().to_string();
    let hashed_token = token_hash(&token);
    let org: Option<&str> = if org_id.is_empty() {
        None
    } else {
        Some(org_id)
    };
    let max_age_secs = kv.session_max_age_secs().max(1);
    let expires_expr = match scoped.dialect() {
        Dialect::Postgres => format!("CURRENT_TIMESTAMP + INTERVAL '{max_age_secs} seconds'"),
        Dialect::Spanner => {
            format!("TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL {max_age_secs} SECOND)")
        }
        Dialect::Sqlite => format!("datetime(CURRENT_TIMESTAMP, '+{max_age_secs} seconds')"),
    };

    let sql = format!(
        "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, fingerprint, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, {}, {})",
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
        .bind(fingerprint)
        .execute(scoped.pool())
        .await?;

    let created_at = scoped.as_text("created_at");
    let created_at_epoch = scoped.epoch_seconds("created_at");
    let sql = format!(
        "SELECT {created_at}, {created_at_epoch} FROM sessions WHERE instance_id = $1 AND id = $2"
    );
    let (created_at, created_at_epoch): (String, i64) = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(&session_id)
        .fetch_one(scoped.pool())
        .await?;

    Ok(CreatedSession {
        session_id,
        token,
        created_at,
        created_at_epoch: created_at_epoch as u64,
    })
}

pub(crate) async fn find_session_by_token_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    raw_token: &str,
) -> anyhow::Result<Option<SessionRecord>> {
    match lookup_session_by_token_impl(kv, instance_id, raw_token).await? {
        SessionLookupOutcome::Active(record) => Ok(Some(record)),
        SessionLookupOutcome::Inactive | SessionLookupOutcome::Missing => Ok(None),
    }
}

pub(crate) async fn lookup_session_by_token_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    raw_token: &str,
) -> anyhow::Result<SessionLookupOutcome> {
    let hashed = token_hash(raw_token);
    if let Some(row) = fetch_session_by_token_unfiltered(&kv.scoped(instance_id), &hashed).await? {
        return Ok(classify_session_row(row));
    }

    let row = match kv.authoritative_scoped(instance_id) {
        Some(scoped) => fetch_session_by_token_unfiltered(&scoped, &hashed).await?,
        None => None,
    };

    Ok(row
        .map(classify_session_row)
        .unwrap_or(SessionLookupOutcome::Missing))
}

pub(crate) async fn list_sessions_impl(
    kv: &SqlKvStore,
    instance_id: &str,
) -> anyhow::Result<Vec<SessionRecord>> {
    let scoped = kv.scoped(instance_id);
    let created_at = scoped.as_text("created_at");
    let created_at_epoch = scoped.epoch_seconds("created_at");
    let expires_at = scoped.as_text("expires_at");
    let revoked_at = scoped.as_text("revoked_at");
    let expires_at_epoch = scoped.epoch_seconds("expires_at");
    let sql = format!(
        "SELECT id, user_id, COALESCE(org_id, ''), token_hash, user_agent, ip_address, COALESCE(fingerprint, ''), {created_at}, {created_at_epoch}, {expires_at}, {revoked_at}, {expires_at_epoch} \
         FROM sessions WHERE instance_id = $1 ORDER BY created_at DESC LIMIT 50"
    );
    let rows: Vec<SessionRow> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .fetch_all(scoped.pool())
        .await?;

    Ok(rows.into_iter().map(map_session_row).collect())
}

pub(crate) async fn get_session_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    session_id: &str,
) -> anyhow::Result<Option<SessionRecord>> {
    if let Some(row) = fetch_session_by_id(&kv.scoped(instance_id), session_id).await? {
        return Ok(Some(map_session_row(row)));
    }

    let row = match kv.authoritative_scoped(instance_id) {
        Some(scoped) => fetch_session_by_id(&scoped, session_id).await?,
        None => None,
    };

    Ok(row.map(map_session_row))
}

pub(crate) async fn revoke_session_impl(
    kv: &SqlKvStore,
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

async fn fetch_session_by_token_unfiltered(
    scoped: &zitadel_db::scoped::ScopedDb,
    hashed_token: &str,
) -> anyhow::Result<Option<SessionRow>> {
    let created_at = scoped.as_text("s.created_at");
    let created_at_epoch = scoped.epoch_seconds("s.created_at");
    let expires_at = scoped.as_text("s.expires_at");
    let revoked_at = scoped.as_text("s.revoked_at");
    let expires_at_epoch = scoped.epoch_seconds("s.expires_at");
    let sql = format!(
        "SELECT s.id, s.user_id, COALESCE(s.org_id, ''), s.token_hash, s.user_agent, s.ip_address, COALESCE(s.fingerprint, ''), {created_at}, {created_at_epoch}, {expires_at}, {revoked_at}, {expires_at_epoch} \
         FROM sessions s \
         JOIN users u ON u.instance_id = s.instance_id AND u.id = s.user_id \
         WHERE s.instance_id = $1 AND s.token_hash = $2 AND u.state = 'active'"
    );

    let row = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(hashed_token)
        .fetch_optional(scoped.pool())
        .await?;

    Ok(row)
}

async fn fetch_session_by_id(
    scoped: &zitadel_db::scoped::ScopedDb,
    session_id: &str,
) -> anyhow::Result<Option<SessionRow>> {
    let created_at = scoped.as_text("created_at");
    let created_at_epoch = scoped.epoch_seconds("created_at");
    let expires_at = scoped.as_text("expires_at");
    let revoked_at = scoped.as_text("revoked_at");
    let expires_at_epoch = scoped.epoch_seconds("expires_at");
    let sql = format!(
        "SELECT id, user_id, COALESCE(org_id, ''), token_hash, user_agent, ip_address, COALESCE(fingerprint, ''), {created_at}, {created_at_epoch}, {expires_at}, {revoked_at}, {expires_at_epoch} \
         FROM sessions WHERE instance_id = $1 AND id = $2"
    );

    let row = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(session_id)
        .fetch_optional(scoped.pool())
        .await?;

    Ok(row)
}
