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
        include_str!("fixtures/arc/arc_struct_basic_drop.ori"),
        "arc_struct_basic_drop",
    );
}

#[test]
fn test_arc_struct_with_string_field() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_struct_with_string_field.ori"),
        "arc_struct_with_string_field",
    );
}

#[test]
fn test_arc_nested_struct_drop() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_nested_struct_drop.ori"),
        "arc_nested_struct_drop",
    );
}

// ─── Struct sharing (refcount > 1) ───

#[test]
fn test_arc_shared_struct() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_shared_struct.ori"),
        "arc_shared_struct",
    );
}

// ─── Function passing (ownership transfer) ───

#[test]
fn test_arc_struct_passed_to_function() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_struct_passed_to_function.ori"),
        "arc_struct_passed_to_function",
    );
}

#[test]
fn test_arc_struct_returned_from_function() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_struct_returned_from_function.ori"),
        "arc_struct_returned_from_function",
    );
}

// ─── Enum drop ───

#[test]
fn test_arc_enum_basic_drop() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_enum_basic_drop.ori"),
        "arc_enum_basic_drop",
    );
}

#[test]
fn test_arc_enum_with_string_payload() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_enum_with_string_payload.ori"),
        "arc_enum_with_string_payload",
    );
}

// ─── Loop allocation (stress test for drops) ───

#[test]
fn test_arc_loop_allocation() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_loop_allocation.ori"),
        "arc_loop_allocation",
    );
}

#[test]
fn test_arc_loop_string_allocation() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_loop_string_allocation.ori"),
        "arc_loop_string_allocation",
    );
}

// ─── List with RC'd elements ───

#[test]
fn test_arc_list_of_ints() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_list_of_ints.ori"),
        "arc_list_of_ints",
    );
}

// ─── Multiple scopes ───

#[test]
fn test_arc_block_scope_drop() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_block_scope_drop.ori"),
        "arc_block_scope_drop",
    );
}

// ─── String operations (RC'd strings) ───

#[test]
fn test_arc_string_concat_drop() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_string_concat_drop.ori"),
        "arc_string_concat_drop",
    );
}

#[test]
fn test_arc_string_loop_concat() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_string_loop_concat.ori"),
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
/// by `ori_rt::tests::leak_detection_positive_control`.
#[test]
fn test_arc_leak_detected_exit_code_2() {
    let (exit_code, _, _) = compile_and_run_capture(include_str!(
        "fixtures/arc/arc_leak_detected_exit_code_2.ori"
    ));
    assert_eq!(
        exit_code, 2,
        "Exit code 2 must propagate through compile_and_run_capture"
    );
}

/// Harness contract: `assert_aot_success` must panic when the binary exits with
/// code 2 (leak detected). Proves the harness catches leak regressions.
///
/// Uses `@main () -> int = 2` as a proxy (see `test_arc_leak_detected_exit_code_2`
/// for rationale). The panic message must contain "leaked memory".
#[test]
fn test_arc_assert_aot_success_catches_leak() {
    let result = std::panic::catch_unwind(|| {
        assert_aot_success(
            include_str!("fixtures/arc/arc_assert_aot_success_catches_leak.ori"),
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
/// `ori_check_leaks`. The proxy tests above prove exit code semantics, but
/// only this test proves the codegen actually wires the leak-check call into
/// the wrapper.
///
/// Combined with `ori_rt::tests::leak_detection_positive_control` (which proves
/// the runtime function detects leaks and returns exit code 2), this creates
/// a complete verification chain:
///   codegen emits call → runtime detects leaks → exit code 2
#[test]
fn test_arc_main_wrapper_calls_ori_check_leaks() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/arc/arc_main_wrapper_calls_ori_check_leaks.ori"
    ));
    let main_ir = extract_function_ir(&ir, "main");
    assert!(
        main_ir.contains("@ori_check_leaks"),
        "main wrapper must call ori_check_leaks.\nMain IR:\n{main_ir}"
    );
}

#[test]
fn test_arc_clean_program_no_leak() {
    // A well-formed program should exit 0 with leak checking enabled.
    let (exit_code, _, _) =
        compile_and_run_capture(include_str!("fixtures/arc/arc_clean_program_no_leak.ori"));
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
        include_str!("fixtures/arc/arc_leak_check_enabled_for_all_aot_tests.ori"),
        "arc_leak_check_enabled",
    );
}

