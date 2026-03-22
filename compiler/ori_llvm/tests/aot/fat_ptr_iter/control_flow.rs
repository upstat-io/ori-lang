//! Control flow regression matrix — element type × iteration pattern cross-product.
//!
//! Covers: str, [int], closures, structs, Option<str> × full, break, yield,
//! two-call, nested, unwind patterns.

use crate::util::assert_aot_success;

#[test]
fn test_matrix_nested_list_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let nested = [[10, 20, 30], [40, 50], [60, 70, 80, 90]];
    let total = 0;
    for inner in nested do {
        for n in inner do {
            if n > 50 then break;
            total = total + n
        }
    };
    if total == 150 then 0 else 1
}
"#,
        "matrix_nested_list_break",
    );
}

/// [[int]] passed to two functions — verifies RC balance across multiple
/// iteration passes over nested collections.
#[test]
fn test_matrix_nested_list_two_calls() {
    assert_aot_success(
        r#"
@sum_nested (lists: [[int]]) -> int = {
    let total = 0;
    for inner in lists do {
        for n in inner do {
            total = total + n
        }
    };
    total
}

@main () -> int = {
    let nested = [[1, 2], [3, 4], [5, 6]];
    let r1 = sum_nested(lists: nested);
    let r2 = sum_nested(lists: nested);
    if r1 == 21 && r2 == 21 then 0 else 1
}
"#,
        "matrix_nested_list_two_calls",
    );
}

/// [[int]] with yield — outer for-yield produces lengths of inner lists.
#[test]
fn test_matrix_nested_list_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let nested = [[1, 2, 3], [4, 5], [6]];
    let lengths = for inner in nested yield inner.len();
    let total = 0;
    for n in lengths do {
        total = total + n
    };
    if total == 6 then 0 else 1
}
"#,
        "matrix_nested_list_yield",
    );
}

/// Struct with Drop field + break — partially consumed iteration.
#[test]
fn test_matrix_struct_break() {
    assert_aot_success(
        r#"
type Item = { label: str, value: int }

@main () -> int = {
    let items = [
        Item { label: "first long label exceeding SSO threshold here", value: 10 },
        Item { label: "second long label also exceeding SSO threshold", value: 20 },
        Item { label: "third long label should not be reached at all", value: 30 }
    ];
    let total = 0;
    for item in items do {
        if item.value > 15 then break;
        total = total + item.value
    };
    if total == 10 then 0 else 1
}
"#,
        "matrix_struct_break",
    );
}

/// Struct with Drop field + yield — extracts labels from structs.
#[test]
fn test_matrix_struct_yield() {
    assert_aot_success(
        r#"
type Item = { label: str, value: int }

@main () -> int = {
    let items = [
        Item { label: "first long label exceeding SSO threshold here", value: 10 },
        Item { label: "second long label also exceeding SSO threshold", value: 20 }
    ];
    let values = for item in items yield item.value;
    let total = 0;
    for v in values do {
        total = total + v
    };
    if total == 30 then 0 else 1
}
"#,
        "matrix_struct_yield",
    );
}

/// Closure list + break — partially consumed closure iteration.
#[test]
fn test_matrix_closure_break() {
    assert_aot_success(
        r#"
@make_fn (n: int) -> (int) -> int = {
    x -> x + n
}

@main () -> int = {
    let fns = [make_fn(n: 100), make_fn(n: 200), make_fn(n: 300)];
    let total = 0;
    for f in fns do {
        let result = f(1);
        if result > 150 then break;
        total = total + result
    };
    if total == 101 then 0 else 1
}
"#,
        "matrix_closure_break",
    );
}

/// Option<str> + yield — extracts string lengths from Some values.
#[test]
fn test_matrix_option_str_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this is a very long string that exceeds SSO threshold"),
        None,
        Some("another very long string that also exceeds the threshold")
    ];
    let lengths = for item in items yield {
        match item {
            Some(s) -> s.len(),
            None -> 0
        }
    };
    let total = 0;
    for n in lengths do {
        total = total + n
    };
    if total == 109 then 0 else 1
}
"#,
        "matrix_option_str_yield",
    );
}

