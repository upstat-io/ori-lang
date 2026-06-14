//! Error Handling AOT Tests
//!
//! Tests for ? operator, Result/Option chaining, error propagation through
//! multiple function calls, error handling in loops, and combined
//! Result+Option patterns.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, assert_panic_exit, compile_and_run_capture};

// ─── Result: basic patterns ───

#[test]
fn test_err_result_ok_unwrap() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_ok_unwrap.ori"),
        "err_result_ok",
    );
}

#[test]
fn test_err_result_err_check() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_err_check.ori"),
        "err_result_err",
    );
}

#[test]
fn test_err_result_unwrap_or_ok() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_unwrap_or_ok.ori"),
        "err_result_unwrap_or_ok",
    );
}

#[test]
fn test_err_result_unwrap_or_err() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_unwrap_or_err.ori"),
        "err_result_unwrap_or_err",
    );
}

// ─── Option: basic patterns ───

#[test]
fn test_err_option_some_unwrap() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_option_some_unwrap.ori"),
        "err_option_some",
    );
}

#[test]
fn test_err_option_none_check() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_option_none_check.ori"),
        "err_option_none",
    );
}

#[test]
fn test_err_option_unwrap_or_some() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_option_unwrap_or_some.ori"),
        "err_option_unwrap_or_some",
    );
}

#[test]
fn test_err_option_unwrap_or_none() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_option_unwrap_or_none.ori"),
        "err_option_unwrap_or_none",
    );
}

// ─── ? operator: Result ───

#[test]
fn test_err_try_result_ok() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_result_ok.ori"),
        "err_try_result_ok",
    );
}

#[test]
fn test_err_try_result_err() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_result_err.ori"),
        "err_try_result_err",
    );
}

#[test]
fn test_err_try_result_chain() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_result_chain.ori"),
        "err_try_chain",
    );
}

#[test]
fn test_err_try_result_early_exit() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_result_early_exit.ori"),
        "err_try_early_exit",
    );
}

// ─── ? operator: Option ───

#[test]
fn test_err_try_option_some() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_option_some.ori"),
        "err_try_option_some",
    );
}

#[test]
fn test_err_try_option_none() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_option_none.ori"),
        "err_try_option_none",
    );
}

// ─── Result with conditional logic ───

#[test]
fn test_err_result_conditional() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_conditional.ori"),
        "err_result_conditional",
    );
}

#[test]
fn test_err_result_chain_conditional() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_chain_conditional.ori"),
        "err_chain_conditional",
    );
}

// ─── Deep ? chains ───

#[test]
fn test_err_deep_try_chain() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_deep_try_chain.ori"),
        "err_deep_try",
    );
}

// ─── Result in loops ───

#[test]
fn test_err_result_in_loop() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_in_loop.ori"),
        "err_result_in_loop",
    );
}

// ─── Option patterns ───

#[test]
fn test_err_option_chain_unwrap() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_option_chain_unwrap.ori"),
        "err_option_chain",
    );
}

// ─── Mixed Result + Option ───

#[test]
fn test_err_result_with_option_payload() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_with_option_payload.ori"),
        "err_result_option",
    );
}

// ─── Return type variants ───

#[test]
fn test_err_result_int_err() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_int_err.ori"),
        "err_result_int_err",
    );
}

// ─── catch(expr:): panic recovery ───

#[test]
fn test_catch_panic_returns_err() {
    // Suppress stderr (ori_panic prints the panic message before unwinding).
    let (exit_code, _stdout, _stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_panic_returns_err.ori"
    ));
    assert_eq!(exit_code, 0, "catch should produce Err on panic");
}

#[test]
fn test_catch_success_returns_ok() {
    assert_aot_success(
        include_str!("fixtures/error_handling/catch_success_returns_ok.ori"),
        "catch_success",
    );
}

#[test]
fn test_catch_ok_unwrap_value() {
    assert_aot_success(
        include_str!("fixtures/error_handling/catch_ok_unwrap_value.ori"),
        "catch_ok_unwrap",
    );
}

