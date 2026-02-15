use super::*;

fn linux_target() -> TargetTripleComponents {
    TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap()
}

fn macos_target() -> TargetTripleComponents {
    TargetTripleComponents::parse("x86_64-apple-darwin").unwrap()
}

fn windows_target() -> TargetTripleComponents {
    TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap()
}

fn wasm_target() -> TargetTripleComponents {
    TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap()
}

#[test]
fn test_syslib_config_for_target() {
    let target = linux_target();
    let config = SysLibConfig::for_target(&target).unwrap();

    assert_eq!(config.target().arch, "x86_64");
    assert_eq!(config.target().os, "linux");
}

#[test]
fn test_syslib_config_with_sysroot() {
    let target = linux_target();
    let sysroot = PathBuf::from("/opt/custom-sysroot");
    let config = SysLibConfig::with_sysroot(&target, sysroot.clone());

    assert_eq!(config.sysroot(), Some(&sysroot));
}

#[test]
fn test_required_libraries_linux() {
    let target = linux_target();
    let config = SysLibConfig::for_target(&target).unwrap();
    let libs = config.required_libraries();

    assert!(libs.contains(&"c"));
    assert!(libs.contains(&"m"));
    assert!(libs.contains(&"pthread"));
    assert!(libs.contains(&"dl"));
}

#[test]
fn test_required_libraries_macos() {
    let target = macos_target();
    let config = SysLibConfig::for_target(&target).unwrap();
    let libs = config.required_libraries();

    assert!(libs.contains(&"c"));
    assert!(libs.contains(&"m"));
    assert!(libs.contains(&"System"));
    assert!(!libs.contains(&"pthread")); // macOS uses libSystem
}

#[test]
fn test_required_libraries_wasm() {
    let target = wasm_target();
    let config = SysLibConfig::for_target(&target).unwrap();
    let libs = config.required_libraries();

    assert!(libs.is_empty());
}

#[test]
fn test_required_libraries_windows() {
    let target = windows_target();
    let config = SysLibConfig::for_target(&target).unwrap();
    let libs = config.required_libraries();

    // Windows libraries are linked automatically
    assert!(libs.is_empty());
}

#[test]
fn test_sysroot_env_var() {
    // This test verifies the environment variable format
    let target = linux_target();
    let env_key = format!(
        "ORI_SYSROOT_{}",
        target.to_string().to_uppercase().replace('-', "_")
    );
    assert_eq!(env_key, "ORI_SYSROOT_X86_64_UNKNOWN_LINUX_GNU");
}

#[test]
fn test_library_search_order_default() {
    assert_eq!(LibrarySearchOrder::default(), LibrarySearchOrder::UserFirst);
}

#[test]
fn test_syslib_error_display() {
    let err = SysLibError {
        target: "x86_64-linux-musl".to_string(),
        message: "sysroot not found".to_string(),
    };

    let msg = err.to_string();
    assert!(msg.contains("x86_64-linux-musl"));
    assert!(msg.contains("sysroot not found"));
}

#[test]
fn test_is_native() {
    let target = linux_target();
    let config = SysLibConfig::for_target(&target).unwrap();

    // This will be true or false depending on the host platform
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    assert!(config.is_native());

    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    assert!(!config.is_native());
}

#[test]
fn test_sysroot_candidates_linux() {
    let target = linux_target();
    let candidates = SysLibConfig::sysroot_candidates(&target);

    // Should include multiarch path
    assert!(candidates
        .iter()
        .any(|p| p.to_string_lossy().contains("x86_64")));
}

#[test]
fn test_sysroot_candidates_wasm() {
    let target = wasm_target();
    let candidates = SysLibConfig::sysroot_candidates(&target);

    // Should include WASI SDK paths
    assert!(candidates
        .iter()
        .any(|p| p.to_string_lossy().contains("wasi")));
}

#[test]
fn test_find_library_not_found() {
    let target = linux_target();
    let paths = vec![PathBuf::from("/nonexistent/path")];

    assert!(find_library("nonexistent", &paths, &target).is_none());
}

#[test]
fn test_library_exists() {
    let target = linux_target();
    let paths = vec![PathBuf::from("/nonexistent/path")];

    assert!(!library_exists("nonexistent", &paths, &target));
}
