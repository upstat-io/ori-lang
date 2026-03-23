//! ARC Memory Management AOT Tests
//!
//! End-to-end tests verifying that ARC reference counting correctly frees
//! memory at runtime. Each test compiles an Ori program that creates RC'd
//! objects, lets them go out of scope, and verifies the drop chain runs
//! without crashing.
//!
//! These tests are slow (compile → link → execute per test) but essential
//! for verifying the full ARC pipeline works end-to-end.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{
    assert_aot_success, compile_and_capture_ir, compile_and_run_capture, extract_function_ir,
};

// ─── Basic struct creation and drop ───

#[test]
fn test_arc_struct_basic_drop() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let p = Point { x: 1, y: 2 };
    if p.x + p.y == 3 then 0 else 1
}
"#,
        "arc_struct_basic_drop",
    );
}

#[test]
fn test_arc_struct_with_string_field() {
    assert_aot_success(
        r#"
type Named = { name: str, value: int }

@main () -> int = {
    let n = Named { name: "hello", value: 42 };
    if n.value == 42 then 0 else 1
}
"#,
        "arc_struct_with_string_field",
    );
}

#[test]
fn test_arc_nested_struct_drop() {
    assert_aot_success(
        r#"
type Inner = { a: int, b: int }
type Outer = { inner: Inner, c: int }

@main () -> int = {
    let i = Inner { a: 1, b: 2 };
    let o = Outer { inner: i, c: 3 };
    if o.inner.a + o.inner.b + o.c == 6 then 0 else 1
}
"#,
        "arc_nested_struct_drop",
    );
}

// ─── Struct sharing (refcount > 1) ───

#[test]
fn test_arc_shared_struct() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let p = Point { x: 10, y: 20 };
    let q = p;
    if p.x + q.y == 30 then 0 else 1
}
"#,
        "arc_shared_struct",
    );
}

// ─── Function passing (ownership transfer) ───

#[test]
fn test_arc_struct_passed_to_function() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@sum (p: Point) -> int = p.x + p.y;

@main () -> int = {
    let p = Point { x: 3, y: 4 };
    if sum(p) == 7 then 0 else 1
}
"#,
        "arc_struct_passed_to_function",
    );
}

#[test]
fn test_arc_struct_returned_from_function() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@make_point (x: int, y: int) -> Point = Point { x: x, y: y }

@main () -> int = {
    let p = make_point(5, 6);
    if p.x + p.y == 11 then 0 else 1
}
"#,
        "arc_struct_returned_from_function",
    );
}

// ─── Enum drop ───

#[test]
fn test_arc_enum_basic_drop() {
    assert_aot_success(
        r#"
type Shape = Circle(radius: int) | Rectangle(width: int, height: int);

@main () -> int = {
    let c = Circle(radius: 5);
    let r = Rectangle(width: 3, height: 4);
    0
}
"#,
        "arc_enum_basic_drop",
    );
}

#[test]
fn test_arc_enum_with_string_payload() {
    assert_aot_success(
        r#"
type Outcome = Good(value: int) | Bad(reason: str);

@main () -> int = {
    let ok = Good(value: 42);
    let err = Bad(reason: "oops");
    0
}
"#,
        "arc_enum_with_string_payload",
    );
}

// ─── Loop allocation (stress test for drops) ───

#[test]
fn test_arc_loop_allocation() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let sum = 0;
    for i in 0..100 do {
        let p = Point { x: i, y: i * 2 };
        sum = sum + p.x + p.y
    };
    if sum == 14850 then 0 else 1
}
"#,
        "arc_loop_allocation",
    );
}

#[test]
fn test_arc_loop_string_allocation() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let count = 0;
    for i in 0..50 do {
        let n = Named { name: "test", id: i };
        count = count + n.id - n.id + 1
    };
    if count == 50 then 0 else 1
}
"#,
        "arc_loop_string_allocation",
    );
}

