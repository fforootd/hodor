#![no_main]
use libfuzzer_sys::fuzz_target;
use zitadel_crypto::generator::{GeneratorProfile, generate};

// Fuzz code generation with arbitrary profiles.
// Tests charset resolution, length handling, and dash insertion.
// Invariant: must never panic (the assert on empty charset should
// be unreachable due to the fallback in charset_bytes).
fuzz_target!(|profile: GeneratorProfile| {
    // Cap length to avoid OOM — fuzzer could generate huge values.
    if profile.length > 1024 {
        return;
    }
    let _ = generate(&profile);
});
