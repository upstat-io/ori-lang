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

use rustc_hash::FxHashSet;

use super::{
    DecisionTree, FlatPattern, PatternMatrix, PatternRow, ScrutineePath, TestKind, TestValue,
};

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
#[allow(clippy::needless_pass_by_value)] // Recursive — sub-calls pass owned specialized matrices
pub fn compile(matrix: PatternMatrix, paths: Vec<ScrutineePath>) -> DecisionTree {
    debug_assert!(
        matrix.iter().all(|row| row.patterns.len() == paths.len()),
        "column count mismatch: paths={}, patterns={}",
        paths.len(),
        matrix.first().map_or(0, |r| r.patterns.len()),
    );

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

// ── Column Selection ────────────────────────────────────────────────

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

// ── Single-Constructor Decomposition ─────────────────────────────────

/// Check if a column contains only single-constructor patterns (Tuple/Struct)
/// plus wildcards. These types don't need a runtime test — they're always
/// the same "shape" and just need field decomposition.
fn is_single_constructor_column(matrix: &PatternMatrix, col: usize) -> bool {
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
fn decompose_single_constructor(
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
            let sub_pats = decompose_single_ctor_pattern(&row.patterns[col], sub_count);
            let mut new_patterns = Vec::with_capacity(row.patterns.len() - 1 + sub_pats.len());
            new_patterns.extend_from_slice(&row.patterns[..col]);
            new_patterns.extend(sub_pats);
            new_patterns.extend_from_slice(&row.patterns[col + 1..]);
            PatternRow {
                patterns: new_patterns,
                arm_index: row.arm_index,
                guard: row.guard,
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
) -> super::PathInstruction {
    use super::PathInstruction;
    for row in matrix {
        let pat = unwrap_at_or(&row.patterns[col]);
        match pat {
            #[allow(clippy::cast_possible_truncation)] // field indices are always < u32::MAX
            FlatPattern::Tuple(_) => return PathInstruction::TupleIndex(index as u32),
            #[allow(clippy::cast_possible_truncation)]
            FlatPattern::Struct { .. } => return PathInstruction::StructField(index as u32),
            _ => {}
        }
    }
    #[allow(clippy::cast_possible_truncation)]
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
        FlatPattern::List { elements, rest } => {
            #[allow(clippy::cast_possible_truncation)]
            // list patterns always have < u32::MAX elements
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

// ── Test Value Collection ───────────────────────────────────────────

/// Collect all distinct test values at a given column.
///
/// Preserves source order for deterministic output.
fn collect_test_values(matrix: &PatternMatrix, col: usize) -> Vec<TestValue> {
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
        FlatPattern::LitChar(v) => vec![TestValue::Int(*v as i64)],
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
        #[allow(clippy::cast_possible_truncation)] // list patterns always have < u32::MAX elements
        FlatPattern::List { elements, rest } => vec![TestValue::ListLen {
            len: elements.len() as u32,
            is_exact: rest.is_none(),
        }],
        FlatPattern::Range { start, end, .. } => {
            if let (Some(lo), Some(hi)) = (start, end) {
                vec![TestValue::IntRange { lo: *lo, hi: *hi }]
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
fn infer_test_kind(values: &[TestValue]) -> TestKind {
    match values.first() {
        Some(TestValue::Int(_)) => TestKind::IntEq,
        Some(TestValue::Str(_)) => TestKind::StrEq,
        Some(TestValue::Bool(_)) => TestKind::BoolEq,
        Some(TestValue::Float(_)) => TestKind::FloatEq,
        Some(TestValue::IntRange { .. }) => TestKind::IntRange,
        Some(TestValue::ListLen { .. }) => TestKind::ListLen,
        Some(TestValue::Tag { .. }) | None => TestKind::EnumTag,
    }
}

// ── Matrix Specialization ───────────────────────────────────────────

/// The result of specializing or defaulting a matrix.
struct Specialized {
    matrix: PatternMatrix,
    paths: Vec<ScrutineePath>,
}

/// Specialize the matrix for a specific test value at a given column.
///
/// For each row:
/// - If the pattern at `col` matches `tv`: decompose it, replace with sub-patterns
/// - If the pattern at `col` is a wildcard: keep (compatible with any value),
///   adding wildcard sub-patterns
/// - If the pattern at `col` is a different constructor: exclude
fn specialize_matrix(
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
    let mut new_matrix = Vec::new();
    for row in matrix {
        if let Some(new_row) = specialize_row(row, col, tv, sub_count) {
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
#[allow(clippy::cast_possible_truncation)] // field indices are always < u32::MAX
fn sub_path_instruction(tv: &TestValue, index: usize) -> super::PathInstruction {
    use super::PathInstruction;
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
) -> Option<PatternRow> {
    let pat = &row.patterns[col];
    match specialize_pattern(pat, tv, expected_sub_count) {
        SpecResult::Match(sub_patterns) => {
            let mut new_patterns = Vec::with_capacity(row.patterns.len() - 1 + sub_patterns.len());
            new_patterns.extend_from_slice(&row.patterns[..col]);
            new_patterns.extend(sub_patterns);
            new_patterns.extend_from_slice(&row.patterns[col + 1..]);
            Some(PatternRow {
                patterns: new_patterns,
                arm_index: row.arm_index,
                guard: row.guard,
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
        (FlatPattern::LitChar(v), TestValue::Int(tv)) => {
            if *v as i64 == *tv {
                SpecResult::Match(vec![])
            } else {
                SpecResult::NoMatch
            }
        }

        // List patterns match ListLen test values.
        (FlatPattern::List { elements, .. }, TestValue::ListLen { len, .. }) => {
            if elements.len() == *len as usize {
                SpecResult::Match(elements.clone())
            } else {
                SpecResult::NoMatch
            }
        }

        // Range patterns match IntRange test values.
        (FlatPattern::Range { start, end, .. }, TestValue::IntRange { lo, hi }) => {
            if start.as_ref() == Some(lo) && end.as_ref() == Some(hi) {
                SpecResult::Match(vec![])
            } else {
                SpecResult::NoMatch
            }
        }

        // Or-pattern: match if ANY alternative matches.
        (FlatPattern::Or(alts), tv) => {
            for alt in alts {
                if let SpecResult::Match(subs) = specialize_pattern(alt, tv, expected_sub_count) {
                    return SpecResult::Match(subs);
                }
            }
            SpecResult::NoMatch
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
fn default_matrix(matrix: &PatternMatrix, col: usize, paths: &[ScrutineePath]) -> Specialized {
    let mut new_paths = Vec::with_capacity(paths.len() - 1);
    new_paths.extend_from_slice(&paths[..col]);
    new_paths.extend_from_slice(&paths[col + 1..]);

    let mut new_matrix = Vec::new();
    for row in matrix {
        if row.patterns[col].is_wildcard_like() {
            let mut new_patterns = Vec::with_capacity(row.patterns.len() - 1);
            new_patterns.extend_from_slice(&row.patterns[..col]);
            new_patterns.extend_from_slice(&row.patterns[col + 1..]);
            new_matrix.push(PatternRow {
                patterns: new_patterns,
                arm_index: row.arm_index,
                guard: row.guard,
            });
        }
    }

    Specialized {
        matrix: new_matrix,
        paths: new_paths,
    }
}

// ── Binding Extraction ──────────────────────────────────────────────

/// Extract all variable bindings from a row where every pattern is
/// a wildcard or binding.
fn extract_all_bindings(
    row: &PatternRow,
    paths: &[ScrutineePath],
) -> Vec<(ori_ir::Name, ScrutineePath)> {
    let mut bindings = Vec::new();
    for (pat, path) in row.patterns.iter().zip(paths.iter()) {
        pat.collect_bindings(path, &mut bindings);
    }
    bindings
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use ori_ir::Name;

    use super::*;
    use crate::decision_tree::*;

    /// Helper: create a simple pattern matrix from flat patterns.
    fn matrix(rows: Vec<(Vec<FlatPattern>, usize)>) -> PatternMatrix {
        rows.into_iter()
            .map(|(patterns, arm_index)| PatternRow {
                patterns,
                arm_index,
                guard: None,
            })
            .collect()
    }

    fn paths(n: usize) -> Vec<ScrutineePath> {
        vec![Vec::new(); n]
    }

    // ── Empty and trivial ───────────────────────────────────────

    #[test]
    fn compile_empty_matrix() {
        let tree = compile(vec![], paths(1));
        assert!(matches!(tree, DecisionTree::Fail));
    }

    #[test]
    fn compile_single_wildcard() {
        let m = matrix(vec![(vec![FlatPattern::Wildcard], 0)]);
        let tree = compile(m, paths(1));
        assert!(matches!(tree, DecisionTree::Leaf { arm_index: 0, .. }));
    }

    #[test]
    fn compile_single_binding() {
        let name = Name::from_raw(1);
        let m = matrix(vec![(vec![FlatPattern::Binding(name)], 0)]);
        let tree = compile(m, paths(1));
        if let DecisionTree::Leaf {
            arm_index,
            bindings,
        } = &tree
        {
            assert_eq!(*arm_index, 0);
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].0, name);
        } else {
            panic!("expected Leaf, got {tree:?}");
        }
    }

    // ── Bool matching ───────────────────────────────────────────

    #[test]
    fn compile_bool_exhaustive() {
        // match b { true -> 0, false -> 1 }
        let m = matrix(vec![
            (vec![FlatPattern::LitBool(true)], 0),
            (vec![FlatPattern::LitBool(false)], 1),
        ]);
        let tree = compile(m, paths(1));

        if let DecisionTree::Switch {
            test_kind,
            edges,
            default,
            ..
        } = &tree
        {
            assert_eq!(*test_kind, TestKind::BoolEq);
            assert_eq!(edges.len(), 2);
            assert!(default.is_none());
        } else {
            panic!("expected Switch, got {tree:?}");
        }
    }

    // ── Int matching with default ───────────────────────────────

    #[test]
    fn compile_int_with_default() {
        // match n { 1 -> a, 2 -> b, _ -> c }
        let m = matrix(vec![
            (vec![FlatPattern::LitInt(1)], 0),
            (vec![FlatPattern::LitInt(2)], 1),
            (vec![FlatPattern::Wildcard], 2),
        ]);
        let tree = compile(m, paths(1));

        if let DecisionTree::Switch {
            test_kind,
            edges,
            default,
            ..
        } = &tree
        {
            assert_eq!(*test_kind, TestKind::IntEq);
            assert_eq!(edges.len(), 2);
            assert!(default.is_some());
            if let Some(def) = default {
                assert!(matches!(**def, DecisionTree::Leaf { arm_index: 2, .. }));
            }
        } else {
            panic!("expected Switch, got {tree:?}");
        }
    }

    // ── Enum variant matching ───────────────────────────────────

    #[test]
    fn compile_option_match() {
        // match opt { Some(x) -> use(x), None -> default }
        let name_x = Name::from_raw(1);
        let some_name = Name::from_raw(10);
        let none_name = Name::from_raw(11);

        let m = matrix(vec![
            (
                vec![FlatPattern::Variant {
                    variant_name: some_name,
                    variant_index: 1,
                    fields: vec![FlatPattern::Binding(name_x)],
                }],
                0,
            ),
            (
                vec![FlatPattern::Variant {
                    variant_name: none_name,
                    variant_index: 0,
                    fields: vec![],
                }],
                1,
            ),
        ]);
        let tree = compile(m, paths(1));

        if let DecisionTree::Switch {
            test_kind,
            edges,
            default,
            ..
        } = &tree
        {
            assert_eq!(*test_kind, TestKind::EnumTag);
            assert_eq!(edges.len(), 2);
            assert!(default.is_none());

            // Some(x) edge should have a Leaf with binding for x.
            let (tv, subtree) = &edges[0];
            assert!(matches!(
                tv,
                TestValue::Tag {
                    variant_index: 1,
                    ..
                }
            ));
            if let DecisionTree::Leaf {
                arm_index,
                bindings,
            } = subtree
            {
                assert_eq!(*arm_index, 0);
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].0, name_x);
            } else {
                panic!("expected Leaf for Some arm, got {subtree:?}");
            }

            // None edge should be a Leaf with no bindings.
            let (tv, subtree) = &edges[1];
            assert!(matches!(
                tv,
                TestValue::Tag {
                    variant_index: 0,
                    ..
                }
            ));
            assert!(matches!(subtree, DecisionTree::Leaf { arm_index: 1, .. }));
        } else {
            panic!("expected Switch, got {tree:?}");
        }
    }

    // ── Wildcard mixed with constructors ────────────────────────

    #[test]
    fn compile_variant_with_wildcard() {
        // match opt { Some(x) -> use(x), _ -> default }
        let name_x = Name::from_raw(1);
        let some_name = Name::from_raw(10);

        let m = matrix(vec![
            (
                vec![FlatPattern::Variant {
                    variant_name: some_name,
                    variant_index: 1,
                    fields: vec![FlatPattern::Binding(name_x)],
                }],
                0,
            ),
            (vec![FlatPattern::Wildcard], 1),
        ]);
        let tree = compile(m, paths(1));

        if let DecisionTree::Switch { edges, default, .. } = &tree {
            assert_eq!(edges.len(), 1); // Only Some edge.
            assert!(default.is_some()); // Wildcard becomes default.
            if let Some(def) = default {
                assert!(matches!(**def, DecisionTree::Leaf { arm_index: 1, .. }));
            }
        } else {
            panic!("expected Switch, got {tree:?}");
        }
    }

    // ── Multi-column matching ───────────────────────────────────

    #[test]
    fn compile_two_column_int() {
        // match (a, b) { (1, 2) -> x, (_, _) -> y }
        let m = matrix(vec![
            (vec![FlatPattern::LitInt(1), FlatPattern::LitInt(2)], 0),
            (vec![FlatPattern::Wildcard, FlatPattern::Wildcard], 1),
        ]);
        let tree = compile(m, paths(2));

        // Should produce a nested switch: test col 0, then col 1.
        if let DecisionTree::Switch { edges, default, .. } = &tree {
            assert_eq!(edges.len(), 1); // Only `1` edge.
            assert!(default.is_some()); // Wildcard default.

            // The `1` edge should produce a sub-switch on column 1.
            let (_, subtree) = &edges[0];
            if let DecisionTree::Switch {
                edges: inner_edges,
                default: inner_default,
                ..
            } = subtree
            {
                assert_eq!(inner_edges.len(), 1); // Only `2` edge.
                assert!(inner_default.is_some()); // Wildcard from outer default.
            } else {
                panic!("expected nested Switch, got {subtree:?}");
            }
        } else {
            panic!("expected Switch, got {tree:?}");
        }
    }

    // ── Guard handling ──────────────────────────────────────────

    #[test]
    fn compile_with_guard() {
        use ori_ir::canon::CanId;

        // match x { v if v > 0 -> pos, _ -> other }
        let name_v = Name::from_raw(1);
        let guard_expr = CanId::new(100);

        let m = vec![
            PatternRow {
                patterns: vec![FlatPattern::Binding(name_v)],
                arm_index: 0,
                guard: Some(guard_expr),
            },
            PatternRow {
                patterns: vec![FlatPattern::Wildcard],
                arm_index: 1,
                guard: None,
            },
        ];
        let tree = compile(m, paths(1));

        if let DecisionTree::Guard {
            arm_index,
            guard,
            on_fail,
            ..
        } = &tree
        {
            assert_eq!(*arm_index, 0);
            assert_eq!(*guard, guard_expr);
            assert!(matches!(**on_fail, DecisionTree::Leaf { arm_index: 1, .. }));
        } else {
            panic!("expected Guard, got {tree:?}");
        }
    }

    // ── Tuple decomposition ─────────────────────────────────────

    #[test]
    fn compile_tuple_all_wildcards() {
        // match pair { (a, b) -> use(a, b) }
        // Tuples are single-constructor, so this should Leaf directly
        // after decomposition (since Tuple produces no test values,
        // the first row is all wildcards after the tuple is "matched").
        let name_a = Name::from_raw(1);
        let name_b = Name::from_raw(2);

        let m = matrix(vec![(
            vec![FlatPattern::Tuple(vec![
                FlatPattern::Binding(name_a),
                FlatPattern::Binding(name_b),
            ])],
            0,
        )]);
        let tree = compile(m, paths(1));

        // Single-constructor decomposition: the Tuple is decomposed inline
        // (no Switch), producing a Leaf with bindings for a and b.
        if let DecisionTree::Leaf {
            arm_index,
            bindings,
        } = &tree
        {
            assert_eq!(*arm_index, 0);
            assert_eq!(bindings.len(), 2);
        } else {
            panic!("expected Leaf after tuple decomposition, got {tree:?}");
        }
    }

    // ── Or-pattern ──────────────────────────────────────────────

    #[test]
    fn compile_or_pattern() {
        // match n { 1 | 2 -> a, _ -> b }
        let m = matrix(vec![
            (
                vec![FlatPattern::Or(vec![
                    FlatPattern::LitInt(1),
                    FlatPattern::LitInt(2),
                ])],
                0,
            ),
            (vec![FlatPattern::Wildcard], 1),
        ]);
        let tree = compile(m, paths(1));

        if let DecisionTree::Switch { edges, default, .. } = &tree {
            // Should have edges for both 1 and 2.
            assert_eq!(edges.len(), 2);
            // Both should map to arm 0.
            for (_, subtree) in edges {
                assert!(matches!(subtree, DecisionTree::Leaf { arm_index: 0, .. }));
            }
            // Default maps to arm 1.
            assert!(default.is_some());
        } else {
            panic!("expected Switch, got {tree:?}");
        }
    }

    // ── pick_column heuristic ───────────────────────────────────

    #[test]
    fn pick_column_prefers_more_constructors() {
        // Column 0: all wildcards. Column 1: has constructors.
        let m = matrix(vec![
            (vec![FlatPattern::Wildcard, FlatPattern::LitInt(1)], 0),
            (vec![FlatPattern::Wildcard, FlatPattern::LitInt(2)], 1),
        ]);
        assert_eq!(pick_column(&m), 1);
    }

    #[test]
    fn pick_column_leftmost_on_tie() {
        // Both columns have 1 constructor each. Should pick leftmost (0).
        let m = matrix(vec![(
            vec![FlatPattern::LitInt(1), FlatPattern::LitBool(true)],
            0,
        )]);
        assert_eq!(pick_column(&m), 0);
    }

    // ── String matching ─────────────────────────────────────────

    #[test]
    fn compile_string_match() {
        let hello = Name::from_raw(1);
        let world = Name::from_raw(2);

        let m = matrix(vec![
            (vec![FlatPattern::LitStr(hello)], 0),
            (vec![FlatPattern::LitStr(world)], 1),
            (vec![FlatPattern::Wildcard], 2),
        ]);
        let tree = compile(m, paths(1));

        if let DecisionTree::Switch {
            test_kind, edges, ..
        } = &tree
        {
            assert_eq!(*test_kind, TestKind::StrEq);
            assert_eq!(edges.len(), 2);
        } else {
            panic!("expected Switch, got {tree:?}");
        }
    }

    // ── Or-pattern with variant bindings ──────────────────────

    #[test]
    fn compile_or_pattern_variant_bindings() {
        // match shape { Circle(r) | Sphere(r) -> use(r), _ -> other }
        // Both Circle and Sphere are different constructors sharing arm 0.
        let name_r = Name::from_raw(1);
        let circle = Name::from_raw(10);
        let sphere = Name::from_raw(11);

        let m = matrix(vec![
            (
                vec![FlatPattern::Or(vec![
                    FlatPattern::Variant {
                        variant_name: circle,
                        variant_index: 0,
                        fields: vec![FlatPattern::Binding(name_r)],
                    },
                    FlatPattern::Variant {
                        variant_name: sphere,
                        variant_index: 1,
                        fields: vec![FlatPattern::Binding(name_r)],
                    },
                ])],
                0,
            ),
            (vec![FlatPattern::Wildcard], 1),
        ]);
        let tree = compile(m, paths(1));

        // Should produce a Switch on tag with:
        //   Circle(0) → Leaf(arm 0, r bound)
        //   Sphere(1) → Leaf(arm 0, r bound)  (same arm_index!)
        //   default → Leaf(arm 1)
        if let DecisionTree::Switch {
            test_kind,
            edges,
            default,
            ..
        } = &tree
        {
            assert_eq!(*test_kind, TestKind::EnumTag);
            assert_eq!(edges.len(), 2);

            // Both edges should map to arm 0 with binding for r.
            for (_, subtree) in edges {
                if let DecisionTree::Leaf {
                    arm_index,
                    bindings,
                } = subtree
                {
                    assert_eq!(*arm_index, 0);
                    assert_eq!(bindings.len(), 1);
                    assert_eq!(bindings[0].0, name_r);
                } else {
                    panic!("expected Leaf for or-pattern arm, got {subtree:?}");
                }
            }

            // Default maps to arm 1.
            assert!(default.is_some());
        } else {
            panic!("expected Switch, got {tree:?}");
        }
    }

    // ── Guards with overlapping patterns ──────────────────────

    #[test]
    fn compile_guards_overlapping_variants() {
        use ori_ir::canon::CanId;

        // match opt {
        //   Some(x) if x > 0 -> positive
        //   Some(x) if x < 0 -> negative
        //   Some(x) -> zero
        //   None -> default
        // }
        let name_x = Name::from_raw(1);
        let some_name = Name::from_raw(10);
        let none_name = Name::from_raw(11);
        let guard1 = CanId::new(101);
        let guard2 = CanId::new(102);

        let m = vec![
            PatternRow {
                patterns: vec![FlatPattern::Variant {
                    variant_name: some_name,
                    variant_index: 1,
                    fields: vec![FlatPattern::Binding(name_x)],
                }],
                arm_index: 0,
                guard: Some(guard1),
            },
            PatternRow {
                patterns: vec![FlatPattern::Variant {
                    variant_name: some_name,
                    variant_index: 1,
                    fields: vec![FlatPattern::Binding(name_x)],
                }],
                arm_index: 1,
                guard: Some(guard2),
            },
            PatternRow {
                patterns: vec![FlatPattern::Variant {
                    variant_name: some_name,
                    variant_index: 1,
                    fields: vec![FlatPattern::Binding(name_x)],
                }],
                arm_index: 2,
                guard: None,
            },
            PatternRow {
                patterns: vec![FlatPattern::Variant {
                    variant_name: none_name,
                    variant_index: 0,
                    fields: vec![],
                }],
                arm_index: 3,
                guard: None,
            },
        ];
        let tree = compile(m, paths(1));

        // Should produce:
        //   Switch(tag):
        //     Some → Guard(arm 0, guard1,
        //              on_fail: Guard(arm 1, guard2,
        //                on_fail: Leaf(arm 2)))
        //     None → Leaf(arm 3)
        if let DecisionTree::Switch {
            test_kind, edges, ..
        } = &tree
        {
            assert_eq!(*test_kind, TestKind::EnumTag);

            // Find the Some edge.
            let some_tree = edges.iter().find_map(|(tv, tree)| {
                matches!(
                    tv,
                    TestValue::Tag {
                        variant_index: 1,
                        ..
                    }
                )
                .then_some(tree)
            });
            let Some(some_tree) = some_tree else {
                panic!("should have Some edge");
            };

            // The Some subtree should be Guard(arm 0, on_fail: Guard(arm 1, on_fail: Leaf(arm 2)))
            if let DecisionTree::Guard {
                arm_index: 0,
                guard,
                on_fail,
                ..
            } = some_tree
            {
                assert_eq!(*guard, guard1);
                if let DecisionTree::Guard {
                    arm_index: 1,
                    guard: g2,
                    on_fail: inner_fail,
                    ..
                } = on_fail.as_ref()
                {
                    assert_eq!(*g2, guard2);
                    assert!(matches!(
                        inner_fail.as_ref(),
                        DecisionTree::Leaf { arm_index: 2, .. }
                    ));
                } else {
                    panic!("expected inner Guard, got {on_fail:?}");
                }
            } else {
                panic!("expected Guard for Some arm, got {some_tree:?}");
            }

            // Find the None edge.
            let none_tree = edges.iter().find_map(|(tv, tree)| {
                matches!(
                    tv,
                    TestValue::Tag {
                        variant_index: 0,
                        ..
                    }
                )
                .then_some(tree)
            });
            let Some(none_tree) = none_tree else {
                panic!("should have None edge");
            };
            assert!(matches!(none_tree, DecisionTree::Leaf { arm_index: 3, .. }));
        } else {
            panic!("expected Switch, got {tree:?}");
        }
    }

    // ── Struct decomposition ──────────────────────────────────

    #[test]
    fn compile_struct_decomposition() {
        // match point { { x, y } -> use(x, y) }
        let name_x = Name::from_raw(1);
        let name_y = Name::from_raw(2);
        let field_x = Name::from_raw(10);
        let field_y = Name::from_raw(11);

        let m = matrix(vec![(
            vec![FlatPattern::Struct {
                fields: vec![
                    (field_x, FlatPattern::Binding(name_x)),
                    (field_y, FlatPattern::Binding(name_y)),
                ],
            }],
            0,
        )]);
        let tree = compile(m, paths(1));

        // Struct is single-constructor → decomposed inline → Leaf
        if let DecisionTree::Leaf {
            arm_index,
            bindings,
        } = &tree
        {
            assert_eq!(*arm_index, 0);
            assert_eq!(bindings.len(), 2);
            assert_eq!(bindings[0].0, name_x);
            assert_eq!(bindings[1].0, name_y);
            // Check paths: x at [StructField(0)], y at [StructField(1)]
            assert_eq!(
                bindings[0].1.as_slice(),
                &[super::super::PathInstruction::StructField(0)]
            );
            assert_eq!(
                bindings[1].1.as_slice(),
                &[super::super::PathInstruction::StructField(1)]
            );
        } else {
            panic!("expected Leaf after struct decomposition, got {tree:?}");
        }
    }

    // ── Nested enum inside tuple ──────────────────────────────

    #[test]
    fn compile_nested_enum_in_tuple() {
        // match (tag, x) { (Some(v), _) -> a, (None, _) -> b }
        let name_v = Name::from_raw(1);
        let some_name = Name::from_raw(10);
        let none_name = Name::from_raw(11);

        let m = matrix(vec![
            (
                vec![
                    FlatPattern::Variant {
                        variant_name: some_name,
                        variant_index: 1,
                        fields: vec![FlatPattern::Binding(name_v)],
                    },
                    FlatPattern::Wildcard,
                ],
                0,
            ),
            (
                vec![
                    FlatPattern::Variant {
                        variant_name: none_name,
                        variant_index: 0,
                        fields: vec![],
                    },
                    FlatPattern::Wildcard,
                ],
                1,
            ),
        ]);
        let tree = compile(m, paths(2));

        // Should switch on column 0 (enum tag).
        if let DecisionTree::Switch {
            test_kind, edges, ..
        } = &tree
        {
            assert_eq!(*test_kind, TestKind::EnumTag);
            assert_eq!(edges.len(), 2);

            // Some edge should bind v.
            let (_, some_tree) = &edges[0];
            if let DecisionTree::Leaf {
                arm_index,
                bindings,
            } = some_tree
            {
                assert_eq!(*arm_index, 0);
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].0, name_v);
            } else {
                panic!("expected Leaf for Some arm, got {some_tree:?}");
            }
        } else {
            panic!("expected Switch, got {tree:?}");
        }
    }
}