// ─── List with RC'd elements ───

#[test]
fn test_arc_list_of_ints() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let sum = 0;
    for x in xs do sum = sum + x;
    if sum == 15 then 0 else 1
}
"#,
        "arc_list_of_ints",
    );
}

// ─── Multiple scopes ───

#[test]
fn test_arc_block_scope_drop() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let a = {
        let p = Point { x: 1, y: 2 };
        p.x + p.y
    };
    let b = {
        let q = Point { x: 3, y: 4 };
        q.x + q.y
    };
    if a + b == 10 then 0 else 1
}
"#,
        "arc_block_scope_drop",
    );
}

// ─── String operations (RC'd strings) ───

#[test]
fn test_arc_string_concat_drop() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = " world";
    let c = a + b;
    if c == "hello world" then 0 else 1
}
"#,
        "arc_string_concat_drop",
    );
}

#[test]
fn test_arc_string_loop_concat() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    for _ in 0..10 do s = s + "x";
    if s == "xxxxxxxxxx" then 0 else 1
}
"#,
        "arc_string_loop_concat",
    );
}

// ─── Leak detection ───

/// Exit code 2 propagation: verify that `compile_and_run_capture` correctly
/// propagates exit code 2 (the leak detection indicator) from a compiled binary.
///
/// Uses `@main () -> int = 2` as a proxy since deliberate runtime leaks
/// require `extern "c"` FFI to call `ori_rc_alloc` without free (not yet
/// supported in AOT). The runtime-level leak-to-exit-code-2 chain is verified
/// by `ori_rt::tests::leak_detection_positive_control`. (TPR-05-008)
#[test]
fn test_arc_leak_detected_exit_code_2() {
    let (exit_code, _, _) = compile_and_run_capture(
        r#"
@main () -> int = 2;
"#,
    );
    assert_eq!(
        exit_code, 2,
        "Exit code 2 must propagate through compile_and_run_capture"
    );
}

/// Harness contract: `assert_aot_success` must panic when the binary exits with
/// code 2 (leak detected). Proves the harness catches leak regressions.
///
/// Uses `@main () -> int = 2` as a proxy (see `test_arc_leak_detected_exit_code_2`
/// for rationale). The panic message must contain "leaked memory". (TPR-05-008)
#[test]
fn test_arc_assert_aot_success_catches_leak() {
    let result = std::panic::catch_unwind(|| {
        assert_aot_success(
            r#"
@main () -> int = 2;
"#,
            "deliberate_exit_code_2",
        );
    });
    assert!(
        result.is_err(),
        "assert_aot_success must panic for exit code 2"
    );
    // Verify the panic message mentions "leaked memory"
    if let Err(payload) = result {
        let msg = payload.downcast_ref::<String>().map_or("", |s| s.as_str());
        assert!(
            msg.contains("leaked memory"),
            "panic message should mention 'leaked memory', got: {msg}"
        );
    }
}

/// Structural verification: the LLVM-generated main wrapper emits a call to
/// `ori_check_leaks`. This is the missing link that TPR-05-009 identified —
/// the proxy tests above prove exit code semantics, but only this test proves
/// the codegen actually wires the leak-check call into the wrapper.
///
/// Combined with `ori_rt::tests::leak_detection_positive_control` (which proves
/// the runtime function detects leaks and returns exit code 2), this creates
/// a complete verification chain:
///   codegen emits call → runtime detects leaks → exit code 2
#[test]
fn test_arc_main_wrapper_calls_ori_check_leaks() {
    let ir = compile_and_capture_ir(
        r#"
@main () -> int = 0;
"#,
    );
    let main_ir = extract_function_ir(&ir, "main");
    assert!(
        main_ir.contains("@ori_check_leaks"),
        "main wrapper must call ori_check_leaks.\nMain IR:\n{main_ir}"
    );
}

