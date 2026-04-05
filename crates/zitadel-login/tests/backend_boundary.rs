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
fn converted_login_runtime_paths_do_not_bypass_db_repositories() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for rel in [
        "src/legacy.rs",
        "src/session.rs",
        "src/sso.rs",
        "src/steps.rs",
    ] {
        assert_no_sql_runtime_calls(&manifest.join(rel));
    }
}

#[test]
fn login_router_no_longer_uses_spanner_guard() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_source_does_not_contain(&manifest.join("src/lib.rs"), "spanner_login_guard");
}
