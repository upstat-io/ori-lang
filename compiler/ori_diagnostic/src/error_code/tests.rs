use std::str::FromStr;

use super::*;

#[test]
fn test_error_code_display() {
    assert_eq!(ErrorCode::E1001.to_string(), "E1001");
    assert_eq!(ErrorCode::E2001.as_str(), "E2001");
}

#[test]
fn test_arc_error_codes() {
    assert_eq!(ErrorCode::E4001.as_str(), "E4001");
    assert_eq!(ErrorCode::E4002.as_str(), "E4002");
    assert_eq!(ErrorCode::E4003.as_str(), "E4003");

    assert!(ErrorCode::E4001.is_arc_error());
    assert!(ErrorCode::E4002.is_arc_error());
    assert!(ErrorCode::E4003.is_arc_error());

    assert!(!ErrorCode::E4001.is_codegen_error());
    assert!(!ErrorCode::E4001.is_parser_error());
    assert!(!ErrorCode::E4001.is_warning());
}

#[test]
fn test_codegen_error_codes() {
    assert_eq!(ErrorCode::E5001.as_str(), "E5001");
    assert_eq!(ErrorCode::E5005.as_str(), "E5005");
    assert_eq!(ErrorCode::E5009.as_str(), "E5009");

    assert!(ErrorCode::E5001.is_codegen_error());
    assert!(ErrorCode::E5006.is_codegen_error());
    assert!(ErrorCode::E5009.is_codegen_error());

    assert!(!ErrorCode::E5001.is_arc_error());
    assert!(!ErrorCode::E5001.is_parser_error());
    assert!(!ErrorCode::E5001.is_warning());
}

#[test]
fn test_eval_error_codes() {
    assert_eq!(ErrorCode::E6001.as_str(), "E6001");
    assert_eq!(ErrorCode::E6020.as_str(), "E6020");
    assert_eq!(ErrorCode::E6099.as_str(), "E6099");

    assert!(ErrorCode::E6001.is_eval_error());
    assert!(ErrorCode::E6031.is_eval_error());
    assert!(ErrorCode::E6099.is_eval_error());

    assert!(!ErrorCode::E6001.is_arc_error());
    assert!(!ErrorCode::E6001.is_codegen_error());
    assert!(!ErrorCode::E6001.is_parser_error());
    assert!(!ErrorCode::E6001.is_warning());
}

/// Every variant in `ErrorCode::ALL` must be classified by exactly one `is_*` predicate.
///
/// Catches drift when a new variant is added to the enum and `as_str()` but
/// omitted from its `is_*` predicate.
#[test]
fn test_all_variants_classified() {
    for &code in ErrorCode::ALL {
        let flags = [
            ("is_lexer_error", code.is_lexer_error()),
            ("is_parser_error", code.is_parser_error()),
            ("is_type_error", code.is_type_error()),
            ("is_pattern_error", code.is_pattern_error()),
            ("is_arc_error", code.is_arc_error()),
            ("is_codegen_error", code.is_codegen_error()),
            ("is_eval_error", code.is_eval_error()),
            ("is_internal_error", code.is_internal_error()),
            ("is_warning", code.is_warning()),
        ];
        let true_count = flags.iter().filter(|(_, f)| *f).count();
        let matching: Vec<_> = flags.iter().filter(|(_, f)| *f).map(|(n, _)| *n).collect();
        assert_eq!(
            true_count, 1,
            "{code}: expected exactly 1 predicate, got {true_count} ({matching:?})"
        );
    }
}

/// Verify `ErrorCode::ALL` has no duplicate entries and matches expected count.
///
/// With the `define_error_codes!` macro, `ALL` is guaranteed to contain every
/// variant. This test catches accidental duplicates and verifies the count
/// hasn't changed unintentionally.
#[test]
fn test_all_is_complete() {
    use std::collections::HashSet;
    let strings: HashSet<&str> = ErrorCode::ALL.iter().map(ErrorCode::as_str).collect();
    assert_eq!(
        strings.len(),
        ErrorCode::ALL.len(),
        "ALL contains duplicate entries"
    );
    assert_eq!(ErrorCode::ALL.len(), ErrorCode::COUNT);
    assert_eq!(
        ErrorCode::COUNT,
        143,
        "COUNT changed — did you add a new ErrorCode variant? Update this number."
    );
}

/// Every error code has a non-empty description.
#[test]
fn test_all_have_descriptions() {
    for &code in ErrorCode::ALL {
        assert!(
            !code.description().is_empty(),
            "{}: description is empty",
            code.as_str()
        );
    }
}

