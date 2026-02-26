//! Matrix specialization and test value collection.
//!
//! Implements the core Maranget specialization step: given a test value
//! at a column, filter and decompose matrix rows into those compatible
//! with that value. Also handles default matrix construction and test
//! value collection from patterns.

use rustc_hash::FxHashSet;

use super::super::{FlatPattern, PatternMatrix, PatternRow, ScrutineePath, TestKind, TestValue};
use super::{collect_consumed_bindings, Specialized};

// Test value collection

/// Collect all distinct test values at a given column.
///
/// Preserves source order for deterministic output.
pub(super) fn collect_test_values(matrix: &PatternMatrix, col: usize) -> Vec<TestValue> {
    let mut seen = FxHashSet::default();
    let mut values = Vec::new();

    for row in matrix {
        for tv in test_values_from_pattern(&row.patterns[col]) {
            let key = constructor_key_for_test_value(&tv);
            if seen.insert(key) {
                values.push(tv);
            }
        }
    }

    values
}

/// Extract the test value(s) from a pattern.
///
/// Most patterns produce one test value. Or-patterns produce one per
/// alternative. Wildcards produce none.
fn test_values_from_pattern(pat: &FlatPattern) -> Vec<TestValue> {
    match pat {
        FlatPattern::Wildcard | FlatPattern::Binding(_) => vec![],
        FlatPattern::LitInt(v) => vec![TestValue::Int(*v)],
        FlatPattern::LitFloat(v) => vec![TestValue::Float(*v)],
        FlatPattern::LitBool(v) => vec![TestValue::Bool(*v)],
        FlatPattern::LitStr(v) => vec![TestValue::Str(*v)],
        FlatPattern::LitChar(v) => vec![TestValue::Char(*v)],
        FlatPattern::Variant {
            variant_index,
            variant_name,
            ..
        } => vec![TestValue::Tag {
            variant_index: *variant_index,
            variant_name: *variant_name,
        }],
        FlatPattern::Tuple(_) | FlatPattern::Struct { .. } => {
            // Tuples and structs are always the same "constructor" — they
            // don't need a tag test. They produce no test value because the
            // type system guarantees the scrutinee IS a tuple/struct.
            // Instead, specialization directly decomposes their fields.
            vec![]
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "list patterns always have < u32::MAX elements"
        )]
        FlatPattern::List { elements, rest } => vec![TestValue::ListLen {
            len: elements.len() as u32,
            is_exact: rest.is_none(),
        }],
        FlatPattern::Range {
            start,
            end,
            inclusive,
        } => {
            if let (Some(lo), Some(hi)) = (start, end) {
                vec![TestValue::IntRange {
                    lo: *lo,
                    hi: *hi,
                    inclusive: *inclusive,
                }]
            } else {
                // Open-ended ranges are treated as wildcards for decision purposes.
                vec![]
            }
        }
        FlatPattern::Or(alts) => {
            let mut result = Vec::new();
            for alt in alts {
                result.extend(test_values_from_pattern(alt));
            }
            result
        }
        FlatPattern::At { inner, .. } => test_values_from_pattern(inner),
    }
}

/// A key for deduplicating test values.
fn constructor_key_for_test_value(tv: &TestValue) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    tv.hash(&mut hasher);
    hasher.finish()
}

/// Infer the `TestKind` from the collected test values.
///
/// All test values at a given column should have the same kind (you don't
/// mix `Tag` and `Int` tests at the same scrutinee position). This
/// function determines the kind from the first value.
pub(super) fn infer_test_kind(values: &[TestValue]) -> TestKind {
    match values.first() {
        Some(TestValue::Int(_)) => TestKind::IntEq,
        Some(TestValue::Str(_)) => TestKind::StrEq,
        Some(TestValue::Bool(_)) => TestKind::BoolEq,
        Some(TestValue::Float(_)) => TestKind::FloatEq,
        Some(TestValue::Char(_)) => TestKind::CharEq,
        Some(TestValue::IntRange { .. }) => TestKind::IntRange,
        Some(TestValue::ListLen { .. }) => TestKind::ListLen,
        Some(TestValue::Tag { .. }) | None => TestKind::EnumTag,
    }
}

// Matrix specialization

