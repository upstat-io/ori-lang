//! For-yield tests — F3 (transform), F10 (identity yield), break/continue in yield.
//!
//! Yield identity: the loop variable `w` is borrowed from the iterator. When yielded
//! directly (not transformed to a scalar), the element escapes the iterator's borrow
//! scope. The ARC pipeline must emit `RcInc` on `w` before passing it to `ori_list_push`.

use crate::util::assert_aot_success;

// Yield identity — for w in words yield w

#[test]
fn test_yield_identity_str_list() {
    // `for w in words yield w` — yields the actual string element
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let copy = for w in words yield w;
    let total = 0;
    for w in copy do {
        total = total + w.len();
    };
    if total == 109 then 0 else 1
}
"#,
        "yield_identity_str_list",
    );
}

#[test]
fn test_yield_identity_str_list_borrowed_param() {
    // Borrowed param + yield identity: the borrowed [str] is iterated
    // and each element is yielded into a new list.
    assert_aot_success(
        r#"
@clone_list (words: [str]) -> [str] = {
    for w in words yield w
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let copy = clone_list(words: words);
    let total = 0;
    for w in copy do {
        total = total + w.len();
    };
    // Verify original is still valid
    let total2 = 0;
    for w in words do {
        total2 = total2 + w.len();
    };
    if total == 109 && total2 == 109 then 0 else 1
}
"#,
        "yield_identity_str_list_borrowed_param",
    );
}

#[test]
fn test_yield_identity_str_list_two_calls() {
    // Two calls to a function that does yield identity on borrowed [str].
    // Stresses RC: original must survive both clones.
    assert_aot_success(
        r#"
@clone_list (words: [str]) -> [str] = {
    for w in words yield w
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = clone_list(words: words);
    let b = clone_list(words: words);
    let sa = 0;
    for w in a do { sa = sa + w.len(); };
    let sb = 0;
    for w in b do { sb = sb + w.len(); };
    if sa == 109 && sb == 109 then 0 else 1
}
"#,
        "yield_identity_str_list_two_calls",
    );
}

// For-yield with non-scalar elements — elem_dec_fn correctness

/// `[str]` for-yield identity — borrowed str elements yielded into new list.
/// Verifies `elem_dec_fn` correctly cleans up source list elements when the
/// iterator drops, while the result list owns its own copies.
#[test]
fn test_for_yield_str_identity() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let copied = for w in words yield w;
    let expected = 53 + 56;
    if copied[0].length() + copied[1].length() == expected then 0 else 1
}
"#,
        "for_yield_str_identity",
    );
}

/// [str] for-yield with scalar transformation — str elements borrowed,
/// lengths (int) yielded. Verifies no leak on source str elements.
#[test]
fn test_for_yield_str_to_lengths() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let lengths = for w in words yield w.length();
    if lengths[0] == 53 && lengths[1] == 56 then 0 else 1
}
"#,
        "for_yield_str_to_lengths",
    );
}

/// `[[int]]` for-yield — nested list elements borrowed from outer list,
/// inner sums yielded as scalars. Verifies `elem_dec_fn` on nested `[int]`.
#[test]
fn test_for_yield_nested_list() {
    assert_aot_success(
        r#"
@sum_list (xs: [int]) -> int = {
    let total = 0;
    for x in xs do {
        total = total + x;
    };
    total
}

@main () -> int = {
    let lists = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    let sums = for l in lists yield sum_list(l);
    if sums[0] == 6 && sums[1] == 15 && sums[2] == 24 then 0 else 1
}
"#,
        "for_yield_nested_list",
    );
}

/// `[Option<str>]` for-yield with match — `Option<str>` elements borrowed,
/// pattern-matched to extract lengths. Verifies `elem_dec_fn` on `Option<str>`.
#[test]
fn test_for_yield_option_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items: [Option<str>] = [
        Some("this is a very long string that exceeds SSO threshold"),
        None,
        Some("another very long string exceeding threshold here")
    ];
    let lengths = for item in items yield match item {
        Some(s) -> s.length(),
        None -> 0,
    };
    if lengths[0] == 53 && lengths[1] == 0 && lengths[2] == 49 then 0 else 1
}
"#,
        "for_yield_option_str",
    );
}

