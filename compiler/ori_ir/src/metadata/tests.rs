use super::*;
use crate::StringInterner;

fn test_interner() -> StringInterner {
    StringInterner::new()
}

#[test]
fn test_module_extra_default() {
    let extra = ModuleExtra::new();
    assert!(extra.comments.is_empty());
    assert!(extra.blank_lines.is_empty());
    assert!(extra.newlines.is_empty());
    assert!(extra.trailing_commas.is_empty());
}

#[test]
fn test_has_blank_line_between() {
    let mut extra = ModuleExtra::new();
    extra.add_blank_line(50);
    extra.add_blank_line(100);

    assert!(!extra.has_blank_line_between(0, 40));
    assert!(extra.has_blank_line_between(40, 60));
    assert!(extra.has_blank_line_between(0, 150));
    assert!(!extra.has_blank_line_between(60, 90));
}

#[test]
fn test_line_number() {
    let mut extra = ModuleExtra::new();
    // Simulate newlines at positions 10, 25, 40
    extra.add_newline(10);
    extra.add_newline(25);
    extra.add_newline(40);

    assert_eq!(extra.line_number(0), 1);
    assert_eq!(extra.line_number(5), 1);
    assert_eq!(extra.line_number(15), 2);
    assert_eq!(extra.line_number(30), 3);
    assert_eq!(extra.line_number(50), 4);
}

#[test]
fn test_is_multiline() {
    let mut extra = ModuleExtra::new();
    extra.add_newline(20);
    extra.add_newline(40);

    assert!(!extra.is_multiline(Span::new(0, 15)));
    assert!(extra.is_multiline(Span::new(10, 30)));
    assert!(extra.is_multiline(Span::new(0, 50)));
}

#[test]
fn test_doc_comments_for() {
    let interner = test_interner();
    let mut extra = ModuleExtra::new();

    // Add some comments
    extra.comments.push(Comment::new(
        interner.intern("Regular comment"),
        Span::new(0, 20),
        CommentKind::Regular,
    ));
    extra.comments.push(Comment::new(
        interner.intern("Description"),
        Span::new(25, 45),
        CommentKind::DocDescription,
    ));
    extra.comments.push(Comment::new(
        interner.intern(" * x: The value"),
        Span::new(46, 60),
        CommentKind::DocMember,
    ));

    // Declaration starts at position 65
    let docs = extra.doc_comments_for(65);
    assert_eq!(docs.len(), 2);
    assert!(docs[0].kind.is_doc());
    assert!(docs[1].kind.is_doc());
}

#[test]
fn test_doc_comments_blocked_by_blank_line() {
    let interner = test_interner();
    let mut extra = ModuleExtra::new();

    // Doc comment
    extra.comments.push(Comment::new(
        interner.intern("Description"),
        Span::new(0, 20),
        CommentKind::DocDescription,
    ));

    // Blank line at position 25
    extra.add_blank_line(25);

    // Declaration at position 30
    let docs = extra.doc_comments_for(30);
    assert!(
        docs.is_empty(),
        "Blank line should block doc comment attachment"
    );
}

#[test]
fn test_trailing_commas() {
    let mut extra = ModuleExtra::new();
    extra.add_trailing_comma(50);
    extra.add_trailing_comma(100);

    assert!(extra.has_trailing_comma(50));
    assert!(extra.has_trailing_comma(100));
    assert!(!extra.has_trailing_comma(75));
}

#[test]
fn test_module_extra_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    let extra1 = ModuleExtra::new();
    let extra2 = ModuleExtra::new();

    set.insert(extra1);
    set.insert(extra2);

    assert_eq!(set.len(), 1, "Equal ModuleExtra should hash the same");
}

#[test]
fn test_unattached_doc_comments() {
    let interner = test_interner();
    let mut extra = ModuleExtra::new();

    // Doc comment at start
    extra.comments.push(Comment::new(
        interner.intern("Orphan doc"),
        Span::new(0, 15),
        CommentKind::DocDescription,
    ));

    // Blank line
    extra.add_blank_line(20);

    // Another doc comment after blank line
    extra.comments.push(Comment::new(
        interner.intern("Attached doc"),
        Span::new(25, 40),
        CommentKind::DocDescription,
    ));

    // Declaration starts
    let declaration_starts = vec![45u32];
    let unattached = extra.unattached_doc_comments(&declaration_starts);

    assert_eq!(unattached.len(), 1);
    assert_eq!(unattached[0].span.start, 0);
}
