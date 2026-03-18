//! Iterator-collection RC ownership matrix tests.
//!
//! Comprehensive combinatorial coverage: 6 element types × 8 iteration
//! patterns × 2 loop variants = 88 tests. Exercises every RC-relevant
//! combination to prevent regression.
//!
//! Matrix dimensions:
//!   Loop:    for-do (P1-P2, P4-P8), for-yield (P1-P8)
//!   Type:    E1=str, E2=[int] nested, E3=Option<str>, E4=closure, E5=struct, E6=map
//!   Pattern: P1=full, P2=break, P3=yield-transform, P4=two-call, P5=nested,
//!            P6=guard, P7=unwind, P8=continue
//!
//! E7 (Set<str>) is excluded — Set<str> not yet implemented in AOT.
//! E2×P5 for-yield excluded — nested yield of [int] collapses to flat int; covered by P1.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// -----------------------------------------------------------------------
// E1: [str] — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_str_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "second string also exceeds the SSO threshold of twenty three bytes",
        "third string that is also long enough to require heap allocation"
    ];
    let count = 0;
    for s in items do count = count + 1;
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_str_full",
    );
}

#[test]
fn test_iter_rc_for_do_str_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "stop: this string triggers break and is also long enough for heap",
        "third string that should never be visited after the break fires"
    ];
    let count = 0;
    for s in items do {
        if s.starts_with(prefix: "stop") then break;
        count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "iter_rc_for_do_str_break",
    );
}

#[test]
fn test_iter_rc_for_do_str_two_call() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "second string also exceeds the SSO threshold of twenty three bytes"
    ];
    let count1 = 0;
    for s in items do count1 = count1 + 1;
    let count2 = 0;
    for s in items do count2 = count2 + 1;
    if count1 == 2 && count2 == 2 then 0 else 1
}
"#,
        "iter_rc_for_do_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_str_nested() {
    assert_aot_success(
        r#"
@main () -> int = {
    let outer = [
        ["this string exceeds SSO threshold by being very long indeed",
         "second string also exceeds the SSO threshold for heap alloc"],
        ["third string that is long enough to require heap allocation"]
    ];
    let count = 0;
    for inner in outer do {
        for s in inner do count = count + 1;
    };
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_str_nested",
    );
}

#[test]
fn test_iter_rc_for_do_str_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "keep: this string passes the guard and is also long for heap alloc",
        "this string exceeds SSO threshold by being very long indeed too"
    ];
    let count = 0;
    for s in items do {
        if s.starts_with(prefix: "keep") then count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "iter_rc_for_do_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_str_unwind() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "panic: this string triggers panic and is long enough for heap alloc",
        "third string that should not be visited after panic fires at all"
    ];
    let result = catch(expr: () -> {
        for s in items do {
            if s.starts_with(prefix: "panic") then panic(msg: "test unwind");
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_do_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_str_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "skip: this string should be skipped via continue in the loop body",
        "this string exceeds SSO threshold by being very long indeed count",
        "skip: another string that should also be skipped via continue here"
    ];
    let count = 0;
    for s in items do {
        if s.starts_with(prefix: "skip") then continue;
        count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "iter_rc_for_do_str_continue",
    );
}

// -----------------------------------------------------------------------
// E2: [[int]] nested list — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_nested_list_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    let count = 0;
    for inner in items do count = count + 1;
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_nested_list_full",
    );
}

#[test]
fn test_iter_rc_for_do_nested_list_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2], [3, 4], [5, 6]];
    let count = 0;
    for inner in items do {
        if count == 1 then break;
        count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "iter_rc_for_do_nested_list_break",
    );
}