/// Spot-check that descriptions match expectations.
#[test]
fn test_description_examples() {
    assert_eq!(ErrorCode::E2001.description(), "Type mismatch");
    assert_eq!(
        ErrorCode::E0001.description(),
        "Unterminated string literal"
    );
    assert_eq!(
        ErrorCode::W2001.description(),
        "Infinite iterator consumed without bound"
    );
}

/// Every variant in `ErrorCode::ALL` round-trips through `from_str(as_str())`.
///
/// This guarantees `from_str()` can parse every code the compiler can emit,
/// since it derives from the same `ALL` array and `as_str()` match.
#[test]
fn test_from_str_round_trip() {
    for &code in ErrorCode::ALL {
        let s = code.as_str();
        let parsed = ErrorCode::from_str(s);
        assert_eq!(
            parsed,
            Ok(code),
            "from_str({s:?}) should return Ok({code:?})"
        );
    }
}

/// `from_str()` is case-insensitive.
#[test]
fn test_from_str_case_insensitive() {
    assert_eq!(ErrorCode::from_str("e2001"), Ok(ErrorCode::E2001));
    assert_eq!(ErrorCode::from_str("w2001"), Ok(ErrorCode::W2001));
}

/// `from_str()` returns `Err` for unrecognized strings.
#[test]
fn test_from_str_unknown() {
    assert!(ErrorCode::from_str("E9999").is_err());
    assert!(ErrorCode::from_str("hello").is_err());
    assert!(ErrorCode::from_str("").is_err());
}

#[test]
fn test_new_predicates() {
    assert!(ErrorCode::E0001.is_lexer_error());
    assert!(ErrorCode::E0911.is_lexer_error());
    assert!(!ErrorCode::E0001.is_parser_error());

    assert!(ErrorCode::E2001.is_type_error());
    assert!(ErrorCode::E2018.is_type_error());
    assert!(!ErrorCode::E2001.is_parser_error());

    assert!(ErrorCode::E3001.is_pattern_error());
    assert!(!ErrorCode::E3001.is_type_error());

    assert!(ErrorCode::E9001.is_internal_error());
    assert!(ErrorCode::E9002.is_internal_error());
    assert!(!ErrorCode::E9001.is_eval_error());
}

#[test]
fn lifecycle_classifies_reserved_and_tracked_codes() {
    assert!(ErrorCode::E2045.lifecycle().is_reserved());
    assert!(ErrorCode::E0911.lifecycle().is_tracked());
    assert!(
        ErrorCode::E4001.lifecycle().is_reserved() || ErrorCode::E4001.lifecycle().is_tracked()
    );
}

#[test]
fn lifecycle_marks_source_confirmed_graph_misses_as_emitted() {
    for code in [
        ErrorCode::E0932,
        ErrorCode::E1013,
        ErrorCode::E1016,
        ErrorCode::E1018,
        ErrorCode::E1019,
        ErrorCode::E1020,
        ErrorCode::E2004,
    ] {
        assert!(
            code.lifecycle().is_emitted(),
            "{code} has a live production emitter and must not be allowlisted as a graph miss"
        );
    }
}

#[test]
fn emitted_lifecycle_codes_have_production_construction_sites() {
    let production_source = production_error_code_source();
    let mut missing = Vec::new();

    for &code in ErrorCode::ALL {
        if code.lifecycle().is_emitted()
            && !production_source.contains(&format!("ErrorCode::{}", code.as_str()))
        {
            missing.push(code.as_str());
        }
    }

    assert!(
        missing.is_empty(),
        "codes marked emitted lack production ErrorCode construction sites: {missing:?}"
    );
}

#[allow(
    clippy::expect_used,
    reason = "test helper uses parent directory structure"
)]
fn production_error_code_source() -> String {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiler_dir = manifest_dir
        .parent()
        .expect("ori_diagnostic should live under compiler/");
    let mut source = String::new();
    collect_rust_source(compiler_dir, &mut source);
    source
}

#[allow(clippy::expect_used, reason = "test helper reads files from disk")]
fn collect_rust_source(dir: &std::path::Path, out: &mut String) {
    for entry in std::fs::read_dir(dir).expect("read compiler source directory") {
        let entry = entry.expect("read compiler source entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source(&path, out);
            continue;
        }

        if path.extension().and_then(std::ffi::OsStr::to_str) != Some("rs")
            || is_non_production_error_code_consumer(&path)
        {
            continue;
        }

        out.push_str(&std::fs::read_to_string(&path).expect("read Rust source file"));
        out.push('\n');
    }
}

fn is_non_production_error_code_consumer(path: &std::path::Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized.contains("/ori_diagnostic/src/error_code/")
        || normalized.contains("/ori_diagnostic/src/errors/")
        || normalized.contains("/oric/src/commands/fmt/diagnostics.rs")
        || normalized.contains("/tests.rs")
        || normalized.contains("/tests/")
}
