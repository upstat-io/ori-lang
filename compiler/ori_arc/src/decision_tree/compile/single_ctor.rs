//! Single-constructor decomposition for Tuple and Struct patterns.
//!
//! These types have only one "shape" — there's no runtime test needed,
//! just decomposition into sub-patterns. This module handles detecting
//! single-constructor columns and expanding them into sub-pattern columns.

use super::super::{FlatPattern, PatternMatrix, PatternRow, ScrutineePath};
use super::{collect_consumed_bindings, Specialized};

/// Check if a column contains only single-constructor patterns (Tuple/Struct)
/// plus wildcards. These types don't need a runtime test — they're always
/// the same "shape" and just need field decomposition.
pub(super) fn is_single_constructor_column(matrix: &PatternMatrix, col: usize) -> bool {
    let mut has_single_ctor = false;
    for row in matrix {
        let pat = unwrap_at_or(&row.patterns[col]);
        match pat {
            FlatPattern::Tuple(_) | FlatPattern::Struct { .. } => {
                has_single_ctor = true;
            }
            FlatPattern::Wildcard | FlatPattern::Binding(_) => {}
            _ => return false,
        }
    }
    has_single_ctor
}

/// Unwrap At and Or patterns to get the underlying pattern.
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
) -> Specialized {
    // Find the sub-pattern count from the first concrete pattern.
    let sub_count = find_single_ctor_sub_count(matrix, col);

    // Build new paths: replace column `col` with sub-pattern paths.
    let mut new_paths = Vec::with_capacity(paths.len() - 1 + sub_count);
    new_paths.extend_from_slice(&paths[..col]);
    for i in 0..sub_count {
        let mut sub_path = base_path.clone();
        // Determine instruction based on the constructor type.
        let instr = find_single_ctor_path_instruction(matrix, col, i);
        sub_path.push(instr);
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
            let mut new_patterns = Vec::with_capacity(row.patterns.len() - 1 + sub_pats.len());
            new_patterns.extend_from_slice(&row.patterns[..col]);
            new_patterns.extend(sub_pats);
            new_patterns.extend_from_slice(&row.patterns[col + 1..]);
            PatternRow {
                patterns: new_patterns,
                arm_index: row.arm_index,
                guard: row.guard,
                bindings,
            }
        })
        .collect();

    Specialized {
        matrix: new_matrix,
        paths: new_paths,
    }
}

/// Find the sub-pattern count from the first Tuple/Struct pattern in the column.
fn find_single_ctor_sub_count(matrix: &PatternMatrix, col: usize) -> usize {
    for row in matrix {
        let pat = unwrap_at_or(&row.patterns[col]);
        match pat {
            FlatPattern::Tuple(elements) => return elements.len(),
            FlatPattern::Struct { fields } => return fields.len(),
            _ => {}
        }
    }
    0
}

/// Determine the path instruction for single-constructor decomposition.
fn find_single_ctor_path_instruction(
    matrix: &PatternMatrix,
    col: usize,
    index: usize,
) -> super::super::PathInstruction {
    use super::super::PathInstruction;
    for row in matrix {
        let pat = unwrap_at_or(&row.patterns[col]);
        match pat {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "field indices are always < u32::MAX"
            )]
            FlatPattern::Tuple(_) => return PathInstruction::TupleIndex(index as u32),
            #[expect(
                clippy::cast_possible_truncation,
                reason = "field indices are always < u32::MAX"
            )]
            FlatPattern::Struct { .. } => return PathInstruction::StructField(index as u32),
            _ => {}
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "field indices are always < u32::MAX"
    )]
    PathInstruction::TupleIndex(index as u32) // Fallback (shouldn't happen).
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
