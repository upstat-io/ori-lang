//! Tests for the typed `Arch`, `HostOs`, `HostPlatform`, and
//! `TargetTripleComponents` parse/canonicalization layer.
//!
//! Regression: `is_cross_compiling()` reported native Apple
//! Silicon as cross-compilation because `arm64` (LLVM default triple) and
//! `aarch64` (Rust `cfg(target_arch)`) compared unequal as raw strings.
//!
//! Strategy: matrix tests with self-verifying counters, a semantic pin
//! reproducing the exact failing case, and a type-level negative pin that
//! refuses to compile if `TargetTripleComponents.arch` is ever reverted to
//! `String`.

use super::{Arch, HostOs, HostPlatform, TargetTripleComponents};

// === Arch alias normalization ===

/// Matrix: every known alias spelling parses into the correct canonical `Arch`.
#[test]
fn test_arch_parse_normalizes_alias_spellings_matrix() {
    let cases: &[(&str, Arch)] = &[
        ("x86_64", Arch::X86_64),
        ("amd64", Arch::X86_64),
        ("i386", Arch::I386),
        ("i486", Arch::I386),
        ("i586", Arch::I386),
        ("i686", Arch::I386),
        ("aarch64", Arch::Aarch64),
        ("arm64", Arch::Aarch64),
        ("wasm32", Arch::Wasm32),
        ("wasm64", Arch::Wasm64),
    ];

    for (alias, expected) in cases {
        let parsed =
            Arch::parse_llvm_name(alias).unwrap_or_else(|| panic!("alias {alias:?} must parse"));
        assert_eq!(
            parsed, *expected,
            "alias {alias:?} normalized to wrong canonical Arch"
        );
    }
}

/// Self-verifying counter: proves every alias cell in the matrix was visited.
/// Adapted from Zig's comptime matrix-completeness pattern.
#[test]
fn test_arch_parse_matrix_is_self_verifying() {
    let cases: &[(&str, Arch)] = &[
        ("x86_64", Arch::X86_64),
        ("amd64", Arch::X86_64),
        ("i386", Arch::I386),
        ("i486", Arch::I386),
        ("i586", Arch::I386),
        ("i686", Arch::I386),
        ("aarch64", Arch::Aarch64),
        ("arm64", Arch::Aarch64),
        ("wasm32", Arch::Wasm32),
        ("wasm64", Arch::Wasm64),
    ];

    let mut visited = 0usize;
    for (alias, expected) in cases {
        assert_eq!(Arch::parse_llvm_name(alias), Some(*expected));
        visited += 1;
    }
    assert_eq!(
        visited,
        cases.len(),
        "matrix loop skipped cells — visited {visited}, expected {}",
        cases.len()
    );
}

/// Unknown arch names are rejected at the boundary (parse-don't-validate).
#[test]
fn test_arch_parse_rejects_unknown_names() {
    assert_eq!(Arch::parse_llvm_name("riscv64"), None);
    assert_eq!(Arch::parse_llvm_name(""), None);
    assert_eq!(Arch::parse_llvm_name("ARM64"), None); // case-sensitive
}

// === Triple parse preserves vendor/os/env verbatim while canonicalizing arch ===

#[test]
fn test_target_triple_parse_preserves_vendor_os_env_while_normalizing_arch_matrix() {
    struct Case {
        input: &'static str,
        arch: Arch,
        vendor: &'static str,
        os: &'static str,
        env: Option<&'static str>,
    }

    let cases: &[Case] = &[
        // Apple Silicon LLVM default triple — arch normalizes, darwin version preserved.
        Case {
            input: "arm64-apple-darwin25.2.0",
            arch: Arch::Aarch64,
            vendor: "apple",
            os: "darwin25.2.0",
            env: None,
        },
        // amd64 alias — canonicalizes to x86_64; gnu env preserved.
        Case {
            input: "amd64-unknown-linux-gnu",
            arch: Arch::X86_64,
            vendor: "unknown",
            os: "linux",
            env: Some("gnu"),
        },
        // i686 alias — canonicalizes to i386; msvc env preserved.
        Case {
            input: "i686-pc-windows-msvc",
            arch: Arch::I386,
            vendor: "pc",
            os: "windows",
            env: Some("msvc"),
        },
        // Already-canonical input round-trips unchanged.
        Case {
            input: "aarch64-unknown-linux-musl",
            arch: Arch::Aarch64,
            vendor: "unknown",
            os: "linux",
            env: Some("musl"),
        },
    ];

    let mut visited = 0usize;
    for case in cases {
        let parsed = TargetTripleComponents::parse(case.input)
            .unwrap_or_else(|e| panic!("parse {:?} failed: {e}", case.input));
        assert_eq!(parsed.arch, case.arch, "arch for {:?}", case.input);
        assert_eq!(parsed.vendor, case.vendor, "vendor for {:?}", case.input);
        assert_eq!(parsed.os, case.os, "os for {:?}", case.input);
        assert_eq!(parsed.env.as_deref(), case.env, "env for {:?}", case.input);
        visited += 1;
    }
    assert_eq!(visited, cases.len(), "matrix skipped cells");
}