#[test]
fn test_arc_clean_program_no_leak() {
    // A well-formed program should exit 0 with leak checking enabled.
    let (exit_code, _, _) = compile_and_run_capture(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let p = Point { x: 1, y: 2 };
    if p.x + p.y == 3 then 0 else 1
}
"#,
    );
    assert_eq!(
        exit_code, 0,
        "clean program should exit 0 with leak checking enabled"
    );
}

#[test]
fn test_arc_leak_check_enabled_for_all_aot_tests() {
    // Verify that assert_aot_success uses leak checking by running a
    // known-clean program. If leak checking causes false positives,
    // this test would catch it.
    assert_aot_success(
        r#"
type Wrapper = { value: int }

@main () -> int = {
    let w = Wrapper { value: 42 };
    let w2 = w;
    if w.value + w2.value == 84 then 0 else 1
}
"#,
        "arc_leak_check_enabled",
    );
}

// ─── Lambda / Closure ───

#[test]
fn test_arc_lambda_capture_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 10;
    let f = (y: int) -> x + y;
    if f(5) == 15 then 0 else 1
}
"#,
        "arc_lambda_capture_int",
    );
}

#[test]
fn test_arc_lambda_no_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let f = (a: int, b: int) -> a + b;
    if f(3, 4) == 7 then 0 else 1
}
"#,
        "arc_lambda_no_capture",
    );
}

#[test]
fn test_arc_lambda_capture_multiple() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 100;
    let b = 23;
    let f = (x: int) -> a + b + x;
    if f(0) == 123 then 0 else 1
}
"#,
        "arc_lambda_capture_multiple",
    );
}

#[test]
fn test_arc_lambda_passed_to_function() {
    assert_aot_success(
        r#"
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    let base = 40;
    let adder = (n: int) -> base + n;
    if apply(adder, 2) == 42 then 0 else 1
}
"#,
        "arc_lambda_passed_to_function",
    );
}

#[test]
fn test_arc_lambda_returned_from_function() {
    assert_aot_success(
        r#"
@make_adder (base: int) -> (int) -> int = {
    (n: int) -> base + n
};

@main () -> int = {
    let add10 = make_adder(10);
    if add10(5) == 15 then 0 else 1
}
"#,
        "arc_lambda_returned_from_function",
    );
}

#[test]
fn test_arc_lambda_nested_capture() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 100;
    let outer = (b: int) -> {
        let inner = (c: int) -> a + b + c;
        inner(3)
    };
    if outer(20) == 123 then 0 else 1
}
"#,
        "arc_lambda_nested_capture",
    );
}

#[test]
fn test_arc_lambda_capture_bool() {
    assert_aot_success(
        r#"
@main () -> int = {
    let flag = true;
    let check = (x: int) -> if flag then x else 0;
    if check(42) == 42 then 0 else 1
}
"#,
        "arc_lambda_capture_bool",
    );
}

// ─── Closure lifecycle: loop + passed closures ───

#[test]
fn test_arc_closure_loop_no_leak() {
    // Closures created in a loop must be freed each iteration.
    // ORI_CHECK_LEAKS=1 (set by assert_aot_success) catches accumulation.
    assert_aot_success(
        r#"
@make_adder (n: int) -> (int) -> int = {
    (x: int) -> n + x
};

@main () -> int = {
    let sum = 0;
    for i in 0..100 do {
        let $f = make_adder(i);
        sum = sum + f(1)
    };
    if sum == 5050 then 0 else 1
}
"#,
        "arc_closure_loop_no_leak",
    );
}

#[test]
fn test_arc_closure_passed_and_freed() {
    // Closure passed to another function — must be freed after last use.
    assert_aot_success(
        r#"
@apply_twice (f: (int) -> int, x: int) -> int = {
    f(f(x))
};

@main () -> int = {
    let $n = 10;
    let $adder = (x: int) -> x + n;
    let $result = apply_twice(f: adder, x: 5);
    if result == 25 then 0 else 1
}
"#,
        "arc_closure_passed_and_freed",
    );
}

