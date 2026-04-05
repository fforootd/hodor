use anyhow::Context;
use google_cloud_spanner::{
    client::Error as SpannerError, row::Row as SpannerRow, statement::Statement,
};
use serde_json::Value;
use sqlx::{Any, QueryBuilder};
use uuid::Uuid;
use zitadel_app::{
    event::DomainEvent,
    repo::{
        ActionRecord, ActionRepository, BoxFuture, CreatedSession, CredentialRepository,
        EventQueryParams, EventRecord, EventRepository, FgaRelation, FgaRepository,
        LinkedIdentityRecord, ListParams, ListResult, LoginFlowRecord, LoginFlowRepository,
        OidcAuthRequest, OidcClientInfo, OidcRepository, PatRecord, PatRepository, ResolvedPat,
        SessionInfo, SessionRepository, UserClaims,
    },
};
use zitadel_crypto::token_hash;

use crate::{
    Db, Dialect, delete_instance_row, find_linked_identity, get_action, get_login_flow_record,
    get_oidc_client_record, list_actions, list_login_flow_records, load_user_claims_record,
    replace_password_credential,
};

const DEFAULT_SESSION_MAX_AGE_SECS: u64 = 86_400;

type EventRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    String,
);

#[derive(Clone)]
pub struct DbCredentialRepository {
    db: Db,
}

impl DbCredentialRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbPatRepository {
    db: Db,
}

impl DbPatRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbSessionRepository {
    db: Db,
    session_max_age_secs: u64,
}

impl DbSessionRepository {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            session_max_age_secs: DEFAULT_SESSION_MAX_AGE_SECS,
        }
    }

    pub fn with_max_age_secs(db: Db, session_max_age_secs: u64) -> Self {
        Self {
            db,
            session_max_age_secs,
        }
    }
}

#[derive(Clone)]
pub struct DbLoginFlowRepository {
    db: Db,
}

impl DbLoginFlowRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbEventRepository {
    db: Db,
}

impl DbEventRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbOidcRepository {
    db: Db,
}

impl DbOidcRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbFgaRepository {
    db: Db,
}

impl DbFgaRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct DbActionRepository {
    db: Db,
}

impl DbActionRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl CredentialRepository for DbCredentialRepository {
    fn set_password(
        &self,
        instance_id: &str,
        user_id: &str,
        password_hash: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user_id = user_id.to_string();
        let password_hash = password_hash.to_string();
        Box::pin(async move {
            replace_password_credential(
                &db,
                &instance_id,
                &user_id,
                &Uuid::now_v7().to_string(),
                &serde_json::json!({ "hash": password_hash }).to_string(),
            )
            .await
        })
    }

    fn get_password_hash(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<String>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move { load_password_hash(&db, &instance_id, &user_id).await })
    }

    fn link_identity(
        &self,
        instance_id: &str,
        link: &LinkedIdentityRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let link = link.clone();
        Box::pin(async move {
            create_linked_identity(
                &db,
                &instance_id,
                &link.id,
                &link.user_id,
                &link.provider_id,
                &link.external_sub,
                link.external_email.as_deref().unwrap_or_default(),
                &link.raw_claims,
            )
            .await
        })
    }

    fn unlink_identity(
        &self,
        instance_id: &str,
        user_id: &str,
        provider_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user_id = user_id.to_string();
        let provider_id = provider_id.to_string();
        Box::pin(async move { unlink_identity(&db, &instance_id, &user_id, &provider_id).await })
    }

    fn list_linked_identities(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<LinkedIdentityRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move { list_linked_identities(&db, &instance_id, &user_id).await })
    }

    fn find_by_external_sub(
        &self,
        instance_id: &str,
        provider_id: &str,
        external_sub: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<LinkedIdentityRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let provider_id = provider_id.to_string();
        let external_sub = external_sub.to_string();
        Box::pin(async move {
            Ok(
                find_linked_identity(&db, &instance_id, &provider_id, &external_sub)
                    .await?
                    .map(linked_identity_from_retained),
            )
        })
    }
}

impl PatRepository for DbPatRepository {
    fn create(
        &self,
        instance_id: &str,
        pat: &PatRecord,
        token_hash: &str,
    ) -> BoxFuture<'_, anyhow::Result<String>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let pat = pat.clone();
        let token_hash = token_hash.to_string();
        Box::pin(async move {
            create_pat_record(&db, &instance_id, &pat, &token_hash).await?;
            Ok(pat.id)
        })
    }

    fn get(
        &self,
        instance_id: &str,
        pat_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<PatRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let pat_id = pat_id.to_string();
        Box::pin(async move { get_pat_record(&db, &instance_id, &pat_id).await })
    }

    fn list(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<PatRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move { list_pat_records(&db, &instance_id, &user_id).await })
    }

    fn revoke(&self, instance_id: &str, pat_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let pat_id = pat_id.to_string();
        Box::pin(async move {
            revoke_pat_record(&db, &instance_id, &pat_id).await?;
            Ok(())
        })
    }

    fn resolve_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<ResolvedPat>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let raw_token = raw_token.to_string();
        Box::pin(async move { resolve_pat(&db, &instance_id, &raw_token).await })
    }
}

impl SessionRepository for DbSessionRepository {
    fn create(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        auth_method: &str,
    ) -> BoxFuture<'_, anyhow::Result<CreatedSession>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user_id = user_id.to_string();
        let org_id = org_id.to_string();
        let auth_method = auth_method.to_string();
        let session_max_age_secs = self.session_max_age_secs;
        Box::pin(async move {
            create_session_record(
                &db,
                &instance_id,
                &user_id,
                &org_id,
                &auth_method,
                session_max_age_secs,
            )
            .await
        })
    }

    fn find_by_token(
        &self,
        instance_id: &str,
        token: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SessionInfo>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let token = token.to_string();
        Box::pin(async move { find_session_by_token(&db, &instance_id, &token).await })
    }

    fn revoke(&self, instance_id: &str, session_id: &str) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let session_id = session_id.to_string();
        Box::pin(async move { revoke_session_record(&db, &instance_id, &session_id).await })
    }
}

impl LoginFlowRepository for DbLoginFlowRepository {
    fn get_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<LoginFlowRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let flow_id = flow_id.to_string();
        Box::pin(async move {
            Ok(get_login_flow_record(&db, &instance_id, &flow_id)
                .await?
                .map(login_flow_from_retained))
        })
    }

    fn list_flows(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Vec<LoginFlowRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            Ok(list_login_flow_records(&db, &instance_id, "", i64::MAX)
                .await?
                .into_iter()
                .map(login_flow_from_retained)
                .collect())
        })
    }

    fn upsert_flow(
        &self,
        instance_id: &str,
        flow: &LoginFlowRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let flow = flow.clone();
        Box::pin(async move { upsert_login_flow(&db, &instance_id, &flow).await })
    }

    fn delete_flow(&self, instance_id: &str, flow_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let flow_id = flow_id.to_string();
        Box::pin(async move {
            delete_instance_row(&db, &instance_id, "login_flows", &flow_id).await?;
            Ok(())
        })
    }
}

