use anyhow::Context;
use google_cloud_spanner::{
    client::Error as SpannerError, row::Row as SpannerRow, statement::Statement,
};
use zitadel_app::{
    effect::{Effect, EffectStatus, EffectType},
    repo::{BoxFuture, EffectRepository},
};

use super::entities::{json_string, json_value};
use crate::{Db, current_timestamp_sql, timestamp_plus_expr};

type EffectSqlRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    i32,
    i32,
    String,
    String,
    Option<String>,
);

#[derive(Clone)]
pub struct DbEffectRepository {
    db: Db,
}

impl DbEffectRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl EffectRepository for DbEffectRepository {
    fn enqueue_batch(
        &self,
        instance_id: &str,
        effects: &[Effect],
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let effects = effects.to_vec();
        Box::pin(async move {
            if effects.is_empty() {
                return Ok(());
            }

            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "INSERT INTO effects \
                         (instance_id, id, event_id, source_key, effect_type, status, config, payload, attempt, max_attempts, last_error) \
                         VALUES ($1, $2, $3, $4, $5, $6, {}, {}, $7, $8, $9) \
                         ON CONFLICT(instance_id, source_key) DO NOTHING",
                        scoped.json_bind(10),
                        scoped.json_bind(11),
                    );
                    for effect in &effects {
                        let config_json = json_string(&effect.config)?;
                        let payload_json = json_string(&effect.payload)?;
                        sqlx::query(&sql)
                            .bind(scoped.instance_id())
                            .bind(&effect.id)
                            .bind(&effect.event_id)
                            .bind(&effect.source_key)
                            .bind(effect.effect_type.as_str())
                            .bind(effect.status.as_str())
                            .bind(effect.attempt)
                            .bind(effect.max_attempts)
                            .bind(&effect.last_error)
                            .bind(&config_json)
                            .bind(&payload_json)
                            .execute(scoped.pool())
                            .await?;
                    }
                }
                Db::Spanner(spanner) => {
                    let prepared_effects = effects
                        .iter()
                        .map(|effect| {
                            Ok::<_, anyhow::Error>((
                                effect.clone(),
                                json_string(&effect.config).context("serialize effect config")?,
                                json_string(&effect.payload).context("serialize effect payload")?,
                            ))
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    let iid = instance_id.clone();
                    let _ = spanner
                        .client()
                        .read_write_transaction(|tx| {
                            let prepared_effects = prepared_effects.clone();
                            let iid = iid.clone();
                            Box::pin(async move {
                                for (effect, config_json, payload_json) in &prepared_effects {
                                    let mut exists = Statement::new(
                                        "SELECT id FROM effects \
                                         WHERE instance_id = @instance_id AND source_key = @source_key \
                                         LIMIT 1",
                                    );
                                    exists.add_param("instance_id", &iid);
                                    exists.add_param("source_key", &effect.source_key);
                                    let mut rows = tx.query(exists).await?;
                                    if rows.next().await?.is_some() {
                                        continue;
                                    }

                                    let mut stmt = Statement::new(
                                        "INSERT INTO effects \
                                         (instance_id, id, event_id, source_key, effect_type, status, config, payload, attempt, max_attempts, last_error) \
                                         VALUES \
                                         (@instance_id, @id, @event_id, @source_key, @effect_type, @status, @config, @payload, @attempt, @max_attempts, @last_error)",
                                    );
                                    stmt.add_param("instance_id", &iid);
                                    stmt.add_param("id", &effect.id);
                                    stmt.add_param("event_id", &effect.event_id);
                                    stmt.add_param("source_key", &effect.source_key);
                                    stmt.add_param("effect_type", &effect.effect_type.as_str());
                                    stmt.add_param("status", &effect.status.as_str());
                                    stmt.add_param("config", config_json);
                                    stmt.add_param("payload", payload_json);
                                    stmt.add_param("attempt", &(effect.attempt as i64));
                                    stmt.add_param("max_attempts", &(effect.max_attempts as i64));
                                    stmt.add_param("last_error", &effect.last_error);
                                    tx.update(stmt).await?;
                                }
                                Ok::<(), SpannerError>(())
                            })
                        })
                        .await?;
                }
            }

            Ok(())
        })
    }

    fn claim_due(
        &self,
        instance_id: &str,
        worker_id: &str,
        lease_ttl_secs: u64,
        limit: u32,
    ) -> BoxFuture<'_, anyhow::Result<Vec<Effect>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let worker_id = worker_id.to_string();
        Box::pin(async move {
            if limit == 0 {
                return Ok(Vec::new());
            }

            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let now_expr = current_timestamp_sql(scoped.dialect());
                    let lease_expr = timestamp_plus_expr(scoped.dialect(), lease_ttl_secs);
                    let due_predicate = format!(
                        "((status IN ('pending', 'failed') AND next_retry_at <= {now_expr}) \
                          OR (status = 'processing' AND lease_expires_at IS NOT NULL AND lease_expires_at <= {now_expr}))"
                    );
                    let claim_sql = format!(
                        "UPDATE effects \
                         SET status = 'processing', lease_owner = $2, lease_expires_at = {lease_expr} \
                         WHERE instance_id = $1 \
                           AND id IN ( \
                               SELECT id FROM effects \
                               WHERE instance_id = $1 AND {due_predicate} \
                               ORDER BY next_retry_at ASC, id ASC \
                               LIMIT $3 \
                           ) \
                           AND {due_predicate}"
                    );

                    let select_sql = effect_select_sql(
                        &scoped,
                        "instance_id = $1 AND lease_owner = $2 AND status = 'processing'",
                    );

                    let mut tx = scoped.pool().begin().await?;
                    sqlx::query(&claim_sql)
                        .bind(scoped.instance_id())
                        .bind(&worker_id)
                        .bind(limit as i64)
                        .execute(&mut *tx)
                        .await?;

                    let rows = sqlx::query_as::<_, EffectSqlRow>(&select_sql)
                        .bind(scoped.instance_id())
                        .bind(&worker_id)
                        .fetch_all(&mut *tx)
                        .await?;
                    tx.commit().await?;

                    Ok(rows.into_iter().map(effect_from_sql_row).collect())
                }
                Db::Spanner(spanner) => {
                    let iid = instance_id.clone();
                    let owner = worker_id.clone();
                    let (_, claimed) = spanner
                        .client()
                        .read_write_transaction(|tx| {
                            let iid = iid.clone();
                            let owner = owner.clone();
                            Box::pin(async move {
                                let mut due_stmt = Statement::new(
                                    "SELECT id FROM effects \
                                     WHERE instance_id = @instance_id \
                                       AND ( \
                                            (status IN ('pending', 'failed') AND next_retry_at <= CURRENT_TIMESTAMP()) \
                                            OR (status = 'processing' AND lease_expires_at IS NOT NULL AND lease_expires_at <= CURRENT_TIMESTAMP()) \
                                       ) \
                                     ORDER BY next_retry_at ASC, id ASC \
                                     LIMIT @limit",
                                );
                                due_stmt.add_param("instance_id", &iid);
                                due_stmt.add_param("limit", &(limit as i64));

                                let mut rows = tx.query(due_stmt).await?;
                                let mut ids = Vec::new();
                                while let Some(row) = rows.next().await? {
                                    ids.push(row.column_by_name::<String>("id").unwrap_or_default());
                                }
                                if ids.is_empty() {
                                    return Ok::<Vec<Effect>, SpannerError>(Vec::new());
                                }

                                for effect_id in &ids {
                                    let mut stmt = Statement::new(format!(
                                        "UPDATE effects \
                                         SET status = 'processing', \
                                             lease_owner = @lease_owner, \
                                             lease_expires_at = {} \
                                         WHERE instance_id = @instance_id AND id = @id",
                                        timestamp_plus_expr(crate::Dialect::Spanner, lease_ttl_secs),
                                    ));
                                    stmt.add_param("lease_owner", &owner);
                                    stmt.add_param("instance_id", &iid);
                                    stmt.add_param("id", effect_id);
                                    tx.update(stmt).await?;
                                }

                                let mut claimed_stmt = Statement::new(
                                    "SELECT id, event_id, source_key, effect_type, status, \
                                            IFNULL(config, '{}') AS config, IFNULL(payload, '{}') AS payload, \
                                            IFNULL(last_error, '') AS last_error, IFNULL(lease_owner, '') AS lease_owner, \
                                            CAST(lease_expires_at AS STRING) AS lease_expires_at, \
                                            attempt, max_attempts, \
                                            CAST(next_retry_at AS STRING) AS next_retry_at, \
                                            CAST(created_at AS STRING) AS created_at, \
                                            CAST(completed_at AS STRING) AS completed_at \
                                     FROM effects \
                                     WHERE instance_id = @instance_id AND lease_owner = @lease_owner AND status = 'processing' \
                                     ORDER BY next_retry_at ASC, id ASC \
                                     LIMIT @limit",
                                );
                                claimed_stmt.add_param("instance_id", &iid);
                                claimed_stmt.add_param("lease_owner", &owner);
                                claimed_stmt.add_param("limit", &(limit as i64));
                                let mut claimed_rows = tx.query(claimed_stmt).await?;
                                let mut claimed = Vec::new();
                                while let Some(row) = claimed_rows.next().await? {
                                    claimed.push(effect_from_spanner_row(&row));
                                }
                                Ok::<Vec<Effect>, SpannerError>(claimed)
                            })
                        })
                        .await?;
                    Ok(claimed)
                }
            }
        })
    }

    fn mark_completed(
        &self,
        instance_id: &str,
        effect_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let effect_id = effect_id.to_string();
        Box::pin(async move {
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "UPDATE effects \
                         SET status = 'completed', completed_at = {}, lease_owner = '', lease_expires_at = NULL \
                         WHERE instance_id = $1 AND id = $2",
                        current_timestamp_sql(scoped.dialect()),
                    );
                    sqlx::query(&sql)
                        .bind(scoped.instance_id())
                        .bind(&effect_id)
                        .execute(scoped.pool())
                        .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "UPDATE effects \
                         SET status = 'completed', completed_at = CURRENT_TIMESTAMP(), lease_owner = '', lease_expires_at = NULL \
                         WHERE instance_id = @instance_id AND id = @id",
                    );
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("id", &effect_id);
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
        })
    }

    fn record_failure(
        &self,
        instance_id: &str,
        effect_id: &str,
        error: &str,
        next_retry_at: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let effect_id = effect_id.to_string();
        let error = error.to_string();
        let next_retry_at = next_retry_at.to_string();
        Box::pin(async move {
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    sqlx::query(
                        "UPDATE effects \
                         SET status = 'failed', attempt = attempt + 1, last_error = $3, next_retry_at = $4, \
                             lease_owner = '', lease_expires_at = NULL \
                         WHERE instance_id = $1 AND id = $2",
                    )
                    .bind(scoped.instance_id())
                    .bind(&effect_id)
                    .bind(&error)
                    .bind(&next_retry_at)
                    .execute(scoped.pool())
                    .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "UPDATE effects \
                         SET status = 'failed', attempt = attempt + 1, last_error = @last_error, \
                             next_retry_at = TIMESTAMP(@next_retry_at), lease_owner = '', lease_expires_at = NULL \
                         WHERE instance_id = @instance_id AND id = @id",
                    );
                    stmt.add_param("last_error", &error);
                    stmt.add_param("next_retry_at", &next_retry_at);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("id", &effect_id);
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
        })
    }

    fn mark_dead(
        &self,
        instance_id: &str,
        effect_id: &str,
        error: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let effect_id = effect_id.to_string();
        let error = error.to_string();
        Box::pin(async move {
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "UPDATE effects \
                         SET status = 'dead', last_error = $3, completed_at = {}, lease_owner = '', lease_expires_at = NULL \
                         WHERE instance_id = $1 AND id = $2",
                        current_timestamp_sql(scoped.dialect()),
                    );
                    sqlx::query(&sql)
                        .bind(scoped.instance_id())
                        .bind(&effect_id)
                        .bind(&error)
                        .execute(scoped.pool())
                        .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "UPDATE effects \
                         SET status = 'dead', last_error = @last_error, completed_at = CURRENT_TIMESTAMP(), \
                             lease_owner = '', lease_expires_at = NULL \
                         WHERE instance_id = @instance_id AND id = @id",
                    );
                    stmt.add_param("last_error", &error);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("id", &effect_id);
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
        })
    }

    fn cleanup(
        &self,
        instance_id: &str,
        older_than: &str,
        limit: u32,
    ) -> BoxFuture<'_, anyhow::Result<u64>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let older_than = older_than.to_string();
        Box::pin(async move {
            if limit == 0 {
                return Ok(0);
            }

            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = "DELETE FROM effects \
                               WHERE instance_id = $1 \
                                 AND id IN ( \
                                     SELECT id FROM effects \
                                     WHERE instance_id = $1 \
                                       AND status IN ('completed', 'dead') \
                                       AND completed_at IS NOT NULL \
                                       AND completed_at < $2 \
                                     ORDER BY completed_at ASC, id ASC \
                                     LIMIT $3 \
                                 )";
                    let result = sqlx::query(sql)
                        .bind(scoped.instance_id())
                        .bind(&older_than)
                        .bind(limit as i64)
                        .execute(scoped.pool())
                        .await?;
                    Ok(result.rows_affected())
                }
                Db::Spanner(spanner) => {
                    let iid = instance_id.clone();
                    let cutoff = older_than.clone();
                    let (_, deleted) = spanner
                        .client()
                        .read_write_transaction(|tx| {
                            let iid = iid.clone();
                            let cutoff = cutoff.clone();
                            Box::pin(async move {
                                let mut ids_stmt = Statement::new(
                                    "SELECT id FROM effects \
                                     WHERE instance_id = @instance_id \
                                       AND status IN ('completed', 'dead') \
                                       AND completed_at IS NOT NULL \
                                       AND completed_at < TIMESTAMP(@older_than) \
                                     ORDER BY completed_at ASC, id ASC \
                                     LIMIT @limit",
                                );
                                ids_stmt.add_param("instance_id", &iid);
                                ids_stmt.add_param("older_than", &cutoff);
                                ids_stmt.add_param("limit", &(limit as i64));
                                let mut rows = tx.query(ids_stmt).await?;
                                let mut ids = Vec::new();
                                while let Some(row) = rows.next().await? {
                                    ids.push(row.column_by_name::<String>("id").unwrap_or_default());
                                }
                                let mut affected = 0u64;
                                for effect_id in ids {
                                    let mut stmt = Statement::new(
                                        "DELETE FROM effects WHERE instance_id = @instance_id AND id = @id",
                                    );
                                    stmt.add_param("instance_id", &iid);
                                    stmt.add_param("id", &effect_id);
                                    affected += tx.update(stmt).await? as u64;
                                }
                                Ok::<u64, SpannerError>(affected)
                            })
                        })
                        .await?;
                    Ok(deleted)
                }
            }
        })
    }
}

