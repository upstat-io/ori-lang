//! Decision tree evaluation for compiled pattern matching.
//!
//! Walks pre-compiled `DecisionTree` nodes (produced by `ori_canon::patterns`)
//! against runtime `Value`s. This is the evaluator counterpart to the Maranget
//! (2008) compilation in `ori_arc::decision_tree::compile`.
//!
//! # Status
//!
//! Implemented and tested independently. NOT wired into the main interpreter
//! dispatch yet — Section 07 (Backend Migration) will do that.
//!
//! # Architecture
//!
//! The walker is pure: it takes a `DecisionTree` and a root `Value`, and returns
//! the matched arm index + variable bindings. Guard evaluation requires a callback
//! because it needs the interpreter's environment.

use ori_ir::canon::tree::{DecisionTree, PathInstruction, ScrutineePath, TestKind, TestValue};
use ori_ir::{Name, StringInterner};
use ori_patterns::{EvalError, Value};

/// Result of evaluating a decision tree: the matched arm index and variable bindings.
#[derive(Debug)]
pub struct MatchResult {
    /// Index of the matched arm in the original match expression.
    pub arm_index: usize,
    /// Variable bindings produced by the match (name → value).
    pub bindings: Vec<(Name, Value)>,
}

/// Evaluate a compiled decision tree against a scrutinee value.
///
/// Walks the tree recursively, testing the scrutinee at each `Switch` node,
/// binding variables at `Leaf` nodes, and evaluating guards via the provided
/// callback at `Guard` nodes.
///
/// # Arguments
///
/// - `tree`: The compiled decision tree.
/// - `scrutinee`: The runtime value being matched.
/// - `interner`: String interner for resolving `Name` values in string comparisons.
/// - `eval_guard`: Callback to evaluate guard expressions. Takes the guard's `CanId`
///   and a slice of bindings; returns `Ok(true)` if the guard passes, `Ok(false)` if
///   it fails, or `Err` for evaluation errors.
///
/// # Returns
///
/// `Ok(MatchResult)` with the matched arm index and bindings, or `Err` if
/// no arm matches (non-exhaustive, should not happen with correct compilation).
pub fn eval_decision_tree<F>(
    tree: &DecisionTree,
    scrutinee: &Value,
    interner: &StringInterner,
    eval_guard: &mut F,
) -> Result<MatchResult, EvalError>
where
    F: FnMut(ori_ir::canon::CanId, &[(Name, Value)]) -> Result<bool, EvalError>,
{
    match tree {
        DecisionTree::Leaf {
            arm_index,
            bindings,
        } => {
            let resolved = resolve_bindings(scrutinee, bindings)?;
            Ok(MatchResult {
                arm_index: *arm_index,
                bindings: resolved,
            })
        }

        DecisionTree::Guard {
            arm_index,
            bindings,
            guard,
            on_fail,
        } => {
            let resolved = resolve_bindings(scrutinee, bindings)?;
            let guard_passed = eval_guard(*guard, &resolved)?;
            if guard_passed {
                Ok(MatchResult {
                    arm_index: *arm_index,
                    bindings: resolved,
                })
            } else {
                eval_decision_tree(on_fail, scrutinee, interner, eval_guard)
            }
        }

        DecisionTree::Switch {
            path,
            test_kind,
            edges,
            default,
        } => {
            let sub_value = resolve_path(scrutinee, path.as_slice())?;
            for (test_value, subtree) in edges {
                if test_matches(&sub_value, *test_kind, test_value, interner) {
                    return eval_decision_tree(subtree, scrutinee, interner, eval_guard);
                }
            }
            // No edge matched — try default.
            if let Some(default_tree) = default {
                eval_decision_tree(default_tree, scrutinee, interner, eval_guard)
            } else {
                Err(EvalError::new("non-exhaustive match: no arm matched"))
            }
        }

        DecisionTree::Fail => Err(EvalError::new("non-exhaustive match: unreachable arm")),
    }
}

// Path resolution

/// Result of one step of ref-based path resolution.
///
/// Most instructions extract a reference to an existing sub-value (`Ref`).
/// `ListRest` constructs a new list value (`Owned`).
enum Resolved<'a> {
    Ref(&'a Value),
    Owned(Value),
}

impl Resolved<'_> {
    fn into_value(self) -> Value {
        match self {
            Resolved::Ref(r) => r.clone(),
            Resolved::Owned(v) => v,
        }
    }
}

/// Navigate from a root value to a sub-value following a scrutinee path.
///
/// Navigates by reference where possible, cloning only the final leaf value.
/// `ListRest` instructions (which construct new lists) fall back to owned
/// navigation for the remainder of the path.
fn resolve_path(root: &Value, path: &[PathInstruction]) -> Result<Value, EvalError> {
    let mut current = Resolved::Ref(root);
    for instruction in path {
        current = match current {
            Resolved::Ref(r) => step_path_ref(r, *instruction)?,
            Resolved::Owned(o) => Resolved::Owned(step_path(&o, *instruction)?),
        };
    }
    Ok(current.into_value())
}

