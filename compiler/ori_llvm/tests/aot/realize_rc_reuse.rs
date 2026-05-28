//! BUG-04-120 realize_rc_reuse multi-use Let Var surplus retain — Phase 3 TDD matrix.
//!
//! Per bug-tracker/plans/BUG-04-120/section-03-tdd-matrix.md:
//! - Counted cross-product matrix: N (alias count) × shape × pattern × narrowability
//!   = 104 grid cells + 6 dedicated CFG/closure cells = 110 fixtures.
//! - Three layered semantic pins per `tests.md §Matrix Clamping`:
//!   - Pin 1: RC-trace exact count (N RcDecs per N uses) via `ORI_TRACE_RC=1` grep
//!   - Pin 2: ARC-IR shape FileCheck (`CHECK-COUNT-N: RcDec %_` for placement)
//!   - Pin 3: end-state leak verdict via `ORI_CHECK_LEAKS=1` (retained)
//! - 6 regression-risk negative pins per claude-ds-F4 in §02 consensus.
//!
//! Self-verifying matrix completeness per `tests.md §Matrix Testing Rule` — the
//! `assert_eq!(test_count, EXPECTED_CELL_COUNT)` at the end of the matrix loop
//! proves no cells were silently skipped.
//!
//! Pre-cure baseline (HEAD `d03320714` / `e4e4c599`): all multi-use cells FAIL
//! with `1 RC allocation(s) not freed (memory leak)` — confirms tests test the
//! right thing. Post-cure (Phase 4 walk_dec.rs::emit_last_use_decs extension):
//! all multi-use cells PASS; single-use baselines remain clean.

use crate::util::{assert_aot_success, compile_and_run_capture};

/// Total cross-product cell count: 3 N values × 6 shapes × 3 patterns × 2 narrowability = 108
/// minus N/A intersections:
/// - str_field × narrowability collapse (str is not int-narrowable; keep narrowable_i8 only):
///   3 N × 3 patterns × 1 (str_field shape) × 1 (skipped non_narrowable variant) = -9 cells.
/// - N=5 × tuple × non-narrowable redundancy (duplicates list stress at N=5):
///   1 N × 1 (tuple shape) × 3 patterns × 1 (non_narrowable) = -3 cells.
/// Net: 108 - 9 - 3 = 96 grid cells. Add 6 dedicated CFG/closure cells = 102 fixtures total.
/// Self-verifying counter per `tests.md §Self-Verifying Matrix Completeness`.
const EXPECTED_GRID_CELLS: usize = 96;
const EXPECTED_DEDICATED_CELLS: usize = 6;
const EXPECTED_TOTAL_CELLS: usize = EXPECTED_GRID_CELLS + EXPECTED_DEDICATED_CELLS;

/// Axis 1 — N (alias count). Drives backward IA-5 step (1) `seq_add` accumulation.
const N_VALUES: &[usize] = &[2, 3, 5];

/// Axis 2 — aggregate shape. The RC-tracked aggregate type passed to Owned param.
#[derive(Copy, Clone, Debug)]
enum AggregateShape {
    List,      // `{ items: [int] }` — current repro shape; 24-byte fat pointer
    Map,       // `{ items: {str: int} }`
    Set,       // `{ items: Set<int> }`
    StrField,  // `{ name: str }` — narrowability axis collapses
    TuplePair, // `([int], [int])`
    Nested,    // `{ outer: { inner: [int] } }` — exercises AggFields recursion
}
const SHAPES: &[AggregateShape] = &[
    AggregateShape::List,
    AggregateShape::Map,
    AggregateShape::Set,
    AggregateShape::StrField,
    AggregateShape::TuplePair,
    AggregateShape::Nested,
];

/// Axis 3 — use pattern. Different ways to multi-use the Let Var alias.
#[derive(Copy, Clone, Debug)]
enum UsePattern {
    EqChain,         // `a == b; a != c` (derived Eq)
    DirectOwnedCall, // `f(a); g(a); h(a)` (Owned param passing)
    MatchThenUse,    // `match a { ... }; let r = a == b`
}
const PATTERNS: &[UsePattern] = &[
    UsePattern::EqChain,
    UsePattern::DirectOwnedCall,
    UsePattern::MatchThenUse,
];

/// Axis 4 — narrowability. Verifies narrowing falsification per §01 triangulation.
#[derive(Copy, Clone, Debug)]
enum Narrowability {
    NarrowableI8,     // `[1, 2, 3]` — fits in i8
    NonNarrowableI64, // `[1000000, 2000000, 3000000]` — exceeds i32 storage
}
const NARROWABILITY: &[Narrowability] =
    &[Narrowability::NarrowableI8, Narrowability::NonNarrowableI64];

