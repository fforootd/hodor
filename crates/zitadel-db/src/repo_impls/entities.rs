use anyhow::Context;
use google_cloud_spanner::{
    client::Error as SpannerError, row::Row as SpannerRow, statement::Statement,
};
use serde_json::{Map, Value};
use std::collections::HashSet;

use crate::{
    DEFAULT_ORG_ID, Db, Dialect, SpannerDb, add_instance_domain, delete_instance_row,
    delete_provider, first_org_id, get_org, get_schema_record, get_user, list_instance_domains,
    list_managed_instances, list_schema_registry, provider, resolve_domain_route,
};
use zitadel_app::repo::{
    BoxFuture, DomainRecord, GroupRecord, GroupRepository, InstanceRecord, InstanceRepository,
    ListParams, ListResult, OrgRecord, OrgRepository, ProviderRecord, ProviderRepository,
    RouteResolution, SchemaRecord, SchemaRepository, SearchRepository, SearchResult,
    SettingsRecord, SettingsRepository, UserRecord, UserRepository,
};

const DEFAULT_LIST_LIMIT: i64 = 50;
const MAX_LIST_LIMIT: i64 = 200;

#[derive(Clone)]
pub struct SqlUserRepository {
    db: Db,
}

#[derive(Clone)]
pub struct SqlOrgRepository {
    db: Db,
}

#[derive(Clone)]
pub struct SqlGroupRepository {
    db: Db,
}

#[derive(Clone)]
pub struct SqlInstanceRepository {
    db: Db,
}

#[derive(Clone)]
pub struct SqlProviderRepository {
    db: Db,
}

#[derive(Clone)]
pub struct SqlSchemaRepository {
    db: Db,
}

#[derive(Clone)]
pub struct SqlSettingsRepository {
    db: Db,
}

#[derive(Clone)]
pub struct SqlSearchRepository {
    db: Db,
}

