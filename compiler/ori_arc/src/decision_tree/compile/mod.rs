//! Decision tree construction via the Maranget (2008) algorithm.
//!
//! Compiles a [`PatternMatrix`] into a [`DecisionTree`] by recursively
//! selecting the best column to split on and specializing the matrix
//! for each distinct constructor.
//!
//! # Algorithm
//!
//! 1. **Base cases**: empty matrix → `Fail`; first row all wildcards → `Leaf`/`Guard`
//! 2. **Pick column**: choose the column with the most distinct constructors
//! 3. **Gather edges**: collect distinct test values at the chosen column
//! 4. **Specialize**: for each test value, filter compatible rows and recurse
//! 5. **Default**: rows with wildcards at the chosen column form the default
//!
//! # References
//!
//! - Maranget (2008) "Compiling Pattern Matching to Good Decision Trees"
//! - Roc `crates/compiler/mono/src/ir/decision_tree.rs`

mod single_ctor;
mod specialize;

use rustc_hash::FxHashSet;

use super::{DecisionTree, FlatPattern, PatternMatrix, PatternRow, ScrutineePath, TestValue};

use self::single_ctor::{decompose_single_constructor, is_single_constructor_column};
use self::specialize::{collect_test_values, default_matrix, infer_test_kind, specialize_matrix};

/// Compile a pattern matrix into a decision tree.
///
/// `paths` provides the scrutinee path for each column. Initially, this is
/// a single-element vec with an empty path (the root scrutinee). As the
/// algorithm recurses, columns are added for sub-patterns and paths are
/// extended.
///
/// # Panics
///
/// Debug-panics if `paths.len() != matrix[i].patterns.len()` for any row.
#[expect(
    clippy::needless_pass_by_value,
    reason = "recursive — sub-calls pass owned specialized matrices"
)]
pub fn compile(matrix: PatternMatrix, paths: Vec<ScrutineePath>) -> DecisionTree {
    if cfg!(debug_assertions) {
        for (i, row) in matrix.iter().enumerate() {
            if row.patterns.len() != paths.len() {
                tracing::error!("DECISION TREE BUG");
                tracing::error!(
                    "Row {i}: paths={}, patterns={}, arm_index={}",
                    paths.len(),
                    row.patterns.len(),
                    row.arm_index
                );
                for (j, p) in row.patterns.iter().enumerate() {
                    tracing::error!("  pattern[{j}]: {p:?}");
                }
                tracing::error!("All rows:");
                for (ri, r) in matrix.iter().enumerate() {
                    tracing::error!(
                        "  row[{ri}] (arm {}): {} patterns",
                        r.arm_index,
                        r.patterns.len()
                    );
                    for (j, p) in r.patterns.iter().enumerate() {
                        tracing::error!("    [{j}]: {p:?}");
                    }
                }
                tracing::error!("Paths: {paths:?}");
                panic!(
                    "column count mismatch at row {i}: paths={}, patterns={}, arm_index={}",
                    paths.len(),
                    row.patterns.len(),
                    row.arm_index,
                );
            }
        }
    }

    // 1. EMPTY MATRIX: no arms left → Fail (unreachable by exhaustiveness).
    if matrix.is_empty() {
        return DecisionTree::Fail;
    }

    // 2. FIRST ROW ALL WILDCARDS: match found → Leaf or Guard.
    if matrix[0].patterns.iter().all(FlatPattern::is_wildcard_like) {
        let bindings = extract_all_bindings(&matrix[0], &paths);

        if let Some(guard) = matrix[0].guard {
            // Guard present: if guard fails, continue matching with
            // remaining compatible rows.
            let remaining = matrix[1..].to_vec();
            let on_fail = compile(remaining, paths);
            return DecisionTree::Guard {
                arm_index: matrix[0].arm_index,
                bindings,
                guard,
                on_fail: Box::new(on_fail),
            };
        }

        return DecisionTree::Leaf {
            arm_index: matrix[0].arm_index,
            bindings,
        };
    }

    // 3. PICK COLUMN: choose the best column to split on.
    let col = pick_column(&matrix);
    let path = paths[col].clone();

    // 3b. SINGLE-CONSTRUCTOR DECOMPOSITION: Tuple and Struct patterns are
    // "single-constructor" types — there's only one shape they can be.
    // They don't need a runtime test (no Switch), just decomposition into
    // their sub-patterns. We handle this by directly decomposing.
    if is_single_constructor_column(&matrix, col) {
        let decomposed = decompose_single_constructor(&matrix, col, &paths, &path);
        return compile(decomposed.matrix, decomposed.paths);
    }

    // 4. GATHER EDGES: collect all distinct test values at the chosen column.
    let test_values = collect_test_values(&matrix, col);
    let test_kind = infer_test_kind(&test_values);

    // 5. BUILD EDGES: for each test value, specialize the matrix and recurse.
    let edges: Vec<(TestValue, DecisionTree)> = test_values
        .into_iter()
        .map(|tv| {
            let Specialized {
                matrix: sub_matrix,
                paths: sub_paths,
            } = specialize_matrix(&matrix, col, &tv, &paths, &path);
            let subtree = compile(sub_matrix, sub_paths);
            (tv, subtree)
        })
        .collect();

    // 6. DEFAULT: rows with wildcards at the chosen column form the default.
    let default_spec = default_matrix(&matrix, col, &paths);
    let default = if default_spec.matrix.is_empty() {
        None
    } else {
        Some(Box::new(compile(default_spec.matrix, default_spec.paths)))
    };

    DecisionTree::Switch {
        path,
        test_kind,
        edges,
        default,
    }
}

