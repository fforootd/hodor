use std::collections::{HashMap, HashSet};

use async_recursion::async_recursion;

use crate::dto::*;
use crate::error::FgaError;
use crate::service::FgaService;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ObjectRef {
    pub(crate) object_type: String,
    pub(crate) object_id: String,
}

impl ObjectRef {
    pub(crate) fn parse(raw: &str) -> Result<Self, FgaError> {
        let Some((object_type, object_id)) = raw.split_once(':') else {
            return Err(FgaError::BadRequest(format!(
                "invalid object reference {raw}"
            )));
        };
        if object_type.is_empty() || object_id.is_empty() {
            return Err(FgaError::BadRequest(format!(
                "invalid object reference {raw}"
            )));
        }
        Ok(Self {
            object_type: object_type.to_string(),
            object_id: object_id.to_string(),
        })
    }

    pub(crate) fn as_raw(&self) -> String {
        format!("{}:{}", self.object_type, self.object_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum UserRef {
    Object(ObjectRef),
    Userset { object: ObjectRef, relation: String },
    Wildcard { object_type: String },
}

impl UserRef {
    pub(crate) fn parse(raw: &str) -> Result<Self, FgaError> {
        if let Some(prefix) = raw.strip_suffix(":*") {
            if prefix.is_empty() {
                return Err(FgaError::BadRequest(format!(
                    "invalid user reference {raw}"
                )));
            }
            return Ok(Self::Wildcard {
                object_type: prefix.to_string(),
            });
        }
        let (base, relation) = match raw.split_once('#') {
            Some((base, relation)) => (base, Some(relation.to_string())),
            None => (raw, None),
        };
        let object = ObjectRef::parse(base)?;
        Ok(match relation {
            Some(relation) => Self::Userset { object, relation },
            None => Self::Object(object),
        })
    }

    pub(crate) fn as_raw(&self) -> String {
        match self {
            Self::Object(object) => object.as_raw(),
            Self::Userset { object, relation } => format!("{}#{relation}", object.as_raw()),
            Self::Wildcard { object_type } => format!("{object_type}:*"),
        }
    }

    pub(crate) fn user_type(&self) -> &str {
        match self {
            Self::Object(object) | Self::Userset { object, .. } => &object.object_type,
            Self::Wildcard { object_type } => object_type,
        }
    }

    pub(crate) fn user_id(&self) -> &str {
        match self {
            Self::Object(object) | Self::Userset { object, .. } => &object.object_id,
            Self::Wildcard { .. } => "*",
        }
    }

    pub(crate) fn relation_name(&self) -> Option<&str> {
        match self {
            Self::Userset { relation, .. } => Some(relation.as_str()),
            _ => None,
        }
    }

    pub(crate) fn matches(&self, candidate: &UserRef) -> bool {
        match self {
            Self::Object(object) => matches!(candidate, Self::Object(other) if other == object),
            Self::Userset { object, relation } => {
                matches!(candidate, Self::Userset { object: other, relation: other_relation } if other == object && other_relation == relation)
            }
            Self::Wildcard { object_type } => candidate.user_type() == object_type,
        }
    }
}

pub(crate) fn stored_user_from_parts(user_type: &str, user_id: &str, user_relation: &str) -> UserRef {
    if user_id == "*" {
        return UserRef::Wildcard {
            object_type: user_type.to_string(),
        };
    }
    let object = ObjectRef {
        object_type: user_type.to_string(),
        object_id: user_id.to_string(),
    };
    if user_relation.is_empty() {
        UserRef::Object(object)
    } else {
        UserRef::Userset {
            object,
            relation: user_relation.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ParsedTupleKey {
    pub(crate) user: UserRef,
    pub(crate) relation: String,
    pub(crate) object: ObjectRef,
    pub(crate) condition: Option<RelationshipCondition>,
}

impl ParsedTupleKey {
    pub(crate) fn parse(tuple: TupleKey) -> Result<Self, FgaError> {
        Ok(Self {
            user: UserRef::parse(&tuple.user)?,
            relation: tuple.relation,
            object: ObjectRef::parse(&tuple.object)?,
            condition: tuple.condition,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StoredTuple {
    pub(crate) user: UserRef,
    pub(crate) raw_user: String,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredModelFragments {
    pub(crate) core_model_version: String,
    pub(crate) custom_model: String,
    pub(crate) module_fragments: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CachedModel {
    pub(crate) model_id: String,
    pub(crate) raw: String,
    pub(crate) created_at: String,
    pub(crate) core_model_version: String,
    pub(crate) compiled: std::sync::Arc<CompiledModel>,
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledModel {
    pub(crate) schema_version: String,
    pub(crate) raw_types: HashMap<String, serde_json::Value>,
    pub(crate) types: HashMap<String, CompiledType>,
}

impl CompiledModel {
    pub(crate) fn from_request(request: &AuthorizationModelWriteRequest) -> Result<Self, FgaError> {
        if request.schema_version != crate::SCHEMA_VERSION_1_1 {
            return Err(FgaError::BadRequest(format!(
                "schema_version {} is not supported",
                request.schema_version
            )));
        }
        if !request.conditions.is_empty() {
            return Err(FgaError::Unsupported(
                "conditions are not supported by the embedded v1 server".into(),
            ));
        }

        let mut types = HashMap::new();
        let mut raw_types = HashMap::new();
        for type_def in &request.type_definitions {
            if raw_types.contains_key(&type_def.type_name) {
                return Err(FgaError::BadRequest(format!(
                    "duplicate type definition {}",
                    type_def.type_name
                )));
            }
            let metadata = crate::core_model::parse_relation_metadata(type_def.metadata.as_ref())?;
            let mut relations = HashMap::new();
            for (relation, expr) in &type_def.relations {
                let expr = crate::core_model::parse_relation_expr(expr)?;
                let semantics = CompiledRelationSemantics {
                    allows_direct_tuples: expr.contains_this_leaf(),
                    allowed_direct_users: metadata.get(relation).cloned().unwrap_or_default(),
                };
                if metadata.contains_key(relation) && !semantics.allows_direct_tuples {
                    return Err(FgaError::BadRequest(format!(
                        "relation {}#{} cannot declare directly_related_user_types without a direct this branch",
                        type_def.type_name, relation
                    )));
                }
                relations.insert(relation.clone(), CompiledRelation { expr, semantics });
            }
            raw_types.insert(
                type_def.type_name.clone(),
                serde_json::to_value(type_def).map_err(|e| FgaError::Internal(e.into()))?,
            );
            types.insert(type_def.type_name.clone(), CompiledType { relations });
        }

        Ok(Self {
            schema_version: request.schema_version.clone(),
            raw_types,
            types,
        })
    }

    pub(crate) fn relation_expr(&self, object_type: &str, relation: &str) -> Result<&RelationExpr, FgaError> {
        self.types
            .get(object_type)
            .and_then(|type_def| type_def.relations.get(relation))
            .map(|compiled| &compiled.expr)
            .ok_or_else(|| {
                FgaError::BadRequest(format!(
                    "relation {relation} is not defined on type {object_type}"
                ))
            })
    }

    pub(crate) fn relation_semantics(
        &self,
        object_type: &str,
        relation: &str,
    ) -> Result<&CompiledRelationSemantics, FgaError> {
        self.types
            .get(object_type)
            .and_then(|type_def| type_def.relations.get(relation))
            .map(|compiled| &compiled.semantics)
            .ok_or_else(|| {
                FgaError::BadRequest(format!(
                    "relation {relation} is not defined on type {object_type}"
                ))
            })
    }

    pub(crate) fn list_plan(&self, object_type: &str, relation: &str) -> Result<ListPlan, FgaError> {
        Ok(match self.relation_expr(object_type, relation)? {
            RelationExpr::This => ListPlan::Planned {
                sources: vec![CandidateSource::Direct],
            },
            RelationExpr::ComputedUserset { relation } => ListPlan::Planned {
                sources: vec![CandidateSource::ComputedUserset {
                    relation: relation.clone(),
                }],
            },
            RelationExpr::TupleToUserset {
                tupleset,
                computed_userset,
            } => ListPlan::Planned {
                sources: vec![CandidateSource::TupleToUserset {
                    tupleset: tupleset.clone(),
                    computed_userset: computed_userset.clone(),
                }],
            },
            RelationExpr::Union { children } => {
                let mut sources = Vec::new();
                for child in children {
                    let ListPlan::Planned {
                        sources: child_sources,
                    } = Self::plan_from_expr(child)
                    else {
                        return Ok(ListPlan::ScanFallback);
                    };
                    sources.extend(child_sources);
                }
                ListPlan::Planned { sources }
            }
            RelationExpr::Intersection { children } => children
                .iter()
                .find_map(|child| match Self::plan_from_expr(child) {
                    ListPlan::Planned { sources } => Some(ListPlan::Planned { sources }),
                    ListPlan::ScanFallback => None,
                })
                .unwrap_or(ListPlan::ScanFallback),
            RelationExpr::Difference { base, .. } => Self::plan_from_expr(base),
        })
    }

    fn plan_from_expr(expr: &RelationExpr) -> ListPlan {
        match expr {
            RelationExpr::This => ListPlan::Planned {
                sources: vec![CandidateSource::Direct],
            },
            RelationExpr::ComputedUserset { relation } => ListPlan::Planned {
                sources: vec![CandidateSource::ComputedUserset {
                    relation: relation.clone(),
                }],
            },
            RelationExpr::TupleToUserset {
                tupleset,
                computed_userset,
            } => ListPlan::Planned {
                sources: vec![CandidateSource::TupleToUserset {
                    tupleset: tupleset.clone(),
                    computed_userset: computed_userset.clone(),
                }],
            },
            RelationExpr::Union { children } => {
                let mut sources = Vec::new();
                for child in children {
                    let ListPlan::Planned {
                        sources: child_sources,
                    } = Self::plan_from_expr(child)
                    else {
                        return ListPlan::ScanFallback;
                    };
                    sources.extend(child_sources);
                }
                ListPlan::Planned { sources }
            }
            RelationExpr::Intersection { children } => children
                .iter()
                .find_map(|child| match Self::plan_from_expr(child) {
                    ListPlan::Planned { sources } => Some(ListPlan::Planned { sources }),
                    ListPlan::ScanFallback => None,
                })
                .unwrap_or(ListPlan::ScanFallback),
            RelationExpr::Difference { base, .. } => Self::plan_from_expr(base),
        }
    }

    pub(crate) fn validate_tuple(&self, tuple: &ParsedTupleKey) -> Result<(), FgaError> {
        let semantics = self.relation_semantics(&tuple.object.object_type, &tuple.relation)?;
        if !semantics.allows_direct_tuples {
            return Err(FgaError::BadRequest(format!(
                "relation {}#{} is computed-only and cannot accept direct tuples",
                tuple.object.object_type, tuple.relation
            )));
        }
        if !semantics.allowed_direct_users.is_empty()
            && !semantics
                .allowed_direct_users
                .iter()
                .any(|candidate| candidate.matches(&tuple.user))
        {
            return Err(FgaError::BadRequest(format!(
                "user {} cannot be directly related to {}#{}",
                tuple.user.as_raw(),
                tuple.object.as_raw(),
                tuple.relation
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledType {
    pub(crate) relations: HashMap<String, CompiledRelation>,
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledRelation {
    pub(crate) expr: RelationExpr,
    pub(crate) semantics: CompiledRelationSemantics,
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledRelationSemantics {
    pub(crate) allows_direct_tuples: bool,
    pub(crate) allowed_direct_users: Vec<AllowedDirectUser>,
}

#[derive(Clone, Debug)]
pub(crate) struct AllowedDirectUser {
    pub(crate) user_type: String,
    pub(crate) relation: Option<String>,
    pub(crate) wildcard: bool,
}

impl AllowedDirectUser {
    pub(crate) fn matches(&self, user: &UserRef) -> bool {
        if self.user_type != user.user_type() {
            return false;
        }
        match user {
            UserRef::Object(_) => self.relation.is_none() && !self.wildcard,
            UserRef::Userset { relation, .. } => self.relation.as_deref() == Some(relation),
            UserRef::Wildcard { .. } => self.wildcard,
        }
    }

    pub(crate) fn is_userset(&self) -> bool {
        self.relation.is_some()
    }
}

#[derive(Clone, Debug)]
pub(crate) enum RelationExpr {
    This,
    ComputedUserset {
        relation: String,
    },
    TupleToUserset {
        tupleset: String,
        computed_userset: String,
    },
    Union {
        children: Vec<RelationExpr>,
    },
    Intersection {
        children: Vec<RelationExpr>,
    },
    Difference {
        base: Box<RelationExpr>,
        subtract: Box<RelationExpr>,
    },
}

impl RelationExpr {
    pub(crate) fn contains_this_leaf(&self) -> bool {
        match self {
            Self::This => true,
            Self::ComputedUserset { .. } => false,
            Self::TupleToUserset { .. } => false,
            Self::Union { children } | Self::Intersection { children } => {
                children.iter().any(Self::contains_this_leaf)
            }
            Self::Difference { base, subtract } => {
                base.contains_this_leaf() || subtract.contains_this_leaf()
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ListPlan {
    Planned { sources: Vec<CandidateSource> },
    ScanFallback,
}

#[derive(Clone, Debug)]
pub(crate) enum CandidateSource {
    Direct,
    ComputedUserset {
        relation: String,
    },
    TupleToUserset {
        tupleset: String,
        computed_userset: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DenyReason {
    NoMatch,
    CycleDetected,
    DepthExhausted,
}

impl DenyReason {
    pub(crate) fn prefer(self, other: Self) -> Self {
        if other.priority() > self.priority() {
            other
        } else {
            self
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::NoMatch => 0,
            Self::CycleDetected => 1,
            Self::DepthExhausted => 2,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NoMatch => "no_match",
            Self::CycleDetected => "cycle_detected",
            Self::DepthExhausted => "depth_exhausted",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvalOutcome {
    Allow,
    Deny(DenyReason),
}

impl EvalOutcome {
    pub(crate) fn is_allowed(self) -> bool {
        matches!(self, Self::Allow)
    }

    pub(crate) fn deny_reason(self) -> Option<DenyReason> {
        match self {
            Self::Allow => None,
            Self::Deny(reason) => Some(reason),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EvalIssue {
    pub(crate) reason: DenyReason,
    pub(crate) user: String,
    pub(crate) relation: String,
    pub(crate) object: String,
}

pub(crate) struct EvaluatorContext<'a> {
    pub(crate) service: &'a FgaService,
    pub(crate) instance_id: &'a str,
    pub(crate) store_id: &'a str,
    pub(crate) model: &'a CompiledModel,
    pub(crate) contextual: &'a [ParsedTupleKey],
    pub(crate) tuple_cache: HashMap<(String, String), Vec<StoredTuple>>,
    pub(crate) decision_cache: HashMap<(String, String, String), EvalOutcome>,
    pub(crate) active: HashSet<(String, String, String)>,
    pub(crate) max_depth: usize,
    pub(crate) request_issue: Option<EvalIssue>,
}

impl<'a> EvaluatorContext<'a> {
    pub(crate) async fn tuples_for(
        &mut self,
        object: &ObjectRef,
        relation: &str,
    ) -> Result<Vec<StoredTuple>, FgaError> {
        let key = (object.as_raw(), relation.to_string());
        if let Some(cached) = self.tuple_cache.get(&key) {
            return Ok(cached.clone());
        }
        let tuples = self
            .service
            .load_direct_tuples(
                self.instance_id,
                self.store_id,
                object,
                relation,
                self.contextual,
            )
            .await?;
        self.tuple_cache.insert(key, tuples.clone());
        Ok(tuples)
    }

    pub(crate) async fn check(
        &mut self,
        user: &UserRef,
        relation: &str,
        object: &ObjectRef,
        depth: usize,
    ) -> Result<EvalOutcome, FgaError> {
        if depth > self.max_depth {
            return Ok(EvalOutcome::Deny(DenyReason::DepthExhausted));
        }
        let key = (user.as_raw(), relation.to_string(), object.as_raw());
        if let Some(cached) = self.decision_cache.get(&key) {
            return Ok(*cached);
        }
        if !self.active.insert(key.clone()) {
            return Ok(EvalOutcome::Deny(DenyReason::CycleDetected));
        }
        let expr = self
            .model
            .relation_expr(&object.object_type, relation)?
            .clone();
        let allowed = self.eval_expr(user, relation, object, &expr, depth).await;
        self.active.remove(&key);
        let allowed = allowed?;
        self.decision_cache.insert(key, allowed);
        Ok(allowed)
    }

    #[async_recursion]
    async fn eval_expr(
        &mut self,
        user: &UserRef,
        relation: &str,
        object: &ObjectRef,
        expr: &RelationExpr,
        depth: usize,
    ) -> Result<EvalOutcome, FgaError> {
        match expr {
            RelationExpr::This => {
                let mut denied = DenyReason::NoMatch;
                for tuple in self.tuples_for(object, relation).await? {
                    match &tuple.user {
                        UserRef::Object(_) | UserRef::Wildcard { .. } => {
                            if tuple.user.matches(user) {
                                return Ok(EvalOutcome::Allow);
                            }
                        }
                        UserRef::Userset {
                            object: user_object,
                            relation: user_relation,
                        } => {
                            let outcome = self
                                .check(user, user_relation, user_object, depth + 1)
                                .await?;
                            if outcome.is_allowed() {
                                return Ok(EvalOutcome::Allow);
                            }
                            if let Some(reason) = outcome.deny_reason() {
                                denied = denied.prefer(reason);
                            }
                        }
                    }
                }
                Ok(EvalOutcome::Deny(denied))
            }
            RelationExpr::ComputedUserset { relation } => {
                self.check(user, relation, object, depth + 1).await
            }
            RelationExpr::TupleToUserset {
                tupleset,
                computed_userset,
            } => {
                let mut denied = DenyReason::NoMatch;
                for tuple in self.tuples_for(object, tupleset).await? {
                    if let UserRef::Object(target) = &tuple.user {
                        let outcome = self
                            .check(user, computed_userset, target, depth + 1)
                            .await?;
                        if outcome.is_allowed() {
                            return Ok(EvalOutcome::Allow);
                        }
                        if let Some(reason) = outcome.deny_reason() {
                            denied = denied.prefer(reason);
                        }
                    }
                }
                Ok(EvalOutcome::Deny(denied))
            }
            RelationExpr::Union { children } => {
                let mut denied = DenyReason::NoMatch;
                for child in children {
                    let outcome = self
                        .eval_expr(user, relation, object, child, depth + 1)
                        .await?;
                    if outcome.is_allowed() {
                        return Ok(EvalOutcome::Allow);
                    }
                    if let Some(reason) = outcome.deny_reason() {
                        denied = denied.prefer(reason);
                    }
                }
                Ok(EvalOutcome::Deny(denied))
            }
            RelationExpr::Intersection { children } => {
                let mut denied = DenyReason::NoMatch;
                let mut all_allowed = true;
                for child in children {
                    let outcome = self
                        .eval_expr(user, relation, object, child, depth + 1)
                        .await?;
                    if !outcome.is_allowed() {
                        all_allowed = false;
                        if let Some(reason) = outcome.deny_reason() {
                            denied = denied.prefer(reason);
                        }
                    }
                }
                if all_allowed {
                    Ok(EvalOutcome::Allow)
                } else {
                    Ok(EvalOutcome::Deny(denied))
                }
            }
            RelationExpr::Difference { base, subtract } => {
                let base = self
                    .eval_expr(user, relation, object, base, depth + 1)
                    .await?;
                if !base.is_allowed() {
                    return Ok(base);
                }
                let subtract = self
                    .eval_expr(user, relation, object, subtract, depth + 1)
                    .await?;
                match subtract {
                    EvalOutcome::Allow => Ok(EvalOutcome::Deny(DenyReason::NoMatch)),
                    EvalOutcome::Deny(DenyReason::NoMatch) => Ok(EvalOutcome::Allow),
                    EvalOutcome::Deny(reason) => Ok(EvalOutcome::Deny(reason)),
                }
            }
        }
    }

    pub(crate) fn record_request_issue(
        &mut self,
        user: &UserRef,
        relation: &str,
        object: &ObjectRef,
        outcome: EvalOutcome,
    ) {
        let Some(reason) = outcome.deny_reason() else {
            return;
        };
        if matches!(reason, DenyReason::NoMatch) || self.request_issue.is_some() {
            return;
        }
        self.request_issue = Some(EvalIssue {
            reason,
            user: user.as_raw(),
            relation: relation.to_string(),
            object: object.as_raw(),
        });
    }

    pub(crate) fn warn_if_needed(&self, model_id: &str) {
        let Some(issue) = &self.request_issue else {
            return;
        };
        tracing::warn!(
            instance_id = self.instance_id,
            store_id = self.store_id,
            model_id,
            user = %issue.user,
            relation = %issue.relation,
            object = %issue.object,
            deny_reason = issue.reason.as_str(),
            max_depth = self.max_depth,
            "fga evaluation denied due to evaluator guardrail"
        );
    }

    #[async_recursion]
    pub(crate) async fn expand(
        &mut self,
        object: &ObjectRef,
        relation: &str,
        depth: usize,
    ) -> Result<ExpandNode, FgaError> {
        if depth > self.max_depth {
            return Ok(ExpandNode {
                name: format!("{}#{relation}", object.as_raw()),
                children: Vec::new(),
                users: vec!["depth_limit".into()],
            });
        }
        let expr = self
            .model
            .relation_expr(&object.object_type, relation)?
            .clone();
        self.expand_expr(object, relation, &expr, depth).await
    }

    #[async_recursion]
    async fn expand_expr(
        &mut self,
        object: &ObjectRef,
        relation: &str,
        expr: &RelationExpr,
        depth: usize,
    ) -> Result<ExpandNode, FgaError> {
        let name = format!("{}#{relation}", object.as_raw());
        match expr {
            RelationExpr::This => {
                let tuples = self.tuples_for(object, relation).await?;
                let mut children = Vec::new();
                let mut users = Vec::new();
                for tuple in tuples {
                    match &tuple.user {
                        UserRef::Userset {
                            object: user_object,
                            relation: user_relation,
                        } => {
                            children
                                .push(self.expand(user_object, user_relation, depth + 1).await?);
                        }
                        _ => users.push(tuple.raw_user),
                    }
                }
                Ok(ExpandNode {
                    name,
                    children,
                    users,
                })
            }
            RelationExpr::ComputedUserset { relation: target } => Ok(ExpandNode {
                name,
                children: vec![self.expand(object, target, depth + 1).await?],
                users: Vec::new(),
            }),
            RelationExpr::TupleToUserset {
                tupleset,
                computed_userset,
            } => {
                let tuples = self.tuples_for(object, tupleset).await?;
                let mut children = Vec::new();
                for tuple in tuples {
                    if let UserRef::Object(target) = &tuple.user {
                        children.push(self.expand(target, computed_userset, depth + 1).await?);
                    }
                }
                Ok(ExpandNode {
                    name,
                    children,
                    users: Vec::new(),
                })
            }
            RelationExpr::Union { children } => {
                let mut expanded = Vec::new();
                for child in children {
                    expanded.push(self.expand_expr(object, relation, child, depth + 1).await?);
                }
                Ok(ExpandNode {
                    name,
                    children: expanded,
                    users: Vec::new(),
                })
            }
            RelationExpr::Intersection { children } => {
                let mut expanded = Vec::new();
                for child in children {
                    expanded.push(self.expand_expr(object, relation, child, depth + 1).await?);
                }
                Ok(ExpandNode {
                    name,
                    children: expanded,
                    users: Vec::new(),
                })
            }
            RelationExpr::Difference { base, subtract } => Ok(ExpandNode {
                name,
                children: vec![
                    self.expand_expr(object, relation, base, depth + 1).await?,
                    self.expand_expr(object, relation, subtract, depth + 1)
                        .await?,
                ],
                users: Vec::new(),
            }),
        }
    }
}

pub(crate) fn collect_graph_edges(
    type_name: &str,
    relation: &str,
    expr: &RelationExpr,
    edges: &mut Vec<ModelGraphEdge>,
) {
    match expr {
        RelationExpr::This => {}
        RelationExpr::ComputedUserset { relation: target } => {
            edges.push(ModelGraphEdge {
                from: type_name.to_string(),
                to: type_name.to_string(),
                relation: format!("{relation} -> {target}"),
                kind: "computed_userset".into(),
            });
        }
        RelationExpr::TupleToUserset {
            tupleset,
            computed_userset,
        } => edges.push(ModelGraphEdge {
            from: type_name.to_string(),
            to: type_name.to_string(),
            relation: format!("{tupleset}->{computed_userset}"),
            kind: "tuple_to_userset".into(),
        }),
        RelationExpr::Union { children } | RelationExpr::Intersection { children } => {
            for child in children {
                collect_graph_edges(type_name, relation, child, edges);
            }
        }
        RelationExpr::Difference { base, subtract } => {
            collect_graph_edges(type_name, relation, base, edges);
            collect_graph_edges(type_name, relation, subtract, edges);
        }
    }
}

pub(crate) fn parse_contextual(contextual: Option<ContextualTuples>) -> Result<Vec<ParsedTupleKey>, FgaError> {
    contextual
        .map(|tuples| {
            tuples
                .tuple_keys
                .into_iter()
                .map(ParsedTupleKey::parse)
                .collect()
        })
        .transpose()
        .map(|value| value.unwrap_or_default())
}

pub(crate) fn decode_offset(token: Option<&str>) -> Result<i64, FgaError> {
    match token {
        None | Some("") => Ok(0),
        Some(token) => token
            .parse::<i64>()
            .map_err(|_| FgaError::BadRequest("invalid continuation token".into())),
    }
}

pub(crate) fn user_matches_filter(user: &UserRef, filter: &UserFilter) -> bool {
    user.user_type() == filter.user_type
        && filter
            .relation
            .as_deref()
            .is_none_or(|relation| user.relation_name() == Some(relation))
}

pub(crate) fn validate_duplicate_request_tuples(request: &WriteRequest) -> Result<(), FgaError> {
    let mut seen = HashSet::new();
    for tuple in &request.writes.tuple_keys {
        let key = (&tuple.user, &tuple.relation, &tuple.object);
        if !seen.insert((key.0.clone(), key.1.clone(), key.2.clone(), "write")) {
            return Err(FgaError::BadRequest("duplicate tuple in writes".into()));
        }
    }
    let mut deletes = HashSet::new();
    for tuple in &request.deletes.tuple_keys {
        let key = (&tuple.user, &tuple.relation, &tuple.object);
        if !deletes.insert((key.0.clone(), key.1.clone(), key.2.clone(), "delete")) {
            return Err(FgaError::BadRequest("duplicate tuple in deletes".into()));
        }
        if seen.contains(&(key.0.clone(), key.1.clone(), key.2.clone(), "write")) {
            return Err(FgaError::BadRequest(
                "cannot write and delete the same tuple in one request".into(),
            ));
        }
    }
    Ok(())
}