impl SqlUserRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlOrgRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlGroupRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlInstanceRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlProviderRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlSchemaRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlSettingsRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlSearchRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

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
                        "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, \
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
                        "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, \
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
                        "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, \
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
                        "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, \
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
}

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
                    let mut stmt = Statement::new(
                        "INSERT INTO groups (id, instance_id, org_id, name, state, metadata) \
                         VALUES (@id, @instance_id, @org_id, @name, @state, @metadata)",
                    );
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
                        "SELECT id, org_id, name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
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
                    let mut sql = String::from(
                        "SELECT id, org_id, name, state, IFNULL(metadata, '{}') AS metadata, \
                                CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                         FROM groups WHERE instance_id = @instance_id AND id > @cursor",
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
                    let mut stmt = Statement::new(
                        "UPDATE groups SET org_id = @org_id, name = @name, state = @state, metadata = @metadata, \
                             updated_at = CURRENT_TIMESTAMP() WHERE instance_id = @instance_id AND id = @id",
                    );
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

impl InstanceRepository for SqlInstanceRepository {
    fn create(
        &self,
        root_instance_id: &str,
        instance: &InstanceRecord,
    ) -> BoxFuture<'_, anyhow::Result<InstanceRecord>> {
        let db = self.db.clone();
        let root_instance_id = root_instance_id.to_string();
        let instance = instance.clone();
        Box::pin(async move {
            let feature_overrides_json = json_string(&instance.feature_overrides)?;
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(root_instance_id.clone());
                    let mut tx = scoped.pool().begin().await?;
                    let sql = format!(
                        "INSERT INTO instances \
                         (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, region_key, feature_overrides) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, {})",
                        scoped.json_bind(8),
                    );
                    sqlx::query(&sql)
                        .bind(&instance.instance_id)
                        .bind(&root_instance_id)
                        .bind(&instance.owner_org_id)
                        .bind(&instance.kind)
                        .bind(&instance.state)
                        .bind(&instance.placement_mode)
                        .bind(&instance.region_key)
                        .bind(&feature_overrides_json)
                        .execute(&mut *tx)
                        .await?;
                    if let Some(primary_domain) = &instance.primary_domain {
                        sqlx::query(
                            "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                             VALUES ($1, $2, TRUE, 'active', FALSE)",
                        )
                        .bind(primary_domain)
                        .bind(&instance.instance_id)
                        .execute(&mut *tx)
                        .await?;
                    }
                    tx.commit().await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmts = vec![{
                        let mut stmt = Statement::new(
                            "INSERT INTO instances \
                             (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, region_key, feature_overrides) \
                             VALUES (@instance_id, @parent_instance_id, @owner_org_id, @kind, @state, @placement_mode, @region_key, @feature_overrides)",
                        );
                        stmt.add_param("instance_id", &instance.instance_id);
                        stmt.add_param("parent_instance_id", &root_instance_id);
                        stmt.add_param("owner_org_id", &instance.owner_org_id);
                        stmt.add_param("kind", &instance.kind);
                        stmt.add_param("state", &instance.state);
                        stmt.add_param("placement_mode", &instance.placement_mode);
                        stmt.add_param("region_key", &instance.region_key);
                        stmt.add_param("feature_overrides", &feature_overrides_json);
                        stmt
                    }];
                    if let Some(primary_domain) = &instance.primary_domain {
                        let mut stmt = Statement::new(
                            "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                             VALUES (@domain, @instance_id, TRUE, 'active', FALSE)",
                        );
                        stmt.add_param("domain", primary_domain);
                        stmt.add_param("instance_id", &instance.instance_id);
                        stmts.push(stmt);
                    }
                    write_spanner_many(spanner, stmts).await?;
                }
            }
            load_instance(&db, &instance.instance_id)
                .await?
                .context("created instance but could not reload it")
        })
    }

    fn get(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Option<InstanceRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move { load_instance(&db, &instance_id).await })
    }

    fn list(
        &self,
        root_instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<InstanceRecord>>> {
        let db = self.db.clone();
        let root_instance_id = root_instance_id.to_string();
        let params = params.clone();
        Box::pin(async move {
            let limit = limit_from_params(&params);
            let cursor = params.cursor.unwrap_or_default();
            let search = params.search.map(|value| value.to_lowercase());
            let mut items: Vec<InstanceRecord> =
                list_managed_instances(&db, &root_instance_id, None, &cursor, limit)
                    .await?
                    .into_iter()
                    .map(instance_from_retained)
                    .collect();

            if let Some(search) = search {
                items.retain(|item| {
                    item.instance_id.to_lowercase().contains(&search)
                        || item.owner_org_id.to_lowercase().contains(&search)
                        || item
                            .primary_domain
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&search)
                });
            }

            Ok(ListResult {
                next_cursor: next_cursor(&items, limit, |item| item.instance_id.as_str()),
                total_count: None,
                items,
            })
        })
    }

    fn update(&self, instance: &InstanceRecord) -> BoxFuture<'_, anyhow::Result<InstanceRecord>> {
        let db = self.db.clone();
        let instance = instance.clone();
        Box::pin(async move {
            let feature_overrides_json = json_string(&instance.feature_overrides)?;
            let updated = match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance.instance_id.clone());
                    let sql = format!(
                        "UPDATE instances \
                         SET owner_org_id = $1, kind = $2, state = $3, placement_mode = $4, \
                             region_key = $5, feature_overrides = {}, updated_at = CURRENT_TIMESTAMP \
                         WHERE instance_id = $7",
                        scoped.json_bind(6),
                    );
                    sqlx::query(&sql)
                        .bind(&instance.owner_org_id)
                        .bind(&instance.kind)
                        .bind(&instance.state)
                        .bind(&instance.placement_mode)
                        .bind(&instance.region_key)
                        .bind(&feature_overrides_json)
                        .bind(&instance.instance_id)
                        .execute(scoped.pool())
                        .await?
                        .rows_affected()
                        > 0
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "UPDATE instances \
                         SET owner_org_id = @owner_org_id, kind = @kind, state = @state, \
                             placement_mode = @placement_mode, region_key = @region_key, \
                             feature_overrides = @feature_overrides, updated_at = CURRENT_TIMESTAMP() \
                         WHERE instance_id = @instance_id",
                    );
                    stmt.add_param("owner_org_id", &instance.owner_org_id);
                    stmt.add_param("kind", &instance.kind);
                    stmt.add_param("state", &instance.state);
                    stmt.add_param("placement_mode", &instance.placement_mode);
                    stmt.add_param("region_key", &instance.region_key);
                    stmt.add_param("feature_overrides", &feature_overrides_json);
                    stmt.add_param("instance_id", &instance.instance_id);
                    write_spanner_count(spanner, stmt).await? > 0
                }
            };
            if !updated {
                anyhow::bail!("instance not found");
            }
            if let Some(primary_domain) = &instance.primary_domain {
                let domain = DomainRecord {
                    domain: primary_domain.clone(),
                    is_primary: true,
                    state: "active".to_string(),
                    verified: false,
                    created_at: String::new(),
                    updated_at: String::new(),
                };
                upsert_domain(&db, &instance.instance_id, &domain).await?;
            }
            load_instance(&db, &instance.instance_id)
                .await?
                .context("updated instance but could not reload it")
        })
    }

    fn deprovision(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    sqlx::query(
                        "UPDATE instances SET state = $1, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $2",
                    )
                    .bind("deprovisioning")
                    .bind(&instance_id)
                    .execute(scoped.pool())
                    .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "UPDATE instances SET state = @state, updated_at = CURRENT_TIMESTAMP() \
                         WHERE instance_id = @instance_id",
                    );
                    stmt.add_param("state", &"deprovisioning");
                    stmt.add_param("instance_id", &instance_id);
                    let _ = write_spanner_count(spanner, stmt).await?;
                }
            }
            Ok(())
        })
    }

    fn resolve_domain(
        &self,
        domain: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<RouteResolution>>> {
        let db = self.db.clone();
        let domain = domain.to_string();
        Box::pin(async move {
            Ok(resolve_domain_route(&db, &domain)
                .await?
                .map(|row| RouteResolution {
                    instance_id: row.instance_id,
                    resolved_org_id: row.resolved_org_id,
                    placement_mode: row.placement_mode,
                    region_key: row.region_key,
                }))
        })
    }

    fn list_domains(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Vec<DomainRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            Ok(list_instance_domains(&db, &instance_id)
                .await?
                .into_iter()
                .map(domain_from_retained)
                .collect())
        })
    }

    fn set_domain(
        &self,
        instance_id: &str,
        domain: &DomainRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let domain = domain.clone();
        Box::pin(async move { upsert_domain(&db, &instance_id, &domain).await })
    }
}

