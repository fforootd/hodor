use crate::{DEFAULT_INSTANCE_ID, Db};
use uuid::Uuid;

/// Bootstrap creates the default org and admin user if they don't exist.
/// Safe to run repeatedly; it only inserts missing defaults.
pub async fn bootstrap(db: &Db) -> anyhow::Result<bool> {
    let pool = db.pool();
    let mut tx = pool.begin().await?;
    let mut changed = false;

    let org_id = match sqlx::query_as::<_, (String,)>(
        "SELECT id FROM orgs WHERE instance_id = $1 ORDER BY created_at ASC LIMIT 1",
    )
    .bind(DEFAULT_INSTANCE_ID)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some((id,)) => id,
        None => {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO orgs (id, instance_id, name, state) VALUES ($1, $2, 'Default', 'active')"
            )
            .bind(&id)
            .bind(DEFAULT_INSTANCE_ID)
            .execute(&mut *tx)
            .await?;
            changed = true;
            id
        }
    };

    let admin = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM users WHERE instance_id = $1 AND org_id = $2 AND identifier = 'admin' LIMIT 1"
    )
    .bind(DEFAULT_INSTANCE_ID)
    .bind(&org_id)
    .fetch_optional(&mut *tx)
    .await?;

    let admin_id = match admin {
        Some((id,)) => id,
        None => {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state) \
                 VALUES ($1, $2, $3, 'admin', 'Admin', 'human', 'active')"
            )
            .bind(&id)
            .bind(DEFAULT_INSTANCE_ID)
            .bind(&org_id)
            .execute(&mut *tx)
            .await?;
            changed = true;
            id
        }
    };

    // Ensure a default login flow exists.
    let has_login_flow = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM login_flows WHERE instance_id = $1 AND is_default = TRUE",
    )
    .bind(DEFAULT_INSTANCE_ID)
    .fetch_one(&mut *tx)
    .await?
    .0;
    if has_login_flow == 0 {
        let flow_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO login_flows (id, instance_id, name, strategy, is_default, enabled, state, priority, auth_methods) \
             VALUES ($1, $2, 'Default', 'identifier_first', TRUE, TRUE, 'active', 100, \
             '{\"password\":{\"enabled\":true,\"interactive\":true,\"position\":0},\"passkey\":{\"enabled\":true,\"interactive\":true,\"position\":1},\"sso\":{\"enabled\":true,\"interactive\":true,\"position\":2}}')",
        )
        .bind(&flow_id)
        .bind(DEFAULT_INSTANCE_ID)
        .execute(&mut *tx)
        .await?;
        changed = true;
        tracing::info!(flow_id, "bootstrapped default login flow");
    }

    tx.commit().await?;

    if changed {
        tracing::info!(
            org_id,
            admin_id,
            "bootstrapped default org, admin user, and login flow"
        );
    } else {
        tracing::debug!("bootstrap skipped — defaults already exist");
    }

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate;

    #[tokio::test]
    async fn bootstrap_is_idempotent() {
        let db = Db::open("").await.unwrap();
        migrate::migrate(&db).await.unwrap();

        assert!(bootstrap(&db).await.unwrap());
        assert!(!bootstrap(&db).await.unwrap());

        let orgs: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM orgs WHERE instance_id = $1 AND name = 'Default'")
                .bind(DEFAULT_INSTANCE_ID)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(orgs.0, 1);

        let admins: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM users WHERE instance_id = $1 AND identifier = 'admin'",
        )
        .bind(DEFAULT_INSTANCE_ID)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(admins.0, 1);

        let flows: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM login_flows WHERE instance_id = $1 AND is_default = TRUE",
        )
        .bind(DEFAULT_INSTANCE_ID)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(flows.0, 1);
    }
}
