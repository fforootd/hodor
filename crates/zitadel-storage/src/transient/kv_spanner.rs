use google_cloud_spanner::{client::Error as SpannerError, statement::Statement};
use serde_json::Value;
use uuid::Uuid;
use zitadel_crypto::token_hash;
use zitadel_db::Db;

use super::{
    AuthRequestRedirect, AuthRequestRequirements, CreatedSession, KvStore, LoginFlowRuntimeState,
    NewLoginFlowState, ProviderAuthState, SessionRecord,
    semantics::{SessionLookupOutcome, session_lookup_outcome},
};

#[derive(Clone)]
pub struct SpannerKvStore {
    db: Db,
    session_max_age_secs: u64,
}

impl SpannerKvStore {
    pub fn new(db: Db, session_max_age_secs: u64) -> Self {
        Self {
            db,
            session_max_age_secs,
        }
    }

    fn client(&self) -> &google_cloud_spanner::client::Client {
        self.db
            .spanner()
            .expect("spanner kv store requires native spanner backend")
            .client()
    }

    async fn session_timestamps(&self) -> anyhow::Result<(String, u64, Option<String>)> {
        let mut stmt = Statement::new(
            "SELECT CAST(CURRENT_TIMESTAMP() AS STRING) AS created_at, \
                    UNIX_SECONDS(CURRENT_TIMESTAMP()) AS created_at_epoch, \
                    CAST(TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL @max_age SECOND) AS STRING) AS expires_at",
        );
        let max_age = self.session_max_age_secs.max(1) as i64;
        stmt.add_param("max_age", &max_age);
        let mut tx = self.client().single().await?;
        let mut rows = tx.query(stmt).await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| anyhow::anyhow!("spanner timestamp query returned no row"))?;
        Ok((
            row.column_by_name::<String>("created_at")?,
            row.column_by_name::<i64>("created_at_epoch")? as u64,
            Some(row.column_by_name::<String>("expires_at")?),
        ))
    }
}

impl KvStore for SpannerKvStore {
    fn should_emit_provider_state_record(&self) -> bool {
        false
    }

    async fn create_session(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        user_agent: &str,
        ip_address: &str,
        fingerprint: &str,
    ) -> anyhow::Result<CreatedSession> {
        let session_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let token_hash = token_hash(&token);
        let org: Option<&str> = if org_id.is_empty() { None } else { Some(org_id) };
        let (created_at, created_at_epoch, expires_at) = self.session_timestamps().await?;
        let expires_at_value = expires_at.clone().unwrap_or_default();
        let mut stmt = Statement::new(
            "INSERT INTO sessions \
             (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, fingerprint, metadata, created_at, expires_at) \
             VALUES \
             (@id, @instance_id, @user_id, @org_id, @token_hash, @user_agent, @ip_address, @fingerprint, @metadata, TIMESTAMP(@created_at), TIMESTAMP(@expires_at))",
        );
        stmt.add_param("id", &session_id);
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("user_id", &user_id);
        stmt.add_param("org_id", &org.unwrap_or(""));
        stmt.add_param("token_hash", &token_hash);
        stmt.add_param("user_agent", &user_agent);
        stmt.add_param("ip_address", &ip_address);
        stmt.add_param("fingerprint", &fingerprint);
        stmt.add_param("metadata", &"{}");
        stmt.add_param("created_at", &created_at);
        stmt.add_param("expires_at", &expires_at_value);
        let _ = self
            .client()
            .read_write_transaction(|tx| {
                let stmt = stmt.clone();
                Box::pin(async move {
                    tx.update(stmt).await?;
                    Ok::<(), SpannerError>(())
                })
            })
            .await?;

        Ok(CreatedSession {
            session_id,
            token,
            created_at,
            created_at_epoch,
        })
    }

