#![no_main]
use libfuzzer_sys::fuzz_target;

// Fuzz the manual URL form-encoded authorize parameter parser.
// Invariant: must never panic on any input.
fuzz_target!(|data: &str| {
    let _ = zitadel_oidc::authorize::parse_authorize_params(data);
});
