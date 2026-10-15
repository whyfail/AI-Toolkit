fn main() {
    let attributes = tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new_without_app_manifest(),
    );
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
    embed_windows_test_manifest();
}

// tauri-build links its Common-Controls v6 manifest via a .res that only
// reaches lib and bin builds, so `cargo test` executables start with
// Common-Controls v5 and die with STATUS_ENTRYPOINT_NOT_FOUND
// (tauri-apps/tauri#11028). Providing the manifest once for every target
// via /MANIFEST:EMBED replaces it — the .res and /MANIFEST:EMBED cannot
// coexist (CVT1100 duplicate resource), so the winresource one is disabled
// above. Same workaround as tauri's own workspace build script.
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