#[test]
fn test_catch_simple_expression() {
    assert_aot_success(
        include_str!("fixtures/error_handling/catch_simple_expression.ori"),
        "catch_simple_expr",
    );
}

#[test]
fn test_catch_panic_explicit() {
    let (exit_code, _stdout, _stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_panic_explicit.ori"
    ));
    assert_eq!(exit_code, 0, "catch should capture explicit panic");
}

#[test]
fn test_catch_multiple_independent() {
    let (exit_code, _stdout, _stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_multiple_independent.ori"
    ));
    assert_eq!(exit_code, 0, "independent catches should work");
}

#[test]
fn test_catch_in_conditional() {
    let (exit_code, _stdout, _stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_in_conditional.ori"
    ));
    assert_eq!(exit_code, 0, "catch with conditional panics");
}

// ─── panic-message ownership: ori_panic owns + releases its message ───

/// Regression: a heap panic message aliased past the catch boundary must
/// survive the caught panic (the panic transfer dup-incs the still-live
/// message; the runtime releases only the transferred reference).
#[test]
fn test_catch_panic_aliased_message_survives_catch() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_panic_aliased_message_survives.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "aliased message must stay valid after the caught panic, leak-clean:\n{stderr}"
    );
}

/// Regression: nested catches each recover their own heap panic message;
/// every message buffer is released exactly once (no leak, no double-free).
#[test]
fn test_nested_catch_panic_heap_messages_release_once() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/nested_catch_panic_heap_messages.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "nested catch must recover both messages, leak-clean:\n{stderr}"
    );
}

/// Over-fire negative: an SSO panic message has no heap buffer — the
/// runtime release must no-op (no corruption, no spurious free).
#[test]
fn test_catch_panic_sso_message_no_heap_release() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_panic_sso_message.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "SSO message catch must stay clean (release no-ops on SSO):\n{stderr}"
    );
}

/// Over-fire negative: the uncaught-panic exit path is unchanged — panic
/// exit code, message on stderr, and the message buffer released before
/// the unwind (leak-clean even though the process is exiting).
#[test]
fn test_uncaught_panic_heap_message_exit_unchanged() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/uncaught_panic_heap_message.ori"
    ));
    assert_panic_exit(exit_code, "uncaught heap-message panic", &stderr);
    assert!(
        stderr.contains("uncaught heap panic message"),
        "panic message must reach stderr intact:\n{stderr}"
    );
    assert!(
        !stderr.contains("not freed"),
        "uncaught panic path must not leak the message buffer:\n{stderr}"
    );
}

/// Over-fire negative: `expect` panics route the user message through the
/// inline panic emission (manufactured transfer inc) — the panic exit and
/// message are unchanged.
#[test]
fn test_expect_panic_heap_message_exit_unchanged() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/expect_panic_heap_message_uncaught.ori"
    ));
    assert_panic_exit(exit_code, "uncaught expect panic", &stderr);
    assert!(
        stderr.contains("expect failure message"),
        "expect panic message must reach stderr intact:\n{stderr}"
    );
}

// catch(expr:) catchability parity: may-panic builtins (eval == AOT)

/// Parity: `expect` on None inside `catch` returns Err carrying the user
/// message — the interpreter catches this; AOT must too (Spec: Clause 17.4).
#[test]
fn test_catch_expect_none_returns_err_with_message() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_expect_none_returns_err.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "catch must capture the in-frame expect panic with the message intact, leak-clean:\n{stderr}"
    );
}

/// Parity: `unwrap` on None inside `catch` returns Err (Spec: Clause 17.4).
#[test]
fn test_catch_unwrap_none_returns_err() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_unwrap_none_returns_err.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "catch must capture the in-frame unwrap panic:\n{stderr}"
    );
}

/// Parity: `Result.unwrap` on Err inside `catch` returns Err.
#[test]
fn test_catch_result_unwrap_err_returns_err() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_result_unwrap_err_returns_err.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "catch must capture the in-frame Result.unwrap panic:\n{stderr}"
    );
}

