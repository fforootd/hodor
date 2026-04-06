use zitadel_authn::cookie::CookieConfig;

/// Build the `Set-Cookie` header value for a newly created session.
pub(crate) fn build_session_cookie(cookie_config: &CookieConfig, session_token: &str) -> String {
    let signed = zitadel_authn::cookie::sign(session_token, &cookie_config.secrets[0]);
    let cookie_name = cookie_config.cookie_name();
    let secure_flag = if cookie_config.secure { "; Secure" } else { "" };
    format!(
        "{cookie_name}={signed}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{secure_flag}",
        cookie_config.max_age,
    )
}
