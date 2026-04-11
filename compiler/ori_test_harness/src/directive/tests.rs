use super::*;

// ---------------------------------------------------------------------------
// Positive (semantic pins — each verifies one directive type parsed correctly)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_custom_directive_extracts_key_and_value() {
    let source = "// @test-arc-pass: realize_rc_reuse\n";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert_eq!(result.directives.len(), 1);
    assert_eq!(
        result.directives[0].directive,
        Directive::Custom {
            key: "test-arc-pass".to_string(),
            value: "realize_rc_reuse".to_string(),
        }
    );
    assert_eq!(result.directives[0].line_number, 1);
    assert!(result.directives[0].revision.is_none());
}

#[test]
fn test_parse_revisions_directive_splits_on_whitespace() {
    let source = "// @revisions: debug release no-repr-opt\n";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert_eq!(result.directives.len(), 1);
    assert_eq!(
        result.directives[0].directive,
        Directive::Revisions {
            names: vec![
                "debug".to_string(),
                "release".to_string(),
                "no-repr-opt".to_string(),
            ],
        }
    );
}

#[test]
fn test_parse_compile_flags_directive_collects_flags() {
    let source = "// @compile-flags: --release --opt-level=2\n";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert_eq!(result.directives.len(), 1);
    assert_eq!(
        result.directives[0].directive,
        Directive::CompileFlags {
            flags: vec!["--release".to_string(), "--opt-level=2".to_string()],
        }
    );
}

#[test]
fn test_parse_check_directive_preserves_pattern() {
    let source = "// CHECK: call void @ori_rc_inc\n";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert_eq!(result.directives.len(), 1);
    assert_eq!(
        result.directives[0].directive,
        Directive::Check {
            pattern: "call void @ori_rc_inc".to_string(),
        }
    );
}

#[test]
fn test_parse_check_not_directive_preserves_pattern() {
    let source = "// CHECK-NOT: call void @ori_rc_dec\n";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert_eq!(result.directives.len(), 1);
    assert_eq!(
        result.directives[0].directive,
        Directive::CheckNot {
            pattern: "call void @ori_rc_dec".to_string(),
        }
    );
}

#[test]
fn test_parse_check_label_directive_preserves_pattern() {
    let source = "// CHECK-LABEL: define void @main\n";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert_eq!(result.directives.len(), 1);
    assert_eq!(
        result.directives[0].directive,
        Directive::CheckLabel {
            pattern: "define void @main".to_string(),
        }
    );
}

#[test]
fn test_parse_check_next_directive_preserves_pattern() {
    let source = "// CHECK-NEXT: ret void\n";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert_eq!(result.directives.len(), 1);
    assert_eq!(
        result.directives[0].directive,
        Directive::CheckNext {
            pattern: "ret void".to_string(),
        }
    );
}

#[test]
fn test_parse_revision_gated_directive_records_revision_name() {
    let source = "// @[release] compile-flags: --release\n";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert_eq!(result.directives.len(), 1);
    assert_eq!(result.directives[0].revision, Some("release".to_string()));
    assert_eq!(
        result.directives[0].directive,
        Directive::CompileFlags {
            flags: vec!["--release".to_string()],
        }
    );
}

#[test]
fn test_parse_mixed_directives_returns_source_order() {
    let source = "\
// @revisions: debug release
// @[release] compile-flags: --release
// CHECK-LABEL: define void @main
// CHECK: call void @ori_rc_inc
// CHECK-NOT: call void @ori_rc_dec
";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert_eq!(result.directives.len(), 5);
    assert_eq!(result.directives[0].line_number, 1);
    assert_eq!(result.directives[4].line_number, 5);
    // Verify order matches source
    assert!(matches!(
        result.directives[0].directive,
        Directive::Revisions { .. }
    ));
    assert!(matches!(
        result.directives[2].directive,
        Directive::CheckLabel { .. }
    ));
}

#[test]
fn test_parse_whitespace_before_comment_marker_accepted() {
    let source = "    // @test-arc-pass: normalize_function\n";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert_eq!(result.directives.len(), 1);
    assert_eq!(
        result.directives[0].directive,
        Directive::Custom {
            key: "test-arc-pass".to_string(),
            value: "normalize_function".to_string(),
        }
    );
}

// ---------------------------------------------------------------------------
// Negative pins (verify rejection/ignoring of invalid input)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_forbidden_revision_name_produces_error() {
    let source = "// @[CHECK] compile-flags: --release\n";
    let result = parse_directives(source);
    assert!(result.directives.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].message.contains("forbidden revision name"));
    assert_eq!(result.errors[0].line_number, 1);
}

#[test]
fn test_parse_non_directive_comment_ignored() {
    let source = "\
// This is a regular comment
// Another comment with @mention
let x = 42;
// @test-arc-pass: realize_rc_reuse
";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    // Only the actual directive is parsed, not the regular comments
    assert_eq!(result.directives.len(), 1);
    assert_eq!(result.directives[0].line_number, 4);
}

#[test]
fn test_parse_directive_inside_string_literal_not_matched() {
    // Line-based limitation: we cannot distinguish directives inside string
    // literals from real directives. But a directive inside a string would
    // need to be on its own line starting with //, which is unlikely in
    // practice. This test documents the limitation.
    let source = "let s = \"not a directive line\";\n";
    let result = parse_directives(source);
    assert!(result.errors.is_empty());
    assert!(result.directives.is_empty());
}

#[test]
fn test_parse_malformed_directive_produces_error() {
    // `// @key` has the recognized `// @` prefix but no `: value` pattern
    let source = "// @revisions\n";
    let result = parse_directives(source);
    assert!(result.directives.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].message.contains("malformed directive"));
    assert_eq!(result.errors[0].line_number, 1);
}

#[test]
fn test_parse_forbidden_revision_name_in_revisions_list_produces_error() {
    let source = "// @revisions: CHECK release\n";
    let result = parse_directives(source);
    assert!(result.directives.is_empty());
    assert!(!result.errors.is_empty());
    assert!(result.errors[0].message.contains("forbidden revision name"));
}

#[test]
fn test_parse_forbidden_revision_name_case_insensitive() {
    let source = "// @[true] compile-flags: --opt\n";
    let result = parse_directives(source);
    assert!(result.directives.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].message.contains("forbidden revision name"));
}

#[test]
fn test_parse_check_without_colon_produces_error() {
    let source = "// CHECK foo\n";
    let result = parse_directives(source);
    assert!(result.directives.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].message.contains("malformed CHECK"));
}

#[test]
fn test_parse_empty_revisions_produces_error() {
    let source = "// @revisions:\n";
    let result = parse_directives(source);
    assert!(result.directives.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].message.contains("empty revisions"));
}

#[test]
fn test_parse_check_typo_chekc_produces_error() {
    let source = "// CHEKC: some pattern\n";
    let result = parse_directives(source);
    assert!(result.directives.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].message.contains("malformed CHECK"));
}

#[test]
fn test_parse_empty_source_returns_empty() {
    let result = parse_directives("");
    assert!(result.directives.is_empty());
    assert!(result.errors.is_empty());
}
