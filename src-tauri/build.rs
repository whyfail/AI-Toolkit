fn main() {
    tauri_build::build();
    embed_windows_test_manifest();
}

// tauri-build only links its Common-Controls v6 manifest into bin targets, so
// `cargo test` executables start with Common-Controls v5 and die with
// STATUS_ENTRYPOINT_NOT_FOUND once a dependency imports the v6 entry points
// (tauri-apps/tauri#11028). Same workaround as tauri's own workspace, scoped
// to tests so it does not clash with the bin target's embedded manifest.
fn embed_windows_test_manifest() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
        manifest.display()
    );
}
