use std::{fs, path::Path};

fn load_runtime_source(path: &Path) -> String {
    let source = fs::read_to_string(path).expect("read source");
    source
        .split("#[cfg(test)]")
        .next()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn server_routing_stays_behind_repository_boundary() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("src/routing.rs");
    let source = load_runtime_source(&path);
    for forbidden in ["sqlx::query", ".pool()", ".scoped(", ".scoped_default()"] {
        assert!(
            !source.contains(forbidden),
            "{} still contains forbidden runtime SQL boundary marker `{forbidden}`",
            path.display()
        );
    }
}

#[test]
fn server_runtime_wiring_does_not_import_repo_impls_directly() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for rel in ["src/wiring.rs", "src/jobs.rs"] {
        let path = manifest.join(rel);
        let source = load_runtime_source(&path);
        for forbidden in ["use zitadel_db::repo_impls", "repo_impls::"] {
            assert!(
                !source.contains(forbidden),
                "{} still contains forbidden repository implementation import `{forbidden}`",
                path.display()
            );
        }
    }
}