// ─── Aliasing: shared RC buffer passed to multiple params ───

#[test]
fn test_arc_aliased_list_params() {
    // Both `a` and `b` share the same RC buffer — must produce correct
    // result even though the LLVM IR pointers alias. This verifies that
    // we do NOT blanket-apply `noalias` to function pointer params.
    assert_aot_success(
        r#"
@sum_first (a: [int], b: [int]) -> int = a[0] + b[0];

@main () -> int = {
    let xs = [42, 10, 20];
    let result = sum_first(a: xs, b: xs);
    if result == 84 then 0 else 1
}
"#,
        "arc_aliased_list_params",
    );
}

#[test]
fn test_arc_aliased_string_params() {
    // Same aliasing test with strings — both params share the same buffer.
    assert_aot_success(
        r#"
@both_equal (a: str, b: str) -> bool = a == b;

@main () -> int = {
    let s = "hello world";
    if both_equal(a: s, b: s) then 0 else 1
}
"#,
        "arc_aliased_string_params",
    );
}

// Alias chain: three variables all pointing to the same heap string.
// The pre-use RcInc for intermediate aliases must not be suppressed.

// Alias chain: a → b → c, all used after aliasing.
// Verifies pre-use RcInc for intermediate aliases is not suppressed.
// The ARC IR must have enough RcIncs to balance the RcDecs (one per
// terminal alias). Without this, the binary double-frees due to UB.

#[test]
fn test_arc_alias_chain_no_double_free() {
    // Run multiple times — double-free UB is non-deterministic; a single
    // run may succeed if the allocator hasn't reclaimed the freed page.
    for _ in 0..5 {
        assert_aot_success(
            r#"
@main () -> int = {
    let a = "this is a heap string for alias chain";
    let b = a;
    let c = b;
    if a == b && b == c then 0 else 1
}
"#,
            "arc_alias_chain_no_double_free",
        );
    }
}

#[test]
fn test_arc_alias_chain_three_way_use() {
    // All three aliases used independently after the chain.
    for _ in 0..5 {
        assert_aot_success(
            r#"
@check (x: str, y: str) -> bool = x == y;

@main () -> int = {
    let a = "shared heap string";
    let b = a;
    let c = b;
    let $r1 = check(x: a, y: b);
    let $r2 = check(x: b, y: c);
    let $r3 = check(x: a, y: c);
    if r1 && r2 && r3 then 0 else 1
}
"#,
            "arc_alias_chain_three_way_use",
        );
    }
}

// ─── RC identity + projection regression matrices (Matrix A) ───
//
// These fixtures systematically exercise the interaction between:
// - Let { dst, value: Var(src) } alias chains
// - Project source semantics (scalar = borrowing, non-scalar = transfer)
// - Path-sensitive cleanup (Switch / branch successors)
// - Exact RcInc placement in the unified forward walk
//
// AIMS verification Matrix A: RC placement correctness

// A1: Scalar Project from aliased scrutinee.
// catch(expr:) returns Result<str, str>. Matching on the result creates an
// alias of the scrutinee. The tag check (Ok vs Err) is a scalar Project —
// no extra RcInc should be inserted on the alias before the tag Project,
// and no extra RcDec should appear in the Ok block.
#[test]
fn test_rc_catch_heap_alias_scalar_project() {
    for _ in 0..5 {
        assert_aot_success(
            r#"
@main () -> int = {
    let r = catch(expr: "heap string for alias test");
    match r {
        Ok(v) -> if v == "heap string for alias test" then 0 else 1,
        Err(_) -> 2,
    }
}
"#,
            "rc_catch_heap_alias_scalar_project",
        );
    }
}

