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

// ----- EnumVariant-family positive pins: the root-collector widening admits
// fresh sum-aggregate `CtorKind::EnumVariant` Constructs into the no-sink +
// dead-param modes. Spec: Annex E §AIMS RL-2 + RL-4 + RL-5. -----

/// EV pin 1: the crasher. A fresh recursive `Tree = Leaf | Node([Tree])` borrowed
/// into a recursive `sum_tree` (Scalar result), result used in a branch (no
/// dead-param sink). The bare inline dec freed the live tree before the borrowing
/// read; the variant drop-glue freed the [Tree] children, the recursive walk
/// re-inc'd freed buffers -> UAF/SIGSEGV. `EnumVariant` admission suppresses the
/// bare dec + claims the carrier for the Cat-2 per-edge release.
#[test]
fn recursive_tree_no_sink_releases_borrowed_arg_no_crash() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/recursive_tree_no_sink.ori"),
        "recursive_tree_no_sink_releases_borrowed_arg_no_crash",
    );
}

/// EV pin 3: a fresh non-recursive user sum `Shape = Circle(r) | Pair(xs:[int])`
/// `Pair` borrowed into a user fn returning int — the `EnumVariant` family beyond
/// the recursive case.
#[test]
fn user_sum_pair_no_sink_releases_borrowed_arg_no_leak() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/user_sum_pair_no_sink.ori"),
        "user_sum_pair_no_sink_releases_borrowed_arg_no_leak",
    );
}

/// EV pin 4: fresh builtin `Option<[int]>` (`Some([..])`) and `Result<[int], str>`
/// (`Ok([..])`) borrowed into user fns returning int — the niche/builtin
/// `EnumVariant` Constructs are covered.
#[test]
fn builtin_sum_payload_no_sink_releases_borrowed_arg_no_leak() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/builtin_sum_payload_no_sink.ori"),
        "builtin_sum_payload_no_sink_releases_borrowed_arg_no_leak",
    );
}

/// EV pin 5: dead-param mode unchanged. A fresh `EnumVariant` `Box = Wrap(xs:[int])`
/// borrowed into a may-unwind call inside a `catch`, threaded to a merge/return
/// DEAD block-param — the existing dead-param arm claims it (no regression from
/// the no-sink widening).
#[test]
fn enum_dead_param_mode_unchanged_no_leak() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/enum_dead_param_no_regression.ori"),
        "enum_dead_param_mode_unchanged_no_leak",
    );
}

/// EV pin 12: nesting does not confuse the family test. A fresh
/// `Option<Option<[int]>>` (`Some(Some([..]))`) borrowed into a user fn returning
/// int — the per-Construct family test claims the carrier regardless of depth.
#[test]
fn nested_enum_variant_no_sink_releases_borrowed_arg_no_leak() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/nested_enum_variant_no_sink.ori"),
        "nested_enum_variant_no_sink_releases_borrowed_arg_no_leak",
    );
}

/// EV pin 13a: multi-variant one-heap-payload, HEAP arm exercised. The
/// heap-carrying variant (`Bag(xs:[int])`) flows through the borrow — the claim
/// fires correctly on the heap path, releasing the [int] exactly once.
#[test]
fn multi_variant_heap_arm_releases_borrowed_arg_no_leak() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/multi_variant_heap_arm.ori"),
        "multi_variant_heap_arm_releases_borrowed_arg_no_leak",
    );
}

// ----- EnumVariant-family negative GREEN guards (must stay clean — no over-fire) -----

