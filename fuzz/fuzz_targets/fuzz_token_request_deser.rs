#![no_main]
use libfuzzer_sys::fuzz_target;
use zitadel_oidc::token::TokenRequest;

// Fuzz form-encoded deserialization of the OIDC token request.
// Invariant: must never panic on any input.
fuzz_target!(|data: &[u8]| {
    let _ = serde_urlencoded::from_bytes::<TokenRequest>(data);
});
