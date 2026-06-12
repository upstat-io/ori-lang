//! Template-literal str-concat chain RC discipline on the heap-string path.
//!
//! Mechanism under test: every borrowed `@concat` arg in a template-literal
//! chain gets a Phase-5 fresh-site `BurdenInc` + inline last-use `BurdenDec`
//! placed BEFORE the consuming `Invoke`. When a heap concat intermediate
//! (running prefix > 23 bytes, past the SSO max) is live across the next
//! interpolation's may-unwind `@to_str` `Invoke`, the pair splits across
//! blocks and the Phase-7 position-blind fresh-inc elision strips the inc
//! while keeping the dec — rc hits 0 BEFORE the borrowed `@concat` read
//! (use-after-free; `copy_nonoverlapping` overlap abort in debug). A
//! sibling defect in the same lineage family leaks the heap final template
//! string (net +1, no normal-path release). Per RL-2
//! `RL2_borrowed_param_emits_caller_dec` the borrowed arg's release belongs
//! AFTER the consuming call; per RL-1 `RL1_emit_iff_not_elidable` an elided
//! inc must be genuinely surplus.
//!
//! Matrix: output length [23 = SSO max GREEN clamp, 24 = first heap byte,
//! 25/26/27 = heap prefix intermediates] x interpolation count [1, 2, 3] x
//! intermediate liveness [live across a later `@to_str` vs immediately
//! consumed by the next concat] — list-free throughout (scalar
//! interpolations only) so the str lineage is isolated from the
//! collection-fork cells in `burden_dup_inc.rs` (whose
//! `test_list_three_owned_invoke_forks_base_unmodified`,
//! `test_diamond_sharing_set_fork_does_not_mutate_base`, and
//! `test_list_three_user_fn_owned_arg_forks_base_unmodified` cells share
//! this root cause and must flip GREEN with the same cure).
//! Every cell runs under `ORI_CHECK_LEAKS=1` (via `assert_cell_output`), so
//! a cell that exits 0 but leaks the final template string is still RED
//! (exit 2). Interpreter parity verified for every cell program.
//! Spec: Annex E §AIMS RL-1 + RL-2.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_cell_output;

// ----- SSO-boundary clamp (output <= 23 bytes stays GREEN) -----

/// 2 interpolations, 23-byte output — the exact SSO max. Every concat
/// intermediate and the final string are SSO-inline (no heap, no RC), so
/// the burden path emits nothing for the chain. Always-pass GREEN clamp
/// from below.
#[test]
fn test_template_two_interp_sso_23_byte_output_clean_exit() {
    assert_cell_output(
        r#"
@main () -> void = {
    let a = 7;
    let b = 9;
    print(msg: `score {a} of {b} qqqqqqqqqq`)
}
"#,
        "template_two_interp_sso_23_byte_output",
        "score 7 of 9 qqqqqqqqqq",
    );
}

// ----- Heap-boundary cells (first heap byte at output 24) -----

/// 2 interpolations, 24-byte output (first heap byte), every INTERMEDIATE
/// still SSO (prefix-before-last-interp = 23 bytes). No premature free —
/// stdout is correct — but the heap FINAL template string is never
/// released (the `cow_mutated`-kept lineage half): leaks today (exit 2).
#[test]
fn test_template_two_interp_heap_24_byte_output_no_leak() {
    assert_cell_output(
        r#"
@main () -> void = {
    let a = 7;
    let b = 9;
    print(msg: `xxxxxxxxxxxxxxxxxxxxxx{a}{b}`)
}
"#,
        "template_two_interp_heap_24_byte_output",
        "xxxxxxxxxxxxxxxxxxxxxx79",
    );
}

/// 2 interpolations, 25-byte output: the 23-byte literal + first
/// interpolation produce a 24-byte HEAP intermediate live across the
/// second interpolation's `@to_str` — the smallest split-pair shape.
/// Aborts today (exit 134, `copy_nonoverlapping` overlap precondition:
/// the freed intermediate's block is reused by `@concat`'s fresh alloc).
#[test]
fn test_template_two_interp_heap_intermediate_live_across_to_str_no_uaf() {
    assert_cell_output(
        r#"
@main () -> void = {
    let a = 7;
    let b = 9;
    print(msg: `xxxxxxxxxxxxxxxxxxxxxxx{a}{b}`)
}
"#,
        "template_two_interp_heap_intermediate_live_across_to_str",
        "xxxxxxxxxxxxxxxxxxxxxxx79",
    );
}

// ----- Interpolation-count axis -----

