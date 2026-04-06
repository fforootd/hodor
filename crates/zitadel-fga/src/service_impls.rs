use std::collections::{BTreeSet, HashSet};

use anyhow::Context;
use async_trait::async_trait;
use google_cloud_spanner::{
    client::Error as SpannerError, row::Row as SpannerRow, statement::Statement,
};
use sqlx::Row;
use zitadel_db::Db;

use crate::dto::*;
use crate::error::FgaError;
use crate::evaluation::*;
use crate::service::FgaService;
use crate::traits::*;

#[async_trait]
impl StoreResolver for FgaService {
    async fn initialize_instance(&self, instance_id: &str) -> Result<StoreInfo, FgaError> {
        self.invalidate_store_cache(instance_id).await;
        let store = self.ensure_store_row(instance_id).await?;
        self.invalidate_model_caches(instance_id, &store.id).await;
        self.ensure_default_model(instance_id, &store.id).await?;
        Ok(store)
    }

    async fn discover_store(&self, instance_id: &str) -> Result<StoreInfo, FgaError> {
        self.ensure_store_row(instance_id).await
    }
}

#[async_trait]
impl ModelRepository for FgaService {
    async fn read_model(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> Result<AuthorizationModelMetadata, FgaError> {
        let model = self.load_model_row(instance_id, store_id, model_id).await?;
        let request: AuthorizationModelWriteRequest =
            serde_json::from_str(&model.raw).context("parse authorization model row")?;
        Ok(AuthorizationModelMetadata {
            authorization_model_id: model.model_id,
            schema_version: request.schema_version,
            type_definitions: request.type_definitions,
            conditions: request.conditions,
            created_at: model.created_at,
        })
    }

    async fn read_models(
        &self,
        instance_id: &str,
        store_id: &str,
    ) -> Result<AuthorizationModelsListResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        let mut models = Vec::new();
        match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let rows = sqlx::query(
                    "SELECT model_id, compiled_model, CAST(created_at AS TEXT) AS created_at FROM fga_authorization_models WHERE scope_id = $1 AND store_id = $2 ORDER BY created_at DESC",
                )
                .bind(instance_id)
                .bind(store_id)
                .fetch_all(scoped.pool())
                .await
                .context("list authorization models")?;

                models.reserve(rows.len());
                for row in rows {
                    let raw = row.get::<String, _>("compiled_model");
                    let request: AuthorizationModelWriteRequest =
                        serde_json::from_str(&raw).context("parse authorization model in list")?;
                    models.push(AuthorizationModelMetadata {
                        authorization_model_id: row.get::<String, _>("model_id"),
                        schema_version: request.schema_version,
                        type_definitions: request.type_definitions,
                        conditions: request.conditions,
                        created_at: row.get::<String, _>("created_at"),
                    });
                }
            }
            Db::Spanner(_) => {
                let mut stmt = Statement::new(
                    "SELECT model_id, compiled_model, CAST(created_at AS STRING) AS created_at \
                     FROM fga_authorization_models \
                     WHERE scope_id = @scope_id AND store_id = @store_id \
                     ORDER BY created_at DESC",
                );
                stmt.add_param("scope_id", &instance_id);
                stmt.add_param("store_id", &store_id);
                let rows: Vec<SpannerRow> = self
                    .spanner_query_all(stmt, "list authorization models")
                    .await?;

                models.reserve(rows.len());
                for row in rows {
                    let raw = row
                        .column_by_name::<String>("compiled_model")
                        .context("read spanner compiled_model")?;
                    let request: AuthorizationModelWriteRequest =
                        serde_json::from_str(&raw).context("parse authorization model in list")?;
                    models.push(AuthorizationModelMetadata {
                        authorization_model_id: row
                            .column_by_name::<String>("model_id")
                            .context("read spanner model_id")?,
                        schema_version: request.schema_version,
                        type_definitions: request.type_definitions,
                        conditions: request.conditions,
                        created_at: row
                            .column_by_name::<String>("created_at")
                            .context("read spanner created_at")?,
                    });
                }
            }
        }

        Ok(AuthorizationModelsListResponse {
            authorization_models: models,
        })
    }

    async fn write_model(
        &self,
        instance_id: &str,
        store_id: &str,
        request: AuthorizationModelWriteRequest,
    ) -> Result<AuthorizationModelWriteResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        let model_id = self.persist_model(instance_id, store_id, request).await?;

        Ok(AuthorizationModelWriteResponse {
            authorization_model_id: model_id,
        })
    }
}

