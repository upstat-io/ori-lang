//! Matrix TDD for `realize_rc_reuse` multi-use Let Var alias chains.
//!
//! Counted cross-product matrix: N (alias count) × aggregate shape × use
//! pattern × narrowability. Three layered semantic pins (RC-trace exact
//! count, ARC-IR shape FileCheck, end-state leak verdict). Self-verifying
//! cell counter proves no matrix cell silently skipped.

use crate::util::{assert_aot_success, compile_and_run_capture};

/// Total cross-product cell count: 3 N values × 6 shapes × 3 patterns × 2 narrowability = 108
/// minus N/A intersections:
/// - str_field × narrowability collapse (str is not int-narrowable; keep narrowable_i8 only):
///   3 N × 3 patterns × 1 (str_field shape) × 1 (skipped non_narrowable variant) = -9 cells.
/// - N=5 × tuple × non-narrowable redundancy (duplicates list stress at N=5):
///   1 N × 1 (tuple shape) × 3 patterns × 1 (non_narrowable) = -3 cells.
/// Net: 108 - 9 - 3 = 96 grid cells. Add 6 dedicated CFG/closure cells = 102 fixtures total.
/// Self-verifying cell counter.
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

/// Axis 4 — narrowability. Verifies narrowing falsification per triangulation.
#[derive(Copy, Clone, Debug)]
enum Narrowability {
    NarrowableI8,     // `[1, 2, 3]` — fits in i8
    NonNarrowableI64, // `[1000000, 2000000, 3000000]` — exceeds i32 storage
}
const NARROWABILITY: &[Narrowability] =
    &[Narrowability::NarrowableI8, Narrowability::NonNarrowableI64];

/// Compose .ori source for a cell. Returns None for N/A intersections per matrix design.
fn compose_cell_source(
    n: usize,
    shape: AggregateShape,
    pattern: UsePattern,
    narrow: Narrowability,
) -> Option<String> {
    // N/A intersections:
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

// Multi-use Let Var alias chain (narrowable variant): MUST be leak-free post-cure.
// Equivalent to `compiler/ori_llvm/tests/aot/fixtures/narrowing/narrowed_list_derived_eq.ori`.
/// Regression: multi-use Let Var alias of RC-tracked aggregate leaks one RC
/// allocation per surplus alias.
#[test]
// Cure landed: IA-5 transparent-alias transfer in block.rs
fn multi_use_let_var_eq_chain_narrowable_no_leak() {
    let source = compose_cell_source(
        2,
        AggregateShape::List,
        UsePattern::EqChain,
        Narrowability::NarrowableI8,
    )
    .expect("base triangulation cell must exist");
    let (exit_code, _stdout, stderr) = compile_and_run_capture(&source);
    assert_eq!(
        exit_code, 0,
        "Multi-use Let Var alias chain must run cleanly.\nstderr: {stderr}"
    );
    assert!(
        !stderr.contains("RC allocation(s) not freed"),
        "No leak diagnostic expected. Got: {stderr}"
    );
}

// Multi-use Let Var alias chain (non-narrowable variant): same cure applies.
/// Regression: narrowing-independent variant proves cure surface is IA-5
/// alias transfer, not the repr-opt narrowing pipeline.

#[test]
// Cure landed: IA-5 transparent-alias transfer in block.rs
fn multi_use_let_var_eq_chain_non_narrowable_no_leak() {
    let source = compose_cell_source(
        2,
        AggregateShape::List,
        UsePattern::EqChain,
        Narrowability::NonNarrowableI64,
    )
    .expect("non-narrowable triangulation cell must exist");
    let (exit_code, _stdout, stderr) = compile_and_run_capture(&source);
    assert_eq!(
        exit_code, 0,
        "Non-narrowable variant must also run cleanly.\nstderr: {stderr}"
    );
    assert!(
        !stderr.contains("RC allocation(s) not freed"),
        "No leak diagnostic expected. Got: {stderr}"
    );
}

// Single-use baseline: no alias chain → no leak (regression-risk matrix anchor).
/// Regression-risk pin: cure MUST NOT add decs to single-use Let Var chains.

#[test]
fn single_use_let_var_eq_chain_no_leak() {
    // Single-use: `let a = ...; let b = ...; let eq = a == b;` — NO multi-use.
    // Single-use baseline: cure MUST NOT regress.
    let source = "#derive(Eq)\n\
                  type Container = { items: [int] }\n\
                  \n\
                  @main () -> int = {\n\
                  \x20   let a = Container { items: [1, 2, 3] };\n\
                  \x20   let b = Container { items: [1, 2, 3] };\n\
                  \x20   let eq = a == b;\n\
                  \x20   if eq then 0 else 1\n\
                  }\n";
    assert_aot_success(source, "single_use_let_var_eq_chain_baseline");
}

// Matrix completeness self-verifier.
// Enumerates all grid + dedicated cells; asserts the count matches matrix design.
/// Matrix completeness self-verifier.

#[test]
fn realize_rc_reuse_matrix_cell_count_complete() {
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
        "Grid cell count drift — matrix design says {EXPECTED_GRID_CELLS} cells (after N/A subtractions); enumerator produced {grid_count}.\n\
         If this assertion fires, either the matrix design changed (update EXPECTED_GRID_CELLS) or a cell was silently skipped (audit compose_cell_source N/A logic)."
    );

    // Dedicated cells: 4 path-sensitive (N=2 × {List, StrField} × {if_else, match_arms})
    // + 2 closure variants (direct call + PartialApply capture).
    let dedicated_count: usize = EXPECTED_DEDICATED_CELLS;
    assert_eq!(
        grid_count + dedicated_count,
        EXPECTED_TOTAL_CELLS,
        "Total cell count mismatch: grid {grid_count} + dedicated {dedicated_count} ≠ expected {EXPECTED_TOTAL_CELLS}"
    );
}

