#![no_main]
use libfuzzer_sys::fuzz_target;

// Fuzz the HMAC-SHA256 cookie verification path.
// Invariant: must never panic. Any input must return None or Some(token).
fuzz_target!(|data: &str| {
    let secrets = vec!["fuzz-secret-1".to_string(), "fuzz-secret-2".to_string()];
    let _ = zitadel_authn::cookie::verify(data, &secrets);
});