// ─── Lambda / Closure ───

#[test]
fn test_arc_lambda_capture_int() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_lambda_capture_int.ori"),
        "arc_lambda_capture_int",
    );
}

#[test]
fn test_arc_lambda_no_capture() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_lambda_no_capture.ori"),
        "arc_lambda_no_capture",
    );
}

#[test]
fn test_arc_lambda_capture_multiple() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_lambda_capture_multiple.ori"),
        "arc_lambda_capture_multiple",
    );
}

#[test]
fn test_arc_lambda_passed_to_function() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_lambda_passed_to_function.ori"),
        "arc_lambda_passed_to_function",
    );
}

#[test]
fn test_arc_lambda_returned_from_function() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_lambda_returned_from_function.ori"),
        "arc_lambda_returned_from_function",
    );
}

#[test]
fn test_arc_lambda_nested_capture() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_lambda_nested_capture.ori"),
        "arc_lambda_nested_capture",
    );
}

#[test]
fn test_arc_lambda_capture_bool() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_lambda_capture_bool.ori"),
        "arc_lambda_capture_bool",
    );
}

// ─── Curried closure RC captures (TPR-04B-014 regression) ───

#[test]
fn test_arc_curried_closure_capture_list() {
    // Regression: TPR-04B-014 — curried closure capturing list.
    // Fixed by closure-ownership Section 02: arg_ownership on ApplyIndirect.
    assert_aot_success(
        include_str!("fixtures/arc/arc_curried_closure_capture_list.ori"),
        "arc_curried_closure_capture_list",
    );
}

#[test]
fn test_arc_curried_closure_capture_str() {
    // Fixed by closure-ownership Section 02: arg_ownership on ApplyIndirect.
    assert_aot_success(
        include_str!("fixtures/arc/arc_curried_closure_capture_str.ori"),
        "arc_curried_closure_capture_str",
    );
}

#[test]
fn test_arc_curried_closure_capture_nested() {
    // Nested curried closures: outer captures list, inner returns it.
    // Fixed by closure-ownership Section 02: arg_ownership on ApplyIndirect.
    assert_aot_success(
        include_str!("fixtures/arc/arc_curried_closure_capture_nested.ori"),
        "arc_curried_closure_capture_nested",
    );
}

#[test]
fn test_arc_curried_closure_scalar_no_inc() {
    // Negative: scalar captures must NOT get RcInc.
    assert_aot_success(
        include_str!("fixtures/arc/arc_curried_closure_scalar_no_inc.ori"),
        "arc_curried_closure_scalar_no_inc",
    );
}

// ─── Opaque closure wrapper (Section 02 soundness proof) ───

#[test]
fn test_arc_opaque_closure_wrapper() {
    // Opaque higher-order wrapper: closure passed as function parameter,
    // called indirectly. Verifies all-Borrowed fallback is RC-balanced.
    // closure-ownership Section 02 soundness proof.
    assert_aot_success(
        include_str!("fixtures/arc/arc_opaque_closure_wrapper.ori"),
        "arc_opaque_closure_wrapper",
    );
}

// ─── Closure lifecycle: loop + passed closures ───

#[test]
fn test_arc_closure_loop_no_leak() {
    // Closures created in a loop must be freed each iteration.
    // ORI_CHECK_LEAKS=1 (set by assert_aot_success) catches accumulation.
    assert_aot_success(
        include_str!("fixtures/arc/arc_closure_loop_no_leak.ori"),
        "arc_closure_loop_no_leak",
    );
}

#[test]
fn test_arc_closure_passed_and_freed() {
    // Closure passed to another function — must be freed after last use.
    assert_aot_success(
        include_str!("fixtures/arc/arc_closure_passed_and_freed.ori"),
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
        include_str!("fixtures/arc/arc_aliased_list_params.ori"),
        "arc_aliased_list_params",
    );
}

#[test]
fn test_arc_aliased_string_params() {
    // Same aliasing test with strings — both params share the same buffer.
    assert_aot_success(
        include_str!("fixtures/arc/arc_aliased_string_params.ori"),
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
            include_str!("fixtures/arc/arc_alias_chain_no_double_free.ori"),
            "arc_alias_chain_no_double_free",
        );
    }
}

