//! RC Matrix Tests — Leak Regression Guard
//!
//! Systematically tests RC correctness across the cross-product of:
//! - 5 value types: str, [int], [str], {str: int}, struct w/ heap fields
//! - 3 loop patterns: for-range, manual loop (≈while), for+break
//! - 5 scope contexts: simple scope, if-else, match, function arg, function return
//! - 10 nested/composed patterns
//!
//! Every test uses `assert_aot_success` which enables `ORI_CHECK_LEAKS=1`.
//! Exit code 0 = correct result + zero leaks. Exit code 2 = leak detected.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ── 04.2 Value Type × Loop Pattern Matrix (15 tests) ──
//
// Each test reassigns a heap-allocated variable 30 times inside a loop.
// The old value must be RC-decremented on each reassignment.

// str × for-range
#[test]
fn test_matrix_str_for_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello_world_this_is_heap";
    for i in 0..30 do {
        s = s + "x";
    };
    // 24 original chars + 30 'x' = 54
    if s.len() == 54 then 0 else 1
}
"#,
        "matrix_str_for_loop",
    );
}

// str × manual loop (while equivalent)
#[test]
fn test_matrix_str_while_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello_world_this_is_heap";
    let i = 0;
    loop {
        if i >= 30 then break;
        s = s + "x";
        i = i + 1;
    };
    if s.len() == 54 then 0 else 1
}
"#,
        "matrix_str_while_loop",
    );
}

// str × for+break (early exit)
#[test]
fn test_matrix_str_loop_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello_world_this_is_heap";
    for i in 0..100 do {
        s = s + "x";
        if i >= 29 then break;
    };
    // 24 + 30 = 54
    if s.len() == 54 then 0 else 1
}
"#,
        "matrix_str_loop_break",
    );
}

// [int] × for-range
#[test]
fn test_matrix_list_int_for_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    for i in 0..30 do {
        xs = xs.push(i);
    };
    // 3 original + 30 pushed = 33
    if xs.len() == 33 then 0 else 1
}
"#,
        "matrix_list_int_for_loop",
    );
}

// [int] × manual loop
#[test]
fn test_matrix_list_int_while_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let i = 0;
    loop {
        if i >= 30 then break;
        xs = xs.push(i);
        i = i + 1;
    };
    if xs.len() == 33 then 0 else 1
}
"#,
        "matrix_list_int_while_loop",
    );
}

// [int] × for+break
#[test]
fn test_matrix_list_int_loop_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    for i in 0..100 do {
        xs = xs.push(i);
        if i >= 29 then break;
    };
    if xs.len() == 33 then 0 else 1
}
"#,
        "matrix_list_int_loop_break",
    );
}

// [str] × for-range
#[test]
fn test_matrix_list_str_for_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = ["hello", "world"];
    for i in 0..30 do {
        xs = xs.push("item_" + str(i));
    };
    // 2 original + 30 pushed = 32
    if xs.len() == 32 then 0 else 1
}
"#,
        "matrix_list_str_for_loop",
    );
}

// [str] × manual loop
#[test]
fn test_matrix_list_str_while_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = ["hello", "world"];
    let i = 0;
    loop {
        if i >= 30 then break;
        xs = xs.push("item_" + str(i));
        i = i + 1;
    };
    if xs.len() == 32 then 0 else 1
}
"#,
        "matrix_list_str_while_loop",
    );
}

// [str] × for+break
#[test]
fn test_matrix_list_str_loop_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = ["hello", "world"];
    for i in 0..100 do {
        xs = xs.push("item_" + str(i));
        if i >= 29 then break;
    };
    if xs.len() == 32 then 0 else 1
}
"#,
        "matrix_list_str_loop_break",
    );
}

// {str: int} × for-range
#[test]
fn test_matrix_map_for_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"init": 0};
    for i in 0..30 do {
        m = m.insert("key_" + str(i), i);
    };
    // 1 original + 30 inserted = 31
    if m.len() == 31 then 0 else 1
}
"#,
        "matrix_map_for_loop",
    );
}