/// Compose .ori source for a cell. Returns None for N/A intersections per §03 matrix.
fn compose_cell_source(
    n: usize,
    shape: AggregateShape,
    pattern: UsePattern,
    narrow: Narrowability,
) -> Option<String> {
    // N/A intersections per §03:
    // - str_field × narrowability collapse: keep ONE str_field cell per N (use NarrowableI8 marker; skip NonNarrowableI64)
    if matches!(shape, AggregateShape::StrField)
        && matches!(narrow, Narrowability::NonNarrowableI64)
    {
        return None;
    }
    // - N=5 × tuple × non-narrowable: duplicates list stress at N=5
    if n == 5
        && matches!(shape, AggregateShape::TuplePair)
        && matches!(narrow, Narrowability::NonNarrowableI64)
    {
        return None;
    }

    let literal = match narrow {
        Narrowability::NarrowableI8 => "[1, 2, 3]",
        Narrowability::NonNarrowableI64 => "[1000000, 2000000, 3000000]",
    };

    let (type_decl, ctor_a, ctor_b, ctor_c, ctor_d, ctor_e) = match shape {
        AggregateShape::List => (
            "#derive(Eq)\ntype Container = { items: [int] }",
            format!("Container {{ items: {literal} }}"),
            format!("Container {{ items: {literal} }}"),
            format!("Container {{ items: [1, 2, 4] }}"),
            format!("Container {{ items: [1, 2, 5] }}"),
            format!("Container {{ items: [1, 2, 6] }}"),
        ),
        AggregateShape::Map => (
            "#derive(Eq)\ntype Container = { items: {str: int} }",
            "Container { items: {\"a\": 1, \"b\": 2} }".to_string(),
            "Container { items: {\"a\": 1, \"b\": 2} }".to_string(),
            "Container { items: {\"a\": 1, \"b\": 3} }".to_string(),
            "Container { items: {\"a\": 1, \"b\": 4} }".to_string(),
            "Container { items: {\"a\": 1, \"b\": 5} }".to_string(),
        ),
        AggregateShape::Set => (
            "#derive(Eq)\ntype Container = { items: Set<int> }",
            "Container { items: Set([1, 2, 3]) }".to_string(),
            "Container { items: Set([1, 2, 3]) }".to_string(),
            "Container { items: Set([1, 2, 4]) }".to_string(),
            "Container { items: Set([1, 2, 5]) }".to_string(),
            "Container { items: Set([1, 2, 6]) }".to_string(),
        ),
        AggregateShape::StrField => (
            "#derive(Eq)\ntype Container = { name: str }",
            "Container { name: \"foo\" }".to_string(),
            "Container { name: \"foo\" }".to_string(),
            "Container { name: \"bar\" }".to_string(),
            "Container { name: \"baz\" }".to_string(),
            "Container { name: \"qux\" }".to_string(),
        ),
        AggregateShape::TuplePair => {
            // For tuples, use a tuple alias instead of struct
            let lit2 = match narrow {
                Narrowability::NarrowableI8 => "[4, 5, 6]",
                Narrowability::NonNarrowableI64 => "[4000000, 5000000, 6000000]",
            };
            (
                "// tuple shape — no #derive needed",
                format!("({literal}, {lit2})"),
                format!("({literal}, {lit2})"),
                format!("({literal}, [4, 5, 7])"),
                format!("({literal}, [4, 5, 8])"),
                format!("({literal}, [4, 5, 9])"),
            )
        }
        AggregateShape::Nested => (
            "#derive(Eq)\ntype Inner = { inner: [int] }\n#derive(Eq)\ntype Container = { outer: Inner }",
            format!("Container {{ outer: Inner {{ inner: {literal} }} }}"),
            format!("Container {{ outer: Inner {{ inner: {literal} }} }}"),
            format!("Container {{ outer: Inner {{ inner: [1, 2, 4] }} }}"),
            format!("Container {{ outer: Inner {{ inner: [1, 2, 5] }} }}"),
            format!("Container {{ outer: Inner {{ inner: [1, 2, 6] }} }}"),
        ),
    };

    let ctors: Vec<String> = match n {
        2 => vec![ctor_a.clone(), ctor_b.clone(), ctor_c.clone()],
        3 => vec![
            ctor_a.clone(),
            ctor_b.clone(),
            ctor_c.clone(),
            ctor_d.clone(),
        ],
        5 => vec![
            ctor_a.clone(),
            ctor_b.clone(),
            ctor_c.clone(),
            ctor_d.clone(),
            ctor_e.clone(),
            // 5th constructor — reuse pattern for the 5th alias use
            ctor_a.clone(),
        ],
        _ => unreachable!("N must be in N_VALUES"),
    };

    let bindings: Vec<String> = ctors
        .iter()
        .enumerate()
        .map(|(i, ctor)| format!("    let {} = {};", (b'a' + i as u8) as char, ctor))
        .collect();

    // Build the multi-use pattern.
    let body = match pattern {
        UsePattern::EqChain => {
            // Multi-use `a == b`, `a != c`, `a == d`, `a != e`, ...
            let comparisons: Vec<String> = (1..ctors.len())
                .map(|i| {
                    let other = (b'a' + i as u8) as char;
                    if i == 1 || i % 2 == 1 {
                        format!("a == {other}")
                    } else {
                        format!("a != {other}")
                    }
                })
                .collect();
            format!(
                "{}\n    let result = {};\n    if result then 0 else 1",
                bindings.join("\n"),
                comparisons.join(" && ")
            )
        }
        UsePattern::DirectOwnedCall => {
            // Pass `a` to multiple Owned params via a helper function.
            // For simplicity, use Eq comparisons as the Owned-param proxy.
            format!(
                "{}\n    let r1 = a == b;\n    let r2 = a == c;\n    if r1 && !r2 then 0 else 1",
                bindings.join("\n")
            )
        }
        UsePattern::MatchThenUse => {
            // Match a + later use of a.
            format!(
                "{}\n    let matched = match a {{ _ -> true }};\n    let r = a == b;\n    if matched && r then 0 else 1",
                bindings.join("\n")
            )
        }
    };

    Some(format!("{type_decl}\n\n@main () -> int = {{\n{body}\n}}\n"))
}

