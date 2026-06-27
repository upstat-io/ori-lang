//! AOT pins for the prelude builtin `thread_id()` codegen-lowering gap
//! (BUG-04-229): `thread_id` is registered driver-side as `() -> int` and as
//! an interpreter `function_val`, but had NO AOT/LLVM lowering — every AOT
//! call reached `arc_emitter/apply.rs::resolve_callee` unresolved and aborted
//! with `error[E5001]`: `unresolved function 'thread_id' in apply -- missing
//! mono instance?` (the generic apply-dispatch fallback string, NOT a real
//! monomorphization gap — the call has zero type params). The fix adds an
//! `ori_thread_id() -> i64` runtime symbol + a scalar `emit_thread_id` arm.
//!
//! `thread_id()` is non-deterministic per-thread, so these pins assert
//! AOT-compilation success + a STRUCTURAL property (non-negative; same-thread
//! calls return the same value) encoded as exit-code 0, NOT a fixed stdout
//! value (cross-process value-equality is unstable by design — Refinement B).
//!
//! FAIL-FIRST: every cell fails pre-fix with E5001 (no codegen lowering).

use crate::util::assert_aot_success;

/// Direct, non-generic call — the minimal repro. Pre-fix: E5001.
#[test]
fn thread_id_direct_non_generic_aot() {
    assert_aot_success(
        "@main () -> int = { let x = thread_id(); if x >= 0 then 0 else 1 }",
        "thread_id_direct_non_generic",
    );
}

/// Structural property: the id is non-negative (exit 0 iff `id >= 0`).
#[test]
fn thread_id_non_negative_aot() {
    assert_aot_success(
        "@main () -> int = { if thread_id() >= 0 then 0 else 1 }",
        "thread_id_non_negative",
    );
}

/// Structural property: same-thread calls return the same value (the
/// dual-exec-parity-relevant invariant — exit 0 iff `a == b`).
#[test]
fn thread_id_same_thread_stable_aot() {
    assert_aot_success(
        "@main () -> int = { let a = thread_id(); let b = thread_id(); if a == b then 0 else 1 }",
        "thread_id_same_thread_stable",
    );
}

/// Pattern cell: `thread_id()` consumed through a non-generic helper call.
#[test]
fn thread_id_through_helper_aot() {
    assert_aot_success(
        "@tid () -> int = thread_id();\n@main () -> int = { if tid() >= 0 then 0 else 1 }",
        "thread_id_through_helper",
    );
}
