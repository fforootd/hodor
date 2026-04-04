use super::{AuthRequestRedirect, AuthRequestRequirements, SqlKvStore};

pub(crate) async fn load_auth_request_redirect_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    auth_request_id: &str,
) -> anyhow::Result<Option<AuthRequestRedirect>> {
    if auth_request_id.is_empty() {
        return Ok(None);
    }
    let row = fetch_auth_request_redirect(&kv.scoped(instance_id), auth_request_id).await?;
    let row = match row {
        Some(row) => Some(row),
        None => match kv.authoritative_scoped(instance_id) {
            Some(scoped) => fetch_auth_request_redirect(&scoped, auth_request_id).await?,
            None => None,
        },
    };

    match row {
        Some((redirect_uri, state)) => Ok(Some(AuthRequestRedirect {
            redirect_uri,
            state,
        })),
        None => anyhow::bail!("auth request not found for instance {instance_id}"),
    }
}

pub(crate) async fn complete_auth_request_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    auth_request_id: &str,
    user_id: &str,
    code: &str,
    auth_time: Option<&str>,
) -> anyhow::Result<()> {
    if auth_request_id.is_empty() {
        return Ok(());
    }

    let result = update_auth_request(
        &kv.scoped(instance_id),
        auth_request_id,
        user_id,
        code,
        auth_time,
    )
    .await?;
    if result > 0 {
        return Ok(());
    }

    if let Some(scoped) = kv.authoritative_scoped(instance_id) {
        let result =
            update_auth_request(&scoped, auth_request_id, user_id, code, auth_time).await?;
        if result > 0 {
            return Ok(());
        }
    }

    anyhow::bail!("auth request not found for instance {instance_id}");
}

pub(crate) async fn load_auth_request_requirements_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    auth_request_id: &str,
) -> anyhow::Result<AuthRequestRequirements> {
    let row = fetch_auth_request_requirements(&kv.scoped(instance_id), auth_request_id).await?;
    let row = match row {
        Some(row) => Some(row),
        None => match kv.authoritative_scoped(instance_id) {
            Some(scoped) => fetch_auth_request_requirements(&scoped, auth_request_id).await?,
            None => None,
        },
    };

    Ok(row
        .map(|(prompt_json, max_age)| AuthRequestRequirements {
            prompt: serde_json::from_str::<Vec<String>>(&prompt_json).unwrap_or_default(),
            max_age: max_age.and_then(|value| u64::try_from(value).ok()),
        })
        .unwrap_or_default())
}

async fn fetch_auth_request_redirect(
    scoped: &zitadel_db::scoped::ScopedDb,
    auth_request_id: &str,
) -> anyhow::Result<Option<(String, String)>> {
    let row = sqlx::query_as(
        "SELECT redirect_uri, COALESCE(state, '') FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2",
    )
    .bind(scoped.instance_id())
    .bind(auth_request_id)
    .fetch_optional(scoped.pool())
    .await?;

    Ok(row)
}

async fn update_auth_request(
    scoped: &zitadel_db::scoped::ScopedDb,
    auth_request_id: &str,
    user_id: &str,
    code: &str,
    auth_time: Option<&str>,
) -> anyhow::Result<u64> {
    let result = if let Some(auth_time) = auth_time {
        sqlx::query(
            "UPDATE oidc_auth_requests SET user_id = $1, done = 1, auth_time = $2, code = $3 WHERE instance_id = $4 AND id = $5",
        )
        .bind(user_id)
        .bind(auth_time)
        .bind(code)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .execute(scoped.pool())
        .await?
    } else {
        sqlx::query(
            "UPDATE oidc_auth_requests SET user_id = $1, done = 1, auth_time = CURRENT_TIMESTAMP, code = $2 WHERE instance_id = $3 AND id = $4",
        )
        .bind(user_id)
        .bind(code)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .execute(scoped.pool())
        .await?
    };

    Ok(result.rows_affected())
}

async fn fetch_auth_request_requirements(
    scoped: &zitadel_db::scoped::ScopedDb,
    auth_request_id: &str,
) -> anyhow::Result<Option<(String, Option<i64>)>> {
    let prompt = scoped.as_text("prompt");
    let sql = format!(
        "SELECT COALESCE({prompt}, '[]'), max_age FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2"
    );

    let row = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .fetch_optional(scoped.pool())
        .await?;

    Ok(row)
}