fn effect_select_sql(scoped: &crate::scoped::ScopedDb, predicate: &str) -> String {
    let config = scoped.as_text("config");
    let payload = scoped.as_text("payload");
    let next_retry_at = scoped.as_text("next_retry_at");
    let lease_expires_at = scoped.as_text("lease_expires_at");
    let created_at = scoped.as_text("created_at");
    let completed_at = scoped.as_text("completed_at");
    format!(
        "SELECT id, event_id, source_key, effect_type, status, \
                COALESCE({config}, '{{}}'), COALESCE({payload}, '{{}}'), \
                COALESCE(last_error, ''), COALESCE(lease_owner, ''), {lease_expires_at}, \
                attempt, max_attempts, {next_retry_at}, {created_at}, {completed_at} \
         FROM effects \
         WHERE {predicate} \
         ORDER BY next_retry_at ASC, id ASC"
    )
}

fn effect_from_sql_row(row: EffectSqlRow) -> Effect {
    Effect {
        id: row.0,
        event_id: row.1,
        source_key: row.2,
        effect_type: EffectType::parse(&row.3).unwrap_or(EffectType::Log),
        status: EffectStatus::parse(&row.4).unwrap_or(EffectStatus::Pending),
        config: json_value(&row.5),
        payload: json_value(&row.6),
        last_error: row.7,
        lease_owner: row.8,
        lease_expires_at: row.9,
        attempt: row.10,
        max_attempts: row.11,
        next_retry_at: row.12,
        created_at: row.13,
        completed_at: row.14,
    }
}

