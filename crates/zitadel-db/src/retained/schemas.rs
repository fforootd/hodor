use google_cloud_spanner::{
    client::Error as SpannerError, statement::Statement,
};

use crate::Db;
use super::{
    SchemaRegistryRecord,
    spanner_query_all,
};

pub async fn list_schema_registry(
    db: &Db,
    after_id: &str,
    type_filter: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<SchemaRegistryRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped_default();
            let is_default = scoped.bool_as_int("is_default");
            let created_at = scoped.as_text("created_at");
            let (sql, bind_type) = if type_filter.is_some() {
                (
                    format!(
                        "SELECT id, type, {}, version, {is_default}, visibility, {created_at} \
                         FROM schemas WHERE id > $1 AND type = $2 ORDER BY type, version DESC LIMIT $3",
                        scoped.as_text("schema"),
                    ),
                    type_filter,
                )
            } else {
                (
                    format!(
                        "SELECT id, type, {}, version, {is_default}, visibility, {created_at} \
                         FROM schemas WHERE id > $1 ORDER BY type, version DESC LIMIT $2",
                        scoped.as_text("schema"),
                    ),
                    None,
                )
            };
            let rows = if let Some(type_filter) = bind_type {
                sqlx::query_as::<_, (String, String, String, i64, i64, String, String)>(&sql)
                    .bind(after_id)
                    .bind(type_filter)
                    .bind(limit)
                    .fetch_all(scoped.pool())
                    .await?
            } else {
                sqlx::query_as::<_, (String, String, String, i64, i64, String, String)>(&sql)
                    .bind(after_id)
                    .bind(limit)
                    .fetch_all(scoped.pool())
                    .await?
            };
            Ok(rows
                .into_iter()
                .map(|row| SchemaRegistryRecord {
                    id: row.0,
                    type_: row.1,
                    schema_json: row.2,
                    version: row.3,
                    is_default: row.4 != 0,
                    visibility: row.5,
                    created_at: row.6,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let sql = if type_filter.is_some() {
                "SELECT id, type, schema, version, is_default, visibility, CAST(created_at AS STRING) AS created_at \
                 FROM schemas WHERE id > @after_id AND type = @type ORDER BY type, version DESC LIMIT @limit"
            } else {
                "SELECT id, type, schema, version, is_default, visibility, CAST(created_at AS STRING) AS created_at \
                 FROM schemas WHERE id > @after_id ORDER BY type, version DESC LIMIT @limit"
            };
            let mut stmt = Statement::new(sql);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            if let Some(type_filter) = type_filter {
                stmt.add_param("type", &type_filter);
            }
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| SchemaRegistryRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    type_: row.column_by_name::<String>("type").unwrap_or_default(),
                    schema_json: row.column_by_name::<String>("schema").unwrap_or_default(),
                    version: row.column_by_name::<i64>("version").unwrap_or(1),
                    is_default: row.column_by_name::<bool>("is_default").unwrap_or(false),
                    visibility: row
                        .column_by_name::<String>("visibility")
                        .unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn get_schema_record(db: &Db, id: &str) -> anyhow::Result<Option<SchemaRegistryRecord>> {
    let items = list_schema_registry(db, "", None, i64::MAX).await?;
    Ok(items.into_iter().find(|item| item.id == id))
}

pub async fn create_schema_record(
    db: &Db,
    id: &str,
    type_: &str,
    schema_json: &str,
    visibility: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped_default();
            let sql = format!(
                "INSERT INTO schemas (id, type, schema, visibility) VALUES ($1, $2, {}, $3)",
                scoped.json_bind(4),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(type_)
                .bind(visibility)
                .bind(schema_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO schemas (id, type, schema, visibility) VALUES (@id, @type, @schema, @visibility)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("type", &type_);
            stmt.add_param("schema", &schema_json);
            stmt.add_param("visibility", &visibility);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move {
                        tx.update(stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
        }
    }
    Ok(())
}

pub async fn update_schema_record(db: &Db, id: &str, schema_json: &str) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped_default();
            let sql = format!(
                "UPDATE schemas SET schema = {}, version = version + 1 WHERE id = $1",
                scoped.json_bind(2),
            );
            Ok(sqlx::query(&sql)
                .bind(id)
                .bind(schema_json)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE schemas SET schema = @schema, version = version + 1 WHERE id = @id",
            );
            stmt.add_param("schema", &schema_json);
            stmt.add_param("id", &id);
            let (_, affected) = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
            Ok(affected > 0)
        }
    }
}

pub async fn promote_schema_record(db: &Db, id: &str) -> anyhow::Result<bool> {
    let Some(record) = get_schema_record(db, id).await? else {
        return Ok(false);
    };
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped_default();
            sqlx::query("UPDATE schemas SET is_default = FALSE WHERE type = $1")
                .bind(&record.type_)
                .execute(scoped.pool())
                .await?;
            sqlx::query(
                "UPDATE schemas SET is_default = TRUE, visibility = 'public' WHERE id = $1",
            )
            .bind(id)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mut reset_stmt =
                Statement::new("UPDATE schemas SET is_default = FALSE WHERE type = @type");
            reset_stmt.add_param("type", &record.type_);
            let mut promote_stmt = Statement::new(
                "UPDATE schemas SET is_default = TRUE, visibility = 'public' WHERE id = @id",
            );
            promote_stmt.add_param("id", &id);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let reset_stmt = reset_stmt.clone();
                    let promote_stmt = promote_stmt.clone();
                    Box::pin(async move {
                        tx.update(reset_stmt).await?;
                        tx.update(promote_stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
        }
    }
    Ok(true)
}
