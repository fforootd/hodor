use zitadel_db::{BackendKind, schema::render_baseline_migration};

fn parse_backend(raw: &str) -> anyhow::Result<BackendKind> {
    match raw {
        "sqlite" => Ok(BackendKind::Sqlite),
        "postgres" => Ok(BackendKind::Postgres),
        "spanner" => Ok(BackendKind::Spanner),
        _ => anyhow::bail!("unknown backend '{raw}', expected one of: sqlite, postgres, spanner"),
    }
}

fn main() -> anyhow::Result<()> {
    let backend = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("expected backend argument: sqlite | postgres | spanner"))
        .and_then(|raw| parse_backend(&raw))?;

    print!("{}", render_baseline_migration(backend));
    Ok(())
}
