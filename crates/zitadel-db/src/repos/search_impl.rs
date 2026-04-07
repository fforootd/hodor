use google_cloud_spanner::{row::Row as SpannerRow, statement::Statement};
use std::collections::HashSet;

use crate::{Db, SpannerDb, list_schema_registry, provider, spanner_ident};
use zitadel_app::repo::{BoxFuture, SearchRepository, SearchResult};

#[derive(Clone)]
pub struct SqlSearchRepository {
    db: Db,
    replica: Option<Db>,
}

impl SqlSearchRepository {
    pub fn new(db: Db) -> Self {
        Self { db, replica: None }
    }

    pub fn with_replica(db: Db, replica: Db) -> Self {
        Self {
            db,
            replica: Some(replica),
        }
    }
}

impl SearchRepository for SqlSearchRepository {
    fn search(
        &self,
        instance_id: &str,
        query: &str,
        resource_types: Option<&[&str]>,
        limit: Option<u32>,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SearchResult>>> {
        let db = self.db.clone();
        let replica = self.replica.clone();
        let instance_id = instance_id.to_string();
        let query = query.to_string();
        let allowed = normalized_resource_types(resource_types);
        let limit = limit.map(i64::from).unwrap_or(20).clamp(1, 100);
        Box::pin(async move {
            if let Some(replica_db) = replica {
                match run_search(replica_db, &instance_id, &query, &allowed, limit).await {
                    Ok(results) => return Ok(results),
                    Err(error) => {
                        tracing::warn!(
                            %error,
                            instance_id,
                            query,
                            "replica-backed search failed, falling back to primary"
                        );
                    }
                }
            }

            run_search(db, &instance_id, &query, &allowed, limit).await
        })
    }
}

async fn run_search(
    db: Db,
    instance_id: &str,
    query: &str,
    allowed: &HashSet<String>,
    limit: i64,
) -> anyhow::Result<Vec<SearchResult>> {
    let pattern = format!("%{query}%");
    let query_lc = query.to_lowercase();
    let mut results = Vec::new();

    if allowed.is_empty() || allowed.contains("user") {
        match &db {
            Db::Sql(_) => {
                let scoped = db.scoped(instance_id.to_string());
                let rows: Vec<(String, String, String)> = sqlx::query_as(
                            "SELECT id, identifier, display_name FROM users \
                             WHERE instance_id = $1 AND (identifier LIKE $2 OR display_name LIKE $2) \
                             ORDER BY display_name, id LIMIT $3",
                        )
                        .bind(&instance_id)
                        .bind(&pattern)
                        .bind(limit)
                        .fetch_all(scoped.pool())
                        .await?;
                results.extend(rows.into_iter().map(|row| SearchResult {
                    resource_type: "user".to_string(),
                    id: row.0,
                    title: row.2,
                    subtitle: Some(row.1),
                }));
            }
            Db::Spanner(spanner) => {
                let mut stmt = Statement::new(
                    "SELECT id, identifier, display_name FROM users \
                             WHERE instance_id = @instance_id AND (identifier LIKE @pattern OR display_name LIKE @pattern) \
                             ORDER BY display_name, id LIMIT @limit",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("pattern", &pattern);
                stmt.add_param("limit", &limit);
                for row in spanner_query_all(spanner, stmt).await? {
                    results.push(SearchResult {
                        resource_type: "user".to_string(),
                        id: row.column_by_name::<String>("id").unwrap_or_default(),
                        title: row
                            .column_by_name::<String>("display_name")
                            .unwrap_or_default(),
                        subtitle: Some(
                            row.column_by_name::<String>("identifier")
                                .unwrap_or_default(),
                        ),
                    });
                }
            }
        }
    }

    if allowed.is_empty() || allowed.contains("org") {
        match &db {
            Db::Sql(_) => {
                let scoped = db.scoped(instance_id.to_string());
                let rows: Vec<(String, String)> = sqlx::query_as(
                            "SELECT id, name FROM orgs WHERE instance_id = $1 AND name LIKE $2 ORDER BY name, id LIMIT $3",
                        )
                        .bind(&instance_id)
                        .bind(&pattern)
                        .bind(limit)
                        .fetch_all(scoped.pool())
                        .await?;
                results.extend(rows.into_iter().map(|row| SearchResult {
                    resource_type: "org".to_string(),
                    id: row.0.clone(),
                    title: row.1,
                    subtitle: Some(format!("Organization {}", row.0)),
                }));
            }
            Db::Spanner(spanner) => {
                let mut stmt = Statement::new(
                    "SELECT id, name FROM orgs WHERE instance_id = @instance_id AND name LIKE @pattern \
                             ORDER BY name, id LIMIT @limit",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("pattern", &pattern);
                stmt.add_param("limit", &limit);
                for row in spanner_query_all(spanner, stmt).await? {
                    let id = row.column_by_name::<String>("id").unwrap_or_default();
                    results.push(SearchResult {
                        resource_type: "org".to_string(),
                        id: id.clone(),
                        title: row.column_by_name::<String>("name").unwrap_or_default(),
                        subtitle: Some(format!("Organization {id}")),
                    });
                }
            }
        }
    }

    if allowed.is_empty() || allowed.contains("group") {
        match &db {
            Db::Sql(_) => {
                let scoped = db.scoped(instance_id.to_string());
                let rows: Vec<(String, String, String)> = sqlx::query_as(
                    "SELECT id, COALESCE(org_id, ''), name FROM groups \
                             WHERE instance_id = $1 AND name LIKE $2 ORDER BY name, id LIMIT $3",
                )
                .bind(&instance_id)
                .bind(&pattern)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?;
                results.extend(rows.into_iter().map(|row| SearchResult {
                    resource_type: "group".to_string(),
                    id: row.0,
                    title: row.2,
                    subtitle: Some(format!("Org {}", row.1)),
                }));
            }
            Db::Spanner(spanner) => {
                let groups = spanner_ident("groups");
                let mut stmt = Statement::new(format!(
                    "SELECT id, IFNULL(org_id, '') AS org_id, name FROM {groups} \
                             WHERE instance_id = @instance_id AND name LIKE @pattern ORDER BY name, id LIMIT @limit"
                ));
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("pattern", &pattern);
                stmt.add_param("limit", &limit);
                for row in spanner_query_all(spanner, stmt).await? {
                    results.push(SearchResult {
                        resource_type: "group".to_string(),
                        id: row.column_by_name::<String>("id").unwrap_or_default(),
                        title: row.column_by_name::<String>("name").unwrap_or_default(),
                        subtitle: Some(format!(
                            "Org {}",
                            row.column_by_name::<String>("org_id").unwrap_or_default()
                        )),
                    });
                }
            }
        }
    }

    if allowed.is_empty() || allowed.contains("provider") {
        for provider in provider::list_providers_for(&db, instance_id).await? {
            if provider
                .payload
                .display_name
                .to_lowercase()
                .contains(&query_lc)
            {
                results.push(SearchResult {
                    resource_type: "provider".to_string(),
                    id: provider.id,
                    title: provider.payload.display_name,
                    subtitle: Some(provider.payload.protocol),
                });
            }
        }
    }

    if allowed.is_empty() || allowed.contains("schema") {
        for schema in list_schema_registry(&db, "", None, limit).await? {
            if schema.type_.to_lowercase().contains(&query_lc)
                || schema.id.to_lowercase().contains(&query_lc)
            {
                results.push(SearchResult {
                    resource_type: "schema".to_string(),
                    id: schema.id,
                    title: schema.type_,
                    subtitle: Some(format!("v{} {}", schema.version, schema.visibility)),
                });
            }
        }
    }

    results.truncate(limit as usize);
    Ok(results)
}

fn normalized_resource_types(resource_types: Option<&[&str]>) -> HashSet<String> {
    resource_types
        .into_iter()
        .flatten()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

async fn spanner_query_all(
    spanner: &SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Vec<SpannerRow>> {
    let mut tx = spanner.client().single().await?;
    let mut rows = tx.query(stmt).await?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().await? {
        result.push(row);
    }
    Ok(result)
}
