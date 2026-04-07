fn main() {
    // The server binary embeds the built frontend from web/dist.
    // Rebuild whenever those assets change so CI browser binaries do not
    // accidentally serve placeholder or stale HTML from cache.
    println!("cargo:rerun-if-changed=../../web/dist");
}