// A2/A4: Non-scalar Project from aliased scrutinee + borrowing/transfer split.
// Result<int, str> — Ok(int) is borrowing (scalar payload), Err(str) is
// transfer (heap payload). The borrowing branch must drop the root Result;
// the transfer branch suppresses root drop (ownership flows to extracted str).
#[test]
fn test_rc_try_result_int_str_projection_split() {
    for _ in 0..5 {
        assert_aot_success(
            r#"
@maybe_int (x: int) -> Result<int, str> = {
    if x > 0 then Ok(x) else Err("negative value")
};

@main () -> int = {
    let r1 = maybe_int(5);
    let r2 = maybe_int(-1);
    let $ok_val = match r1 {
        Ok(v) -> v,
        Err(_) -> -1,
    };
    let $err_msg = match r2 {
        Ok(_) -> "",
        Err(e) -> e,
    };
    if ok_val == 5 && err_msg == "negative value" then 0 else 1
}
"#,
            "rc_try_result_int_str_projection_split",
        );
    }
}

// A7/A8: Alias of alias used only through borrowing primops (compare).
// a → b → c chain where all three are compared. Intermediate alias must
// receive RcInc when both source and downstream aliases stay live.
#[test]
fn test_rc_alias_chain_compare_heap_string() {
    for _ in 0..5 {
        assert_aot_success(
            r#"
@main () -> int = {
    let a = "alias chain comparison string";
    let b = a;
    let c = b;
    let $r1 = a == b;
    let $r2 = b == c;
    let $r3 = a == c;
    if r1 && r2 && r3 then 0 else 1
}
"#,
            "rc_alias_chain_compare_heap_string",
        );
    }
}

// A5: Owned call after alias split.
// Alias b consumed by owned callee (returns the string, transferring
// ownership). Root a is used after the call — exactly one RcInc must be
// inserted at the ownership divergence point.
#[test]
fn test_rc_alias_owned_call_then_root_use() {
    for _ in 0..5 {
        assert_aot_success(
            r#"
@take_and_return (s: str) -> str = s;

@main () -> int = {
    let a = "owned call alias test string";
    let b = a;
    let $result = take_and_return(b);
    if result == a then 0 else 1
}
"#,
            "rc_alias_owned_call_then_root_use",
        );
    }
}

// A6: Borrowed call after alias split.
// Alias b passed to borrowed callee (only reads). No spurious RcInc for
// the borrowed call, but final owner must still be dropped exactly once.
#[test]
fn test_rc_alias_borrowed_call_then_root_use() {
    for _ in 0..5 {
        assert_aot_success(
            r#"
@check_length (s: str) -> bool = len(collection: s) > 0;

@main () -> int = {
    let a = "borrowed call alias test string";
    let b = a;
    let $ok = check_length(b);
    if ok && a == "borrowed call alias test string" then 0 else 1
}
"#,
            "rc_alias_borrowed_call_then_root_use",
        );
    }
}

// A3: Borrowing projection on both successor paths.
// Both branches of an if/else project scalar fields from the same struct.
// Root aggregate is decremented at last borrowing use in each branch,
// not at the branch point.
#[test]
fn test_rc_switch_two_scalar_borrow_branches() {
    for _ in 0..5 {
        assert_aot_success(
            r#"
type Pair = { x: int, y: int }

@main () -> int = {
    let p = Pair { x: 10, y: 20 };
    let $flag = true;
    let $result = if flag then p.x else p.y;
    if result == 10 then 0 else 1
}
"#,
            "rc_switch_two_scalar_borrow_branches",
        );
    }
}

// ─── Loop reassignment RC leak tests ───
//
// These test that RC values reassigned inside loops are properly freed.
// The ARC pipeline must emit RcDec for the OLD value when a mutable
// binding is overwritten in a loop body.