    async fn find_session_by_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        let hashed = token_hash(raw_token);
        let mut stmt = Statement::new(
            "SELECT s.id, s.user_id, s.org_id, s.token_hash, s.user_agent, s.ip_address, s.fingerprint, \
                    CAST(s.created_at AS STRING) AS created_at, \
                    UNIX_SECONDS(s.created_at) AS created_at_epoch, \
                    CAST(s.expires_at AS STRING) AS expires_at, \
                    CAST(s.revoked_at AS STRING) AS revoked_at, \
                    UNIX_SECONDS(s.expires_at) AS expires_at_epoch \
             FROM sessions s \
             JOIN users u ON u.instance_id = s.instance_id AND u.id = s.user_id \
             WHERE s.instance_id = @instance_id AND s.token_hash = @token_hash AND u.state = 'active' \
             LIMIT 1",
        );
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("token_hash", &hashed);
        let mut tx = self.client().single().await?;
        let mut rows = tx.query(stmt).await?;
        Ok(match rows.next().await? {
            Some(row) => match session_lookup_outcome(
                SessionRecord {
                    id: row.column_by_name::<String>("id")?,
                    user_id: row.column_by_name::<String>("user_id")?,
                    org_id: row.column_by_name::<String>("org_id")?,
                    token_hash: row.column_by_name::<String>("token_hash")?,
                    user_agent: row.column_by_name::<String>("user_agent")?,
                    ip_address: row.column_by_name::<String>("ip_address")?,
                    fingerprint: row.column_by_name::<String>("fingerprint")?,
                    metadata: Value::Object(Default::default()),
                    created_at: row.column_by_name::<String>("created_at")?,
                    created_at_epoch: row.column_by_name::<i64>("created_at_epoch")? as u64,
                    expires_at: row.column_by_name::<Option<String>>("expires_at")?,
                    revoked_at: row.column_by_name::<Option<String>>("revoked_at")?,
                },
                row.column_by_name::<Option<i64>>("expires_at_epoch")?
                    .and_then(|value| u64::try_from(value).ok()),
            ) {
                SessionLookupOutcome::Active(record) => Some(record),
                SessionLookupOutcome::Inactive | SessionLookupOutcome::Missing => None,
            },
            None => None,
        })
    }

    async fn list_sessions(&self, instance_id: &str) -> anyhow::Result<Vec<SessionRecord>> {
        let mut stmt = Statement::new(
            "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, fingerprint, \
                    CAST(created_at AS STRING) AS created_at, \
                    UNIX_SECONDS(created_at) AS created_at_epoch, \
                    CAST(expires_at AS STRING) AS expires_at, \
                    CAST(revoked_at AS STRING) AS revoked_at \
             FROM sessions WHERE instance_id = @instance_id \
             ORDER BY created_at DESC LIMIT 50",
        );
        stmt.add_param("instance_id", &instance_id);
        let mut tx = self.client().single().await?;
        let mut rows = tx.query(stmt).await?;
        let mut sessions = Vec::new();
        while let Some(row) = rows.next().await? {
            sessions.push(SessionRecord {
                id: row.column_by_name::<String>("id")?,
                user_id: row.column_by_name::<String>("user_id")?,
                org_id: row.column_by_name::<String>("org_id")?,
                token_hash: row.column_by_name::<String>("token_hash")?,
                user_agent: row.column_by_name::<String>("user_agent")?,
                ip_address: row.column_by_name::<String>("ip_address")?,
                fingerprint: row.column_by_name::<String>("fingerprint")?,
                metadata: Value::Object(Default::default()),
                created_at: row.column_by_name::<String>("created_at")?,
                created_at_epoch: row.column_by_name::<i64>("created_at_epoch")? as u64,
                expires_at: row.column_by_name::<Option<String>>("expires_at")?,
                revoked_at: row.column_by_name::<Option<String>>("revoked_at")?,
            });
        }
        Ok(sessions)
    }

    async fn get_session(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        let mut stmt = Statement::new(
            "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, fingerprint, \
                    CAST(created_at AS STRING) AS created_at, \
                    UNIX_SECONDS(created_at) AS created_at_epoch, \
                    CAST(expires_at AS STRING) AS expires_at, \
                    CAST(revoked_at AS STRING) AS revoked_at \
             FROM sessions WHERE instance_id = @instance_id AND id = @session_id LIMIT 1",
        );
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("session_id", &session_id);
        let mut tx = self.client().single().await?;
        let mut rows = tx.query(stmt).await?;
        Ok(match rows.next().await? {
            Some(row) => Some(SessionRecord {
                id: row.column_by_name::<String>("id")?,
                user_id: row.column_by_name::<String>("user_id")?,
                org_id: row.column_by_name::<String>("org_id")?,
                token_hash: row.column_by_name::<String>("token_hash")?,
                user_agent: row.column_by_name::<String>("user_agent")?,
                ip_address: row.column_by_name::<String>("ip_address")?,
                fingerprint: row.column_by_name::<String>("fingerprint")?,
                metadata: Value::Object(Default::default()),
                created_at: row.column_by_name::<String>("created_at")?,
                created_at_epoch: row.column_by_name::<i64>("created_at_epoch")? as u64,
                expires_at: row.column_by_name::<Option<String>>("expires_at")?,
                revoked_at: row.column_by_name::<Option<String>>("revoked_at")?,
            }),
            None => None,
        })
    }

    async fn revoke_session(&self, instance_id: &str, session_id: &str) -> anyhow::Result<bool> {
        let mut stmt = Statement::new(
            "UPDATE sessions SET revoked_at = CURRENT_TIMESTAMP() \
             WHERE instance_id = @instance_id AND id = @session_id",
        );
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("session_id", &session_id);
        let (_, affected) = self
            .client()
            .read_write_transaction(|tx| {
                let stmt = stmt.clone();
                Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
            })
            .await?;
        Ok(affected > 0)
    }

    async fn create_login_flow(
        &self,
        instance_id: &str,
        input: &NewLoginFlowState,
    ) -> anyhow::Result<()> {
        let data = serde_json::to_string(&input.data).unwrap_or_default();
        let mut stmt = Statement::new(
            "INSERT INTO auth_states (id, instance_id, type, state, redirect_uri, data, step, done) \
             VALUES (@id, @instance_id, 'login_flow', @state, @redirect_uri, @data, 'identifier', FALSE)",
        );
        stmt.add_param("id", &input.flow_id);
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("state", &input.state);
        stmt.add_param("redirect_uri", &input.redirect_uri);
        stmt.add_param("data", &data);
        let _ = self
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

    async fn load_login_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> anyhow::Result<Option<LoginFlowRuntimeState>> {
        let mut stmt = Statement::new(
            "SELECT IFNULL(step, 'identifier') AS step, IFNULL(data, '{}') AS data, IFNULL(redirect_uri, '') AS redirect_uri \
             FROM auth_states \
             WHERE instance_id = @instance_id AND id = @flow_id AND type = 'login_flow' \
               AND done = FALSE AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP()) \
             LIMIT 1",
        );
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("flow_id", &flow_id);
        let mut tx = self.client().single().await?;
        let mut rows = tx.query(stmt).await?;
        Ok(match rows.next().await? {
            Some(row) => Some(LoginFlowRuntimeState {
                flow_id: flow_id.to_string(),
                step: row.column_by_name::<String>("step")?,
                redirect_uri: row.column_by_name::<String>("redirect_uri")?,
                data: serde_json::from_str(&row.column_by_name::<String>("data")?)
                    .unwrap_or_default(),
            }),
            None => None,
        })
    }

    async fn set_login_flow_step(
        &self,
        instance_id: &str,
        flow_id: &str,
        step: &str,
    ) -> anyhow::Result<bool> {
        let mut stmt = Statement::new(
            "UPDATE auth_states SET step = @step \
             WHERE instance_id = @instance_id AND id = @flow_id AND type = 'login_flow' \
               AND done = FALSE AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP())",
        );
        stmt.add_param("step", &step);
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("flow_id", &flow_id);
        let (_, affected) = self
            .client()
            .read_write_transaction(|tx| {
                let stmt = stmt.clone();
                Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
            })
            .await?;
        Ok(affected > 0)
    }

    async fn advance_login_flow_to_password(
        &self,
        instance_id: &str,
        flow_id: &str,
        user_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        let data = serde_json::to_string(data).unwrap_or_else(|_| "{}".into());
        let mut stmt = Statement::new(
            "UPDATE auth_states SET step = 'password', user_id = @user_id, data = @data \
             WHERE instance_id = @instance_id AND id = @flow_id AND type = 'login_flow' \
               AND done = FALSE AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP())",
        );
        stmt.add_param("user_id", &user_id);
        stmt.add_param("data", &data);
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("flow_id", &flow_id);
        let (_, affected) = self
            .client()
            .read_write_transaction(|tx| {
                let stmt = stmt.clone();
                Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
            })
            .await?;
        Ok(affected > 0)
    }

    async fn update_login_flow_data(
        &self,
        instance_id: &str,
        flow_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        let data = serde_json::to_string(data).unwrap_or_else(|_| "{}".into());
        let mut stmt = Statement::new(
            "UPDATE auth_states SET data = @data \
             WHERE instance_id = @instance_id AND id = @flow_id AND type = 'login_flow' \
               AND done = FALSE AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP())",
        );
        stmt.add_param("data", &data);
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("flow_id", &flow_id);
        let (_, affected) = self
            .client()
            .read_write_transaction(|tx| {
                let stmt = stmt.clone();
                Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
            })
            .await?;
        Ok(affected > 0)
    }

    async fn complete_login_flow(&self, instance_id: &str, flow_id: &str) -> anyhow::Result<bool> {
        let mut stmt = Statement::new(
            "UPDATE auth_states SET step = 'complete', done = TRUE \
             WHERE instance_id = @instance_id AND id = @flow_id AND type = 'login_flow' \
               AND done = FALSE AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP())",
        );
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("flow_id", &flow_id);
        let (_, affected) = self
            .client()
            .read_write_transaction(|tx| {
                let stmt = stmt.clone();
                Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
            })
            .await?;
        Ok(affected > 0)
    }

    async fn load_auth_request_redirect(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        if auth_request_id.is_empty() {
            return Ok(None);
        }
        let mut stmt = Statement::new(
            "SELECT redirect_uri, IFNULL(state, '') AS state \
             FROM oidc_auth_requests \
             WHERE instance_id = @instance_id AND id = @id \
               AND done = FALSE \
               AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP()) \
             LIMIT 1",
        );
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("id", &auth_request_id);
        let mut tx = self.client().single().await?;
        let mut rows = tx.query(stmt).await?;
        match rows.next().await? {
            Some(row) => Ok(Some(AuthRequestRedirect {
                redirect_uri: row.column_by_name::<String>("redirect_uri")?,
                state: row.column_by_name::<String>("state")?,
            })),
            None => anyhow::bail!("auth request not found for instance {instance_id}"),
        }
    }

    async fn complete_auth_request(
        &self,
        instance_id: &str,
        auth_request_id: &str,
        user_id: &str,
        session_id: Option<&str>,
        code: &str,
        auth_time: Option<&str>,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        if auth_request_id.is_empty() {
            return Ok(None);
        }
        let select_stmt = {
            let mut stmt = Statement::new(
                "SELECT redirect_uri, IFNULL(state, '') AS state \
                 FROM oidc_auth_requests \
                 WHERE instance_id = @instance_id AND id = @id \
                   AND done = FALSE \
                   AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP()) \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &auth_request_id);
            stmt
        };
        let update_stmt = {
            let mut stmt = if auth_time.is_some() {
                Statement::new(
                    "UPDATE oidc_auth_requests SET user_id = @user_id, session_id = @session_id, done = TRUE, auth_time = TIMESTAMP(@auth_time), code = @code \
                     WHERE instance_id = @instance_id AND id = @id \
                       AND done = FALSE \
                       AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP())",
                )
            } else {
                Statement::new(
                    "UPDATE oidc_auth_requests SET user_id = @user_id, session_id = @session_id, done = TRUE, auth_time = CURRENT_TIMESTAMP(), code = @code \
                     WHERE instance_id = @instance_id AND id = @id \
                       AND done = FALSE \
                       AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP())",
                )
            };
            stmt.add_param("user_id", &user_id);
            stmt.add_param("session_id", &session_id.unwrap_or_default());
            stmt.add_param("code", &code);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &auth_request_id);
            if let Some(auth_time) = auth_time {
                stmt.add_param("auth_time", &auth_time);
            }
            stmt
        };
        let (_, (redirect, affected)) = self
            .client()
            .read_write_transaction(|tx| {
                let select_stmt = select_stmt.clone();
                let update_stmt = update_stmt.clone();
                Box::pin(async move {
                    let mut rows = tx.query(select_stmt).await?;
                    let redirect = match rows.next().await? {
                        Some(row) => Some(AuthRequestRedirect {
                            redirect_uri: row.column_by_name::<String>("redirect_uri")?,
                            state: row.column_by_name::<String>("state")?,
                        }),
                        None => None,
                    };
                    let affected = if redirect.is_some() {
                        tx.update(update_stmt).await?
                    } else {
                        0
                    };
                    Ok::<(Option<AuthRequestRedirect>, i64), SpannerError>((redirect, affected))
                })
            })
            .await?;
        if affected == 0 {
            anyhow::bail!("auth request not found for instance {instance_id}");
        }
        Ok(redirect)
    }

    async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<AuthRequestRequirements> {
        let mut stmt = Statement::new(
            "SELECT IFNULL(prompt, '[]') AS prompt, max_age \
             FROM oidc_auth_requests \
             WHERE instance_id = @instance_id AND id = @id \
               AND done = FALSE \
               AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP()) \
             LIMIT 1",
        );
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("id", &auth_request_id);
        let mut tx = self.client().single().await?;
        let mut rows = tx.query(stmt).await?;
        Ok(match rows.next().await? {
            Some(row) => AuthRequestRequirements {
                prompt: serde_json::from_str(&row.column_by_name::<String>("prompt")?)
                    .unwrap_or_default(),
                max_age: row
                    .column_by_name::<Option<i64>>("max_age")?
                    .and_then(|value| u64::try_from(value).ok()),
            },
            None => AuthRequestRequirements::default(),
        })
    }

    async fn create_provider_auth_state(
        &self,
        instance_id: &str,
        state: &ProviderAuthState,
    ) -> anyhow::Result<()> {
        let mut stmt = Statement::new(
            "INSERT INTO oidc_rp_auth_states \
             (id, instance_id, provider_id, state, nonce, pkce_verifier, flow_id, redirect_uri, expected_issuer, callback_uri) \
             VALUES (@id, @instance_id, @provider_id, @state, @nonce, @pkce_verifier, @flow_id, @redirect_uri, @expected_issuer, @callback_uri)",
        );
        stmt.add_param("id", &Uuid::new_v4().to_string());
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("provider_id", &state.provider_id);
        stmt.add_param("state", &state.state);
        stmt.add_param("nonce", &state.nonce);
        stmt.add_param("pkce_verifier", &state.pkce_verifier);
        stmt.add_param("flow_id", &state.flow_id);
        stmt.add_param("redirect_uri", &state.redirect_uri);
        stmt.add_param("expected_issuer", &state.expected_issuer);
        stmt.add_param("callback_uri", &state.callback_uri);
        let _ = self
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

    async fn consume_provider_auth_state(
        &self,
        instance_id: &str,
        state: &str,
    ) -> anyhow::Result<Option<ProviderAuthState>> {
        let instance_id = instance_id.to_string();
        let state = state.to_string();
        let (_, result) = self
            .client()
            .read_write_transaction(move |tx| {
                let instance_id = instance_id.clone();
                let state = state.clone();
                Box::pin(async move {
                    let mut select_stmt = Statement::new(
                        "SELECT id, provider_id, state, nonce, pkce_verifier, flow_id, redirect_uri, expected_issuer, callback_uri \
                         FROM oidc_rp_auth_states \
                         WHERE instance_id = @instance_id AND state = @state \
                           AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP()) \
                         LIMIT 1",
                    );
                    select_stmt.add_param("instance_id", &instance_id);
                    select_stmt.add_param("state", &state);

                    let mut rows = tx.query(select_stmt).await?;
                    let row = match rows.next().await? {
                        Some(row) => row,
                        None => return Ok::<Option<ProviderAuthState>, SpannerError>(None),
                    };

                    let id = row.column_by_name::<String>("id")?;
                    let parsed = ProviderAuthState {
                        provider_id: row.column_by_name::<String>("provider_id")?,
                        state: row.column_by_name::<String>("state")?,
                        nonce: row.column_by_name::<String>("nonce")?,
                        pkce_verifier: row.column_by_name::<String>("pkce_verifier")?,
                        flow_id: row.column_by_name::<String>("flow_id")?,
                        redirect_uri: row.column_by_name::<String>("redirect_uri")?,
                        expected_issuer: row.column_by_name::<String>("expected_issuer")?,
                        callback_uri: row.column_by_name::<String>("callback_uri")?,
                    };

                    let mut delete_stmt = Statement::new(
                        "DELETE FROM oidc_rp_auth_states WHERE instance_id = @instance_id AND id = @id",
                    );
                    delete_stmt.add_param("instance_id", &instance_id);
                    delete_stmt.add_param("id", &id);
                    tx.update(delete_stmt).await?;

                    Ok(Some(parsed))
                })
            })
            .await?;
        Ok(result)
    }
}