// For-yield mutable variable threading (TPR-02-002 regression)

/// Outer mutable variable mutation inside for-yield body — the body
/// assigns to `sum` which is declared outside the for-yield. Verifies
/// that the assignment is correctly propagated through the loop's SSA
/// block parameters. Regression test for TPR-02-002 where
/// `clear_mutable_names()` silently dropped the assignment in AOT.
#[test]
fn test_for_yield_outer_mutable_mutation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    let result = for x in [10, 20, 30] yield {
        sum = sum + x;
        x
    };
    // sum should be 60, result should be [10, 20, 30]
    if sum == 60 && result[0] == 10 && result[2] == 30 then 0 else 1
}
"#,
        "for_yield_outer_mutable_mutation",
    );
}

/// Nested for-do inside for-yield body — inner loop mutates an outer
/// variable. Verifies mutable threading works with nested control flow.
#[test]
fn test_for_yield_nested_for_do_mutation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    let result = for x in [1, 2, 3] yield {
        for y in [10, 20] do {
            total = total + y
        };
        total
    };
    // Each iteration adds 30 to total: [30, 60, 90]
    // total ends at 90
    if total == 90 && result[0] == 30 && result[2] == 90 then 0 else 1
}
"#,
        "for_yield_nested_for_do_mutation",
    );
}

/// For-yield with str elements and outer mutable counter — combines
/// fat pointer iteration with mutable variable threading and leak check.
#[test]
fn test_for_yield_str_with_mutable_counter() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    let lengths = for s in ["this is a very long string that exceeds SSO threshold", "another very long exceeding SSO"] yield {
        count = count + 1;
        s.length()
    };
    if count == 2 && lengths[0] == 53 && lengths[1] == 31 then 0 else 1
}
"#,
        "for_yield_str_with_mutable_counter",
    );
}

// For-yield RC balance tests (Section 03.4)

/// For-yield with closure elements — closures applied to argument in body.
#[test]
fn test_for_yield_closure_elements() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fns = [
        x -> x + 1,
        x -> x * 2,
        x -> x - 3
    ];
    let results = for f in fns yield f(10);
    if results[0] == 11 && results[1] == 20 && results[2] == 7 then 0 else 1
}
"#,
        "for_yield_closure_elements",
    );
}

/// For-yield with struct elements containing str fields — struct field
/// access in body, verifies element RC through struct aggregates.
#[test]
fn test_for_yield_struct_elements() {
    assert_aot_success(
        r#"
type Item = { name: str }

@main () -> int = {
    let items = [
        Item { name: "this is a very long name that exceeds SSO threshold" },
        Item { name: "another long name exceeding SSO for sure here" }
    ];
    let lengths = for item in items yield item.name.length();
    if lengths[0] == 51 && lengths[1] == 45 then 0 else 1
}
"#,
        "for_yield_struct_elements",
    );
}

/// For-yield with guard on `[str]` — filters short strings, yields only
/// long ones. Verifies `guard_skip` path threads mutable params correctly.
#[test]
fn test_for_yield_guard_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "short",
        "this is a very long string that exceeds SSO threshold",
        "hi",
        "another very long string exceeding SSO for sure here"
    ];
    let long_lengths = for w in words if w.length() > 10 yield w.length();
    if long_lengths[0] == 53 && long_lengths[1] == 52 then 0 else 1
}
"#,
        "for_yield_guard_str",
    );
}

/// For-yield on `[[str]]` — nested list of str lists. Verifies element
/// cleanup for nested fat pointer types.
#[test]
fn test_for_yield_nested_str_loops() {
    assert_aot_success(
        r#"
@main () -> int = {
    let lists = [
        ["this is a very long string exceeding SSO threshold here", "short"],
        ["another very long string that exceeds SSO"]
    ];
    let counts = for sublist in lists yield sublist.length();
    if counts[0] == 2 && counts[1] == 1 then 0 else 1
}
"#,
        "for_yield_nested_str_loops",
    );
}

