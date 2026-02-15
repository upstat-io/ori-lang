//! Pattern exhaustiveness and redundancy checking.
//!
//! Walks a compiled [`DecisionTree`] to detect two classes of problems:
//!
//! 1. **Non-exhaustive matches**: A reachable `Fail` node or a `Switch` with
//!    no default and missing constructors means some runtime value has no match.
//!
//! 2. **Redundant arms**: An arm that is never reached by any path through the
//!    tree is dead code.
//!
//! # Algorithm
//!
//! Rather than re-analyzing source patterns (which would duplicate the Maranget
//! algorithm's work), we walk the already-compiled decision tree. This is sound
//! because the tree is a faithful encoding of the pattern matrix's coverage.
//!
//! # Scope
//!
//! - Phase 1: Bool exhaustiveness, infinite type detection, redundant arm detection
//! - Phase 2: Enum variant enumeration (user-defined, Option, Result) at root level
//! - Phase 3: Nested enum exhaustiveness, list pattern coverage analysis

use ori_ir::canon::tree::{DecisionTree, PathInstruction, ScrutineePath, TestKind, TestValue};
use ori_ir::canon::PatternProblem;
use ori_ir::{Span, StringInterner};

/// Result of exhaustiveness checking on a single decision tree.
pub(crate) struct CheckResult {
    pub problems: Vec<PatternProblem>,
}

/// Get the field types for a specific variant of a type.
///
/// Handles Enum, Option, and Result types. Returns an empty Vec for
/// variants with no fields or when the type is not a recognized enum kind.
fn variant_field_types(
    pool: &ori_types::Pool,
    type_idx: ori_types::Idx,
    variant_index: u32,
) -> Vec<ori_types::Idx> {
    let tag = pool.tag(type_idx);
    match tag {
        ori_types::Tag::Enum => {
            let (_, fields) = pool.enum_variant(type_idx, variant_index as usize);
            fields
        }
        ori_types::Tag::Option => match variant_index {
            1 => vec![pool.option_inner(type_idx)], // Some(T)
            _ => vec![],                            // None or unknown
        },
        ori_types::Tag::Result => match variant_index {
            0 => vec![pool.result_ok(type_idx)],  // Ok(T)
            1 => vec![pool.result_err(type_idx)], // Err(E)
            _ => vec![],
        },
        _ => vec![],
    }
}

/// Wrap a missing pattern in the nesting context for diagnostics.
///
/// Each entry in `nesting` is a wrapper string like `"Some({})"`.
/// Wrapping is applied from innermost to outermost:
/// `["Some({})", "Ok({})"]` wrapping `"None"` → `"Some(Ok(None))"`.
fn wrap_pattern(nesting: &[String], pattern: &str) -> String {
    if nesting.is_empty() {
        return pattern.to_string();
    }
    let mut result = pattern.to_string();
    for wrapper in nesting.iter().rev() {
        result = wrapper.replace("{}", &result);
    }
    result
}

/// Check a compiled decision tree for exhaustiveness and redundancy.
///
/// # Arguments
///
/// - `tree`: The compiled decision tree from pattern compilation.
/// - `arm_count`: Total number of arms in the source match expression.
/// - `match_span`: Span of the match expression (for diagnostics).
/// - `arm_spans`: Span of each arm (indexed by `arm_index`).
/// - `scrutinee_type`: Type of the match scrutinee (for enum variant enumeration).
/// - `pool`: Type pool for looking up enum definitions.
/// - `interner`: String interner for variant name lookup.
pub(crate) fn check_exhaustiveness(
    tree: &DecisionTree,
    arm_count: usize,
    match_span: Span,
    arm_spans: &[Span],
    scrutinee_type: ori_types::Idx,
    pool: &ori_types::Pool,
    interner: &StringInterner,
) -> CheckResult {
    let mut reachable = vec![false; arm_count];
    let mut missing = Vec::new();
    let mut path_types = rustc_hash::FxHashMap::default();
    path_types.insert(vec![], scrutinee_type);
    let mut nesting = Vec::new();

    walk(
        tree,
        &mut reachable,
        &mut missing,
        pool,
        interner,
        &mut path_types,
        &mut nesting,
    );

    let mut problems = Vec::new();

    // Non-exhaustive: any missing patterns found during walk.
    if !missing.is_empty() {
        // Deduplicate missing patterns (multiple Fail nodes may report "_").
        missing.sort();
        missing.dedup();
        problems.push(PatternProblem::NonExhaustive {
            match_span,
            missing,
        });
    }

    // Redundant arms: any arm not reachable by any path through the tree.
    for (i, &reached) in reachable.iter().enumerate() {
        if !reached {
            let arm_span = arm_spans.get(i).copied().unwrap_or(match_span);
            problems.push(PatternProblem::RedundantArm {
                arm_span,
                match_span,
                arm_index: i,
            });
        }
    }

    CheckResult { problems }
}

