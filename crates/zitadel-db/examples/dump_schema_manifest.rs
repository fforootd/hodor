use zitadel_db::{Db, migrate, schema::inspect_schema};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Db::open("").await?;
    migrate::migrate(&db).await?;
    let manifest = inspect_schema(&db).await?;
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}
