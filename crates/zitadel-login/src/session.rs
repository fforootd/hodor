use crate::LoginState;
use zitadel_db::{current_instance_id, load_session_user_profile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionUser {
    pub user_id: String,
    pub identifier: String,
    pub display_name: String,
    pub authenticated_at: String,
    pub authenticated_at_epoch: u64,
}

pub(crate) fn session_satisfies_max_age(
    authenticated_at_epoch: u64,
    max_age: Option<u64>,
    now_epoch: u64,
) -> bool {
    match max_age {
        Some(max_age) => now_epoch.saturating_sub(authenticated_at_epoch) <= max_age,
        None => true,
    }
}

pub(crate) fn now_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

/// Extract the current session and user details from the session cookie if present.
pub(crate) async fn extract_session_user(
    state: &LoginState,
    headers: &axum::http::HeaderMap,
) -> Option<SessionUser> {
    let instance_id = current_instance_id();
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;

    // Parse cookies to find the session cookie.
    let cookie_name = state.cookie_config.cookie_name();
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{cookie_name}=")) {
            let token = zitadel_authn::cookie::verify(value, &state.cookie_config.secrets)?;
            let session = state
                .transient
                .find_session_by_token(&instance_id, &token)
                .await
                .ok()??;

            // Load user details.
            let (identifier, display_name) =
                load_session_user_profile(&state.db, &instance_id, &session.user_id)
                    .await
                    .ok()??;
            return Some(SessionUser {
                user_id: session.user_id,
                identifier,
                display_name,
                authenticated_at: session.created_at,
                authenticated_at_epoch: session.created_at_epoch,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::session_satisfies_max_age;

    #[test]
    fn max_age_accepts_fresh_session() {
        assert!(session_satisfies_max_age(100, Some(10), 109));
    }

    #[test]
    fn max_age_rejects_stale_session() {
        assert!(!session_satisfies_max_age(100, Some(10), 111));
    }
}
