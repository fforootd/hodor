use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{AppRecord, NamedResourceRecord, Repositories};
use std::sync::Arc;

fn app_to_named_resource(app: AppRecord) -> NamedResourceRecord {
    NamedResourceRecord {
        id: app.id,
        name: app.name,
        state: app.state,
        created_at: app.created_at,
        updated_at: app.updated_at,
    }
}

pub struct CreateNamedResource {
    repos: Arc<Repositories>,
}

pub struct CreateNamedResourceCommand {
    pub kind: String,
    pub name: String,
    pub org_id: String,
}

impl CreateNamedResource {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.create_named_resource", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateNamedResourceCommand,
    ) -> Result<NamedResourceRecord, AppError> {
        let id = uuid::Uuid::now_v7().to_string();

        let result = match cmd.kind.as_str() {
            "projects" => self
                .repos
                .projects
                .create(
                    ctx.instance_id(),
                    &NamedResourceRecord {
                        id,
                        name: cmd.name.clone(),
                        state: "active".to_string(),
                        created_at: String::new(),
                        updated_at: String::new(),
                    },
                    &cmd.org_id,
                )
                .await
                .map_err(AppError::Internal)?,
            "apps" => app_to_named_resource(
                self.repos
                    .apps
                    .create(
                        ctx.instance_id(),
                        &AppRecord {
                            id,
                            group_id: cmd.org_id.clone(),
                            name: cmd.name.clone(),
                            protocol: String::new(),
                            state: "active".to_string(),
                            metadata: serde_json::Value::Object(Default::default()),
                            created_at: String::new(),
                            updated_at: String::new(),
                        },
                    )
                    .await
                    .map_err(AppError::Internal)?,
            ),
            _ => {
                return Err(AppError::validation(format!(
                    "unsupported named resource kind: {}",
                    cmd.kind
                )));
            }
        };

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::ResourceCreated {
                    resource_id: result.id.clone(),
                    kind: cmd.kind,
                    name: cmd.name,
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(result)
    }
}

pub struct GetNamedResource {
    repos: Arc<Repositories>,
}

impl GetNamedResource {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_named_resource", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        kind: &str,
        id: &str,
    ) -> Result<NamedResourceRecord, AppError> {
        match kind {
            "projects" => self
                .repos
                .projects
                .get(ctx.instance_id(), id)
                .await
                .map_err(AppError::Internal)?
                .ok_or_else(|| AppError::not_found(kind, id)),
            "apps" => self
                .repos
                .apps
                .get(ctx.instance_id(), id)
                .await
                .map_err(AppError::Internal)?
                .map(app_to_named_resource)
                .ok_or_else(|| AppError::not_found(kind, id)),
            _ => Err(AppError::validation(format!(
                "unsupported named resource kind: {kind}"
            ))),
        }
    }
}

pub struct ListNamedResources {
    repos: Arc<Repositories>,
}

impl ListNamedResources {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_named_resources", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        kind: &str,
        cursor: &str,
        limit: i64,
    ) -> Result<Vec<NamedResourceRecord>, AppError> {
        let params = crate::repo::ListParams {
            limit: Some(limit.max(1) as u32),
            cursor: if cursor.is_empty() {
                None
            } else {
                Some(cursor.to_string())
            },
            search: None,
        };
        match kind {
            "projects" => self
                .repos
                .projects
                .list(ctx.instance_id(), &params)
                .await
                .map(|result| result.items)
                .map_err(AppError::Internal),
            "apps" => self
                .repos
                .apps
                .list(ctx.instance_id(), None, &params)
                .await
                .map(|result| {
                    result
                        .items
                        .into_iter()
                        .map(app_to_named_resource)
                        .collect()
                })
                .map_err(AppError::Internal),
            _ => Err(AppError::validation(format!(
                "unsupported named resource kind: {kind}"
            ))),
        }
    }
}

pub struct UpdateNamedResource {
    repos: Arc<Repositories>,
}

pub struct UpdateNamedResourceCommand {
    pub kind: String,
    pub id: String,
    pub name: String,
}

impl UpdateNamedResource {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.update_named_resource", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpdateNamedResourceCommand,
    ) -> Result<bool, AppError> {
        let updated = match cmd.kind.as_str() {
            "projects" => self
                .repos
                .projects
                .update_name(ctx.instance_id(), &cmd.id, &cmd.name)
                .await
                .map_err(AppError::Internal)?,
            "apps" => self
                .repos
                .apps
                .update_name(ctx.instance_id(), &cmd.id, &cmd.name)
                .await
                .map_err(AppError::Internal)?,
            _ => {
                return Err(AppError::validation(format!(
                    "unsupported named resource kind: {}",
                    cmd.kind
                )));
            }
        };

        if updated {
            self.repos
                .events
                .append(
                    ctx.instance_id(),
                    &DomainEvent::ResourceUpdated {
                        resource_id: cmd.id,
                        kind: cmd.kind,
                        fields_changed: vec!["name".to_string()],
                        actor_id: ctx.user_id().to_string(),
                    },
                    None,
                    None,
                    None,
                )
                .await
                .map_err(AppError::Internal)?;
        }

        Ok(updated)
    }
}

pub struct DeleteNamedResource {
    repos: Arc<Repositories>,
}

impl DeleteNamedResource {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.delete_named_resource", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        kind: &str,
        id: &str,
    ) -> Result<bool, AppError> {
        let deleted = match kind {
            "projects" => self
                .repos
                .projects
                .delete(ctx.instance_id(), id)
                .await
                .map_err(AppError::Internal)?,
            "apps" => self
                .repos
                .apps
                .delete(ctx.instance_id(), id)
                .await
                .map_err(AppError::Internal)?,
            _ => {
                return Err(AppError::validation(format!(
                    "unsupported named resource kind: {kind}"
                )));
            }
        };

        if deleted {
            self.repos
                .events
                .append(
                    ctx.instance_id(),
                    &DomainEvent::ResourceDeleted {
                        resource_id: id.to_string(),
                        kind: kind.to_string(),
                        actor_id: ctx.user_id().to_string(),
                    },
                    None,
                    None,
                    None,
                )
                .await
                .map_err(AppError::Internal)?;
        }

        Ok(deleted)
    }
}
