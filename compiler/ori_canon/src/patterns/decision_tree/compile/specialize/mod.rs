//! Pattern-matrix specialization.
//!
//! Each test value filters compatible rows and expands constructor payloads;
//! wildcard rows form the default matrix.

mod test_values;

pub(super) use test_values::{collect_test_values, infer_test_kind};

use super::{collect_consumed_bindings, Specialized};
use ori_ir::canon::tree::{
    FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestValue,
};
use rustc_hash::FxHashSet;

/// Retains rows compatible with `tv` and expands payload columns at `col`.
///
/// Wildcards synthesize payload wildcards; incompatible constructors are omitted.
pub(super) fn specialize_matrix(
    matrix: &PatternMatrix,
    col: usize,
    tv: &TestValue,
    paths: &[ScrutineePath],
    base_path: &ScrutineePath,
) -> Specialized {
    let sub_count = infer_sub_pattern_count(matrix, col, tv);

    let mut new_paths = Vec::with_capacity(paths.len() - 1 + sub_count);
    new_paths.extend_from_slice(&paths[..col]);
    for i in 0..sub_count {
        let mut sub_path = base_path.clone();
        sub_path.push(sub_path_instruction(tv, i));
        new_paths.push(sub_path);
    }
    new_paths.extend_from_slice(&paths[col + 1..]);

    let col_path = &paths[col];
    let mut new_matrix = Vec::new();
    for row in matrix {
        if let Some(new_row) = specialize_row(row, col, tv, sub_count, col_path) {
            new_matrix.push(new_row);
        }
    }

    Specialized {
        matrix: new_matrix,
        paths: new_paths,
    }
}

/// Returns the payload arity produced by `tv`.
///
/// Literals produce no payloads, variants use the matching pattern's field
/// count, and list-length tests use their element count.
fn infer_sub_pattern_count(matrix: &PatternMatrix, col: usize, tv: &TestValue) -> usize {
    match tv {
        TestValue::Tag { variant_index, .. } => {
            for row in matrix {
                if let Some(count) = variant_field_count(&row.patterns[col], *variant_index) {
                    return count;
                }
            }
            0
        }
        TestValue::Int(_)
        | TestValue::Str(_)
        | TestValue::Bool(_)
        | TestValue::Float(_)
        | TestValue::Char(_)
        | TestValue::IntRange { .. } => 0,
        TestValue::ListLen { len, .. } => *len as usize,
    }
}

/// Extract the field count from a pattern if it's a Variant with the given index.
///
/// Recurses through Or and At patterns to find the underlying Variant.
fn variant_field_count(pat: &FlatPattern, target_index: u32) -> Option<usize> {
    match pat {
        FlatPattern::Variant {
            variant_index,
            fields,
            ..
        } if *variant_index == target_index => Some(fields.len()),
        FlatPattern::Or(alts) => {
            for alt in alts {
                if let Some(count) = variant_field_count(alt, target_index) {
                    return Some(count);
                }
            }
            None
        }
        FlatPattern::At { inner, .. } => variant_field_count(inner, target_index),
        _ => None,
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "field/element indices are always < u32::MAX"
)]
fn sub_path_instruction(tv: &TestValue, index: usize) -> PathInstruction {
    match tv {
        TestValue::Tag { .. } => PathInstruction::TagPayload(index as u32),
        TestValue::ListLen { .. } => PathInstruction::ListElement(index as u32),
        _ => unreachable!("sub_path_instruction called for test value with no sub-patterns"),
    }
}