/// For-yield on empty `[str]` — zero iterations, empty result list.
/// Verifies no leak from allocated-but-empty growable list.
#[test]
fn test_for_yield_empty_str_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words: [str] = [];
    let result = for w in words yield w.length();
    if result.length() == 0 then 0 else 1
}
"#,
        "for_yield_empty_str_list",
    );
}

// For-yield break/continue lowering

/// `break` in for-yield: stop early, return accumulated list.
#[test]
fn test_for_yield_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = for x in [1, 2, 3, 4, 5] yield {
        if x == 4 then break;
        x * 2
    };
    // Expected: [2, 4, 6] (x=1→2, x=2→4, x=3→6, x=4→break)
    if result.length() == 3 then 0 else 1
}
"#,
        "for_yield_break",
    );
}

/// `break value` in for-yield: push value then stop.
#[test]
fn test_for_yield_break_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = for x in [1, 2, 3, 4, 5] yield {
        if x == 4 then break 99;
        x
    };
    // Expected: [1, 2, 3, 99] (x=4→break 99 pushes 99)
    let ok = result.length() == 4;
    if ok then {
        // Verify last element is 99
        ok = result[3] == 99
    };
    if ok then 0 else 1
}
"#,
        "for_yield_break_value",
    );
}

/// `continue` in for-yield: skip element, don't push.
#[test]
fn test_for_yield_continue() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = for x in [1, 2, 3, 4, 5] yield {
        if x == 3 then continue;
        x
    };
    // Expected: [1, 2, 4, 5] (x=3 skipped)
    if result.length() == 4 then 0 else 1
}
"#,
        "for_yield_continue",
    );
}

/// `continue value` in for-yield: push substituted value.
#[test]
fn test_for_yield_continue_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = for x in [1, 2, 3, 4, 5] yield {
        if x == 3 then continue 0;
        x
    };
    // Expected: [1, 2, 0, 4, 5] (x=3→continue 0 pushes 0 instead)
    let ok = result.length() == 5;
    if ok then {
        ok = result[2] == 0
    };
    if ok then 0 else 1
}
"#,
        "for_yield_continue_value",
    );
}

/// `break` in for-yield over str list: RC correctness with fat pointers.
#[test]
fn test_for_yield_break_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold",
        "third string for good measure and beyond SSO"
    ];
    let result = for w in words yield {
        if w.length() > 55 then break;
        w
    };
    // Expected: ["this is a very long string..."] — first string (53 chars) passes,
    // second (56 chars) triggers break
    if result.length() == 1 then 0 else 1
}
"#,
        "for_yield_break_str",
    );
}

/// `continue` in for-yield over str list: skip without leaking.
#[test]
fn test_for_yield_continue_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "short",
        "another very long string that also exceeds the threshold"
    ];
    let result = for w in words yield {
        if w.length() < 10 then continue;
        w
    };
    // Expected: 2 elements (both long strings, "short" skipped)
    if result.length() == 2 then 0 else 1
}
"#,
        "for_yield_continue_str",
    );
}

/// `break` + mutable var: mutable variable threading preserved across break.
#[test]
fn test_for_yield_break_mutable() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    let result = for x in [10, 20, 30, 40, 50] yield {
        count = count + 1;
        if x == 30 then break;
        x
    };
    // result = [10, 20], count = 3 (incremented before break check)
    let ok = result.length() == 2;
    if ok then {
        ok = count == 3
    };
    if ok then 0 else 1
}
"#,
        "for_yield_break_mutable",
    );
}

/// `continue value` + mutable var: mutable variable threading through continue.
#[test]
fn test_for_yield_continue_value_mutable() {
    assert_aot_success(
        r#"
@main () -> int = {
    let skipped = 0;
    let result = for x in [1, 2, 3, 4, 5] yield {
        if x % 2 == 0 then {
            skipped = skipped + 1;
            continue 0
        };
        x
    };
    // result = [1, 0, 3, 0, 5], skipped = 2
    let ok = result.length() == 5;
    if ok then {
        ok = skipped == 2
    };
    if ok then 0 else 1
}
"#,
        "for_yield_continue_value_mutable",
    );
}