impl OidcRepository for DbOidcRepository {
    fn find_client(
        &self,
        instance_id: &str,
        client_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OidcClientInfo>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let client_id = client_id.to_string();
        Box::pin(async move {
            Ok(get_oidc_client_record(&db, &instance_id, &client_id)
                .await?
                .map(|record| OidcClientInfo {
                    client_id,
                    client_secret: non_empty(Some(record.client_secret)),
                    redirect_uris: parse_string_list(&record.redirect_uris_json),
                    grant_types: parse_string_list(&record.grant_types_json),
                    response_types: parse_string_list(&record.response_types_json),
                    state: record.state,
                }))
        })
    }

    fn create_auth_request(
        &self,
        instance_id: &str,
        request: &OidcAuthRequest,
    ) -> BoxFuture<'_, anyhow::Result<String>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let request = request.clone();
        Box::pin(async move { create_oidc_auth_request(&db, &instance_id, &request).await })
    }

    fn consume_auth_code(
        &self,
        instance_id: &str,
        code: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OidcAuthRequest>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let code = code.to_string();
        Box::pin(async move { consume_oidc_auth_code(&db, &instance_id, &code).await })
    }

    fn load_user_claims(
        &self,
        instance_id: &str,
        subject: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserClaims>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let subject = subject.to_string();
        Box::pin(async move {
            Ok(load_user_claims_record(&db, &instance_id, &subject)
                .await?
                .map(|record| UserClaims {
                    sub: subject,
                    name: non_empty(Some(record.display_name)),
                    email: non_empty(Some(record.identifier.clone())),
                    email_verified: Some(!record.identifier.is_empty()),
                }))
        })
    }
}

impl EventRepository for DbEventRepository {
    fn append(
        &self,
        instance_id: &str,
        event: &DomainEvent,
        request_id: Option<&str>,
        session_id: Option<&str>,
        flow_id: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let event = event.clone();
        let request_id = request_id.map(ToString::to_string);
        let session_id = session_id.map(ToString::to_string);
        let flow_id = flow_id.map(ToString::to_string);
        Box::pin(async move {
            append_domain_event(
                &db,
                &instance_id,
                &event,
                request_id.as_deref(),
                session_id.as_deref(),
                flow_id.as_deref(),
            )
            .await
        })
    }

    fn list(
        &self,
        instance_id: &str,
        params: &EventQueryParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<EventRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let params = params.clone();
        Box::pin(async move { list_domain_events(&db, &instance_id, &params).await })
    }
}

impl FgaRepository for DbFgaRepository {
    fn check(
        &self,
        instance_id: &str,
        user: &str,
        relation: &str,
        object: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user = user.to_string();
        let relation = relation.to_string();
        let object = object.to_string();
        Box::pin(async move { fga_check(&db, &instance_id, &user, &relation, &object).await })
    }

    fn write_tuple(
        &self,
        instance_id: &str,
        user: &str,
        relation: &str,
        object: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user = user.to_string();
        let relation = relation.to_string();
        let object = object.to_string();
        Box::pin(async move { write_fga_tuple(&db, &instance_id, &user, &relation, &object).await })
    }

    fn delete_tuple(
        &self,
        instance_id: &str,
        user: &str,
        relation: &str,
        object: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user = user.to_string();
        let relation = relation.to_string();
        let object = object.to_string();
        Box::pin(
            async move { delete_fga_tuple(&db, &instance_id, &user, &relation, &object).await },
        )
    }

    fn list_relations(
        &self,
        instance_id: &str,
        user: &str,
        object_type: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<FgaRelation>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let user = user.to_string();
        let object_type = object_type.to_string();
        Box::pin(async move { list_fga_relations(&db, &instance_id, &user, &object_type).await })
    }
}

impl ActionRepository for DbActionRepository {
    fn list(
        &self,
        instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<ActionRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let params = params.clone();
        Box::pin(async move {
            let mut actions = list_actions(&db, &instance_id)
                .await?
                .into_iter()
                .map(action_from_retained)
                .collect::<Vec<_>>();
            if let Some(search) = params.search.as_deref().map(str::to_lowercase) {
                actions.retain(|action| {
                    action.name.to_lowercase().contains(&search)
                        || action.hook.to_lowercase().contains(&search)
                        || action.action_type.to_lowercase().contains(&search)
                });
            }
            Ok(paginate_by_id(actions, &params))
        })
    }

    fn get(
        &self,
        instance_id: &str,
        action_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<ActionRecord>>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let action_id = action_id.to_string();
        Box::pin(async move {
            Ok(get_action(&db, &instance_id, &action_id)
                .await?
                .map(action_from_retained))
        })
    }

    fn create(
        &self,
        instance_id: &str,
        action: &ActionRecord,
    ) -> BoxFuture<'_, anyhow::Result<ActionRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let action = action.clone();
        Box::pin(async move {
            upsert_action(&db, &instance_id, &action, false).await?;
            Ok(get_action(&db, &instance_id, &action.id)
                .await?
                .map(action_from_retained)
                .unwrap_or(action))
        })
    }

    fn update(
        &self,
        instance_id: &str,
        action: &ActionRecord,
    ) -> BoxFuture<'_, anyhow::Result<ActionRecord>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let action = action.clone();
        Box::pin(async move {
            upsert_action(&db, &instance_id, &action, true).await?;
            Ok(get_action(&db, &instance_id, &action.id)
                .await?
                .map(action_from_retained)
                .unwrap_or(action))
        })
    }

    fn delete(&self, instance_id: &str, action_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.db.clone();
        let instance_id = instance_id.to_string();
        let action_id = action_id.to_string();
        Box::pin(async move {
            delete_instance_row(&db, &instance_id, "actions", &action_id).await?;
            Ok(())
        })
    }
}

fn parse_json(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::Object(Default::default()))
}

fn parse_string_list(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|raw| !raw.is_empty())
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

fn linked_identity_from_retained(
    record: crate::retained::LinkedIdentityRecord,
) -> LinkedIdentityRecord {
    LinkedIdentityRecord {
        id: record.id,
        user_id: record.user_id,
        provider_id: record.provider_id,
        external_sub: record.external_sub,
        external_email: non_empty(Some(record.external_email)),
        raw_claims: parse_json(&record.raw_claims_json),
    }
}

