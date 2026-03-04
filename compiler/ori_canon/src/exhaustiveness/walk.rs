//! Recursive tree walker for exhaustiveness checking.
//!
//! Traverses a compiled [`DecisionTree`], marking reachable arms and
//! collecting missing pattern descriptions. Variant coverage analysis
//! is delegated back to the parent module's `check_missing_constructors`.

use ori_ir::canon::tree::{DecisionTree, PathInstruction, ScrutineePath, TestKind, TestValue};
use ori_ir::StringInterner;

/// Get the field types for a specific variant of a type.
///
/// Handles Enum, Option, Result, and Ordering types. Returns an empty Vec for
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
        // Ordering (Less/Equal/Greater) and all other types have no variant fields.
        _ => vec![],
    }
}

/// Recursively walk the decision tree, marking reachable arms and collecting
/// missing pattern descriptions.
///
/// `path_types` maps scrutinee paths to their resolved types, enabling
/// exhaustiveness checking at nested Switch nodes (not just the root).
/// `nesting` tracks variant wrappers for diagnostic formatting (e.g.,
/// `["Some({})"]` so missing `"None"` is reported as `"Some(None)"`).
pub(super) fn walk(
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
            missing.push(super::wrap_pattern(nesting, "_"));
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
                let mut added_field_count = 0usize;
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
                            added_field_count = field_types.len();

                            // Record field types for child paths (move into map,
                            // reconstruct keys for cleanup to avoid extra clone).
                            #[expect(
                                clippy::cast_possible_truncation,
                                reason = "field index bounded by variant field count (max ~256)"
                            )]
                            for (i, &ft) in field_types.iter().enumerate() {
                                let mut child_path = path.clone();
                                child_path.push(PathInstruction::TagPayload(i as u32));
                                path_types.insert(child_path, ft);
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
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "field index bounded by variant field count (max ~256)"
                )]
                for i in 0..added_field_count {
                    let mut key = path.clone();
                    key.push(PathInstruction::TagPayload(i as u32));
                    path_types.remove(&key);
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
                super::check_missing_constructors(
                    *test_kind, edges, path, missing, path_types, pool, interner, nesting,
                );
            }
        }
    }
}
