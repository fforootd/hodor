fn main() {
    // The server binary embeds the built frontend from web/dist via rust-embed.
    // cargo:rerun-if-changed on a *directory* only triggers on the directory's
    // own mtime, not on changes to files inside it. We must walk the tree so
    // that cargo detects when the Rust build cache has stale placeholder assets
    // and the real web build is now present.
    let dist = std::path::Path::new("../../web/dist");
    println!("cargo:rerun-if-changed={}", dist.display());
    if dist.is_dir() {
        walk_and_track(dist);
    }
}

fn walk_and_track(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        println!("cargo:rerun-if-changed={}", path.display());
        if path.is_dir() {
            walk_and_track(&path);
        }
    }
}