// Column selection

/// Choose the best column to split on.
///
/// Heuristic: pick the column with the most distinct constructors (most
/// branching power). Break ties by choosing the leftmost column. This
/// follows Maranget's "column with the most information" strategy.
fn pick_column(matrix: &PatternMatrix) -> usize {
    let ncols = matrix[0].patterns.len();
    let mut best_col = 0;
    let mut best_score = 0;

    for col in 0..ncols {
        // Skip columns where the first non-wildcard pattern hasn't been found.
        let score = count_distinct_constructors(matrix, col);
        if score > best_score {
            best_score = score;
            best_col = col;
        }
    }

    // If no constructors found at all, pick the first column with a non-wildcard.
    if best_score == 0 {
        for col in 0..ncols {
            if matrix
                .iter()
                .any(|row| !row.patterns[col].is_wildcard_like())
            {
                return col;
            }
        }
    }

    best_col
}

/// Count the number of distinct constructors at a given column.
fn count_distinct_constructors(matrix: &PatternMatrix, col: usize) -> usize {
    let mut seen = FxHashSet::default();
    for row in matrix {
        if let Some(key) = constructor_key(&row.patterns[col]) {
            seen.insert(key);
        }
    }
    seen.len()
}

/// A hashable key identifying a constructor (ignoring sub-patterns).
///
/// Two patterns with the same constructor key will be tested by the same
/// `TestValue`. Sub-patterns are not included — they're handled by
/// matrix specialization.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ConstructorKey {
    Variant(u32), // variant index
    LitInt(i64),
    LitFloat(u64),
    LitBool(bool),
    LitStr(ori_ir::Name),
    LitChar(char),
    Tuple,
    Struct,
    ListLen(u32, bool), // (element count, has_rest)
    Range(Option<i64>, Option<i64>, bool),
}

fn constructor_key(pat: &FlatPattern) -> Option<ConstructorKey> {
    match pat {
        FlatPattern::Wildcard | FlatPattern::Binding(_) => None,
        FlatPattern::LitInt(v) => Some(ConstructorKey::LitInt(*v)),
        FlatPattern::LitFloat(v) => Some(ConstructorKey::LitFloat(*v)),
        FlatPattern::LitBool(v) => Some(ConstructorKey::LitBool(*v)),
        FlatPattern::LitStr(v) => Some(ConstructorKey::LitStr(*v)),
        FlatPattern::LitChar(v) => Some(ConstructorKey::LitChar(*v)),
        FlatPattern::Variant { variant_index, .. } => Some(ConstructorKey::Variant(*variant_index)),
        FlatPattern::Tuple(_) => Some(ConstructorKey::Tuple),
        FlatPattern::Struct { .. } => Some(ConstructorKey::Struct),
        FlatPattern::List { elements, rest } =>
        {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "list patterns always have < u32::MAX elements"
            )]
            Some(ConstructorKey::ListLen(
                elements.len() as u32,
                rest.is_some(),
            ))
        }
        FlatPattern::Range {
            start,
            end,
            inclusive,
        } => Some(ConstructorKey::Range(*start, *end, *inclusive)),
        FlatPattern::Or(alts) => {
            // Use the first alternative's constructor.
            alts.first().and_then(constructor_key)
        }
        FlatPattern::At { inner, .. } => constructor_key(inner),
    }
}

/// The result of specializing or defaulting a matrix.
pub(super) struct Specialized {
    pub(super) matrix: PatternMatrix,
    pub(super) paths: Vec<ScrutineePath>,
}

// Binding extraction

/// Extract all variable bindings from a row where every pattern is
/// a wildcard or binding.
///
/// Merges the row's accumulated bindings (from prior specialization steps)
/// with any bindings found in the remaining patterns.
fn extract_all_bindings(
    row: &PatternRow,
    paths: &[ScrutineePath],
) -> Vec<(ori_ir::Name, ScrutineePath)> {
    let mut bindings = row.bindings.clone();
    for (pat, path) in row.patterns.iter().zip(paths.iter()) {
        pat.collect_bindings(path, &mut bindings);
    }
    bindings
}

/// Collect variable bindings from a pattern being consumed at a given path.
///
/// When a pattern is removed from a row during specialization or decomposition,
/// any `Binding(name)`, `At { name, .. }`, or `List { rest: Some(name) }` at
/// the top level would lose their binding information. This function collects
/// those bindings so they can be added to the row's accumulated bindings.
pub(super) fn collect_consumed_bindings(
    pat: &FlatPattern,
    path: &ScrutineePath,
) -> Vec<(ori_ir::Name, ScrutineePath)> {
    match pat {
        FlatPattern::Binding(name) => vec![(*name, path.clone())],
        FlatPattern::At { name, inner } => {
            let mut bindings = vec![(*name, path.clone())];
            bindings.extend(collect_consumed_bindings(inner, path));
            bindings
        }
        FlatPattern::List {
            elements,
            rest: Some(name),
        } => {
            let mut rest_path = path.clone();
            #[expect(
                clippy::cast_possible_truncation,
                reason = "List patterns have << u32::MAX elements"
            )]
            rest_path.push(super::PathInstruction::ListRest(elements.len() as u32));
            vec![(*name, rest_path)]
        }
        _ => vec![],
    }
}

// Tests

#[cfg(test)]
mod tests;