// {str: int} × manual loop
#[test]
fn test_matrix_map_while_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"init": 0};
    let i = 0;
    loop {
        if i >= 30 then break;
        m = m.insert("key_" + str(i), i);
        i = i + 1;
    };
    if m.len() == 31 then 0 else 1
}
"#,
        "matrix_map_while_loop",
    );
}

// {str: int} × for+break
#[test]
fn test_matrix_map_loop_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {"init": 0};
    for i in 0..100 do {
        m = m.insert("key_" + str(i), i);
        if i >= 29 then break;
    };
    if m.len() == 31 then 0 else 1
}
"#,
        "matrix_map_loop_break",
    );
}

// struct w/ heap fields × for-range
#[test]
fn test_matrix_struct_for_loop() {
    assert_aot_success(
        r#"
type Record = { name: str, values: [int] }

@main () -> int = {
    let r = Record { name: "initial_name_for_heap", values: [1, 2, 3] };
    for i in 0..30 do {
        r = Record { name: r.name + "x", values: r.values.push(i) };
    };
    if r.values.len() == 33 then 0 else 1
}
"#,
        "matrix_struct_for_loop",
    );
}

// struct w/ heap fields × manual loop
#[test]
fn test_matrix_struct_while_loop() {
    assert_aot_success(
        r#"
type Record = { name: str, values: [int] }

@main () -> int = {
    let r = Record { name: "initial_name_for_heap", values: [1, 2, 3] };
    let i = 0;
    loop {
        if i >= 30 then break;
        r = Record { name: r.name + "x", values: r.values.push(i) };
        i = i + 1;
    };
    if r.values.len() == 33 then 0 else 1
}
"#,
        "matrix_struct_while_loop",
    );
}

// struct w/ heap fields × for+break
#[test]
fn test_matrix_struct_loop_break() {
    assert_aot_success(
        r#"
type Record = { name: str, values: [int] }

@main () -> int = {
    let r = Record { name: "initial_name_for_heap", values: [1, 2, 3] };
    for i in 0..100 do {
        r = Record { name: r.name + "x", values: r.values.push(i) };
        if i >= 29 then break;
    };
    if r.values.len() == 33 then 0 else 1
}
"#,
        "matrix_struct_loop_break",
    );
}

// ── 04.3 Value Type × Scope Pattern Matrix (25 tests) ──
//
// Each test creates a heap value in a specific scope context and verifies
// correct cleanup when the scope ends.

// str × simple scope
#[test]
fn test_matrix_str_scope() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $result = {
        let s = "a_heap_string_longer_than_sso";
        s.len()
    };
    if result == 29 then 0 else 1
}
"#,
        "matrix_str_scope",
    );
}

// str × if-else
#[test]
fn test_matrix_str_if_else() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $x = 42;
    let $s = if x > 20
        then "this_is_the_true_branch_heap"
        else "this_is_false_branch_heap_x";
    if s.len() == 28 then 0 else 1
}
"#,
        "matrix_str_if_else",
    );
}

// str × match
#[test]
fn test_matrix_str_match() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $x = Some(42);
    let $s = match x {
        Some(v) -> "matched_some_heap_value_" + str(v),
        None -> "matched_none_heap_fallback",
    };
    if s.starts_with(prefix: "matched_some") then 0 else 1
}
"#,
        "matrix_str_match",
    );
}

// str × function arg
#[test]
fn test_matrix_str_arg() {
    assert_aot_success(
        r#"
@process (s: str) -> int = s.len();

@main () -> int = {
    let $s = "a_heap_string_passed_as_argument";
    let $n = process(s: s);
    if n == 32 then 0 else 1
}
"#,
        "matrix_str_arg",
    );
}