#[test]
fn test_arc_alias_chain_three_way_use() {
    // All three aliases used independently after the chain.
    for _ in 0..5 {
        assert_aot_success(
            include_str!("fixtures/arc/arc_alias_chain_three_way_use.ori"),
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
            include_str!("fixtures/arc/rc_catch_heap_alias_scalar_project.ori"),
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
            include_str!("fixtures/arc/rc_try_result_int_str_projection_split.ori"),
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
            include_str!("fixtures/arc/rc_alias_chain_compare_heap_string.ori"),
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
            include_str!("fixtures/arc/rc_alias_owned_call_then_root_use.ori"),
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
            include_str!("fixtures/arc/rc_alias_borrowed_call_then_root_use.ori"),
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
            include_str!("fixtures/arc/rc_switch_two_scalar_borrow_branches.ori"),
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
        include_str!("fixtures/arc/arc_loop_string_reassignment_no_leak.ori"),
        "arc_loop_string_reassignment_no_leak",
    );
}

#[test]
fn test_arc_loop_string_reassignment_correctness() {
    // Verify the loop produces correct output (not just no crash).
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/arc/arc_loop_string_reassignment_correctness.ori"
    ));
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
        include_str!("fixtures/arc/arc_loop_string_reassignment_manual_loop_no_leak.ori"),
        "arc_loop_string_reassignment_manual_loop_no_leak",
    );
}

#[test]
fn test_arc_loop_list_reassignment_no_leak() {
    // List push in a loop: `xs = xs.push(i)` must RC-dec the old list.
    assert_aot_success(
        include_str!("fixtures/arc/arc_loop_list_reassignment_no_leak.ori"),
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
        include_str!("fixtures/arc/arc_borrowed_param_cow_push_use_after.ori"),
        "arc_borrowed_param_cow_push_use_after",
    );
}

/// Borrowed parameter: diamond sharing — push on borrowed param, verify original.
#[test]
fn test_arc_borrowed_param_cow_push_diamond() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_borrowed_param_cow_push_diamond.ori"),
        "arc_borrowed_param_cow_push_diamond",
    );
}

/// Borrowed string list parameter: push on borrowed param with heap elements.
#[test]
fn test_arc_borrowed_param_cow_push_str_list() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_borrowed_param_cow_push_str_list.ori"),
        "arc_borrowed_param_cow_push_str_list",
    );
}

/// Borrowed string parameter with concat — must NOT get COW `RcInc` guard.
/// String concat is a borrowing operation (produces new string), not COW.
/// Regression: COW pre-pass emitted `RcInc` with `HeapPointer` strategy
/// on a `FatPointer` (string) variable, causing `debug_assert` abort in `rc_ops.rs`.
#[test]
fn test_arc_borrowed_param_str_concat_not_cow() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_borrowed_param_str_concat_not_cow.ori"),
        "arc_borrowed_param_str_concat_not_cow",
    );
}

/// Borrowed string parameter with add — must NOT get COW `RcInc` guard.
/// String add is a borrowing operation, not COW.
/// Regression: "add" is in `all_cow_method_names` but is type-qualified
/// (COW for lists, borrowing for strings).
#[test]
fn test_arc_borrowed_param_str_add_not_cow() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_borrowed_param_str_add_not_cow.ori"),
        "arc_borrowed_param_str_add_not_cow",
    );
}

/// Borrowed string parameter: original must survive after callee produces new string.
/// Verifies that the caller's string reference is not invalidated.
#[test]
fn test_arc_borrowed_param_str_concat_caller_survives() {
    assert_aot_success(
        include_str!("fixtures/arc/arc_borrowed_param_str_concat_caller_survives.ori"),
        "arc_borrowed_param_str_concat_caller_survives",
    );
}

// Project alias closure at CFG merge with two distinct parents.
// The merge block param receives projections from two different aggregates
// depending on control flow — both parents must stay alive.
//
// TPR-02-007/Branch-local RcDec must be emitted per-predecessor
// (not block-level in merge), and only on the specific merge edge (not all
// outgoing edges of the defining predecessor). Tests exercise BOTH branches
// to confirm branch-local cleanup is correct regardless of which path is taken.
#[test]
fn test_rc_project_merge_two_distinct_parents() {
    // Test 1: condition true → takes then-branch (p1.first selected, p2 cleaned up)
    for _ in 0..5 {
        assert_aot_success(
            include_str!("fixtures/arc/rc_project_merge_two_distinct_parents.ori"),
            "arc_project_merge_then_branch",
        );
    }

    // Test 2: condition false → takes else-branch (p2.first selected, p1 cleaned up)
    for _ in 0..5 {
        assert_aot_success(
            include_str!("fixtures/arc/rc_project_merge_two_distinct_parents.ori"),
            "arc_project_merge_else_branch",
        );
    }
}