/// Recursively walk the decision tree, marking reachable arms and collecting
/// missing pattern descriptions.
///
/// `path_types` maps scrutinee paths to their resolved types, enabling
/// exhaustiveness checking at nested Switch nodes (not just the root).
/// `nesting` tracks variant wrappers for diagnostic formatting (e.g.,
/// `["Some({})"]` so missing `"None"` is reported as `"Some(None)"`).
fn walk(
    tree: &DecisionTree,
    reachable: &mut [bool],
    missing: &mut Vec<String>,
    pool: &ori_types::Pool,
    interner: &StringInterner,
    path_types: &mut rustc_hash::FxHashMap<ScrutineePath, ori_types::Idx>,
    nesting: &mut Vec<String>,
) {
    match tree {
        DecisionTree::Leaf { arm_index, .. } => {
            if let Some(slot) = reachable.get_mut(*arm_index) {
                *slot = true;
            }
        }
        DecisionTree::Guard {
            arm_index, on_fail, ..
        } => {
            // The guarded arm is reachable (guard may succeed).
            if let Some(slot) = reachable.get_mut(*arm_index) {
                *slot = true;
            }
            // Walk the on_fail subtree (guard may also fail).
            walk(
                on_fail, reachable, missing, pool, interner, path_types, nesting,
            );
        }
        DecisionTree::Fail => {
            // A Fail node means the matrix was empty at this point —
            // some value reaches here with no matching arm.
            missing.push(wrap_pattern(nesting, "_"));
        }
        DecisionTree::Switch {
            path,
            test_kind,
            edges,
            default,
        } => {
            // Walk each edge subtree. For EnumTag edges, populate child path
            // types so nested switches can resolve their scrutinee type.
            for (test_value, subtree) in edges {
                let mut added_paths = Vec::new();
                let mut pushed_wrapper = false;

                if *test_kind == TestKind::EnumTag {
                    if let TestValue::Tag {
                        variant_index,
                        variant_name,
                    } = test_value
                    {
                        if let Some(&type_at_path) = path_types.get(path) {
                            let resolved = pool.resolve_fully(type_at_path);
                            let field_types = variant_field_types(pool, resolved, *variant_index);

                            // Record field types for child paths.
                            #[expect(
                                clippy::cast_possible_truncation,
                                reason = "field index bounded by variant field count (max ~256)"
                            )]
                            for (i, &ft) in field_types.iter().enumerate() {
                                let mut child_path = path.clone();
                                child_path.push(PathInstruction::TagPayload(i as u32));
                                path_types.insert(child_path.clone(), ft);
                                added_paths.push(child_path);
                            }

                            // Push nesting wrapper for diagnostic formatting.
                            // Single-field: "Some({})", multi-field: "Pair({}, _)"
                            if !field_types.is_empty() {
                                let name = interner.lookup(*variant_name);
                                let wrapper = if field_types.len() == 1 {
                                    format!("{name}({{}})")
                                } else {
                                    // Multi-field: first field gets the placeholder,
                                    // remaining fields get wildcards.
                                    let mut parts = vec!["{}".to_string()];
                                    parts.extend(std::iter::repeat_n(
                                        "_".to_string(),
                                        field_types.len() - 1,
                                    ));
                                    format!("{name}({})", parts.join(", "))
                                };
                                nesting.push(wrapper);
                                pushed_wrapper = true;
                            }
                        }
                    }
                }

                walk(
                    subtree, reachable, missing, pool, interner, path_types, nesting,
                );

                // Cleanup: remove child types and nesting wrapper to avoid
                // leaking context to sibling edges.
                if pushed_wrapper {
                    nesting.pop();
                }
                for p in &added_paths {
                    path_types.remove(p);
                }
            }

            // Walk default if present.
            if let Some(default_tree) = default {
                walk(
                    default_tree,
                    reachable,
                    missing,
                    pool,
                    interner,
                    path_types,
                    nesting,
                );
            } else {
                // No default branch — check if all constructors are covered.
                check_missing_constructors(
                    *test_kind, edges, path, missing, path_types, pool, interner, nesting,
                );
            }
        }
    }
}

