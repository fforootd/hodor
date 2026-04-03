#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Input {
    authorization: Option<String>,
    form_client_id: String,
    form_client_secret: String,
}

// Fuzz client authentication extraction from Authorization header + form fields.
// Invariant: must never panic.
fuzz_target!(|input: Input| {
    let auth_ref = input.authorization.as_deref();
    let _ = zitadel_oidc::op::resolve_client_auth(
        auth_ref,
        &input.form_client_id,
        &input.form_client_secret,
    );
});