fn login_flow_from_retained(record: crate::retained::LoginFlowRecord) -> LoginFlowRecord {
    LoginFlowRecord {
        id: record.id,
        name: record.name,
        strategy: record.strategy,
        state: record.state,
        is_default: record.is_default,
        enabled: record.enabled,
        priority: clamp_i64_to_i32(record.priority),
        config: parse_json(&record.config_json),
        audience: parse_json(&record.audience_json),
        auth_methods: parse_json(&record.auth_methods_json),
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn action_from_retained(record: crate::retained::ActionRecord) -> ActionRecord {
    ActionRecord {
        id: record.id,
        org_id: record.org_id,
        name: record.name,
        hook: record.hook,
        action_type: record.action_type,
        trigger_expr: record.trigger_expr,
        config: parse_json(&record.config_json),
        priority: clamp_i64_to_i32(record.priority),
        enabled: record.enabled,
        fail_open: record.fail_open,
        metadata: parse_json(&record.metadata_json),
        created_at: record.created_at,
    }
}

fn paginate_by_id<T>(items: Vec<T>, params: &ListParams) -> ListResult<T>
where
    T: Clone + HasId,
{
    let total_count = Some(items.len() as u64);
    let mut filtered = items;
    if let Some(cursor) = params.cursor.as_deref()
        && let Some(position) = filtered.iter().position(|item| item.id() == cursor)
    {
        filtered = filtered.into_iter().skip(position + 1).collect();
    }
    let limit = params.limit.unwrap_or(50).max(1) as usize;
    let has_more = filtered.len() > limit;
    let mut page = filtered.into_iter().take(limit).collect::<Vec<_>>();
    let next_cursor = if has_more {
        page.last().map(|item| item.id().to_string())
    } else {
        None
    };
    ListResult {
        items: std::mem::take(&mut page),
        next_cursor,
        total_count,
    }
}

trait HasId {
    fn id(&self) -> &str;
}

impl HasId for ActionRecord {
    fn id(&self) -> &str {
        &self.id
    }
}

async fn load_password_hash(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Option<String>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "SELECT COALESCE({}, '{{}}') FROM credentials WHERE instance_id = $1 AND user_id = $2 AND type = 'password'",
                scoped.as_text("data"),
            );
            let row: Option<(String,)> = sqlx::query_as(&sql)
                .bind(scoped.instance_id())
                .bind(user_id)
                .fetch_optional(scoped.pool())
                .await?;
            Ok(row
                .map(|(json,)| parse_json(&json))
                .and_then(|json| json.get("hash").and_then(Value::as_str).map(str::to_owned)))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT data FROM credentials \
                 WHERE instance_id = @instance_id AND user_id = @user_id AND type = 'password' \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .and_then(|row| row.column_by_name::<String>("data").ok())
                .map(|json| parse_json(&json))
                .and_then(|json| json.get("hash").and_then(Value::as_str).map(str::to_owned)))
        }
    }
}

async fn create_linked_identity(
    db: &Db,
    instance_id: &str,
    id: &str,
    user_id: &str,
    provider_id: &str,
    external_sub: &str,
    external_email: &str,
    raw_claims: &Value,
) -> anyhow::Result<()> {
    crate::create_linked_identity_record(
        db,
        instance_id,
        id,
        user_id,
        provider_id,
        external_sub,
        external_email,
        &raw_claims.to_string(),
    )
    .await
}

async fn unlink_identity(
    db: &Db,
    instance_id: &str,
    user_id: &str,
    provider_id: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "DELETE FROM linked_identities WHERE instance_id = $1 AND user_id = $2 AND provider_id = $3",
            )
            .bind(scoped.instance_id())
            .bind(user_id)
            .bind(provider_id)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "DELETE FROM linked_identities \
                 WHERE instance_id = @instance_id AND user_id = @user_id AND provider_id = @provider_id",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            stmt.add_param("provider_id", &provider_id);
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
        }
    }
    Ok(())
}

async fn list_linked_identities(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Vec<LinkedIdentityRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let raw_claims = scoped.as_text("raw_claims");
            let sql = format!(
                "SELECT id, user_id, provider_id, external_sub, COALESCE(external_email, ''), COALESCE({raw_claims}, '{{}}') \
                 FROM linked_identities \
                 WHERE instance_id = $1 AND user_id = $2 \
                 ORDER BY provider_id, external_sub"
            );
            let rows: Vec<(String, String, String, String, String, String)> = sqlx::query_as(&sql)
                .bind(scoped.instance_id())
                .bind(user_id)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| LinkedIdentityRecord {
                    id: row.0,
                    user_id: row.1,
                    provider_id: row.2,
                    external_sub: row.3,
                    external_email: non_empty(Some(row.4)),
                    raw_claims: parse_json(&row.5),
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, user_id, provider_id, external_sub, IFNULL(external_email, '') AS external_email, \
                        IFNULL(raw_claims, '{}') AS raw_claims \
                 FROM linked_identities \
                 WHERE instance_id = @instance_id AND user_id = @user_id \
                 ORDER BY provider_id, external_sub",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| LinkedIdentityRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    user_id: row.column_by_name::<String>("user_id").unwrap_or_default(),
                    provider_id: row
                        .column_by_name::<String>("provider_id")
                        .unwrap_or_default(),
                    external_sub: row
                        .column_by_name::<String>("external_sub")
                        .unwrap_or_default(),
                    external_email: non_empty(row.column_by_name::<String>("external_email").ok()),
                    raw_claims: parse_json(
                        &row.column_by_name::<String>("raw_claims")
                            .unwrap_or_else(|_| "{}".to_string()),
                    ),
                })
                .collect())
        }
    }
}

async fn create_pat_record(
    db: &Db,
    instance_id: &str,
    pat: &PatRecord,
    hashed_token: &str,
) -> anyhow::Result<()> {
    crate::create_pat(
        db,
        instance_id,
        &pat.id,
        &pat.user_id,
        &pat.name,
        hashed_token,
        "[]",
    )
    .await
}

