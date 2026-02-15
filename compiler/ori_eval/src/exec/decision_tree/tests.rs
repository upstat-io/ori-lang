use ori_ir::canon::tree::{DecisionTree, PathInstruction, TestKind, TestValue};
use ori_ir::canon::CanId;
use ori_ir::{Name, SharedInterner};
use ori_patterns::Value;

use super::{eval_decision_tree, resolve_path, test_tag_by_name};

fn test_interner() -> SharedInterner {
    SharedInterner::new()
}

// No guard callback — panics if called.
fn no_guard(_: CanId, _: &[(Name, Value)]) -> Result<bool, ori_patterns::EvalError> {
    panic!("guard should not be called in this test")
}

// resolve_path

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

// eval_decision_tree: Leaf

#[test]
fn leaf_returns_arm_index() {
    let tree = DecisionTree::Leaf {
        arm_index: 2,
        bindings: vec![],
    };
    let interner = test_interner();
    let result =
        eval_decision_tree(&tree, &Value::int(0), &interner, &mut no_guard).expect("should match");
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
    let result =
        eval_decision_tree(&tree, &Value::int(42), &interner, &mut no_guard).expect("should match");
    assert_eq!(result.arm_index, 0);
    assert_eq!(result.bindings.len(), 1);
    assert_eq!(result.bindings[0].0, name_x);
    assert_eq!(result.bindings[0].1.as_int(), Some(42));
}

// eval_decision_tree: Switch

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

// eval_decision_tree: Guard

#[test]
fn guard_pass() {
    // match x { v if guard -> arm 0, _ -> arm 1 }
    let interner = test_interner();
    let name_v = interner.intern("v");

    let tree = DecisionTree::Guard {
        arm_index: 0,
        bindings: vec![(name_v, vec![])],
        guard: CanId::new(100),
        on_fail: Box::new(DecisionTree::Leaf {
            arm_index: 1,
            bindings: vec![],
        }),
    };

    // Guard passes → arm 0.
    let mut guard_fn = |_: CanId, _: &[(Name, Value)]| Ok(true);
    let r =
        eval_decision_tree(&tree, &Value::int(5), &interner, &mut guard_fn).expect("should match");
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
        guard: CanId::new(100),
        on_fail: Box::new(DecisionTree::Leaf {
            arm_index: 1,
            bindings: vec![],
        }),
    };

    // Guard fails → fall through to arm 1.
    let mut guard_fn = |_: CanId, _: &[(Name, Value)]| Ok(false);
    let r =
        eval_decision_tree(&tree, &Value::int(5), &interner, &mut guard_fn).expect("should match");
    assert_eq!(r.arm_index, 1);
}

// eval_decision_tree: Fail

#[test]
fn fail_returns_error() {
    let tree = DecisionTree::Fail;
    let interner = test_interner();
    let r = eval_decision_tree(&tree, &Value::int(0), &interner, &mut no_guard);
    assert!(r.is_err());
}

// test_tag_by_name

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
