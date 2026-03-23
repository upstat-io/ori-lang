//! Nested list iteration tests — T2 (`[[int]]`) and T3 (`[[str]]`).

use crate::util::assert_aot_success;

// T2: [[int]] — inner lists are RC-managed buffers

#[test]
fn test_nested_list_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lists = [[1, 2, 3], [4, 5, 6]];
    let total = 0;
    for inner in lists do {
        for n in inner do {
            total = total + n;
        };
    };
    if total == 21 then 0 else 1
}
"#,
        "nested_list_iteration",
    );
}

// T3: [[str]] — doubly-nested fat pointers (inner str elements need elem_dec_fn)

#[test]
fn test_nested_str_list_iteration() {
    // Both the outer list and inner lists need elem_dec_fn cleanup.
    // Outer: elem_dec_fn decs inner [str] buffers.
    // Inner: elem_dec_fn decs str elements within each inner list.
    assert_aot_success(
        r#"
@main () -> int = {
    let lists = [
        ["this is a very long string that exceeds SSO threshold", "also long enough to be on the heap here"],
        ["third very long string exceeding SSO threshold bytes", "fourth long string on the heap too"]
    ];
    let total = 0;
    for inner in lists do {
        for s in inner do {
            total = total + s.len();
        };
    };
    // 53 + 39 + 52 + 34 = 178
    if total == 178 then 0 else 1
}
"#,
        "nested_str_list_iteration",
    );
}

// T3-F2: [[str]] with break in outer loop — un-consumed outer elements cleaned up.
// Break after first outer iteration: second and third inner [str] lists are un-consumed
// and must have both their buffer RC and element (str) RC decremented.

#[test]
fn test_nested_str_list_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lists = [
        ["this is a very long string that exceeds SSO threshold", "also long enough to be on the heap here"],
        ["second inner list with long strings exceeding SSO th", "not reached string also over SSO limit"],
        ["third list also not reached because outer broke early", "these strings must still be cleaned up"]
    ];
    let total = 0;
    let count = 0;
    for inner in lists do {
        for s in inner do {
            total = total + s.len();
        };
        count = count + 1;
        if count == 1 then break;
    };
    // First inner only: 53 + 39 = 92. Second and third un-consumed.
    if total == 92 then 0 else 1
}
"#,
        "nested_str_list_break",
    );
}

// T3-F5: [[str]] passed to function twice — shared ownership, no double-free.

#[test]
fn test_nested_str_list_two_calls() {
    assert_aot_success(
        r#"
@sum_lengths (lists: [[str]]) -> int = {
    let total = 0;
    for inner in lists do {
        for s in inner do {
            total = total + s.len();
        };
    };
    total
}

@main () -> int = {
    let lists = [
        ["this is a very long string that exceeds SSO threshold", "also long enough to be on the heap here"],
        ["third very long string exceeding SSO threshold bytes", "fourth long string on the heap too"]
    ];
    let r1 = sum_lengths(lists: lists);
    let r2 = sum_lengths(lists: lists);
    // 53 + 39 + 52 + 34 = 178 each call
    if r1 == 178 && r2 == 178 then 0 else 1
}
"#,
        "nested_str_list_two_calls",
    );
}
