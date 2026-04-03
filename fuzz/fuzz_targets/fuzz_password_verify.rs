#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Input {
    encoded_hash: String,
    password: String,
}

// Fuzz password hash format parsing and verification.
// Uses dev params (fast argon2id) to keep throughput high.
// Invariant: must never panic.
fuzz_target!(|input: Input| {
    let swapper = zitadel_authn::password::Swapper::dev();
    let _ = swapper.verify(&input.encoded_hash, &input.password);
});
