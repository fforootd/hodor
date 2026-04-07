use crate::{
    Db, create_named_resource, delete_instance_row, list_named_resources,
    update_named_resource_name,
};
use zitadel_app::repo::{
    AppRecord, AppRepository, BoxFuture, ListParams, ListResult, NamedResourceRecord,
    ProjectRepository,
};

#[derive(Clone)]
pub struct DbAppRepository {
    db: Db,
}

impl DbAppRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbProjectRepository {
    db: Db,
}

impl DbProjectRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl AppRepository for DbAppRepository {
    fn create(
        &self,
        instance_id: &str,
        app: &AppRecord,
    ) -> BoxFuture<'_, anyhow::Result<AppRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let app = app.clone();
        Box::pin(async move {
            let record =
                create_named_resource(&db, &instance_id, "apps", &app.id, &app.name, &app.group_id)
                    .await?;
            Ok(AppRecord {
                id: record.id,
                group_id: app.group_id,
                name: record.name,
                protocol: app.protocol,
                state: record.state,
                metadata: app.metadata,
                created_at: record.created_at,
                updated_at: record.updated_at,
            })
        })
    }

    fn get(
        &self,
        instance_id: &str,
        app_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<AppRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let app_id = app_id.to_string();
        Box::pin(async move {
            Ok(
                crate::get_named_resource(&db, &instance_id, "apps", &app_id)
                    .await?
                    .map(|record| AppRecord {
                        id: record.id,
                        group_id: String::new(),
                        name: record.name,
                        protocol: String::new(),
                        state: record.state,
                        metadata: serde_json::Value::Object(Default::default()),
                        created_at: record.created_at,
                        updated_at: record.updated_at,
                    }),
            )
        })
    }

    fn list(
        &self,
        instance_id: &str,
        _group_id: Option<&str>,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<AppRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let params = params.clone();
        Box::pin(async move {
            let limit = params.limit.unwrap_or(50).max(1) as i64;
            let cursor = params.cursor.unwrap_or_default();
            let rows = list_named_resources(&db, &instance_id, "apps", &cursor, limit + 1).await?;
            let has_more = rows.len() as i64 > limit;
            let items: Vec<AppRecord> = rows
                .into_iter()
                .take(limit as usize)
                .map(|record| AppRecord {
                    id: record.id,
                    group_id: String::new(),
                    name: record.name,
                    protocol: String::new(),
                    state: record.state,
                    metadata: serde_json::Value::Object(Default::default()),
                    created_at: record.created_at,
                    updated_at: record.updated_at,
                })
                .collect();
            Ok(ListResult {
                next_cursor: if has_more {
                    items.last().map(|item| item.id.clone())
                } else {
                    None
                },
                total_count: None,
                items,
            })
        })
    }

    fn update_name(
        &self,
        instance_id: &str,
        app_id: &str,
        name: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let app_id = app_id.to_string();
        let name = name.to_string();
        Box::pin(async move {
            update_named_resource_name(&db, &instance_id, "apps", &app_id, &name).await
        })
    }

    fn delete(&self, instance_id: &str, app_id: &str) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let app_id = app_id.to_string();
        Box::pin(async move { delete_instance_row(&db, &instance_id, "apps", &app_id).await })
    }
}

impl ProjectRepository for DbProjectRepository {
    fn create(
        &self,
        instance_id: &str,
        project: &NamedResourceRecord,
        org_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<NamedResourceRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let project = project.clone();
        let org_id = org_id.to_string();
        Box::pin(async move {
            let record = create_named_resource(
                &db,
                &instance_id,
                "projects",
                &project.id,
                &project.name,
                &org_id,
            )
            .await?;
            Ok(NamedResourceRecord {
                id: record.id,
                name: record.name,
                state: record.state,
                created_at: record.created_at,
                updated_at: record.updated_at,
            })
        })
    }

    fn get(
        &self,
        instance_id: &str,
        project_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<NamedResourceRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let project_id = project_id.to_string();
        Box::pin(async move {
            Ok(
                crate::get_named_resource(&db, &instance_id, "projects", &project_id)
                    .await?
                    .map(|record| NamedResourceRecord {
                        id: record.id,
                        name: record.name,
                        state: record.state,
                        created_at: record.created_at,
                        updated_at: record.updated_at,
                    }),
            )
        })
    }

    fn list(
        &self,
        instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<NamedResourceRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let params = params.clone();
        Box::pin(async move {
            let limit = params.limit.unwrap_or(50).max(1) as i64;
            let cursor = params.cursor.unwrap_or_default();
            let rows =
                list_named_resources(&db, &instance_id, "projects", &cursor, limit + 1).await?;
            let has_more = rows.len() as i64 > limit;
            let items: Vec<NamedResourceRecord> = rows
                .into_iter()
                .take(limit as usize)
                .map(|record| NamedResourceRecord {
                    id: record.id,
                    name: record.name,
                    state: record.state,
                    created_at: record.created_at,
                    updated_at: record.updated_at,
                })
                .collect();
            Ok(ListResult {
                next_cursor: if has_more {
                    items.last().map(|item| item.id.clone())
                } else {
                    None
                },
                total_count: None,
                items,
            })
        })
    }

    fn update_name(
        &self,
        instance_id: &str,
        project_id: &str,
        name: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let project_id = project_id.to_string();
        let name = name.to_string();
        Box::pin(async move {
            update_named_resource_name(&db, &instance_id, "projects", &project_id, &name).await
        })
    }

    fn delete(&self, instance_id: &str, project_id: &str) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let project_id = project_id.to_string();
        Box::pin(
            async move { delete_instance_row(&db, &instance_id, "projects", &project_id).await },
        )
    }
}
