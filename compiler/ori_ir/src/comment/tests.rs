use super::*;
use crate::StringInterner;

fn test_interner() -> StringInterner {
    StringInterner::new()
}

#[test]
fn test_comment_creation() {
    let interner = test_interner();
    let content = interner.intern("This is a comment");
    let span = Span::new(0, 20);

    let comment = Comment::regular(content, span);

    assert_eq!(comment.kind, CommentKind::Regular);
    assert_eq!(comment.span, span);
}

#[test]
fn test_comment_kind_sort_order() {
    // Description comes first
    assert!(CommentKind::DocDescription.sort_order() < CommentKind::DocMember.sort_order());
    // Member comes before Warning
    assert!(CommentKind::DocMember.sort_order() < CommentKind::DocWarning.sort_order());
    // Warning comes before Example
    assert!(CommentKind::DocWarning.sort_order() < CommentKind::DocExample.sort_order());
    // Regular comments come last
    assert!(CommentKind::DocExample.sort_order() < CommentKind::Regular.sort_order());
}

#[test]
fn test_comment_kind_is_doc() {
    assert!(!CommentKind::Regular.is_doc());
    assert!(CommentKind::DocDescription.is_doc());
    assert!(CommentKind::DocMember.is_doc());
    assert!(CommentKind::DocWarning.is_doc());
    assert!(CommentKind::DocExample.is_doc());
}

#[test]
fn test_comment_list_operations() {
    let interner = test_interner();
    let mut list = CommentList::new();
    assert!(list.is_empty());

    let c1 = Comment::regular(interner.intern("comment 1"), Span::new(0, 10));
    let c2 = Comment::regular(interner.intern("comment 2"), Span::new(20, 30));

    list.push(c1);
    list.push(c2);

    assert_eq!(list.len(), 2);
    assert!(!list.is_empty());
}

#[test]
fn test_comment_list_comments_before() {
    let interner = test_interner();
    let comments = CommentList::from_vec(vec![
        Comment::regular(interner.intern("c1"), Span::new(0, 10)),
        Comment::regular(interner.intern("c2"), Span::new(15, 25)),
        Comment::regular(interner.intern("c3"), Span::new(30, 40)),
    ]);

    let before_20: Vec<_> = comments.comments_before(20).collect();
    assert_eq!(before_20.len(), 1);

    let before_30: Vec<_> = comments.comments_before(30).collect();
    assert_eq!(before_30.len(), 2);
}

#[test]
fn test_comment_list_hash() {
    use std::collections::HashSet;
    let interner = test_interner();
    let mut set = HashSet::new();

    let list1 = CommentList::from_vec(vec![Comment::regular(
        interner.intern("test"),
        Span::new(0, 10),
    )]);
    let list2 = CommentList::from_vec(vec![Comment::regular(
        interner.intern("test"),
        Span::new(0, 10),
    )]);
    let list3 = CommentList::from_vec(vec![Comment::regular(
        interner.intern("other"),
        Span::new(0, 10),
    )]);

    set.insert(list1);
    set.insert(list2); // same as list1
    set.insert(list3);

    assert_eq!(set.len(), 2);
}
