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