// regression: verify that merge-edge decs with successor scoping
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
            include_str!("fixtures/arc/rc_project_merge_edge_scoped_cleanup.ori"),
            "arc_merge_edge_scoped",
        );
    }
}

// regression: verify that two distinct projected fields from the
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
            include_str!("fixtures/arc/rc_project_merge_edge_two_fields_escape.ori"),
            "arc_merge_edge_two_fields_struct",
        );
    }
    // Case 2: Conditional with both fields crossing a merge edge.
    for _ in 0..10 {
        assert_aot_success(
            include_str!("fixtures/arc/rc_project_merge_edge_two_fields_escape.ori"),
            "arc_merge_edge_two_fields_cond",
        );
    }
    // Case 3: Three fields from same parent — ensures fix handles >2 fields.
    for _ in 0..10 {
        assert_aot_success(
            include_str!("fixtures/arc/rc_project_merge_edge_two_fields_escape.ori"),
            "arc_merge_edge_three_fields",
        );
    }
}

// ─── Trivial inline enum ARC elision (§02 ) ───

/// §02 regression pin: trivial `Option<int>` must emit NO `ori_rc_inc`,
/// `ori_rc_dec`, or `_ori_drop$` in the generated LLVM IR. This proves
/// that the transitive triviality classification correctly identifies
/// `Option<int>` as trivial and elides all ARC operations.
#[test]
fn test_trivial_option_int_no_rc_ops() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/arc/trivial_option_int_no_rc_ops.ori"
    ));
    let main_ir = extract_function_ir(&ir, "_ori_main");
    assert!(
        !main_ir.contains("ori_rc_inc"),
        "Trivial Option<int> should NOT emit ori_rc_inc:\n{main_ir}"
    );
    assert!(
        !main_ir.contains("ori_rc_dec"),
        "Trivial Option<int> should NOT emit ori_rc_dec:\n{main_ir}"
    );
    assert!(
        !ir.contains("_ori_drop$"),
        "Trivial Option<int> should NOT generate a drop function:\n{ir}"
    );
}

/// §02 regression pin: trivial `Result<int, int>` must emit NO RC ops.
/// Both `Ok` and `Err` payloads are trivial scalars.
#[test]
fn test_trivial_result_int_int_no_rc_ops() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/arc/trivial_result_int_int_no_rc_ops.ori"
    ));
    let main_ir = extract_function_ir(&ir, "_ori_main");
    assert!(
        !main_ir.contains("ori_rc_inc"),
        "Trivial Result<int, int> should NOT emit ori_rc_inc:\n{main_ir}"
    );
    assert!(
        !main_ir.contains("ori_rc_dec"),
        "Trivial Result<int, int> should NOT emit ori_rc_dec:\n{main_ir}"
    );
}

/// §02 negative pin: non-trivial `Option<str>` MUST emit RC ops.
/// `str` is heap-allocated with RC, so `Option<str>` is non-trivial.
/// This is the companion to `test_trivial_option_int_no_rc_ops`.
#[test]
fn test_nontrivial_option_str_has_rc_ops() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/arc/nontrivial_option_str_has_rc_ops.ori"
    ));
    // Non-trivial Option<str> must have RC operations somewhere in the IR.
    assert!(
        ir.contains("ori_rc_dec") || ir.contains("_ori_drop$"),
        "Non-trivial Option<str> MUST emit RC ops or drop functions:\n{ir}"
    );
}

/// §02 negative pin: non-trivial `Result<int, str>` MUST emit RC ops.
/// The `Err` variant payload is `str` (non-trivial).
#[test]
fn test_nontrivial_result_int_str_has_rc_ops() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/arc/nontrivial_result_int_str_has_rc_ops.ori"
    ));
    assert!(
        ir.contains("ori_rc_dec") || ir.contains("_ori_drop$"),
        "Non-trivial Result<int, str> MUST emit RC ops or drop functions:\n{ir}"
    );
}