/// Parity: `Result.unwrap_err` on Ok inside `catch` returns Err.
#[test]
fn test_catch_unwrap_err_on_ok_returns_err() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_unwrap_err_on_ok_returns_err.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "catch must capture the in-frame unwrap_err panic:\n{stderr}"
    );
}

/// Parity + RC pin: `Result.expect` on Err with a heap message and heap
/// payload — caught message content intact, leak-clean.
#[test]
fn test_catch_result_expect_heap_message_caught() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_result_expect_heap_message.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "catch must capture Result.expect with heap message + payload, leak-clean:\n{stderr}"
    );
}

/// Parity: list index OOB inside `catch` returns Err — implicit panics are
/// catchable (Spec: Clause 17.2.4).
#[test]
fn test_catch_index_oob_returns_err() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_index_oob_returns_err.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "catch must capture the in-frame index-OOB panic:\n{stderr}"
    );
}

/// RC pin: the indexed list (heap str elements) survives the caught OOB
/// panic — the unwind-edge cleanup must not release a value still live
/// after the catch.
#[test]
fn test_catch_index_oob_str_elements_survive_no_leak() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_index_oob_str_elements_no_leak.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "list must survive the caught OOB panic, leak-clean:\n{stderr}"
    );
}

/// RC pin: a constructed (definitely-heap) expect message dead after the
/// call is released exactly once across the caught panic path.
#[test]
fn test_catch_expect_constructed_message_no_leak() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_expect_constructed_message_no_leak.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "constructed expect message released exactly once across the caught panic:\n{stderr}"
    );
}

/// Over-fire negative: non-panicking `expect` inside `catch` returns Ok
/// with the payload — the hot path is behaviorally unchanged.
#[test]
fn test_catch_expect_some_returns_ok() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_expect_some_returns_ok.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "non-panicking expect inside catch returns Ok(payload):\n{stderr}"
    );
}

/// Over-fire negative: in-bounds index inside `catch` returns Ok.
#[test]
fn test_catch_index_inbounds_returns_ok() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_index_inbounds_returns_ok.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "in-bounds index inside catch returns Ok(element):\n{stderr}"
    );
}

/// Cross-frame regression pin: an OOB index inside a CALLED function is
/// caught by the caller's catch via the user-call Invoke edge (catchability
/// holds), AND the borrowed [int]-list arg buffer is freed on the caught path.
/// The fresh collection borrowed into the may-unwind user-call `Invoke`, whose
/// lineage dies at the catch-match's DEAD merge block-param, gets its sole RL-5
/// dead-at-entry release from the construct-fed-dead-param collection arm.
#[test]
fn test_catch_callee_index_oob_returns_err() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_callee_index_oob_returns_err.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "cross-frame index-OOB panic caught at the caller's catch + no buffer leak:\n{stderr}"
    );
}

/// Borrowed [int]-list arg at a may-unwind user-call `Invoke`, NORMAL in-bounds
/// path: the fresh list lineage dies at the catch-match's dead merge block-param;
/// the construct-fed-dead-param collection arm supplies the sole release so the
/// 24-byte buffer is freed (`ORI_CHECK_LEAKS=1` exit 0). Spec: Annex E §AIMS
/// RL-5 + RL-2.
#[test]
fn test_catch_callee_borrowed_intlist_normal_no_leak() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_callee_borrowed_intlist_normal_no_leak.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "borrowed [int]-list arg at may-unwind Invoke, normal path, no buffer leak:\n{stderr}"
    );
}

/// Borrowed [str]-list arg at a may-unwind user-call `Invoke`, NORMAL in-bounds
/// path: the heap-str-element buffer + its element strings are freed.
#[test]
fn test_catch_callee_borrowed_strlist_normal_no_leak() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_callee_borrowed_strlist_normal_no_leak.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "borrowed [str]-list arg at may-unwind Invoke, normal path, no leak:\n{stderr}"
    );
}

