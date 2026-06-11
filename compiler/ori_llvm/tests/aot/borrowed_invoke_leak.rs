//! No-sink borrowed-`Invoke` collection leak — a fresh collection buffer passed
//! BORROWED into a may-unwind user-call `Invoke` whose receiver dies on the
//! call's successor edges with NO dead-param sink leaked the buffer.
//!
//! Mechanism: the base walk placed the receiver's `BurdenDec` INLINE before the
//! borrowing `Invoke` terminator; that dec netted with the fresh-site inc to
//! zero pre-call, so the rc=1 allocation was never released on any executing
//! successor path (a leak, not a UAF, by accident of the net-0 pair). Per RL-2
//! `RL2_borrowed_param_emits_caller_dec` the caller MUST release a borrowed arg
//! after its last read; per RL-4 `RL4_edge_release_balanced` the release lands
//! once on each dying successor edge.
//!
//! Cure: the no-sink EDGE-DEATH mode in the borrowed-`Invoke` lineage scan
//! suppresses the broken inline dec (removes the var from
//! `owned_vars_needing_rc`) + claims the var so the landed Category-2 per-edge
//! `deadAtSucc` emission frees it exactly once per executing path. Live-across
//! receivers (read past the call) release at the lineage's true post-call last
//! read.
//!
//! Matrix: single-param list-Eq, two-param both-leak, `[str]` element variant,
//! live-across read-after-call (positive pins); owned-param-consume,
//! main-level-fresh-both, Option-valued, iter-consume-decline,
//! non-collection-struct-root (negative GREEN guards). All cells run under
//! `ORI_CHECK_LEAKS=1` (`assert_aot_success` panics on exit 2). Subprocess-
//! isolated — parallel-safe. Spec: Annex E §AIMS RL-2 + RL-4.

use crate::util::assert_aot_success;

// ----- Positive pins (the no-sink edge-death cure) -----

/// Pin 1: single-param list-Eq. The minimal repro — a fresh `[int]` borrowed
/// into a may-unwind user call comparing it, dying on the call edges with no
/// dead-param sink.
#[test]
fn single_param_list_eq_releases_borrowed_arg_no_leak() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/single_param_eq.ori"),
        "single_param_list_eq_releases_borrowed_arg_no_leak",
    );
}

/// Pin 2: two-param both-leak. Both fresh `[int]` args are distinct allocations
/// dying on the call edges; each is released exactly once.
#[test]
fn two_param_list_eq_releases_both_borrowed_args_no_leak() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/two_param_eq.ori"),
        "two_param_list_eq_releases_both_borrowed_args_no_leak",
    );
}

/// Pin 3: `[str]` element variant — the heap-element buffer and its element
/// strings are released by the caller after the borrowed call.
#[test]
fn str_list_eq_releases_borrowed_arg_no_leak() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/str_list_eq.ori"),
        "str_list_eq_releases_borrowed_arg_no_leak",
    );
}

/// Pin 6b: live-across receiver read via `.len()` AFTER the first borrowed
/// call. The release lands at the lineage's TRUE post-call last read, not
/// before the first Invoke.
#[test]
fn live_across_read_releases_at_post_call_last_read_no_leak() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/live_across_read.ori"),
        "live_across_read_releases_at_post_call_last_read_no_leak",
    );
}

// ----- Negative GREEN guards (must stay clean — no over-fire) -----

/// Pin 7: an OWNED param indexed/len/summed in the callee — an
/// ownership-transfer consume, not a borrow-read. The no-sink mode must not
/// perturb it.
#[test]
fn owned_param_indexed_in_callee_clean() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/owned_param_indexed.ori"),
        "owned_param_indexed_in_callee_clean",
    );
}

/// Pin 8: main-level fresh-both-sides `[7,8] == [7,8]` — both operands are
/// caller-local fresh literals released at their own scope exit; no borrowed
/// user-call lineage. Must stay clean.
#[test]
fn main_level_fresh_both_sides_eq_clean() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/main_level_fresh_both.ori"),
        "main_level_fresh_both_sides_eq_clean",
    );
}

/// Pin 9: an Option-valued borrowed compare — a niche-family sum, not a
/// fresh-collection-Construct root, so the no-sink collection mode (gate a)
/// declines it. Must stay clean.
#[test]
fn option_valued_borrowed_compare_clean() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/option_valued.ori"),
        "option_valued_borrowed_compare_clean",
    );
}

/// Pin 10: an iter-consuming borrowed arg — the callee's `ori_iter_drop` frees
/// the buffer (RL-2 iter-consume transfer), so the caller emits NO dec. The
/// no-sink mode (gate c2) must DECLINE — a caller release here double-frees.
#[test]
fn iter_consuming_borrowed_arg_declined_clean() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/iter_consume_decline.ori"),
        "iter_consuming_borrowed_arg_declined_clean",
    );
}

/// Pin 11: a non-collection (struct) Construct root passed borrowed — the
/// struct `Construct` is not a ListLiteral/MapLiteral/SetLiteral, so the no-sink
/// collection mode (gate a) never makes it a candidate. Must stay clean.
#[test]
fn non_collection_struct_root_unperturbed_clean() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/struct_root_decline.ori"),
        "non_collection_struct_root_unperturbed_clean",
    );
}

// ----- Semantic pin: the toggle restores the leak (load-bearing) -----

/// Semantic pin: `ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE=1` restores the
/// pin-1 minimal leak — proves the no-sink edge-death mode is the cure surface.
/// The toggle gates the WHOLE scan (dead-param + no-sink modes), so disabling it
/// reverts the inline-before-terminator dec placement.
#[test]
fn toggle_disables_no_sink_release_pin1_leaks_again() {
    use crate::util::compile_and_run_with_build_env;
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(
        include_str!("fixtures/borrowed_invoke_leak/single_param_eq.ori"),
        &[("ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE", "1")],
    );
    assert_eq!(
        exit, 2,
        "with the borrowed-Invoke lineage release disabled, pin-1 must leak (exit 2)\nstderr:\n{stderr}"
    );
}