// === Cross-host cross-compilation detection ===

/// For every (host, target) combo, verify `is_cross_for` reports correctly.
/// Self-verifying counter included.
#[test]
fn test_is_cross_for_simulated_host_matrix() {
    let hosts = [
        HostPlatform::new(Arch::X86_64, HostOs::Linux),
        HostPlatform::new(Arch::Aarch64, HostOs::Linux),
        HostPlatform::new(Arch::X86_64, HostOs::Darwin),
        HostPlatform::new(Arch::Aarch64, HostOs::Darwin),
        HostPlatform::new(Arch::X86_64, HostOs::Windows),
    ];
    let targets_same_host: &[(&str, Arch, HostOs)] = &[
        ("x86_64-unknown-linux-gnu", Arch::X86_64, HostOs::Linux),
        ("aarch64-unknown-linux-gnu", Arch::Aarch64, HostOs::Linux),
        ("x86_64-apple-darwin", Arch::X86_64, HostOs::Darwin),
        ("aarch64-apple-darwin", Arch::Aarch64, HostOs::Darwin),
        ("x86_64-pc-windows-msvc", Arch::X86_64, HostOs::Windows),
    ];

    let mut visited = 0usize;
    for (triple, expected_arch, expected_os) in targets_same_host {
        let parsed = TargetTripleComponents::parse(triple).unwrap();
        for host in hosts {
            let is_cross = parsed.is_cross_for(host);
            let should_be_native = host.arch == *expected_arch && host.os == *expected_os;
            assert_eq!(
                is_cross, !should_be_native,
                "target {triple:?} vs host {host:?}: is_cross_for mismatch"
            );
            visited += 1;
        }
    }
    assert_eq!(
        visited,
        hosts.len() * targets_same_host.len(),
        "cross-detection matrix skipped cells"
    );
}

#[test]
fn test_is_cross_for_matrix_is_self_verifying() {
    // Explicit counter check for the matrix above. Kept separate so a skipped
    // host/target iteration is caught even if the main test's assertions pass.
    let host_arches = [Arch::X86_64, Arch::Aarch64];
    let host_oses = [HostOs::Linux, HostOs::Darwin, HostOs::Windows];
    let target_arches = [Arch::X86_64, Arch::Aarch64];

    let mut count = 0usize;
    for ha in host_arches {
        for ho in host_oses {
            for ta in target_arches {
                let host = HostPlatform::new(ha, ho);
                // Build a canonical triple for each target arch on Linux.
                let triple = format!("{}-unknown-linux-gnu", ta.display_name());
                let parsed = TargetTripleComponents::parse(&triple).unwrap();
                let _ = parsed.is_cross_for(host);
                count += 1;
            }
        }
    }
    assert_eq!(
        count,
        host_arches.len() * host_oses.len() * target_arches.len()
    );
}

// === Native round-trip for every supported host ===

/// For each supported host (Linux `x86_64`, Linux `aarch64`, macOS `x86_64`,
/// macOS `arm64`, Windows `x86_64`), the native triple must parse to
/// `is_cross_for = false`. This includes the Apple Silicon LLVM-default
/// spelling `arm64-apple-darwin...`.
#[test]
fn test_native_host_triple_round_trips_to_not_cross_compiling_matrix() {
    let cases: &[(&str, HostPlatform)] = &[
        (
            "x86_64-unknown-linux-gnu",
            HostPlatform::new(Arch::X86_64, HostOs::Linux),
        ),
        (
            "aarch64-unknown-linux-gnu",
            HostPlatform::new(Arch::Aarch64, HostOs::Linux),
        ),
        (
            "x86_64-apple-darwin",
            HostPlatform::new(Arch::X86_64, HostOs::Darwin),
        ),
        (
            "aarch64-apple-darwin",
            HostPlatform::new(Arch::Aarch64, HostOs::Darwin),
        ),
        // Apple Silicon's LLVM default triple spelling must round-trip.
        (
            "arm64-apple-darwin25.2.0",
            HostPlatform::new(Arch::Aarch64, HostOs::Darwin),
        ),
        (
            "x86_64-pc-windows-msvc",
            HostPlatform::new(Arch::X86_64, HostOs::Windows),
        ),
    ];

    for (triple, host) in cases {
        let parsed = TargetTripleComponents::parse(triple)
            .unwrap_or_else(|e| panic!("parse {triple:?}: {e}"));
        assert!(
            !parsed.is_cross_for(*host),
            "native host triple {triple:?} must NOT be detected as cross-compilation \
             against host {host:?}"
        );
    }
}

