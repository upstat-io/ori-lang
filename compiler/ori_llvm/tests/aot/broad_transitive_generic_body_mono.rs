//! AOT pins for the BROAD transitive-generic-body monomorphization gap
//! (ROOT-B): a generic function whose body calls a trait method, a passed
//! closure, or a generic constructor on its OWN rigid type param, invoked from
//! ANOTHER generic body with a concrete instantiation. The outer generic fn
//! records its own `MonoInstance`, but the nested call stays type-checked
//! against rigid `T` and is never recorded per concrete instantiation, so at
//! true AOT (`ori build` -> native binary) the dispatch path
//! (`arc_emitter/apply.rs::resolve_callee` -> `lookup_mono_dispatch`,
//! `mono_instance_id = None` fallback) starves and emission aborts with
//! `error[E5001]`: `unresolved function 'X' in apply -- missing mono
//! instance?`. The interpreter computes the value (dynamic dispatch); the
//! dual-execution divergence IS the regression guard.
//!
//! These cells drive the REAL `ori build` production entry point
//! (`compile_and_run_capture` shells out to the workspace `ori` binary and runs
//! the native binary under `ORI_CHECK_LEAKS=1`): L8 AOT, L12 production entry
//! point, L10 leak-freedom. The interpreter (`ori run`) passes for every
//! FAIL-FIRST fixture — the parity break is the gap.
//!
//! FAIL-FIRST cells (missing mono instance pre-fix; green once nested calls are
//! recorded per concrete instantiation): `assert_eq`, `empty_queue`, `dequeue`,
//! `buffer_pop`. They are live red pins, not `#[ignore]`d — the red state is the
//! TDD gate. The CURED guard cell (`repeat`) passes pre-fix and pins an
//! already-resolved face of the same shape against regression.
//!
//! Out of scope here:
//! - `thread_id` (`#[ignore]`'d against BUG-04-229): the prelude builtin
//!   `thread_id()` has no `ori_rt`/LLVM AOT lowering — it fails E5001 even in a
//!   DIRECT non-generic call, so it is NOT the transitive-generic-body mono gap.
//!   `get_id<int>` monomorphizes correctly after the cure; the nested
//!   `thread_id()` call simply has no AOT target. Separate deliverable.
//! - `repeat_value` (const-only generic method `@m<$N: int>`): its AOT miss is
//!   the const-only-method monomorphization gate (the `tests/spec` corpus marks
//!   it `#skip("BUG-02-023: ...")`), and it ALSO fails at interp with E6020 —
//!   not a clean interp-pass / AOT-miss parity break. Distinct root, own bug.
//! - The closure (`cb`) and trait-method-chain (`transform`) faces of this
//!   shape are AOT-resolved today and already pinned at the spec-corpus level
//!   by `tests/spec/generics/transitive_generic_body_mono.ori`.
//! - The generic-constructor face (`Box.create` / `[T]` builder): AOT codegen
//!   RESOLVES the nested mono instance (no missing-mono), but the emitted binary
//!   trips a runtime RC double-free (SIGABRT) on the burden-coexistence floor —
//!   an `ori_arc`-owned concern, not this monomorphization gap. Not pinned here.

use crate::util::assert_aot_success;

// ---------------------------------------------------------------------------
// FAIL-FIRST — `assert_eq` cluster. A generic struct (`Box<T>`, deriving
// `Eq + Debug`) passed to `assert_eq` inside `check_box<T>`; the nested
// `assert_eq<Box<int>>` is never recorded against the concrete `Box<int>`.
// Pre-fix: AOT E5001 `unresolved function 'assert_eq'`. Interp passes.
// ---------------------------------------------------------------------------
#[test]
fn assert_eq_on_generic_struct_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/assert_eq.ori"),
        "broad_transitive_assert_eq",
    );
}

// ---------------------------------------------------------------------------
// FAIL-FIRST — `empty_queue` cluster (generic constructor on rigid `U`).
// `make_empty<U>` calls `empty_queue<U>`; nested `empty_queue<int>` unrecorded.
// Pre-fix: AOT E5001 `unresolved function 'empty_queue'`. Interp passes.
// ---------------------------------------------------------------------------
#[test]
fn empty_queue_generic_ctor_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/empty_queue.ori"),
        "broad_transitive_empty_queue",
    );
}

// ---------------------------------------------------------------------------
// FAIL-FIRST — `dequeue` cluster (generic fn on rigid `U`). `drain<U>` calls
// `dequeue<U>`; nested `dequeue<int>` unrecorded. Pre-fix: AOT E5001
// `unresolved function 'dequeue'`. Interp passes (Some(42) -> exit 0).
// ---------------------------------------------------------------------------
#[test]
fn dequeue_generic_fn_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/dequeue.ori"),
        "broad_transitive_dequeue",
    );
}

// ---------------------------------------------------------------------------
// FAIL-FIRST — `buffer_pop` cluster (generic fn on rigid `U`). `pop_one<U>`
// calls `buffer_pop<U>`; nested `buffer_pop<int>` unrecorded. Pre-fix: AOT
// E5001 `unresolved function 'buffer_pop'`. Interp passes (None -> exit 0).
// ---------------------------------------------------------------------------
#[test]
fn buffer_pop_generic_fn_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/buffer_pop.ori"),
        "broad_transitive_buffer_pop",
    );
}

// ---------------------------------------------------------------------------
// SEPARATE ROOT (BUG-04-229) — `thread_id` is a prelude builtin registered
// ONLY as an interpreter `function_val` (no `ori_rt` runtime symbol, no LLVM
// builtin-call lowering). It fails AOT with E5001 `unresolved function
// 'thread_id'` even in a DIRECT non-generic call (`@main = { thread_id() }`),
// so it is NOT the transitive-generic-body mono gap (ROOT-B): `get_id<int>`
// now monomorphizes correctly after the s-a86e53ac cures, but the
// `thread_id()` call inside its body has no AOT target. Ignored against
// BUG-04-229 (prelude-builtin AOT codegen lowering) until that lands.
// ---------------------------------------------------------------------------
#[test]
#[ignore = "BUG-04-229: prelude builtin thread_id() has no AOT/LLVM lowering — separate root from the ROOT-B transitive-mono gap; fails AOT even non-generically"]
fn thread_id_nested_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/thread_id.ori"),
        "broad_transitive_thread_id",
    );
}

// ---------------------------------------------------------------------------
// CURED guard — prelude generic `repeat` (T: Clone) on rigid `U`. Passes
// pre-fix; pins the already-resolved face against regression.
// ---------------------------------------------------------------------------
#[test]
fn repeat_prelude_generic_in_generic_body_stays_resolved() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/repeat_guard.ori"),
        "broad_transitive_repeat_guard",
    );
}
