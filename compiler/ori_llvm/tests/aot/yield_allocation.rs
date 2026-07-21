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
fn registered_list_set_alias_preserves_compact_local_storage() {
    let source = r#"
@local_doors () -> int = {
    let doors = for _ in 0..4 yield false;
    let changed = doors.set(index: 1, value: true);
    if changed[1] then changed.length() else 0
}
@main () -> int = if local_doors() == 4 then 0 else 1;
"#;
    assert_bounded_local(source, "_ori_local_doors");
    assert_aot_success(source, "yield_local_registered_list_set");
}

#[test]
fn compact_local_updated_oob_uses_storage_independent_panic() {
    let source = r#"
@compact_oob () -> int = {
    let doors = for _ in 0..4 yield false;
    doors[4] = true;
    0
}
@main () -> int = compact_oob();
"#;
    let ir = function_ir(source, "_ori_compact_oob");
    assert!(
        ir.contains("%yield.local.data = alloca [4 x i8]"),
        "fixture must exercise compact headerless backing:\n{ir}"
    );
    assert!(
        ir.contains("call void @ori_panic_index_out_of_bounds"),
        "compact OOB must avoid the header-reading COW runtime:\n{ir}"
    );
    assert!(
        !ir.contains("call void @ori_list_updated_cow"),
        "compact OOB must not pass headerless data to general COW cleanup:\n{ir}"
    );

    let (exit_code, _stdout, stderr) = compile_and_run_capture(source);
    assert_ne!(exit_code, 0, "OOB update must panic");
    assert!(
        stderr.contains("index out of bounds: index 4, length 4"),
        "compact OOB must preserve the actionable bounds diagnostic:\n{stderr}"
    );
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
fn jump_threaded_returned_option_yield_remains_managed() {
    let source = r#"
@make () -> [int] = {
    loop {
        let opt = Some(5);
        let values = for x in opt if (if x == 5 then break else true) yield x * 10;
        break values
    }
}
@main () -> int = if make().length() == 0 then 0 else 1;
"#;
    assert_exact_heap_capacity(source, "_ori_make", 1, 8);
    assert_aot_success(source, "yield_jump_threaded_return_must_not_stack");
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
fn production_length_observer_uses_private_count_projection() {
    let source = include_str!("../../../../tests/run-pass/rosetta/001_100_doors/ori/100_doors.ori");
    let ir = compile_and_capture_ir(source);
    let clone = extract_function_ir(&ir, "_ori_simulate$24length_only");
    assert!(
        clone.starts_with("define internal fastcc"),
        "length projection must remain module-local:\n{clone}"
    );
    assert!(
        clone.contains("%yield.length_only.count = alloca i32"),
        "small bounded length projection must use a vectorizable i32 count:\n{clone}"
    );
    assert!(
        !clone.contains("call ptr @ori_list_new"),
        "length-only projection must not materialize its returned list:\n{clone}"
    );
    assert!(
        !clone.contains("yield.length_only.push.has_capacity"),
        "header-only count projection must not retain a memory-capacity branch:\n{clone}"
    );
    assert!(
        !clone.contains("call void @ori_buffer_drop_unique"),
        "trivial scalar local storage must not retain a runtime buffer teardown:\n{clone}"
    );
    assert!(
        clone.contains("%yield.local.data = alloca [100 x i8]"),
        "closed primitive-scalar lineage must use compact element-only backing:\n{clone}"
    );

    let main = extract_function_ir(&ir, "_ori_main");
    assert!(
        main.contains("@\"_ori_simulate$24length_only\""),
        "qualified length-only call site must target the private clone:\n{main}"
    );
    assert!(
        !main.contains("call fastcc void @_ori_simulate("),
        "length-only call site must not materialize the ordinary result:\n{main}"
    );
}

#[test]
fn non_length_result_use_fails_closed_to_ordinary_callee() {
    let source = r#"
@make () -> [int] = for i in 0..4 yield i + 1;
@main () -> int = {
    let values = make();
    values.length() + values[0]
}
"#;
    let ir = compile_and_capture_ir(source);
    assert!(
        !ir.contains("_ori_make$24length_only"),
        "an indexed result must not qualify for the length-only clone:\n{ir}"
    );
    let main = extract_function_ir(&ir, "_ori_main");
    assert!(
        main.contains("call fastcc void @_ori_make("),
        "non-length result use must retain the ordinary managed callee:\n{main}"
    );
}

#[test]
fn element_reading_observer_fails_closed_to_ordinary_callee() {
    let source = r#"
@observe (values: [int]) -> int = {
    let first = values[0];
    values.length()
}
@make () -> [int] = for i in 0..4 yield i + 1;
@main () -> int = observe(values: make());
"#;
    let ir = compile_and_capture_ir(source);
    assert!(
        !ir.contains("_ori_make$24length_only"),
        "an observer that reads an element must not qualify for the length-only clone:\n{ir}"
    );
    let main = extract_function_ir(&ir, "_ori_main");
    assert!(
        main.contains("call fastcc void @_ori_make("),
        "element-reading observer must retain the ordinary managed callee:\n{main}"
    );
}

#[test]
fn production_length_projection_preserves_checksum() {
    let source = include_str!("../../../../tests/run-pass/rosetta/001_100_doors/ori/100_doors.ori");
    let (exit, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit, 0, "projected production workload failed: {stderr}");
    assert_eq!(stdout.trim(), "540000");
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

#[test]
fn nested_range_yield_reentrant_inner_allocation_stays_managed() {
    let source = r#"
@exercise (salt: int) -> int = {
    let acc = 0;
    let rows = for i in 0..10 yield (for j in 0..10 yield i * j);
    for row in rows do { for value in row do { acc = acc + value } };
    acc + (salt % 100)
}

@main () -> int = {
    let total = 0;
    for n in 0..100 do { total = total + exercise(salt: n) };
    if total == 207450 then 0 else 1
}
"#;
    let ir = function_ir(source, "_ori_exercise");
    assert_eq!(
        ir.matches("call ptr @ori_list_new(i64 10, i64 8)").count(),
        1,
        "the loop-reentered inner allocation must retain one managed builder site:\n{ir}"
    );
    assert!(
        ir.contains("%yield.local.builder = alloca"),
        "the single-execution outer allocation should remain bounded local storage:\n{ir}"
    );
    assert_aot_success(source, "yield_nested_range_reentrant_inner");
}
