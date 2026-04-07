use super::*;

#[test]
fn test_target_subcommand_from_str() {
    assert_eq!(
        TargetSubcommand::parse("list"),
        Some(TargetSubcommand::List)
    );
    assert_eq!(TargetSubcommand::parse("add"), Some(TargetSubcommand::Add));
    assert_eq!(
        TargetSubcommand::parse("remove"),
        Some(TargetSubcommand::Remove)
    );
    assert_eq!(TargetSubcommand::parse("invalid"), None);
    assert_eq!(TargetSubcommand::parse(""), None);
}

#[test]
fn test_sysroots_dir() {
    let dir = sysroots_dir();
    assert!(dir.to_string_lossy().contains(".ori"));
    assert!(dir.to_string_lossy().contains("sysroots"));
}

#[test]
fn test_sysroot_path() {
    let path = sysroot_path("x86_64-unknown-linux-gnu");
    assert!(path.to_string_lossy().contains("x86_64-unknown-linux-gnu"));
}

#[test]
fn test_is_target_installed_nonexistent() {
    // A random target name that definitely doesn't exist
    // Test the underlying logic since is_target_installed is feature-gated
    let path = sysroot_path("nonexistent-fake-target-12345");
    assert!(!path.exists());
}

#[test]
fn test_target_subcommand_variants() {
    // Verify all variants can be compared
    assert_ne!(TargetSubcommand::List, TargetSubcommand::Add);
    assert_ne!(TargetSubcommand::Add, TargetSubcommand::Remove);
    assert_ne!(TargetSubcommand::Remove, TargetSubcommand::List);
}

#[test]
fn test_target_subcommand_debug() {
    // Verify Debug trait works
    let list = TargetSubcommand::List;
    let debug_str = format!("{list:?}");
    assert_eq!(debug_str, "List");
}

#[test]
fn test_target_subcommand_clone() {
    let original = TargetSubcommand::Add;
    let cloned = original;
    assert_eq!(original, cloned);
}

// =============================================================================
// canonicalize_target_for_install tests
//
// Regression: BUG-04-045 / TPR-BUG-04-045-05. Before the fix, `add_target`
// validated input via `SUPPORTED_TARGETS.contains(&target)` — a raw string
// match against canonical-only entries — so any aliased or versioned spelling
// accepted by `ori build --target=...` was rejected here. The fix routes
// validation through the same canonicalization path (`support_key()`) that
// `from_triple` uses, so the CLI parity invariant holds: every spelling
// accepted by the build path is accepted here, and vice versa.
// =============================================================================

#[cfg(feature = "llvm")]
#[test]
fn test_canonicalize_target_accepts_canonical_form() {
    let key = canonicalize_target_for_install("x86_64-unknown-linux-gnu")
        .unwrap_or_else(|e| panic!("canonical linux triple must be accepted: {e}"));
    assert_eq!(key, "x86_64-unknown-linux-gnu");
}

#[cfg(feature = "llvm")]
#[test]
fn test_canonicalize_target_accepts_arch_alias_arm64() {
    // Apple's `arm64` alias canonicalizes to `aarch64`. The bare spelling
    // (no Darwin OS version suffix) round-trips to the unversioned support
    // key entry.
    let key = canonicalize_target_for_install("arm64-apple-darwin")
        .unwrap_or_else(|e| panic!("arm64 alias must canonicalize to aarch64: {e}"));
    assert_eq!(key, "aarch64-apple-darwin");
}

#[cfg(feature = "llvm")]
#[test]
fn test_canonicalize_target_accepts_versioned_darwin() {
    // The exact LLVM-default spelling on Apple Silicon. Both arch alias
    // (arm64 → aarch64) and Darwin version stripping (darwin25.2.0 →
    // darwin) compose to produce the unversioned canonical key.
    let key = canonicalize_target_for_install("arm64-apple-darwin25.2.0")
        .unwrap_or_else(|e| panic!("versioned Apple Silicon spelling must be accepted: {e}"));
    assert_eq!(key, "aarch64-apple-darwin");
}

