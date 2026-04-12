use super::*;

#[test]
fn test_diff_shows_added_lines_with_plus_prefix() {
    let diff = generate_diff("", "new line\n");
    assert!(diff.contains("+new line"));
}

#[test]
fn test_diff_shows_removed_lines_with_minus_prefix() {
    let diff = generate_diff("old line\n", "");
    assert!(diff.contains("-old line"));
}

#[test]
fn test_diff_includes_context_lines() {
    let expected = "line1\nline2\nline3\nline4\nline5\n";
    let actual = "line1\nline2\nCHANGED\nline4\nline5\n";
    let diff = generate_diff(expected, actual);
    // Context lines should include surrounding unchanged lines
    assert!(diff.contains("line2"));
    assert!(diff.contains("line4"));
}

#[test]
fn test_diff_empty_expected_shows_all_actual_as_added() {
    let diff = generate_diff("", "line1\nline2\n");
    assert!(diff.contains("+line1"));
    assert!(diff.contains("+line2"));
}

#[test]
fn test_diff_identical_inputs_produces_empty_output() {
    let diff = generate_diff("same\n", "same\n");
    // Unified diff of identical inputs is empty
    assert!(diff.is_empty() || !diff.contains('+') && !diff.contains('-'));
}