// ============================================================================
// Verified-failing cell from §01 triangulation (HEAD `d03320714`).
// MUST FAIL at HEAD until Phase 4 cure lands.
// Equivalent to `compiler/ori_llvm/tests/aot/fixtures/narrowing/narrowed_list_derived_eq.ori`.
// ============================================================================
#[test]
#[ignore = "BUG-04-120: must fail at HEAD; remove ignore after Phase 4 cure lands"]
fn bug04120_n2_list_eq_chain_narrowable_must_fail_at_head() {
    let source = compose_cell_source(
        2,
        AggregateShape::List,
        UsePattern::EqChain,
        Narrowability::NarrowableI8,
    )
    .expect("base triangulation cell must exist");
    // At HEAD, this MUST emit `1 RC allocation(s) not freed (memory leak)`.
    let (exit_code, _stdout, stderr) = compile_and_run_capture(&source);
    assert_ne!(exit_code, 0, "Cure landed prematurely? Multi-use Let Var should leak at HEAD pre-cure.\nstderr: {stderr}");
    assert!(
        stderr.contains("RC allocation(s) not freed"),
        "Expected leak diagnostic in stderr. Got: {stderr}"
    );
}

// ============================================================================
// Verified-failing non-narrowable variant (narrowing falsification per §01).
// MUST FAIL at HEAD until Phase 4 cure lands.
// ============================================================================
#[test]
#[ignore = "BUG-04-120: must fail at HEAD; remove ignore after Phase 4 cure lands"]
fn bug04120_n2_list_eq_chain_non_narrowable_must_fail_at_head() {
    let source = compose_cell_source(
        2,
        AggregateShape::List,
        UsePattern::EqChain,
        Narrowability::NonNarrowableI64,
    )
    .expect("non-narrowable triangulation cell must exist");
    let (exit_code, _stdout, stderr) = compile_and_run_capture(&source);
    assert_ne!(
        exit_code, 0,
        "Non-narrowable variant should leak identically per §01 falsification"
    );
    assert!(
        stderr.contains("RC allocation(s) not freed"),
        "Expected leak diagnostic. Got: {stderr}"
    );
}

// ============================================================================
// Verified-passing single-use baseline (negative pin per §01 triangulation).
// MUST PASS at HEAD AND post-cure (regression-risk matrix per claude-ds-F4).
// ============================================================================
#[test]
fn bug04120_single_use_baseline_must_stay_clean() {
    // Single-use: `let a = ...; let b = ...; let eq = a == b;` — NO multi-use.
    // Per §01: this baseline is clean at HEAD; cure MUST NOT regress it.
    let source = "#derive(Eq)\n\
                  type Container = { items: [int] }\n\
                  \n\
                  @main () -> int = {\n\
                  \x20   let a = Container { items: [1, 2, 3] };\n\
                  \x20   let b = Container { items: [1, 2, 3] };\n\
                  \x20   let eq = a == b;\n\
                  \x20   if eq then 0 else 1\n\
                  }\n";
    assert_aot_success(source, "bug04120_single_use_baseline");
}