#[test]
fn test_arc_loop_string_reassignment_no_leak() {
    // String concat in a loop: `s = s + "x"` must RC-dec the old `s`
    // each iteration. Without the dec, every old string leaks.
    // ORI_CHECK_LEAKS=1 (set by assert_aot_success) catches this.
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    for i in 0..30 do {
        s = s + "x"
    };
    if s.len() == 30 then 0 else 1
}
"#,
        "arc_loop_string_reassignment_no_leak",
    );
}

#[test]
fn test_arc_loop_string_reassignment_correctness() {
    // Verify the loop produces correct output (not just no crash).
    let (exit_code, stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let s = "";
    for i in 0..5 do {
        s = s + str(i)
    };
    print(msg: s);
    if s == "01234" then 0 else 1
}
"#,
    );
    assert_eq!(
        exit_code, 0,
        "loop string reassignment produced wrong result (exit {exit_code}):\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert_eq!(stdout.trim(), "01234");
}

#[test]
fn test_arc_loop_string_reassignment_manual_loop_no_leak() {
    // Same pattern with a manual loop instead of for.
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    let i = 0;
    loop {
        if i >= 30 then break;
        s = s + "a";
        i = i + 1
    };
    if s.len() == 30 then 0 else 1
}
"#,
        "arc_loop_string_reassignment_manual_loop_no_leak",
    );
}

#[test]
fn test_arc_loop_list_reassignment_no_leak() {
    // List push in a loop: `xs = xs.push(i)` must RC-dec the old list.
    assert_aot_success(
        r#"
@main () -> int = {
    let xs: [int] = [];
    for i in 0..30 do {
        xs = xs.push(i)
    };
    if xs.len() == 30 then 0 else 1
}
"#,
        "arc_loop_list_reassignment_no_leak",
    );
}

// ─── Borrowed parameter + COW push ───

/// Semantic pin: borrowed parameter passed to COW push must not invalidate
/// the caller's reference. Without `RcInc` before the push, the push sees
/// RC=1 (unique), reallocs in place, and the caller's pointer becomes stale.
#[test]
fn test_arc_borrowed_param_cow_push_use_after() {
    assert_aot_success(
        r#"
@check (list: [int]) -> bool = {
    let modified = list.push(99);
    modified.len() == 4 && list.len() == 3
}

@main () -> int = {
    let result = check(list: [1, 2, 3]);
    if result then 0 else 1
}
"#,
        "arc_borrowed_param_cow_push_use_after",
    );
}

/// Borrowed parameter: diamond sharing — push on borrowed param, verify original.
#[test]
fn test_arc_borrowed_param_cow_push_diamond() {
    assert_aot_success(
        r#"
@fork (base: [int], val: int) -> [int] = {
    base.push(val)
}

@main () -> int = {
    let base = [10, 20, 30];
    let a = fork(base: base, val: 40);
    let b = fork(base: base, val: 50);
    if base.len() == 3 && a.len() == 4 && b.len() == 4
        && a[3] == 40 && b[3] == 50
    then 0 else 1
}
"#,
        "arc_borrowed_param_cow_push_diamond",
    );
}

/// Borrowed string list parameter: push on borrowed param with heap elements.
#[test]
fn test_arc_borrowed_param_cow_push_str_list() {
    assert_aot_success(
        r#"
@extend_list (items: [str]) -> [str] = {
    items.push("added")
}

@main () -> int = {
    let original = ["hello", "world"];
    let extended = extend_list(items: original);
    if original.len() == 2 && extended.len() == 3
        && original[0] == "hello" && extended[2] == "added"
    then 0 else 1
}
"#,
        "arc_borrowed_param_cow_push_str_list",
    );
}

/// Borrowed string parameter with concat — must NOT get COW `RcInc` guard.
/// String concat is a borrowing operation (produces new string), not COW.
/// Regression: TPR-05-003 — pre-pass emitted `RcInc` with `HeapPointer` strategy
/// on a `FatPointer` (string) variable, causing `debug_assert` abort in `rc_ops.rs`.
#[test]
fn test_arc_borrowed_param_str_concat_not_cow() {
    assert_aot_success(
        r#"
@grow (s: str) -> str = {
    s.concat(other: "!")
}

@main () -> int = {
    let $result = grow(s: "hello");
    if result == "hello!" then 0 else 1
}
"#,
        "arc_borrowed_param_str_concat_not_cow",
    );
}