fn effect_from_spanner_row(row: &SpannerRow) -> Effect {
    Effect {
        id: row.column_by_name::<String>("id").unwrap_or_default(),
        event_id: row.column_by_name::<String>("event_id").unwrap_or_default(),
        source_key: row
            .column_by_name::<String>("source_key")
            .unwrap_or_default(),
        effect_type: EffectType::parse(
            &row.column_by_name::<String>("effect_type")
                .unwrap_or_else(|_| "log".to_string()),
        )
        .unwrap_or(EffectType::Log),
        status: EffectStatus::parse(
            &row.column_by_name::<String>("status")
                .unwrap_or_else(|_| "pending".to_string()),
        )
        .unwrap_or(EffectStatus::Pending),
        config: json_value(
            &row.column_by_name::<String>("config")
                .unwrap_or_else(|_| "{}".to_string()),
        ),
        payload: json_value(
            &row.column_by_name::<String>("payload")
                .unwrap_or_else(|_| "{}".to_string()),
        ),
        last_error: row
            .column_by_name::<String>("last_error")
            .unwrap_or_default(),
        lease_owner: row
            .column_by_name::<String>("lease_owner")
            .unwrap_or_default(),
        lease_expires_at: row
            .column_by_name::<Option<String>>("lease_expires_at")
            .unwrap_or(None),
        attempt: row.column_by_name::<i64>("attempt").unwrap_or_default() as i32,
        max_attempts: row.column_by_name::<i64>("max_attempts").unwrap_or(5) as i32,
        next_retry_at: row
            .column_by_name::<String>("next_retry_at")
            .unwrap_or_default(),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
        completed_at: row
            .column_by_name::<Option<String>>("completed_at")
            .unwrap_or(None),
    }
}

