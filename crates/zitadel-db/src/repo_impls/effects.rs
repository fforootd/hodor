use zitadel_app::{
    effect::{Effect, EffectStatus, EffectType},
    repo::{BoxFuture, EffectRepository},
};

use crate::Db;

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
    fn create_batch(
        &self,
        instance_id: &str,
        effects: &[Effect],
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let instance_id = instance_id.to_string();
        let effects = effects.to_vec();
        Box::pin(async move {
            if effects.is_empty() {
                return Ok(());
            }
            let scoped = self.db.scoped(instance_id);
            let pool = scoped.pool();
            for effect in &effects {
                sqlx::query(
                    "INSERT INTO effects (instance_id, id, event_id, effect_type, status, config, payload, attempt, max_attempts, next_retry_at, last_error)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, datetime('now'), $10)",
                )
                .bind(scoped.instance_id())
                .bind(&effect.id)
                .bind(&effect.event_id)
                .bind(effect.effect_type.as_str())
                .bind(effect.status.as_str())
                .bind(effect.config.to_string())
                .bind(effect.payload.to_string())
                .bind(effect.attempt)
                .bind(effect.max_attempts)
                .bind(&effect.last_error)
                .execute(&*pool)
                .await?;
            }
            Ok(())
        })
    }

    fn fetch_pending(
        &self,
        instance_id: &str,
        limit: u32,
    ) -> BoxFuture<'_, anyhow::Result<Vec<Effect>>> {
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            let scoped = self.db.scoped(instance_id);
            let rows: Vec<(String, String, String, String, String, String, String, i32, i32, String, String, String, Option<String>)> = sqlx::query_as(
                "SELECT id, event_id, effect_type, status, config, payload, last_error, attempt, max_attempts, next_retry_at, created_at, COALESCE(last_error, ''), completed_at
                 FROM effects
                 WHERE instance_id = $1
                   AND status IN ('pending', 'failed')
                   AND next_retry_at <= datetime('now')
                 ORDER BY next_retry_at ASC
                 LIMIT $2",
            )
            .bind(scoped.instance_id())
            .bind(limit as i64)
            .fetch_all(&*scoped.pool())
            .await?;

            let effects = rows
                .into_iter()
                .map(|(id, event_id, effect_type, status, config, payload, last_error, attempt, max_attempts, next_retry_at, created_at, _error2, completed_at)| {
                    Effect {
                        id,
                        event_id,
                        effect_type: EffectType::parse(&effect_type).unwrap_or(EffectType::Log),
                        status: EffectStatus::parse(&status).unwrap_or(EffectStatus::Pending),
                        config: serde_json::from_str(&config).unwrap_or_default(),
                        payload: serde_json::from_str(&payload).unwrap_or_default(),
                        attempt,
                        max_attempts,
                        next_retry_at,
                        last_error,
                        created_at,
                        completed_at,
                    }
                })
                .collect();
            Ok(effects)
        })
    }

    fn mark_completed(
        &self,
        instance_id: &str,
        effect_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let instance_id = instance_id.to_string();
        let effect_id = effect_id.to_string();
        Box::pin(async move {
            let scoped = self.db.scoped(instance_id);
            sqlx::query(
                "UPDATE effects SET status = 'completed', completed_at = datetime('now')
                 WHERE instance_id = $1 AND id = $2",
            )
            .bind(scoped.instance_id())
            .bind(&effect_id)
            .execute(&*scoped.pool())
            .await?;
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
        let instance_id = instance_id.to_string();
        let effect_id = effect_id.to_string();
        let error = error.to_string();
        let next_retry_at = next_retry_at.to_string();
        Box::pin(async move {
            let scoped = self.db.scoped(instance_id);
            sqlx::query(
                "UPDATE effects SET status = 'failed', attempt = attempt + 1, last_error = $3, next_retry_at = $4
                 WHERE instance_id = $1 AND id = $2",
            )
            .bind(scoped.instance_id())
            .bind(&effect_id)
            .bind(&error)
            .bind(&next_retry_at)
            .execute(&*scoped.pool())
            .await?;
            Ok(())
        })
    }

    fn mark_dead(
        &self,
        instance_id: &str,
        effect_id: &str,
        error: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let instance_id = instance_id.to_string();
        let effect_id = effect_id.to_string();
        let error = error.to_string();
        Box::pin(async move {
            let scoped = self.db.scoped(instance_id);
            sqlx::query(
                "UPDATE effects SET status = 'dead', last_error = $3, completed_at = datetime('now')
                 WHERE instance_id = $1 AND id = $2",
            )
            .bind(scoped.instance_id())
            .bind(&effect_id)
            .bind(&error)
            .execute(&*scoped.pool())
            .await?;
            Ok(())
        })
    }

    fn cleanup(
        &self,
        instance_id: &str,
        older_than: &str,
        limit: u32,
    ) -> BoxFuture<'_, anyhow::Result<u64>> {
        let instance_id = instance_id.to_string();
        let older_than = older_than.to_string();
        Box::pin(async move {
            let scoped = self.db.scoped(instance_id);
            let result = sqlx::query(
                "DELETE FROM effects
                 WHERE instance_id = $1
                   AND status IN ('completed', 'dead')
                   AND created_at < $2
                   AND rowid IN (
                       SELECT rowid FROM effects
                       WHERE instance_id = $1
                         AND status IN ('completed', 'dead')
                         AND created_at < $2
                       LIMIT $3
                   )",
            )
            .bind(scoped.instance_id())
            .bind(&older_than)
            .bind(limit as i64)
            .execute(&*scoped.pool())
            .await?;
            Ok(result.rows_affected())
        })
    }
}

/// Insert effects within an existing SQL transaction.
pub async fn insert_effects_in_tx(
    tx: &mut sqlx::AnyConnection,
    instance_id: &str,
    effects: &[Effect],
) -> anyhow::Result<()> {
    for effect in effects {
        sqlx::query(
            "INSERT INTO effects (instance_id, id, event_id, effect_type, status, config, payload, attempt, max_attempts, next_retry_at, last_error)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, datetime('now'), $10)",
        )
        .bind(instance_id)
        .bind(&effect.id)
        .bind(&effect.event_id)
        .bind(effect.effect_type.as_str())
        .bind(effect.status.as_str())
        .bind(effect.config.to_string())
        .bind(effect.payload.to_string())
        .bind(effect.attempt)
        .bind(effect.max_attempts)
        .bind(&effect.last_error)
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}