// str × function return
#[test]
fn test_matrix_str_return() {
    assert_aot_success(
        r#"
@make_str () -> str = "a_heap_string_returned_from_fn";

@main () -> int = {
    let $s = make_str();
    if s.len() == 30 then 0 else 1
}
"#,
        "matrix_str_return",
    );
}

// [int] × simple scope
#[test]
fn test_matrix_list_scope() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $result = {
        let xs = [1, 2, 3, 4, 5];
        xs.len()
    };
    if result == 5 then 0 else 1
}
"#,
        "matrix_list_scope",
    );
}

// [int] × if-else
#[test]
fn test_matrix_list_if_else() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $x = 42;
    let $xs = if x > 20 then [1, 2, 3] else [4, 5];
    if xs.len() == 3 then 0 else 1
}
"#,
        "matrix_list_if_else",
    );
}

// [int] × match
#[test]
fn test_matrix_list_match() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $x = Some(3);
    let $xs = match x {
        Some(n) -> [1, 2, 3, 4, 5].take(count: n),
        None -> [],
    };
    if xs.len() == 3 then 0 else 1
}
"#,
        "matrix_list_match",
    );
}

// [int] × function arg
#[test]
fn test_matrix_list_arg() {
    assert_aot_success(
        r#"
@sum_list (xs: [int]) -> int = xs.fold(initial: 0, op: (acc, x) -> acc + x);

@main () -> int = {
    let $xs = [10, 20, 30];
    let $total = sum_list(xs: xs);
    if total == 60 then 0 else 1
}
"#,
        "matrix_list_arg",
    );
}

// [int] × function return
#[test]
fn test_matrix_list_return() {
    assert_aot_success(
        r#"
@make_list () -> [int] = [10, 20, 30, 40, 50];

@main () -> int = {
    let $xs = make_list();
    if xs.len() == 5 then 0 else 1
}
"#,
        "matrix_list_return",
    );
}

// [str] × simple scope
#[test]
fn test_matrix_list_str_scope() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $result = {
        let xs = ["hello_world_heap_str", "another_heap_string_x"];
        xs.len()
    };
    if result == 2 then 0 else 1
}
"#,
        "matrix_list_str_scope",
    );
}

// [str] × if-else
#[test]
fn test_matrix_list_str_if_else() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $x = 42;
    let $xs = if x > 20
        then ["heap_string_one_xxxxxxx", "heap_string_two_xxxxxxx"]
        else ["fallback_heap_string_xx"];
    if xs.len() == 2 then 0 else 1
}
"#,
        "matrix_list_str_if_else",
    );
}

// [str] × match
#[test]
fn test_matrix_list_str_match() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $x: Result<int, str> = Ok(42);
    let $xs = match x {
        Ok(_) -> ["ok_heap_string_value_xx", "ok_heap_second_string_x"],
        Err(_) -> ["err_heap_string_value_x"],
    };
    if xs.len() == 2 then 0 else 1
}
"#,
        "matrix_list_str_match",
    );
}

// [str] × function arg
#[test]
fn test_matrix_list_str_arg() {
    assert_aot_success(
        r#"
@count_long (xs: [str]) -> int = {
    let count = 0;
    for s in xs do {
        if s.len() > 10 then count = count + 1;
    };
    count
};

@main () -> int = {
    let $xs = ["short", "a_longer_heap_string_yes"];
    let $n = count_long(xs: xs);
    if n == 1 then 0 else 1
}
"#,
        "matrix_list_str_arg",
    );
}

// [str] × function return
#[test]
fn test_matrix_list_str_return() {
    assert_aot_success(
        r#"
@make_strs () -> [str] = ["returned_heap_string_a", "returned_heap_string_b"];

@main () -> int = {
    let $xs = make_strs();
    if xs.len() == 2 then 0 else 1
}
"#,
        "matrix_list_str_return",
    );
}

// {str: int} × simple scope
#[test]
fn test_matrix_map_scope() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $result = {
        let m = {"alpha": 1, "beta": 2, "gamma": 3};
        m.len()
    };
    if result == 3 then 0 else 1
}
"#,
        "matrix_map_scope",
    );
}

