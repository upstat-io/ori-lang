//! Single-constructor decomposition for Tuple and Struct patterns.
//!
//! These types have only one "shape" — there's no runtime test needed,
//! just decomposition into sub-patterns. This module handles detecting
//! single-constructor columns and expanding them into sub-pattern columns.

use super::collect_consumed_bindings;
use super::Specialized;
use ori_ir::canon::tree::{FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath};
use rustc_hash::FxHashSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SingleConstructorColumn {
    Tuple { fields: usize },
    Struct { fields: usize },
}

impl SingleConstructorColumn {
    fn field_count(self) -> usize {
        match self {
            Self::Tuple { fields } | Self::Struct { fields } => fields,
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "field indices are always < u32::MAX"
    )]
    fn path_instruction(self, index: usize) -> PathInstruction {
        match self {
            Self::Tuple { .. } => PathInstruction::TupleIndex(index as u32),
            Self::Struct { .. } => PathInstruction::StructField(index as u32),
        }
    }
}

/// Check if a column contains only single-constructor patterns (Tuple/Struct)
/// plus wildcards. These types don't need a runtime test — they're always
/// the same "shape" and just need field decomposition.
pub(super) fn single_constructor_column(
    matrix: &PatternMatrix,
    col: usize,
) -> Option<SingleConstructorColumn> {
    let mut shape = None;
    for row in matrix {
        let pat = unwrap_at_or(&row.patterns[col]);
        let candidate = match pat {
            FlatPattern::Tuple(elements) => Some(SingleConstructorColumn::Tuple {
                fields: elements.len(),
            }),
            FlatPattern::Struct { fields } => Some(SingleConstructorColumn::Struct {
                fields: fields.len(),
            }),
            FlatPattern::Wildcard | FlatPattern::Binding(_) => None,
            _ => return None,
        };
        if let Some(candidate) = candidate {
            match shape {
                Some(existing) if existing != candidate => return None,
                Some(_) => {}
                None => shape = Some(candidate),
            }
        }
    }
    shape
}

/// Unwrap an at-pattern to get its underlying pattern.
fn unwrap_at_or(pat: &FlatPattern) -> &FlatPattern {
    match pat {
        FlatPattern::At { inner, .. } => unwrap_at_or(inner),
        _ => pat,
    }
}

/// Decompose a single-constructor column (Tuple/Struct) into sub-pattern columns.
///
/// This is similar to `specialize_matrix` but without a `TestValue` — the
/// decomposition is unconditional since there's only one possible shape.
pub(super) fn decompose_single_constructor(
    matrix: &PatternMatrix,
    col: usize,
    paths: &[ScrutineePath],
    base_path: &ScrutineePath,
    shape: SingleConstructorColumn,
) -> Specialized {
    let sub_count = shape.field_count();

    // Build new paths: replace column `col` with sub-pattern paths.
    let mut new_paths = Vec::with_capacity(paths.len() - 1 + sub_count);
    new_paths.extend_from_slice(&paths[..col]);
    for i in 0..sub_count {
        let mut sub_path = base_path.clone();
        sub_path.push(shape.path_instruction(i));
        new_paths.push(sub_path);
    }
    new_paths.extend_from_slice(&paths[col + 1..]);

    // Build new rows: decompose each pattern at `col`.
    let new_matrix = matrix
        .iter()
        .map(|row| {
            // Collect any bindings from the consumed pattern (e.g., Binding or At).
            let mut bindings = row.bindings.clone();
            bindings.extend(collect_consumed_bindings(&row.patterns[col], base_path));

            let sub_pats = decompose_single_ctor_pattern(&row.patterns[col], sub_count);
            let mut discard_paths = row.discard_paths.clone();
            let mut seen_discard_paths: FxHashSet<_> = discard_paths.iter().cloned().collect();
            if matches!(
                unwrap_at_or(&row.patterns[col]),
                FlatPattern::Tuple(_) | FlatPattern::Struct { .. }
            ) {
                for (sub_pattern, sub_path) in
                    sub_pats.iter().zip(new_paths[col..col + sub_count].iter())
                {
                    if matches!(sub_pattern, FlatPattern::Wildcard)
                        && seen_discard_paths.insert(sub_path.clone())
                    {
                        discard_paths.push(sub_path.clone());
                    }
                }
            }
            let mut new_patterns = Vec::with_capacity(row.patterns.len() - 1 + sub_pats.len());
            new_patterns.extend_from_slice(&row.patterns[..col]);
            new_patterns.extend(sub_pats);
            new_patterns.extend_from_slice(&row.patterns[col + 1..]);
            PatternRow {
                patterns: new_patterns,
                arm_index: row.arm_index,
                guard: row.guard,
                bindings,
                discard_paths,
            }
        })
        .collect();

    Specialized {
        matrix: new_matrix,
        paths: new_paths,
    }
}

/// Decompose a single-constructor pattern into its sub-patterns.
fn decompose_single_ctor_pattern(pat: &FlatPattern, sub_count: usize) -> Vec<FlatPattern> {
    match pat {
        FlatPattern::Tuple(elements) => elements.clone(),
        FlatPattern::Struct { fields } => fields.iter().map(|(_, sub)| sub.clone()).collect(),
        FlatPattern::Wildcard | FlatPattern::Binding(_) => {
            vec![FlatPattern::Wildcard; sub_count]
        }
        FlatPattern::At { inner, .. } => decompose_single_ctor_pattern(inner, sub_count),
        FlatPattern::Or(alts) => {
            // Use the first alternative's decomposition.
            if let Some(first) = alts.first() {
                decompose_single_ctor_pattern(first, sub_count)
            } else {
                vec![FlatPattern::Wildcard; sub_count]
            }
        }
        _ => vec![FlatPattern::Wildcard; sub_count],
    }
}
