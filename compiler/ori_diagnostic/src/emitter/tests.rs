use super::*;

#[test]
fn test_escape_json() {
    assert_eq!(escape_json("hello"), "hello");
    assert_eq!(escape_json("\"quoted\""), "\\\"quoted\\\"");
    assert_eq!(escape_json("line1\nline2"), "line1\\nline2");
    assert_eq!(escape_json("path\\file"), "path\\\\file");
    assert_eq!(escape_json("tab\there"), "tab\\there");
}