// {str: int} × if-else
#[test]
fn test_matrix_map_if_else() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $x = 42;
    let $m = if x > 20 then {"yes": 1} else {"no": 0};
    if m.len() == 1 then 0 else 1
}
"#,
        "matrix_map_if_else",
    );
}

// {str: int} × match
#[test]
fn test_matrix_map_match() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $x = Some(5);
    let $m = match x {
        Some(v) -> {"value": v, "doubled": v * 2},
        None -> {"value": 0},
    };
    if m.len() == 2 then 0 else 1
}
"#,
        "matrix_map_match",
    );
}

// {str: int} × function arg
#[test]
fn test_matrix_map_arg() {
    assert_aot_success(
        r#"
@count_entries (m: {str: int}) -> int = m.len();

@main () -> int = {
    let $m = {"a": 1, "b": 2, "c": 3};
    let $n = count_entries(m: m);
    if n == 3 then 0 else 1
}
"#,
        "matrix_map_arg",
    );
}

// {str: int} × function return
#[test]
fn test_matrix_map_return() {
    assert_aot_success(
        r#"
@make_map () -> {str: int} = {"x": 10, "y": 20, "z": 30};

@main () -> int = {
    let $m = make_map();
    if m.len() == 3 then 0 else 1
}
"#,
        "matrix_map_return",
    );
}

// struct w/ heap × simple scope
#[test]
fn test_matrix_struct_scope() {
    assert_aot_success(
        r#"
type Item = { name: str, tags: [int] }

@main () -> int = {
    let $result = {
        let item = Item { name: "scoped_heap_item_name_x", tags: [1, 2, 3] };
        item.tags.len()
    };
    if result == 3 then 0 else 1
}
"#,
        "matrix_struct_scope",
    );
}

// struct w/ heap × if-else
#[test]
fn test_matrix_struct_if_else() {
    assert_aot_success(
        r#"
type Item = { name: str, count: int }

@main () -> int = {
    let $x = 42;
    let $item = if x > 20
        then Item { name: "true_branch_heap_name_x", count: x }
        else Item { name: "false_branch_heap_name_", count: 0 };
    if item.count == 42 then 0 else 1
}
"#,
        "matrix_struct_if_else",
    );
}

// struct w/ heap × match
#[test]
fn test_matrix_struct_match() {
    assert_aot_success(
        r#"
type Item = { label: str, value: int }

@main () -> int = {
    let $x: Result<int, str> = Ok(7);
    let $item = match x {
        Ok(v) -> Item { label: "ok_label_heap_string_xx", value: v },
        Err(_) -> Item { label: "err_label_heap_string_x", value: -1 },
    };
    if item.value == 7 then 0 else 1
}
"#,
        "matrix_struct_match",
    );
}

// struct w/ heap × function arg
#[test]
fn test_matrix_struct_arg() {
    assert_aot_success(
        r#"
type Item = { name: str, score: int }

@get_score (item: Item) -> int = item.score;

@main () -> int = {
    let $item = Item { name: "passed_as_arg_heap_name", score: 99 };
    let $s = get_score(item: item);
    if s == 99 then 0 else 1
}
"#,
        "matrix_struct_arg",
    );
}

// struct w/ heap × function return
#[test]
fn test_matrix_struct_return() {
    assert_aot_success(
        r#"
type Item = { name: str, score: int }

@make_item () -> Item = Item { name: "returned_item_heap_name", score: 42 };

@main () -> int = {
    let $item = make_item();
    if item.score == 42 then 0 else 1
}
"#,
        "matrix_struct_return",
    );
}

// ── 04.4 Nested & Composed Pattern Matrix (10 tests) ──
//
// Tests combinations that compose multiple dimensions — highest risk.

