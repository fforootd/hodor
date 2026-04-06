pub mod billing;
pub mod infra;
pub mod license;
pub mod staff;
pub mod support;
pub mod usage;

use zitadel_config::CloudConfig;

/// Check whether cloud features should activate.
/// Requires both `cloud.enabled = true` and a valid license key.
pub fn is_enabled(config: &CloudConfig, is_dev: bool) -> bool {
    config.enabled && license::validate(&config.license_key, is_dev).is_ok()
}

/// Register cloud routes and background workers onto the application.
///
/// Returns `Ok(false)` if cloud is not enabled (no-op).
/// Returns `Ok(true)` if cloud routes were registered.
/// Returns `Err` if cloud is enabled but the license key is invalid.
pub async fn register(
    _app: &mut axum::Router,
    config: &CloudConfig,
    is_dev: bool,
) -> anyhow::Result<bool> {
    if !config.enabled {
        return Ok(false);
    }

    let claims = license::validate(&config.license_key, is_dev)?;

    tracing::info!(
        sub = claims.sub,
        features = ?claims.features,
        max_instances = claims.max_instances,
        "cloud features enabled"
    );

    // TODO: register cloud-specific routes and background workers
    //
    // Planned:
    //   - POST /v1/cloud/subscriptions      (billing)
    //   - POST /v1/cloud/usage              (usage reporting)
    //   - Staff admin endpoints             (staff)
    //   - GCP LB/TLS cert provisioning      (infra)
    //   - HubSpot sync workers              (support)

    Ok(true)
}