/// Borrowed string parameter with add — must NOT get COW `RcInc` guard.
/// String add is a borrowing operation, not COW.
/// Regression: TPR-05-003 — "add" is in `all_cow_method_names` but is
/// type-qualified (COW for lists, borrowing for strings).
#[test]
fn test_arc_borrowed_param_str_add_not_cow() {
    assert_aot_success(
        r#"
@prefix (s: str) -> str = {
    s + "!"
}

@main () -> int = {
    let $result = prefix(s: "hi");
    if result == "hi!" then 0 else 1
}
"#,
        "arc_borrowed_param_str_add_not_cow",
    );
}

/// Borrowed string parameter: original must survive after callee produces new string.
/// Verifies that the caller's string reference is not invalidated.
#[test]
fn test_arc_borrowed_param_str_concat_caller_survives() {
    assert_aot_success(
        r#"
@append_world (s: str) -> str = {
    s.concat(other: " world")
}

@main () -> int = {
    let $original = "hello";
    let $extended = append_world(s: original);
    if original == "hello" && extended == "hello world"
    then 0 else 1
}
"#,
        "arc_borrowed_param_str_concat_caller_survives",
    );
}

// TPR-02-006: Project alias closure at CFG merge with two distinct parents.
// The merge block param receives projections from two different aggregates
// depending on control flow — both parents must stay alive.
//
// TPR-02-007/TPR-02-010: Branch-local RcDec must be emitted per-predecessor
// (not block-level in merge), and only on the specific merge edge (not all
// outgoing edges of the defining predecessor). Tests exercise BOTH branches
// to confirm branch-local cleanup is correct regardless of which path is taken.
#[test]
fn test_rc_project_merge_two_distinct_parents() {
    // Test 1: condition true → takes then-branch (p1.first selected, p2 cleaned up)
    for _ in 0..5 {
        assert_aot_success(
            r#"
type Pair = { first: str, second: str }

@main () -> int = {
    let $p1 = Pair { first: "alpha-heap-string-over-23-bytes!", second: "beta-heap-string-also-over-23-bytes!" };
    let $p2 = Pair { first: "gamma-heap-string-over-23-bytes!", second: "delta-heap-string-also-over-23-bytes!" };
    // Branch: merge param receives projection from different parent aggregates.
    // Branch-local parent (p1 on then-path, p2 on else-path) must be cleaned
    // up only on its own merge edge, not on the opposite path.
    let $result = if p1.first.len() > 0
        then p1.first
        else p2.first;
    if result == "alpha-heap-string-over-23-bytes!" then 0 else 1
}
"#,
            "arc_project_merge_then_branch",
        );
    }

    // Test 2: condition false → takes else-branch (p2.first selected, p1 cleaned up)
    for _ in 0..5 {
        assert_aot_success(
            r#"
type Pair = { first: str, second: str }

@main () -> int = {
    let $p1 = Pair { first: "", second: "beta-heap-string-also-over-23-bytes!" };
    let $p2 = Pair { first: "gamma-heap-string-over-23-bytes!", second: "delta-heap-string-also-over-23-bytes!" };
    let $result = if p1.first.len() > 0
        then p1.first
        else p2.first;
    if result == "gamma-heap-string-over-23-bytes!" then 0 else 1
}
"#,
            "arc_project_merge_else_branch",
        );
    }
}

