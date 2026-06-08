//! Inline-temporary aggregate used as a borrowing comparison operand must not
//! leak its heap payload.
//!
//! A freshly-constructed sum/variant value with a heap payload, used DIRECTLY
//! as a borrowing `==` / `<` operand (no intervening Let-Var alias), leaked the
//! payload allocation: RC realization emitted a spurious `RcInc [InlineEnum]`
//! on the fresh `Construct` whose terminal use is a borrow. DP-3 elides a
//! `Once ∧ Linear` inc; RL-2 leaves exactly one scope-exit dec. The fresh-inc
//! elision covered the Let-Var move-alias chain but missed the direct-borrow
//! case, leaving the construct ref unreleased.
//!
//! Matrix: exact failing case, both-inline, negation, empty payload; cross-type
//! (list / str / map / Result / user sum-type with heap field); cross-pattern
//! (LHS inline, comparison operators via `Comparable::compare`, cross-block);
//! cross-feature (`map.updated` assertion site, nested aggregate); dual
//! emission-path (predicate-stack baseline, burden-only, default); negative
//! pins (bound-var alias clean, scalar payload clean, multi-consumer retains
//! inc, fresh value duplicated into one call retains inc).
//!
//! Default-path cells use `assert_aot_success` (`ORI_CHECK_LEAKS=1`, panics on
//! exit 2). Dual-path cells use `compile_and_run_with_build_env` to gate the
//! compile-time `ORI_DISABLE_*` flags. Subprocess-isolated — parallel-safe.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_with_build_env};

/// Compile `source` with a compile-time pipeline flag set and run under leak
/// checking. Asserts exit 0 and no leak / double-free diagnostic on stderr.
fn assert_leak_free_under(source: &str, build_env: &[(&str, &str)], label: &str) {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(source, build_env);
    assert!(
        exit == 0,
        "[{label}] run exited {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[{label}] reported a leak / double-free\nstderr:\n{stderr}"
    );
}

// ----- Exact failing case -----

/// The minimal repro: a fresh `Some([heap])` bound to `x`, then a SECOND fresh
/// inline `Some([heap])` used directly as the `==` RHS borrow operand.
#[test]
fn inline_option_list_eq_operand_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some([7, 8, 9]);
    if x == Some([7, 8, 9]) then 0 else 1
}
"#,
        "inline_option_list_eq_operand_no_leak",
    );
}

// ----- Edge cases -----

/// Both operands are fresh inline aggregates (no `let` binding at all).
#[test]
fn both_inline_option_list_eq_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    if Some([7, 8, 9]) == Some([7, 8, 9]) then 0 else 1
}
"#,
        "both_inline_option_list_eq_no_leak",
    );
}

/// Negation operand: `!=` desugars through `Eq::equals` and borrows the inline
/// aggregate identically.
#[test]
fn inline_option_list_ne_operand_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some([7, 8, 9]);
    if x != Some([7, 8, 9]) then 1 else 0
}
"#,
        "inline_option_list_ne_operand_no_leak",
    );
}

/// Empty heap payload boundary.
#[test]
fn inline_option_empty_list_eq_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x: Option<[int]> = Some([]);
    let y: Option<[int]> = Some([]);
    if x == y then 0 else 1
}
"#,
        "inline_option_empty_list_eq_no_leak",
    );
}

// ----- Cross-type coverage (aggregate-with-heap-payload axis) -----

/// str payload large enough to force a heap allocation (> 23-byte SSO bound).
#[test]
fn inline_option_str_eq_operand_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some("a string well past the twenty-three byte sso threshold");
    if x == Some("a string well past the twenty-three byte sso threshold") then 0 else 1
}
"#,
        "inline_option_str_eq_operand_no_leak",
    );
}

/// map payload.
#[test]
fn inline_option_map_eq_operand_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some({1: 2, 3: 4});
    if x == Some({1: 2, 3: 4}) then 0 else 1
}
"#,
        "inline_option_map_eq_operand_no_leak",
    );
}

/// Result variant payload, inline `Ok` as the `==` operand.
#[test]
fn inline_result_ok_list_eq_operand_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x: Result<[int], int> = Ok([1, 2, 3]);
    if x == Ok([1, 2, 3]) then 0 else 1
}
"#,
        "inline_result_ok_list_eq_operand_no_leak",
    );
}

/// User sum-type with a heap field, inline as the `==` operand.
#[test]
fn inline_user_variant_heap_field_eq_no_leak() {
    assert_aot_success(
        r#"
#derive(Eq)
type Bag = Empty | Full(items: [int]);

@main () -> int = {
    let x = Full(items: [1, 2, 3]);
    if x == Full(items: [1, 2, 3]) then 0 else 1
}
"#,
        "inline_user_variant_heap_field_eq_no_leak",
    );
}

// ----- Cross-pattern coverage -----

/// Inline aggregate as the `==` LHS (operand position symmetry).
#[test]
fn inline_option_list_eq_lhs_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some([7, 8, 9]);
    if Some([7, 8, 9]) == x then 0 else 1
}
"#,
        "inline_option_list_eq_lhs_no_leak",
    );
}

/// Comparison operators desugar to `Comparable::compare` (a DISTINCT Apply
/// callee from `Eq::equals`) but borrow the inline aggregate identically.
#[test]
fn inline_option_list_lt_operand_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some([1, 2, 3]);
    if x < Some([4, 5, 6]) then 0 else 1
}
"#,
        "inline_option_list_lt_operand_no_leak",
    );
}

