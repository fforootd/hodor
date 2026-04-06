//! Use case unit tests with mock repositories (ADR-032 CLAUDE-4).
//!
//! These tests verify business logic in isolation — no database, no HTTP.

use std::sync::Arc;
use zitadel_app::ApplicationServices;
use zitadel_app::context::{ActorContext, AuthContext, Capability, Identity, InstanceContext};
use zitadel_app::error::AppError;
use zitadel_app::hook::HookPipeline;
use zitadel_app::mock::{MockEventRepository, MockOrgRepository, MockUserRepository};
use zitadel_app::repo::Repositories;
use zitadel_app::users::{CreateUser, CreateUserCommand};

fn test_ctx() -> ActorContext {
    ActorContext {
        auth: AuthContext {
            identity: Identity {
                user_id: "actor-1".into(),
                session_id: "sess-1".into(),
                token_type: "session".into(),
                org_id: "org-1".into(),
            },
            capabilities: vec![Capability::OperatorAdmin],
        },
        instance: InstanceContext {
            instance_id: "test-instance".into(),
            placement_mode: "global".into(),
            region_key: None,
            feature_overrides: Default::default(),
            host: "localhost".into(),
        },
    }
}

fn test_services() -> (Arc<ApplicationServices>, Arc<Repositories>) {
    let repos = Arc::new(zitadel_app::mock::mock_repositories());
    let hooks = Arc::new(HookPipeline::empty());
    let app = Arc::new(ApplicationServices::new(repos.clone(), hooks));
    (app, repos)
}

// ─── CreateUser tests ─��──────────────────────────────────

#[tokio::test]
async fn create_user_success() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    // Seed a default org so first_org_id returns something.
    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let cmd = CreateUserCommand {
        identifier: "alice@example.com".into(),
        display_name: "Alice".into(),
        user_type: "human".into(),
        schema_id: "default".into(),
        org_id: Some("org-1".into()),
        metadata: serde_json::json!({}),
    };

    let user = app.create_user.execute(&ctx, cmd).await.unwrap();
    assert_eq!(user.identifier, "alice@example.com");
    assert_eq!(user.display_name, "Alice");
    assert_eq!(user.state, "active");
    assert_eq!(user.org_id, "org-1");
}

#[tokio::test]
async fn create_user_empty_identifier_rejected() {
    let (app, _) = test_services();
    let ctx = test_ctx();

    let cmd = CreateUserCommand {
        identifier: "".into(),
        display_name: "Alice".into(),
        user_type: "human".into(),
        schema_id: "default".into(),
        org_id: Some("org-1".into()),
        metadata: serde_json::json!({}),
    };

    let err = app.create_user.execute(&ctx, cmd).await.unwrap_err();
    assert!(matches!(err, AppError::Validation { .. }));
    assert_eq!(err.status_code(), 400);
}

#[tokio::test]
async fn create_user_duplicate_rejected() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let cmd = CreateUserCommand {
        identifier: "alice@example.com".into(),
        display_name: "Alice".into(),
        user_type: "human".into(),
        schema_id: "default".into(),
        org_id: Some("org-1".into()),
        metadata: serde_json::json!({}),
    };

    app.create_user.execute(&ctx, cmd).await.unwrap();

    // Attempt duplicate
    let cmd2 = CreateUserCommand {
        identifier: "alice@example.com".into(),
        display_name: "Alice 2".into(),
        user_type: "human".into(),
        schema_id: "default".into(),
        org_id: Some("org-1".into()),
        metadata: serde_json::json!({}),
    };

    let err = app.create_user.execute(&ctx, cmd2).await.unwrap_err();
    assert!(matches!(err, AppError::AlreadyExists { .. }));
    assert_eq!(err.status_code(), 409);
}

// ─── GetUser tests ───────────────────────────────────────

#[tokio::test]
async fn get_user_not_found() {
    let (app, _) = test_services();
    let ctx = test_ctx();

    let err = app.get_user.execute(&ctx, "nonexistent").await.unwrap_err();
    assert!(matches!(err, AppError::NotFound { .. }));
    assert_eq!(err.status_code(), 404);
}

#[tokio::test]
async fn get_user_after_create() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let created = app
        .create_user
        .execute(
            &ctx,
            CreateUserCommand {
                identifier: "bob".into(),
                display_name: "Bob".into(),
                user_type: "human".into(),
                schema_id: "default".into(),
                org_id: Some("org-1".into()),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let fetched = app.get_user.execute(&ctx, &created.id).await.unwrap();
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.identifier, "bob");
}

// ─── DeactivateUser tests ────────────────────────────────

