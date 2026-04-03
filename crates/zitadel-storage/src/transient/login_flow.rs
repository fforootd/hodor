use serde_json::Value;

use super::{LoginFlowRuntimeState, NewLoginFlowState, SqlTransientCompatKv};

pub(crate) async fn create_login_flow_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    input: &NewLoginFlowState,
) -> anyhow::Result<()> {
    let scoped = kv.scoped(instance_id);
    let sql = format!(
        "INSERT INTO auth_states (id, instance_id, type, state, redirect_uri, data, step) \
         VALUES ($1, $2, 'login_flow', $3, $4, {}, 'identifier')",
        scoped.json_bind(5),
    );
    sqlx::query(&sql)
        .bind(&input.flow_id)
        .bind(scoped.instance_id())
        .bind(&input.state)
        .bind(&input.redirect_uri)
        .bind(serde_json::to_string(&input.data).unwrap_or_default())
        .execute(scoped.pool())
        .await?;
    Ok(())
}

pub(crate) async fn load_login_flow_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    flow_id: &str,
) -> anyhow::Result<Option<LoginFlowRuntimeState>> {
    let scoped = kv.scoped(instance_id);
    let sql = format!(
        "SELECT COALESCE(step, 'identifier'), COALESCE({}, '{{}}'), COALESCE(redirect_uri, '') \
         FROM auth_states WHERE instance_id = $1 AND id = $2 AND type = 'login_flow'",
        scoped.as_text("data"),
    );
    let row: Option<(String, String, String)> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(flow_id)
        .fetch_optional(scoped.pool())
        .await?;

    Ok(row.map(|(step, data, redirect_uri)| LoginFlowRuntimeState {
        flow_id: flow_id.to_string(),
        step,
        redirect_uri,
        data: serde_json::from_str(&data).unwrap_or_default(),
    }))
}

pub(crate) async fn set_login_flow_step_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    flow_id: &str,
    step: &str,
) -> anyhow::Result<bool> {
    let scoped = kv.scoped(instance_id);
    let result = sqlx::query(
        "UPDATE auth_states SET step = $1 WHERE instance_id = $2 AND id = $3 AND type = 'login_flow'",
    )
    .bind(step)
    .bind(scoped.instance_id())
    .bind(flow_id)
    .execute(scoped.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}

pub(crate) async fn advance_login_flow_to_password_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    flow_id: &str,
    user_id: &str,
    data: &Value,
) -> anyhow::Result<bool> {
    let scoped = kv.scoped(instance_id);
    let sql = format!(
        "UPDATE auth_states SET step = 'password', user_id = $1, data = {} WHERE instance_id = $2 AND id = $3",
        scoped.json_bind(4),
    );
    let result = sqlx::query(&sql)
        .bind(user_id)
        .bind(scoped.instance_id())
        .bind(flow_id)
        .bind(serde_json::to_string(data).unwrap_or_else(|_| "{}".into()))
        .execute(scoped.pool())
        .await?;
    Ok(result.rows_affected() > 0)
}

pub(crate) async fn complete_login_flow_impl(
    kv: &SqlTransientCompatKv,
    instance_id: &str,
    flow_id: &str,
) -> anyhow::Result<bool> {
    let scoped = kv.scoped(instance_id);
    let result = sqlx::query(
        "UPDATE auth_states SET step = 'complete', done = 1 WHERE instance_id = $1 AND id = $2",
    )
    .bind(scoped.instance_id())
    .bind(flow_id)
    .execute(scoped.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}
