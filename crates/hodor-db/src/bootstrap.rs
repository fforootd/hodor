use crate::Db;
use uuid::Uuid;

/// Bootstrap creates the default org and admin user if they don't exist.
/// Only runs when bootstrap mode is "auto" and the users table is empty.
pub async fn bootstrap(db: &Db) -> anyhow::Result<bool> {
    let pool = db.pool();

    // Check if any users exist.
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&*pool)
        .await?;

    if row.0 > 0 {
        tracing::debug!("bootstrap skipped — users already exist");
        return Ok(false);
    }

    let org_id = Uuid::new_v4().to_string();
    let admin_id = Uuid::new_v4().to_string();

    // Create default org.
    sqlx::query(
        "INSERT INTO orgs (id, instance_id, name, state) VALUES (?, 'default', 'Default', 'active')"
    )
    .bind(&org_id)
    .execute(&*pool)
    .await?;

    // Create admin user.
    sqlx::query(
        "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state) \
         VALUES (?, 'default', ?, 'admin', 'Admin', 'human', 'active')"
    )
    .bind(&admin_id)
    .bind(&org_id)
    .execute(&*pool)
    .await?;

    tracing::info!(org_id, admin_id, "bootstrapped default org and admin user");
    Ok(true)
}