async fn get_pat_record(
    db: &Db,
    instance_id: &str,
    pat_id: &str,
) -> anyhow::Result<Option<PatRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let created_at = scoped.as_text("created_at");
            let sql = format!(
                "SELECT id, user_id, COALESCE(name, ''), {created_at} \
                 FROM tokens \
                 WHERE instance_id = $1 AND id = $2 AND type = 'pat' AND revoked_at IS NULL \
                 LIMIT 1"
            );
            let row: Option<(String, String, String, String)> = sqlx::query_as(&sql)
                .bind(scoped.instance_id())
                .bind(pat_id)
                .fetch_optional(scoped.pool())
                .await?;
            Ok(row.map(|row| PatRecord {
                id: row.0,
                user_id: row.1,
                name: row.2,
                created_at: row.3,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, user_id, IFNULL(name, '') AS name, CAST(created_at AS STRING) AS created_at \
                 FROM tokens \
                 WHERE instance_id = @instance_id AND id = @id AND type = 'pat' AND revoked_at IS NULL \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &pat_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| PatRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    user_id: row.column_by_name::<String>("user_id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                }))
        }
    }
}

async fn list_pat_records(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Vec<PatRecord>> {
    Ok(crate::list_pats_for_instance(db, instance_id)
        .await?
        .into_iter()
        .filter(|record| record.user_id == user_id)
        .map(|record| PatRecord {
            id: record.id,
            user_id: record.user_id,
            name: record.name,
            created_at: record.created_at,
        })
        .collect())
}

async fn revoke_pat_record(db: &Db, instance_id: &str, pat_id: &str) -> anyhow::Result<()> {
    crate::revoke_pat(db, instance_id, pat_id).await?;
    Ok(())
}

async fn resolve_pat(
    db: &Db,
    instance_id: &str,
    raw_token: &str,
) -> anyhow::Result<Option<ResolvedPat>> {
    let hashed = token_hash(raw_token);
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let row: Option<(String, Option<String>, String)> = sqlx::query_as(
                "SELECT t.user_id, t.session_id, COALESCE(u.org_id, '') \
                 FROM tokens t \
                 JOIN users u ON u.id = t.user_id AND u.instance_id = t.instance_id \
                 WHERE t.instance_id = $1 AND t.token_hash = $2 AND t.type = 'pat' AND t.revoked_at IS NULL \
                 LIMIT 1",
            )
            .bind(scoped.instance_id())
            .bind(&hashed)
            .fetch_optional(scoped.pool())
            .await?;
            Ok(row.map(|(user_id, session_id, org_id)| ResolvedPat {
                user_id,
                session_id: session_id.unwrap_or_default(),
                org_id,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT t.user_id, t.session_id, u.org_id \
                 FROM tokens t \
                 JOIN users u ON u.id = t.user_id AND u.instance_id = t.instance_id \
                 WHERE t.instance_id = @instance_id AND t.token_hash = @token_hash AND t.type = 'pat' AND t.revoked_at IS NULL \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("token_hash", &hashed);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| ResolvedPat {
                    user_id: row.column_by_name::<String>("user_id").unwrap_or_default(),
                    session_id: row
                        .column_by_name::<Option<String>>("session_id")
                        .unwrap_or(None)
                        .unwrap_or_default(),
                    org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
                }))
        }
    }
}

async fn create_session_record(
    db: &Db,
    instance_id: &str,
    user_id: &str,
    org_id: &str,
    auth_method: &str,
    session_max_age_secs: u64,
) -> anyhow::Result<CreatedSession> {
    let session_id = Uuid::new_v4().to_string();
    let token = Uuid::new_v4().to_string();
    let hashed_token = token_hash(&token);
    let org_id = if org_id.is_empty() { "_global" } else { org_id };

    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let expires_expr = match scoped.dialect() {
                Dialect::Postgres => {
                    format!(
                        "CURRENT_TIMESTAMP + INTERVAL '{} seconds'",
                        session_max_age_secs.max(1)
                    )
                }
                Dialect::Spanner => unreachable!("spanner does not use ScopedDb"),
                Dialect::Sqlite => {
                    format!(
                        "datetime(CURRENT_TIMESTAMP, '+{} seconds')",
                        session_max_age_secs.max(1)
                    )
                }
            };
            let sql = format!(
                "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, fingerprint, metadata, created_at, expires_at) \
                 VALUES ($1, $2, $3, $4, $5, '', '', '', {}, {}, {})",
                scoped.json_bind(6),
                scoped.timestamp_now(),
                expires_expr,
            );
            sqlx::query(&sql)
                .bind(&session_id)
                .bind(scoped.instance_id())
                .bind(user_id)
                .bind(org_id)
                .bind(&hashed_token)
                .bind(serde_json::json!({ "auth_method": auth_method }).to_string())
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let metadata_json = serde_json::json!({ "auth_method": auth_method }).to_string();
            let max_age = session_max_age_secs.max(1) as i64;
            let mut stmt = Statement::new(
                "INSERT INTO sessions \
                 (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, fingerprint, metadata, created_at, expires_at) \
                 VALUES \
                 (@id, @instance_id, @user_id, @org_id, @token_hash, '', '', '', @metadata, CURRENT_TIMESTAMP(), TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL @max_age SECOND))",
            );
            stmt.add_param("id", &session_id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            stmt.add_param("org_id", &org_id);
            stmt.add_param("token_hash", &hashed_token);
            stmt.add_param("metadata", &metadata_json);
            stmt.add_param("max_age", &max_age);
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
        }
    }

    Ok(CreatedSession { session_id, token })
}

async fn find_session_by_token(
    db: &Db,
    instance_id: &str,
    raw_token: &str,
) -> anyhow::Result<Option<SessionInfo>> {
    let hashed = token_hash(raw_token);
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let row: Option<(String, String, String)> = sqlx::query_as(
                "SELECT id, user_id, org_id \
                 FROM sessions \
                 WHERE instance_id = $1 AND token_hash = $2 AND revoked_at IS NULL \
                   AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP) \
                 LIMIT 1",
            )
            .bind(scoped.instance_id())
            .bind(&hashed)
            .fetch_optional(scoped.pool())
            .await?;
            Ok(row.map(|row| SessionInfo {
                session_id: row.0,
                user_id: row.1,
                org_id: row.2,
                token_type: "session".to_string(),
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, user_id, org_id \
                 FROM sessions \
                 WHERE instance_id = @instance_id AND token_hash = @token_hash AND revoked_at IS NULL \
                   AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP()) \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("token_hash", &hashed);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| SessionInfo {
                    session_id: row.column_by_name::<String>("id").unwrap_or_default(),
                    user_id: row.column_by_name::<String>("user_id").unwrap_or_default(),
                    org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
                    token_type: "session".to_string(),
                }))
        }
    }
}

async fn revoke_session_record(
    db: &Db,
    instance_id: &str,
    session_id: &str,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query(
                "UPDATE sessions SET revoked_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND id = $2",
            )
            .bind(scoped.instance_id())
            .bind(session_id)
            .execute(scoped.pool())
            .await?
            .rows_affected()
            > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE sessions SET revoked_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND id = @id",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &session_id);
            let (_, affected) = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
            Ok(affected > 0)
        }
    }
}

async fn upsert_login_flow(
    db: &Db,
    instance_id: &str,
    flow: &LoginFlowRecord,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let exists: Option<(String,)> =
                sqlx::query_as("SELECT id FROM login_flows WHERE instance_id = $1 AND id = $2")
                    .bind(scoped.instance_id())
                    .bind(&flow.id)
                    .fetch_optional(scoped.pool())
                    .await?;
            if exists.is_some() {
                let sql = format!(
                    "UPDATE login_flows SET name = $1, strategy = $2, state = $3, is_default = $4, enabled = $5, priority = $6, \
                     config = {}, audience = {}, auth_methods = {}, updated_at = CURRENT_TIMESTAMP \
                     WHERE instance_id = $7 AND id = $8",
                    scoped.json_bind(9),
                    scoped.json_bind(10),
                    scoped.json_bind(11),
                );
                sqlx::query(&sql)
                    .bind(&flow.name)
                    .bind(&flow.strategy)
                    .bind(&flow.state)
                    .bind(flow.is_default)
                    .bind(flow.enabled)
                    .bind(i64::from(flow.priority))
                    .bind(scoped.instance_id())
                    .bind(&flow.id)
                    .bind(flow.config.to_string())
                    .bind(flow.audience.to_string())
                    .bind(flow.auth_methods.to_string())
                    .execute(scoped.pool())
                    .await?;
            } else {
                let sql = format!(
                    "INSERT INTO login_flows \
                     (id, instance_id, name, strategy, state, is_default, enabled, priority, config, audience, auth_methods) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, {}, {}, {})",
                    scoped.json_bind(9),
                    scoped.json_bind(10),
                    scoped.json_bind(11),
                );
                sqlx::query(&sql)
                    .bind(&flow.id)
                    .bind(scoped.instance_id())
                    .bind(&flow.name)
                    .bind(&flow.strategy)
                    .bind(&flow.state)
                    .bind(flow.is_default)
                    .bind(flow.enabled)
                    .bind(i64::from(flow.priority))
                    .bind(flow.config.to_string())
                    .bind(flow.audience.to_string())
                    .bind(flow.auth_methods.to_string())
                    .execute(scoped.pool())
                    .await?;
            }
        }
        Db::Spanner(spanner) => {
            let mut exists_stmt = Statement::new(
                "SELECT id FROM login_flows WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            exists_stmt.add_param("instance_id", &instance_id);
            exists_stmt.add_param("id", &flow.id);
            let exists = spanner_query_optional(spanner, exists_stmt)
                .await?
                .is_some();
            let config_json = flow.config.to_string();
            let audience_json = flow.audience.to_string();
            let auth_methods_json = flow.auth_methods.to_string();
            let priority = i64::from(flow.priority);
            let stmt = if exists {
                let mut stmt = Statement::new(
                    "UPDATE login_flows SET name = @name, strategy = @strategy, state = @state, \
                     is_default = @is_default, enabled = @enabled, priority = @priority, \
                     config = @config, audience = @audience, auth_methods = @auth_methods, \
                     updated_at = CURRENT_TIMESTAMP() \
                     WHERE instance_id = @instance_id AND id = @id",
                );
                stmt.add_param("name", &flow.name);
                stmt.add_param("strategy", &flow.strategy);
                stmt.add_param("state", &flow.state);
                stmt.add_param("is_default", &flow.is_default);
                stmt.add_param("enabled", &flow.enabled);
                stmt.add_param("priority", &priority);
                stmt.add_param("config", &config_json);
                stmt.add_param("audience", &audience_json);
                stmt.add_param("auth_methods", &auth_methods_json);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("id", &flow.id);
                stmt
            } else {
                let mut stmt = Statement::new(
                    "INSERT INTO login_flows \
                     (id, instance_id, name, strategy, state, is_default, enabled, priority, config, audience, auth_methods) \
                     VALUES \
                     (@id, @instance_id, @name, @strategy, @state, @is_default, @enabled, @priority, @config, @audience, @auth_methods)",
                );
                stmt.add_param("id", &flow.id);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("name", &flow.name);
                stmt.add_param("strategy", &flow.strategy);
                stmt.add_param("state", &flow.state);
                stmt.add_param("is_default", &flow.is_default);
                stmt.add_param("enabled", &flow.enabled);
                stmt.add_param("priority", &priority);
                stmt.add_param("config", &config_json);
                stmt.add_param("audience", &audience_json);
                stmt.add_param("auth_methods", &auth_methods_json);
                stmt
            };
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
        }
    }
    Ok(())
}