/// Specialize a single row for a test value at column `col`.
///
/// Returns `None` if the row is incompatible (different constructor).
/// `expected_sub_count` is the number of sub-patterns this test value
/// produces, determined by scanning the matrix for Variant field counts.
fn specialize_row(
    row: &PatternRow,
    col: usize,
    tv: &TestValue,
    expected_sub_count: usize,
    col_path: &ScrutineePath,
) -> Option<PatternRow> {
    let pat = &row.patterns[col];
    match specialize_pattern(pat, tv, expected_sub_count) {
        SpecResult::Match(sub_patterns) => {
            let mut bindings = row.bindings.clone();
            bindings.extend(collect_consumed_bindings(pat, col_path));

            let mut discard_paths = row.discard_paths.clone();
            let mut seen_discard_paths: FxHashSet<_> = discard_paths.iter().cloned().collect();
            if exposes_constructor_fields(pat, tv) {
                for (index, sub_pattern) in sub_patterns.iter().enumerate() {
                    if pattern_always_discards(sub_pattern) {
                        let mut discard_path = col_path.clone();
                        discard_path.push(sub_path_instruction(tv, index));
                        if seen_discard_paths.insert(discard_path.clone()) {
                            discard_paths.push(discard_path);
                        }
                    }
                }
            }

            let mut new_patterns = Vec::with_capacity(row.patterns.len() - 1 + sub_patterns.len());
            new_patterns.extend_from_slice(&row.patterns[..col]);
            new_patterns.extend(sub_patterns);
            new_patterns.extend_from_slice(&row.patterns[col + 1..]);
            Some(PatternRow {
                patterns: new_patterns,
                arm_index: row.arm_index,
                guard: row.guard,
                bindings,
                discard_paths,
            })
        }
        SpecResult::NoMatch => None,
    }
}

/// Whether specialization exposes real source-pattern children.
///
/// Wildcard and binding rows synthesize placeholder children to keep the
/// matrix rectangular; those placeholders are not blank-pattern cleanup
/// obligations. Concrete variant/list children are semantic and must retain
/// any explicit `_` paths.
fn exposes_constructor_fields(pat: &FlatPattern, tv: &TestValue) -> bool {
    match (pat, tv) {
        (
            FlatPattern::Variant { variant_index, .. },
            TestValue::Tag {
                variant_index: tested,
                ..
            },
        ) => variant_index == tested,
        (FlatPattern::List { .. }, TestValue::ListLen { .. }) => true,
        (FlatPattern::Or(alternatives), _) => alternatives
            .iter()
            .any(|alternative| exposes_constructor_fields(alternative, tv)),
        (FlatPattern::At { inner, .. }, _) => exposes_constructor_fields(inner, tv),
        _ => false,
    }
}

/// True only when every route represented by this specialized child is `_`.
fn pattern_always_discards(pattern: &FlatPattern) -> bool {
    match pattern {
        FlatPattern::Wildcard => true,
        FlatPattern::Or(alternatives) => {
            !alternatives.is_empty() && alternatives.iter().all(pattern_always_discards)
        }
        _ => false,
    }
}

enum SpecResult {
    Match(Vec<FlatPattern>),
    NoMatch,
}

/// Specialize a single pattern against a test value.
///
/// `expected_sub_count` is the number of sub-patterns that this specialization
/// should produce for wildcard expansion (determined by scanning the matrix
/// for the first concrete constructor pattern).
fn specialize_pattern(pat: &FlatPattern, tv: &TestValue, expected_sub_count: usize) -> SpecResult {
    if let Some(result) = specialize_literal(pat, tv) {
        return result;
    }

    match (pat, tv) {
        (FlatPattern::Wildcard | FlatPattern::Binding(_), _) => {
            SpecResult::Match(vec![FlatPattern::Wildcard; expected_sub_count])
        }
        (
            FlatPattern::Variant {
                variant_index: pattern_index,
                fields,
                ..
            },
            TestValue::Tag {
                variant_index: tested_index,
                ..
            },
        ) => match_result(pattern_index == tested_index, fields.clone()),
        (FlatPattern::List { elements, rest }, TestValue::ListLen { len, is_exact }) => {
            specialize_list_pattern(elements, rest.is_some(), *len as usize, *is_exact)
        }
        (
            FlatPattern::Range {
                start,
                end,
                inclusive,
            },
            TestValue::IntRange {
                lo,
                hi,
                inclusive: tested_inclusive,
            },
        ) => {
            let matches = start.as_ref() == Some(lo)
                && end.as_ref() == Some(hi)
                && inclusive == tested_inclusive;
            match_result(matches, Vec::new())
        }
        (FlatPattern::Or(alternatives), tested) => {
            specialize_or_pattern(alternatives, tested, expected_sub_count)
        }
        (FlatPattern::At { inner, .. }, tested) => {
            specialize_pattern(inner, tested, expected_sub_count)
        }
        _ => SpecResult::NoMatch,
    }
}