#[async_trait]
impl TupleRepository for FgaService {
    async fn read_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ReadRequest,
    ) -> Result<ReadResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        let page_size = request.page_size.unwrap_or(100).clamp(1, 500) as i64;
        let offset = decode_offset(request.continuation_token.as_deref())?;
        let filter = request.tuple_key.unwrap_or(TupleFilter {
            user: None,
            relation: None,
            object: None,
        });
        let tuples = match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let rows = sqlx::query(
                    "SELECT raw_user, relation, raw_object, CAST(inserted_at AS TEXT) AS inserted_at
                     FROM fga_tuples
                     WHERE scope_id = $1
                       AND store_id = $2
                       AND ($3 = '' OR raw_user = $3)
                       AND ($4 = '' OR relation = $4)
                       AND ($5 = '' OR raw_object = $5)
                     ORDER BY raw_object, relation, raw_user
                     LIMIT $6 OFFSET $7",
                )
                .bind(instance_id)
                .bind(store_id)
                .bind(filter.user.clone().unwrap_or_default())
                .bind(filter.relation.clone().unwrap_or_default())
                .bind(filter.object.clone().unwrap_or_default())
                .bind(page_size)
                .bind(offset)
                .fetch_all(scoped.pool())
                .await
                .context("read tuples")?;

                rows.iter()
                    .map(|row| TupleRecord {
                        key: TupleKey {
                            user: row.get::<String, _>("raw_user"),
                            relation: row.get::<String, _>("relation"),
                            object: row.get::<String, _>("raw_object"),
                            condition: None,
                        },
                        timestamp: Some(row.get::<String, _>("inserted_at")),
                    })
                    .collect()
            }
            Db::Spanner(_) => {
                let mut stmt = Statement::new(&format!(
                    "SELECT raw_user, relation, raw_object, CAST(inserted_at AS STRING) AS inserted_at \
                     FROM fga_tuples \
                     WHERE scope_id = @scope_id \
                       AND store_id = @store_id \
                       AND (@raw_user = '' OR raw_user = @raw_user) \
                       AND (@relation = '' OR relation = @relation) \
                       AND (@raw_object = '' OR raw_object = @raw_object) \
                     ORDER BY raw_object, relation, raw_user \
                     LIMIT {} OFFSET {}",
                    page_size, offset
                ));
                stmt.add_param("scope_id", &instance_id);
                stmt.add_param("store_id", &store_id);
                stmt.add_param("raw_user", &filter.user.unwrap_or_default());
                stmt.add_param("relation", &filter.relation.unwrap_or_default());
                stmt.add_param("raw_object", &filter.object.unwrap_or_default());
                let rows: Vec<SpannerRow> = self.spanner_query_all(stmt, "read tuples").await?;
                rows.into_iter()
                    .map(|row| -> Result<TupleRecord, FgaError> {
                        Ok(TupleRecord {
                            key: TupleKey {
                                user: row
                                    .column_by_name::<String>("raw_user")
                                    .context("read spanner raw_user")?,
                                relation: row
                                    .column_by_name::<String>("relation")
                                    .context("read spanner relation")?,
                                object: row
                                    .column_by_name::<String>("raw_object")
                                    .context("read spanner raw_object")?,
                                condition: None,
                            },
                            timestamp: Some(
                                row.column_by_name::<String>("inserted_at")
                                    .context("read spanner inserted_at")?,
                            ),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
        };
        let next = if tuples.len() as i64 == page_size {
            Some((offset + page_size).to_string())
        } else {
            None
        };

        Ok(ReadResponse {
            tuples,
            continuation_token: next,
        })
    }

    async fn write_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        request: WriteRequest,
    ) -> Result<(), FgaError> {
        self.require_store(instance_id, store_id).await?;
        let model_id = self
            .validate_model_id(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let (_, model) = self
            .load_compiled_model(instance_id, store_id, Some(&model_id))
            .await?;
        validate_duplicate_request_tuples(&request)?;
        let writes = request
            .writes
            .tuple_keys
            .into_iter()
            .map(ParsedTupleKey::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let deletes = request
            .deletes
            .tuple_keys
            .into_iter()
            .map(ParsedTupleKey::parse)
            .collect::<Result<Vec<_>, _>>()?;

        for parsed in &writes {
            if parsed.condition.is_some() {
                return Err(FgaError::Unsupported(
                    "conditional tuples are not supported by the embedded v1 server".into(),
                ));
            }
            model.validate_tuple(parsed)?;
        }

        match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let mut tx = scoped
                    .pool()
                    .begin()
                    .await
                    .context("begin tuple transaction")?;

                for parsed in &writes {
                    let insert = match scoped.dialect() {
                        zitadel_db::Dialect::Sqlite => {
                            "INSERT OR IGNORE INTO fga_tuples (scope_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
                        }
                        zitadel_db::Dialect::Postgres => {
                            "INSERT INTO fga_tuples (scope_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT DO NOTHING"
                        }
                        zitadel_db::Dialect::Spanner => {
                            unreachable!("native Spanner does not use ScopedDb")
                        }
                    };
                    let result = sqlx::query(insert)
                        .bind(instance_id)
                        .bind(store_id)
                        .bind(&parsed.object.object_type)
                        .bind(&parsed.object.object_id)
                        .bind(&parsed.relation)
                        .bind(parsed.user.user_type())
                        .bind(parsed.user.user_id())
                        .bind(parsed.user.relation_name().unwrap_or_default())
                        .bind(parsed.object.as_raw())
                        .bind(parsed.user.as_raw())
                        .execute(&mut *tx)
                        .await
                        .context("insert fga tuple")?;
                    if result.rows_affected() > 0 {
                        sqlx::query(
                            "INSERT INTO fga_tuple_changes (scope_id, store_id, operation, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user, authorization_model_id) VALUES ($1, $2, 'WRITE', $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                        )
                        .bind(instance_id)
                        .bind(store_id)
                        .bind(&parsed.object.object_type)
                        .bind(&parsed.object.object_id)
                        .bind(&parsed.relation)
                        .bind(parsed.user.user_type())
                        .bind(parsed.user.user_id())
                        .bind(parsed.user.relation_name().unwrap_or_default())
                        .bind(parsed.object.as_raw())
                        .bind(parsed.user.as_raw())
                        .bind(&model_id)
                        .execute(&mut *tx)
                        .await
                        .context("insert tuple change")?;
                    }
                }

                for parsed in &deletes {
                    let result = sqlx::query(
                        "DELETE FROM fga_tuples WHERE scope_id = $1 AND store_id = $2 AND object_type = $3 AND object_id = $4 AND relation = $5 AND user_type = $6 AND user_id = $7 AND user_relation = $8",
                    )
                    .bind(instance_id)
                    .bind(store_id)
                    .bind(&parsed.object.object_type)
                    .bind(&parsed.object.object_id)
                    .bind(&parsed.relation)
                    .bind(parsed.user.user_type())
                    .bind(parsed.user.user_id())
                    .bind(parsed.user.relation_name().unwrap_or_default())
                    .execute(&mut *tx)
                    .await
                    .context("delete fga tuple")?;
                    if result.rows_affected() > 0 {
                        sqlx::query(
                            "INSERT INTO fga_tuple_changes (scope_id, store_id, operation, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user, authorization_model_id) VALUES ($1, $2, 'DELETE', $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                        )
                        .bind(instance_id)
                        .bind(store_id)
                        .bind(&parsed.object.object_type)
                        .bind(&parsed.object.object_id)
                        .bind(&parsed.relation)
                        .bind(parsed.user.user_type())
                        .bind(parsed.user.user_id())
                        .bind(parsed.user.relation_name().unwrap_or_default())
                        .bind(parsed.object.as_raw())
                        .bind(parsed.user.as_raw())
                        .bind(&model_id)
                        .execute(&mut *tx)
                        .await
                        .context("insert tuple delete change")?;
                    }
                }

                tx.commit().await.context("commit tuple transaction")?;
            }
            Db::Spanner(spanner) => {
                let scope_id = instance_id.to_string();
                let store_id = store_id.to_string();
                let model_id = model_id.clone();
                let writes = writes.clone();
                let deletes = deletes.clone();
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let scope_id = scope_id.clone();
                        let store_id = store_id.clone();
                        let model_id = model_id.clone();
                        let writes = writes.clone();
                        let deletes = deletes.clone();
                        Box::pin(async move {
                            for parsed in writes {
                                let mut exists = Statement::new(
                                    "SELECT raw_user FROM fga_tuples \
                                     WHERE scope_id = @scope_id AND store_id = @store_id \
                                       AND object_type = @object_type AND object_id = @object_id \
                                       AND relation = @relation AND user_type = @user_type \
                                       AND user_id = @user_id AND user_relation = @user_relation \
                                     LIMIT 1",
                                );
                                exists.add_param("scope_id", &scope_id);
                                exists.add_param("store_id", &store_id);
                                exists.add_param("object_type", &parsed.object.object_type);
                                exists.add_param("object_id", &parsed.object.object_id);
                                exists.add_param("relation", &parsed.relation);
                                exists.add_param("user_type", &parsed.user.user_type());
                                exists.add_param("user_id", &parsed.user.user_id());
                                exists.add_param(
                                    "user_relation",
                                    &parsed.user.relation_name().unwrap_or_default(),
                                );
                                let mut rows = tx.query(exists).await?;
                                if rows.next().await?.is_some() {
                                    continue;
                                }

                                let raw_object = parsed.object.as_raw();
                                let raw_user = parsed.user.as_raw();
                                let mut insert = Statement::new(
                                    "INSERT INTO fga_tuples \
                                     (scope_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user) \
                                     VALUES \
                                     (@scope_id, @store_id, @object_type, @object_id, @relation, @user_type, @user_id, @user_relation, @raw_object, @raw_user)",
                                );
                                insert.add_param("scope_id", &scope_id);
                                insert.add_param("store_id", &store_id);
                                insert.add_param("object_type", &parsed.object.object_type);
                                insert.add_param("object_id", &parsed.object.object_id);
                                insert.add_param("relation", &parsed.relation);
                                insert.add_param("user_type", &parsed.user.user_type());
                                insert.add_param("user_id", &parsed.user.user_id());
                                insert.add_param(
                                    "user_relation",
                                    &parsed.user.relation_name().unwrap_or_default(),
                                );
                                insert.add_param("raw_object", &raw_object);
                                insert.add_param("raw_user", &raw_user);
                                tx.update(insert).await?;

                                let mut change = Statement::new(
                                    "INSERT INTO fga_tuple_changes \
                                     (scope_id, store_id, operation, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user, authorization_model_id) \
                                     VALUES \
                                     (@scope_id, @store_id, 'WRITE', @object_type, @object_id, @relation, @user_type, @user_id, @user_relation, @raw_object, @raw_user, @authorization_model_id)",
                                );
                                change.add_param("scope_id", &scope_id);
                                change.add_param("store_id", &store_id);
                                change.add_param("object_type", &parsed.object.object_type);
                                change.add_param("object_id", &parsed.object.object_id);
                                change.add_param("relation", &parsed.relation);
                                change.add_param("user_type", &parsed.user.user_type());
                                change.add_param("user_id", &parsed.user.user_id());
                                change.add_param(
                                    "user_relation",
                                    &parsed.user.relation_name().unwrap_or_default(),
                                );
                                change.add_param("raw_object", &raw_object);
                                change.add_param("raw_user", &raw_user);
                                change.add_param("authorization_model_id", &model_id);
                                tx.update(change).await?;
                            }

                            for parsed in deletes {
                                let mut exists = Statement::new(
                                    "SELECT raw_user FROM fga_tuples \
                                     WHERE scope_id = @scope_id AND store_id = @store_id \
                                       AND object_type = @object_type AND object_id = @object_id \
                                       AND relation = @relation AND user_type = @user_type \
                                       AND user_id = @user_id AND user_relation = @user_relation \
                                     LIMIT 1",
                                );
                                exists.add_param("scope_id", &scope_id);
                                exists.add_param("store_id", &store_id);
                                exists.add_param("object_type", &parsed.object.object_type);
                                exists.add_param("object_id", &parsed.object.object_id);
                                exists.add_param("relation", &parsed.relation);
                                exists.add_param("user_type", &parsed.user.user_type());
                                exists.add_param("user_id", &parsed.user.user_id());
                                exists.add_param(
                                    "user_relation",
                                    &parsed.user.relation_name().unwrap_or_default(),
                                );
                                let mut rows = tx.query(exists).await?;
                                if rows.next().await?.is_none() {
                                    continue;
                                }

                                let raw_object = parsed.object.as_raw();
                                let raw_user = parsed.user.as_raw();
                                let mut delete = Statement::new(
                                    "DELETE FROM fga_tuples \
                                     WHERE scope_id = @scope_id AND store_id = @store_id \
                                       AND object_type = @object_type AND object_id = @object_id \
                                       AND relation = @relation AND user_type = @user_type \
                                       AND user_id = @user_id AND user_relation = @user_relation",
                                );
                                delete.add_param("scope_id", &scope_id);
                                delete.add_param("store_id", &store_id);
                                delete.add_param("object_type", &parsed.object.object_type);
                                delete.add_param("object_id", &parsed.object.object_id);
                                delete.add_param("relation", &parsed.relation);
                                delete.add_param("user_type", &parsed.user.user_type());
                                delete.add_param("user_id", &parsed.user.user_id());
                                delete.add_param(
                                    "user_relation",
                                    &parsed.user.relation_name().unwrap_or_default(),
                                );
                                tx.update(delete).await?;

                                let mut change = Statement::new(
                                    "INSERT INTO fga_tuple_changes \
                                     (scope_id, store_id, operation, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user, authorization_model_id) \
                                     VALUES \
                                     (@scope_id, @store_id, 'DELETE', @object_type, @object_id, @relation, @user_type, @user_id, @user_relation, @raw_object, @raw_user, @authorization_model_id)",
                                );
                                change.add_param("scope_id", &scope_id);
                                change.add_param("store_id", &store_id);
                                change.add_param("object_type", &parsed.object.object_type);
                                change.add_param("object_id", &parsed.object.object_id);
                                change.add_param("relation", &parsed.relation);
                                change.add_param("user_type", &parsed.user.user_type());
                                change.add_param("user_id", &parsed.user.user_id());
                                change.add_param(
                                    "user_relation",
                                    &parsed.user.relation_name().unwrap_or_default(),
                                );
                                change.add_param("raw_object", &raw_object);
                                change.add_param("raw_user", &raw_user);
                                change.add_param("authorization_model_id", &model_id);
                                tx.update(change).await?;
                            }

                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await
                    .context("commit spanner tuple transaction")?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ChangeRepository for FgaService {
    async fn read_changes(
        &self,
        instance_id: &str,
        store_id: &str,
        object_type: Option<&str>,
        page_size: u32,
        continuation_token: Option<&str>,
    ) -> Result<ReadChangesResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        let after_seq = decode_offset(continuation_token)?;
        let limit = page_size.clamp(1, 500) as i64;
        let mut next = None;
        let mut changes = Vec::new();
        match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let rows = sqlx::query(
                    "SELECT seq, operation, raw_user, relation, raw_object, CAST(created_at AS TEXT) AS created_at
                     FROM fga_tuple_changes
                     WHERE scope_id = $1
                       AND store_id = $2
                       AND seq > $3
                       AND ($4 = '' OR object_type = $4)
                     ORDER BY seq ASC
                     LIMIT $5",
                )
                .bind(instance_id)
                .bind(store_id)
                .bind(after_seq)
                .bind(object_type.unwrap_or_default())
                .bind(limit)
                .fetch_all(scoped.pool())
                .await
                .context("read fga changes")?;

                changes.reserve(rows.len());
                for row in rows {
                    let seq: i64 = row.get("seq");
                    next = Some(seq.to_string());
                    changes.push(TupleChangeRecord {
                        tuple_key: TupleKey {
                            user: row.get("raw_user"),
                            relation: row.get("relation"),
                            object: row.get("raw_object"),
                            condition: None,
                        },
                        operation: row.get("operation"),
                        timestamp: row.get("created_at"),
                    });
                }
            }
            Db::Spanner(_) => {
                let mut stmt = Statement::new(&format!(
                    "SELECT seq, operation, raw_user, relation, raw_object, CAST(created_at AS STRING) AS created_at \
                     FROM fga_tuple_changes \
                     WHERE scope_id = @scope_id \
                       AND store_id = @store_id \
                       AND seq > @after_seq \
                       AND (@object_type = '' OR object_type = @object_type) \
                     ORDER BY seq ASC \
                     LIMIT {}",
                    limit
                ));
                stmt.add_param("scope_id", &instance_id);
                stmt.add_param("store_id", &store_id);
                stmt.add_param("after_seq", &after_seq);
                stmt.add_param("object_type", &object_type.unwrap_or_default());
                let rows: Vec<SpannerRow> =
                    self.spanner_query_all(stmt, "read fga changes").await?;
                changes.reserve(rows.len());
                for row in rows {
                    let seq = row
                        .column_by_name::<i64>("seq")
                        .context("read spanner change seq")?;
                    next = Some(seq.to_string());
                    changes.push(TupleChangeRecord {
                        tuple_key: TupleKey {
                            user: row
                                .column_by_name::<String>("raw_user")
                                .context("read spanner raw_user")?,
                            relation: row
                                .column_by_name::<String>("relation")
                                .context("read spanner relation")?,
                            object: row
                                .column_by_name::<String>("raw_object")
                                .context("read spanner raw_object")?,
                            condition: None,
                        },
                        operation: row
                            .column_by_name::<String>("operation")
                            .context("read spanner change operation")?,
                        timestamp: row
                            .column_by_name::<String>("created_at")
                            .context("read spanner change created_at")?,
                    });
                }
            }
        }
        if changes.len() < limit as usize {
            next = None;
        }
        Ok(ReadChangesResponse {
            changes,
            continuation_token: next,
        })
    }
}

#[async_trait]
impl Evaluator for FgaService {
    async fn check(
        &self,
        instance_id: &str,
        store_id: &str,
        request: CheckRequest,
    ) -> Result<CheckResponse, FgaError> {
        if request.context.is_some() {
            return Err(FgaError::Unsupported(
                "request context is not supported by the embedded v1 server".into(),
            ));
        }
        let tuple = ParsedTupleKey::parse(request.tuple_key)?;
        let (model_id, model) = self
            .load_compiled_model(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let contextual = parse_contextual(request.contextual_tuples)?;
        let mut ctx = self.evaluate_internal(instance_id, store_id, model.as_ref(), &contextual);
        let outcome = ctx
            .check(&tuple.user, &tuple.relation, &tuple.object, 0)
            .await?;
        ctx.record_request_issue(&tuple.user, &tuple.relation, &tuple.object, outcome);
        ctx.warn_if_needed(&model_id);
        Ok(CheckResponse {
            allowed: outcome.is_allowed(),
        })
    }

    async fn batch_check(
        &self,
        instance_id: &str,
        store_id: &str,
        request: BatchCheckRequest,
    ) -> Result<BatchCheckResponse, FgaError> {
        if request.context.is_some() {
            return Err(FgaError::Unsupported(
                "request context is not supported by the embedded v1 server".into(),
            ));
        }
        let (model_id, model) = self
            .load_compiled_model(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let contextual = parse_contextual(request.contextual_tuples)?;
        let mut ctx = self.evaluate_internal(instance_id, store_id, model.as_ref(), &contextual);
        let mut results = Vec::with_capacity(request.checks.len());
        for item in request.checks {
            let tuple = ParsedTupleKey::parse(item.tuple_key)?;
            let outcome = ctx
                .check(&tuple.user, &tuple.relation, &tuple.object, 0)
                .await?;
            ctx.record_request_issue(&tuple.user, &tuple.relation, &tuple.object, outcome);
            results.push(BatchCheckResult {
                correlation_id: item.correlation_id,
                allowed: outcome.is_allowed(),
            });
        }
        ctx.warn_if_needed(&model_id);
        Ok(BatchCheckResponse { results })
    }

    async fn expand(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ExpandRequest,
    ) -> Result<ExpandResponse, FgaError> {
        let object = ObjectRef::parse(&request.object)?;
        let (_, model) = self
            .load_compiled_model(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let contextual = parse_contextual(request.contextual_tuples)?;
        let mut ctx = self.evaluate_internal(instance_id, store_id, model.as_ref(), &contextual);
        let tree = ctx.expand(&object, &request.relation, 0).await?;
        Ok(ExpandResponse { tree })
    }

    async fn list_objects(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ListObjectsRequest,
    ) -> Result<ListObjectsResponse, FgaError> {
        let user = UserRef::parse(&request.user)?;
        let (model_id, model) = self
            .load_compiled_model(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let contextual = parse_contextual(request.contextual_tuples)?;
        let candidates = match self
            .planned_object_candidates(
                instance_id,
                store_id,
                model.as_ref(),
                &request.object_type,
                &request.relation,
                &user,
                &contextual,
                &mut HashSet::new(),
            )
            .await?
        {
            Some(candidates) => candidates,
            None => {
                self.scan_candidate_objects(
                    instance_id,
                    store_id,
                    &request.object_type,
                    &contextual,
                )
                .await?
            }
        };
        let mut ctx = self.evaluate_internal(instance_id, store_id, model.as_ref(), &contextual);
        let mut objects = Vec::new();
        for object in candidates {
            let outcome = ctx.check(&user, &request.relation, &object, 0).await?;
            ctx.record_request_issue(&user, &request.relation, &object, outcome);
            if outcome.is_allowed() {
                objects.push(object.as_raw());
            }
        }
        ctx.warn_if_needed(&model_id);
        Ok(ListObjectsResponse { objects })
    }

    async fn list_users(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ListUsersRequest,
    ) -> Result<ListUsersResponse, FgaError> {
        let object = ObjectRef::parse(&request.object)?;
        let (model_id, model) = self
            .load_compiled_model(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let contextual = parse_contextual(request.contextual_tuples)?;
        let mut ctx = self.evaluate_internal(instance_id, store_id, model.as_ref(), &contextual);
        let mut users = BTreeSet::new();
        for filter in &request.user_filters {
            let candidates = match self
                .planned_user_candidates(
                    instance_id,
                    store_id,
                    model.as_ref(),
                    &object,
                    &request.relation,
                    filter,
                    &contextual,
                    &mut HashSet::new(),
                )
                .await?
            {
                Some(candidates) => candidates,
                None => {
                    self.scan_candidate_users(instance_id, store_id, filter, &contextual)
                        .await?
                }
            };
            for user in candidates {
                let outcome = ctx.check(&user, &request.relation, &object, 0).await?;
                ctx.record_request_issue(&user, &request.relation, &object, outcome);
                if outcome.is_allowed() {
                    users.insert(user.as_raw());
                }
            }
        }
        ctx.warn_if_needed(&model_id);
        Ok(ListUsersResponse {
            users: users.into_iter().collect(),
        })
    }
}

#[async_trait]
impl FgaApi for FgaService {
    async fn legacy_model(&self, instance_id: &str) -> Result<LegacyModelResponse, FgaError> {
        let store = self.ensure_store_row(instance_id).await?;
        let model = self.read_model(instance_id, &store.id, None).await?;
        Ok(LegacyModelResponse {
            schema_version: model.schema_version,
            types: model
                .type_definitions
                .into_iter()
                .map(|type_def| LegacyModelType {
                    type_name: type_def.type_name,
                    relations: type_def.relations.keys().cloned().collect(),
                })
                .collect(),
        })
    }

    async fn legacy_model_graph(&self, instance_id: &str) -> Result<ModelGraphResponse, FgaError> {
        let store = self.ensure_store_row(instance_id).await?;
        let (_, compiled) = self
            .load_compiled_model(instance_id, &store.id, None)
            .await?;
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for (type_name, type_def) in &compiled.types {
            let mut permissions = Vec::new();
            for (relation, compiled_relation) in &type_def.relations {
                if !matches!(&compiled_relation.expr, RelationExpr::This) {
                    permissions.push(relation.clone());
                }
                collect_graph_edges(type_name, relation, &compiled_relation.expr, &mut edges);
            }
            nodes.push(ModelGraphNode {
                id: type_name.clone(),
                relations: type_def.relations.keys().cloned().collect(),
                permissions,
            });
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        edges.sort_by(|a, b| {
            a.from
                .cmp(&b.from)
                .then(a.to.cmp(&b.to))
                .then(a.relation.cmp(&b.relation))
        });
        Ok(ModelGraphResponse { nodes, edges })
    }
}