async fn create_oidc_auth_request(
    db: &Db,
    instance_id: &str,
    request: &OidcAuthRequest,
) -> anyhow::Result<String> {
    let request_id = if request.id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        request.id.clone()
    };
    let user_id = request.user_id.clone().unwrap_or_default();
    let nonce = request.nonce.clone().unwrap_or_default();
    let code_challenge = request.code_challenge.clone().unwrap_or_default();
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "INSERT INTO oidc_auth_requests \
                 (id, instance_id, user_id, client_id, redirect_uri, scope, nonce, response_type, code_challenge, code_challenge_method, prompt, done, auth_time) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, 'code', $8, '', '[]', 0, $9)",
            )
            .bind(&request_id)
            .bind(scoped.instance_id())
            .bind(&user_id)
            .bind(&request.client_id)
            .bind(&request.redirect_uri)
            .bind(&request.scope)
            .bind(&nonce)
            .bind(&code_challenge)
            .bind(request.auth_time.as_deref())
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO oidc_auth_requests \
                 (id, instance_id, user_id, client_id, redirect_uri, scope, nonce, response_type, code_challenge, code_challenge_method, prompt, done, auth_time) \
                 VALUES \
                 (@id, @instance_id, @user_id, @client_id, @redirect_uri, @scope, @nonce, 'code', @code_challenge, '', '[]', FALSE, @auth_time)",
            );
            stmt.add_param("id", &request_id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            stmt.add_param("client_id", &request.client_id);
            stmt.add_param("redirect_uri", &request.redirect_uri);
            stmt.add_param("scope", &request.scope);
            stmt.add_param("nonce", &nonce);
            stmt.add_param("code_challenge", &code_challenge);
            stmt.add_param("auth_time", &request.auth_time);
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
        }
    }
    Ok(request_id)
}

async fn consume_oidc_auth_code(
    db: &Db,
    instance_id: &str,
    code: &str,
) -> anyhow::Result<Option<OidcAuthRequest>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let auth_time = scoped.as_text("auth_time");
            let mut tx = scoped.pool().begin().await?;
            let sql = format!(
                "SELECT id, user_id, client_id, redirect_uri, scope, nonce, code_challenge, {auth_time} \
                 FROM oidc_auth_requests \
                 WHERE instance_id = $1 AND code = $2 AND done = 1 \
                 LIMIT 1"
            );
            let row: Option<(
                String,
                String,
                String,
                String,
                String,
                String,
                String,
                Option<String>,
            )> = sqlx::query_as(&sql)
                .bind(scoped.instance_id())
                .bind(code)
                .fetch_optional(&mut *tx)
                .await?;
            let Some(row) = row else {
                tx.rollback().await?;
                return Ok(None);
            };
            sqlx::query("DELETE FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2")
                .bind(scoped.instance_id())
                .bind(&row.0)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            Ok(Some(OidcAuthRequest {
                id: row.0,
                user_id: non_empty(Some(row.1)),
                client_id: row.2,
                redirect_uri: row.3,
                scope: row.4,
                nonce: non_empty(Some(row.5)),
                code_challenge: non_empty(Some(row.6)),
                auth_time: row.7,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, user_id, client_id, redirect_uri, scope, nonce, code_challenge, CAST(auth_time AS STRING) AS auth_time \
                 FROM oidc_auth_requests \
                 WHERE instance_id = @instance_id AND code = @code AND done = TRUE \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("code", &code);
            let Some(row) = spanner_query_optional(spanner, stmt).await? else {
                return Ok(None);
            };
            let auth_request_id = row.column_by_name::<String>("id")?;
            let record = OidcAuthRequest {
                id: auth_request_id.clone(),
                user_id: non_empty(row.column_by_name::<String>("user_id").ok()),
                client_id: row.column_by_name::<String>("client_id")?,
                redirect_uri: row.column_by_name::<String>("redirect_uri")?,
                scope: row.column_by_name::<String>("scope")?,
                nonce: non_empty(row.column_by_name::<String>("nonce").ok()),
                code_challenge: non_empty(row.column_by_name::<String>("code_challenge").ok()),
                auth_time: row.column_by_name::<Option<String>>("auth_time")?,
            };
            let mut delete_stmt = Statement::new(
                "DELETE FROM oidc_auth_requests WHERE instance_id = @instance_id AND id = @id",
            );
            delete_stmt.add_param("instance_id", &instance_id);
            delete_stmt.add_param("id", &auth_request_id);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = delete_stmt.clone();
                    Box::pin(async move {
                        tx.update(stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
            Ok(Some(record))
        }
    }
}

async fn append_domain_event(
    db: &Db,
    instance_id: &str,
    event: &DomainEvent,
    request_id: Option<&str>,
    session_id: Option<&str>,
    flow_id: Option<&str>,
) -> anyhow::Result<()> {
    let event_id = Uuid::now_v7().to_string();
    let org_id = event_org_id(event);
    let actor_id = non_empty(Some(event.actor_id().to_string()));
    let aggregate_id = non_empty(Some(event.aggregate_id().to_string()));
    let aggregate_type = Some(event.category().to_string());
    let resource_type = aggregate_type.clone();
    let payload_json = serde_json::to_string(event).context("serialize domain event payload")?;

    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO events \
                 (id, instance_id, event_type, category, org_id, actor_id, aggregate_id, aggregate_type, resource_type, payload, metadata, request_id, session_id, flow_id, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, {}, {}, $10, $11, $12, {})",
                scoped.json_bind(13),
                scoped.json_bind(14),
                scoped.timestamp_now(),
            );
            sqlx::query(&sql)
                .bind(&event_id)
                .bind(scoped.instance_id())
                .bind(event.event_type())
                .bind(event.category())
                .bind(&org_id)
                .bind(actor_id)
                .bind(aggregate_id)
                .bind(aggregate_type)
                .bind(resource_type)
                .bind(request_id)
                .bind(session_id)
                .bind(flow_id)
                .bind(payload_json)
                .bind("{}")
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO events \
                 (id, instance_id, event_type, category, org_id, actor_id, aggregate_id, aggregate_type, resource_type, payload, metadata, request_id, session_id, flow_id, created_at) \
                 VALUES \
                 (@id, @instance_id, @event_type, @category, @org_id, @actor_id, @aggregate_id, @aggregate_type, @resource_type, @payload, @metadata, @request_id, @session_id, @flow_id, CURRENT_TIMESTAMP())",
            );
            stmt.add_param("id", &event_id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("event_type", &event.event_type());
            stmt.add_param("category", &event.category());
            stmt.add_param("org_id", &org_id);
            stmt.add_param("actor_id", &actor_id);
            stmt.add_param("aggregate_id", &aggregate_id);
            stmt.add_param("aggregate_type", &aggregate_type);
            stmt.add_param("resource_type", &resource_type);
            stmt.add_param("payload", &payload_json);
            stmt.add_param("metadata", &"{}");
            stmt.add_param("request_id", &request_id);
            stmt.add_param("session_id", &session_id);
            stmt.add_param("flow_id", &flow_id);
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
        }
    }

    Ok(())
}

