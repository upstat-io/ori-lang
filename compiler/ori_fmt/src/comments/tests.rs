use super::*;
use ori_ir::StringInterner;

fn test_interner() -> StringInterner {
    StringInterner::new()
}

#[test]
fn test_extract_member_name_any_star_format() {
    assert_eq!(extract_member_name_any(" * x: The value"), "x");
    assert_eq!(extract_member_name_any(" * my_param: A value"), "my_param");
    assert_eq!(extract_member_name_any(" * name:"), "name");
    assert_eq!(extract_member_name_any("* x: val"), "x");
    assert_eq!(extract_member_name_any(" * :"), ""); // empty name
}

#[test]
fn test_extract_member_name_any_legacy_param() {
    assert_eq!(extract_member_name_any(" @param x The value"), "x");
    assert_eq!(extract_member_name_any(" @param foo description"), "foo");
    assert_eq!(extract_member_name_any(" @param "), "");
}

#[test]
fn test_extract_member_name_any_legacy_field() {
    assert_eq!(extract_member_name_any(" @field x The coordinate"), "x");
    assert_eq!(extract_member_name_any(" @field name description"), "name");
    assert_eq!(extract_member_name_any(" @field "), "");
}

#[test]
fn test_extract_member_name_any_unknown() {
    assert_eq!(extract_member_name_any("not a member"), "");
}

#[test]
fn test_format_comment() {
    let interner = test_interner();
    let comment = Comment::regular(interner.intern(" hello world"), Span::new(0, 15));

    let formatted = format_comment(&comment, &interner);
    assert_eq!(formatted, "// hello world");
}

#[test]
fn test_format_comment_doc() {
    let interner = test_interner();
    let comment = Comment::new(
        interner.intern(" #Description"),
        Span::new(0, 15),
        CommentKind::DocDescription,
    );

    let formatted = format_comment(&comment, &interner);
    assert_eq!(formatted, "// #Description");
}

#[test]
fn test_sort_comments_by_kind() {
    // Example before Description - should be reordered
    let refs = vec![
        CommentRef {
            index: 0,
            kind: CommentKind::DocExample,
        },
        CommentRef {
            index: 1,
            kind: CommentKind::DocDescription,
        },
    ];

    let sorted = sort_comments_by_kind(refs);

    // Description (sort_order=0) should come before Example (sort_order=3)
    assert_eq!(sorted[0].index, 1);
    assert_eq!(sorted[1].index, 0);
}

#[test]
fn test_sort_regular_comments_preserved() {
    let refs = vec![
        CommentRef {
            index: 0,
            kind: CommentKind::Regular,
        },
        CommentRef {
            index: 1,
            kind: CommentKind::Regular,
        },
        CommentRef {
            index: 2,
            kind: CommentKind::Regular,
        },
    ];

    let sorted = sort_comments_by_kind(refs);

    // Order should be preserved
    assert_eq!(sorted[0].index, 0);
    assert_eq!(sorted[1].index, 1);
    assert_eq!(sorted[2].index, 2);
}

#[test]
fn test_comment_index_basic() {
    let interner = test_interner();
    let comments = CommentList::from_vec(vec![
        Comment::regular(interner.intern(" first"), Span::new(0, 8)),
        Comment::regular(interner.intern(" second"), Span::new(10, 19)),
    ]);

    // Tokens at positions 9 (after first comment) and 20 (after second)
    let token_positions = vec![9, 20];

    let mut index = CommentIndex::new(&comments, &token_positions);

    // Get comments before position 9
    let before_9 = index.take_comments_before(9);
    assert_eq!(before_9, vec![0]);

    // Get comments before position 20
    let before_20 = index.take_comments_before(20);
    assert_eq!(before_20, vec![1]);
}

#[test]
fn test_reorder_param_comments_legacy() {
    let interner = test_interner();
    let comments = CommentList::from_vec(vec![
        Comment::new(
            interner.intern(" @param b Second"),
            Span::new(0, 20),
            CommentKind::DocMember,
        ),
        Comment::new(
            interner.intern(" @param a First"),
            Span::new(21, 40),
            CommentKind::DocMember,
        ),
    ]);

    let param_names = ["a", "b"];
    let indices = vec![0, 1];

    let reordered = reorder_param_comments(&indices, &comments, &param_names, &interner);

    // Should be reordered to match param order: a (index 1) then b (index 0)
    assert_eq!(reordered, vec![1, 0]);
}

#[test]
fn test_group_comments_for_reordering() {
    let interner = test_interner();
    let comments = CommentList::from_vec(vec![
        Comment::regular(interner.intern(" regular"), Span::new(0, 10)),
        Comment::new(
            interner.intern(" #Description"),
            Span::new(11, 25),
            CommentKind::DocDescription,
        ),
        Comment::new(
            interner.intern(" * x: A param"),
            Span::new(26, 35),
            CommentKind::DocMember,
        ),
        Comment::regular(interner.intern(" another regular"), Span::new(36, 55)),
    ]);

    let indices = vec![0, 1, 2, 3];
    let groups = group_comments_for_reordering(&indices, &comments);

    // Should have 3 groups: [0] (regular), [1,2] (doc), [3] (regular)
    assert_eq!(groups.len(), 3);
    assert_eq!(groups[0], vec![0]);
    assert_eq!(groups[1], vec![1, 2]);
    assert_eq!(groups[2], vec![3]);
}

#[test]
fn test_reorder_member_param_comments() {
    let interner = test_interner();
    let comments = CommentList::from_vec(vec![
        Comment::new(
            interner.intern(" * b: Second"),
            Span::new(0, 20),
            CommentKind::DocMember,
        ),
        Comment::new(
            interner.intern(" * a: First"),
            Span::new(21, 40),
            CommentKind::DocMember,
        ),
    ]);

    let param_names = ["a", "b"];
    let indices = vec![0, 1];

    let reordered = reorder_param_comments(&indices, &comments, &param_names, &interner);

    // Should be reordered to match param order: a (index 1) then b (index 0)
    assert_eq!(reordered, vec![1, 0]);
}

#[test]
fn test_reorder_member_field_comments() {
    let interner = test_interner();
    let comments = CommentList::from_vec(vec![
        Comment::new(
            interner.intern(" * y: The Y"),
            Span::new(0, 20),
            CommentKind::DocMember,
        ),
        Comment::new(
            interner.intern(" * x: The X"),
            Span::new(21, 40),
            CommentKind::DocMember,
        ),
    ]);

    let field_names = ["x", "y"];
    let indices = vec![0, 1];

    let reordered = reorder_field_comments(&indices, &comments, &field_names, &interner);

    // Should be reordered to match field order: x (index 1) then y (index 0)
    assert_eq!(reordered, vec![1, 0]);
}
