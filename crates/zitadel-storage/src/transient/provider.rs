use uuid::Uuid;

use super::{ProviderAuthState, SqlKvStore};

pub(crate) async fn create_provider_auth_state_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    state: &ProviderAuthState,
) -> anyhow::Result<()> {
    let scoped = kv.scoped(instance_id);
    sqlx::query(
        "INSERT INTO oidc_rp_auth_states (id, instance_id, provider_id, state, nonce, pkce_verifier, flow_id, redirect_uri, expected_issuer, callback_uri) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(scoped.instance_id())
    .bind(&state.provider_id)
    .bind(&state.state)
    .bind(&state.nonce)
    .bind(&state.pkce_verifier)
    .bind(&state.flow_id)
    .bind(&state.redirect_uri)
    .bind(&state.expected_issuer)
    .bind(&state.callback_uri)
    .execute(scoped.pool())
    .await?;
    Ok(())
}

pub(crate) async fn consume_provider_auth_state_impl(
    kv: &SqlKvStore,
    instance_id: &str,
    state: &str,
) -> anyhow::Result<Option<ProviderAuthState>> {
    let scoped = kv.scoped(instance_id);
    let mut tx = scoped.pool().begin().await?;
    type ProviderAuthRow = (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    );
    let row: Option<ProviderAuthRow> = sqlx::query_as(
            "SELECT id, provider_id, state, nonce, pkce_verifier, flow_id, redirect_uri, expected_issuer, callback_uri \
             FROM oidc_rp_auth_states \
             WHERE instance_id = $1 AND state = $2 \
               AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
        )
        .bind(scoped.instance_id())
        .bind(state)
        .fetch_optional(&mut *tx)
        .await?;

    let Some(row) = row else {
        tx.rollback().await?;
        return Ok(None);
    };

    sqlx::query("DELETE FROM oidc_rp_auth_states WHERE instance_id = $1 AND id = $2")
        .bind(scoped.instance_id())
        .bind(&row.0)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Some(ProviderAuthState {
        provider_id: row.1,
        state: row.2,
        nonce: row.3,
        pkce_verifier: row.4,
        flow_id: row.5,
        redirect_uri: row.6,
        expected_issuer: row.7,
        callback_uri: row.8,
    }))
}
