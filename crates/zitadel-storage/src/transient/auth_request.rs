use super::{
    AuthRequestRedirect, AuthRequestRequirements, SqlKvStore,
    semantics::{TransientStateMeta, TransientStateOutcome, transient_state_outcome},
};

pub(crate) async fn load_auth_request_redirect_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    auth_request_id: &str,
) -> anyhow::Result<Option<AuthRequestRedirect>> {
    if auth_request_id.is_empty() {
        return Ok(None);
    }
    let row = match fetch_auth_request_redirect(&kv.scoped(instance_id), auth_request_id).await? {
        TransientStateOutcome::Active(row) => Some(row),
        TransientStateOutcome::Inactive => None,
        TransientStateOutcome::Missing => match kv.authoritative_scoped(instance_id) {
            Some(scoped) => match fetch_auth_request_redirect(&scoped, auth_request_id).await? {
                TransientStateOutcome::Active(row) => Some(row),
                TransientStateOutcome::Inactive | TransientStateOutcome::Missing => None,
            },
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
    session_id: Option<&str>,
    code: &str,
    auth_time: Option<&str>,
) -> anyhow::Result<Option<AuthRequestRedirect>> {
    if auth_request_id.is_empty() {
        return Ok(None);
    }

    if let Some(redirect) = complete_auth_request_in_tx(
        &kv.scoped(instance_id),
        auth_request_id,
        user_id,
        session_id,
        code,
        auth_time,
    )
    .await?
    {
        return Ok(Some(redirect));
    }

    if let Some(scoped) = kv.authoritative_scoped(instance_id)
        && let Some(redirect) = complete_auth_request_in_tx(
            &scoped,
            auth_request_id,
            user_id,
            session_id,
            code,
            auth_time,
        )
        .await?
    {
        return Ok(Some(redirect));
    }

    anyhow::bail!("auth request not found for instance {instance_id}");
}

pub(crate) async fn load_auth_request_requirements_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    auth_request_id: &str,
) -> anyhow::Result<AuthRequestRequirements> {
    let row =
        match fetch_auth_request_requirements(&kv.scoped(instance_id), auth_request_id).await? {
            TransientStateOutcome::Active(row) => Some(row),
            TransientStateOutcome::Inactive => None,
            TransientStateOutcome::Missing => match kv.authoritative_scoped(instance_id) {
                Some(scoped) => {
                    match fetch_auth_request_requirements(&scoped, auth_request_id).await? {
                        TransientStateOutcome::Active(row) => Some(row),
                        TransientStateOutcome::Inactive | TransientStateOutcome::Missing => None,
                    }
                }
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
) -> anyhow::Result<TransientStateOutcome<(String, String)>> {
    let expires_at_epoch = scoped.epoch_seconds("expires_at");
    let sql = format!(
        "SELECT redirect_uri, COALESCE(state, ''), CASE WHEN done THEN 1 ELSE 0 END, {expires_at_epoch} \
         FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2"
    );
    let row: Option<(String, String, i64, Option<i64>)> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .fetch_optional(scoped.pool())
        .await?;

    Ok(match row {
        Some((redirect_uri, state, done, expires_at_epoch)) => transient_state_outcome(
            (redirect_uri, state),
            auth_request_meta(done, expires_at_epoch),
        ),
        None => TransientStateOutcome::Missing,
    })
}

async fn auth_request_state(
    scoped: &zitadel_db::scoped::ScopedDb,
    auth_request_id: &str,
) -> anyhow::Result<TransientStateOutcome<()>> {
    let expires_at_epoch = scoped.epoch_seconds("expires_at");
    let sql = format!(
        "SELECT CASE WHEN done THEN 1 ELSE 0 END, {expires_at_epoch} \
         FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2",
    );
    let row: Option<(i64, Option<i64>)> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .fetch_optional(scoped.pool())
        .await?;

    Ok(match row {
        Some((done, expires_at_epoch)) => {
            transient_state_outcome((), auth_request_meta(done, expires_at_epoch))
        }
        None => TransientStateOutcome::Missing,
    })
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
            "UPDATE oidc_auth_requests SET user_id = $1, done = 1, auth_time = $2, code = $3 \
             WHERE instance_id = $4 AND id = $5 AND done = 0 \
               AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
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
            "UPDATE oidc_auth_requests SET user_id = $1, done = 1, auth_time = CURRENT_TIMESTAMP, code = $2 \
             WHERE instance_id = $3 AND id = $4 AND done = 0 \
               AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
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

async fn complete_auth_request_in_tx(
    scoped: &zitadel_db::scoped::ScopedDb,
    auth_request_id: &str,
    user_id: &str,
    session_id: Option<&str>,
    code: &str,
    auth_time: Option<&str>,
) -> anyhow::Result<Option<AuthRequestRedirect>> {
    match auth_request_state(scoped, auth_request_id).await? {
        TransientStateOutcome::Inactive | TransientStateOutcome::Missing => return Ok(None),
        TransientStateOutcome::Active(()) => {}
    }

    let mut tx = scoped.pool().begin().await?;
    let expires_at_epoch = scoped.epoch_seconds("expires_at");
    let sql = format!(
        "SELECT redirect_uri, COALESCE(state, ''), CASE WHEN done THEN 1 ELSE 0 END, {expires_at_epoch} \
         FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2"
    );
    let row: Option<(String, String, i64, Option<i64>)> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .fetch_optional(&mut *tx)
        .await?;

    let Some((redirect_uri, state, done, expires_at_epoch)) = row else {
        tx.rollback().await?;
        return Ok(None);
    };
    match transient_state_outcome((), auth_request_meta(done, expires_at_epoch)) {
        TransientStateOutcome::Inactive | TransientStateOutcome::Missing => {
            tx.rollback().await?;
            return Ok(None);
        }
        TransientStateOutcome::Active(()) => {}
    }

    let rows_affected = if let Some(auth_time) = auth_time {
        sqlx::query(
            "UPDATE oidc_auth_requests SET user_id = $1, session_id = $2, done = 1, auth_time = $3, code = $4 \
             WHERE instance_id = $5 AND id = $6 AND done = 0 \
               AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
        )
        .bind(user_id)
        .bind(session_id.unwrap_or_default())
        .bind(auth_time)
        .bind(code)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
    } else {
        sqlx::query(
            "UPDATE oidc_auth_requests SET user_id = $1, session_id = $2, done = 1, auth_time = CURRENT_TIMESTAMP, code = $3 \
             WHERE instance_id = $4 AND id = $5 AND done = 0 \
               AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
        )
        .bind(user_id)
        .bind(session_id.unwrap_or_default())
        .bind(code)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
    };

    if rows_affected == 0 {
        tx.rollback().await?;
        return Ok(None);
    }

    tx.commit().await?;
    Ok(Some(AuthRequestRedirect {
        redirect_uri,
        state,
    }))
}

async fn fetch_auth_request_requirements(
    scoped: &zitadel_db::scoped::ScopedDb,
    auth_request_id: &str,
) -> anyhow::Result<TransientStateOutcome<(String, Option<i64>)>> {
    let prompt = scoped.as_text("prompt");
    let expires_at_epoch = scoped.epoch_seconds("expires_at");
    let sql = format!(
        "SELECT COALESCE({prompt}, '[]'), max_age, CASE WHEN done THEN 1 ELSE 0 END, {expires_at_epoch} \
         FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2"
    );

    let row: Option<(String, Option<i64>, i64, Option<i64>)> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .fetch_optional(scoped.pool())
        .await?;

    Ok(match row {
        Some((prompt_json, max_age, done, expires_at_epoch)) => transient_state_outcome(
            (prompt_json, max_age),
            auth_request_meta(done, expires_at_epoch),
        ),
        None => TransientStateOutcome::Missing,
    })
}

fn auth_request_meta(done: i64, expires_at_epoch: Option<i64>) -> TransientStateMeta {
    TransientStateMeta {
        expires_at_epoch: expires_at_epoch.and_then(|value| u64::try_from(value).ok()),
        consumed_or_done: done != 0,
        revoked: false,
    }
}
