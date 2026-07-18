//! Maranget-style pattern-matrix compilation.
//!
//! Recursive specialization chooses a constructor column, emits one edge per
//! test value, and preserves a default matrix for wildcard rows. Tuple and
//! struct columns decompose without a runtime test.

mod single_ctor;
mod specialize;

use rustc_hash::FxHashSet;

use ori_ir::canon::tree::{
    DecisionTree, FlatPattern, LeafDiscardPaths, PatternMatrix, PatternRow, ScrutineePath,
    TestValue,
};

use self::single_ctor::{decompose_single_constructor, single_constructor_column};
use self::specialize::{collect_test_values, default_matrix, infer_test_kind, specialize_matrix};

/// A behavioral decision tree plus exact blank-pattern cleanup carriers.
///
/// `leaf_discard_paths` follows the same static success-node preorder used by
/// ARC emission: switch edges in order, then default; Guard success, then its
/// `on_fail` subtree. Keeping the carrier parallel avoids teaching behavioral
/// consumers such as exhaustiveness and evaluation about ownership mechanics.
#[derive(Debug)]
pub struct CompiledDecisionTree {
    pub tree: DecisionTree,
    pub leaf_discard_paths: Vec<LeafDiscardPaths>,
}

/// Compiles `matrix` using `paths` as the scrutinee path for each column.
///
/// The initial root call supplies one empty path; specialization extends paths
/// for constructor payloads.
///
/// # Panics
///
/// Panics if `paths.len() != matrix[i].patterns.len()` for any row.
#[expect(
    clippy::needless_pass_by_value,
    reason = "recursive — sub-calls pass owned specialized matrices"
)]
pub fn compile(matrix: PatternMatrix, paths: Vec<ScrutineePath>) -> CompiledDecisionTree {
    assert_matrix_path_alignment(&matrix, &paths);

    if matrix.is_empty() {
        return CompiledDecisionTree {
            tree: DecisionTree::Fail,
            leaf_discard_paths: Vec::new(),
        };
    }

    if matrix[0].patterns.iter().all(FlatPattern::is_wildcard_like) {
        let bindings = extract_all_bindings(&matrix[0], &paths);
        let discard_paths = uncovered_discard_paths(&matrix[0], &bindings);

        if let Some(guard) = matrix[0].guard {
            let remaining = matrix[1..].to_vec();
            let on_fail = compile(remaining, paths);
            let mut leaf_discard_paths = vec![discard_paths];
            leaf_discard_paths.extend(on_fail.leaf_discard_paths);
            return CompiledDecisionTree {
                tree: DecisionTree::Guard {
                    arm_index: matrix[0].arm_index,
                    bindings,
                    guard,
                    on_fail: Box::new(on_fail.tree),
                },
                leaf_discard_paths,
            };
        }

        return CompiledDecisionTree {
            tree: DecisionTree::Leaf {
                arm_index: matrix[0].arm_index,
                bindings,
            },
            leaf_discard_paths: vec![discard_paths],
        };
    }

    let col = pick_column(&matrix);
    let path = paths[col].clone();

    if let Some(shape) = single_constructor_column(&matrix, col) {
        let decomposed = decompose_single_constructor(&matrix, col, &paths, &path, shape);
        return compile(decomposed.matrix, decomposed.paths);
    }

    let test_values = collect_test_values(&matrix, col);
    let test_kind = infer_test_kind(&test_values);

    let mut edges: Vec<(TestValue, DecisionTree)> = Vec::with_capacity(test_values.len());
    let mut leaf_discard_paths = Vec::new();
    for tv in test_values {
        let Specialized {
            matrix: sub_matrix,
            paths: sub_paths,
        } = specialize_matrix(&matrix, col, &tv, &paths, &path);
        let subtree = compile(sub_matrix, sub_paths);
        leaf_discard_paths.extend(subtree.leaf_discard_paths);
        edges.push((tv, subtree.tree));
    }

    let default_spec = default_matrix(&matrix, col, &paths);
    let default = if default_spec.matrix.is_empty() {
        None
    } else {
        let subtree = compile(default_spec.matrix, default_spec.paths);
        leaf_discard_paths.extend(subtree.leaf_discard_paths);
        Some(Box::new(subtree.tree))
    };

    CompiledDecisionTree {
        tree: DecisionTree::Switch {
            path,
            test_kind,
            edges,
            default,
        },
        leaf_discard_paths,
    }
}

fn assert_matrix_path_alignment(matrix: &PatternMatrix, paths: &[ScrutineePath]) {
    for (i, row) in matrix.iter().enumerate() {
        if row.patterns.len() == paths.len() {
            continue;
        }
        tracing::error!("DECISION TREE BUG");
        tracing::error!(
            "Row {i}: paths={}, patterns={}, arm_index={}",
            paths.len(),
            row.patterns.len(),
            row.arm_index
        );
        for (j, pattern) in row.patterns.iter().enumerate() {
            tracing::error!("  pattern[{j}]: {pattern:?}");
        }
        tracing::error!("All rows:");
        for (row_index, candidate) in matrix.iter().enumerate() {
            tracing::error!(
                "  row[{row_index}] (arm {}): {} patterns",
                candidate.arm_index,
                candidate.patterns.len()
            );
            for (j, pattern) in candidate.patterns.iter().enumerate() {
                tracing::error!("    [{j}]: {pattern:?}");
            }
        }
        tracing::error!("Paths: {paths:?}");
        assert_eq!(
            paths.len(),
            row.patterns.len(),
            "column count mismatch at row {i}, arm_index={}",
            row.arm_index
        );
    }
}

// Column selection.

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
        let score = count_distinct_constructors(matrix, col);
        if score > best_score {
            best_score = score;
            best_col = col;
        }
    }

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
/// Patterns with the same key share a `TestValue`; matrix specialization owns
/// their sub-patterns.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ConstructorKey {
    Variant(u32),
    LitInt(i64),
    LitFloat(u64),
    LitBool(bool),
    LitStr(ori_ir::Name),
    LitChar(char),
    Tuple,
    Struct,
    ListLen(u32, bool),
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
        FlatPattern::Or(alts) => alts.first().and_then(constructor_key),
        FlatPattern::At { inner, .. } => constructor_key(inner),
    }
}

/// Matrix and paths produced by specialization.
#[derive(Debug)]
pub(super) struct Specialized {
    pub(super) matrix: PatternMatrix,
    pub(super) paths: Vec<ScrutineePath>,
}

// Binding extraction.

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

/// Keep only blank paths not retained by a whole-value ancestor binding.
///
/// An at-pattern such as `whole @ (x, _)` binds the root at the empty path;
/// its eventual cleanup owns every descendant, so separately discarding the
/// wildcard field would double-release it. Ordinary `(x, _)` has no covering
/// binding and preserves the field cleanup carrier.
fn uncovered_discard_paths(
    row: &PatternRow,
    bindings: &[(ori_ir::Name, ScrutineePath)],
) -> LeafDiscardPaths {
    let binding_paths: FxHashSet<_> = bindings
        .iter()
        .map(|(_, binding)| binding.as_slice())
        .collect();

    row.discard_paths
        .iter()
        .filter(|discard| {
            !(0..=discard.len()).any(|length| binding_paths.contains(&discard[..length]))
        })
        .cloned()
        .collect()
}

/// Preserves top-level bindings carried by a pattern before specialization.
///
/// Binding, at-pattern, and list-rest names join the row's accumulated bindings.
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
            rest_path.push(ori_ir::canon::tree::PathInstruction::ListRest(
                elements.len() as u32,
            ));
            vec![(*name, rest_path)]
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests;