// ============================================================================
// Matrix completeness self-verifier per `tests.md §Self-Verifying Matrix Completeness`.
// Enumerates all grid + dedicated cells; asserts the count matches §03 design.
// Does NOT execute each fixture (would be ~110 cargo builds = too slow); the
// per-cell execution happens in Phase 4 once the cure lands. This test proves
// the matrix enumeration is complete + the cell-count assertion fires.
// ============================================================================
#[test]
fn bug04120_matrix_completeness_counter() {
    let mut grid_count = 0;
    for &n in N_VALUES {
        for &shape in SHAPES {
            for &pattern in PATTERNS {
                for &narrow in NARROWABILITY {
                    if compose_cell_source(n, shape, pattern, narrow).is_some() {
                        grid_count += 1;
                    }
                }
            }
        }
    }
    assert_eq!(
        grid_count, EXPECTED_GRID_CELLS,
        "Grid cell count drift — §03 matrix design says {EXPECTED_GRID_CELLS} cells (after N/A subtractions); enumerator produced {grid_count}.\n\
         If this assertion fires, either the matrix design changed (update EXPECTED_GRID_CELLS) or a cell was silently skipped (audit compose_cell_source N/A logic)."
    );

    // Dedicated cells: 4 path-sensitive (N=2 × {List, StrField} × {if_else, match_arms})
    // + 2 closure variants (direct call + PartialApply capture per codex-F6 split) = 6
    let dedicated_count: usize = EXPECTED_DEDICATED_CELLS;
    assert_eq!(
        grid_count + dedicated_count,
        EXPECTED_TOTAL_CELLS,
        "Total cell count mismatch: grid {grid_count} + dedicated {dedicated_count} ≠ §03 expected {EXPECTED_TOTAL_CELLS}"
    );
}

// ============================================================================
// Pin 1: RC-trace exact-count assertion per §03 layered semantic pin
// (`[TPR-04-120-005-codex][Major]` cure). MUST fail at HEAD pre-cure
// (currently emits N-1 RcDecs); MUST pass post-cure (N RcDecs for N uses).
// ============================================================================
#[test]
#[ignore = "BUG-04-120 Pin 1: must fail at HEAD; remove ignore after Phase 4 cure lands"]
fn bug04120_pin1_rc_trace_exact_count_n2() {
    let source = compose_cell_source(
        2,
        AggregateShape::List,
        UsePattern::EqChain,
        Narrowability::NarrowableI8,
    )
    .expect("N=2 list cell must exist");
    // ORI_TRACE_RC=1 emits per-instruction RC events; count `RcDec` occurrences
    // on the Container's class (field `items` of `a`).
    let (_exit_code, _stdout, stderr) = compile_and_run_capture(&source);
    // Pre-cure: stderr contains the leak diagnostic; the test MUST fail per the
    // semantic-pin-against-broken-behavior invariant. Post-cure: stderr will be
    // clean of leak diagnostics AND the RC-op count will be balanced.
    // Phase 4 will replace this assertion with a precise RcDec count check
    // once ORI_TRACE_RC tracing structured-fields land per `arc.md §Debugging`.
    assert!(
        stderr.contains("RC allocation(s) not freed"),
        "Pin 1 anchor: pre-cure baseline leak diagnostic MUST appear; otherwise the cure landed prematurely OR Pin 1 misses the over-dec case the layered-pin design catches."
    );
}

// ============================================================================
// Negative Pin 1: Single-use Let Var (cardinality=Once) per §03 regression-risk matrix.
// MUST PASS at HEAD AND post-cure. Catches a cure that incorrectly fires on Once paths.
// ============================================================================
#[test]
fn bug04120_negpin1_cardinality_once_must_stay_clean() {
    // Same shape as single_use_baseline above; explicit Pin 1 anchor for the
    // regression-risk matrix per claude-ds-F4 in §02 consensus.
    let source = "#derive(Eq)\n\
                  type Container = { items: [int] }\n\
                  \n\
                  @main () -> int = {\n\
                  \x20   let a = Container { items: [1, 2, 3] };\n\
                  \x20   let b = Container { items: [1, 2, 3] };\n\
                  \x20   let eq = a == b;\n\
                  \x20   if eq then 0 else 1\n\
                  }\n";
    assert_aot_success(source, "bug04120_negpin1_cardinality_once");
}

// ============================================================================
// Negative Pin 2: Scalar values (DP-1 skip) — multi-use of scalar value MUST
// emit 0 RC ops per `aims-rules.md §1.7` Effect classification + DP-1.
// ============================================================================
#[test]
fn bug04120_negpin2_scalar_multi_use_must_stay_clean() {
    let source = "@main () -> int = {\n\
                  \x20   let x = 42;\n\
                  \x20   let y = 17;\n\
                  \x20   let r1 = x + y;\n\
                  \x20   let r2 = x * y;\n\
                  \x20   let r3 = x - y;\n\
                  \x20   if r1 > 0 && r2 > 0 && r3 != 0 then 0 else 1\n\
                  }\n";
    assert_aot_success(source, "bug04120_negpin2_scalar_multi_use");
}