#[test]
fn test_iter_rc_for_do_nested_list_two_call() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4, 5, 6]];
    let count1 = 0;
    for inner in items do count1 = count1 + 1;
    let count2 = 0;
    for inner in items do count2 = count2 + 1;
    if count1 == 2 && count2 == 2 then 0 else 1
}
"#,
        "iter_rc_for_do_nested_list_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_nested_list_nested() {
    assert_aot_success(
        r#"
@main () -> int = {
    let outer = [[[1, 2], [3, 4]], [[5, 6]]];
    let count = 0;
    for middle in outer do {
        for inner in middle do count = count + 1;
    };
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_nested_list_nested",
    );
}

#[test]
fn test_iter_rc_for_do_nested_list_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    let count = 0;
    for inner in items do {
        if inner.length() == 3 then count = count + 1;
    };
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_nested_list_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_nested_list_unwind() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    let result = catch(expr: () -> {
        for inner in items do {
            if inner.length() == 3 then panic(msg: "test unwind nested list");
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_do_nested_list_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_nested_list_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4], [7, 8, 9]];
    let count = 0;
    for inner in items do {
        if inner.length() == 1 then continue;
        count = count + 1;
    };
    if count == 2 then 0 else 1
}
"#,
        "iter_rc_for_do_nested_list_continue",
    );
}

// -----------------------------------------------------------------------
// E3: [Option<str>] — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_option_str_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let count = 0;
    for opt in items do count = count + 1;
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_option_str_full",
    );
}

#[test]
fn test_iter_rc_for_do_option_str_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let count = 0;
    for opt in items do {
        match opt {
            None -> break,
            Some(_) -> count = count + 1,
        };
    };
    if count == 1 then 0 else 1
}
"#,
        "iter_rc_for_do_option_str_break",
    );
}

#[test]
fn test_iter_rc_for_do_option_str_two_call() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let count1 = 0;
    for opt in items do count1 = count1 + 1;
    let count2 = 0;
    for opt in items do count2 = count2 + 1;
    if count1 == 3 && count2 == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_option_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_option_str_nested() {
    assert_aot_success(
        r#"
@main () -> int = {
    let outer = [
        [Some("this string exceeds SSO threshold by being very long indeed"),
         None],
        [Some("third string that is also long enough to require heap alloc")]
    ];
    let count = 0;
    for inner in outer do {
        for opt in inner do count = count + 1;
    };
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_option_str_nested",
    );
}

#[test]
fn test_iter_rc_for_do_option_str_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let count = 0;
    for opt in items do {
        if is_some(option: opt) then count = count + 1;
    };
    if count == 2 then 0 else 1
}
"#,
        "iter_rc_for_do_option_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_option_str_unwind() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let result = catch(expr: () -> {
        for opt in items do {
            if is_none(option: opt) then panic(msg: "test unwind option str");
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_do_option_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_option_str_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let count = 0;
    for opt in items do {
        if is_none(option: opt) then continue;
        count = count + 1;
    };
    if count == 2 then 0 else 1
}
"#,
        "iter_rc_for_do_option_str_continue",
    );
}

// -----------------------------------------------------------------------
// E4: [closure] — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_closure_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let count = 0;
    for f in fns do count = count + 1;
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_closure_full",
    );
}

#[test]
fn test_iter_rc_for_do_closure_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let count = 0;
    for f in fns do {
        if count == 1 then break;
        count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "iter_rc_for_do_closure_break",
    );
}

#[test]
fn test_iter_rc_for_do_closure_two_call() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let count1 = 0;
    for f in fns do count1 = count1 + 1;
    let count2 = 0;
    for f in fns do count2 = count2 + 1;
    if count1 == 3 && count2 == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_closure_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_closure_nested() {
    assert_aot_success(
        r#"
@main () -> int = {
    let outer = [[x -> x + 1, x -> x * 2], [x -> x - 1]];
    let count = 0;
    for inner in outer do {
        for f in inner do count = count + 1;
    };
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_closure_nested",
    );
}

#[test]
fn test_iter_rc_for_do_closure_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let sum = 0;
    let idx = 0;
    for f in fns do {
        if idx == 1 then sum = sum + f(10);
        idx = idx + 1;
    };
    if sum == 20 then 0 else 1
}
"#,
        "iter_rc_for_do_closure_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_closure_unwind() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let result = catch(expr: () -> {
        let idx = 0;
        for f in fns do {
            if idx == 1 then panic(msg: "test unwind closure");
            idx = idx + 1;
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_do_closure_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_closure_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let count = 0;
    let idx = 0;
    for f in fns do {
        if idx == 1 then {
            idx = idx + 1;
            continue;
        };
        count = count + 1;
        idx = idx + 1;
    };
    if count == 2 then 0 else 1
}
"#,
        "iter_rc_for_do_closure_continue",
    );
}