/// Borrowed heap-`str` arg (plus a list) at a may-unwind user-call `Invoke`,
/// NORMAL path: the str-literal-rooted lineage and the list lineage both die at
/// the catch-match dead block-param; both buffers are freed. Covers the
/// `Let { Literal::String }` heap-buffer root of the collection arm.
#[test]
fn test_catch_callee_borrowed_str_normal_no_leak() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_callee_borrowed_str_normal_no_leak.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "borrowed str + list args at may-unwind Invoke, normal path, no leak:\n{stderr}"
    );
}

/// Borrowed [str]-list arg at a may-unwind user-call `Invoke`, CAUGHT-panic
/// (OOB) path: the buffer is freed on the recovered path.
#[test]
fn test_catch_callee_borrowed_strlist_caught_no_leak() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_callee_borrowed_strlist_caught_no_leak.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "borrowed [str]-list arg at may-unwind Invoke, caught path, no leak:\n{stderr}"
    );
}

/// Over-fire negative pin: a fresh list borrowed into a catch AND iter-consumed
/// (`fold`) at the SAME single call site stays freed without double-free. The
/// construct-fed-dead-param collection arm's call-site-count ≤ 1 gate admits
/// this (single call), distinct from the live-across multi-call shape that the
/// gate declines (proven by `fat_ptr_iter::unwind::test_unwind_list_reusable_after_catch`).
#[test]
fn test_catch_callee_iterconsume_no_regression() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_callee_iterconsume_no_regression.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "fresh list borrowed into a single catch'd fold, no leak / no double-free:\n{stderr}"
    );
}

/// Nested-catch pin: the builtin panic unwinds to the INNERMOST handler
/// (catch-target save/restore holds for intercepted emissions).
#[test]
fn test_catch_nested_inner_expect_caught_by_inner() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_nested_inner_expect.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "inner catch captures the expect panic; outer sees Ok:\n{stderr}"
    );
}

/// Over-fire negative: the uncaught index-OOB exit path is unchanged —
/// panic exit code and message on stderr.
#[test]
fn test_index_oob_uncaught_exit_unchanged() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/index_oob_uncaught_exit_unchanged.ori"
    ));
    assert_panic_exit(exit_code, "uncaught index-OOB panic", &stderr);
    assert!(
        stderr.contains("out of bounds"),
        "OOB panic message must reach stderr intact:\n{stderr}"
    );
}

// ─── borrowed-Invoke-terminator-arg successor-edge release (Spec: Clause 17.2.4) ───

/// Normal-path correctness: an in-bounds `xs[1]` flows through the converted
/// Invoke carrier and the receiver is read again after the catch — the
/// per-successor-edge release must NOT fire on the normal edge where the
/// receiver stays live (exit 0, zero leaks). Spec: Annex E §AIMS RL-2 + RL-4.
#[test]
fn test_catch_index_inbounds_receiver_reused_no_leak() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_index_inbounds_receiver_reused_no_leak.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "in-bounds index via Invoke carrier, receiver live after, no leak:\n{stderr}"
    );
}

/// Per-succ liveness filter: a user may-unwind callee with a borrowed receiver
/// read on the NORMAL path only — the successor release lands only where the
/// receiver dies, not on the path it stays live (exit 0, zero leaks).
/// Spec: Annex E §AIMS RL-2 + RL-4.
#[test]
fn test_catch_user_borrowed_invoke_normal_only_receiver() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_user_borrowed_invoke_normal_only_receiver.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "borrowed receiver read on the normal path only, no leak / no double-free:\n{stderr}"
    );
}

/// Over-fire clamp: `catch(expr: xs[9]); xs.len()` — the receiver is LIVE after
/// the catch, so the successor release must NOT be emitted on edges where the
/// receiver stays live (exit 0, no double-free). Spec: Annex E §AIMS RL-4.
#[test]
fn test_catch_index_oob_receiver_reused_after_catch() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_index_oob_receiver_reused_after_catch.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "receiver live after the caught OOB panic, no double-free:\n{stderr}"
    );
}

/// Element-type dimension: same-frame OOB catch on a `[bool]` receiver — the
/// element type carries no heap RC, but the receiver buffer does (exit 0).
/// Spec: Clause 17.2.4.
#[test]
fn test_catch_index_oob_bool_elements() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_index_oob_bool_elements.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "OOB catch on a [bool] receiver, no leak:\n{stderr}"
    );
}

