use axum::{
    Form,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use uuid::Uuid;
use zitadel_db::current_instance_id;

use crate::{
    LoginState,
    redirect::{build_auth_error_redirect, build_auth_redirect},
    session::{extract_session_user, now_epoch_seconds, session_satisfies_max_age},
};

#[derive(Deserialize, Default)]
pub(crate) struct ConformanceLoginQuery {
    #[serde(default)]
    auth_request_id: String,
}

#[derive(Deserialize, Default)]
pub(crate) struct ConformanceLoginForm {
    #[serde(default)]
    auth_request_id: String,
    #[serde(default)]
    identifier: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    action: String,
}

pub(crate) async fn login_get(
    State(state): State<LoginState>,
    headers: HeaderMap,
    Query(query): Query<ConformanceLoginQuery>,
) -> Response {
    let instance_id = current_instance_id();
    if !state.conformance_login_html {
        let target = if query.auth_request_id.is_empty() {
            "/login".to_string()
        } else {
            format!("/login?auth_request_id={}", query.auth_request_id)
        };
        return Redirect::temporary(&target).into_response();
    }

    if query.auth_request_id.is_empty() {
        return html_response(
            StatusCode::BAD_REQUEST,
            render_error_page("Missing auth_request_id"),
        );
    }

    let requirements = match state
        .transient
        .load_auth_request_prompts(&instance_id, &query.auth_request_id)
        .await
    {
        Ok(requirements) => requirements,
        Err(_) => {
            return html_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                render_error_page("Failed to load authorization request"),
            );
        }
    };

    let prompts = requirements.prompt;
    let allow_reuse =
        !prompts.contains(&"login".to_string()) && !prompts.contains(&"select_account".to_string());
    let silent = prompts.contains(&"none".to_string());

    if let Some(session_user) = extract_session_user(&state, &headers).await {
        let can_reuse = allow_reuse
            && session_satisfies_max_age(
                session_user.authenticated_at_epoch,
                requirements.max_age,
                now_epoch_seconds(),
            );
        if can_reuse {
            return complete_auth_request_redirect(
                &state,
                &query.auth_request_id,
                &session_user.user_id,
                Some(&session_user.session_id),
                Some(&session_user.authenticated_at),
            )
            .await;
        }
        if silent {
            return auth_error_redirect_response(&state, &query.auth_request_id, "login_required")
                .await;
        }
    } else if silent {
        return auth_error_redirect_response(&state, &query.auth_request_id, "login_required")
            .await;
    }

    html_response(
        StatusCode::OK,
        render_login_page(&query.auth_request_id, None, None),
    )
}

pub(crate) async fn login_post(
    State(state): State<LoginState>,
    headers: HeaderMap,
    Form(form): Form<ConformanceLoginForm>,
) -> Response {
    let instance_id = current_instance_id();
    if !state.conformance_login_html {
        return Redirect::temporary("/login").into_response();
    }

    if form.auth_request_id.is_empty() {
        return html_response(
            StatusCode::BAD_REQUEST,
            render_error_page("Missing auth_request_id"),
        );
    }

    if form.action == "use_session" {
        let Some(session_user) = extract_session_user(&state, &headers).await else {
            return html_response(
                StatusCode::UNAUTHORIZED,
                render_login_page(
                    &form.auth_request_id,
                    Some(&form.identifier),
                    Some("Session no longer available. Sign in again."),
                ),
            );
        };
        let requirements = match state
            .transient
            .load_auth_request_prompts(&instance_id, &form.auth_request_id)
            .await
        {
            Ok(requirements) => requirements,
            Err(_) => {
                return html_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    render_error_page("Failed to load authorization request"),
                );
            }
        };
        if !session_satisfies_max_age(
            session_user.authenticated_at_epoch,
            requirements.max_age,
            now_epoch_seconds(),
        ) {
            return html_response(
                StatusCode::UNAUTHORIZED,
                render_login_page(
                    &form.auth_request_id,
                    Some(&form.identifier),
                    Some("A fresh sign-in is required."),
                ),
            );
        }
        return complete_auth_request_redirect(
            &state,
            &form.auth_request_id,
            &session_user.user_id,
            Some(&session_user.session_id),
            Some(&session_user.authenticated_at),
        )
        .await;
    }

    if form.action == "back" {
        return html_response(
            StatusCode::OK,
            render_login_page(&form.auth_request_id, None, None),
        );
    }

    let user = match state
        .stateful
        .find_active_user_by_identifier(&instance_id, &form.identifier)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return html_response(
                StatusCode::UNAUTHORIZED,
                render_login_page(
                    &form.auth_request_id,
                    Some(&form.identifier),
                    Some("Invalid credentials"),
                ),
            );
        }
        Err(_) => {
            return html_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                render_error_page("Failed to look up user"),
            );
        }
    };

    let hash = match state
        .stateful
        .load_password_hash(&instance_id, &user.user_id)
        .await
    {
        Ok(Some(hash)) => hash,
        Ok(None) => {
            return html_response(
                StatusCode::UNAUTHORIZED,
                render_login_page(
                    &form.auth_request_id,
                    Some(&form.identifier),
                    Some("Invalid credentials"),
                ),
            );
        }
        Err(_) => {
            return html_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                render_error_page("Failed to load password"),
            );
        }
    };

    if state.passwords.verify(&hash, &form.password).is_err() {
        return html_response(
            StatusCode::UNAUTHORIZED,
            render_login_page(
                &form.auth_request_id,
                Some(&form.identifier),
                Some("Invalid credentials"),
            ),
        );
    }

    let created_session = match state
        .transient
        .create_session(&instance_id, &user.user_id, &user.org_id, "", "", "")
        .await
    {
        Ok(session) => session,
        Err(_) => {
            return html_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                render_error_page("Failed to create session"),
            );
        }
    };

    let redirect = match complete_auth_request_location(
        &state,
        &form.auth_request_id,
        &user.user_id,
        Some(&created_session.session_id),
        Some(&created_session.created_at),
    )
    .await
    {
        Ok(redirect) => redirect,
        Err(response) => return response,
    };

    redirect_with_session_cookie(&redirect, &state.cookie_config, &created_session.token)
}

