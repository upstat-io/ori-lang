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

// Single-constructor decomposition

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
) -> super::PathInstruction {
    use super::PathInstruction;
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

// Test value collection

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
fn infer_test_kind(values: &[TestValue]) -> TestKind {
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
fn default_matrix(matrix: &PatternMatrix, col: usize, paths: &[ScrutineePath]) -> Specialized {
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
fn collect_consumed_bindings(
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
