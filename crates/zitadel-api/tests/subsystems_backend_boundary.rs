use std::{fs, path::Path};

fn load_runtime_source(path: &Path) -> String {
    let source = fs::read_to_string(path).expect("read source");
    source
        .split("#[cfg(test)]")
        .next()
        .unwrap_or_default()
        .to_string()
}

fn assert_no_sql_runtime_calls(path: &Path) {
    let source = load_runtime_source(path);
    for forbidden in ["sqlx::query", ".pool()", ".scoped(", ".scoped_default()"] {
        assert!(
            !source.contains(forbidden),
            "{} still contains forbidden runtime SQL boundary marker `{forbidden}`",
            path.display()
        );
    }
}

fn assert_source_does_not_contain(path: &Path, forbidden: &str) {
    let source = fs::read_to_string(path).expect("read source");
    assert!(
        !source.contains(forbidden),
        "{} still contains forbidden marker `{forbidden}`",
        path.display()
    );
}

#[test]
fn api_runtime_handlers_stay_behind_repository_boundaries() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for rel in [
        "src/account.rs",
        "src/actions.rs",
        "src/admin.rs",
        "src/analytics.rs",
        "src/apps.rs",
        "src/auth.rs",
        "src/catalog.rs",
        "src/console.rs",
        "src/events.rs",
        "src/fga.rs",
        "src/groups.rs",
        "src/instances.rs",
        "src/jobs.rs",
        "src/login_flows.rs",
        "src/middleware.rs",
        "src/orgs.rs",
        "src/pats.rs",
        "src/projects.rs",
        "src/providers.rs",
        "src/search.rs",
        "src/sessions.rs",
        "src/settings.rs",
        "src/telemetry.rs",
        "src/users.rs",
    ] {
        let path = manifest.join(rel);
        if path.exists() {
            assert_no_sql_runtime_calls(&path);
        }
    }
}

#[test]
fn api_router_avoids_legacy_spanner_guard_wiring() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_source_does_not_contain(&manifest.join("src/lib.rs"), "spanner_backend_guard");
}

/// Handlers must not access `state.db` or `s.db` directly — all data access
/// should go through `state.app.repos.*` repository traits.
#[test]
fn api_handlers_do_not_access_db_field_directly() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let forbidden = ["state.db.", "s.db.", "&state.db", "&s.db"];
    for rel in [
        "src/account.rs",
        "src/actions.rs",
        "src/admin.rs",
        "src/analytics.rs",
        "src/apps.rs",
        "src/auth.rs",
        "src/catalog.rs",
        "src/console.rs",
        "src/events.rs",
        "src/fga.rs",
        "src/groups.rs",
        "src/instances.rs",
        "src/jobs.rs",
        "src/login_flows.rs",
        "src/observability.rs",
        "src/orgs.rs",
        "src/pats.rs",
        "src/projects.rs",
        "src/providers.rs",
        "src/search.rs",
        "src/sessions.rs",
        "src/settings.rs",
        "src/telemetry.rs",
        "src/users.rs",
        "src/generic_named_resource.rs",
    ] {
        let path = manifest.join(rel);
        if path.exists() {
            let source = load_runtime_source(&path);
            for pattern in forbidden {
                assert!(
                    !source.contains(pattern),
                    "{} contains forbidden direct DB access `{pattern}` — use state.app.repos.* instead",
                    path.display()
                );
            }
        }
    }
}

/// Handlers must not call `state.fga.*` directly — FGA operations should go
/// through `state.app.repos.fga_admin.*` or `state.app.repos.fga.*`.
#[test]
fn api_handlers_do_not_access_fga_service_directly() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let forbidden = ["state.fga.", "s.fga."];
    for rel in [
        "src/account.rs",
        "src/actions.rs",
        "src/apps.rs",
        "src/auth.rs",
        "src/catalog.rs",
        "src/console.rs",
        "src/events.rs",
        "src/fga.rs",
        "src/groups.rs",
        "src/jobs.rs",
        "src/login_flows.rs",
        "src/observability.rs",
        "src/orgs.rs",
        "src/pats.rs",
        "src/projects.rs",
        "src/providers.rs",
        "src/search.rs",
        "src/sessions.rs",
        "src/settings.rs",
        "src/telemetry.rs",
        "src/users.rs",
        "src/generic_named_resource.rs",
    ] {
        let path = manifest.join(rel);
        if path.exists() {
            let source = load_runtime_source(&path);
            for pattern in forbidden {
                assert!(
                    !source.contains(pattern),
                    "{} contains forbidden direct FGA access `{pattern}` — use state.app.repos.fga_admin.* instead",
                    path.display()
                );
            }
        }
    }
}