impl ProviderRepository for SqlProviderRepository {
    fn create(
        &self,
        instance_id: &str,
        provider_record: &ProviderRecord,
    ) -> BoxFuture<'_, anyhow::Result<ProviderRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let provider_record = provider_record.clone();
        Box::pin(async move {
            let org_id = if let Some(org_id) = provider_org_id(&provider_record.config) {
                org_id
            } else {
                first_org_id(&db, &instance_id)
                    .await?
                    .unwrap_or_else(|| DEFAULT_ORG_ID.to_string())
            };
            let payload = provider_payload_from_record(&provider_record)?;
            provider::insert_provider_for(
                &db,
                &instance_id,
                &provider_record.id,
                &org_id,
                &payload,
            )
            .await?;
            load_provider(&db, &instance_id, &provider_record.id)
                .await?
                .context("created provider but could not reload it")
        })
    }

    fn get(
        &self,
        instance_id: &str,
        provider_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<ProviderRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let provider_id = provider_id.to_string();
        Box::pin(async move { load_provider(&db, &instance_id, &provider_id).await })
    }

    fn list(
        &self,
        instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<ProviderRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let params = params.clone();
        Box::pin(async move {
            let limit = limit_from_params(&params);
            let cursor = params.cursor.unwrap_or_default();
            let search = params.search.map(|value| value.to_lowercase());
            let mut items: Vec<ProviderRecord> = provider::list_providers_for(&db, &instance_id)
                .await?
                .into_iter()
                .filter(|item| item.id > cursor)
                .map(provider_from_storage)
                .collect::<anyhow::Result<_>>()?;
            if let Some(search) = search {
                items.retain(|item| {
                    item.name.to_lowercase().contains(&search)
                        || item.protocol.to_lowercase().contains(&search)
                });
            }
            items.truncate(limit as usize);
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
        provider_record: &ProviderRecord,
    ) -> BoxFuture<'_, anyhow::Result<ProviderRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let provider_record = provider_record.clone();
        Box::pin(async move {
            let payload = provider_payload_from_record(&provider_record)?;
            let updated =
                provider::update_provider_for(&db, &instance_id, &provider_record.id, &payload)
                    .await?;
            if !updated {
                anyhow::bail!("provider not found");
            }
            load_provider(&db, &instance_id, &provider_record.id)
                .await?
                .context("updated provider but could not reload it")
        })
    }

    fn delete(&self, instance_id: &str, provider_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let provider_id = provider_id.to_string();
        Box::pin(async move {
            delete_provider(&db, &instance_id, &provider_id).await?;
            Ok(())
        })
    }
}