/// 1 interpolation, 28-byte output. The single concat chain has no later
/// `@to_str` for an intermediate to be live across, so there is no
/// premature free and stdout is correct — but the heap final string still
/// leaks today (exit 2). Clamps the interpolation-count axis from below.
#[test]
fn test_template_one_interp_heap_output_no_leak() {
    assert_cell_output(
        r#"
@main () -> void = {
    let a = 7;
    print(msg: `yyyyyyyyyyyyyyyyyyyyyyyyyyy{a}`)
}
"#,
        "template_one_interp_heap_output",
        "yyyyyyyyyyyyyyyyyyyyyyyyyyy7",
    );
}

/// 3 interpolations, 25-byte output: the chain crosses the SSO max at the
/// concat feeding the THIRD interpolation, so the 24-byte heap
/// intermediate is live across the third `@to_str`. Aborts today
/// (exit 134) — pins that the defect scales past two interpolations and
/// that the crossing point (not the count) is the trigger.
#[test]
fn test_template_three_interp_heap_intermediate_no_uaf() {
    assert_cell_output(
        r#"
@main () -> void = {
    let a = 7;
    let b = 9;
    let c = 5;
    print(msg: `wwwwwwwwwwwwwwwwwwww{a} {b} {c}`)
}
"#,
        "template_three_interp_heap_intermediate",
        "wwwwwwwwwwwwwwwwwwww7 9 5",
    );
}

// ----- List-free minimal repros (the RCA shape, 27-byte output) -----

/// List-free minimal abort repro: 2 scalar interpolations, 27-byte output,
/// 26-byte heap intermediate born at the concat immediately before the
/// second `@to_str` (adjacent interpolations after a 25-byte literal).
/// Aborts today (exit 134): the intermediate's rc hits 0 before the
/// borrowed `@concat` read and the freed block is immediately reused by
/// the next concat's fresh allocation (src == dst overlap).
#[test]
fn test_template_minimal_two_interp_27_byte_output_no_uaf() {
    assert_cell_output(
        r#"
@main () -> void = {
    let a = 7;
    let b = 9;
    print(msg: `xxxxxxxxxxxxxxxxxxxxxxxxx{a}{b}`)
}
"#,
        "template_minimal_two_interp_27_byte_output",
        "xxxxxxxxxxxxxxxxxxxxxxxxx79",
    );
}

/// Same 27-byte output with a literal separating the interpolations: the
/// heap crossing happens at `concat(lit, a_str)` whose result is appended
/// in place by the next literal concat, so the block survives the second
/// `@to_str` read (stdout correct) — but the lineage's release never
/// lands: leaks today (exit 2). Distinguishes the abort half (split pair
/// at the to_str-adjacent concat) from the leak half (no normal-path
/// release) of the same lineage family.
#[test]
fn test_template_two_interp_separated_27_byte_output_no_leak() {
    assert_cell_output(
        r#"
@main () -> void = {
    let a = 7;
    let b = 9;
    print(msg: `xxxxxxxxxxxxxxxxxxxxxxx {a} {b}`)
}
"#,
        "template_two_interp_separated_27_byte_output",
        "xxxxxxxxxxxxxxxxxxxxxxx 7 9",
    );
}

// ----- Immediately-consumed intermediates (no intervening @to_str) -----

/// Heap intermediate consumed IMMEDIATELY by the next concat (trailing
/// literal, 1 interpolation mid-string): the Phase-5 inc/dec pair stays
/// same-block and cancels whole, so no premature free (stdout correct) —
/// the heap final string still leaks today (exit 2). Negative clamp on
/// the live-across discriminator.
#[test]
fn test_template_heap_intermediate_immediately_consumed_no_leak() {
    assert_cell_output(
        r#"
@main () -> void = {
    let a = 7;
    print(msg: `xxxxxxxxxxxxxxxxxxxxxxxx{a}q`)
}
"#,
        "template_heap_intermediate_immediately_consumed",
        "xxxxxxxxxxxxxxxxxxxxxxxx7q",
    );
}

/// 2 adjacent leading interpolations + 22-byte literal tail: every
/// intermediate live across a `@to_str` is SSO (1 byte), the heap
/// crossing happens only at the final concat. No premature free (stdout
/// correct); heap final string leaks today (exit 2). Clamps that
/// interpolation ADJACENCY alone is not the trigger — heap-prefix
/// liveness across `@to_str` is.
#[test]
fn test_template_adjacent_interps_sso_prefix_literal_tail_no_leak() {
    assert_cell_output(
        r#"
@main () -> void = {
    let a = 7;
    let b = 9;
    print(msg: `{a}{b}vvvvvvvvvvvvvvvvvvvvvv`)
}
"#,
        "template_adjacent_interps_sso_prefix_literal_tail",
        "79vvvvvvvvvvvvvvvvvvvvvv",
    );
}
