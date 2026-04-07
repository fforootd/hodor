fn main() {
    // Trigger rebuild when the embedded web/dist assets change.
    // With rust-embed's `debug-embed` feature, assets are always baked into
    // the binary (even in debug builds), so this primarily helps cargo
    // detect when web/dist goes from placeholder to real content.
    println!("cargo:rerun-if-changed=../../web/dist");
}