/// Specialize the matrix for a specific test value at a given column.
///
/// For each row:
/// - If the pattern at `col` matches `tv`: decompose it, replace with sub-patterns
/// - If the pattern at `col` is a wildcard: keep (compatible with any value),
///   adding wildcard sub-patterns
/// - If the pattern at `col` is a different constructor: exclude
pub(super) fn specialize_matrix(
    matrix: &PatternMatrix,
    col: usize,
    tv: &TestValue,
    paths: &[ScrutineePath],
    base_path: &ScrutineePath,
) -> Specialized {
    // Determine how many sub-patterns this test value produces.
    // For Tag variants, this varies per constructor — we scan the matrix
    // to find the first Variant pattern with this tag and use its field count.
    let sub_count = infer_sub_pattern_count(matrix, col, tv);

    // Build new paths: remove col, insert sub-pattern paths at its position.
    let mut new_paths = Vec::with_capacity(paths.len() - 1 + sub_count);
    new_paths.extend_from_slice(&paths[..col]);
    for i in 0..sub_count {
        let mut sub_path = base_path.clone();
        sub_path.push(sub_path_instruction(tv, i));
        new_paths.push(sub_path);
    }
    new_paths.extend_from_slice(&paths[col + 1..]);

    // Build new rows.
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

/// Determine how many sub-patterns specializing on a test value produces.
///
/// For literal test values (Int, Bool, Str, Float, `IntRange`), the answer
/// is always 0 — they have no sub-structure.
///
/// For Tag variants, the field count depends on the specific variant (e.g.
/// `Some` has 1 field, `None` has 0). We scan the matrix at the given column
/// to find the first `Variant` pattern matching this tag and use its field count.
///
/// For `ListLen`, the count equals the number of list elements in the pattern.
fn infer_sub_pattern_count(matrix: &PatternMatrix, col: usize, tv: &TestValue) -> usize {
    match tv {
        TestValue::Tag { variant_index, .. } => {
            // Scan matrix for the first Variant pattern at this column
            // with the matching variant_index.
            for row in matrix {
                if let Some(count) = variant_field_count(&row.patterns[col], *variant_index) {
                    return count;
                }
            }
            0 // No variant pattern found (all wildcards) — 0 sub-patterns.
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

/// Get the path instruction for the i-th sub-pattern of a test value.
#[expect(
    clippy::cast_possible_truncation,
    reason = "field/element indices are always < u32::MAX"
)]
fn sub_path_instruction(tv: &TestValue, index: usize) -> super::super::PathInstruction {
    use super::super::PathInstruction;
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
            // Accumulate bindings from the consumed pattern.
            let mut bindings = row.bindings.clone();
            bindings.extend(collect_consumed_bindings(pat, col_path));

            let mut new_patterns = Vec::with_capacity(row.patterns.len() - 1 + sub_patterns.len());
            new_patterns.extend_from_slice(&row.patterns[..col]);
            new_patterns.extend(sub_patterns);
            new_patterns.extend_from_slice(&row.patterns[col + 1..]);
            Some(PatternRow {
                patterns: new_patterns,
                arm_index: row.arm_index,
                guard: row.guard,
                bindings,
            })
        }
        SpecResult::NoMatch => None,
    }
}

enum SpecResult {
    /// Pattern matches the test value; yields sub-patterns.
    Match(Vec<FlatPattern>),
    /// Pattern does not match the test value.
    NoMatch,
}

