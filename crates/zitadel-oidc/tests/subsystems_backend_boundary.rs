use std::{fs, path::Path};

fn load_runtime_source(path: &Path) -> String {
    let source = fs::read_to_string(path).expect("read source");
    source
        .split("#[cfg(test)]")
        .next()
        .unwrap_or_default()
        .to_string()
}

fn assert_runtime_boundary(path: &Path) {
    let source = load_runtime_source(&path);
    for forbidden in [
        "sqlx::query",
        ".pool()",
        ".scoped(",
        ".scoped_default()",
        "google_cloud_spanner",
        "zitadel_db::Db",
    ] {
        assert!(
            !source.contains(forbidden),
            "{} still contains forbidden runtime SQL boundary marker `{forbidden}`",
            path.display()
        );
    }
}

#[test]
fn oidc_runtime_adapter_stays_behind_repository_boundary() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_runtime_boundary(&manifest.join("src/adapters.rs"));
}

#[test]
fn oidc_runtime_store_stays_behind_repository_boundary() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_runtime_boundary(&manifest.join("src/stores.rs"));
}