// -----------------------------------------------------------------------
// E5: [struct with str field] — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_struct_str_full() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 },
        Item { name: "third item name that is also long enough for heap alloc", val: 30 }
    ];
    let count = 0;
    for item in items do count = count + 1;
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_struct_str_full",
    );
}

#[test]
fn test_iter_rc_for_do_struct_str_break() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "stop: this item triggers break and name is long for heap", val: 20 },
        Item { name: "third item that should not be visited after break fires", val: 30 }
    ];
    let count = 0;
    for item in items do {
        if item.name.starts_with(prefix: "stop") then break;
        count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "iter_rc_for_do_struct_str_break",
    );
}

#[test]
fn test_iter_rc_for_do_struct_str_two_call() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 }
    ];
    let count1 = 0;
    for item in items do count1 = count1 + 1;
    let count2 = 0;
    for item in items do count2 = count2 + 1;
    if count1 == 2 && count2 == 2 then 0 else 1
}
"#,
        "iter_rc_for_do_struct_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_struct_str_nested() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let outer = [
        [Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
         Item { name: "second item name also exceeds the SSO threshold heap", val: 20 }],
        [Item { name: "third item name that is also long enough for heap alloc", val: 30 }]
    ];
    let count = 0;
    for inner in outer do {
        for item in inner do count = count + 1;
    };
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_struct_str_nested",
    );
}

#[test]
fn test_iter_rc_for_do_struct_str_guard() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 },
        Item { name: "third item name that is also long enough for heap alloc", val: 30 }
    ];
    let sum = 0;
    for item in items do {
        if item.val > 15 then sum = sum + item.val;
    };
    if sum == 50 then 0 else 1
}
"#,
        "iter_rc_for_do_struct_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_struct_str_unwind() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 },
        Item { name: "third item name that is also long enough for heap alloc", val: 30 }
    ];
    let result = catch(expr: () -> {
        for item in items do {
            if item.val == 20 then panic(msg: "test unwind struct str");
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_do_struct_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_struct_str_continue() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 },
        Item { name: "third item name that is also long enough for heap alloc", val: 30 }
    ];
    let sum = 0;
    for item in items do {
        if item.val == 20 then continue;
        sum = sum + item.val;
    };
    if sum == 40 then 0 else 1
}
"#,
        "iter_rc_for_do_struct_str_continue",
    );
}

// -----------------------------------------------------------------------
// E6: {str: int} map — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_map_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let count = 0;
    for entry in m do count = count + 1;
    if count == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_map_full",
    );
}

#[test]
fn test_iter_rc_for_do_map_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20
    };
    let count = 0;
    for entry in m do {
        if count == 1 then break;
        count = count + 1;
    };
    if count == 1 then 0 else 1
}
"#,
        "iter_rc_for_do_map_break",
    );
}

#[test]
fn test_iter_rc_for_do_map_two_call() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let count1 = 0;
    for entry in m do count1 = count1 + 1;
    let count2 = 0;
    for entry in m do count2 = count2 + 1;
    if count1 == 3 && count2 == 3 then 0 else 1
}
"#,
        "iter_rc_for_do_map_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_map_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let sum = 0;
    for entry in m do {
        if entry.1 > 15 then sum = sum + entry.1;
    };
    if sum == 50 then 0 else 1
}
"#,
        "iter_rc_for_do_map_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_map_unwind() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let result = catch(expr: () -> {
        for entry in m do {
            if entry.1 == 20 then panic(msg: "test unwind map");
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_do_map_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_map_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let sum = 0;
    for entry in m do {
        if entry.1 == 20 then continue;
        sum = sum + entry.1;
    };
    if sum == 40 then 0 else 1
}
"#,
        "iter_rc_for_do_map_continue",
    );
}