/// Insert effects within an existing SQL transaction.
pub async fn insert_effects_in_tx(
    tx: &mut sqlx::AnyConnection,
    scoped: &crate::scoped::ScopedDb,
    effects: &[Effect],
) -> anyhow::Result<()> {
    if effects.is_empty() {
        return Ok(());
    }

    let sql = format!(
        "INSERT INTO effects \
         (instance_id, id, event_id, source_key, effect_type, status, config, payload, attempt, max_attempts, last_error) \
         VALUES ($1, $2, $3, $4, $5, $6, {}, {}, $7, $8, $9) \
         ON CONFLICT(instance_id, source_key) DO NOTHING",
        scoped.json_bind(10),
        scoped.json_bind(11),
    );

    for effect in effects {
        let config_json = json_string(&effect.config)?;
        let payload_json = json_string(&effect.payload)?;
        sqlx::query(&sql)
            .bind(scoped.instance_id())
            .bind(&effect.id)
            .bind(&effect.event_id)
            .bind(&effect.source_key)
            .bind(effect.effect_type.as_str())
            .bind(effect.status.as_str())
            .bind(effect.attempt)
            .bind(effect.max_attempts)
            .bind(&effect.last_error)
            .bind(&config_json)
            .bind(&payload_json)
            .execute(&mut *tx)
            .await?;
    }

    Ok(())
}
