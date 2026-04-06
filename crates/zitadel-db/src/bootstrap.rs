use crate::{DEFAULT_INSTANCE_ID, DEFAULT_ORG_ID, Db};
use google_cloud_spanner::mutation::insert_or_update;
use uuid::Uuid;

/// Bootstrap creates the default org and admin user if they don't exist.
/// Safe to run repeatedly; it only inserts missing defaults.
///
/// When `external_domain` is provided, a domain mapping is created so that
/// cloud-mode instance routing can resolve the host to the default instance.
pub async fn bootstrap(db: &Db, external_domain: Option<&str>) -> anyhow::Result<bool> {
    if let Some(spanner) = db.spanner() {
        bootstrap_spanner(spanner, external_domain).await?;
        tracing::info!("bootstrapped default org, admin user, and login flow for spanner");
        return Ok(true);
    }

    let pool = db.pool();
    let mut tx = pool.begin().await?;
    let mut changed = false;

    let operator_metadata = r#"{"capabilities":["operator_admin"]}"#;
    let root_updated = sqlx::query(
        "UPDATE instances \
         SET kind = 'root', state = 'active', placement_mode = 'global', updated_at = CURRENT_TIMESTAMP \
         WHERE instance_id = $1 AND kind != 'root'",
    )
    .bind(DEFAULT_INSTANCE_ID)
    .execute(&mut *tx)
    .await?;
    if root_updated.rows_affected() > 0 {
        changed = true;
    }

    let org_id = match sqlx::query_as::<_, (String,)>(
        "SELECT id FROM orgs WHERE instance_id = $1 AND id = $2 LIMIT 1",
    )
    .bind(DEFAULT_INSTANCE_ID)
    .bind(DEFAULT_ORG_ID)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some((id,)) => id,
        None => {
            sqlx::query(
                "INSERT INTO orgs (id, instance_id, name, state) VALUES ($1, $2, 'Default', 'active')"
            )
            .bind(DEFAULT_ORG_ID)
            .bind(DEFAULT_INSTANCE_ID)
            .execute(&mut *tx)
            .await?;
            changed = true;
            DEFAULT_ORG_ID.to_string()
        }
    };

    let admin = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM users WHERE instance_id = $1 AND identifier = 'admin' AND (org_id = $2 OR org_id IS NULL) LIMIT 1"
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
                "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, metadata) \
                 VALUES ($1, $2, $3, 'admin', 'Admin', 'human', 'active', $4)"
            )
            .bind(&id)
            .bind(DEFAULT_INSTANCE_ID)
            .bind(&org_id)
            .bind(operator_metadata)
            .execute(&mut *tx)
            .await?;
            changed = true;
            id
        }
    };

    let operator_metadata_updated = sqlx::query(
        "UPDATE users SET metadata = $1, updated_at = CURRENT_TIMESTAMP \
         WHERE instance_id = $2 AND id = $3 AND identifier = 'admin' \
         AND COALESCE(CAST(metadata AS TEXT), '{}') != $1",
    )
    .bind(operator_metadata)
    .bind(DEFAULT_INSTANCE_ID)
    .bind(&admin_id)
    .execute(&mut *tx)
    .await?;
    if operator_metadata_updated.rows_affected() > 0 {
        changed = true;
    }

    let membership_sql = match db.dialect() {
        crate::Dialect::Postgres => {
            "INSERT INTO memberships (instance_id, resource_type, resource_id, user_id, role) \
             VALUES ($1, 'org', $2, $3, 'owner') \
             ON CONFLICT (instance_id, resource_type, resource_id, user_id) DO NOTHING"
        }
        crate::Dialect::Sqlite => {
            "INSERT OR IGNORE INTO memberships (instance_id, resource_type, resource_id, user_id, role) \
             VALUES ($1, 'org', $2, $3, 'owner')"
        }
        crate::Dialect::Spanner => unreachable!(),
    };
    let membership_inserted = sqlx::query(membership_sql)
        .bind(DEFAULT_INSTANCE_ID)
        .bind(&org_id)
        .bind(&admin_id)
        .execute(&mut *tx)
        .await?;
    if membership_inserted.rows_affected() > 0 {
        changed = true;
    }

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

    // Ensure a domain mapping exists for the external domain so that
    // cloud-mode instance routing can resolve the host to the default instance.
    if let Some(domain) = external_domain.filter(|d| !d.is_empty()) {
        let domain_sql = match db.dialect() {
            crate::Dialect::Postgres => {
                "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                 VALUES ($1, $2, TRUE, 'active', TRUE) \
                 ON CONFLICT (domain) DO NOTHING"
            }
            crate::Dialect::Sqlite => {
                "INSERT OR IGNORE INTO domains (domain, instance_id, is_primary, state, verified) \
                 VALUES ($1, $2, TRUE, 'active', TRUE)"
            }
            crate::Dialect::Spanner => unreachable!(),
        };
        let domain_inserted = sqlx::query(domain_sql)
            .bind(domain)
            .bind(DEFAULT_INSTANCE_ID)
            .execute(&mut *tx)
            .await?;
        if domain_inserted.rows_affected() > 0 {
            changed = true;
            tracing::info!(domain, "bootstrapped domain mapping for default instance");
        }
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

async fn bootstrap_spanner(
    spanner: &crate::SpannerDb,
    external_domain: Option<&str>,
) -> anyhow::Result<()> {
    let operator_metadata = r#"{"capabilities":["operator_admin"]}"#;
    let auth_methods = r#"{"password":{"enabled":true,"interactive":true,"position":0},"passkey":{"enabled":true,"interactive":true,"position":1},"sso":{"enabled":true,"interactive":true,"position":2}}"#;

    let mut mutations = vec![
        insert_or_update(
            "instances",
            &[
                "instance_id",
                "kind",
                "state",
                "placement_mode",
                "feature_overrides",
            ],
            &[&DEFAULT_INSTANCE_ID, &"root", &"active", &"global", &"{}"],
        ),
        insert_or_update(
            "orgs",
            &["instance_id", "id", "name", "state", "metadata"],
            &[
                &DEFAULT_INSTANCE_ID,
                &DEFAULT_ORG_ID,
                &"Default",
                &"active",
                &"{}",
            ],
        ),
        insert_or_update(
            "users",
            &[
                "instance_id",
                "id",
                "org_id",
                "identifier",
                "display_name",
                "user_type",
                "state",
                "metadata",
            ],
            &[
                &DEFAULT_INSTANCE_ID,
                &"default-admin",
                &DEFAULT_ORG_ID,
                &"admin",
                &"Admin",
                &"human",
                &"active",
                &operator_metadata,
            ],
        ),
        insert_or_update(
            "memberships",
            &[
                "instance_id",
                "resource_type",
                "resource_id",
                "user_id",
                "role",
            ],
            &[
                &DEFAULT_INSTANCE_ID,
                &"org",
                &DEFAULT_ORG_ID,
                &"default-admin",
                &"owner",
            ],
        ),
        insert_or_update(
            "login_flows",
            &[
                "instance_id",
                "id",
                "org_id",
                "name",
                "strategy",
                "is_default",
                "enabled",
                "state",
                "priority",
                "auth_methods",
            ],
            &[
                &DEFAULT_INSTANCE_ID,
                &"default-login-flow",
                &DEFAULT_ORG_ID,
                &"Default",
                &"identifier_first",
                &true,
                &true,
                &"active",
                &100i64,
                &auth_methods,
            ],
        ),
    ];

    if let Some(domain) = external_domain.filter(|d| !d.is_empty()) {
        mutations.push(insert_or_update(
            "domains",
            &["domain", "instance_id", "is_primary", "state", "verified"],
            &[&domain, &DEFAULT_INSTANCE_ID, &true, &"active", &true],
        ));
    }

    spanner.client().apply(mutations).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate;

    #[tokio::test]
    async fn bootstrap_is_idempotent() {
        let db = Db::open("").await.unwrap();
        migrate::migrate(&db).await.unwrap();

        assert!(bootstrap(&db, None).await.unwrap());
        assert!(!bootstrap(&db, None).await.unwrap());

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