// -----------------------------------------------------------------------
// E1: [str] — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_str_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "second string also exceeds the SSO threshold of twenty three bytes",
        "third string that is also long enough to require heap allocation"
    ];
    let lengths = for s in items yield s.len();
    if lengths.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_str_full",
    );
}

#[test]
fn test_iter_rc_for_yield_str_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "stop: this string triggers break and is also long enough for heap",
        "third string that should never be visited after the break fires"
    ];
    let result = for s in items yield {
        if s.starts_with(prefix: "stop") then break;
        s.len()
    };
    if result.length() == 1 then 0 else 1
}
"#,
        "iter_rc_for_yield_str_break",
    );
}

#[test]
fn test_iter_rc_for_yield_str_yield_transform() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "second string also exceeds the SSO threshold of twenty three bytes"
    ];
    let doubled = for s in items yield s.len() * 2;
    let sum = 0;
    for n in doubled do sum = sum + n;
    if sum > 0 then 0 else 1
}
"#,
        "iter_rc_for_yield_str_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_str_two_call() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "second string also exceeds the SSO threshold of twenty three bytes"
    ];
    let r1 = for s in items yield s.len();
    let r2 = for s in items yield s.len();
    if r1.length() == 2 && r2.length() == 2 then 0 else 1
}
"#,
        "iter_rc_for_yield_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_str_nested() {
    // Nested for-yield: outer yields inner lists, which are then iterated.
    assert_aot_success(
        r#"
@main () -> int = {
    let outer = [
        ["this string exceeds SSO threshold by being very long indeed",
         "second string also exceeds the SSO threshold for heap alloc"],
        ["third string that is also long enough to require heap allocation"]
    ];
    let counts = for inner in outer yield inner.length();
    let total = 0;
    for n in counts do total = total + n;
    if total == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_str_nested",
    );
}

#[test]
fn test_iter_rc_for_yield_str_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "keep: passes the guard filter and is also long enough for heap",
        "third string that is also long enough to require heap allocation"
    ];
    let kept = for s in items if s.starts_with(prefix: "keep") yield s.len();
    if kept.length() == 1 then 0 else 1
}
"#,
        "iter_rc_for_yield_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_str_unwind() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "panic: this string triggers panic and is long enough for heap alloc",
        "third string that should not be visited after panic fires at all"
    ];
    let result = catch(expr: () -> {
        let _ = for s in items yield {
            if s.starts_with(prefix: "panic") then panic(msg: "test unwind yield str");
            s.len()
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_yield_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_str_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "skip: this string should be skipped via continue value substitution",
        "third string that is also long enough to require heap allocation"
    ];
    // continue value 0 substitutes for the skipped element
    let lengths = for s in items yield {
        if s.starts_with(prefix: "skip") then continue 0;
        s.len()
    };
    if lengths.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_str_continue",
    );
}

// -----------------------------------------------------------------------
// E2: [[int]] nested list — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_nested_list_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    let lens = for inner in items yield inner.length();
    let sum = 0;
    for n in lens do sum = sum + n;
    if sum == 9 then 0 else 1
}
"#,
        "iter_rc_for_yield_nested_list_full",
    );
}

#[test]
fn test_iter_rc_for_yield_nested_list_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    let result = for inner in items yield {
        if inner.length() == 3 then break;
        inner.length()
    };
    if result.length() == 0 then 0 else 1
}
"#,
        "iter_rc_for_yield_nested_list_break",
    );
}