/// [str] + unwind during nested function + catch — most complex
/// scenario combining fat pointers, iteration, and error recovery.
#[test]
fn test_matrix_str_nested_unwind() {
    assert_aot_success(
        r#"
@process_word (w: str) -> int = {
    if w.starts_with(prefix: "panic") then {
        panic(msg: "word triggered panic")
    };
    w.len()
}

@process_all (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + process_word(w: w)
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "panic trigger long string for heap allocation needed",
        "unreachable very long string that is also on the heap"
    ];
    let r1 = catch(expr: process_all(words: words));
    let r2 = process_all(words: [
        "first safe long string exceeding SSO threshold here",
        "second safe long string also exceeding the threshold"
    ]);
    let caught = match r1 { Ok(_) -> false, Err(_) -> true };
    if caught && r2 == 103 then 0 else 1
}
"#,
        "matrix_str_nested_unwind",
    );
}

// F4: For-guard (for x in coll if predicate do body)

/// T1-F4: [str] for-do with guard — elements failing the guard must be cleaned up.
#[test]
fn test_str_list_for_do_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "short",
        "this is a very long string that exceeds SSO threshold",
        "tiny",
        "another very long string that also exceeds the threshold"
    ];
    let total = 0;
    for w in words if w.len() > 10 do {
        total = total + w.len();
    };
    // Only the two long strings: 53 + 56 = 109
    if total == 109 then 0 else 1
}
"#,
        "str_list_for_do_guard",
    );
}

/// T2-F4: [[int]] for-do with guard — inner list elements skipped by guard cleaned up.
#[test]
fn test_nested_list_for_do_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lists = [[1, 2, 3], [10, 20, 30], [4, 5]];
    let total = 0;
    for inner in lists if inner.len() == 3 do {
        for n in inner do {
            total = total + n;
        };
    };
    // [1,2,3] (len=3): 6. [10,20,30] (len=3): 60. [4,5] (len=2): skipped.
    if total == 66 then 0 else 1
}
"#,
        "nested_list_for_do_guard",
    );
}

// F7: Continue (skip element)

/// T1-F7: [str] for-do with continue — skipped str elements must be cleaned up.
#[test]
fn test_str_list_for_do_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "skip this very long string because starts with skip xx",
        "another very long string that also exceeds the threshold"
    ];
    let total = 0;
    for w in words do {
        if w.starts_with(prefix: "skip") then { continue; };
        total = total + w.len();
    };
    // First (53) + third (56) = 109, skip middle.
    if total == 109 then 0 else 1
}
"#,
        "str_list_for_do_continue",
    );
}

/// T2-F7: [[int]] for-do with continue — skipped inner lists must be cleaned up.
#[test]
fn test_nested_list_for_do_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lists = [[1, 2, 3], [100, 200], [4, 5, 6]];
    let total = 0;
    for inner in lists do {
        if inner.len() == 2 then { continue; };
        for n in inner do {
            total = total + n;
        };
    };
    // [1,2,3] = 6, skip [100,200], [4,5,6] = 15. Total = 21.
    if total == 21 then 0 else 1
}
"#,
        "nested_list_for_do_continue",
    );
}

// F8: Iteration in match arm

/// T1-F8: [str] iteration inside a match arm — cleanup regardless of which arm executes.
#[test]
fn test_str_list_iteration_in_match() {
    assert_aot_success(
        r#"
@main () -> int = {
    let maybe_words: Option<[str]> = Some([
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ]);
    let total = match maybe_words {
        Some(words) -> {
            let t = 0;
            for w in words do {
                t = t + w.len();
            };
            t
        },
        None -> 0
    };
    // 53 + 56 = 109
    if total == 109 then 0 else 1
}
"#,
        "str_list_iteration_in_match",
    );
}

// F9: Slice iteration

/// T1-F9: [str] slice iteration — create list, take slice, iterate slice.
/// Verifies `elem_dec_fn` is read from the ORIGINAL buffer's header.
#[test]
fn test_str_list_slice_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold",
        "third very long string also well exceeding SSO threshold"
    ];
    let slice = words.slice(start: 0, end: 2);
    let total = 0;
    for w in slice do {
        total = total + w.len();
    };
    // First two: 53 + 56 = 109
    if total == 109 then 0 else 1
}
"#,
        "str_list_slice_iteration",
    );
}
