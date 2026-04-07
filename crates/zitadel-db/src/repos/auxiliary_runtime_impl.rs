use crate::{
    Db, add_membership, create_saved_query, delete_saved_query, list_fingerprints,
    list_jobs_for_instance, list_memberships, list_saved_queries, load_console_bootstrap_data,
    load_entity_counts, remove_membership, upsert_fingerprint,
};
use zitadel_app::repo::{
    BoxFuture, ConsoleBootstrapData, ConsoleQueryRepository, FingerprintRecord, InstanceInfo,
    JobRecord, JobRepository, MembershipRecord, MembershipRepository, OrgSummary, SavedQueryRecord,
    SavedQueryRepository, TelemetryRepository,
};

#[derive(Clone)]
pub struct DbMembershipRepository {
    db: Db,
}

impl DbMembershipRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbConsoleQueryRepository {
    db: Db,
}

impl DbConsoleQueryRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbTelemetryRepository {
    db: Db,
}

impl DbTelemetryRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbJobRepository {
    db: Db,
}

impl DbJobRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbSavedQueryRepository {
    db: Db,
}

impl DbSavedQueryRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl MembershipRepository for DbMembershipRepository {
    fn list(
        &self,
        instance_id: &str,
        entity_type: &str,
        entity_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<MembershipRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let entity_type = entity_type.to_string();
        let entity_id = entity_id.to_string();
        Box::pin(async move {
            Ok(
                list_memberships(&db, &instance_id, &entity_type, &entity_id)
                    .await?
                    .into_iter()
                    .map(|row| MembershipRecord {
                        user_id: row.user_id,
                        display_name: row.display_name.filter(|value| !value.is_empty()),
                        role: row.role,
                        added_at: row.added_at,
                    })
                    .collect(),
            )
        })
    }

    fn add(
        &self,
        instance_id: &str,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        role: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let entity_type = entity_type.to_string();
        let entity_id = entity_id.to_string();
        let user_id = user_id.to_string();
        let role = role.to_string();
        Box::pin(async move {
            add_membership(&db, &instance_id, &entity_type, &entity_id, &user_id, &role).await
        })
    }

    fn remove(
        &self,
        instance_id: &str,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let entity_type = entity_type.to_string();
        let entity_id = entity_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move {
            remove_membership(&db, &instance_id, &entity_type, &entity_id, &user_id).await
        })
    }
}

impl ConsoleQueryRepository for DbConsoleQueryRepository {
    fn load_console_bootstrap(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<ConsoleBootstrapData>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            let data = load_console_bootstrap_data(&db, &instance_id).await?;
            Ok(ConsoleBootstrapData {
                counts: data.counts.into_iter().collect(),
                orgs: data
                    .orgs
                    .into_iter()
                    .map(|org| OrgSummary {
                        id: org.id,
                        name: org.name,
                        state: org.state,
                    })
                    .collect(),
                instance: InstanceInfo {
                    instance_id: data.instance.instance_id,
                    kind: data.instance.kind,
                    feature_overrides_json: data.instance.feature_overrides_json,
                    parent_instance_id: data.instance.parent_instance_id,
                },
            })
        })
    }

    fn load_entity_counts(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<(String, i64)>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            Ok(load_entity_counts(&db, &instance_id)
                .await?
                .into_iter()
                .collect())
        })
    }
}

impl TelemetryRepository for DbTelemetryRepository {
    fn list_fingerprints(
        &self,
        instance_id: &str,
        cursor: &str,
        limit: i64,
    ) -> BoxFuture<'_, anyhow::Result<Vec<FingerprintRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let cursor = cursor.to_string();
        Box::pin(async move {
            Ok(list_fingerprints(&db, &instance_id, &cursor, limit)
                .await?
                .into_iter()
                .map(|record| FingerprintRecord {
                    id: record.id,
                    type_: record.type_,
                    raw_data_json: record.raw_data_json,
                    created_at: record.created_at,
                })
                .collect())
        })
    }

    fn upsert_fingerprint(
        &self,
        instance_id: &str,
        id: &str,
        type_: &str,
        raw_data: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let id = id.to_string();
        let type_ = type_.to_string();
        let raw_data = raw_data.to_string();
        Box::pin(async move {
            upsert_fingerprint(&db, &instance_id, &id, &type_, &raw_data)
                .await
                .map(|_| ())
        })
    }
}

impl JobRepository for DbJobRepository {
    fn list_jobs(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Vec<JobRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            Ok(list_jobs_for_instance(&db, &instance_id)
                .await?
                .into_iter()
                .map(|row| JobRecord {
                    name: row.name,
                    display_name: row.display_name,
                    description: row.description,
                    cron: row.cron,
                    enabled: row.enabled,
                    last_status: row.last_status,
                    last_error: row.last_error,
                    run_count: row.run_count,
                    last_rows_removed: row.last_rows_removed,
                    last_run_at: row.last_run_at,
                    next_run_at: row.next_run_at,
                    lease_expires_at: row.lease_expires_at,
                    config_json: row.config_json,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                })
                .collect())
        })
    }
}

impl SavedQueryRepository for DbSavedQueryRepository {
    fn list_saved_queries(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SavedQueryRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            Ok(list_saved_queries(&db, &instance_id)
                .await?
                .into_iter()
                .map(|row| SavedQueryRecord {
                    id: row.id,
                    name: row.name,
                    description: row.description,
                    sql: row.sql,
                    created_at: row.created_at,
                })
                .collect())
        })
    }

    fn create_saved_query(
        &self,
        instance_id: &str,
        id: &str,
        name: &str,
        description: &str,
        sql: &str,
    ) -> BoxFuture<'_, anyhow::Result<SavedQueryRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let id = id.to_string();
        let name = name.to_string();
        let description = description.to_string();
        let sql = sql.to_string();
        Box::pin(async move {
            let row = create_saved_query(&db, &instance_id, &id, &name, &description, &sql).await?;
            Ok(SavedQueryRecord {
                id: row.id,
                name: row.name,
                description: row.description,
                sql: row.sql,
                created_at: row.created_at,
            })
        })
    }

    fn delete_saved_query(
        &self,
        instance_id: &str,
        id: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let id = id.to_string();
        Box::pin(async move { delete_saved_query(&db, &instance_id, &id).await })
    }
}