#[test]
fn test_iter_rc_for_yield_nested_list_yield_transform() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4, 5, 6]];
    let doubled_lens = for inner in items yield inner.length() * 2;
    let sum = 0;
    for n in doubled_lens do sum = sum + n;
    if sum == 12 then 0 else 1
}
"#,
        "iter_rc_for_yield_nested_list_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_nested_list_two_call() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4, 5, 6]];
    let r1 = for inner in items yield inner.length();
    let r2 = for inner in items yield inner.length();
    if r1.length() == 2 && r2.length() == 2 then 0 else 1
}
"#,
        "iter_rc_for_yield_nested_list_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_nested_list_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4], [7, 8, 9]];
    let long_lens = for inner in items if inner.length() > 1 yield inner.length();
    if long_lens.length() == 2 then 0 else 1
}
"#,
        "iter_rc_for_yield_nested_list_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_nested_list_unwind() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    let result = catch(expr: () -> {
        let _ = for inner in items yield {
            if inner.length() == 3 then panic(msg: "test unwind nested list yield");
            inner.length()
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_yield_nested_list_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_nested_list_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [[1, 2, 3], [4], [7, 8, 9]];
    // continue 0 substitutes for single-element inner lists
    let lens = for inner in items yield {
        if inner.length() == 1 then continue 0;
        inner.length()
    };
    if lens.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_nested_list_continue",
    );
}

// -----------------------------------------------------------------------
// E3: [Option<str>] — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_option_str_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let bools = for opt in items yield is_some(option: opt);
    if bools.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_option_str_full",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let result = for opt in items yield {
        if is_none(option: opt) then break;
        1
    };
    if result.length() == 1 then 0 else 1
}
"#,
        "iter_rc_for_yield_option_str_break",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_yield_transform() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let flags = for opt in items yield if is_some(option: opt) then 1 else 0;
    let sum = 0;
    for n in flags do sum = sum + n;
    if sum == 2 then 0 else 1
}
"#,
        "iter_rc_for_yield_option_str_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_two_call() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let r1 = for opt in items yield is_some(option: opt);
    let r2 = for opt in items yield is_some(option: opt);
    if r1.length() == 3 && r2.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_option_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_nested() {
    assert_aot_success(
        r#"
@main () -> int = {
    let outer = [
        [Some("this string exceeds SSO threshold by being very long indeed"),
         None],
        [Some("third string that is also long enough to require heap alloc")]
    ];
    let counts = for inner in outer yield inner.length();
    let total = 0;
    for n in counts do total = total + n;
    if total == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_option_str_nested",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let somes = for opt in items if is_some(option: opt) yield 1;
    if somes.length() == 2 then 0 else 1
}
"#,
        "iter_rc_for_yield_option_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_option_str_unwind() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    let result = catch(expr: () -> {
        let _ = for opt in items yield {
            if is_none(option: opt) then panic(msg: "test unwind option str yield");
            1
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_yield_option_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        Some("this string exceeds SSO threshold by being very long indeed"),
        None,
        Some("third string that is also long enough to require heap allocation")
    ];
    // continue 0 for None, 1 for Some
    let flags = for opt in items yield {
        if is_none(option: opt) then continue 0;
        1
    };
    if flags.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_option_str_continue",
    );
}

