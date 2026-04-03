#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

#[derive(Arbitrary, Debug)]
struct Input {
    ciphertext: Vec<u8>,
    nonce: Vec<u8>,
    key_id: String,
}

// Fuzz AES-256-GCM decryption with arbitrary nonce lengths and key IDs.
// Before the nonce-length fix, this would panic on nonce.len() != 12.
// Invariant: must never panic.
fuzz_target!(|input: Input| {
    let mut keys = HashMap::new();
    keys.insert("k1".to_string(), "a".repeat(64));
    let sb = zitadel_crypto::SecretBox::new("k1", &keys).unwrap();
    let _ = sb.open(&input.ciphertext, &input.nonce, &input.key_id);
});
