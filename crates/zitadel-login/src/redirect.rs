/// Build a successful OIDC auth redirect URL with code and optional state.
pub(crate) fn build_auth_redirect(redirect_uri: &str, state: &str, code: &str) -> String {
    let state_param = if state.is_empty() {
        String::new()
    } else {
        format!("&state={state}")
    };
    format!("{redirect_uri}?code={code}{state_param}")
}

/// Build an OIDC error redirect URL.
pub(crate) fn build_auth_error_redirect(
    redirect_uri: &str,
    state: &str,
    error: &str,
    description: &str,
) -> String {
    let mut url = format!(
        "{redirect_uri}?error={}&error_description={}",
        urlencoding_encode(error),
        urlencoding_encode(description),
    );
    if !state.is_empty() {
        url.push_str("&state=");
        url.push_str(&urlencoding_encode(state));
    }
    url
}

pub(crate) fn urlencoding_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