async fn list_domain_events(
    db: &Db,
    instance_id: &str,
    params: &EventQueryParams,
) -> anyhow::Result<ListResult<EventRecord>> {
    let limit = params.limit.unwrap_or(50).clamp(1, 500) as usize;
    let rows = match db {
        Db::Sql(_) => list_domain_events_sql(db, instance_id, params, limit).await?,
        Db::Spanner(spanner) => {
            list_domain_events_spanner(spanner, instance_id, params, limit).await?
        }
    };
    let has_more = rows.len() > limit;
    let items = rows
        .into_iter()
        .take(limit)
        .map(event_row_to_record)
        .collect::<Vec<_>>();
    let next_cursor = if has_more {
        items
            .last()
            .map(|event| format!("{}|{}", event.created_at, event.id))
    } else {
        None
    };
    Ok(ListResult {
        items,
        next_cursor,
        total_count: None,
    })
}

async fn list_domain_events_sql(
    db: &Db,
    instance_id: &str,
    params: &EventQueryParams,
    limit: usize,
) -> anyhow::Result<Vec<EventRow>> {
    let scoped = db.scoped(instance_id.to_string());
    let payload = scoped.as_text("payload");
    let metadata = scoped.as_text("metadata");
    let created_at = scoped.as_text("created_at");
    let mut builder = QueryBuilder::<Any>::new(format!(
        "SELECT id, event_type, category, COALESCE(org_id, ''), actor_id, aggregate_id, aggregate_type, resource_type, \
                COALESCE({payload}, '{{}}'), COALESCE({metadata}, '{{}}'), request_id, session_id, flow_id, sequence, {created_at} \
         FROM events WHERE instance_id = "
    ));
    builder.push_bind(scoped.instance_id());
    if let Some(event_type) = params.event_type.as_deref() {
        builder.push(" AND event_type = ").push_bind(event_type);
    }
    if let Some(category) = params.category.as_deref() {
        builder.push(" AND category = ").push_bind(category);
    }
    if let Some(aggregate_id) = params.aggregate_id.as_deref() {
        builder.push(" AND aggregate_id = ").push_bind(aggregate_id);
    }
    if let Some(session_id) = params.session_id.as_deref() {
        builder.push(" AND session_id = ").push_bind(session_id);
    }
    if let Some((cursor_created_at, cursor_id)) = decode_event_cursor(params.cursor.as_deref()) {
        builder
            .push(" AND (created_at < ")
            .push_bind(cursor_created_at.clone())
            .push(" OR (created_at = ")
            .push_bind(cursor_created_at)
            .push(" AND id < ")
            .push_bind(cursor_id)
            .push("))");
    }
    builder
        .push(" ORDER BY created_at DESC, id DESC LIMIT ")
        .push_bind((limit + 1) as i64);
    let query = builder.build_query_as::<EventRow>();
    Ok(query.fetch_all(scoped.pool()).await?)
}

