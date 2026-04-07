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
// Regression: Before this SSOT was extracted,
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

// =============================================================================
// target_sysroot_env_key tests
//
// Regression: The per-target
// `ORI_SYSROOT_<TARGET>` override key was previously built from
// `target.to_string()` (the version-preserving canonical form), so
// `arm64-apple-darwin25.2.0` produced
// `ORI_SYSROOT_AARCH64_APPLE_DARWIN25.2.0` — a name that:
//   1. Did not match the documented/exportable canonical key
//      (`ORI_SYSROOT_AARCH64_APPLE_DARWIN`).
//   2. Contained dots, making it not a valid POSIX/zsh shell variable
//      name (zsh rejects it as `not valid in this context`).
// The fix routes the env key through `support_key()` so versioned and
// aliased spellings collapse to the canonical shell-safe form.
// =============================================================================

#[test]
fn test_target_sysroot_env_key_canonical_linux() {
    let target = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(
        target_sysroot_env_key(&target),
        "ORI_SYSROOT_X86_64_UNKNOWN_LINUX_GNU"
    );
}

/// Semantic pin: versioned Darwin spelling collapses to the unversioned
/// canonical key. The output must NOT contain dots and must NOT contain
/// the version suffix `25.2.0`. Reverting `support_key` → `to_string`
/// breaks this test.
#[test]
fn test_target_sysroot_env_key_versioned_darwin_collapses() {
    let target = TargetTripleComponents::parse("arm64-apple-darwin25.2.0").unwrap();
    let key = target_sysroot_env_key(&target);
    assert_eq!(
        key, "ORI_SYSROOT_AARCH64_APPLE_DARWIN",
        "versioned Darwin must collapse to canonical key"
    );
    assert!(
        !key.contains('.'),
        "env var name must not contain dots (zsh rejects them as variable names)"
    );
    assert!(
        !key.contains("25"),
        "env var name must not contain the version suffix"
    );
}

/// Sibling: arch alias normalization composes with version stripping.
#[test]
fn test_target_sysroot_env_key_arm64_alias_collapses() {
    let target = TargetTripleComponents::parse("arm64-apple-darwin").unwrap();
    let key = target_sysroot_env_key(&target);
    assert_eq!(key, "ORI_SYSROOT_AARCH64_APPLE_DARWIN");
}

/// Sibling: `amd64` alias collapses to `x86_64`.
#[test]
fn test_target_sysroot_env_key_amd64_alias_collapses() {
    let target = TargetTripleComponents::parse("amd64-unknown-linux-gnu").unwrap();
    let key = target_sysroot_env_key(&target);
    assert_eq!(key, "ORI_SYSROOT_X86_64_UNKNOWN_LINUX_GNU");
}

/// Negative pin (shell-safety invariant): for every supported target +
/// known alias, the resulting env key must be a valid POSIX shell
/// variable name. POSIX 3.231 says shell variables consist of uppercase
/// letters, digits, and underscores, starting with a letter or
/// underscore. The matrix below enumerates every alias spelling that
/// could plausibly flow through the env-key construction; none may
/// produce a name with dots, hyphens, or other shell-hostile characters.
#[test]
fn test_target_sysroot_env_key_is_always_shell_safe_matrix() {
    let aliases: &[&str] = &[
        "x86_64-unknown-linux-gnu",
        "amd64-unknown-linux-gnu",
        "i686-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "arm64-apple-darwin",
        "arm64-apple-darwin25.2.0",
        "aarch64-apple-darwin25.2.0",
        "x86_64-apple-darwin23.6.0",
        "aarch64-apple-macos",
        "x86_64-pc-windows-msvc",
        "x86_64-pc-windows-gnu",
        "wasm32-unknown-unknown",
        "wasm32-unknown-wasip1",
    ];
    let mut visited = 0usize;
    for alias in aliases {
        let target =
            TargetTripleComponents::parse(alias).unwrap_or_else(|e| panic!("parse {alias:?}: {e}"));
        let key = target_sysroot_env_key(&target);

        // Every char must be ASCII uppercase letter, digit, or underscore.
        assert!(
            key.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
            "env key {key:?} for input {alias:?} contains shell-unsafe characters"
        );
        // Must start with `ORI_SYSROOT_`.
        assert!(
            key.starts_with("ORI_SYSROOT_"),
            "env key {key:?} must start with ORI_SYSROOT_"
        );
        // Must not contain a dot or hyphen.
        assert!(!key.contains('.'), "env key {key:?} contains '.'");
        assert!(!key.contains('-'), "env key {key:?} contains '-'");
        visited += 1;
    }
    assert_eq!(visited, aliases.len(), "matrix loop skipped cells");
}