// -----------------------------------------------------------------------
// E4: [closure] — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_closure_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let results = for f in fns yield f(10);
    if results.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_closure_full",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let idx = 0;
    let result = for f in fns yield {
        if idx == 1 then break;
        let r = f(10);
        idx = idx + 1;
        r
    };
    if result.length() == 1 then 0 else 1
}
"#,
        "iter_rc_for_yield_closure_break",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_yield_transform() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let results = for f in fns yield f(5) * 2;
    // (5+1)*2=12, (5*2)*2=20, (5-1)*2=8 → sum=40
    let sum = 0;
    for n in results do sum = sum + n;
    if sum == 40 then 0 else 1
}
"#,
        "iter_rc_for_yield_closure_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_two_call() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let r1 = for f in fns yield f(10);
    let r2 = for f in fns yield f(10);
    if r1.length() == 3 && r2.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_closure_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_nested() {
    assert_aot_success(
        r#"
@main () -> int = {
    let outer = [[x -> x + 1, x -> x * 2], [x -> x - 1]];
    let counts = for inner in outer yield inner.length();
    let total = 0;
    for n in counts do total = total + n;
    if total == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_closure_nested",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let idx = 0;
    let results = for f in fns yield {
        let r = f(idx);
        idx = idx + 1;
        r
    };
    // f(0)=0+1=1, f(1)=1*2=2, f(2)=2-1=1 → sum=4
    let sum = 0;
    for n in results do sum = sum + n;
    if sum == 4 then 0 else 1
}
"#,
        "iter_rc_for_yield_closure_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_closure_unwind() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let result = catch(expr: () -> {
        let idx = 0;
        let _ = for f in fns yield {
            if idx == 1 then panic(msg: "test unwind closure yield");
            let r = f(10);
            idx = idx + 1;
            r
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_yield_closure_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [x -> x + 1, x -> x * 2, x -> x - 1];
    let idx = 0;
    let results = for f in fns yield {
        if idx == 1 then {
            idx = idx + 1;
            continue 0;
        };
        let r = f(10);
        idx = idx + 1;
        r
    };
    if results.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_closure_continue",
    );
}

// -----------------------------------------------------------------------
// E5: [struct with str field] — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_struct_str_full() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 },
        Item { name: "third item name that is also long enough for heap alloc", val: 30 }
    ];
    let vals = for item in items yield item.val;
    if vals.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_struct_str_full",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_break() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "stop: this item triggers break and name is long for heap", val: 20 },
        Item { name: "third item that should not be visited after break fires", val: 30 }
    ];
    let result = for item in items yield {
        if item.name.starts_with(prefix: "stop") then break;
        item.val
    };
    if result.length() == 1 then 0 else 1
}
"#,
        "iter_rc_for_yield_struct_str_break",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_yield_transform() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 }
    ];
    let doubled = for item in items yield item.val * 2;
    let sum = 0;
    for n in doubled do sum = sum + n;
    if sum == 60 then 0 else 1
}
"#,
        "iter_rc_for_yield_struct_str_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_two_call() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 }
    ];
    let r1 = for item in items yield item.val;
    let r2 = for item in items yield item.val;
    if r1.length() == 2 && r2.length() == 2 then 0 else 1
}
"#,
        "iter_rc_for_yield_struct_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_nested() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let outer = [
        [Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
         Item { name: "second item name also exceeds the SSO threshold heap", val: 20 }],
        [Item { name: "third item name that is also long enough for heap alloc", val: 30 }]
    ];
    let counts = for inner in outer yield inner.length();
    let total = 0;
    for n in counts do total = total + n;
    if total == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_struct_str_nested",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_guard() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 },
        Item { name: "third item name that is also long enough for heap alloc", val: 30 }
    ];
    let big_vals = for item in items if item.val > 15 yield item.val;
    if big_vals.length() == 2 then 0 else 1
}
"#,
        "iter_rc_for_yield_struct_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_struct_str_unwind() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 },
        Item { name: "third item name that is also long enough for heap alloc", val: 30 }
    ];
    let result = catch(expr: () -> {
        let _ = for item in items yield {
            if item.val == 20 then panic(msg: "test unwind struct str yield");
            item.val
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_yield_struct_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_continue() {
    assert_aot_success(
        r#"
type Item = { name: str, val: int }

@main () -> int = {
    let items = [
        Item { name: "this item name exceeds the SSO threshold by being long", val: 10 },
        Item { name: "second item name also exceeds the SSO threshold for heap", val: 20 },
        Item { name: "third item name that is also long enough for heap alloc", val: 30 }
    ];
    // continue 0 substitutes for items with val == 20
    let vals = for item in items yield {
        if item.val == 20 then continue 0;
        item.val
    };
    if vals.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_struct_str_continue",
    );
}

// -----------------------------------------------------------------------
// E6: {str: int} map — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_map_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let vals = for entry in m yield entry.1;
    if vals.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_map_full",
    );
}

#[test]
fn test_iter_rc_for_yield_map_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let count = 0;
    let result = for entry in m yield {
        if count == 1 then break;
        count = count + 1;
        entry.1
    };
    if result.length() == 1 then 0 else 1
}
"#,
        "iter_rc_for_yield_map_break",
    );
}

