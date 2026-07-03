//! Test value collection and kind inference.
//!
//! Collects distinct test values at a given column and infers the
//! [`TestKind`] from them. This feeds into the Maranget specialization
//! step by determining what tests to emit at each decision node.

use rustc_hash::FxHashSet;

use ori_ir::canon::tree::{FlatPattern, PatternMatrix, TestKind, TestValue};

/// Collect all distinct test values at a given column.
///
/// Preserves source order for deterministic output.
pub(crate) fn collect_test_values(matrix: &PatternMatrix, col: usize) -> Vec<TestValue> {
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
/// Most columns are homogeneous (kind = first value's). The one valid
/// heterogeneous mix is exact int literals with ranges at an int column
/// (`match n { 0, 1..10, .. }`), producing both `Int` and `IntRange`. A range
/// forces the comparison-chain path (an int `Switch` cannot represent a range —
/// every range collapses to a duplicate case); the chain handles exact-equality
/// and range edges together.
pub(crate) fn infer_test_kind(values: &[TestValue]) -> TestKind {
    if values
        .iter()
        .any(|v| matches!(v, TestValue::IntRange { .. }))
    {
        return TestKind::IntRange;
    }
    match values.first() {
        Some(TestValue::Int(_)) => TestKind::IntEq,
        Some(TestValue::Str(_)) => TestKind::StrEq,
        Some(TestValue::Bool(_)) => TestKind::BoolEq,
        Some(TestValue::Float(_)) => TestKind::FloatEq,
        Some(TestValue::Char(_)) => TestKind::CharEq,
        // An IntRange-first column already returned IntRange via the any() guard
        // above; reaching it here would mean that guard was bypassed.
        Some(TestValue::IntRange { .. }) => {
            unreachable!("IntRange-first column is caught by the any() guard above")
        }
        Some(TestValue::ListLen { .. }) => TestKind::ListLen,
        Some(TestValue::Tag { .. }) | None => TestKind::EnumTag,
    }
}
