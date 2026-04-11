use super::*;
use std::path::Path;

#[test]
fn test_expected_path_is_sibling_of_source_file() {
    let result = resolve_expected(Path::new("tests/rc/basic.ori"), "ll", None);
    assert_eq!(result, Path::new("tests/rc/basic.ll"));
}

#[test]
fn test_actual_path_is_under_target_test_harness() {
    let result = resolve_actual(Path::new("tests/rc/basic.ori"), "ll", None);
    assert_eq!(result, Path::new("target/test-harness/tests/rc/basic.ll"));
}

#[test]
fn test_actual_path_preserves_parent_dir_to_prevent_collision() {
    let a = resolve_actual(Path::new("tests/codegen/basic.ori"), "ll", None);
    let b = resolve_actual(Path::new("tests/other/basic.ori"), "ll", None);
    assert_ne!(a, b, "same-stem files in different dirs must not collide");
}

#[test]
fn test_resolve_without_revision_omits_revision_suffix() {
    let result = resolve_expected(Path::new("tests/rc/basic.ori"), "ll", None);
    assert_eq!(result, Path::new("tests/rc/basic.ll"));
}

#[test]
fn test_resolve_with_revision_inserts_suffix_before_extension() {
    let result = resolve_expected(Path::new("tests/rc/basic.ori"), "ll", Some("release"));
    assert_eq!(result, Path::new("tests/rc/basic.release.ll"));
}

#[test]
fn test_revision_suffix_ordering_is_deterministic() {
    let a = resolve_expected(Path::new("t/x.ori"), "arc", Some("debug"));
    let b = resolve_expected(Path::new("t/x.ori"), "arc", Some("debug"));
    assert_eq!(a, b);
}

#[test]
fn test_resolve_actual_with_revision_inserts_suffix_before_extension() {
    let result = resolve_actual(Path::new("tests/rc/basic.ori"), "ll", Some("debug"));
    assert_eq!(
        result,
        Path::new("target/test-harness/tests/rc/basic.debug.ll")
    );
}