/// When a `Switch` has no default, check whether all constructors of the
/// tested type are covered by edges. If not, report what's missing.
#[expect(
    clippy::too_many_arguments,
    reason = "internal dispatch helper: splitting into config struct adds complexity without benefit"
)]
fn check_missing_constructors(
    test_kind: TestKind,
    edges: &[(TestValue, DecisionTree)],
    path: &ScrutineePath,
    missing: &mut Vec<String>,
    path_types: &rustc_hash::FxHashMap<ScrutineePath, ori_types::Idx>,
    pool: &ori_types::Pool,
    interner: &StringInterner,
    nesting: &[String],
) {
    match test_kind {
        TestKind::BoolEq => {
            let has_true = edges
                .iter()
                .any(|(v, _)| matches!(v, TestValue::Bool(true)));
            let has_false = edges
                .iter()
                .any(|(v, _)| matches!(v, TestValue::Bool(false)));

            if !has_true {
                missing.push(wrap_pattern(nesting, "true"));
            }
            if !has_false {
                missing.push(wrap_pattern(nesting, "false"));
            }
        }
        // Infinite types: int, str, float, int ranges — cannot enumerate
        // all values. Without a default (wildcard), the match is necessarily
        // non-exhaustive.
        TestKind::IntEq
        | TestKind::StrEq
        | TestKind::FloatEq
        | TestKind::CharEq
        | TestKind::IntRange => {
            missing.push(wrap_pattern(nesting, "_"));
        }
        // List lengths are quasi-finite: a rest pattern ([x, ..rest]) covers
        // all lengths >= its minimum, so exact lengths 0..min-1 plus one rest
        // pattern can be exhaustive.
        TestKind::ListLen => {
            check_list_len(edges, missing, nesting);
        }
        TestKind::EnumTag => {
            if let Some(&type_at_path) = path_types.get(path) {
                check_enum_tag(edges, missing, type_at_path, pool, interner, nesting);
            }
            // Type unknown at this path → skip (conservative, no false positives).
        }
    }
}

/// Check whether all enum constructors are covered for an `EnumTag` switch.
///
/// Supports both root-level and nested switches. The type at the switch's
/// scrutinee path is resolved via `path_types` and passed directly.
fn check_enum_tag(
    edges: &[(TestValue, DecisionTree)],
    missing: &mut Vec<String>,
    type_at_path: ori_types::Idx,
    pool: &ori_types::Pool,
    interner: &StringInterner,
    nesting: &[String],
) {
    // Resolve the scrutinee type through variable links and type aliases.
    let resolved = pool.resolve_fully(type_at_path);
    let tag = pool.tag(resolved);

    // Collect covered variant indices from edges.
    let covered: rustc_hash::FxHashSet<u32> = edges
        .iter()
        .filter_map(|(tv, _)| match tv {
            TestValue::Tag { variant_index, .. } => Some(*variant_index),
            _ => None,
        })
        .collect();

    match tag {
        ori_types::Tag::Enum => {
            check_user_enum(pool, interner, resolved, &covered, missing, nesting);
        }
        ori_types::Tag::Option => {
            check_option(&covered, missing, nesting);
        }
        ori_types::Tag::Result => {
            check_result(&covered, missing, nesting);
        }
        // Unknown or non-enum type at this path — skip (conservative).
        // This can happen if the type checker couldn't resolve the scrutinee.
        _ => {}
    }
}

/// Check whether a variant is uninhabited (can never be constructed).
///
/// A variant is uninhabited if any of its fields has type `Never`,
/// since a `Never` value can never be produced.
fn is_variant_uninhabited(fields: &[ori_types::Idx], pool: &ori_types::Pool) -> bool {
    fields
        .iter()
        .any(|&f| pool.tag(pool.resolve_fully(f)) == ori_types::Tag::Never)
}

