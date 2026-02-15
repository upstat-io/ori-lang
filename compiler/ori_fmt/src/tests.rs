use super::*;

#[test]
fn tabs_to_spaces_single_tab_at_start() {
    assert_eq!(tabs_to_spaces("\t@foo"), "    @foo");
}

#[test]
fn tabs_to_spaces_tab_after_content() {
    // Tab at column 2 should go to column 4
    assert_eq!(tabs_to_spaces("ab\tc"), "ab  c");
}

#[test]
fn tabs_to_spaces_tab_at_column_4() {
    // Tab at column 4 should go to column 8
    assert_eq!(tabs_to_spaces("abcd\te"), "abcd    e");
}

#[test]
fn tabs_to_spaces_multiple_tabs() {
    assert_eq!(tabs_to_spaces("\t\tfoo"), "        foo");
}

#[test]
fn tabs_to_spaces_mixed_content() {
    let input = "fn main\n\treturn 0\n";
    let expected = "fn main\n    return 0\n";
    assert_eq!(tabs_to_spaces(input), expected);
}

#[test]
fn tabs_to_spaces_no_tabs() {
    let input = "    @foo () = 42";
    assert_eq!(tabs_to_spaces(input), input);
}

#[test]
fn tabs_to_spaces_empty_string() {
    assert_eq!(tabs_to_spaces(""), "");
}

#[test]
fn tabs_to_spaces_only_newlines() {
    assert_eq!(tabs_to_spaces("\n\n\n"), "\n\n\n");
}

#[test]
fn tabs_to_spaces_tab_in_middle_of_line() {
    // "x" at col 0, tab at col 1 -> spaces to col 4
    assert_eq!(tabs_to_spaces("x\ty"), "x   y");
}

#[test]
fn tabs_to_spaces_newline_resets_column() {
    // After newline, column resets, so tab goes to col 4
    assert_eq!(tabs_to_spaces("abc\n\tdef"), "abc\n    def");
}
