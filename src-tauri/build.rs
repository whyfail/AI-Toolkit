fn main() {
    tauri_build::build();
    embed_windows_test_manifest();
}

// tauri-build only links its Common-Controls v6 manifest into bin targets, so
// `cargo test` executables start with Common-Controls v5 and die with
// STATUS_ENTRYPOINT_NOT_FOUND once a dependency imports the v6 entry points
// (tauri-apps/tauri#11028). Same workaround as tauri's own workspace: the
// all-targets flag is the only one that also covers the lib unit-test harness,
// and it coexists with the bin's .res manifest just fine.
fn embed_windows_test_manifest() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