/// Check coverage for a user-defined enum type.
///
/// Queries the type pool for all variants and reports any that are not
/// covered by the switch edges. Variants with `Never`-typed fields are
/// skipped because they are uninhabited (can never be constructed).
fn check_user_enum(
    pool: &ori_types::Pool,
    interner: &StringInterner,
    enum_idx: ori_types::Idx,
    covered: &rustc_hash::FxHashSet<u32>,
    missing: &mut Vec<String>,
    nesting: &[String],
) {
    let count = pool.enum_variant_count(enum_idx);

    #[expect(
        clippy::cast_possible_truncation,
        reason = "enum variant count bounded by u8 (max 256)"
    )]
    for i in 0..count {
        let idx = i as u32;
        if !covered.contains(&idx) {
            let (vname, fields) = pool.enum_variant(enum_idx, i);
            // Skip uninhabited variants — they can never be constructed
            if is_variant_uninhabited(&fields, pool) {
                continue;
            }
            let name = interner.lookup(vname);
            let pattern = if fields.is_empty() {
                name.to_string()
            } else {
                let wildcards = vec!["_"; fields.len()].join(", ");
                format!("{name}({wildcards})")
            };
            missing.push(wrap_pattern(nesting, &pattern));
        }
    }
}

/// Check coverage for the builtin `Option` type.
///
/// Option has exactly 2 variants: `None` (index 0) and `Some` (index 1).
fn check_option(
    covered: &rustc_hash::FxHashSet<u32>,
    missing: &mut Vec<String>,
    nesting: &[String],
) {
    if !covered.contains(&0) {
        missing.push(wrap_pattern(nesting, "None"));
    }
    if !covered.contains(&1) {
        missing.push(wrap_pattern(nesting, "Some(_)"));
    }
}

/// Check coverage for the builtin `Result` type.
///
/// Result has exactly 2 variants: `Ok` (index 0) and `Err` (index 1).
fn check_result(
    covered: &rustc_hash::FxHashSet<u32>,
    missing: &mut Vec<String>,
    nesting: &[String],
) {
    if !covered.contains(&0) {
        missing.push(wrap_pattern(nesting, "Ok(_)"));
    }
    if !covered.contains(&1) {
        missing.push(wrap_pattern(nesting, "Err(_)"));
    }
}

/// Check coverage for a `ListLen` switch with no default.
///
/// List lengths are quasi-finite: a rest pattern (`[x, ..rest]`, encoded as
/// `is_exact: false`) covers all lists of length >= its minimum. Combined
/// with exact-length edges for shorter lists, the match can be exhaustive.
///
/// # Exhaustiveness Rules
///
/// 1. If any edge has `is_exact: false` with `len = L`, it covers lengths >= L.
///    The match is exhaustive if exact edges cover every length in 0..L.
/// 2. If all edges are `is_exact: true`, the type is effectively infinite
///    (uncountably many lengths not covered) — report `_` missing.
/// 3. Multiple rest patterns: use the one with the smallest `len` (covers
///    the most lengths). Only exact lengths below that minimum matter.
fn check_list_len(
    edges: &[(TestValue, DecisionTree)],
    missing: &mut Vec<String>,
    nesting: &[String],
) {
    // Find the rest pattern with the smallest minimum length (covers most).
    let min_rest_len: Option<u32> = edges
        .iter()
        .filter_map(|(tv, _)| match tv {
            TestValue::ListLen {
                len,
                is_exact: false,
            } => Some(*len),
            _ => None,
        })
        .min();

    let Some(rest_min) = min_rest_len else {
        // No rest patterns — all edges are exact lengths.
        // Cannot cover infinite number of possible list lengths.
        missing.push(wrap_pattern(nesting, "_"));
        return;
    };

    // Rest pattern covers lengths >= rest_min.
    // Check that exact edges cover every length in 0..rest_min.
    let exact_lengths: rustc_hash::FxHashSet<u32> = edges
        .iter()
        .filter_map(|(tv, _)| match tv {
            TestValue::ListLen {
                len,
                is_exact: true,
            } => Some(*len),
            _ => None,
        })
        .collect();

    for len in 0..rest_min {
        if !exact_lengths.contains(&len) {
            // Format the missing pattern as a list of wildcards.
            let wildcards = vec!["_"; len as usize].join(", ");
            let pattern = format!("[{wildcards}]");
            missing.push(wrap_pattern(nesting, &pattern));
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