// Struct containing [int] reassigned in loop
#[test]
fn test_matrix_struct_with_list_in_loop() {
    assert_aot_success(
        r#"
type Bundle = { label: str, items: [int] }

@main () -> int = {
    let b = Bundle { label: "bundle_label_heap_str_x", items: [1] };
    for i in 0..30 do {
        b = Bundle { label: b.label, items: b.items.push(i) };
    };
    // 1 original + 30 pushed = 31
    if b.items.len() == 31 then 0 else 1
}
"#,
        "matrix_struct_with_list_in_loop",
    );
}

// [str] with push in loop (nested RC: list + string elements)
#[test]
fn test_matrix_list_of_strings_in_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = ["initial_heap_string_val"];
    for i in 0..30 do {
        xs = xs.push("item_number_" + str(i) + "_heap");
    };
    // 1 original + 30 pushed = 31
    if xs.len() == 31 then 0 else 1
}
"#,
        "matrix_list_of_strings_in_loop",
    );
}

// String conditionally updated in loop
#[test]
fn test_matrix_string_in_if_else_in_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "start_heap_string_value";
    for i in 0..30 do {
        s = if i % 2 == 0
            then s + "e"
            else s + "o";
    };
    // 23 + 30 = 53
    if s.len() == 53 then 0 else 1
}
"#,
        "matrix_string_in_if_else_in_loop",
    );
}

// Create slice, use, let both slice and original drop
#[test]
fn test_matrix_slice_in_scope() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $xs = [10, 20, 30, 40, 50];
    let $slice = xs.slice(1, 4);
    let $sum = slice.fold(initial: 0, op: (acc, x) -> acc + x);
    // slice = [20, 30, 40], sum = 90
    if sum == 90 then 0 else 1
}
"#,
        "matrix_slice_in_scope",
    );
}

// Create slices in a loop
#[test]
fn test_matrix_slice_in_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $xs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let total = 0;
    for i in 0..5 do {
        let $slice = xs.slice(0, i + 1);
        total = total + slice.len();
    };
    // lengths: 1+2+3+4+5 = 15
    if total == 15 then 0 else 1
}
"#,
        "matrix_slice_in_loop",
    );
}

// Multiple independent heap variables in one scope
#[test]
fn test_matrix_multiple_heap_locals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $s = "a_heap_string_for_testing";
    let $xs = [1, 2, 3, 4, 5];
    let $m = {"key": 42};
    let $sum = s.len() + xs.len() + m.len();
    // 25 + 5 + 1 = 31
    if sum == 31 then 0 else 1
}
"#,
        "matrix_multiple_heap_locals",
    );
}

// Shadow a heap variable with a new heap value
#[test]
fn test_matrix_heap_var_shadowing() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $s = "first_heap_string_shadow";
    let $n = s.len();
    let $s = "second_heap_string_value_shadow";
    let $m = s.len();
    if n == 24 && m == 31 then 0 else 1
}
"#,
        "matrix_heap_var_shadowing",
    );
}

// Lambda capturing a heap string, called, then dropped
#[test]
fn test_matrix_closure_captures_string() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $s = "captured_heap_string_val";
    let $f = () -> int = s.len();
    let $result = f();
    if result == 24 then 0 else 1
}
"#,
        "matrix_closure_captures_string",
    );
}

// Lambda capturing a [int], called, then dropped
#[test]
fn test_matrix_closure_captures_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $xs = [10, 20, 30, 40, 50];
    let $f = () -> int = xs.len();
    let $result = f();
    if result == 5 then 0 else 1
}
"#,
        "matrix_closure_captures_list",
    );
}

// Lambda created inside loop body capturing loop variable
#[test]
fn test_matrix_closure_in_loop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for i in 0..10 do {
        let $s = "loop_capture_heap_str_" + str(i);
        let $f = () -> int = s.len();
        total = total + f();
    };
    // "loop_capture_heap_str_" is 22 chars + 1 digit = 23 per iteration
    // 10 iterations × 23 = 230
    if total == 230 then 0 else 1
}
"#,
        "matrix_closure_in_loop",
    );
}