/// Specialize a single pattern against a test value.
///
/// `expected_sub_count` is the number of sub-patterns that this specialization
/// should produce for wildcard expansion (determined by scanning the matrix
/// for the first concrete constructor pattern).
#[expect(
    clippy::too_many_lines,
    reason = "exhaustive (FlatPattern, TestValue) specialization dispatch"
)]
fn specialize_pattern(pat: &FlatPattern, tv: &TestValue, expected_sub_count: usize) -> SpecResult {
    match (pat, tv) {
        // Wildcards and bindings match any test value.
        // Produce `expected_sub_count` wildcard sub-patterns to fill the slots.
        (FlatPattern::Wildcard | FlatPattern::Binding(_), _) => {
            SpecResult::Match(vec![FlatPattern::Wildcard; expected_sub_count])
        }

        // Variant matches Tag test value.
        (
            FlatPattern::Variant {
                variant_index: pat_idx,
                fields,
                ..
            },
            TestValue::Tag {
                variant_index: tv_idx,
                ..
            },
        ) => {
            if pat_idx == tv_idx {
                SpecResult::Match(fields.clone())
            } else {
                SpecResult::NoMatch
            }
        }

        // Literal matches.
        (FlatPattern::LitInt(v), TestValue::Int(tv)) => {
            if v == tv {
                SpecResult::Match(vec![])
            } else {
                SpecResult::NoMatch
            }
        }
        (FlatPattern::LitBool(v), TestValue::Bool(tv)) => {
            if v == tv {
                SpecResult::Match(vec![])
            } else {
                SpecResult::NoMatch
            }
        }
        (FlatPattern::LitStr(v), TestValue::Str(tv)) => {
            if v == tv {
                SpecResult::Match(vec![])
            } else {
                SpecResult::NoMatch
            }
        }
        (FlatPattern::LitFloat(v), TestValue::Float(tv)) => {
            if v == tv {
                SpecResult::Match(vec![])
            } else {
                SpecResult::NoMatch
            }
        }
        (FlatPattern::LitChar(v), TestValue::Char(tv)) => {
            if v == tv {
                SpecResult::Match(vec![])
            } else {
                SpecResult::NoMatch
            }
        }

        // List patterns match ListLen test values.
        //
        // Exact list patterns (rest=None, like `[x]`) only match exact-length
        // test values (is_exact=true). Rest patterns (rest=Some, like `[h, ..t]`)
        // match both exact and at-least test values. This prevents exact patterns
        // from appearing in at-least subtrees where they would incorrectly win
        // arm priority over rest patterns.
        (FlatPattern::List { elements, rest }, TestValue::ListLen { len, is_exact }) => {
            if elements.len() != *len as usize {
                return SpecResult::NoMatch;
            }
            // Exact pattern in at-least subtree → exclude
            if rest.is_none() && !is_exact {
                return SpecResult::NoMatch;
            }
            SpecResult::Match(elements.clone())
        }

        // Range patterns match IntRange test values.
        (
            FlatPattern::Range {
                start,
                end,
                inclusive,
            },
            TestValue::IntRange {
                lo,
                hi,
                inclusive: tv_incl,
            },
        ) => {
            if start.as_ref() == Some(lo) && end.as_ref() == Some(hi) && *inclusive == *tv_incl {
                SpecResult::Match(vec![])
            } else {
                SpecResult::NoMatch
            }
        }

        // Or-pattern: combine sub-patterns from ALL matching alternatives.
        (FlatPattern::Or(alts), tv) => {
            let matching: Vec<Vec<FlatPattern>> = alts
                .iter()
                .filter_map(|alt| {
                    if let SpecResult::Match(subs) = specialize_pattern(alt, tv, expected_sub_count)
                    {
                        Some(subs)
                    } else {
                        None
                    }
                })
                .collect();

            match matching.len() {
                0 => SpecResult::NoMatch,
                1 => {
                    // SAFETY: matching.len() == 1, so into_iter().next() is always Some.
                    #[expect(clippy::unwrap_used, reason = "Length checked to be 1")]
                    let single = matching.into_iter().next().unwrap();
                    SpecResult::Match(single)
                }
                _ => {
                    // Multiple alternatives matched: combine sub-patterns
                    // element-wise into Or patterns.
                    let combined: Vec<FlatPattern> = (0..expected_sub_count)
                        .map(|col| {
                            let col_pats: Vec<FlatPattern> =
                                matching.iter().map(|subs| subs[col].clone()).collect();
                            FlatPattern::Or(col_pats)
                        })
                        .collect();
                    SpecResult::Match(combined)
                }
            }
        }

        // At-pattern: match on the inner pattern, keep the binding.
        (FlatPattern::At { inner, .. }, tv) => specialize_pattern(inner, tv, expected_sub_count),

        // Mismatched types (e.g., int pattern vs tag test) → no match.
        _ => SpecResult::NoMatch,
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
            // Accumulate bindings from the consumed pattern.
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
            });
        }
    }

    Specialized {
        matrix: new_matrix,
        paths: new_paths,
    }
}
