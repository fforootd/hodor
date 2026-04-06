pub mod dto;
pub mod error;
pub mod traits;
pub mod service;
pub mod service_impls;
pub mod evaluation;
pub mod core_model;

// Re-export everything for backward compatibility
pub use dto::*;
pub use error::*;
pub use traits::*;
pub use service::FgaService;
pub use core_model::core_authorization_model;

// Constants
pub const SCHEMA_VERSION_1_1: &str = "1.1";
pub const CORE_MODEL_VERSION: &str = "core-2026-04-05-root-hierarchy-v1";
pub(crate) const LIST_SCAN_FALLBACK_LIMIT: usize = 10_000;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::*;
    use crate::traits::Evaluator;

    use serde_json::{Map, json};
    use zitadel_db::{
        CreateManagedInstanceInput, DEFAULT_INSTANCE_ID, DEFAULT_ORG_ID, Db, add_membership,
        create_user, migrate,
    };

    async fn test_service() -> FgaService {
        let db = Db::open("").await.unwrap();
        migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db, None).await.unwrap();
        FgaService::new(db)
    }

    async fn test_service_with_db() -> (Db, FgaService) {
        let db = Db::open("").await.unwrap();
        migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db, None).await.unwrap();
        let service = FgaService::new(db.clone());
        (db, service)
    }

    #[tokio::test]
    async fn initializes_singleton_store_and_default_model() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        assert_eq!(store.id, DEFAULT_INSTANCE_ID);
        let model = service
            .read_model(DEFAULT_INSTANCE_ID, &store.id, None)
            .await
            .unwrap();
        assert_eq!(model.schema_version, SCHEMA_VERSION_1_1);
        assert!(
            model
                .type_definitions
                .iter()
                .any(|type_def| type_def.type_name == "org")
        );
    }

    #[tokio::test]
    async fn write_and_check_direct_relation() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        service
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: vec![TupleKey {
                            user: "user:anne".into(),
                            relation: "member".into(),
                            object: "group:engineering".into(),
                            condition: None,
                        }],
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
            .unwrap();

        let allowed = service
            .check(
                DEFAULT_INSTANCE_ID,
                &store.id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: "user:anne".into(),
                        relation: "member".into(),
                        object: "group:engineering".into(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await
            .unwrap();
        assert!(allowed.allowed);
    }

    #[tokio::test]
    async fn allows_direct_tuple_on_union_relation_when_subject_matches_metadata() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();

        service
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: vec![TupleKey {
                            user: "user:anne".into(),
                            relation: "admin".into(),
                            object: "org:engineering".into(),
                            condition: None,
                        }],
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
            .unwrap();

        let allowed = service
            .check(
                DEFAULT_INSTANCE_ID,
                &store.id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: "user:anne".into(),
                        relation: "member".into(),
                        object: "org:engineering".into(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await
            .unwrap();
        assert!(allowed.allowed);
    }

    #[tokio::test]
    async fn rejects_invalid_direct_tuple_on_union_relation() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();

        let err = service
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: vec![TupleKey {
                            user: "group:engineering#member".into(),
                            relation: "admin".into(),
                            object: "org:engineering".into(),
                            condition: None,
                        }],
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("cannot be directly related to org:engineering#admin")
        );
    }

    #[tokio::test]
    async fn rejects_metadata_on_computed_only_relation() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let model = AuthorizationModelWriteRequest {
            schema_version: SCHEMA_VERSION_1_1.into(),
            type_definitions: {
                let mut types = core_authorization_model().type_definitions;
                types.push(TypeDefinition {
                    type_name: "document".into(),
                    relations: Map::from_iter([
                        ("viewer".into(), json!({ "this": {} })),
                        (
                            "effective_viewer".into(),
                            json!({
                                "difference": {
                                    "base": { "computedUserset": { "relation": "viewer" } },
                                    "subtract": { "computedUserset": { "relation": "viewer" } }
                                }
                            }),
                        ),
                    ]),
                    metadata: Some(json!({
                        "relations": {
                            "viewer": { "directly_related_user_types": [{ "type": "user" }] },
                            "effective_viewer": { "directly_related_user_types": [{ "type": "user" }] }
                        }
                    })),
                });
                types
            },
            conditions: Map::new(),
        };

        let err = service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains(
                "cannot declare directly_related_user_types without a direct this branch"
            )
        );
    }

    #[tokio::test]
    async fn rejects_direct_tuple_on_computed_only_relation() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let model = AuthorizationModelWriteRequest {
            schema_version: SCHEMA_VERSION_1_1.into(),
            type_definitions: {
                let mut types = core_authorization_model().type_definitions;
                types.push(TypeDefinition {
                    type_name: "document".into(),
                    relations: Map::from_iter([
                        ("viewer".into(), json!({ "this": {} })),
                        (
                            "effective_viewer".into(),
                            json!({
                                "difference": {
                                    "base": { "computedUserset": { "relation": "viewer" } },
                                    "subtract": { "computedUserset": { "relation": "viewer" } }
                                }
                            }),
                        ),
                    ]),
                    metadata: Some(json!({
                        "relations": {
                            "viewer": { "directly_related_user_types": [{ "type": "user" }] }
                        }
                    })),
                });
                types
            },
            conditions: Map::new(),
        };
        service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap();

        let err = service
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: vec![TupleKey {
                            user: "user:anne".into(),
                            relation: "effective_viewer".into(),
                            object: "document:file".into(),
                            condition: None,
                        }],
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("relation document#effective_viewer is computed-only")
        );
    }

    #[tokio::test]
    async fn repeated_checks_reuse_cached_active_model() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        service
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: vec![TupleKey {
                            user: "user:anne".into(),
                            relation: "member".into(),
                            object: "group:engineering".into(),
                            condition: None,
                        }],
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
            .unwrap();

        let request = CheckRequest {
            tuple_key: TupleKey {
                user: "user:anne".into(),
                relation: "member".into(),
                object: "group:engineering".into(),
                condition: None,
            },
            authorization_model_id: None,
            contextual_tuples: None,
            context: None,
        };

        let first = service
            .check(DEFAULT_INSTANCE_ID, &store.id, request.clone())
            .await
            .unwrap();
        assert!(first.allowed);

        let active_key = (DEFAULT_INSTANCE_ID.to_string(), store.id.clone());
        let cached_model_id = service
            .active_model_cache
            .read()
            .await
            .get(&active_key)
            .map(|entry| entry.model_id.clone())
            .expect("active model cache should be populated after the first check");

        let second = service
            .check(DEFAULT_INSTANCE_ID, &store.id, request)
            .await
            .unwrap();
        assert!(second.allowed);
        assert_eq!(
            service.active_model_cache.read().await[&active_key].model_id,
            cached_model_id
        );
    }

    #[tokio::test]
    async fn cyclic_checks_cache_cycle_reason() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let model = AuthorizationModelWriteRequest {
            schema_version: SCHEMA_VERSION_1_1.into(),
            type_definitions: {
                let mut types = core_authorization_model().type_definitions;
                types.push(TypeDefinition {
                    type_name: "document".into(),
                    relations: Map::from_iter([(
                        "viewer".into(),
                        json!({ "computedUserset": { "relation": "viewer" } }),
                    )]),
                    metadata: Some(json!({ "relations": {} })),
                });
                types
            },
            conditions: Map::new(),
        };
        service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap();

        let (_, compiled) = service
            .load_compiled_model(DEFAULT_INSTANCE_ID, &store.id, None)
            .await
            .unwrap();
        let mut ctx =
            service.evaluate_internal(DEFAULT_INSTANCE_ID, &store.id, compiled.as_ref(), &[]);
        let user = UserRef::parse("user:anne").unwrap();
        let object = ObjectRef::parse("document:file").unwrap();

        let outcome = ctx.check(&user, "viewer", &object, 0).await.unwrap();
        assert_eq!(outcome, EvalOutcome::Deny(DenyReason::CycleDetected));
        assert_eq!(
            ctx.decision_cache
                .get(&(user.as_raw(), "viewer".to_string(), object.as_raw()))
                .copied(),
            Some(EvalOutcome::Deny(DenyReason::CycleDetected))
        );
    }

    #[tokio::test]
    async fn deep_checks_cache_depth_reason() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();

        let mut relations = Map::new();
        for idx in 0..40 {
            let name = format!("step_{idx}");
            let next = format!("step_{}", idx + 1);
            relations.insert(name, json!({ "computedUserset": { "relation": next } }));
        }
        relations.insert("step_40".into(), json!({ "this": {} }));
        let model = AuthorizationModelWriteRequest {
            schema_version: SCHEMA_VERSION_1_1.into(),
            type_definitions: {
                let mut types = core_authorization_model().type_definitions;
                types.push(TypeDefinition {
                    type_name: "document".into(),
                    relations,
                    metadata: Some(json!({
                        "relations": {
                            "step_40": { "directly_related_user_types": [{ "type": "user" }] }
                        }
                    })),
                });
                types
            },
            conditions: Map::new(),
        };
        service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap();

        let (_, compiled) = service
            .load_compiled_model(DEFAULT_INSTANCE_ID, &store.id, None)
            .await
            .unwrap();
        let mut ctx =
            service.evaluate_internal(DEFAULT_INSTANCE_ID, &store.id, compiled.as_ref(), &[]);
        let user = UserRef::parse("user:anne").unwrap();
        let object = ObjectRef::parse("document:file").unwrap();

        let outcome = ctx.check(&user, "step_0", &object, 0).await.unwrap();
        assert_eq!(outcome, EvalOutcome::Deny(DenyReason::DepthExhausted));
        assert_eq!(
            ctx.decision_cache
                .get(&(user.as_raw(), "step_0".to_string(), object.as_raw()))
                .copied(),
            Some(EvalOutcome::Deny(DenyReason::DepthExhausted))
        );
    }

    #[tokio::test]
    async fn supports_tuple_to_userset_and_difference() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let model = AuthorizationModelWriteRequest {
            schema_version: SCHEMA_VERSION_1_1.into(),
            type_definitions: {
                let mut types = core_authorization_model().type_definitions;
                types.push(TypeDefinition {
                    type_name: "document".into(),
                    relations: Map::from_iter([
                        ("parent".into(), json!({ "this": {} })),
                        (
                            "viewer".into(),
                            json!({
                                "union": {
                                    "child": [
                                        { "this": {} },
                                        { "tupleToUserset": {
                                            "tupleset": { "relation": "parent" },
                                            "computedUserset": { "relation": "viewer" }
                                        }}
                                    ]
                                }
                            }),
                        ),
                        ("blocked".into(), json!({ "this": {} })),
                        (
                            "effective_viewer".into(),
                            json!({
                                "difference": {
                                    "base": { "computedUserset": { "relation": "viewer" } },
                                    "subtract": { "computedUserset": { "relation": "blocked" } }
                                }
                            }),
                        ),
                    ]),
                    metadata: Some(json!({
                        "relations": {
                            "parent": { "directly_related_user_types": [{ "type": "document" }] },
                            "viewer": { "directly_related_user_types": [{ "type": "user" }] },
                            "blocked": { "directly_related_user_types": [{ "type": "user" }] }
                        }
                    })),
                });
                types
            },
            conditions: Map::new(),
        };
        service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap();
        service
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: vec![
                            TupleKey {
                                user: "document:folder".into(),
                                relation: "parent".into(),
                                object: "document:file".into(),
                                condition: None,
                            },
                            TupleKey {
                                user: "user:anne".into(),
                                relation: "viewer".into(),
                                object: "document:folder".into(),
                                condition: None,
                            },
                            TupleKey {
                                user: "user:bob".into(),
                                relation: "viewer".into(),
                                object: "document:file".into(),
                                condition: None,
                            },
                            TupleKey {
                                user: "user:bob".into(),
                                relation: "blocked".into(),
                                object: "document:file".into(),
                                condition: None,
                            },
                        ],
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
            .unwrap();

        let anne = service
            .check(
                DEFAULT_INSTANCE_ID,
                &store.id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: "user:anne".into(),
                        relation: "effective_viewer".into(),
                        object: "document:file".into(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await
            .unwrap();
        assert!(anne.allowed);

        let bob = service
            .check(
                DEFAULT_INSTANCE_ID,
                &store.id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: "user:bob".into(),
                        relation: "effective_viewer".into(),
                        object: "document:file".into(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await
            .unwrap();
        assert!(!bob.allowed);
    }

    #[tokio::test]
    async fn list_objects_uses_planned_candidates_for_core_union_relations() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();

        service
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: vec![
                            TupleKey {
                                user: "user:anne".into(),
                                relation: "admin".into(),
                                object: "org:engineering".into(),
                                condition: None,
                            },
                            TupleKey {
                                user: "user:anne".into(),
                                relation: "member".into(),
                                object: "org:ops".into(),
                                condition: None,
                            },
                        ],
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
            .unwrap();

        let response = service
            .list_objects(
                DEFAULT_INSTANCE_ID,
                &store.id,
                ListObjectsRequest {
                    user: "user:anne".into(),
                    relation: "member".into(),
                    object_type: "org".into(),
                    authorization_model_id: None,
                    contextual_tuples: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            response.objects,
            vec!["org:engineering".to_string(), "org:ops".to_string()]
        );
    }

    #[tokio::test]
    async fn list_users_uses_planned_candidates_for_core_union_relations() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();

        service
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: vec![
                            TupleKey {
                                user: "user:owner".into(),
                                relation: "owner".into(),
                                object: "org:acme".into(),
                                condition: None,
                            },
                            TupleKey {
                                user: "user:admin".into(),
                                relation: "admin".into(),
                                object: "org:acme".into(),
                                condition: None,
                            },
                            TupleKey {
                                user: "user:member".into(),
                                relation: "member".into(),
                                object: "org:acme".into(),
                                condition: None,
                            },
                            TupleKey {
                                user: "user:viewer".into(),
                                relation: "viewer".into(),
                                object: "org:acme".into(),
                                condition: None,
                            },
                        ],
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
            .unwrap();

        let response = service
            .list_users(
                DEFAULT_INSTANCE_ID,
                &store.id,
                ListUsersRequest {
                    object: "org:acme".into(),
                    relation: "viewer".into(),
                    user_filters: vec![UserFilter {
                        user_type: "user".into(),
                        relation: None,
                    }],
                    authorization_model_id: None,
                    contextual_tuples: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            response.users,
            vec![
                "user:admin".to_string(),
                "user:member".to_string(),
                "user:owner".to_string(),
                "user:viewer".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn list_objects_scan_fallback_is_bounded() {
        let (db, service) = test_service_with_db().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let model = AuthorizationModelWriteRequest {
            schema_version: SCHEMA_VERSION_1_1.into(),
            type_definitions: {
                let mut types = core_authorization_model().type_definitions;
                types.push(TypeDefinition {
                    type_name: "document".into(),
                    relations: Map::from_iter([("viewer".into(), json!({ "this": {} }))]),
                    metadata: None,
                });
                types
            },
            conditions: Map::new(),
        };
        service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap();

        let scoped = db.scoped(DEFAULT_INSTANCE_ID.to_string());
        let mut tx = scoped.pool().begin().await.unwrap();
        for idx in 0..=LIST_SCAN_FALLBACK_LIMIT {
            let object_id = format!("doc-{idx}");
            sqlx::query(
                "INSERT INTO fga_tuples \
                 (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user) \
                 VALUES ($1, $2, 'document', $3, 'viewer', 'user', 'anne', '', $4, 'user:anne')",
            )
            .bind(DEFAULT_INSTANCE_ID)
            .bind(&store.id)
            .bind(&object_id)
            .bind(format!("document:{object_id}"))
            .execute(&mut *tx)
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();

        let err = service
            .list_objects(
                DEFAULT_INSTANCE_ID,
                &store.id,
                ListObjectsRequest {
                    user: "user:anne".into(),
                    relation: "viewer".into(),
                    object_type: "document".into(),
                    authorization_model_id: None,
                    contextual_tuples: None,
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, FgaError::Unsupported(_)));
        assert!(
            err.to_string()
                .contains("list operation exceeds embedded planner budget")
        );
    }

    #[tokio::test]
    async fn rejects_sealed_core_mutation() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let mut model = core_authorization_model();
        model.type_definitions[1]
            .relations
            .insert("superadmin".into(), json!({ "this": {} }));
        let err = service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("sealed type instance"));
    }

    #[tokio::test]
    async fn write_model_replaces_active_model_cache() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let original = service
            .read_model(DEFAULT_INSTANCE_ID, &store.id, None)
            .await
            .unwrap();
        let mut model = core_authorization_model();
        model.type_definitions.push(TypeDefinition {
            type_name: "document".into(),
            relations: Map::from_iter([("viewer".into(), json!({ "this": {} }))]),
            metadata: Some(json!({
                "relations": {
                    "viewer": { "directly_related_user_types": [{ "type": "user" }] }
                }
            })),
        });
        let written = service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap();

        let current = service
            .read_model(DEFAULT_INSTANCE_ID, &store.id, None)
            .await
            .unwrap();
        assert_eq!(
            current.authorization_model_id,
            written.authorization_model_id
        );
        assert_ne!(
            current.authorization_model_id,
            original.authorization_model_id
        );

        let active_key = (DEFAULT_INSTANCE_ID.to_string(), store.id.clone());
        assert_eq!(
            service.active_model_cache.read().await[&active_key].model_id,
            written.authorization_model_id
        );
    }

    #[tokio::test]
    async fn explicit_model_reads_stay_available_after_active_model_changes() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let original = service
            .read_model(DEFAULT_INSTANCE_ID, &store.id, None)
            .await
            .unwrap();
        let mut model = core_authorization_model();
        model.type_definitions.push(TypeDefinition {
            type_name: "document".into(),
            relations: Map::from_iter([("viewer".into(), json!({ "this": {} }))]),
            metadata: Some(json!({
                "relations": {
                    "viewer": { "directly_related_user_types": [{ "type": "user" }] }
                }
            })),
        });
        let written = service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap();

        let old_model = service
            .read_model(
                DEFAULT_INSTANCE_ID,
                &store.id,
                Some(&original.authorization_model_id),
            )
            .await
            .unwrap();
        let new_model = service
            .read_model(
                DEFAULT_INSTANCE_ID,
                &store.id,
                Some(&written.authorization_model_id),
            )
            .await
            .unwrap();

        assert_eq!(
            old_model.authorization_model_id,
            original.authorization_model_id
        );
        assert_eq!(
            new_model.authorization_model_id,
            written.authorization_model_id
        );

        let explicit_cache = service.explicit_model_cache.read().await;
        assert!(explicit_cache.contains_key(&(
            DEFAULT_INSTANCE_ID.to_string(),
            store.id.clone(),
            original.authorization_model_id
        )));
        assert!(explicit_cache.contains_key(&(
            DEFAULT_INSTANCE_ID.to_string(),
            store.id.clone(),
            written.authorization_model_id
        )));
    }

    #[tokio::test]
    async fn initialize_instance_replaces_stale_core_models() {
        let (db, service) = test_service_with_db().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let original = service
            .read_model(DEFAULT_INSTANCE_ID, &store.id, None)
            .await
            .unwrap();

        sqlx::query(
            "UPDATE fga_authorization_models SET core_model_version = '' \
             WHERE instance_id = $1 AND store_id = $2 AND model_id = $3",
        )
        .bind(DEFAULT_INSTANCE_ID)
        .bind(&store.id)
        .bind(&original.authorization_model_id)
        .execute(db.pool())
        .await
        .unwrap();

        let reloaded = FgaService::new(db.clone());
        reloaded
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let current = reloaded
            .read_model(DEFAULT_INSTANCE_ID, &store.id, None)
            .await
            .unwrap();

        assert_ne!(
            current.authorization_model_id,
            original.authorization_model_id
        );
        let instance = current
            .type_definitions
            .iter()
            .find(|type_def| type_def.type_name == "instance")
            .unwrap();
        assert_eq!(
            instance.relations.get("admin"),
            Some(&json!({
                "union": {
                    "child": [
                        { "this": {} },
                        { "computedUserset": { "relation": "owner" } }
                    ]
                }
            }))
        );
    }

    #[tokio::test]
    async fn initialize_instance_rebuilds_legacy_module_fragments_and_persists_empty_modules() {
        let (db, service) = test_service_with_db().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();

        sqlx::query(
            "UPDATE fga_authorization_models SET is_active = 0 WHERE instance_id = $1 AND store_id = $2",
        )
        .bind(DEFAULT_INSTANCE_ID)
        .bind(&store.id)
        .execute(db.pool())
        .await
        .unwrap();

        let legacy_module = serde_json::json!([AuthorizationModelWriteRequest {
            schema_version: SCHEMA_VERSION_1_1.into(),
            type_definitions: vec![TypeDefinition {
                type_name: "document".into(),
                relations: Map::from_iter([("viewer".into(), json!({ "this": {} }))]),
                metadata: Some(json!({
                    "relations": {
                        "viewer": { "directly_related_user_types": [{ "type": "user" }] }
                    }
                })),
            }],
            conditions: Map::new(),
        }]);
        let compiled = serde_json::to_string(&core_authorization_model()).unwrap();
        sqlx::query(
            "INSERT INTO fga_authorization_models \
             (instance_id, store_id, model_id, schema_version, core_model_version, compiled_model, custom_model, module_fragments, is_active) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1)",
        )
        .bind(DEFAULT_INSTANCE_ID)
        .bind(&store.id)
        .bind("legacy-model")
        .bind(SCHEMA_VERSION_1_1)
        .bind("")
        .bind(compiled)
        .bind("{}")
        .bind(legacy_module.to_string())
        .execute(db.pool())
        .await
        .unwrap();

        let reloaded = FgaService::new(db.clone());
        reloaded
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let current = reloaded
            .read_model(DEFAULT_INSTANCE_ID, &store.id, None)
            .await
            .unwrap();

        assert!(
            current
                .type_definitions
                .iter()
                .any(|type_def| type_def.type_name == "document")
        );

        let module_fragments: String = sqlx::query_scalar(
            "SELECT module_fragments FROM fga_authorization_models \
             WHERE instance_id = $1 AND store_id = $2 AND is_active = 1 \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(DEFAULT_INSTANCE_ID)
        .bind(&store.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(module_fragments, "[]");
    }

    #[tokio::test]
    async fn reconcile_root_hierarchy_materializes_root_store_only() {
        let (db, service) = test_service_with_db().await;
        let root_user_id = "root-user";
        create_user(
            &db,
            DEFAULT_INSTANCE_ID,
            root_user_id,
            DEFAULT_ORG_ID,
            "root-user@example.com",
            "Root User",
            "",
            "{}",
        )
        .await
        .unwrap();
        add_membership(
            &db,
            DEFAULT_INSTANCE_ID,
            "org",
            DEFAULT_ORG_ID,
            root_user_id,
            "owner",
        )
        .await
        .unwrap();
        zitadel_db::create_managed_instance(
            &db,
            &CreateManagedInstanceInput {
                instance_id: "child-a".into(),
                root_instance_id: DEFAULT_INSTANCE_ID.into(),
                owner_org_id: DEFAULT_ORG_ID.into(),
                primary_domain: "child-a.example.com".into(),
                kind: "managed".into(),
                placement_mode: "global".into(),
                region_key: None,
            },
        )
        .await
        .unwrap();

        service
            .reconcile_root_hierarchy(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();

        let root_store = service.discover_store(DEFAULT_INSTANCE_ID).await.unwrap();
        let root_allowed = service
            .check(
                DEFAULT_INSTANCE_ID,
                &root_store.id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: format!("user:{root_user_id}"),
                        relation: "admin".into(),
                        object: "instance:child-a".into(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await
            .unwrap();
        assert!(root_allowed.allowed);

        let child_store = service.discover_store("child-a").await.unwrap();
        let child_allowed = service
            .check(
                "child-a",
                &child_store.id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: format!("user:{root_user_id}"),
                        relation: "admin".into(),
                        object: "instance:child-a".into(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await
            .unwrap();
        assert!(!child_allowed.allowed);
    }
}