/// Execute one step of ref-based path resolution.
///
/// Returns `Ref` for instructions that extract existing sub-values (zero-copy),
/// and `Owned` for `ListRest` which must construct a new list.
fn step_path_ref(value: &Value, instruction: PathInstruction) -> Result<Resolved<'_>, EvalError> {
    match instruction {
        PathInstruction::TagPayload(field_idx) => {
            let idx = field_idx as usize;
            match value {
                Value::Variant { fields, .. } => {
                    fields.get(idx).map(Resolved::Ref).ok_or_else(|| {
                        EvalError::new(format!(
                            "variant payload index {idx} out of bounds (variant has {} fields)",
                            fields.len()
                        ))
                    })
                }
                Value::Some(inner) if idx == 0 => Ok(Resolved::Ref(inner)),
                Value::Ok(inner) if idx == 0 => Ok(Resolved::Ref(inner)),
                Value::Err(inner) if idx == 0 => Ok(Resolved::Ref(inner)),
                _ => Err(EvalError::new(format!(
                    "cannot extract tag payload from {value:?}"
                ))),
            }
        }

        PathInstruction::TupleIndex(elem_idx) => {
            let idx = elem_idx as usize;
            match value {
                Value::Tuple(elems) => elems.get(idx).map(Resolved::Ref).ok_or_else(|| {
                    EvalError::new(format!(
                        "tuple index {idx} out of bounds (tuple has {} elements)",
                        elems.len()
                    ))
                }),
                _ => Err(EvalError::new(format!(
                    "cannot extract tuple element from {value:?}"
                ))),
            }
        }

        PathInstruction::StructField(field_idx) => {
            let idx = field_idx as usize;
            match value {
                Value::Struct(sv) => sv.fields.get(idx).map(Resolved::Ref).ok_or_else(|| {
                    EvalError::new(format!(
                        "struct field index {idx} out of bounds (struct has {} fields)",
                        sv.fields.len()
                    ))
                }),
                _ => Err(EvalError::new(format!(
                    "cannot extract struct field from {value:?}"
                ))),
            }
        }

        PathInstruction::ListElement(elem_idx) => {
            let idx = elem_idx as usize;
            match value {
                Value::List(items) => items.get(idx).map(Resolved::Ref).ok_or_else(|| {
                    EvalError::new(format!(
                        "list index {idx} out of bounds (list has {} elements)",
                        items.len()
                    ))
                }),
                _ => Err(EvalError::new(format!(
                    "cannot extract list element from {value:?}"
                ))),
            }
        }

        // ListRest constructs a new list — must return Owned.
        PathInstruction::ListRest(start_idx) => {
            let start = start_idx as usize;
            match value {
                Value::List(items) => {
                    let rest = if start <= items.len() {
                        items[start..].to_vec()
                    } else {
                        Vec::new()
                    };
                    Ok(Resolved::Owned(Value::list(rest)))
                }
                _ => Err(EvalError::new(format!(
                    "cannot extract list rest from {value:?}"
                ))),
            }
        }
    }
}

/// Execute one step of owned path resolution (fallback after `ListRest`).
///
/// Thin wrapper over `step_path_ref` that clones any `Ref` result to owned.
fn step_path(value: &Value, instruction: PathInstruction) -> Result<Value, EvalError> {
    step_path_ref(value, instruction).map(Resolved::into_value)
}

/// Resolve all bindings in a leaf/guard node to (Name, Value) pairs.
fn resolve_bindings(
    scrutinee: &Value,
    bindings: &[(Name, ScrutineePath)],
) -> Result<Vec<(Name, Value)>, EvalError> {
    bindings
        .iter()
        .map(|(name, path)| {
            let value = resolve_path(scrutinee, path.as_slice())?;
            Ok((*name, value))
        })
        .collect()
}

// Test matching

/// Check if a runtime value matches a decision tree edge's test value.
fn test_matches(
    value: &Value,
    _test_kind: TestKind,
    test_value: &TestValue,
    interner: &StringInterner,
) -> bool {
    match test_value {
        TestValue::Tag {
            variant_index,
            variant_name,
        } => match value {
            // User-defined enums: compare by interned variant name.
            // TODO(section-07): Use numeric discriminants when evaluator
            // migrates to canonical IR.
            Value::Variant {
                variant_name: vn, ..
            } => *vn == *variant_name,
            // Some/None/Ok/Err are represented as special Value variants.
            Value::Some(_) | Value::Err(_) => *variant_index == 1,
            Value::None | Value::Ok(_) => *variant_index == 0,
            _ => false,
        },

        TestValue::Int(expected) => value.as_int().is_some_and(|v| v == *expected),

        TestValue::Str(expected_name) => {
            if let Some(s) = value.as_str() {
                let expected_str = interner.lookup(*expected_name);
                s == expected_str
            } else {
                false
            }
        }

        TestValue::Bool(expected) => value.as_bool().is_some_and(|v| v == *expected),

        TestValue::Char(expected) => value.as_char().is_some_and(|v| v == *expected),

        TestValue::Float(expected_bits) => value
            .as_float()
            .is_some_and(|v| v.to_bits() == *expected_bits),

        TestValue::IntRange { lo, hi, inclusive } => {
            // Support both int and char ranges (chars compared as code points)
            let v = value
                .as_int()
                .or_else(|| value.as_char().map(|c| i64::from(u32::from(c))));
            v.is_some_and(|v| v >= *lo && if *inclusive { v <= *hi } else { v < *hi })
        }

        TestValue::ListLen { len, is_exact } => match value.as_list() {
            Some(items) => {
                let actual = items.len();
                let expected = *len as usize;
                if *is_exact {
                    actual == expected
                } else {
                    actual >= expected
                }
            }
            _ => false,
        },
    }
}

/// Extended tag test that also supports name-based variant matching.
///
/// The Maranget compiler stores `variant_index` (numeric), but the evaluator
/// represents user-defined variants by name. This helper handles both.
#[cfg(test)]
fn test_tag_by_name(value: &Value, variant_name: Name) -> bool {
    match value {
        Value::Variant {
            variant_name: vn, ..
        } => *vn == variant_name,
        _ => false,
    }
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Tests use expect for brevity")]
mod tests;