/// Element-type dimension: same-frame OOB catch on an `[Option<str>]` receiver —
/// the heap str elements survive the caught panic (exit 0, zero leaks).
/// Spec: Clause 17.2.4.
#[test]
fn test_catch_index_oob_option_str_elements() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_index_oob_option_str_elements.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "OOB catch on an [Option<str>] receiver, heap elements survive:\n{stderr}"
    );
}

/// Negative control: a map index miss returns `Option::None` and NEVER panics,
/// so the `catch` sees `Ok(None)` with no unwind carrier emitted (exit 0). Proves
/// the conversion does not give map `__index` a spurious unwind edge.
/// Spec: Clause 17.2.4.
#[test]
fn test_catch_map_index_miss_no_unwind_carrier() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_map_index_miss_no_unwind_carrier.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "map index miss returns Ok(None) with no spurious unwind:\n{stderr}"
    );
}

/// Unwind-edge-only release: a receiver read after a successful catch is LIVE on
/// the normal path and DEAD on the unwind path — the release must land ONLY on
/// the unwind edge (exit 0 on both outcomes, zero leaks).
/// Spec: Annex E §AIMS RL-4.
#[test]
fn test_catch_index_receiver_live_normal_dead_unwind() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_index_receiver_live_normal_dead_unwind.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "receiver live on normal, dead on unwind — release on the unwind edge only:\n{stderr}"
    );
}

/// Merge-successor double-free clamp: two Invokes sharing one merge successor
/// (sequential catches into one merge) — split-edge placement releases the dying
/// borrowed arg per executing path; a front-of-merge dec would double-free
/// (exit 0, zero leaks). Spec: Annex E §AIMS RL-4.
#[test]
fn test_catch_two_index_oob_shared_merge() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_two_index_oob_shared_merge.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "two catches into one merge, split-edge release, no double-free:\n{stderr}"
    );
}

// ─── BUG-04-159: same-frame catch of checked-op implicit panics (AOT) ───
// Regression family: emit_panic_block emitted a plain call + unreachable (no
// invoke), so the unwind from ori_panic_cstr escaped the in-frame catch
// landing pad. catch(...) must catch div-zero / mod-zero / overflow / shift.
// (Also closes BUG-04-087 and BUG-07-038 — duplicates of the same defect.)

#[test]
fn test_catch_div_by_zero_inline_returns_err() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_catch_div_by_zero.ori"),
        "catch_div_by_zero_inline",
    );
}

#[test]
fn test_catch_mod_by_zero_inline_returns_err() {
    assert_aot_success(
        include_str!("fixtures/error_handling/catch_mod_by_zero.ori"),
        "catch_mod_by_zero_inline",
    );
}

#[test]
fn test_catch_int_overflow_inline_returns_err() {
    assert_aot_success(
        include_str!("fixtures/error_handling/catch_int_overflow.ori"),
        "catch_int_overflow_inline",
    );
}

#[test]
fn test_catch_shift_overflow_inline_returns_err() {
    assert_aot_success(
        include_str!("fixtures/error_handling/catch_shift_overflow.ori"),
        "catch_shift_overflow_inline",
    );
}

// Negative pin: div-by-zero with NO catch in scope MUST still abort — the
// fix adds an unwind edge ONLY when a catch target is in scope.
#[test]
fn test_uncaught_div_by_zero_still_aborts() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/uncaught_div_zero_aborts.ori"
    ));
    assert_panic_exit(exit_code, "uncaught_div_zero_still_aborts", &stderr);
}

// Over-fire guard: a value live AFTER a caught checked-op panic must not be
// released on the unwind edge (mirrors BUG-04-153 receiver-live guard).
#[test]
fn test_catch_div_by_zero_receiver_live_after_no_leak() {
    assert_aot_success(
        include_str!("fixtures/error_handling/catch_div_zero_receiver_live_after.ori"),
        "catch_div_zero_receiver_live_after",
    );
}