fn specialize_literal(pat: &FlatPattern, tv: &TestValue) -> Option<SpecResult> {
    let matches = match (pat, tv) {
        (FlatPattern::LitInt(value), TestValue::Int(tested)) => value == tested,
        (FlatPattern::LitBool(value), TestValue::Bool(tested)) => value == tested,
        (FlatPattern::LitStr(value), TestValue::Str(tested)) => value == tested,
        (FlatPattern::LitFloat(value), TestValue::Float(tested)) => value == tested,
        (FlatPattern::LitChar(value), TestValue::Char(tested)) => value == tested,
        _ => return None,
    };
    Some(match_result(matches, Vec::new()))
}

fn specialize_list_pattern(
    elements: &[FlatPattern],
    has_rest: bool,
    tested_len: usize,
    is_exact: bool,
) -> SpecResult {
    if elements.len() != tested_len || (!has_rest && !is_exact) {
        SpecResult::NoMatch
    } else {
        SpecResult::Match(elements.to_vec())
    }
}

fn specialize_or_pattern(
    alternatives: &[FlatPattern],
    tested: &TestValue,
    expected_sub_count: usize,
) -> SpecResult {
    let matching = alternatives
        .iter()
        .filter_map(|alternative| {
            match specialize_pattern(alternative, tested, expected_sub_count) {
                SpecResult::Match(sub_patterns) => Some(sub_patterns),
                SpecResult::NoMatch => None,
            }
        })
        .collect::<Vec<_>>();

    match matching.as_slice() {
        [] => SpecResult::NoMatch,
        [single] => SpecResult::Match(single.clone()),
        _ => {
            let combined = (0..expected_sub_count)
                .map(|column| {
                    FlatPattern::Or(
                        matching
                            .iter()
                            .map(|sub_patterns| sub_patterns[column].clone())
                            .collect(),
                    )
                })
                .collect();
            SpecResult::Match(combined)
        }
    }
}

fn match_result(matches: bool, sub_patterns: Vec<FlatPattern>) -> SpecResult {
    if matches {
        SpecResult::Match(sub_patterns)
    } else {
        SpecResult::NoMatch
    }
}

/// Compute the default matrix: rows where column `col` is a wildcard.
///
/// These rows match when no explicit constructor matches. The column
/// is removed (it's been tested).
pub(super) fn default_matrix(
    matrix: &PatternMatrix,
    col: usize,
    paths: &[ScrutineePath],
) -> Specialized {
    let mut new_paths = Vec::with_capacity(paths.len() - 1);
    new_paths.extend_from_slice(&paths[..col]);
    new_paths.extend_from_slice(&paths[col + 1..]);

    let col_path = &paths[col];
    let mut new_matrix = Vec::new();
    for row in matrix {
        if row.patterns[col].is_wildcard_like() {
            let mut bindings = row.bindings.clone();
            bindings.extend(collect_consumed_bindings(&row.patterns[col], col_path));

            let mut new_patterns = Vec::with_capacity(row.patterns.len() - 1);
            new_patterns.extend_from_slice(&row.patterns[..col]);
            new_patterns.extend_from_slice(&row.patterns[col + 1..]);
            new_matrix.push(PatternRow {
                patterns: new_patterns,
                arm_index: row.arm_index,
                guard: row.guard,
                bindings,
                discard_paths: row.discard_paths.clone(),
            });
        }
    }

    Specialized {
        matrix: new_matrix,
        paths: new_paths,
    }
}
