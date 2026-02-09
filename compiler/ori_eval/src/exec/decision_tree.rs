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
/// - `eval_guard`: Callback to evaluate guard expressions. Takes the guard's `ExprId`
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
    F: FnMut(ori_ir::ExprId, &[(Name, Value)]) -> Result<bool, EvalError>,
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

// ── Path Resolution ────────────────────────────────────────────────

/// Navigate from a root value to a sub-value following a scrutinee path.
///
/// Each `PathInstruction` extracts a component: variant payload field,
/// tuple element, struct field, or list element.
fn resolve_path(root: &Value, path: &[PathInstruction]) -> Result<Value, EvalError> {
    let mut current = root.clone();
    for instruction in path {
        current = step_path(&current, *instruction)?;
    }
    Ok(current)
}

/// Execute one step of path resolution.
fn step_path(value: &Value, instruction: PathInstruction) -> Result<Value, EvalError> {
    match instruction {
        PathInstruction::TagPayload(field_idx) => {
            let idx = field_idx as usize;
            match value {
                Value::Variant { fields, .. } => fields.get(idx).cloned().ok_or_else(|| {
                    EvalError::new(format!(
                        "variant payload index {idx} out of bounds (variant has {} fields)",
                        fields.len()
                    ))
                }),
                Value::Some(inner) if idx == 0 => Ok((**inner).clone()),
                Value::Ok(inner) if idx == 0 => Ok((**inner).clone()),
                Value::Err(inner) if idx == 0 => Ok((**inner).clone()),
                _ => Err(EvalError::new(format!(
                    "cannot extract tag payload from {value:?}"
                ))),
            }
        }

        PathInstruction::TupleIndex(elem_idx) => {
            let idx = elem_idx as usize;
            match value {
                Value::Tuple(elems) => elems.get(idx).cloned().ok_or_else(|| {
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
                Value::Struct(sv) => sv.fields.get(idx).cloned().ok_or_else(|| {
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
                Value::List(items) => items.get(idx).cloned().ok_or_else(|| {
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
    }
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

// ── Test Matching ──────────────────────────────────────────────────

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

        TestValue::Float(expected_bits) => value
            .as_float()
            .is_some_and(|v| v.to_bits() == *expected_bits),

        TestValue::IntRange { lo, hi } => value.as_int().is_some_and(|v| v >= *lo && v <= *hi),

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
pub fn test_tag_by_name(value: &Value, variant_name: Name) -> bool {
    match value {
        Value::Variant {
            variant_name: vn, ..
        } => *vn == variant_name,
        _ => false,
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Tests use expect for brevity")]
mod tests {
    use ori_ir::canon::tree::{DecisionTree, PathInstruction, TestKind, TestValue};
    use ori_ir::{ExprId, Name, SharedInterner};
    use ori_patterns::Value;

    use super::{eval_decision_tree, resolve_path, test_tag_by_name};

    fn test_interner() -> SharedInterner {
        SharedInterner::new()
    }

    // No guard callback — panics if called.
    fn no_guard(_: ExprId, _: &[(Name, Value)]) -> Result<bool, ori_patterns::EvalError> {
        panic!("guard should not be called in this test")
    }

    // ── resolve_path ──────────────────────────────────────────

    #[test]
    fn resolve_empty_path() {
        let value = Value::int(42);
        let result = resolve_path(&value, &[]).expect("should resolve");
        assert_eq!(result.as_int(), Some(42));
    }

    #[test]
    fn resolve_tuple_index() {
        let value = Value::tuple(vec![Value::int(1), Value::int(2), Value::int(3)]);
        let path = vec![PathInstruction::TupleIndex(1)];
        let result = resolve_path(&value, &path).expect("should resolve");
        assert_eq!(result.as_int(), Some(2));
    }

    #[test]
    fn resolve_nested_tuple() {
        // ((10, 20), 30)
        let inner = Value::tuple(vec![Value::int(10), Value::int(20)]);
        let outer = Value::tuple(vec![inner, Value::int(30)]);
        let path = vec![
            PathInstruction::TupleIndex(0),
            PathInstruction::TupleIndex(1),
        ];
        let result = resolve_path(&outer, &path).expect("should resolve");
        assert_eq!(result.as_int(), Some(20));
    }

    #[test]
    fn resolve_list_element() {
        let value = Value::list(vec![Value::int(10), Value::int(20), Value::int(30)]);
        let path = vec![PathInstruction::ListElement(2)];
        let result = resolve_path(&value, &path).expect("should resolve");
        assert_eq!(result.as_int(), Some(30));
    }

    #[test]
    fn resolve_some_payload() {
        let value = Value::some(Value::int(99));
        let path = vec![PathInstruction::TagPayload(0)];
        let result = resolve_path(&value, &path).expect("should resolve");
        assert_eq!(result.as_int(), Some(99));
    }

    #[test]
    fn resolve_out_of_bounds() {
        let value = Value::tuple(vec![Value::int(1)]);
        let path = vec![PathInstruction::TupleIndex(5)];
        assert!(resolve_path(&value, &path).is_err());
    }

    // ── eval_decision_tree: Leaf ──────────────────────────────

    #[test]
    fn leaf_returns_arm_index() {
        let tree = DecisionTree::Leaf {
            arm_index: 2,
            bindings: vec![],
        };
        let interner = test_interner();
        let result = eval_decision_tree(&tree, &Value::int(0), &interner, &mut no_guard)
            .expect("should match");
        assert_eq!(result.arm_index, 2);
        assert!(result.bindings.is_empty());
    }

    #[test]
    fn leaf_with_binding() {
        let interner = test_interner();
        let name_x = interner.intern("x");
        let tree = DecisionTree::Leaf {
            arm_index: 0,
            bindings: vec![(name_x, vec![])], // bind x to root scrutinee
        };
        let result = eval_decision_tree(&tree, &Value::int(42), &interner, &mut no_guard)
            .expect("should match");
        assert_eq!(result.arm_index, 0);
        assert_eq!(result.bindings.len(), 1);
        assert_eq!(result.bindings[0].0, name_x);
        assert_eq!(result.bindings[0].1.as_int(), Some(42));
    }

    // ── eval_decision_tree: Switch ────────────────────────────

    #[test]
    fn switch_bool() {
        // match b { true -> arm 0, false -> arm 1 }
        let tree = DecisionTree::Switch {
            path: vec![],
            test_kind: TestKind::BoolEq,
            edges: vec![
                (
                    TestValue::Bool(true),
                    DecisionTree::Leaf {
                        arm_index: 0,
                        bindings: vec![],
                    },
                ),
                (
                    TestValue::Bool(false),
                    DecisionTree::Leaf {
                        arm_index: 1,
                        bindings: vec![],
                    },
                ),
            ],
            default: None,
        };
        let interner = test_interner();

        let r1 = eval_decision_tree(&tree, &Value::Bool(true), &interner, &mut no_guard)
            .expect("should match true");
        assert_eq!(r1.arm_index, 0);

        let r2 = eval_decision_tree(&tree, &Value::Bool(false), &interner, &mut no_guard)
            .expect("should match false");
        assert_eq!(r2.arm_index, 1);
    }

    #[test]
    fn switch_int_with_default() {
        // match n { 1 -> arm 0, 2 -> arm 1, _ -> arm 2 }
        let tree = DecisionTree::Switch {
            path: vec![],
            test_kind: TestKind::IntEq,
            edges: vec![
                (
                    TestValue::Int(1),
                    DecisionTree::Leaf {
                        arm_index: 0,
                        bindings: vec![],
                    },
                ),
                (
                    TestValue::Int(2),
                    DecisionTree::Leaf {
                        arm_index: 1,
                        bindings: vec![],
                    },
                ),
            ],
            default: Some(Box::new(DecisionTree::Leaf {
                arm_index: 2,
                bindings: vec![],
            })),
        };
        let interner = test_interner();

        let r1 = eval_decision_tree(&tree, &Value::int(1), &interner, &mut no_guard)
            .expect("should match 1");
        assert_eq!(r1.arm_index, 0);

        let r2 = eval_decision_tree(&tree, &Value::int(2), &interner, &mut no_guard)
            .expect("should match 2");
        assert_eq!(r2.arm_index, 1);

        let r3 = eval_decision_tree(&tree, &Value::int(999), &interner, &mut no_guard)
            .expect("should match default");
        assert_eq!(r3.arm_index, 2);
    }

    #[test]
    fn switch_option_tag() {
        // match opt { Some(v) -> arm 0 with v, None -> arm 1 }
        let interner = test_interner();
        let name_v = interner.intern("v");

        let tree = DecisionTree::Switch {
            path: vec![],
            test_kind: TestKind::EnumTag,
            edges: vec![
                (
                    TestValue::Tag {
                        variant_index: 1,
                        variant_name: interner.intern("Some"),
                    },
                    DecisionTree::Leaf {
                        arm_index: 0,
                        bindings: vec![(name_v, vec![PathInstruction::TagPayload(0)])],
                    },
                ),
                (
                    TestValue::Tag {
                        variant_index: 0,
                        variant_name: interner.intern("None"),
                    },
                    DecisionTree::Leaf {
                        arm_index: 1,
                        bindings: vec![],
                    },
                ),
            ],
            default: None,
        };

        // Some(42) → arm 0 with v=42
        let r1 = eval_decision_tree(
            &tree,
            &Value::some(Value::int(42)),
            &interner,
            &mut no_guard,
        )
        .expect("should match Some");
        assert_eq!(r1.arm_index, 0);
        assert_eq!(r1.bindings.len(), 1);
        assert_eq!(r1.bindings[0].0, name_v);
        assert_eq!(r1.bindings[0].1.as_int(), Some(42));

        // None → arm 1
        let r2 = eval_decision_tree(&tree, &Value::None, &interner, &mut no_guard)
            .expect("should match None");
        assert_eq!(r2.arm_index, 1);
    }

    // ── eval_decision_tree: Guard ─────────────────────────────

    #[test]
    fn guard_pass() {
        // match x { v if guard -> arm 0, _ -> arm 1 }
        let interner = test_interner();
        let name_v = interner.intern("v");

        let tree = DecisionTree::Guard {
            arm_index: 0,
            bindings: vec![(name_v, vec![])],
            guard: ExprId::new(100),
            on_fail: Box::new(DecisionTree::Leaf {
                arm_index: 1,
                bindings: vec![],
            }),
        };

        // Guard passes → arm 0.
        let mut guard_fn = |_: ExprId, _: &[(Name, Value)]| Ok(true);
        let r = eval_decision_tree(&tree, &Value::int(5), &interner, &mut guard_fn)
            .expect("should match");
        assert_eq!(r.arm_index, 0);
        assert_eq!(r.bindings[0].1.as_int(), Some(5));
    }

    #[test]
    fn guard_fail_falls_through() {
        // match x { v if guard -> arm 0, _ -> arm 1 }
        let interner = test_interner();
        let name_v = interner.intern("v");

        let tree = DecisionTree::Guard {
            arm_index: 0,
            bindings: vec![(name_v, vec![])],
            guard: ExprId::new(100),
            on_fail: Box::new(DecisionTree::Leaf {
                arm_index: 1,
                bindings: vec![],
            }),
        };

        // Guard fails → fall through to arm 1.
        let mut guard_fn = |_: ExprId, _: &[(Name, Value)]| Ok(false);
        let r = eval_decision_tree(&tree, &Value::int(5), &interner, &mut guard_fn)
            .expect("should match");
        assert_eq!(r.arm_index, 1);
    }

    // ── eval_decision_tree: Fail ──────────────────────────────

    #[test]
    fn fail_returns_error() {
        let tree = DecisionTree::Fail;
        let interner = test_interner();
        let r = eval_decision_tree(&tree, &Value::int(0), &interner, &mut no_guard);
        assert!(r.is_err());
    }

    // ── test_tag_by_name ──────────────────────────────────────

    #[test]
    fn tag_by_name_matches_variant() {
        let interner = test_interner();
        let variant_name = interner.intern("Running");
        let type_name = interner.intern("Status");

        let value = Value::variant(type_name, variant_name, vec![]);
        assert!(test_tag_by_name(&value, variant_name));
    }

    #[test]
    fn tag_by_name_different_variant() {
        let interner = test_interner();
        let running = interner.intern("Running");
        let stopped = interner.intern("Stopped");
        let type_name = interner.intern("Status");

        let value = Value::variant(type_name, running, vec![]);
        assert!(!test_tag_by_name(&value, stopped));
    }
}
