use super::*;

#[test]
fn test_classify_regular_comment() {
    let (kind, content) = classify_and_normalize_comment(" regular text");
    assert_eq!(kind, CommentKind::Regular);
    assert_eq!(content, " regular text");
}

#[test]
fn test_classify_doc_description() {
    let (kind, content) = classify_and_normalize_comment(" #Description");
    assert_eq!(kind, CommentKind::DocDescription);
    assert_eq!(content, " #Description");

    // With extra spaces
    let (kind, content) = classify_and_normalize_comment("  #Description");
    assert_eq!(kind, CommentKind::DocDescription);
    assert_eq!(content, " #Description");
}

#[test]
fn test_classify_legacy_param_as_member() {
    let (kind, content) = classify_and_normalize_comment(" @param x value");
    assert_eq!(kind, CommentKind::DocMember);
    assert_eq!(content, " @param x value");
}

#[test]
fn test_classify_legacy_field_as_member() {
    let (kind, content) = classify_and_normalize_comment(" @field x coord");
    assert_eq!(kind, CommentKind::DocMember);
    assert_eq!(content, " @field x coord");
}

#[test]
fn test_classify_doc_warning() {
    let (kind, content) = classify_and_normalize_comment(" !Panics");
    assert_eq!(kind, CommentKind::DocWarning);
    assert_eq!(content, " !Panics");
}

#[test]
fn test_classify_doc_example() {
    let (kind, content) = classify_and_normalize_comment(" >foo() -> 1");
    assert_eq!(kind, CommentKind::DocExample);
    // Preserve spacing after > exactly
    assert_eq!(content, " >foo() -> 1");
}

#[test]
fn test_classify_doc_member() {
    let (kind, content) = classify_and_normalize_comment(" * x: The value");
    assert_eq!(kind, CommentKind::DocMember);
    assert_eq!(content, " * x: The value");
}

#[test]
fn test_classify_doc_member_no_description() {
    let (kind, content) = classify_and_normalize_comment(" * name:");
    assert_eq!(kind, CommentKind::DocMember);
    assert_eq!(content, " * name:");
}

#[test]
fn test_classify_doc_member_underscore_name() {
    let (kind, content) = classify_and_normalize_comment(" * my_param: A value");
    assert_eq!(kind, CommentKind::DocMember);
    assert_eq!(content, " * my_param: A value");
}

#[test]
fn test_classify_star_without_colon_is_regular() {
    // `* text` without a colon is a regular comment (bullet list)
    let (kind, _) = classify_and_normalize_comment(" * just a bullet");
    assert_eq!(kind, CommentKind::Regular);
}

#[test]
fn test_classify_star_with_spaces_in_name_is_regular() {
    // `* two words: desc` is regular because "two words" has a space
    let (kind, _) = classify_and_normalize_comment(" * two words: desc");
    assert_eq!(kind, CommentKind::Regular);
}

#[test]
fn test_classify_doc_member_extra_spaces() {
    // Extra leading spaces should still work
    let (kind, content) = classify_and_normalize_comment("  * x: value");
    assert_eq!(kind, CommentKind::DocMember);
    assert_eq!(content, " * x: value");
}

#[test]
fn test_classify_empty_comment() {
    let (kind, content) = classify_and_normalize_comment("");
    assert_eq!(kind, CommentKind::Regular);
    assert_eq!(content, "");
}

#[test]
fn test_classify_no_space_adds_space() {
    let (kind, content) = classify_and_normalize_comment("no space");
    assert_eq!(kind, CommentKind::Regular);
    assert_eq!(content, " no space");
}

#[test]
fn test_legacy_param_emits_doc_member() {
    let (kind, content) = classify_and_normalize_comment(" @param x The value");
    assert_eq!(kind, CommentKind::DocMember);
    assert_eq!(content, " @param x The value");
}

#[test]
fn test_classify_regular_borrows() {
    // The common case (regular comment with space) should borrow, not allocate
    let (kind, content) = classify_and_normalize_comment(" regular text");
    assert_eq!(kind, CommentKind::Regular);
    assert!(matches!(content, Cow::Borrowed(_)));
}

#[test]
fn test_legacy_field_emits_doc_member() {
    let (kind, content) = classify_and_normalize_comment(" @field y The coord");
    assert_eq!(kind, CommentKind::DocMember);
    assert_eq!(content, " @field y The coord");
}