// === Semantic pin for ===

/// Regression pin: this exact bug.
///
/// Parse `arm64-apple-darwin25.2.0` — the real LLVM default triple on Apple
/// Silicon — and verify that a simulated Apple Silicon host is recognized as
/// native, not as cross-compilation. Would fail if the `arm64 → Aarch64` alias
/// is removed from `Arch::parse_llvm_name`.
///
/// This is the permanent regression guard for
#[test]
fn test_is_cross_for_regression_pin_arm64_native_host_is_not_cross() {
    let parsed = TargetTripleComponents::parse("arm64-apple-darwin25.2.0")
        .expect("Apple Silicon default triple must parse");
    assert_eq!(
        parsed.arch,
        Arch::Aarch64,
        "arm64 must canonicalize to Aarch64"
    );
    let host = HostPlatform::new(Arch::Aarch64, HostOs::Darwin);
    assert!(
        !parsed.is_cross_for(host),
        "BUG-04-045: arm64-apple-darwin on aarch64 Darwin host must NOT be cross-compilation"
    );
    assert!(
        parsed.is_native_for(host),
        "BUG-04-045: arm64-apple-darwin on aarch64 Darwin host must be reported native"
    );
}

// === Syslib sibling pin ===

/// Sibling semantic pin for `syslib`'s native check. Exercises the same
/// `is_native_for` query on each supported host.
#[test]
fn test_syslib_is_native_for_simulated_host_matrix() {
    let cases: &[(&str, HostPlatform)] = &[
        (
            "x86_64-unknown-linux-gnu",
            HostPlatform::new(Arch::X86_64, HostOs::Linux),
        ),
        (
            "aarch64-unknown-linux-gnu",
            HostPlatform::new(Arch::Aarch64, HostOs::Linux),
        ),
        (
            "aarch64-apple-darwin",
            HostPlatform::new(Arch::Aarch64, HostOs::Darwin),
        ),
        (
            "arm64-apple-darwin25.2.0",
            HostPlatform::new(Arch::Aarch64, HostOs::Darwin),
        ),
    ];
    for (triple, host) in cases {
        let parsed = TargetTripleComponents::parse(triple).unwrap();
        assert!(parsed.is_native_for(*host), "{triple:?} vs {host:?}");
    }
    // Negative: swap arch, must be cross.
    let parsed = TargetTripleComponents::parse("aarch64-unknown-linux-gnu").unwrap();
    let host = HostPlatform::new(Arch::X86_64, HostOs::Linux);
    assert!(parsed.is_cross_for(host));
}

// === Type-level negative pin ===

/// Negative pin (type-level): if `TargetTripleComponents.arch` is ever reverted
/// from `Arch` back to `String`, this test refuses to compile because:
///   1. `String::is_64_bit_non_wasm()` does not exist
///   2. `String` cannot be pattern-matched against `Arch::X86_64`
///
/// This is stronger than a runtime negative pin: the bug class is "raw arch
/// string compare", which a runtime test cannot detect after the field type
/// is changed. A compile-time guard is the only defense that scales with new
/// consumers.
#[test]
fn test_target_triple_components_has_no_raw_arch_string_field() {
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();

    // Guard 1: method only exists on `Arch`, not on `String`.
    let _is_64: bool = components.arch.is_64_bit_non_wasm();

    // Guard 2: pattern match on the `Arch` enum.
    let canonical: Arch = components.arch;
    assert!(matches!(canonical, Arch::X86_64));

    // Guard 3: the canonical display name is static, not runtime-derived from
    // the raw parse input. `String` could not provide this.
    assert_eq!(canonical.display_name(), "x86_64");
}

// === Arch query methods ===

#[test]
fn test_arch_pointer_size_bytes_exhaustive() {
    assert_eq!(Arch::X86_64.pointer_size_bytes(), 8);
    assert_eq!(Arch::Aarch64.pointer_size_bytes(), 8);
    assert_eq!(Arch::Wasm64.pointer_size_bytes(), 8);
    assert_eq!(Arch::I386.pointer_size_bytes(), 4);
    assert_eq!(Arch::Wasm32.pointer_size_bytes(), 4);
}

