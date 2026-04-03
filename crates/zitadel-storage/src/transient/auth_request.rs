use super::{AuthRequestRedirect, SqlTransientCompatKv};

pub(crate) async fn load_auth_request_redirect_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    auth_request_id: &str,
) -> anyhow::Result<Option<AuthRequestRedirect>> {
    if auth_request_id.is_empty() {
        return Ok(None);
    }

    let scoped = kv.scoped(instance_id);
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT redirect_uri, COALESCE(state, '') FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2",
    )
    .bind(scoped.instance_id())
    .bind(auth_request_id)
    .fetch_optional(scoped.pool())
    .await?;

    match row {
        Some((redirect_uri, state)) => Ok(Some(AuthRequestRedirect {
            redirect_uri,
            state,
        })),
        None => anyhow::bail!("auth request not found for instance {instance_id}"),
    }
}

pub(crate) async fn complete_auth_request_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    auth_request_id: &str,
    user_id: &str,
    code: &str,
) -> anyhow::Result<()> {
    if auth_request_id.is_empty() {
        return Ok(());
    }

    let scoped = kv.scoped(instance_id);
    let result = sqlx::query(
        "UPDATE oidc_auth_requests SET user_id = $1, done = 1, auth_time = CURRENT_TIMESTAMP, code = $2 WHERE instance_id = $3 AND id = $4",
    )
    .bind(user_id)
    .bind(code)
    .bind(scoped.instance_id())
    .bind(auth_request_id)
    .execute(scoped.pool())
    .await?;

    if result.rows_affected() == 0 {
        anyhow::bail!("auth request not found for instance {instance_id}");
    }

    Ok(())
}

pub(crate) async fn load_auth_request_prompts_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    auth_request_id: &str,
) -> anyhow::Result<Vec<String>> {
    let scoped = kv.scoped(instance_id);
    let prompt = scoped.as_text("prompt");
    let sql = format!(
        "SELECT COALESCE({prompt}, '[]') FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2"
    );
    let row: Option<(String,)> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .fetch_optional(scoped.pool())
        .await?;

    Ok(row
        .and_then(|(prompt_json,)| serde_json::from_str::<Vec<String>>(&prompt_json).ok())
        .unwrap_or_default())
}