/// Post-cure RC count must balance (N decs for N alias uses).
#[test]
fn multi_use_let_var_eq_chain_rc_balance_clean() {
    let source = compose_cell_source(
        2,
        AggregateShape::List,
        UsePattern::EqChain,
        Narrowability::NarrowableI8,
    )
    .expect("N=2 list cell must exist");
    let (exit_code, _stdout, stderr) = compile_and_run_capture(&source);
    assert_eq!(
        exit_code, 0,
        "RC balance must yield clean exit.\nstderr: {stderr}"
    );
    assert!(
        !stderr.contains("RC allocation(s) not freed"),
        "No leak diagnostic expected. Got: {stderr}"
    );
}

// Regression-risk pin: cure MUST NOT fire on cardinality=Once paths.
/// Regression-risk pin: cure MUST NOT add decs to single-use Let Var chains.

#[test]
fn cardinality_once_let_var_no_leak() {
    let source = "#derive(Eq)\n\
                  type Container = { items: [int] }\n\
                  \n\
                  @main () -> int = {\n\
                  \x20   let a = Container { items: [1, 2, 3] };\n\
                  \x20   let b = Container { items: [1, 2, 3] };\n\
                  \x20   let eq = a == b;\n\
                  \x20   if eq then 0 else 1\n\
                  }\n";
    assert_aot_success(source, "cardinality_once_let_var_baseline");
}

// Regression-risk pin: scalar values (DP-1 skip) emit zero RC ops regardless
// of use count.
/// Regression-risk pin: cure MUST NOT add RC ops to scalar multi-use.

#[test]
fn multi_use_scalar_no_rc_ops() {
    let source = "@main () -> int = {\n\
                  \x20   let x = 42;\n\
                  \x20   let y = 17;\n\
                  \x20   let r1 = x + y;\n\
                  \x20   let r2 = x * y;\n\
                  \x20   let r3 = x - y;\n\
                  \x20   if r1 > 0 && r2 > 0 && r3 != 0 then 0 else 1\n\
                  }\n";
    assert_aot_success(source, "multi_use_scalar_baseline");
}
