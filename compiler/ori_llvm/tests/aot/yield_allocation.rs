//! Allocation-plan pins for bounded `for .. yield` comprehensions.
//!
//! These tests deliberately inspect realized function IR as well as executing
//! cleanup-sensitive fixtures. Output-only tests cannot distinguish the old
//! repeated-growth heap path from exact capacity or bounded local storage.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in embedded Ori programs"
)]

use crate::util::{
    assert_aot_success, compile_and_capture_ir, compile_and_run_capture, extract_function_ir,
};

fn function_ir(source: &str, function: &str) -> String {
    let ir = compile_and_capture_ir(source);
    extract_function_ir(&ir, function).to_owned()
}

fn assert_exact_heap_capacity(source: &str, function: &str, capacity: u64, elem_size: u64) {
    let ir = function_ir(source, function);
    let expected = format!("call ptr @ori_list_new(i64 {capacity}, i64 {elem_size})");
    assert!(
        ir.contains(&expected),
        "expected exact-capacity managed yield allocation `{expected}`:\n{ir}"
    );
    assert_eq!(
        ir.matches("call ptr @ori_list_new").count(),
        1,
        "escaping fixture must retain exactly one managed yield allocation:\n{ir}"
    );
}

fn assert_bounded_local(source: &str, function: &str) {
    let ir = function_ir(source, function);
    assert!(
        !ir.contains("call ptr @ori_list_new"),
        "proven local static yield must not use the general heap builder:\n{ir}"
    );
    assert!(
        !ir.contains("call ptr @ori_list_alloc_data"),
        "proven local static yield must not allocate list backing on the general heap:\n{ir}"
    );
    assert!(
        ir.contains("%yield.local.builder = alloca"),
        "proven local static yield must expose a bounded local builder slot:\n{ir}"
    );
    assert!(
        ir.contains("%yield.local.data = alloca"),
        "proven local static yield must expose bounded local element storage:\n{ir}"
    );
}

#[test]
fn returned_exclusive_range_reserves_exact_capacity() {
    assert_exact_heap_capacity(
        r#"
@make () -> [int] = for i in 0..100 yield i;
@main () -> int = make().length();
"#,
        "_ori_make",
        100,
        8,
    );
}

#[test]
fn returned_inclusive_step_range_reserves_exact_capacity() {
    let source = r#"
@make () -> [int] = for i in 0..=10 by 2 yield i;
@main () -> int = if make().length() == 6 then 0 else 1;
"#;
    assert_exact_heap_capacity(source, "_ori_make", 6, 8);
    assert_aot_success(source, "yield_extent_inclusive_step");
}

#[test]
fn returned_descending_range_reserves_exact_capacity() {
    let source = r#"
@make () -> [int] = for i in 10..0 by -2 yield i;
@main () -> int = if make().length() == 5 then 0 else 1;
"#;
    assert_exact_heap_capacity(source, "_ori_make", 5, 8);
    assert_aot_success(source, "yield_extent_descending");
}

#[test]
fn returned_empty_range_reserves_zero_capacity() {
    let source = r#"
@make () -> [int] = for i in 5..0 yield i;
@main () -> int = if make().length() == 0 then 0 else 1;
"#;
    assert_exact_heap_capacity(source, "_ori_make", 0, 8);
    assert_aot_success(source, "yield_extent_empty");
}

#[test]
fn guarded_range_reserves_safe_upper_bound() {
    let source = r#"
@make () -> [int] = for i in 0..100 if i % 2 == 0 yield i;
@main () -> int = if make().length() == 50 then 0 else 1;
"#;
    assert_exact_heap_capacity(source, "_ori_make", 100, 8);
    assert_aot_success(source, "yield_extent_guard_upper_bound");
}

#[test]
fn returned_option_yield_reserves_one_element() {
    let source = r#"
@make () -> [int] = for i in Some(5) yield i * 2;
@main () -> int = if make().length() == 1 then 0 else 1;
"#;
    assert_exact_heap_capacity(source, "_ori_make", 1, 8);
    assert_aot_success(source, "yield_extent_option_one");
}

#[test]
fn dynamic_range_capacity_is_not_the_literal_fallback() {
    let source = r#"
@make (end: int, step: int) -> [int] = for i in 0..end by step yield i;
@main () -> int = if make(end: 11, step: 2).length() == 6 then 0 else 1;
"#;
    let ir = function_ir(source, "_ori_make");
    assert!(
        ir.contains("call ptr @ori_list_new"),
        "runtime-sized escaping yield must retain managed allocation:\n{ir}"
    );
    assert!(
        !ir.contains("call ptr @ori_list_new(i64 8,"),
        "runtime-bounded range must derive capacity instead of using literal 8:\n{ir}"
    );
    assert_aot_success(source, "yield_extent_dynamic");
}