#[tokio::test]
async fn deactivate_user_success() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let user = app
        .create_user
        .execute(
            &ctx,
            CreateUserCommand {
                identifier: "charlie".into(),
                display_name: "Charlie".into(),
                user_type: "human".into(),
                schema_id: "default".into(),
                org_id: Some("org-1".into()),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    app.deactivate_user.execute(&ctx, &user.id).await.unwrap();

    let deactivated = app.get_user.execute(&ctx, &user.id).await.unwrap();
    assert_eq!(deactivated.state, "deactivated");
}

#[tokio::test]
async fn deactivate_nonexistent_user_fails() {
    let (app, _) = test_services();
    let ctx = test_ctx();

    let err = app
        .deactivate_user
        .execute(&ctx, "no-such-user")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::NotFound { .. }));
}

// ─── Hook pipeline tests ─────────────────────────────────
// The UseCaseRunner requires `impl UseCase`. Use cases in this crate have custom
// execute methods but don't implement the trait directly. Test the interceptor
// pipeline by creating a trivial inline use case.

use std::future::Future;
use std::pin::Pin;
use zitadel_app::hook::{
    DenyReason, HookContext, HookPhase, InterceptResult, PolicyInterceptor, StepUpKind,
};
use zitadel_app::usecase::UseCase;

struct NoopUseCase;

impl UseCase for NoopUseCase {
    type Command = ();
    type Result = String;
    type Error = AppError;

    fn execute(
        &self,
        _ctx: &ActorContext,
        _cmd: (),
    ) -> impl Future<Output = Result<String, AppError>> + Send {
        async { Ok("ok".to_string()) }
    }
}

#[tokio::test]
async fn hook_pipeline_empty_runs_use_case() {
    let runner = zitadel_app::UseCaseRunner::new(vec![], vec![], vec![]);
    let ctx = test_ctx();
    let result = runner.run(&NoopUseCase, &ctx, (), "test.noop").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "ok");
}

#[tokio::test]
async fn hook_pipeline_deny_interceptor_blocks() {
    struct AlwaysDeny;
    impl PolicyInterceptor for AlwaysDeny {
        fn intercept(
            &self,
            _phase: HookPhase,
            _ctx: &HookContext,
        ) -> Pin<Box<dyn Future<Output = InterceptResult> + Send + '_>> {
            Box::pin(async {
                InterceptResult::Deny(DenyReason {
                    code: "test.denied".into(),
                    message: "blocked by test".into(),
                })
            })
        }
    }

    let runner = zitadel_app::UseCaseRunner::new(
        vec![Arc::new(AlwaysDeny) as Arc<dyn PolicyInterceptor>],
        vec![],
        vec![],
    );
    let ctx = test_ctx();
    let err = runner
        .run(&NoopUseCase, &ctx, (), "test.noop")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::PolicyDenied { .. }));
    assert_eq!(err.status_code(), 403);
}

#[tokio::test]
async fn hook_pipeline_step_up_interceptor() {
    struct RequireMfa;
    impl PolicyInterceptor for RequireMfa {
        fn intercept(
            &self,
            _phase: HookPhase,
            _ctx: &HookContext,
        ) -> Pin<Box<dyn Future<Output = InterceptResult> + Send + '_>> {
            Box::pin(async { InterceptResult::RequireStepUp(StepUpKind::Otp) })
        }
    }

    let runner = zitadel_app::UseCaseRunner::new(
        vec![],
        vec![Arc::new(RequireMfa) as Arc<dyn PolicyInterceptor>],
        vec![],
    );
    let ctx = test_ctx();
    let err = runner
        .run(&NoopUseCase, &ctx, (), "test.noop")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::StepUpRequired { .. }));
}

#[tokio::test]
async fn hook_pipeline_interceptor_ordering() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct OrderedInterceptor {
        expected_order: usize,
    }
    impl PolicyInterceptor for OrderedInterceptor {
        fn intercept(
            &self,
            _phase: HookPhase,
            _ctx: &HookContext,
        ) -> Pin<Box<dyn Future<Output = InterceptResult> + Send + '_>> {
            let expected = self.expected_order;
            Box::pin(async move {
                let actual = COUNTER.fetch_add(1, Ordering::SeqCst);
                assert_eq!(actual, expected, "interceptor ran out of order");
                InterceptResult::Continue
            })
        }
    }

    COUNTER.store(0, Ordering::SeqCst);
    let runner = zitadel_app::UseCaseRunner::new(
        vec![
            Arc::new(OrderedInterceptor { expected_order: 0 }) as Arc<dyn PolicyInterceptor>,
            Arc::new(OrderedInterceptor { expected_order: 1 }),
        ],
        vec![Arc::new(OrderedInterceptor { expected_order: 2 })],
        vec![],
    );
    let ctx = test_ctx();
    let result = runner.run(&NoopUseCase, &ctx, (), "test.order").await;
    assert!(result.is_ok());
    assert_eq!(COUNTER.load(Ordering::SeqCst), 3);
}