async fn list_domain_events_spanner(
    spanner: &crate::SpannerDb,
    instance_id: &str,
    params: &EventQueryParams,
    limit: usize,
) -> anyhow::Result<Vec<EventRow>> {
    let mut sql = String::from(
        "SELECT id, event_type, category, IFNULL(org_id, '') AS org_id, actor_id, aggregate_id, aggregate_type, resource_type, \
                IFNULL(payload, '{}') AS payload, IFNULL(metadata, '{}') AS metadata, request_id, session_id, flow_id, sequence, CAST(created_at AS STRING) AS created_at \
         FROM events WHERE instance_id = @instance_id",
    );
    let event_type = params.event_type.clone();
    let category = params.category.clone();
    let aggregate_id = params.aggregate_id.clone();
    let session_id = params.session_id.clone();
    let cursor = decode_event_cursor(params.cursor.as_deref());
    if params.event_type.is_some() {
        sql.push_str(" AND event_type = @event_type");
    }
    if params.category.is_some() {
        sql.push_str(" AND category = @category");
    }
    if params.aggregate_id.is_some() {
        sql.push_str(" AND aggregate_id = @aggregate_id");
    }
    if params.session_id.is_some() {
        sql.push_str(" AND session_id = @session_id");
    }
    if cursor.is_some() {
        sql.push_str(
            " AND (created_at < TIMESTAMP(@cursor_created_at) OR (created_at = TIMESTAMP(@cursor_created_at) AND id < @cursor_id))",
        );
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT @limit");
    let mut stmt = Statement::new(sql);
    stmt.add_param("instance_id", &instance_id);
    if let Some(ref event_type) = event_type {
        stmt.add_param("event_type", event_type);
    }
    if let Some(ref category) = category {
        stmt.add_param("category", category);
    }
    if let Some(ref aggregate_id) = aggregate_id {
        stmt.add_param("aggregate_id", aggregate_id);
    }
    if let Some(ref session_id) = session_id {
        stmt.add_param("session_id", session_id);
    }
    if let Some((cursor_created_at, cursor_id)) = cursor {
        stmt.add_param("cursor_created_at", &cursor_created_at);
        stmt.add_param("cursor_id", &cursor_id);
    }
    stmt.add_param("limit", &((limit + 1) as i64));
    Ok(spanner_query_all(spanner, stmt)
        .await?
        .into_iter()
        .map(|row| {
            (
                row.column_by_name::<String>("id").unwrap_or_default(),
                row.column_by_name::<String>("event_type")
                    .unwrap_or_default(),
                row.column_by_name::<String>("category").unwrap_or_default(),
                row.column_by_name::<String>("org_id").unwrap_or_default(),
                row.column_by_name::<Option<String>>("actor_id")
                    .unwrap_or(None),
                row.column_by_name::<Option<String>>("aggregate_id")
                    .unwrap_or(None),
                row.column_by_name::<Option<String>>("aggregate_type")
                    .unwrap_or(None),
                row.column_by_name::<Option<String>>("resource_type")
                    .unwrap_or(None),
                row.column_by_name::<String>("payload")
                    .unwrap_or_else(|_| "{}".to_string()),
                row.column_by_name::<String>("metadata")
                    .unwrap_or_else(|_| "{}".to_string()),
                row.column_by_name::<Option<String>>("request_id")
                    .unwrap_or(None),
                row.column_by_name::<Option<String>>("session_id")
                    .unwrap_or(None),
                row.column_by_name::<Option<String>>("flow_id")
                    .unwrap_or(None),
                row.column_by_name::<Option<i64>>("sequence")
                    .unwrap_or(None),
                row.column_by_name::<String>("created_at")
                    .unwrap_or_default(),
            )
        })
        .collect())
}

fn decode_event_cursor(cursor: Option<&str>) -> Option<(String, String)> {
    let raw = cursor?.trim();
    if raw.is_empty() {
        return None;
    }
    let (created_at, id) = raw.split_once('|')?;
    Some((created_at.to_string(), id.to_string()))
}

fn event_row_to_record(row: EventRow) -> EventRecord {
    EventRecord {
        id: row.0,
        event_type: row.1,
        category: row.2,
        org_id: row.3,
        actor_id: row.4,
        aggregate_id: row.5,
        aggregate_type: row.6,
        resource_type: row.7,
        payload: parse_json(&row.8),
        metadata: parse_json(&row.9),
        request_id: row.10,
        session_id: row.11,
        flow_id: row.12,
        sequence: row.13,
        created_at: row.14,
    }
}

fn event_org_id(event: &DomainEvent) -> String {
    match event {
        DomainEvent::UserCreated { org_id, .. } => org_id.clone(),
        DomainEvent::OrgCreated { org_id, .. } | DomainEvent::OrgUpdated { org_id, .. } => {
            org_id.clone()
        }
        DomainEvent::GroupCreated { org_id, .. } => org_id.clone(),
        _ => "0".to_string(),
    }
}

async fn upsert_action(
    db: &Db,
    instance_id: &str,
    action: &ActionRecord,
    update_only: bool,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            if update_only {
                let sql = format!(
                    "UPDATE actions SET org_id = $1, name = $2, hook = $3, action_type = $4, trigger_expr = $5, \
                     config = {}, priority = $6, enabled = $7, fail_open = $8, metadata = {}, updated_at = CURRENT_TIMESTAMP \
                     WHERE instance_id = $9 AND id = $10",
                    scoped.json_bind(11),
                    scoped.json_bind(12),
                );
                sqlx::query(&sql)
                    .bind(&action.org_id)
                    .bind(&action.name)
                    .bind(&action.hook)
                    .bind(&action.action_type)
                    .bind(&action.trigger_expr)
                    .bind(i64::from(action.priority))
                    .bind(action.enabled)
                    .bind(action.fail_open)
                    .bind(scoped.instance_id())
                    .bind(&action.id)
                    .bind(action.config.to_string())
                    .bind(action.metadata.to_string())
                    .execute(scoped.pool())
                    .await?;
            } else {
                let sql = format!(
                    "INSERT INTO actions \
                     (id, instance_id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, metadata) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, {}, $8, $9, $10, {})",
                    scoped.json_bind(11),
                    scoped.json_bind(12),
                );
                sqlx::query(&sql)
                    .bind(&action.id)
                    .bind(scoped.instance_id())
                    .bind(&action.org_id)
                    .bind(&action.name)
                    .bind(&action.hook)
                    .bind(&action.action_type)
                    .bind(&action.trigger_expr)
                    .bind(i64::from(action.priority))
                    .bind(action.enabled)
                    .bind(action.fail_open)
                    .bind(action.config.to_string())
                    .bind(action.metadata.to_string())
                    .execute(scoped.pool())
                    .await?;
            }
        }
        Db::Spanner(spanner) => {
            let priority = i64::from(action.priority);
            let config_json = action.config.to_string();
            let metadata_json = action.metadata.to_string();
            let stmt = if update_only {
                let mut stmt = Statement::new(
                    "UPDATE actions SET org_id = @org_id, name = @name, hook = @hook, action_type = @action_type, \
                     trigger_expr = @trigger_expr, config = @config, priority = @priority, enabled = @enabled, \
                     fail_open = @fail_open, metadata = @metadata, updated_at = CURRENT_TIMESTAMP() \
                     WHERE instance_id = @instance_id AND id = @id",
                );
                stmt.add_param("org_id", &action.org_id);
                stmt.add_param("name", &action.name);
                stmt.add_param("hook", &action.hook);
                stmt.add_param("action_type", &action.action_type);
                stmt.add_param("trigger_expr", &action.trigger_expr);
                stmt.add_param("config", &config_json);
                stmt.add_param("priority", &priority);
                stmt.add_param("enabled", &action.enabled);
                stmt.add_param("fail_open", &action.fail_open);
                stmt.add_param("metadata", &metadata_json);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("id", &action.id);
                stmt
            } else {
                let mut stmt = Statement::new(
                    "INSERT INTO actions \
                     (id, instance_id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, metadata) \
                     VALUES \
                     (@id, @instance_id, @org_id, @name, @hook, @action_type, @trigger_expr, @config, @priority, @enabled, @fail_open, @metadata)",
                );
                stmt.add_param("id", &action.id);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("org_id", &action.org_id);
                stmt.add_param("name", &action.name);
                stmt.add_param("hook", &action.hook);
                stmt.add_param("action_type", &action.action_type);
                stmt.add_param("trigger_expr", &action.trigger_expr);
                stmt.add_param("config", &config_json);
                stmt.add_param("priority", &priority);
                stmt.add_param("enabled", &action.enabled);
                stmt.add_param("fail_open", &action.fail_open);
                stmt.add_param("metadata", &metadata_json);
                stmt
            };
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
        }
    }
    Ok(())
}

#[derive(Clone)]
struct ParsedFgaObject {
    object_type: String,
    object_id: String,
}

#[derive(Clone)]
struct ParsedFgaUser {
    user_type: String,
    user_id: String,
    user_relation: String,
}

fn parse_fga_object(raw: &str) -> anyhow::Result<ParsedFgaObject> {
    let (object_type, object_id) = raw
        .split_once(':')
        .context("invalid FGA object reference")?;
    anyhow::ensure!(
        !object_type.is_empty() && !object_id.is_empty(),
        "invalid FGA object reference"
    );
    Ok(ParsedFgaObject {
        object_type: object_type.to_string(),
        object_id: object_id.to_string(),
    })
}

fn parse_fga_user(raw: &str) -> anyhow::Result<ParsedFgaUser> {
    if let Some(object_type) = raw.strip_suffix(":*") {
        anyhow::ensure!(!object_type.is_empty(), "invalid FGA user reference");
        return Ok(ParsedFgaUser {
            user_type: object_type.to_string(),
            user_id: "*".to_string(),
            user_relation: String::new(),
        });
    }
    let (base, relation) = match raw.split_once('#') {
        Some((base, relation)) => (base, relation.to_string()),
        None => (raw, String::new()),
    };
    let object = parse_fga_object(base)?;
    Ok(ParsedFgaUser {
        user_type: object.object_type,
        user_id: object.object_id,
        user_relation: relation,
    })
}