impl SchemaRepository for SqlSchemaRepository {
    fn register(
        &self,
        _instance_id: &str,
        schema: &SchemaRecord,
    ) -> BoxFuture<'_, anyhow::Result<SchemaRecord>> {
        let db = self.db.clone();
        let schema = schema.clone();
        Box::pin(async move {
            let schema_json = json_string(&schema.schema_json)?;
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped_default();
                    let sql = format!(
                        "INSERT INTO schemas (id, type, schema, version, is_default, visibility) \
                         VALUES ($1, $2, {}, $3, $4, $5)",
                        scoped.json_bind(6),
                    );
                    sqlx::query(&sql)
                        .bind(&schema.id)
                        .bind(&schema.schema_type)
                        .bind(schema.version)
                        .bind(schema.is_default)
                        .bind(&schema.visibility)
                        .bind(&schema_json)
                        .execute(scoped.pool())
                        .await?;
                }
                Db::Spanner(spanner) => {
                    let mut stmt = Statement::new(
                        "INSERT INTO schemas (id, type, schema, version, is_default, visibility) \
                         VALUES (@id, @type, @schema, @version, @is_default, @visibility)",
                    );
                    stmt.add_param("id", &schema.id);
                    stmt.add_param("type", &schema.schema_type);
                    stmt.add_param("schema", &schema_json);
                    stmt.add_param("version", &schema.version);
                    stmt.add_param("is_default", &schema.is_default);
                    stmt.add_param("visibility", &schema.visibility);
                    write_spanner_stmt(spanner, stmt).await?;
                }
            }
            load_schema(&db, &schema.id)
                .await?
                .context("created schema but could not reload it")
        })
    }

    fn get(
        &self,
        _instance_id: &str,
        schema_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SchemaRecord>>> {
        let db = self.db.clone();
        let schema_id = schema_id.to_string();
        Box::pin(async move { load_schema(&db, &schema_id).await })
    }

    fn get_by_type(
        &self,
        _instance_id: &str,
        schema_type: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SchemaRecord>>> {
        let db = self.db.clone();
        let schema_type = schema_type.to_string();
        Box::pin(async move {
            let items = list_schema_registry(&db, "", Some(&schema_type), 1).await?;
            Ok(items.into_iter().next().map(schema_from_retained))
        })
    }

    fn list(
        &self,
        _instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<SchemaRecord>>> {
        let db = self.db.clone();
        let params = params.clone();
        Box::pin(async move {
            let limit = limit_from_params(&params);
            let cursor = params.cursor.unwrap_or_default();
            let search = params.search.map(|value| value.to_lowercase());
            let mut items: Vec<SchemaRecord> = list_schema_registry(&db, &cursor, None, limit)
                .await?
                .into_iter()
                .map(schema_from_retained)
                .collect();
            if let Some(search) = search {
                items.retain(|item| {
                    item.schema_type.to_lowercase().contains(&search)
                        || item.id.to_lowercase().contains(&search)
                });
            }
            items.truncate(limit as usize);
            Ok(ListResult {
                next_cursor: next_cursor(&items, limit, |item| item.id.as_str()),
                total_count: None,
                items,
            })
        })
    }

    fn update(
        &self,
        _instance_id: &str,
        schema: &SchemaRecord,
    ) -> BoxFuture<'_, anyhow::Result<SchemaRecord>> {
        let db = self.db.clone();
        let schema = schema.clone();
        Box::pin(async move {
            let schema_json = json_string(&schema.schema_json)?;
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped_default();
                    let sql = format!(
                        "UPDATE schemas SET type = $1, schema = {}, version = $2, is_default = $3, visibility = $4 WHERE id = $5",
                        scoped.json_bind(6),
                    );
                    sqlx::query(&sql)
                        .bind(&schema.schema_type)
                        .bind(schema.version)
                        .bind(schema.is_default)
                        .bind(&schema.visibility)
                        .bind(&schema.id)
                        .bind(&schema_json)
                        .execute(scoped.pool())
                        .await?;
                    if schema.is_default {
                        sqlx::query(
                            "UPDATE schemas SET is_default = FALSE WHERE type = $1 AND id != $2",
                        )
                        .bind(&schema.schema_type)
                        .bind(&schema.id)
                        .execute(scoped.pool())
                        .await?;
                    }
                }
                Db::Spanner(spanner) => {
                    let mut stmts = vec![{
                        let mut stmt = Statement::new(
                            "UPDATE schemas SET type = @type, schema = @schema, version = @version, \
                                 is_default = @is_default, visibility = @visibility WHERE id = @id",
                        );
                        stmt.add_param("type", &schema.schema_type);
                        stmt.add_param("schema", &schema_json);
                        stmt.add_param("version", &schema.version);
                        stmt.add_param("is_default", &schema.is_default);
                        stmt.add_param("visibility", &schema.visibility);
                        stmt.add_param("id", &schema.id);
                        stmt
                    }];
                    if schema.is_default {
                        let mut stmt = Statement::new(
                            "UPDATE schemas SET is_default = FALSE WHERE type = @type AND id != @id",
                        );
                        stmt.add_param("type", &schema.schema_type);
                        stmt.add_param("id", &schema.id);
                        stmts.push(stmt);
                    }
                    write_spanner_many(spanner, stmts).await?;
                }
            }
            load_schema(&db, &schema.id)
                .await?
                .context("updated schema but could not reload it")
        })
    }
}

impl SettingsRepository for SqlSettingsRepository {
    fn get(
        &self,
        instance_id: &str,
        settings_type: &str,
        scope: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SettingsRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let settings_type = settings_type.to_string();
        let scope = scope.to_string();
        Box::pin(async move {
            let (scope_kind, scope_id) = parse_scope(&scope);
            load_settings_exact(&db, &instance_id, &settings_type, &scope_kind, &scope_id).await
        })
    }

    fn set(
        &self,
        instance_id: &str,
        settings: &SettingsRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let settings = settings.clone();
        Box::pin(async move {
            let (scope, scope_id) = parse_scope(&settings.scope);
            let data_json = json_string(&settings.data)?;
            let id = uuid::Uuid::now_v7().to_string();
            match &db {
                Db::Sql(_) => {
                    let scoped = db.scoped(instance_id.clone());
                    let sql = format!(
                        "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) \
                         VALUES ($1, $2, $3, $4, $5, {}) \
                         ON CONFLICT(instance_id, type, scope, scope_id) DO UPDATE SET \
                            data = excluded.data, updated_at = CURRENT_TIMESTAMP",
                        scoped.json_bind(6),
                    );
                    sqlx::query(&sql)
                        .bind(&id)
                        .bind(&instance_id)
                        .bind(&settings.settings_type)
                        .bind(&scope)
                        .bind(&scope_id)
                        .bind(&data_json)
                        .execute(scoped.pool())
                        .await?;
                }
                Db::Spanner(spanner) => {
                    let existing = load_settings_exact(
                        &db,
                        &instance_id,
                        &settings.settings_type,
                        &scope,
                        &scope_id,
                    )
                    .await?;
                    if existing.is_some() {
                        let mut stmt = Statement::new(
                            "UPDATE settings SET data = @data, updated_at = CURRENT_TIMESTAMP() \
                             WHERE instance_id = @instance_id AND type = @type AND scope = @scope AND scope_id = @scope_id",
                        );
                        stmt.add_param("data", &data_json);
                        stmt.add_param("instance_id", &instance_id);
                        stmt.add_param("type", &settings.settings_type);
                        stmt.add_param("scope", &scope);
                        stmt.add_param("scope_id", &scope_id);
                        let _ = write_spanner_count(spanner, stmt).await?;
                    } else {
                        let mut stmt = Statement::new(
                            "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) \
                             VALUES (@id, @instance_id, @type, @scope, @scope_id, @data)",
                        );
                        stmt.add_param("id", &id);
                        stmt.add_param("instance_id", &instance_id);
                        stmt.add_param("type", &settings.settings_type);
                        stmt.add_param("scope", &scope);
                        stmt.add_param("scope_id", &scope_id);
                        stmt.add_param("data", &data_json);
                        write_spanner_stmt(spanner, stmt).await?;
                    }
                }
            }
            Ok(())
        })
    }