#[test]
fn test_iter_rc_for_yield_map_yield_transform() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let doubled = for entry in m yield entry.1 * 2;
    let sum = 0;
    for n in doubled do sum = sum + n;
    if sum == 120 then 0 else 1
}
"#,
        "iter_rc_for_yield_map_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_map_two_call() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let r1 = for entry in m yield entry.1;
    let r2 = for entry in m yield entry.1;
    if r1.length() == 3 && r2.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_map_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_map_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let big = for entry in m if entry.1 > 15 yield entry.1;
    if big.length() == 2 then 0 else 1
}
"#,
        "iter_rc_for_yield_map_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_map_unwind() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    let result = catch(expr: () -> {
        let _ = for entry in m yield {
            if entry.1 == 20 then panic(msg: "test unwind map yield");
            entry.1
        };
        0
    });
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#,
        "iter_rc_for_yield_map_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_map_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20,
        "third long key string also exceeds SSO threshold needs heap too": 30
    };
    // continue 0 substitutes for entries with val == 20
    let vals = for entry in m yield {
        if entry.1 == 20 then continue 0;
        entry.1
    };
    if vals.length() == 3 then 0 else 1
}
"#,
        "iter_rc_for_yield_map_continue",
    );
}

// Edge case tests

#[test]
fn test_iter_rc_edge_empty_str_for_do() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items: [str] = [];
    let count = 0;
    for s in items do count += 1;
    if count == 0 then 0 else 1
}
"#,
        "iter_rc_edge_empty_str_for_do",
    );
}

#[test]
fn test_iter_rc_edge_empty_str_for_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items: [str] = [];
    let result = for s in items yield s;
    if result.length() == 0 then 0 else 1
}
"#,
        "iter_rc_edge_empty_str_for_yield",
    );
}

#[test]
fn test_iter_rc_edge_single_str_for_do() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = ["this string exceeds SSO threshold by being very long indeed"];
    let count = 0;
    for s in items do count += 1;
    if count == 1 then 0 else 1
}
"#,
        "iter_rc_edge_single_str_for_do",
    );
}

#[test]
fn test_iter_rc_edge_single_str_for_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = ["this string exceeds SSO threshold by being very long indeed"];
    let result = for s in items yield s;
    if result.length() == 1 then 0 else 1
}
"#,
        "iter_rc_edge_single_str_for_yield",
    );
}

#[test]
fn test_iter_rc_edge_large_str_for_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [
        "string number one exceeds SSO by being incredibly long here",
        "string number two exceeds SSO by being incredibly long here",
        "string number three exceeds SSO by being incredibly long too",
        "string number four exceeds SSO by being incredibly long here",
        "string number five exceeds SSO by being incredibly long here",
        "string number six exceeds SSO by being incredibly long there",
        "string number seven exceeds SSO by being incredibly long yep",
        "string number eight exceeds SSO by being incredibly long too",
        "string number nine exceeds SSO by being incredibly long here",
        "string number ten exceeds SSO by being incredibly long right"
    ];
    let result = for s in items yield s;
    if result.length() == 10 then 0 else 1
}
"#,
        "iter_rc_edge_large_str_for_yield",
    );
}

#[test]
fn test_iter_rc_edge_empty_map_for_do() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m: {str: int} = {};
    let count = 0;
    for (k, v) in m do count += 1;
    if count == 0 then 0 else 1
}
"#,
        "iter_rc_edge_empty_map_for_do",
    );
}
