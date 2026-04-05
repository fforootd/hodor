use std::borrow::Cow;
use std::future::Future;

use tokio::task_local;

use crate::DEFAULT_INSTANCE_ID;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstanceContext {
    pub instance_id: String,
    pub resolved_org_id: Option<String>,
    pub placement_mode: String,
    pub region_key: Option<String>,
    pub host: String,
    pub source: String,
}

impl InstanceContext {
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            resolved_org_id: None,
            placement_mode: "global".into(),
            region_key: None,
            host: String::new(),
            source: String::new(),
        }
    }
}

task_local! {
    static INSTANCE_CONTEXT: InstanceContext;
}

pub async fn with_instance_context<F>(context: InstanceContext, fut: F) -> F::Output
where
    F: Future,
{
    INSTANCE_CONTEXT.scope(context, fut).await
}

pub fn current_instance_context() -> Option<InstanceContext> {
    INSTANCE_CONTEXT.try_with(Clone::clone).ok()
}

pub fn current_instance_id() -> Cow<'static, str> {
    current_instance_context()
        .map(|ctx| Cow::Owned(ctx.instance_id))
        .unwrap_or_else(|| Cow::Borrowed(DEFAULT_INSTANCE_ID))
}

pub fn current_instance_id_or<'a>(fallback: &'a str) -> Cow<'a, str> {
    current_instance_context()
        .map(|ctx| Cow::Owned(ctx.instance_id))
        .unwrap_or_else(|| Cow::Borrowed(fallback))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn task_local_context_overrides_default_instance() {
        let before = current_instance_id();
        assert_eq!(before.as_ref(), DEFAULT_INSTANCE_ID);

        let scoped = with_instance_context(
            InstanceContext {
                instance_id: "inst_cloud".into(),
                resolved_org_id: Some("org_parent".into()),
                placement_mode: "regional".into(),
                region_key: Some("europe-west1".into()),
                host: "login.example.com".into(),
                source: "host".into(),
            },
            async {
                let current = current_instance_id();
                assert_eq!(current.as_ref(), "inst_cloud");
                current_instance_context().unwrap()
            },
        )
        .await;

        assert_eq!(scoped.resolved_org_id.as_deref(), Some("org_parent"));
        assert_eq!(current_instance_id().as_ref(), DEFAULT_INSTANCE_ID);
    }
}