// ─── DeleteOrg tests ────────────────────────────────────

#[tokio::test]
async fn delete_org_does_not_delete_users() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    // Create an org and a user in it.
    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let user = app
        .create_user
        .execute(
            &ctx,
            CreateUserCommand {
                identifier: "admin".into(),
                display_name: "Admin".into(),
                user_type: "human".into(),
                schema_id: "default".into(),
                org_id: Some("org-1".into()),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    // Delete the org.
    app.delete_org.execute(&ctx, "org-1").await.unwrap();

    // Org should be gone.
    let err = app.get_org.execute(&ctx, "org-1").await.unwrap_err();
    assert!(matches!(err, AppError::NotFound { .. }));

    // User must still exist (the mock does not cascade — the DB migration
    // changes the FK from CASCADE to SET NULL so the real DB behaves the same).
    let fetched = app.get_user.execute(&ctx, &user.id).await.unwrap();
    assert_eq!(fetched.identifier, "admin");
}

#[tokio::test]
async fn delete_org_success() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-a".into(),
                name: "Org A".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-b".into(),
                name: "Org B".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    // Delete org-a — should succeed.
    app.delete_org.execute(&ctx, "org-a").await.unwrap();

    // org-a gone, org-b still around.
    assert!(matches!(
        app.get_org.execute(&ctx, "org-a").await,
        Err(AppError::NotFound { .. })
    ));
    assert!(app.get_org.execute(&ctx, "org-b").await.is_ok());
}

#[tokio::test]
async fn delete_org_not_found() {
    let (app, _) = test_services();
    let ctx = test_ctx();

    let err = app.delete_org.execute(&ctx, "nonexistent").await.unwrap_err();
    assert!(matches!(err, AppError::NotFound { .. }));
}

#[tokio::test]
async fn delete_org_requires_operator_admin() {
    let (app, repos) = test_services();

    // Create a context without operator admin capability.
    let ctx = ActorContext {
        auth: AuthContext {
            identity: Identity {
                user_id: "actor-1".into(),
                session_id: "sess-1".into(),
                token_type: "session".into(),
                org_id: "org-1".into(),
            },
            capabilities: vec![], // no operator admin
        },
        instance: InstanceContext {
            instance_id: "test-instance".into(),
            placement_mode: "global".into(),
            region_key: None,
            feature_overrides: Default::default(),
            host: "localhost".into(),
        },
    };

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let err = app.delete_org.execute(&ctx, "org-1").await.unwrap_err();
    assert!(matches!(err, AppError::OperatorAdminRequired));
}

// ─── ApplicationServices wiring test ─────────────────────

#[tokio::test]
async fn application_services_wired_correctly() {
    let (app, _) = test_services();

    // Verify all use case fields are accessible (compile-time check + runtime sanity)
    let ctx = test_ctx();

    // Each field should be callable without panicking on construction
    let _ = &app.create_user;
    let _ = &app.get_user;
    let _ = &app.list_users;
    let _ = &app.update_user;
    let _ = &app.deactivate_user;
    let _ = &app.set_password;
    let _ = &app.verify_password;
    let _ = &app.link_identity;
    let _ = &app.start_login;
    let _ = &app.submit_login_step;
    let _ = &app.issue_session;
    let _ = &app.revoke_session;
    let _ = &app.create_org;
    let _ = &app.get_org;
    let _ = &app.list_orgs;
    let _ = &app.update_org;
    let _ = &app.delete_org;
    let _ = &app.create_group;
    let _ = &app.get_group;
    let _ = &app.list_groups;
    let _ = &app.update_group;
    let _ = &app.create_instance;
    let _ = &app.get_instance;
    let _ = &app.list_instances;
    let _ = &app.update_instance;
    let _ = &app.deprovision_instance;
    let _ = &app.get_settings;
    let _ = &app.update_settings;
    let _ = &app.create_provider;
    let _ = &app.get_provider;
    let _ = &app.list_providers;
    let _ = &app.update_provider;
    let _ = &app.delete_provider;
    let _ = &app.register_schema;
    let _ = &app.get_schema;
    let _ = &app.list_schemas;
    let _ = &app.hooks;
}