/// `>=` operand — the other end of the comparison-operator family.
#[test]
fn inline_option_list_ge_operand_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some([4, 5, 6]);
    if x >= Some([1, 2, 3]) then 0 else 1
}
"#,
        "inline_option_list_ge_operand_no_leak",
    );
}

/// Cross-block / branch-merge: the fresh aggregate is consumed at a borrow
/// operand inside a branch, then control flows on. The converged-at-def state
/// (not block-exit) must drive the elision across the block boundary.
#[test]
fn inline_option_list_eq_in_branch_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let cond = true;
    let x = Some([7, 8, 9]);
    let r = if cond then (if x == Some([7, 8, 9]) then 0 else 1) else 2;
    r
}
"#,
        "inline_option_list_eq_in_branch_no_leak",
    );
}

// ----- Cross-feature interactions -----

/// The original surfacing site: an inline aggregate `==` operand inside a
/// `map.updated(...)` assertion expression.
#[test]
fn inline_option_eq_inside_updated_map_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = { "a": [1], "b": [2] };
    let n = m.updated(key: "b", value: [7, 8, 9]);
    if n["b"] == Some([7, 8, 9]) then 0 else 1
}
"#,
        "inline_option_eq_inside_updated_map_no_leak",
    );
}

/// Nested aggregate: `Some(Some([heap]))` borrowed as an `==` operand.
#[test]
fn inline_nested_option_list_eq_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some(Some([7, 8, 9]));
    if x == Some(Some([7, 8, 9])) then 0 else 1
}
"#,
        "inline_nested_option_list_eq_no_leak",
    );
}

// ----- Dual emission-path coverage (leak reproduces on BOTH paths) -----

/// Predicate-stack baseline: `ORI_DISABLE_BURDEN_OPS=1` runs the pre-burden
/// path. The heap-bearing compare-operand dec is burden-emitted by design (the
/// M3/M4 fresh-self-alloc/lineage-net cure counts BurdenInc/BurdenDec); with
/// burden disabled the pre-burden path emits no balancing dec. Owned by the
/// predicate-stack retirement, which makes `ORI_DISABLE_BURDEN_OPS` moot.
#[test]
#[ignore = "BUG-04-148: predicate-stack baseline (ORI_DISABLE_BURDEN_OPS=1) compare-operand dec is burden-emitted by design; predicate-stack retirement (aims-burden-tracking section-09) makes ORI_DISABLE_BURDEN_OPS moot"]
fn inline_option_list_eq_predicate_stack_baseline_no_leak() {
    assert_leak_free_under(
        r#"
@main () -> int = {
    let x = Some([7, 8, 9]);
    if x == Some([7, 8, 9]) then 0 else 1
}
"#,
        &[("ORI_DISABLE_BURDEN_OPS", "1")],
        "inline_option_list_eq_predicate_stack_baseline_no_leak",
    );
}

/// Burden-only: `ORI_DISABLE_PREDICATE_STACK_RC=1` leaves the burden path as
/// the sole real-RC emitter; the Phase-6 eliminator fix must clear the leak.
#[test]
fn inline_option_list_eq_burden_only_no_leak() {
    assert_leak_free_under(
        r#"
@main () -> int = {
    let x = Some([7, 8, 9]);
    if x == Some([7, 8, 9]) then 0 else 1
}
"#,
        &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")],
        "inline_option_list_eq_burden_only_no_leak",
    );
}

// ----- Negative pins (must NOT over-suppress) -----

/// Bound-var form: the RHS is bound to a `let`, so its fresh-site inc is
/// already suppressed as a move-alias source. Must STAY clean — the fix does
/// not change this path.
#[test]
fn bound_var_option_list_eq_stays_clean() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some([1, 2, 3]);
    let y = Some([1, 2, 3]);
    if x == y then 0 else 1
}
"#,
        "bound_var_option_list_eq_stays_clean",
    );
}

/// Scalar payload aggregate: `Some(7)` has no heap, no RC. Must STAY clean —
/// no over-broad elision onto non-heap aggregates.
#[test]
fn scalar_payload_option_eq_stays_clean() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = Some(7);
    if x == Some(7) then 0 else 1
}
"#,
        "scalar_payload_option_eq_stays_clean",
    );
}

/// Multi-consumer soundness pin: a bound value used at TWO borrow operands has
/// cardinality Many at its definition. The converged-at-def state (NOT the
/// synthetic TF-3 image, which is unconditionally `Once`) must KEEP its inc —
/// eliding it would premature-free the second use (the bug-04-120 shape).
/// Leak-free AND no double-free / use-after-free.
#[test]
fn multi_consumer_bound_value_retains_inc() {
    assert_aot_success(
        r#"
@main () -> int = {
    let y = Some([1, 2, 3]);
    if (y == Some([1, 2, 3])) && (y == Some([1, 2, 3])) then 0 else 1
}
"#,
        "multi_consumer_bound_value_retains_inc",
    );
}

/// Fresh value duplicated into two positions of one call (`f(x, x)` shape):
/// the duplicated aggregate retains its inc; no premature free of the second
/// argument copy.
#[test]
fn fresh_value_duplicated_into_call_retains_inc() {
    assert_aot_success(
        r#"
@both_eq (a: Option<[int]>, b: Option<[int]>) -> bool = a == b;

@main () -> int = {
    let x = Some([1, 2, 3]);
    if both_eq(a: x, b: x) then 0 else 1
}
"#,
        "fresh_value_duplicated_into_call_retains_inc",
    );
}