#[test]
fn local_static_bool_yield_uses_bounded_local_storage() {
    let source = r#"
@local_doors () -> int = {
    let doors = for _ in 0..100 yield false;
    doors[3] = true;
    if doors[3] then doors.length() else 0
}
@main () -> int = if local_doors() == 100 then 0 else 1;
"#;
    assert_bounded_local(source, "_ori_local_doors");
    assert_aot_success(source, "yield_local_static_bool");
}

#[test]
fn returned_static_yield_remains_exact_managed_storage() {
    let source = r#"
@make () -> [int] = for i in 0..100 yield i;
@main () -> int = if make().length() == 100 then 0 else 1;
"#;
    assert_exact_heap_capacity(source, "_ori_make", 100, 8);
    assert_aot_success(source, "yield_returned_must_not_stack");
}

#[test]
fn oversized_static_yield_remains_managed_storage() {
    let source = r#"
@make () -> [int] = for i in 0..10000 yield i;
@main () -> int = if make().length() == 10000 then 0 else 1;
"#;
    let ir = function_ir(source, "_ori_make");
    assert!(
        ir.contains("call ptr @ori_list_new"),
        "oversized static yield must retain managed storage:\n{ir}"
    );
    assert!(
        !ir.contains("[10000 x i64]"),
        "oversized static yield must not create a giant stack slot:\n{ir}"
    );
    assert!(
        !ir.contains("call ptr @ori_list_new(i64 8,"),
        "managed oversized yield must not use the old literal fallback:\n{ir}"
    );
    assert_aot_success(source, "yield_oversized_must_not_stack");
}

#[test]
fn production_simulate_has_one_exact_heap_yield_and_local_doors() {
    let source = include_str!("../../../../tests/run-pass/rosetta/001_100_doors/ori/100_doors.ori");
    let ir = function_ir(source, "_ori_simulate");
    assert_eq!(
        ir.matches("call ptr @ori_list_new").count(),
        1,
        "simulate must heap-allocate only its escaping result, not local doors:\n{ir}"
    );
    assert!(
        ir.contains("call ptr @ori_list_new(i64 100, i64 8)"),
        "escaping guarded result must reserve the Range upper bound 100:\n{ir}"
    );
    assert!(
        !ir.contains("call ptr @ori_list_alloc_data"),
        "simulate must not directly allocate local doors backing on the general heap:\n{ir}"
    );
    assert!(
        ir.contains("%yield.local.builder = alloca"),
        "simulate must expose the local doors builder slot:\n{ir}"
    );
    assert!(
        ir.contains("%yield.local.data = alloca"),
        "simulate must expose bounded local doors storage:\n{ir}"
    );
}

#[test]
fn local_string_yield_with_break_uses_local_storage_and_stays_leak_clean() {
    let source = r#"
@local_words () -> int = {
    let words = for i in 0..4 yield {
        if i == 2 then break;
        "this string is deliberately longer than the SSO threshold"
    };
    words.length()
}
@main () -> int = if local_words() == 2 then 0 else 1;
"#;
    assert_bounded_local(source, "_ori_local_words");
    assert_aot_success(source, "yield_local_string_break_cleanup");
}

#[test]
fn local_droppable_elements_run_user_drop_once_each() {
    let source = r#"
type Logged = { value: int }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.value}`);
}

@local_values () -> int = {
    let values = for i in 0..3 yield Logged { value: i };
    values.length()
}

@main () -> int = if local_values() == 3 then 0 else 1;
"#;
    assert_bounded_local(source, "_ori_local_values");
    let (exit, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit, 0, "droppable local yield failed: {stderr}");
    for expected in ["drop-0", "drop-1", "drop-2"] {
        assert_eq!(
            stdout.matches(expected).count(),
            1,
            "{expected} must run exactly once; stdout={stdout:?}"
        );
    }
}

#[test]
fn local_droppable_yield_unwind_is_local_and_leak_clean() {
    let source = r#"
type Item = { name: str, value: int }

@build_then_panic () -> void = {
    let _ = for i in 0..3 yield {
        if i == 2 then panic(msg: "intentional yield unwind");
        Item {
            name: "this item name exceeds the SSO threshold for heap storage",
            value: i,
        }
    }
}

@main () -> int = {
    let result = catch(expr: build_then_panic());
    match result {
        Ok(_) -> 1,
        Err(_) -> 0,
    }
}
"#;
    assert_bounded_local(source, "_ori_build_then_panic");
    assert_aot_success(source, "yield_local_unwind_cleanup");
}
