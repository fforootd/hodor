use anyhow::Context;
use google_cloud_spanner::statement::Statement;

use super::entities::{
    SqlUserRepository, UserSqlRow, json_string, limit_from_params, load_user, next_cursor,
    spanner_query_all, spanner_query_optional, user_from_spanner_row, user_from_sql_row,
    write_spanner_count, write_spanner_stmt,
};
use crate::Db;
use zitadel_app::repo::{BoxFuture, ListParams, ListResult, UserRecord, UserRepository};

impl UserRepository for SqlUserRepository {
    fn create(
        &self,
        instance_id: &str,
        user: &UserRecord,
    ) -> BoxFuture<'_, anyhow::Result<UserRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user = user.clone();
        Box::pin(async move {
            let metadata_json = json_string(&user.metadata)?;
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "INSERT INTO users \
                         (id, instance_id, org_id, identifier, display_name, user_type, state, schema_id, metadata) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, {})",
                        scoped.json_bind(9),
                    );
                    sqlx::query(&sql)
                        .bind(&user.id)
                        .bind(&instance_id)
                        .bind(&user.org_id)
                        .bind(&user.identifier)
                        .bind(&user.display_name)
                        .bind(&user.user_type)
                        .bind(&user.state)
                        .bind(&user.schema_id)
                        .bind(&metadata_json)
                        .execute(scoped.pool())
                        .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "INSERT INTO users \
                         (id, instance_id, org_id, identifier, display_name, user_type, state, schema_id, metadata) \
                         VALUES (@id, @instance_id, @org_id, @identifier, @display_name, @user_type, @state, @schema_id, @metadata)",
                    );
                    stmt.add_param("id", &user.id);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("org_id", &user.org_id);
                    stmt.add_param("identifier", &user.identifier);
                    stmt.add_param("display_name", &user.display_name);
                    stmt.add_param("user_type", &user.user_type);
                    stmt.add_param("state", &user.state);
                    stmt.add_param("schema_id", &user.schema_id);
                    stmt.add_param("metadata", &metadata_json);
                    write_spanner_stmt(spanner, stmt).await?;
                }
            }

            load_user(&db, &instance_id, &user.id)
                .await?
                .context("created user but could not reload it")
        })
    }

    fn get(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move { load_user(&db, &instance_id, &user_id).await })
    }

    fn find_by_identifier(
        &self,
        instance_id: &str,
        identifier: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let identifier = identifier.to_string();
        Box::pin(async move {
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let metadata = scoped.as_text("metadata");
                    let (created_at, updated_at) = scoped.select_timestamps();
                    let sql = format!(
                        "SELECT id, COALESCE(org_id, ''), identifier, display_name, user_type, state, schema_id, \
                                COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                         FROM users \
                         WHERE instance_id = $1 AND identifier = $2 AND state = 'active' \
                         LIMIT 1"
                    );
                    Ok(sqlx::query_as::<_, UserSqlRow>(&sql)
                        .bind(&instance_id)
                        .bind(&identifier)
                        .fetch_optional(scoped.pool())
                        .await?
                        .map(user_from_sql_row))
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "SELECT id, IFNULL(org_id, '') AS org_id, identifier, display_name, user_type, state, schema_id, \
                                IFNULL(metadata, '{}') AS metadata, \
                                CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                         FROM users \
                         WHERE instance_id = @instance_id AND identifier = @identifier AND state = 'active' \
                         LIMIT 1",
                    );
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("identifier", &identifier);
                    Ok(spanner_query_optional(spanner, stmt)
                        .await?
                        .map(user_from_spanner_row))
                }
            }
        })
    }

    fn list(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<UserRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org_id = org_id.map(str::to_string);
        let params = params.clone();
        Box::pin(async move {
            let limit = limit_from_params(&params);
            let cursor = params.cursor.unwrap_or_default();
            let search = params.search.filter(|value| !value.is_empty());

            let items: Vec<UserRecord> = match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let metadata = scoped.as_text("metadata");
                    let (created_at, updated_at) = scoped.select_timestamps();
                    let mut conditions = vec![
                        format!("instance_id = {}", scoped.placeholder(1)),
                        format!("id > {}", scoped.placeholder(2)),
                    ];
                    let mut next = 3usize;
                    if org_id.is_some() {
                        conditions.push(format!("org_id = {}", scoped.placeholder(next)));
                        next += 1;
                    }
                    if search.is_some() {
                        conditions.push(format!(
                            "(identifier LIKE {p} OR display_name LIKE {p})",
                            p = scoped.placeholder(next)
                        ));
                        next += 1;
                    }
                    let sql = format!(
                        "SELECT id, COALESCE(org_id, ''), identifier, display_name, user_type, state, schema_id, \
                                COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                         FROM users WHERE {} ORDER BY id LIMIT {}",
                        conditions.join(" AND "),
                        scoped.placeholder(next),
                    );
                    let mut query = sqlx::query_as::<_, UserSqlRow>(&sql)
                        .bind(&instance_id)
                        .bind(&cursor);
                    if let Some(org_id) = &org_id {
                        query = query.bind(org_id);
                    }
                    if let Some(search) = &search {
                        query = query.bind(format!("%{search}%"));
                    }
                    query = query.bind(limit);
                    query
                        .fetch_all(scoped.pool())
                        .await?
                        .into_iter()
                        .map(user_from_sql_row)
                        .collect::<Vec<_>>()
                }
                Db::Spanner(spanner) => {
                    let mut sql = String::from(
                        "SELECT id, IFNULL(org_id, '') AS org_id, identifier, display_name, user_type, state, schema_id, \
                                IFNULL(metadata, '{}') AS metadata, \
                                CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                         FROM users WHERE instance_id = @instance_id AND id > @cursor",
                    );
                    if org_id.is_some() {
                        sql.push_str(" AND org_id = @org_id");
                    }
                    if search.is_some() {
                        sql.push_str(
                            " AND (identifier LIKE @pattern OR display_name LIKE @pattern)",
                        );
                    }
                    sql.push_str(" ORDER BY id LIMIT @limit");
                    let mut stmt = Statement::new(sql);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("cursor", &cursor);
                    if let Some(org_id) = &org_id {
                        stmt.add_param("org_id", org_id);
                    }
                    if let Some(search) = &search {
                        stmt.add_param("pattern", &format!("%{search}%"));
                    }
                    stmt.add_param("limit", &limit);
                    spanner_query_all(spanner, stmt)
                        .await?
                        .into_iter()
                        .map(user_from_spanner_row)
                        .collect::<Vec<_>>()
                }
            };

            Ok(ListResult {
                next_cursor: next_cursor(&items, limit, |item| item.id.as_str()),
                total_count: None,
                items,
            })
        })
    }

    fn update(
        &self,
        instance_id: &str,
        user: &UserRecord,
    ) -> BoxFuture<'_, anyhow::Result<UserRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user = user.clone();
        Box::pin(async move {
            let metadata_json = json_string(&user.metadata)?;
            let updated = match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "UPDATE users \
                         SET org_id = $1, identifier = $2, display_name = $3, user_type = $4, \
                             state = $5, schema_id = $6, metadata = {}, updated_at = CURRENT_TIMESTAMP \
                         WHERE instance_id = $8 AND id = $9",
                        scoped.json_bind(7),
                    );
                    sqlx::query(&sql)
                        .bind(&user.org_id)
                        .bind(&user.identifier)
                        .bind(&user.display_name)
                        .bind(&user.user_type)
                        .bind(&user.state)
                        .bind(&user.schema_id)
                        .bind(&metadata_json)
                        .bind(&instance_id)
                        .bind(&user.id)
                        .execute(scoped.pool())
                        .await?
                        .rows_affected()
                        > 0
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "UPDATE users \
                         SET org_id = @org_id, identifier = @identifier, display_name = @display_name, \
                             user_type = @user_type, state = @state, schema_id = @schema_id, \
                             metadata = @metadata, updated_at = CURRENT_TIMESTAMP() \
                         WHERE instance_id = @instance_id AND id = @id",
                    );
                    stmt.add_param("org_id", &user.org_id);
                    stmt.add_param("identifier", &user.identifier);
                    stmt.add_param("display_name", &user.display_name);
                    stmt.add_param("user_type", &user.user_type);
                    stmt.add_param("state", &user.state);
                    stmt.add_param("schema_id", &user.schema_id);
                    stmt.add_param("metadata", &metadata_json);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("id", &user.id);
                    write_spanner_count(spanner, stmt).await? > 0
                }
            };
            if !updated {
                anyhow::bail!("user not found");
            }
            load_user(&db, &instance_id, &user.id)
                .await?
                .context("updated user but could not reload it")
        })
    }

    fn deactivate(&self, instance_id: &str, user_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move {
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    sqlx::query(
                        "UPDATE users SET state = $1, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $2 AND id = $3",
                    )
                    .bind("inactive")
                    .bind(&instance_id)
                    .bind(&user_id)
                    .execute(scoped.pool())
                    .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "UPDATE users SET state = @state, updated_at = CURRENT_TIMESTAMP() \
                         WHERE instance_id = @instance_id AND id = @id",
                    );
                    stmt.add_param("state", &"inactive");
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("id", &user_id);
                    let _ = write_spanner_count(spanner, stmt).await?;
                }
            }
            Ok(())
        })
    }

    fn delete(&self, instance_id: &str, user_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move {
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    sqlx::query(
                        "DELETE FROM users WHERE instance_id = $1 AND id = $2",
                    )
                    .bind(&instance_id)
                    .bind(&user_id)
                    .execute(scoped.pool())
                    .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "DELETE FROM users WHERE instance_id = @instance_id AND id = @id",
                    );
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("id", &user_id);
                    let _ = write_spanner_count(spanner, stmt).await?;
                }
            }
            Ok(())
        })
    }
}
