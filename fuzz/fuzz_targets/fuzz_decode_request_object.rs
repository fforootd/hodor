#![no_main]
use libfuzzer_sys::fuzz_target;

// Fuzz the unsigned JWT request object parser (base64url decode + JSON).
// Invariant: must never panic on any input.
fuzz_target!(|data: &str| {
    let _ = zitadel_oidc::authorize::decode_request_object(data);
});