    fn resolve(
        &self,
        instance_id: &str,
        settings_type: &str,
        org_id: Option<&str>,
        app_id: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<SettingsRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let settings_type = settings_type.to_string();
        let org_id = org_id.map(str::to_string);
        let app_id = app_id.map(str::to_string);
        Box::pin(async move {
            if let Some(app_id) = app_id.as_deref() {
                if let Some(record) =
                    load_settings_exact(&db, &instance_id, &settings_type, "app", app_id).await?
                {
                    return Ok(record);
                }
            }
            if let Some(org_id) = org_id.as_deref() {
                if let Some(record) =
                    load_settings_exact(&db, &instance_id, &settings_type, "org", org_id).await?
                {
                    return Ok(record);
                }
            }
            if let Some(record) =
                load_settings_exact(&db, &instance_id, &settings_type, "instance", "").await?
            {
                return Ok(record);
            }
            Ok(SettingsRecord {
                settings_type,
                scope: "instance".to_string(),
                data: Value::Object(Map::new()),
            })
        })
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
        let instance_id = instance_id.to_string();
        let query = query.to_string();
        let allowed = normalized_resource_types(resource_types);
        let limit = limit.map(i64::from).unwrap_or(20).clamp(1, 100);
        Box::pin(async move {
            let pattern = format!("%{query}%");
            let query_lc = query.to_lowercase();
            let mut results = Vec::new();

            if allowed.is_empty() || allowed.contains("user") {
                match &db {
                    Db::Sql(_) => {
                        let scoped = db.scoped(instance_id.clone());
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
                        let scoped = db.scoped(instance_id.clone());
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
                        let scoped = db.scoped(instance_id.clone());
                        let rows: Vec<(String, String, String)> = sqlx::query_as(
                            "SELECT id, org_id, name FROM groups \
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
                        let mut stmt = Statement::new(
                            "SELECT id, org_id, name FROM groups \
                             WHERE instance_id = @instance_id AND name LIKE @pattern ORDER BY name, id LIMIT @limit",
                        );
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
                for provider in provider::list_providers_for(&db, &instance_id).await? {
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
        })
    }
}

type UserSqlRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);
type OrgSqlRow = (String, String, String, String, String, String);
type GroupSqlRow = (String, String, String, String, String, String, String);

fn user_from_sql_row(row: UserSqlRow) -> UserRecord {
    UserRecord {
        id: row.0,
        org_id: row.1,
        identifier: row.2,
        display_name: row.3,
        user_type: row.4,
        state: row.5,
        schema_id: row.6,
        metadata: json_value(&row.7),
        created_at: row.8,
        updated_at: row.9,
    }
}

fn user_from_spanner_row(row: SpannerRow) -> UserRecord {
    UserRecord {
        id: row.column_by_name::<String>("id").unwrap_or_default(),
        org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
        identifier: row
            .column_by_name::<String>("identifier")
            .unwrap_or_default(),
        display_name: row
            .column_by_name::<String>("display_name")
            .unwrap_or_default(),
        user_type: row
            .column_by_name::<String>("user_type")
            .unwrap_or_default(),
        state: row.column_by_name::<String>("state").unwrap_or_default(),
        schema_id: row
            .column_by_name::<String>("schema_id")
            .unwrap_or_default(),
        metadata: json_value(
            &row.column_by_name::<String>("metadata")
                .unwrap_or_else(|_| "{}".to_string()),
        ),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
        updated_at: row
            .column_by_name::<String>("updated_at")
            .unwrap_or_default(),
    }
}

fn org_from_sql_row(row: OrgSqlRow) -> OrgRecord {
    OrgRecord {
        id: row.0,
        name: row.1,
        state: row.2,
        metadata: json_value(&row.3),
        created_at: row.4,
        updated_at: row.5,
    }
}

fn org_from_spanner_row(row: SpannerRow) -> OrgRecord {
    OrgRecord {
        id: row.column_by_name::<String>("id").unwrap_or_default(),
        name: row.column_by_name::<String>("name").unwrap_or_default(),
        state: row.column_by_name::<String>("state").unwrap_or_default(),
        metadata: json_value(
            &row.column_by_name::<String>("metadata")
                .unwrap_or_else(|_| "{}".to_string()),
        ),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
        updated_at: row
            .column_by_name::<String>("updated_at")
            .unwrap_or_default(),
    }
}

fn group_from_sql_row(row: GroupSqlRow) -> GroupRecord {
    GroupRecord {
        id: row.0,
        org_id: row.1,
        name: row.2,
        state: row.3,
        metadata: json_value(&row.4),
        created_at: row.5,
        updated_at: row.6,
    }
}

fn group_from_spanner_row(row: SpannerRow) -> GroupRecord {
    GroupRecord {
        id: row.column_by_name::<String>("id").unwrap_or_default(),
        org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
        name: row.column_by_name::<String>("name").unwrap_or_default(),
        state: row.column_by_name::<String>("state").unwrap_or_default(),
        metadata: json_value(
            &row.column_by_name::<String>("metadata")
                .unwrap_or_else(|_| "{}".to_string()),
        ),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
        updated_at: row
            .column_by_name::<String>("updated_at")
            .unwrap_or_default(),
    }
}

fn instance_from_retained(record: crate::ManagedInstanceRecord) -> InstanceRecord {
    InstanceRecord {
        instance_id: record.instance_id,
        state: record.state,
        kind: record.kind,
        placement_mode: record.placement_mode,
        region_key: record.region_key,
        owner_org_id: record.owner_org_id,
        feature_overrides: json_value(&record.feature_overrides_json),
        primary_domain: record.primary_domain,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn domain_from_retained(record: crate::DomainRecord) -> DomainRecord {
    DomainRecord {
        domain: record.domain,
        is_primary: record.is_primary,
        state: record.state,
        verified: record.verified,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn schema_from_retained(record: crate::SchemaRegistryRecord) -> SchemaRecord {
    SchemaRecord {
        id: record.id,
        schema_type: record.type_,
        schema_json: json_value(&record.schema_json),
        version: record.version,
        is_default: record.is_default,
        visibility: record.visibility,
        created_at: record.created_at,
    }
}

fn provider_from_storage(record: provider::ProviderRecord) -> anyhow::Result<ProviderRecord> {
    let mut config = serde_json::to_value(&record.payload)?;
    if let Value::Object(map) = &mut config {
        map.remove("display_name");
        map.remove("protocol");
        map.insert("org_id".to_string(), Value::String(record.org_id));
    }
    Ok(ProviderRecord {
        id: record.id,
        name: record.payload.display_name,
        protocol: record.payload.protocol,
        state: if record.payload.enabled {
            "active".to_string()
        } else {
            "inactive".to_string()
        },
        config,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

fn provider_payload_from_record(
    record: &ProviderRecord,
) -> anyhow::Result<provider::ProviderPayload> {
    let mut payload = if has_provider_payload_shape(&record.config) {
        serde_json::from_value::<provider::ProviderPayload>(record.config.clone())
            .unwrap_or_default()
    } else {
        let connection =
            serde_json::from_value::<provider::ProviderConnection>(record.config.clone())
                .unwrap_or_default();
        provider::ProviderPayload {
            connection,
            ..provider::ProviderPayload::default()
        }
    };
    payload.display_name = record.name.clone();
    payload.protocol = record.protocol.clone();
    payload.enabled = record.state == "active";
    Ok(payload)
}

fn has_provider_payload_shape(config: &Value) -> bool {
    config
        .as_object()
        .is_some_and(|map| map.contains_key("connection") || map.contains_key("mapping"))
}

fn provider_org_id(config: &Value) -> Option<String> {
    config
        .get("org_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn parse_scope(scope: &str) -> (String, String) {
    if let Some((scope_kind, scope_id)) = scope.split_once(':') {
        (scope_kind.to_string(), scope_id.to_string())
    } else {
        (scope.to_string(), String::new())
    }
}

fn format_scope(scope: &str, scope_id: &str) -> String {
    if scope_id.is_empty() {
        scope.to_string()
    } else {
        format!("{scope}:{scope_id}")
    }
}

fn limit_from_params(params: &ListParams) -> i64 {
    i64::from(params.limit.unwrap_or(DEFAULT_LIST_LIMIT as u32)).clamp(1, MAX_LIST_LIMIT)
}

fn json_string(value: &Value) -> anyhow::Result<String> {
    serde_json::to_string(value).context("serialize json")
}

fn json_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn next_cursor<T, F>(items: &[T], limit: i64, f: F) -> Option<String>
where
    F: Fn(&T) -> &str,
{
    if items.len() < limit as usize {
        None
    } else {
        items.last().map(|item| f(item).to_string())
    }
}

fn normalized_resource_types(resource_types: Option<&[&str]>) -> HashSet<String> {
    resource_types
        .into_iter()
        .flatten()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

async fn load_user(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Option<UserRecord>> {
    Ok(get_user(db, instance_id, user_id)
        .await?
        .map(|row| UserRecord {
            id: row.id,
            org_id: row.org_id,
            identifier: row.identifier,
            display_name: row.display_name,
            user_type: row.user_type,
            state: row.state,
            schema_id: row.schema_id,
            metadata: json_value(&row.metadata_json),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
}

async fn load_org(db: &Db, instance_id: &str, org_id: &str) -> anyhow::Result<Option<OrgRecord>> {
    Ok(get_org(db, instance_id, org_id)
        .await?
        .map(|row| OrgRecord {
            id: row.id,
            name: row.name,
            state: row.state,
            metadata: json_value(&row.metadata_json),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
}

async fn load_group(
    db: &Db,
    instance_id: &str,
    group_id: &str,
) -> anyhow::Result<Option<GroupRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, org_id, name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                 FROM groups WHERE instance_id = $1 AND id = $2"
            );
            Ok(sqlx::query_as::<_, GroupSqlRow>(&sql)
                .bind(instance_id)
                .bind(group_id)
                .fetch_optional(scoped.pool())
                .await?
                .map(group_from_sql_row))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, org_id, name, state, IFNULL(metadata, '{}') AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM groups WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &group_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(group_from_spanner_row))
        }
    }
}

async fn load_instance(db: &Db, instance_id: &str) -> anyhow::Result<Option<InstanceRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let feature_overrides = scoped.as_text("i.feature_overrides");
            let created_at = scoped.as_text("i.created_at");
            let updated_at = scoped.as_text("i.updated_at");
            let sql = format!(
                "SELECT i.instance_id, i.state, i.kind, i.placement_mode, NULLIF(i.region_key, ''), i.owner_org_id, \
                        COALESCE({feature_overrides}, '{{}}'), d.domain, {created_at}, {updated_at} \
                 FROM instances i \
                 LEFT JOIN domains d ON d.instance_id = i.instance_id AND d.org_id IS NULL AND d.is_primary = TRUE \
                 WHERE i.instance_id = $1 LIMIT 1"
            );
            let row: Option<(
                String,
                String,
                String,
                String,
                Option<String>,
                String,
                String,
                Option<String>,
                String,
                String,
            )> = sqlx::query_as(&sql)
                .bind(instance_id)
                .fetch_optional(scoped.pool())
                .await?;
            Ok(row.map(|row| InstanceRecord {
                instance_id: row.0,
                state: row.1,
                kind: row.2,
                placement_mode: row.3,
                region_key: row.4,
                owner_org_id: row.5,
                feature_overrides: json_value(&row.6),
                primary_domain: row.7,
                created_at: row.8,
                updated_at: row.9,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT i.instance_id, i.state, i.kind, i.placement_mode, i.region_key, i.owner_org_id, \
                        IFNULL(i.feature_overrides, '{}') AS feature_overrides, d.domain AS primary_domain, \
                        CAST(i.created_at AS STRING) AS created_at, CAST(i.updated_at AS STRING) AS updated_at \
                 FROM instances i \
                 LEFT JOIN domains d ON d.instance_id = i.instance_id AND d.org_id IS NULL AND d.is_primary = TRUE \
                 WHERE i.instance_id = @instance_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| InstanceRecord {
                    instance_id: row
                        .column_by_name::<String>("instance_id")
                        .unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    kind: row.column_by_name::<String>("kind").unwrap_or_default(),
                    placement_mode: row
                        .column_by_name::<String>("placement_mode")
                        .unwrap_or_default(),
                    region_key: row
                        .column_by_name::<Option<String>>("region_key")
                        .unwrap_or(None)
                        .filter(|value| !value.is_empty()),
                    owner_org_id: row
                        .column_by_name::<String>("owner_org_id")
                        .unwrap_or_default(),
                    feature_overrides: json_value(
                        &row.column_by_name::<String>("feature_overrides")
                            .unwrap_or_else(|_| "{}".to_string()),
                    ),
                    primary_domain: row
                        .column_by_name::<Option<String>>("primary_domain")
                        .unwrap_or(None),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                    updated_at: row
                        .column_by_name::<String>("updated_at")
                        .unwrap_or_default(),
                }))
        }
    }
}

async fn load_provider(
    db: &Db,
    instance_id: &str,
    provider_id: &str,
) -> anyhow::Result<Option<ProviderRecord>> {
    provider::get_provider_for(db, instance_id, provider_id)
        .await?
        .map(provider_from_storage)
        .transpose()
}

async fn load_schema(db: &Db, schema_id: &str) -> anyhow::Result<Option<SchemaRecord>> {
    Ok(get_schema_record(db, schema_id)
        .await?
        .map(schema_from_retained))
}

async fn load_settings_exact(
    db: &Db,
    instance_id: &str,
    settings_type: &str,
    scope: &str,
    scope_id: &str,
) -> anyhow::Result<Option<SettingsRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "SELECT type, scope, scope_id, {} FROM settings \
                 WHERE instance_id = $1 AND type = $2 AND scope = $3 AND scope_id = $4 LIMIT 1",
                scoped.as_text("data"),
            );
            let row: Option<(String, String, String, String)> = sqlx::query_as(&sql)
                .bind(instance_id)
                .bind(settings_type)
                .bind(scope)
                .bind(scope_id)
                .fetch_optional(scoped.pool())
                .await?;
            Ok(row.map(|row| SettingsRecord {
                settings_type: row.0,
                scope: format_scope(&row.1, &row.2),
                data: json_value(&row.3),
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT type, scope, scope_id, IFNULL(data, '{}') AS data FROM settings \
                 WHERE instance_id = @instance_id AND type = @type AND scope = @scope AND scope_id = @scope_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("type", &settings_type);
            stmt.add_param("scope", &scope);
            stmt.add_param("scope_id", &scope_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| SettingsRecord {
                    settings_type: row.column_by_name::<String>("type").unwrap_or_default(),
                    scope: format_scope(
                        &row.column_by_name::<String>("scope").unwrap_or_default(),
                        &row.column_by_name::<String>("scope_id").unwrap_or_default(),
                    ),
                    data: json_value(
                        &row.column_by_name::<String>("data")
                            .unwrap_or_else(|_| "{}".to_string()),
                    ),
                }))
        }
    }
}

async fn upsert_domain(db: &Db, instance_id: &str, domain: &DomainRecord) -> anyhow::Result<()> {
    if domain.is_primary {
        let existing = list_instance_domains(db, instance_id).await?;
        for item in existing {
            if item.is_primary && item.domain != domain.domain {
                match db {
                    Db::Sql(_) => {
                        let scoped = db.scoped(instance_id.to_string());
                        sqlx::query(
                            "UPDATE domains SET is_primary = FALSE, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND domain = $2",
                        )
                        .bind(instance_id)
                        .bind(&item.domain)
                        .execute(scoped.pool())
                        .await?;
                    }
                    Db::Spanner(spanner) => {
                        let mut stmt = Statement::new(
                            "UPDATE domains SET is_primary = FALSE, updated_at = CURRENT_TIMESTAMP() \
                             WHERE instance_id = @instance_id AND domain = @domain",
                        );
                        stmt.add_param("instance_id", &instance_id);
                        stmt.add_param("domain", &item.domain);
                        let _ = write_spanner_count(spanner, stmt).await?;
                    }
                }
            }
        }
    }

    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT(domain) DO UPDATE SET \
                     instance_id = excluded.instance_id, \
                     is_primary = excluded.is_primary, \
                     state = excluded.state, \
                     verified = excluded.verified, \
                     updated_at = CURRENT_TIMESTAMP",
            )
            .bind(&domain.domain)
            .bind(instance_id)
            .bind(domain.is_primary)
            .bind(&domain.state)
            .bind(domain.verified)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mut exists =
                Statement::new("SELECT domain FROM domains WHERE domain = @domain LIMIT 1");
            exists.add_param("domain", &domain.domain);
            if spanner_query_optional(spanner, exists).await?.is_some() {
                let mut stmt = Statement::new(
                    "UPDATE domains SET instance_id = @instance_id, is_primary = @is_primary, \
                         state = @state, verified = @verified, updated_at = CURRENT_TIMESTAMP() \
                     WHERE domain = @domain",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("is_primary", &domain.is_primary);
                stmt.add_param("state", &domain.state);
                stmt.add_param("verified", &domain.verified);
                stmt.add_param("domain", &domain.domain);
                let _ = write_spanner_count(spanner, stmt).await?;
            } else if !domain.is_primary && domain.state == "active" && !domain.verified {
                let _ = add_instance_domain(db, instance_id, &domain.domain).await?;
            } else {
                let mut stmt = Statement::new(
                    "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                     VALUES (@domain, @instance_id, @is_primary, @state, @verified)",
                );
                stmt.add_param("domain", &domain.domain);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("is_primary", &domain.is_primary);
                stmt.add_param("state", &domain.state);
                stmt.add_param("verified", &domain.verified);
                write_spanner_stmt(spanner, stmt).await?;
            }
        }
    }
    Ok(())
}

async fn write_spanner_stmt(spanner: &SpannerDb, stmt: Statement) -> anyhow::Result<()> {
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
    Ok(())
}

async fn write_spanner_count(spanner: &SpannerDb, stmt: Statement) -> anyhow::Result<u64> {
    let (_, affected) = spanner
        .client()
        .read_write_transaction(|tx| {
            let stmt = stmt.clone();
            Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
        })
        .await?;
    Ok(affected as u64)
}

async fn write_spanner_many(spanner: &SpannerDb, stmts: Vec<Statement>) -> anyhow::Result<()> {
    let _ = spanner
        .client()
        .read_write_transaction(|tx| {
            let stmts = stmts.clone();
            Box::pin(async move {
                for stmt in stmts {
                    tx.update(stmt).await?;
                }
                Ok::<(), SpannerError>(())
            })
        })
        .await?;
    Ok(())
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

async fn spanner_query_optional(
    spanner: &SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Option<SpannerRow>> {
    let mut tx = spanner.client().single().await?;
    let mut rows = tx.query(stmt).await?;
    Ok(rows.next().await?)
}
