use anyhow::Context;
use sqlx::{Executor, postgres::PgPoolOptions};
use uuid::Uuid;
use zitadel_db::{
    Db, migrate,
    schema::{SchemaManifest, canonical_manifest, inspect_schema},
    test_support::spanner_db_from_env,
};

#[tokio::test]
async fn migrated_stateful_schema_stays_logically_aligned_across_backends() -> anyhow::Result<()> {
    let canonical = canonical_manifest();

    let sqlite = Db::open("").await?;
    migrate::migrate(&sqlite).await?;
    let sqlite_manifest = inspect_schema(&sqlite).await?;
    assert_same_schema("canonical", canonical, "sqlite", &sqlite_manifest)?;

    let Some(postgres) = postgres_db_from_env().await? else {
        return Ok(());
    };
    migrate::migrate(&postgres).await?;
    let postgres_manifest = inspect_schema(&postgres).await?;
    assert_same_schema("canonical", canonical, "postgres", &postgres_manifest)?;

    let Some(spanner) = spanner_db_from_env("db-schema-parity").await? else {
        return Ok(());
    };
    migrate::migrate(&spanner).await?;
    let spanner_manifest = inspect_schema(&spanner).await?;
    assert_same_schema("canonical", canonical, "spanner", &spanner_manifest)?;

    Ok(())
}

fn assert_same_schema(
    left_name: &str,
    left: &SchemaManifest,
    right_name: &str,
    right: &SchemaManifest,
) -> anyhow::Result<()> {
    if left == right {
        return Ok(());
    }

    let left_json = serde_json::to_string_pretty(left)?;
    let right_json = serde_json::to_string_pretty(right)?;
    anyhow::bail!(
        "{left_name} and {right_name} migrated schemas diverged\nleft:\n{left_json}\n\nright:\n{right_json}"
    );
}

async fn postgres_db_from_env() -> anyhow::Result<Option<Db>> {
    let Some(base_url) = std::env::var("ZITADEL_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping Postgres schema parity test: ZITADEL_TEST_POSTGRES_URL is not set");
        return Ok(None);
    };

    let admin_url = replace_database_name(&base_url, "postgres");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .with_context(|| format!("connect to Postgres admin database {admin_url}"))?;

    let db_name = format!("schema_parity_{}", Uuid::new_v4().simple());
    admin
        .execute(sqlx::query(&format!("CREATE DATABASE \"{db_name}\"")))
        .await
        .with_context(|| format!("create Postgres test database {db_name}"))?;

    let url = replace_database_name(&base_url, &db_name);
    Db::open(&url)
        .await
        .with_context(|| format!("open Postgres test database {url}"))
        .map(Some)
}

fn replace_database_name(url: &str, db_name: &str) -> String {
    let (without_query, query) = match url.split_once('?') {
        Some((without_query, query)) => (without_query, Some(query)),
        None => (url, None),
    };
    let slash = without_query
        .rfind('/')
        .expect("postgres URL should include a database name");
    let mut rewritten = format!("{}/{}", &without_query[..slash], db_name);
    if let Some(query) = query {
        rewritten.push('?');
        rewritten.push_str(query);
    }
    rewritten
}
