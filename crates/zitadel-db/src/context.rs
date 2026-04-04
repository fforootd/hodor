use std::borrow::Cow;
use std::future::Future;

use tokio::task_local;

use crate::DEFAULT_INSTANCE_ID;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstanceContext {
    pub instance_id: String,
    pub customer_id: String,
    pub placement_mode: String,
    pub region_key: Option<String>,
    pub backend_key: String,
    pub backend_kind: String,
    pub backend_url: String,
    pub backend_secret_ref: String,
    pub host: String,
    pub source: String,
}

impl InstanceContext {
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            customer_id: String::new(),
            placement_mode: "global".into(),
            region_key: None,
            backend_key: "default".into(),
            backend_kind: String::new(),
            backend_url: String::new(),
            backend_secret_ref: String::new(),
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
                customer_id: "cust_1".into(),
                placement_mode: "regional".into(),
                region_key: Some("europe-west1".into()),
                backend_key: "eu-primary".into(),
                backend_kind: "spanner".into(),
                backend_url: "spanner://projects/example/instances/eu/databases/identity".into(),
                backend_secret_ref: "projects/example/secrets/eu-primary".into(),
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

        assert_eq!(scoped.backend_key, "eu-primary");
        assert_eq!(scoped.backend_kind, "spanner");
        assert_eq!(current_instance_id().as_ref(), DEFAULT_INSTANCE_ID);
    }
}