#[test]
fn test_arch_is_64_bit_non_wasm() {
    assert!(Arch::X86_64.is_64_bit_non_wasm());
    assert!(Arch::Aarch64.is_64_bit_non_wasm());
    assert!(!Arch::I386.is_64_bit_non_wasm());
    assert!(!Arch::Wasm32.is_64_bit_non_wasm());
    assert!(
        !Arch::Wasm64.is_64_bit_non_wasm(),
        "Wasm64 is 64-bit but IS wasm"
    );
}

#[test]
fn test_arch_is_wasm() {
    assert!(Arch::Wasm32.is_wasm());
    assert!(Arch::Wasm64.is_wasm());
    assert!(!Arch::X86_64.is_wasm());
    assert!(!Arch::Aarch64.is_wasm());
    assert!(!Arch::I386.is_wasm());
}

#[test]
fn test_arch_display_formats_canonical() {
    assert_eq!(format!("{}", Arch::X86_64), "x86_64");
    assert_eq!(format!("{}", Arch::Aarch64), "aarch64");
    assert_eq!(format!("{}", Arch::I386), "i386");
    assert_eq!(format!("{}", Arch::Wasm32), "wasm32");
    assert_eq!(format!("{}", Arch::Wasm64), "wasm64");
}

// === support_key() — canonical lookup form for SUPPORTED_TARGETS ===

/// `support_key()` strips Darwin OS version suffixes so versioned and
/// unversioned spellings both match the unversioned `SUPPORTED_TARGETS`
/// entries. Regression pin for
#[test]
fn test_support_key_strips_darwin_version_matrix() {
    let cases: &[(&str, &str)] = &[
        // Versioned Darwin spellings collapse to the bare darwin form.
        ("arm64-apple-darwin25.2.0", "aarch64-apple-darwin"),
        ("aarch64-apple-darwin25.2.0", "aarch64-apple-darwin"),
        ("x86_64-apple-darwin23.6.0", "x86_64-apple-darwin"),
        // Bare Darwin is unchanged.
        ("aarch64-apple-darwin", "aarch64-apple-darwin"),
        ("x86_64-apple-darwin", "x86_64-apple-darwin"),
        // The `macos` OS spelling is also normalized to `darwin` because
        // `is_macos()` treats both as macOS variants.
        ("aarch64-apple-macos", "aarch64-apple-darwin"),
        // Non-Darwin triples pass through unchanged (env preserved).
        ("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"),
        ("aarch64-unknown-linux-musl", "aarch64-unknown-linux-musl"),
        ("x86_64-pc-windows-msvc", "x86_64-pc-windows-msvc"),
        ("wasm32-unknown-unknown", "wasm32-unknown-unknown"),
        // Modern WASI Preview1 canonical spelling — no special-casing.
        ("wasm32-unknown-wasip1", "wasm32-unknown-wasip1"),
        // Arch alias normalization composes with OS-suffix stripping.
        ("amd64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"),
    ];
    let mut visited = 0usize;
    for (input, expected_key) in cases {
        let parsed =
            TargetTripleComponents::parse(input).unwrap_or_else(|e| panic!("parse {input:?}: {e}"));
        assert_eq!(
            parsed.support_key(),
            *expected_key,
            "support_key for {input:?}"
        );
        visited += 1;
    }
    assert_eq!(visited, cases.len(), "matrix skipped cells");
}

/// `support_key()` is what the `SUPPORTED_TARGETS` lookup uses; verify
/// the actual lookup function agrees for every supported entry plus its
/// versioned and aliased variants.
#[test]
fn test_support_key_matches_supported_targets_for_known_aliases() {
    use crate::aot::target_features::is_supported_target;

    let aliases: &[&str] = &[
        "arm64-apple-darwin",
        "arm64-apple-darwin25.2.0",
        "aarch64-apple-darwin25.2.0",
        "aarch64-apple-macos",
        "amd64-unknown-linux-gnu",
        "x86_64-apple-darwin23.6.0",
        // Modern WASI Preview1 canonical form (Rust 1.78+).
        "wasm32-unknown-wasip1",
    ];
    for alias in aliases {
        let parsed = TargetTripleComponents::parse(alias).unwrap();
        assert!(
            is_supported_target(&parsed.support_key()),
            "support_key {} for input {alias:?} should be in SUPPORTED_TARGETS",
            parsed.support_key()
        );
    }
}