async fn ensure_fga_store(db: &Db, instance_id: &str) -> anyhow::Result<String> {
    if let Some(store_id) = load_fga_store(db, instance_id).await? {
        return Ok(store_id);
    }
    let store_id = instance_id.to_string();
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = match scoped.dialect() {
                Dialect::Sqlite => {
                    "INSERT OR IGNORE INTO fga_instance_stores (instance_id, store_id) VALUES ($1, $2)"
                }
                Dialect::Postgres => {
                    "INSERT INTO fga_instance_stores (instance_id, store_id) VALUES ($1, $2) ON CONFLICT (instance_id) DO NOTHING"
                }
                Dialect::Spanner => unreachable!("spanner does not use ScopedDb"),
            };
            sqlx::query(sql)
                .bind(scoped.instance_id())
                .bind(&store_id)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO fga_instance_stores (instance_id, store_id) VALUES (@instance_id, @store_id)",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("store_id", &store_id);
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
        }
    }
    Ok(store_id)
}

async fn load_fga_store(db: &Db, instance_id: &str) -> anyhow::Result<Option<String>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let row: Option<(String,)> =
                sqlx::query_as("SELECT store_id FROM fga_instance_stores WHERE instance_id = $1")
                    .bind(scoped.instance_id())
                    .fetch_optional(scoped.pool())
                    .await?;
            Ok(row.map(|row| row.0))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT store_id FROM fga_instance_stores WHERE instance_id = @instance_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .and_then(|row| row.column_by_name::<String>("store_id").ok()))
        }
    }
}

async fn fga_check(
    db: &Db,
    instance_id: &str,
    user: &str,
    relation: &str,
    object: &str,
) -> anyhow::Result<bool> {
    let Some(store_id) = load_fga_store(db, instance_id).await? else {
        return Ok(false);
    };
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let row: Option<(i64,)> = sqlx::query_as(
                "SELECT 1 FROM fga_tuples \
                 WHERE instance_id = $1 AND store_id = $2 AND raw_user = $3 AND relation = $4 AND raw_object = $5 \
                 LIMIT 1",
            )
            .bind(scoped.instance_id())
            .bind(&store_id)
            .bind(user)
            .bind(relation)
            .bind(object)
            .fetch_optional(scoped.pool())
            .await?;
            Ok(row.is_some())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT raw_user FROM fga_tuples \
                 WHERE instance_id = @instance_id AND store_id = @store_id AND raw_user = @raw_user AND relation = @relation AND raw_object = @raw_object \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("store_id", &store_id);
            stmt.add_param("raw_user", &user);
            stmt.add_param("relation", &relation);
            stmt.add_param("raw_object", &object);
            Ok(spanner_query_optional(spanner, stmt).await?.is_some())
        }
    }
}

async fn write_fga_tuple(
    db: &Db,
    instance_id: &str,
    user: &str,
    relation: &str,
    object: &str,
) -> anyhow::Result<()> {
    let store_id = ensure_fga_store(db, instance_id).await?;
    let parsed_object = parse_fga_object(object)?;
    let parsed_user = parse_fga_user(user)?;
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = match scoped.dialect() {
                Dialect::Sqlite => {
                    "INSERT OR IGNORE INTO fga_tuples (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
                }
                Dialect::Postgres => {
                    "INSERT INTO fga_tuples (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT DO NOTHING"
                }
                Dialect::Spanner => unreachable!("spanner does not use ScopedDb"),
            };
            sqlx::query(sql)
                .bind(scoped.instance_id())
                .bind(&store_id)
                .bind(&parsed_object.object_type)
                .bind(&parsed_object.object_id)
                .bind(relation)
                .bind(&parsed_user.user_type)
                .bind(&parsed_user.user_id)
                .bind(&parsed_user.user_relation)
                .bind(object)
                .bind(user)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            if fga_check(db, instance_id, user, relation, object).await? {
                return Ok(());
            }
            let mut stmt = Statement::new(
                "INSERT INTO fga_tuples \
                 (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user) \
                 VALUES \
                 (@instance_id, @store_id, @object_type, @object_id, @relation, @user_type, @user_id, @user_relation, @raw_object, @raw_user)",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("store_id", &store_id);
            stmt.add_param("object_type", &parsed_object.object_type);
            stmt.add_param("object_id", &parsed_object.object_id);
            stmt.add_param("relation", &relation);
            stmt.add_param("user_type", &parsed_user.user_type);
            stmt.add_param("user_id", &parsed_user.user_id);
            stmt.add_param("user_relation", &parsed_user.user_relation);
            stmt.add_param("raw_object", &object);
            stmt.add_param("raw_user", &user);
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
        }
    }
    Ok(())
}

async fn delete_fga_tuple(
    db: &Db,
    instance_id: &str,
    user: &str,
    relation: &str,
    object: &str,
) -> anyhow::Result<()> {
    let Some(store_id) = load_fga_store(db, instance_id).await? else {
        return Ok(());
    };
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "DELETE FROM fga_tuples \
                 WHERE instance_id = $1 AND store_id = $2 AND raw_user = $3 AND relation = $4 AND raw_object = $5",
            )
            .bind(scoped.instance_id())
            .bind(&store_id)
            .bind(user)
            .bind(relation)
            .bind(object)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "DELETE FROM fga_tuples \
                 WHERE instance_id = @instance_id AND store_id = @store_id AND raw_user = @raw_user AND relation = @relation AND raw_object = @raw_object",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("store_id", &store_id);
            stmt.add_param("raw_user", &user);
            stmt.add_param("relation", &relation);
            stmt.add_param("raw_object", &object);
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
        }
    }
    Ok(())
}

async fn list_fga_relations(
    db: &Db,
    instance_id: &str,
    user: &str,
    object_type: &str,
) -> anyhow::Result<Vec<FgaRelation>> {
    let Some(store_id) = load_fga_store(db, instance_id).await? else {
        return Ok(Vec::new());
    };
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let rows: Vec<(String, String, String)> = sqlx::query_as(
                "SELECT raw_user, relation, raw_object \
                 FROM fga_tuples \
                 WHERE instance_id = $1 AND store_id = $2 AND raw_user = $3 AND object_type = $4 \
                 ORDER BY relation, raw_object",
            )
            .bind(scoped.instance_id())
            .bind(&store_id)
            .bind(user)
            .bind(object_type)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|row| FgaRelation {
                    user: row.0,
                    relation: row.1,
                    object: row.2,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT raw_user, relation, raw_object \
                 FROM fga_tuples \
                 WHERE instance_id = @instance_id AND store_id = @store_id AND raw_user = @raw_user AND object_type = @object_type \
                 ORDER BY relation, raw_object",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("store_id", &store_id);
            stmt.add_param("raw_user", &user);
            stmt.add_param("object_type", &object_type);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| FgaRelation {
                    user: row.column_by_name::<String>("raw_user").unwrap_or_default(),
                    relation: row.column_by_name::<String>("relation").unwrap_or_default(),
                    object: row
                        .column_by_name::<String>("raw_object")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

async fn spanner_query_optional(
    spanner: &crate::SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Option<SpannerRow>> {
    let mut tx = spanner.client().single().await?;
    let mut rows = tx.query(stmt).await?;
    rows.next().await.map_err(Into::into)
}

async fn spanner_query_all(
    spanner: &crate::SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Vec<SpannerRow>> {
    let mut tx = spanner.client().single().await?;
    let mut rows = tx.query(stmt).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(row);
    }
    Ok(out)
}