// TPR-02-010 regression: verify that merge-edge decs with successor scoping
// don't cause leaks when both branches produce heap strings and the untaken
// branch's parent aggregate needs cleanup via edge-specific RcDec.
#[test]
fn test_rc_project_merge_edge_scoped_cleanup() {
    // Condition-variable driven selection between two structs, each containing
    // heap strings. The untaken path's struct must be cleaned up on the edge
    // (not in the merge block), and the taken path's struct must survive until
    // its projected field is consumed.
    for _ in 0..10 {
        assert_aot_success(
            r#"
type Record = { name: str, data: str }

@make_alpha () -> Record =
    Record { name: "alpha-record-name-over-23-bytes!", data: "alpha-data-payload-over-23-bytes!" };

@make_beta () -> Record =
    Record { name: "beta-record-name-over-23-bytes!", data: "beta-data-payload-over-23-bytes!" };

@main () -> int = {
    let $a = make_alpha();
    let $b = make_beta();
    // Runtime-variable condition: a.name is 32 chars, b.name is 31 chars
    let $pick = if a.name.len() > b.name.len()
        then a.name
        else b.name;
    // Untaken branch's parent must be cleaned up without double-free or leak
    if pick == "alpha-record-name-over-23-bytes!" then 0 else 1
}
"#,
            "arc_merge_edge_scoped",
        );
    }
}

// TPR-01-001 regression: verify that two distinct projected fields from the
// same parent aggregate both survive edge cleanup when both escape via
// terminator args. The old code stored a single `parent -> Project dst` in
// `find_edge_decced_project_parents()`, so the second projection overwrote
// the first and only one child got the compensating RcInc.
//
// Test matrix: struct destructuring (same parent), tuple destructuring,
// and conditional merge — all with heap-allocated (>23 byte) str fields.
#[test]
fn test_rc_project_merge_edge_two_fields_escape() {
    // Case 1: Struct destructuring — both fields projected from same parent.
    for _ in 0..10 {
        assert_aot_success(
            r#"
type Pair = { first: str, second: str }

@make_pair () -> Pair =
    Pair { first: "first-field-value-over-twenty-three!", second: "second-field-value-over-twenty-three!" };

@main () -> int = {
    let $p = make_pair();
    let { first, second } = p;
    if first == "first-field-value-over-twenty-three!"
        then (if second == "second-field-value-over-twenty-three!" then 0 else 1)
        else 2
}
"#,
            "arc_merge_edge_two_fields_struct",
        );
    }
    // Case 2: Conditional with both fields crossing a merge edge.
    for _ in 0..10 {
        assert_aot_success(
            r#"
type Pair = { first: str, second: str }

@make_pair () -> Pair =
    Pair { first: "first-field-value-over-twenty-three!", second: "second-field-value-over-twenty-three!" };

@pick (cond: bool) -> (str, str) = {
    let $p = make_pair();
    let { first, second } = p;
    if cond then (first, second) else (second, first)
}

@main () -> int = {
    let $result = pick(cond: true);
    let (f, s) = result;
    if f == "first-field-value-over-twenty-three!"
        then (if s == "second-field-value-over-twenty-three!" then 0 else 1)
        else 2
}
"#,
            "arc_merge_edge_two_fields_cond",
        );
    }
    // Case 3: Three fields from same parent — ensures fix handles >2 fields.
    for _ in 0..10 {
        assert_aot_success(
            r#"
type Triple = { a: str, b: str, c: str }

@make_triple () -> Triple =
    Triple { a: "aaa-field-over-twenty-three-bytes!", b: "bbb-field-over-twenty-three-bytes!", c: "ccc-field-over-twenty-three-bytes!" };

@main () -> int = {
    let $t = make_triple();
    let { a, b, c } = t;
    if a == "aaa-field-over-twenty-three-bytes!"
        then (if b == "bbb-field-over-twenty-three-bytes!"
            then (if c == "ccc-field-over-twenty-three-bytes!" then 0 else 1)
            else 2)
        else 3
}
"#,
            "arc_merge_edge_three_fields",
        );
    }
}
