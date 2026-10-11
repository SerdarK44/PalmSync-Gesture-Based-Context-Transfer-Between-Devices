// The product version lives in the repo-root VERSION file (bumped by
// release.ps1). Expose it as PALMSYNC_VERSION so the handshake announces the same
// product version as the daemon, installer, tray app and camera module.
fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../VERSION");
    println!("cargo:rerun-if-changed={}", path);
    let version = std::fs::read_to_string(path)
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    println!("cargo:rustc-env=PALMSYNC_VERSION={}", version);
}
