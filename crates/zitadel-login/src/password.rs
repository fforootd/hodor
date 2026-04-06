use axum::{Json, response::IntoResponse, response::Response};

use crate::LoginState;
use crate::steps::{FlowStepResponse, LoginStep};
use crate::ui::{UINode, default_branding, password_step_nodes};
use zitadel_app::ActorContext;
use zitadel_app::credentials::{SetPasswordCommand, VerifyPasswordCommand};

/// Verify the user's password and transparently migrate the hash if needed.
///
/// On success returns `Ok(())`. On failure (wrong password, user not found,
/// etc.) returns a UI-friendly error `Response` that keeps the user on the
/// password step.
pub(crate) async fn verify_and_migrate(
    state: &LoginState,
    ctx: &ActorContext,
    user_id: &str,
    password: &str,
    identifier: &str,
    flow_id: &str,
) -> Result<(), Response> {
    // Use the verify_password use case to load the password hash from the repository.
    let uc_result = state
        .app
        .verify_password
        .execute(
            ctx,
            VerifyPasswordCommand {
                user_id: user_id.to_string(),
                password: password.to_string(),
            },
        )
        .await;

    let hash = match uc_result {
        Ok(result) => result.new_hash,
        Err(_) => None,
    };

    // The use case returns the stored hash for the transport adapter to verify
    // via the password Swapper (which handles argon2id, bcrypt, etc).
    let verify_result = match hash.as_deref() {
        Some(h) => state.passwords.verify(h, password).ok(),
        None => None,
    };

    let verify_result = match verify_result {
        Some(result) => result,
        None => {
            return Err(Json(FlowStepResponse {
                flow_id: flow_id.to_string(),
                step: LoginStep::Password.as_str().into(),
                nodes: {
                    let mut n = vec![UINode::Error {
                        message: "Invalid credentials".into(),
                    }];
                    n.extend(password_step_nodes(identifier));
                    n
                },
                redirect_uri: None,
                branding: Some(default_branding()),
                ..Default::default()
            })
            .into_response());
        }
    };

    // Transparent hash migration: if the stored hash uses an outdated algorithm,
    // re-hash and persist the updated credential via the set_password use case.
    if let zitadel_authn::password::VerifyResult::NeedUpdate(new_hash) = verify_result {
        let cred_json = zitadel_authn::password::encode_credential_json(&new_hash);
        let _ = state
            .app
            .set_password
            .execute(
                ctx,
                SetPasswordCommand {
                    user_id: user_id.to_string(),
                    password_hash: cred_json,
                },
            )
            .await;
    }

    Ok(())
}
