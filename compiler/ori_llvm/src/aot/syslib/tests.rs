use super::*;
use crate::aot::target_features::Arch;

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

    assert_eq!(config.target().arch, Arch::X86_64);
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

// =============================================================================
// Install-paths SSOT tests
//
// Regression: BUG-04-045 / TPR-BUG-04-045-06. Before this SSOT was extracted,
// `oric::commands::target` hard-coded `~/.ori/sysroots/<target>` as the
// install location while `SysLibConfig::detect_sysroot` only looked at env
// vars and `/opt/wasi-sdk` / `/usr/share/wasi-sysroot` system paths. The
// install side reported success whose results were invisible to subsequent
// builds. The fix is a single canonical home for "where Ori puts and
// looks for managed sysroots", consulted by both sides.
// =============================================================================

#[test]
fn test_ori_sysroots_dir_for_home_uses_dot_ori_sysroots() {
    let home = PathBuf::from("/tmp/test-home");
    let dir = ori_sysroots_dir_for_home(&home);
    assert_eq!(dir, PathBuf::from("/tmp/test-home/.ori/sysroots"));
}

#[test]
fn test_ori_sysroot_path_for_home_appends_canonical_key() {
    let home = PathBuf::from("/tmp/test-home");
    let path = ori_sysroot_path_for_home(&home, "aarch64-apple-darwin");
    assert_eq!(
        path,
        PathBuf::from("/tmp/test-home/.ori/sysroots/aarch64-apple-darwin")
    );
}

#[test]
fn test_home_wasi_sdk_sysroot_for_home_uses_dot_wasi_sdk() {
    let home = PathBuf::from("/tmp/test-home");
    let path = home_wasi_sdk_sysroot_for_home(&home);
    assert_eq!(
        path,
        PathBuf::from("/tmp/test-home/.wasi-sdk/share/wasi-sysroot")
    );
}

/// Round-trip pin: a sysroot installed under `~/.ori/sysroots/<canonical-key>`
/// is discoverable via `SysLibConfig::detect_sysroot_for_home` for any
/// aliased or versioned input that canonicalizes to that key. This is the
/// contract that makes the install side and the discovery side agree.
///
/// Uses the parameterized `_for_home` variant against a tempdir so the
/// test is hermetic — no process-global `HOME`/`USERPROFILE` mutation
/// (which would race with parallel tests).
#[test]
fn test_install_then_detect_round_trip_canonical_darwin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    let canonical = "aarch64-apple-darwin";

    // Simulate `ori target add` writing to the canonical install location.
    let install = ori_sysroot_path_for_home(home, canonical);
    std::fs::create_dir_all(&install).expect("mkdir install");

    // Parse a versioned darwin spelling — must canonicalize to
    // `aarch64-apple-darwin` and discover the install above.
    let target = TargetTripleComponents::parse("arm64-apple-darwin25.2.0").unwrap();
    assert_eq!(
        target.support_key(),
        canonical,
        "support_key must canonicalize to the install directory name"
    );

    let detected = SysLibConfig::detect_sysroot_for_home(&target, home);
    assert_eq!(
        detected.as_deref(),
        Some(install.as_path()),
        "discovery must find the install at the canonical location"
    );
}

/// Sibling: a sysroot installed under the canonical name is also
/// discoverable when the user later types the unversioned canonical
/// spelling — proves the round-trip is direction-symmetric.
#[test]
fn test_install_then_detect_round_trip_unversioned_input() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    let canonical = "aarch64-apple-darwin";

    let install = ori_sysroot_path_for_home(home, canonical);
    std::fs::create_dir_all(&install).expect("mkdir install");

    let target = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
    let detected = SysLibConfig::detect_sysroot_for_home(&target, home);
    assert_eq!(detected.as_deref(), Some(install.as_path()));
}

/// Negative pin: nothing installed → discovery returns None for the
/// per-user install location (assuming no system sysroot exists at the
/// hard-coded paths and no env override is set).
#[test]
fn test_detect_sysroot_for_home_returns_none_when_nothing_installed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    // Use a target whose hard-coded system candidates won't exist on
    // CI (Windows MSVC sysroot at /opt/windows-sdk).
    let target = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    let detected = SysLibConfig::detect_sysroot_for_home(&target, home);
    assert!(
        detected.is_none(),
        "no install should yield no detection, got: {detected:?}"
    );
}

/// WASI Preview1 install under the per-user `~/.wasi-sdk` location is
/// in the candidate list. The candidate list must include the home WASI
/// SDK path BEFORE the system-wide `/opt/wasi-sdk` and
/// `/usr/share/wasi-sysroot` so a user-local install takes precedence.
#[test]
fn test_sysroot_candidates_for_wasi_includes_home_wasi_sdk() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    let target = TargetTripleComponents::parse("wasm32-unknown-wasip1").unwrap();
    let candidates = SysLibConfig::sysroot_candidates_for_home(&target, home);

    let expected_home_wasi = home_wasi_sdk_sysroot_for_home(home);
    let has_home_wasi = candidates.iter().any(|p| p == &expected_home_wasi);
    assert!(
        has_home_wasi,
        "WASI candidate list must include the user-local ~/.wasi-sdk install \
         location ({expected_home_wasi:?}), got: {candidates:?}"
    );

    // System-wide candidates should still be present.
    let has_opt = candidates
        .iter()
        .any(|p| p == &PathBuf::from("/opt/wasi-sdk/share/wasi-sysroot"));
    assert!(has_opt, "WASI candidates must include /opt/wasi-sdk");

    // Order matters: home WASI SDK must come BEFORE the system-wide
    // candidates so a user-local install takes precedence over the
    // system one.
    let home_idx = candidates.iter().position(|p| p == &expected_home_wasi);
    let opt_idx = candidates
        .iter()
        .position(|p| p == &PathBuf::from("/opt/wasi-sdk/share/wasi-sysroot"));
    assert!(
        home_idx < opt_idx,
        "user-local ~/.wasi-sdk must precede system /opt/wasi-sdk in candidate order"
    );
}