/// EV pin 8: non-Scalar-result carrier. The borrowing Invoke returns `[Tree]` (a
/// heap value, a possible same-alloc VIEW of the carrier) — the heap-result
/// carrier gate DECLINES the no-sink claim (dead-param mode only). The no-sink
/// scan correctly declines (toggle-proven: disabling the lineage scan does not
/// change the outcome), but this shape hits an ORTHOGONAL pre-existing
/// double-free in RC codegen for a borrowed-Invoke `[Tree]` result. The carrier
/// decline is unit-pinned by `no_sink_declines_heap_result_carrier`.
#[test]
#[ignore = "BUG-04-164: borrowed-Invoke recursive-sum-list [Tree] result double-frees (orthogonal to the no-sink scan)"]
fn non_scalar_result_carrier_declined_clean() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/non_scalar_result_carrier.ori"),
        "non_scalar_result_carrier_declined_clean",
    );
}

/// EV pin 10: LIVE Project-extract. A same-alloc `Project` view of a lineage
/// member is read AFTER the carrier on the normal successor — the Project-extract
/// decline gate DECLINES the no-sink claim (a release would double-free the buffer
/// the extract holds). Leak-free via the base path. Must stay clean.
#[test]
fn live_project_extract_declined_clean() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/live_project_extract_decline.ori"),
        "live_project_extract_declined_clean",
    );
}

/// EV pin 11: closure-capture decline. The borrowed sum's payload is CAPTURED
/// into a closure (`PartialApply` member use) — the closure-vet (gate d) declines
/// any `PartialApply` / `Set` / `SetTag` / COW-machinery member. The no-sink scan
/// correctly declines (toggle-proven), but this shape hits an ORTHOGONAL
/// pre-existing double-free in RC codegen for a closure capturing a niche-payload
/// Project extract. The vet decline is unit-pinned by
/// `vetted_closure_declines_owned_position_consume`.
#[test]
#[ignore = "BUG-04-165: closure capturing a niche-payload Project extract double-frees (orthogonal to the no-sink scan)"]
fn mutation_in_closure_declined_clean() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/mutation_in_closure_decline.ori"),
        "mutation_in_closure_declined_clean",
    );
}

/// EV pin 13b: multi-variant one-heap-payload, NON-heap arm exercised. The
/// heap-free variant (`Tag(n:int)`) flows through — no over-suppression, no
/// spurious release on the non-heap path. Must stay clean.
#[test]
fn multi_variant_nonheap_arm_unperturbed_clean() {
    assert_aot_success(
        include_str!("fixtures/borrowed_invoke_leak/multi_variant_nonheap_arm.ori"),
        "multi_variant_nonheap_arm_unperturbed_clean",
    );
}

/// EV semantic pin: `ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE=1` restores the
/// recursive-tree crasher — proves the no-sink edge-death mode (now reaching
/// `EnumVariant` roots) is the cure surface for the bare-dec UAF.
#[test]
fn toggle_disables_release_recursive_tree_crashes_again() {
    use crate::util::compile_and_run_with_build_env;
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(
        include_str!("fixtures/borrowed_invoke_leak/recursive_tree_no_sink.ori"),
        &[
            ("ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE", "1"),
            // Isolate the lineage-release bisection axis: the borrowed-Invoke-arg
            // dec relocation independently covers this shape, so it is disabled
            // here.
            ("ORI_DISABLE_BORROWED_INVOKE_ARG_DEC_RELOCATION", "1"),
        ],
    );
    assert_ne!(
        exit, 0,
        "with the borrowed-Invoke lineage release disabled, the recursive-tree pin \
         must regress (crash/leak, exit != 0)\nstderr:\n{stderr}"
    );
}

// ----- Original (collection-family) positive pins (the no-sink edge-death cure) -----

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
        &[
            ("ORI_DISABLE_BORROWED_INVOKE_LINEAGE_RELEASE", "1"),
            // Isolate the lineage-release bisection axis: the borrowed-Invoke-arg
            // dec relocation independently covers this shape, so it is disabled
            // here.
            ("ORI_DISABLE_BORROWED_INVOKE_ARG_DEC_RELOCATION", "1"),
        ],
    );
    assert_eq!(
        exit, 2,
        "with the borrowed-Invoke lineage release disabled, pin-1 must leak (exit 2)\nstderr:\n{stderr}"
    );
}
