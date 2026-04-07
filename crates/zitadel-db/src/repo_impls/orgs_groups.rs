use anyhow::Context;
use google_cloud_spanner::statement::Statement;

use super::entities::{
    GroupSqlRow, OrgSqlRow, SqlGroupRepository, SqlOrgRepository, group_from_spanner_row,
    group_from_sql_row, json_string, limit_from_params, load_group, load_org, next_cursor,
    org_from_spanner_row, org_from_sql_row, spanner_query_all, spanner_query_optional,
    write_spanner_count, write_spanner_stmt,
};
use crate::{Db, Dialect, delete_instance_row, first_org_id, spanner_ident};
use zitadel_app::repo::{
    BoxFuture, GroupRecord, GroupRepository, ListParams, ListResult, OrgRecord, OrgRepository,
};

impl OrgRepository for SqlOrgRepository {
    fn create(
        &self,
        instance_id: &str,
        org: &OrgRecord,
    ) -> BoxFuture<'_, anyhow::Result<OrgRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org = org.clone();
        Box::pin(async move {
            let metadata_json = json_string(&org.metadata)?;
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "INSERT INTO orgs (id, instance_id, name, state, metadata) \
                         VALUES ($1, $2, $3, $4, {})",
                        scoped.json_bind(5),
                    );
                    sqlx::query(&sql)
                        .bind(&org.id)
                        .bind(&instance_id)
                        .bind(&org.name)
                        .bind(&org.state)
                        .bind(&metadata_json)
                        .execute(scoped.pool())
                        .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "INSERT INTO orgs (id, instance_id, name, state, metadata) \
                         VALUES (@id, @instance_id, @name, @state, @metadata)",
                    );
                    stmt.add_param("id", &org.id);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("name", &org.name);
                    stmt.add_param("state", &org.state);
                    stmt.add_param("metadata", &metadata_json);
                    write_spanner_stmt(spanner, stmt).await?;
                }
            }
            load_org(&db, &instance_id, &org.id)
                .await?
                .context("created org but could not reload it")
        })
    }

    fn get(
        &self,
        instance_id: &str,
        org_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OrgRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org_id = org_id.to_string();
        Box::pin(async move { load_org(&db, &instance_id, &org_id).await })
    }

    fn list(
        &self,
        instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<OrgRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let params = params.clone();
        Box::pin(async move {
            let limit = limit_from_params(&params);
            let cursor = params.cursor.unwrap_or_default();
            let search = params.search.filter(|value| !value.is_empty());
            let items: Vec<OrgRecord> = match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let metadata = scoped.as_text("metadata");
                    let (created_at, updated_at) = scoped.select_timestamps();
                    let mut sql = format!(
                        "SELECT id, name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                         FROM orgs WHERE instance_id = $1 AND id > $2"
                    );
                    if search.is_some() {
                        sql.push_str(" AND name LIKE $3");
                        sql.push_str(" ORDER BY id LIMIT $4");
                    } else {
                        sql.push_str(" ORDER BY id LIMIT $3");
                    }
                    let mut query = sqlx::query_as::<_, OrgSqlRow>(&sql)
                        .bind(&instance_id)
                        .bind(&cursor);
                    if let Some(search) = &search {
                        query = query.bind(format!("%{search}%"));
                    }
                    query = query.bind(limit);
                    query
                        .fetch_all(scoped.pool())
                        .await?
                        .into_iter()
                        .map(org_from_sql_row)
                        .collect::<Vec<_>>()
                }
                Db::Spanner(spanner) => {
                    let mut sql = String::from(
                        "SELECT id, name, state, IFNULL(metadata, '{}') AS metadata, \
                                CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                         FROM orgs WHERE instance_id = @instance_id AND id > @cursor",
                    );
                    if search.is_some() {
                        sql.push_str(" AND name LIKE @pattern");
                    }
                    sql.push_str(" ORDER BY id LIMIT @limit");
                    let mut stmt = Statement::new(sql);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("cursor", &cursor);
                    if let Some(search) = &search {
                        stmt.add_param("pattern", &format!("%{search}%"));
                    }
                    stmt.add_param("limit", &limit);
                    spanner_query_all(spanner, stmt)
                        .await?
                        .into_iter()
                        .map(org_from_spanner_row)
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
        org: &OrgRecord,
    ) -> BoxFuture<'_, anyhow::Result<OrgRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org = org.clone();
        Box::pin(async move {
            let metadata_json = json_string(&org.metadata)?;
            let updated = match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "UPDATE orgs SET name = $1, state = $2, metadata = {}, updated_at = CURRENT_TIMESTAMP \
                         WHERE instance_id = $4 AND id = $5",
                        scoped.json_bind(3),
                    );
                    sqlx::query(&sql)
                        .bind(&org.name)
                        .bind(&org.state)
                        .bind(&metadata_json)
                        .bind(&instance_id)
                        .bind(&org.id)
                        .execute(scoped.pool())
                        .await?
                        .rows_affected()
                        > 0
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "UPDATE orgs SET name = @name, state = @state, metadata = @metadata, \
                             updated_at = CURRENT_TIMESTAMP() WHERE instance_id = @instance_id AND id = @id",
                    );
                    stmt.add_param("name", &org.name);
                    stmt.add_param("state", &org.state);
                    stmt.add_param("metadata", &metadata_json);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("id", &org.id);
                    write_spanner_count(spanner, stmt).await? > 0
                }
            };
            if !updated {
                anyhow::bail!("org not found");
            }
            load_org(&db, &instance_id, &org.id)
                .await?
                .context("updated org but could not reload it")
        })
    }

    fn delete(&self, instance_id: &str, org_id: &str) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org_id = org_id.to_string();
        Box::pin(async move {
            // Detach resources from this org before deleting it.
            // SQLite cannot do column-specific ON DELETE SET NULL for composite FKs,
            // so we handle it in application code for all backends.
            const DETACH_TABLES: &[&str] = &[
                "users",
                "sessions",
                "apps",
                "providers",
                "login_flows",
                "login_flow_assets",
                "groups",
                "projects",
                "actions",
                "domains",
            ];
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    for table in DETACH_TABLES {
                        sqlx::query(&format!(
                            "UPDATE {table} SET org_id = NULL WHERE instance_id = $1 AND org_id = $2"
                        ))
                        .bind(&instance_id)
                        .bind(&org_id)
                        .execute(scoped.pool())
                        .await?;
                    }
                }
                Db::Spanner(spanner) => {
                    for table in DETACH_TABLES {
                        let table = spanner_ident(table);
                        let mut stmt = Statement::new(format!(
                            "UPDATE {table} SET org_id = NULL WHERE instance_id = @instance_id AND org_id = @org_id"
                        ));
                        stmt.add_param("instance_id", &instance_id);
                        stmt.add_param("org_id", &org_id);
                        let _ = write_spanner_count(spanner, stmt).await;
                    }
                }
            }

            let deleted = match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    sqlx::query("DELETE FROM orgs WHERE instance_id = $1 AND id = $2")
                        .bind(&instance_id)
                        .bind(&org_id)
                        .execute(scoped.pool())
                        .await?
                        .rows_affected()
                        > 0
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "DELETE FROM orgs WHERE instance_id = @instance_id AND id = @id",
                    );
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("id", &org_id);
                    write_spanner_count(spanner, stmt).await? > 0
                }
            };
            Ok(deleted)
        })
    }

    fn first_org_id(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Option<String>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move { first_org_id(&db, &instance_id).await })
    }
}

