use crate::LoginState;

/// Extract user_id, identifier, and display_name from the session cookie if present.
pub(crate) async fn extract_session_user(
    state: &LoginState,
    headers: &axum::http::HeaderMap,
) -> Option<(String, String, String)> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;

    // Parse cookies to find the session cookie.
    let cookie_name = state.cookie_config.cookie_name();
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{cookie_name}=")) {
            let token = zitadel_authn::cookie::verify(value, &state.cookie_config.secrets)?;
            let scoped = state.db.scoped_default();
            let session_store = zitadel_authn::session::SessionStore::new(state.db.clone());
            let session = session_store.find_by_token(&scoped, &token).await.ok()??;

            // Load user details.
            let user: Option<(String, String)> = sqlx::query_as(
                "SELECT identifier, display_name FROM users WHERE instance_id = $1 AND id = $2",
            )
            .bind(scoped.instance_id())
            .bind(&session.user_id)
            .fetch_optional(scoped.pool())
            .await
            .ok()?;

            let (identifier, display_name) = user?;
            return Some((session.user_id, identifier, display_name));
        }
    }
    None
}