async fn complete_auth_request_redirect(
    state: &LoginState,
    auth_request_id: &str,
    user_id: &str,
    session_id: Option<&str>,
    auth_time: Option<&str>,
) -> Response {
    match complete_auth_request_location(state, auth_request_id, user_id, session_id, auth_time)
        .await
    {
        Ok(redirect) => Redirect::temporary(&redirect).into_response(),
        Err(response) => response,
    }
}

async fn complete_auth_request_location(
    state: &LoginState,
    auth_request_id: &str,
    user_id: &str,
    session_id: Option<&str>,
    auth_time: Option<&str>,
) -> Result<String, Response> {
    let instance_id = current_instance_id();
    let code = Uuid::new_v4().to_string();
    let auth_request = match state
        .transient
        .complete_auth_request(
            &instance_id,
            auth_request_id,
            user_id,
            session_id,
            &code,
            auth_time,
        )
        .await
    {
        Ok(Some(auth_req)) => auth_req,
        Ok(None) => {
            return Err(html_response(
                StatusCode::BAD_REQUEST,
                render_error_page("Authorization request no longer exists"),
            ));
        }
        Err(error) => {
            return Err(html_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                render_error_page(&format!(
                    "Failed to complete authorization request: {error}"
                )),
            ));
        }
    };

    Ok(build_auth_redirect(
        &auth_request.redirect_uri,
        &auth_request.state,
        &code,
    ))
}

async fn auth_error_redirect_response(
    state: &LoginState,
    auth_request_id: &str,
    error: &str,
) -> Response {
    let instance_id = current_instance_id();
    match state
        .transient
        .load_auth_request_redirect(&instance_id, auth_request_id)
        .await
    {
        Ok(Some(auth_req)) => {
            let redirect = build_auth_error_redirect(
                &auth_req.redirect_uri,
                &auth_req.state,
                error,
                "prompt=none requires an existing session",
            );
            Redirect::temporary(&redirect).into_response()
        }
        Ok(None) => html_response(
            StatusCode::BAD_REQUEST,
            render_error_page("Authorization request no longer exists"),
        ),
        Err(_) => html_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            render_error_page("Failed to load authorization request"),
        ),
    }
}

fn redirect_with_session_cookie(
    location: &str,
    cookie_config: &zitadel_authn::cookie::CookieConfig,
    token: &str,
) -> Response {
    let signed = zitadel_authn::cookie::sign(token, &cookie_config.secrets[0]);
    let cookie_name = cookie_config.cookie_name();
    let secure_flag = if cookie_config.secure { "; Secure" } else { "" };
    let cookie_value = format!(
        "{cookie_name}={signed}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{secure_flag}",
        cookie_config.max_age,
    );

    let mut response = Redirect::temporary(location).into_response();
    if let Ok(value) = cookie_value.parse() {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

fn html_response(status: StatusCode, html: String) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .expect("html response")
}

fn render_login_page(
    auth_request_id: &str,
    identifier: Option<&str>,
    error: Option<&str>,
) -> String {
    let error_html = error
        .map(|message| format!("<p style=\"color:#b91c1c\">{message}</p>"))
        .unwrap_or_default();
    let identifier = identifier.unwrap_or_default();

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Sign in</title></head>\
         <body><main><h1>Sign in</h1>{error_html}\
         <form method=\"post\" action=\"/conformance/login\">\
         <input type=\"hidden\" name=\"auth_request_id\" value=\"{auth_request_id}\">\
         <label>Email or username<input name=\"identifier\" value=\"{identifier}\" autocomplete=\"username\"></label>\
         <label>Enter your password<input type=\"password\" name=\"password\" autocomplete=\"current-password\"></label>\
         <button type=\"submit\">Sign in</button>\
         </form></main></body></html>"
    )
}

#[allow(dead_code)]
fn render_session_reuse_page(
    auth_request_id: &str,
    identifier: &str,
    display_name: &str,
) -> String {
    let account = if display_name.is_empty() {
        identifier.to_string()
    } else {
        format!("{display_name} · {identifier}")
    };

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Reuse session</title></head>\
         <body><main><h1>Use your existing session?</h1><p>{account}</p>\
         <form method=\"post\" action=\"/conformance/login\">\
         <input type=\"hidden\" name=\"auth_request_id\" value=\"{auth_request_id}\">\
         <button type=\"submit\" name=\"action\" value=\"use_session\">Continue with this session</button>\
         <button type=\"submit\" name=\"action\" value=\"back\">Use a different account</button>\
         </form></main></body></html>"
    )
}

fn render_error_page(message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Login error</title></head>\
         <body><main><h1>Login error</h1><p>{message}</p></main></body></html>"
    )
}