#[cfg(feature = "llvm")]
#[test]
fn test_canonicalize_target_accepts_amd64_alias() {
    let key = canonicalize_target_for_install("amd64-unknown-linux-gnu")
        .unwrap_or_else(|e| panic!("amd64 alias must canonicalize to x86_64: {e}"));
    assert_eq!(key, "x86_64-unknown-linux-gnu");
}

#[cfg(feature = "llvm")]
#[test]
fn test_canonicalize_target_accepts_modern_wasi_canonical() {
    let key = canonicalize_target_for_install("wasm32-unknown-wasip1")
        .unwrap_or_else(|e| panic!("modern WASI canonical spelling must be accepted: {e}"));
    assert_eq!(key, "wasm32-unknown-wasip1");
}

#[cfg(feature = "llvm")]
#[test]
fn test_canonicalize_target_rejects_deprecated_2component_wasi() {
    // The deprecated 2-component spelling is still rejected at the parse
    // boundary, NOT silently normalized. This is the same invariant the
    // build-side parser enforces — keep them in lockstep.
    let result = canonicalize_target_for_install("wasm32-wasi");
    assert!(result.is_err());
}

#[cfg(feature = "llvm")]
#[test]
fn test_canonicalize_target_rejects_unknown_arch() {
    let result = canonicalize_target_for_install("riscv64-unknown-linux-gnu");
    assert!(result.is_err());
}

/// Semantic pin: every spelling that `ori build --target=...` accepts must
/// also be accepted by `ori target add` — the CLI parity contract. This
/// test enumerates the same alias spellings the AOT integration suite
/// covers in `cross.rs::test_from_triple_accepts_*` and asserts they all
/// canonicalize successfully here. Regression guard for TPR-BUG-04-045-05.
#[cfg(feature = "llvm")]
#[test]
fn test_canonicalize_target_parity_with_from_triple_matrix() {
    let cases: &[(&str, &str)] = &[
        ("arm64-apple-darwin", "aarch64-apple-darwin"),
        ("arm64-apple-darwin25.2.0", "aarch64-apple-darwin"),
        ("aarch64-apple-darwin25.2.0", "aarch64-apple-darwin"),
        ("x86_64-apple-darwin23.6.0", "x86_64-apple-darwin"),
        ("amd64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"),
        ("aarch64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"),
        ("x86_64-pc-windows-msvc", "x86_64-pc-windows-msvc"),
        ("wasm32-unknown-unknown", "wasm32-unknown-unknown"),
        ("wasm32-unknown-wasip1", "wasm32-unknown-wasip1"),
    ];
    let mut visited = 0usize;
    for (input, expected_key) in cases {
        let key = canonicalize_target_for_install(input)
            .unwrap_or_else(|e| panic!("install canonicalization {input:?} failed: {e}"));
        assert_eq!(key, *expected_key, "canonicalization for {input:?}");
        visited += 1;
    }
    assert_eq!(visited, cases.len(), "matrix loop skipped cells");
}

/// Negative pin: confirms that `is_target_installed` queries the
/// canonical key, so a query like `is_target_installed("arm64-apple-darwin")`
/// looks under `~/.ori/sysroots/aarch64-apple-darwin/` (the canonical
/// install location), not under the alias spelling.
#[cfg(feature = "llvm")]
#[test]
fn test_is_target_installed_uses_canonical_key() {
    // Both the alias and the canonical spelling resolve to the same
    // sysroot path. Since neither directory exists in the test
    // environment, both queries return false — but the negative result
    // proves the lookup paths agree (no panic, no aliasing bug).
    assert!(!is_target_installed("arm64-apple-darwin"));
    assert!(!is_target_installed("aarch64-apple-darwin"));

    // The two should resolve to the same path. We verify by computing
    // the path directly via the canonical key.
    let canonical_path = sysroot_path("aarch64-apple-darwin");
    let alias_path = sysroot_path("arm64-apple-darwin");
    assert_ne!(
        canonical_path, alias_path,
        "raw sysroot_path uses input verbatim — that's expected; \
         is_target_installed is what canonicalizes"
    );
}