impl GroupRepository for SqlGroupRepository {
    fn create(
        &self,
        instance_id: &str,
        group: &GroupRecord,
    ) -> BoxFuture<'_, anyhow::Result<GroupRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let group = group.clone();
        Box::pin(async move {
            let metadata_json = json_string(&group.metadata)?;
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "INSERT INTO groups (id, instance_id, org_id, name, state, metadata) \
                         VALUES ($1, $2, $3, $4, $5, {})",
                        scoped.json_bind(6),
                    );
                    sqlx::query(&sql)
                        .bind(&group.id)
                        .bind(&instance_id)
                        .bind(&group.org_id)
                        .bind(&group.name)
                        .bind(&group.state)
                        .bind(&metadata_json)
                        .execute(scoped.pool())
                        .await?;
                }
                Db::Spanner(spanner) => {
                    let groups = spanner_ident("groups");
                    let mut stmt = Statement::new(format!(
                        "INSERT INTO {groups} (id, instance_id, org_id, name, state, metadata) \
                         VALUES (@id, @instance_id, @org_id, @name, @state, @metadata)"
                    ));
                    stmt.add_param("id", &group.id);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("org_id", &group.org_id);
                    stmt.add_param("name", &group.name);
                    stmt.add_param("state", &group.state);
                    stmt.add_param("metadata", &metadata_json);
                    write_spanner_stmt(spanner, stmt).await?;
                }
            }
            load_group(&db, &instance_id, &group.id)
                .await?
                .context("created group but could not reload it")
        })
    }

    fn get(
        &self,
        instance_id: &str,
        group_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<GroupRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let group_id = group_id.to_string();
        Box::pin(async move { load_group(&db, &instance_id, &group_id).await })
    }

    fn list(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<GroupRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let org_id = org_id.map(str::to_string);
        let params = params.clone();
        Box::pin(async move {
            let limit = limit_from_params(&params);
            let cursor = params.cursor.unwrap_or_default();
            let search = params.search.filter(|value| !value.is_empty());
            let items: Vec<GroupRecord> = match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let metadata = scoped.as_text("metadata");
                    let (created_at, updated_at) = scoped.select_timestamps();
                    let mut conditions =
                        vec!["instance_id = $1".to_string(), "id > $2".to_string()];
                    let mut next = 3usize;
                    if org_id.is_some() {
                        conditions.push(format!("org_id = ${next}"));
                        next += 1;
                    }
                    if search.is_some() {
                        conditions.push(format!("name LIKE ${next}"));
                        next += 1;
                    }
                    let sql = format!(
                        "SELECT id, COALESCE(org_id, ''), name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                         FROM groups WHERE {} ORDER BY id LIMIT ${next}",
                        conditions.join(" AND "),
                    );
                    let mut query = sqlx::query_as::<_, GroupSqlRow>(&sql)
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
                        .map(group_from_sql_row)
                        .collect::<Vec<_>>()
                }
                Db::Spanner(spanner) => {
                    let groups = spanner_ident("groups");
                    let mut sql = format!(
                        "SELECT id, IFNULL(org_id, '') AS org_id, name, state, IFNULL(metadata, '{{}}') AS metadata, \
                         CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                         FROM {groups} WHERE instance_id = @instance_id AND id > @cursor"
                    );
                    if org_id.is_some() {
                        sql.push_str(" AND org_id = @org_id");
                    }
                    if search.is_some() {
                        sql.push_str(" AND name LIKE @pattern");
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
                        .map(group_from_spanner_row)
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
        group: &GroupRecord,
    ) -> BoxFuture<'_, anyhow::Result<GroupRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let group = group.clone();
        Box::pin(async move {
            let metadata_json = json_string(&group.metadata)?;
            let updated = match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "UPDATE groups SET org_id = $1, name = $2, state = $3, metadata = {}, updated_at = CURRENT_TIMESTAMP \
                         WHERE instance_id = $5 AND id = $6",
                        scoped.json_bind(4),
                    );
                    sqlx::query(&sql)
                        .bind(&group.org_id)
                        .bind(&group.name)
                        .bind(&group.state)
                        .bind(&metadata_json)
                        .bind(&instance_id)
                        .bind(&group.id)
                        .execute(scoped.pool())
                        .await?
                        .rows_affected()
                        > 0
                }
                Db::Spanner(spanner) => {
                    let groups = spanner_ident("groups");
                    let mut stmt = Statement::new(format!(
                        "UPDATE {groups} SET org_id = @org_id, name = @name, state = @state, metadata = @metadata, \
                         updated_at = CURRENT_TIMESTAMP() WHERE instance_id = @instance_id AND id = @id"
                    ));
                    stmt.add_param("org_id", &group.org_id);
                    stmt.add_param("name", &group.name);
                    stmt.add_param("state", &group.state);
                    stmt.add_param("metadata", &metadata_json);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("id", &group.id);
                    write_spanner_count(spanner, stmt).await? > 0
                }
            };
            if !updated {
                anyhow::bail!("group not found");
            }
            load_group(&db, &instance_id, &group.id)
                .await?
                .context("updated group but could not reload it")
        })
    }

    fn delete(&self, instance_id: &str, group_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let group_id = group_id.to_string();
        Box::pin(async move {
            delete_instance_row(&db, &instance_id, "groups", &group_id).await?;
            Ok(())
        })
    }

    fn add_member(
        &self,
        instance_id: &str,
        group_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let group_id = group_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move {
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = match db.dialect() {
                        Dialect::Postgres => {
                            "INSERT INTO memberships (instance_id, resource_type, resource_id, user_id, role) \
                             VALUES ($1, $2, $3, $4, $5) \
                             ON CONFLICT (instance_id, resource_type, resource_id, user_id) DO NOTHING"
                        }
                        Dialect::Sqlite => {
                            "INSERT OR IGNORE INTO memberships (instance_id, resource_type, resource_id, user_id, role) \
                             VALUES ($1, $2, $3, $4, $5)"
                        }
                        Dialect::Spanner => unreachable!(),
                    };
                    sqlx::query(sql)
                        .bind(&instance_id)
                        .bind("group")
                        .bind(&group_id)
                        .bind(&user_id)
                        .bind("member")
                        .execute(scoped.pool())
                        .await?;
                }
                Db::Spanner(spanner) => {
                    let mut exists = Statement::new(
                        "SELECT user_id FROM memberships \
                         WHERE instance_id = @instance_id AND resource_type = @resource_type \
                           AND resource_id = @resource_id AND user_id = @user_id LIMIT 1",
                    );
                    exists.add_param("instance_id", &instance_id);
                    exists.add_param("resource_type", &"group");
                    exists.add_param("resource_id", &group_id);
                    exists.add_param("user_id", &user_id);
                    if spanner_query_optional(spanner, exists).await?.is_none() {
                        let mut stmt = Statement::new(
                            "INSERT INTO memberships (instance_id, resource_type, resource_id, user_id, role) \
                             VALUES (@instance_id, @resource_type, @resource_id, @user_id, @role)",
                        );
                        stmt.add_param("instance_id", &instance_id);
                        stmt.add_param("resource_type", &"group");
                        stmt.add_param("resource_id", &group_id);
                        stmt.add_param("user_id", &user_id);
                        stmt.add_param("role", &"member");
                        write_spanner_stmt(spanner, stmt).await?;
                    }
                }
            }
            Ok(())
        })
    }

    fn remove_member(
        &self,
        instance_id: &str,
        group_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let group_id = group_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move {
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    sqlx::query(
                        "DELETE FROM memberships WHERE instance_id = $1 AND resource_type = $2 AND resource_id = $3 AND user_id = $4",
                    )
                    .bind(&instance_id)
                    .bind("group")
                    .bind(&group_id)
                    .bind(&user_id)
                    .execute(scoped.pool())
                    .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "DELETE FROM memberships \
                         WHERE instance_id = @instance_id AND resource_type = @resource_type \
                           AND resource_id = @resource_id AND user_id = @user_id",
                    );
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("resource_type", &"group");
                    stmt.add_param("resource_id", &group_id);
                    stmt.add_param("user_id", &user_id);
                    let _ = write_spanner_count(spanner, stmt).await?;
                }
            }
            Ok(())
        })
    }
}