/// Round-trip pin for the env-override path: the discovery side computes
/// the canonical env key (`ORI_SYSROOT_AARCH64_APPLE_DARWIN`) for any
/// aliased / versioned input that resolves to the same `support_key`,
/// and uses it to query the env. This is the exact contract Codex flagged
/// as an Apple Silicon user who exports
/// `ORI_SYSROOT_AARCH64_APPLE_DARWIN=...` (the documented form) MUST
/// have it picked up when building with `arm64-apple-darwin25.2.0` (the
/// LLVM-default spelling).
///
/// Uses the hermetic `detect_sysroot_with_env` so the env getter is
/// injected — no process-global `std::env::set_var`, no race with
/// parallel tests, no `unsafe`. The closure stubs the canonical key
/// and asserts the discovery side asks for the right key.
#[test]
fn test_env_override_round_trips_versioned_darwin_to_canonical_key() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let sysroot = tmp.path().to_path_buf();
    let sysroot_str = sysroot.to_string_lossy().into_owned();

    // The discovery side will look up exactly this key — the canonical
    // form built from `support_key()`, with no Darwin OS version suffix.
    let expected_key = "ORI_SYSROOT_AARCH64_APPLE_DARWIN";

    // Also assert what `target_sysroot_env_key` would emit, to pin the
    // SSOT explicitly.
    let canonical_target = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
    assert_eq!(
        target_sysroot_env_key(&canonical_target),
        expected_key,
        "target_sysroot_env_key must produce the canonical key"
    );

    // Closure-based env stub: returns Some(sysroot) only for the
    // canonical key, None otherwise. Captures whether the closure was
    // called with the expected key (proving the discovery side built
    // the right name from the versioned input).
    let observed_keys = std::cell::RefCell::new(Vec::<String>::new());
    let env_stub = |key: &str| -> Option<String> {
        observed_keys.borrow_mut().push(key.to_string());
        if key == expected_key {
            Some(sysroot_str.clone())
        } else {
            None
        }
    };

    // Now query with a versioned darwin spelling. The discovery side
    // must compute the canonical key from the versioned input and find
    // the override.
    let target = TargetTripleComponents::parse("arm64-apple-darwin25.2.0").unwrap();
    let detected = SysLibConfig::detect_sysroot_with_env(&target, tmp.path(), env_stub);

    assert_eq!(
        detected.as_deref(),
        Some(sysroot.as_path()),
        "ORI_SYSROOT_AARCH64_APPLE_DARWIN export must be discoverable for \
         arm64-apple-darwin25.2.0 input — both compute the same canonical key"
    );

    // The first env lookup must be the canonical key — if discovery
    // ever falls back to `to_string()` again, it would query
    // `ORI_SYSROOT_AARCH64_APPLE_DARWIN25.2.0` instead and this assert
    // would fail. Negative pin against the bug class.
    let observed = observed_keys.borrow();
    assert_eq!(
        observed.first().map(String::as_str),
        Some(expected_key),
        "first env lookup must be the canonical key, observed keys: {observed:?}"
    );
    // No observed key may contain a dot (would not be a valid POSIX
    // shell variable name).
    for key in observed.iter() {
        assert!(
            !key.contains('.'),
            "discovery queried shell-unsafe env key {key:?}"
        );
    }
}

/// Hermetic env-getter that always returns `None`. Used by the install-
/// path round-trip tests to guarantee no process-global env contamination
/// from parallel tests can affect discovery.
fn no_env(_key: &str) -> Option<String> {
    None
}

/// Round-trip pin: a sysroot installed under `~/.ori/sysroots/<canonical-key>`
/// is discoverable via the hermetic `detect_sysroot_with_env` for any
/// aliased or versioned input that canonicalizes to that key. This is the
/// contract that makes the install side and the discovery side agree.
///
/// Uses both `_for_home` (hermetic home dir) and `_with_env` with an
/// always-`None` env getter so no process-global state can race the test.
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

    let detected = SysLibConfig::detect_sysroot_with_env(&target, home, no_env);
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
    let detected = SysLibConfig::detect_sysroot_with_env(&target, home, no_env);
    assert_eq!(detected.as_deref(), Some(install.as_path()));
}

/// Negative pin: nothing installed → discovery returns None.
/// Hermetic against env vars via `no_env`.
#[test]
fn test_detect_sysroot_for_home_returns_none_when_nothing_installed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    // Use a target whose hard-coded system candidates won't exist on
    // CI (Windows MSVC sysroot at /opt/windows-sdk).
    let target = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    let detected = SysLibConfig::detect_sysroot_with_env(&target, home, no_env);
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
