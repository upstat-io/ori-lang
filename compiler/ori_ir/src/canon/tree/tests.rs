use std::mem;

use crate::Name;

use super::*;

// ── PathInstruction ─────────────────────────────────────────

#[test]
fn path_instruction_size() {
    // PathInstruction should be small (8 bytes: discriminant + u32).
    assert!(mem::size_of::<PathInstruction>() <= 8);
}

#[test]
fn path_instruction_equality() {
    assert_eq!(
        PathInstruction::TagPayload(0),
        PathInstruction::TagPayload(0),
    );
    assert_ne!(
        PathInstruction::TagPayload(0),
        PathInstruction::TagPayload(1),
    );
    assert_ne!(
        PathInstruction::TagPayload(0),
        PathInstruction::TupleIndex(0),
    );
}

// ── ScrutineePath ───────────────────────────────────────────

#[test]
fn scrutinee_path_empty() {
    let path: ScrutineePath = Vec::new();
    assert!(path.is_empty());
}

#[test]
fn scrutinee_path_multiple_elements() {
    let path: ScrutineePath = vec![
        PathInstruction::TagPayload(0),
        PathInstruction::TupleIndex(1),
        PathInstruction::StructField(2),
        PathInstruction::ListElement(3),
    ];
    assert_eq!(path.len(), 4);
}

#[test]
fn scrutinee_path_clone_independence() {
    let original: ScrutineePath = vec![PathInstruction::TagPayload(0)];
    let mut cloned = original.clone();
    cloned.push(PathInstruction::TupleIndex(1));
    assert_eq!(original.len(), 1);
    assert_eq!(cloned.len(), 2);
}

// ── TestKind ────────────────────────────────────────────────

#[test]
fn test_kind_equality() {
    assert_eq!(TestKind::EnumTag, TestKind::EnumTag);
    assert_ne!(TestKind::EnumTag, TestKind::IntEq);
}

// ── TestValue ───────────────────────────────────────────────

#[test]
fn test_value_tag() {
    let tv = TestValue::Tag {
        variant_index: 0,
        variant_name: Name::from_raw(42),
    };
    assert_eq!(
        tv,
        TestValue::Tag {
            variant_index: 0,
            variant_name: Name::from_raw(42),
        }
    );
}

#[test]
fn test_value_int() {
    assert_eq!(TestValue::Int(42), TestValue::Int(42));
    assert_ne!(TestValue::Int(42), TestValue::Int(43));
}

#[test]
fn test_value_bool() {
    assert_ne!(TestValue::Bool(true), TestValue::Bool(false));
}

#[test]
fn test_value_str() {
    let s1 = TestValue::Str(Name::from_raw(1));
    let s2 = TestValue::Str(Name::from_raw(1));
    assert_eq!(s1, s2);
}

#[test]
fn test_value_list_len() {
    let exact = TestValue::ListLen {
        len: 3,
        is_exact: true,
    };
    let min = TestValue::ListLen {
        len: 3,
        is_exact: false,
    };
    assert_ne!(exact, min);
}

#[test]
fn test_value_int_range() {
    let r = TestValue::IntRange {
        lo: 1,
        hi: 10,
        inclusive: false,
    };
    assert_eq!(
        r,
        TestValue::IntRange {
            lo: 1,
            hi: 10,
            inclusive: false
        }
    );
    assert_ne!(
        r,
        TestValue::IntRange {
            lo: 1,
            hi: 11,
            inclusive: false
        }
    );
}

// ── DecisionTree ────────────────────────────────────────────

#[test]
fn decision_tree_leaf() {
    let leaf = DecisionTree::Leaf {
        arm_index: 0,
        bindings: vec![],
    };
    assert!(matches!(leaf, DecisionTree::Leaf { arm_index: 0, .. }));
}

#[test]
fn decision_tree_fail() {
    let fail = DecisionTree::Fail;
    assert!(matches!(fail, DecisionTree::Fail));
}

#[test]
fn decision_tree_switch_simple() {
    // match x { true -> 1, false -> 0 }
    let tree = DecisionTree::Switch {
        path: Vec::new(),
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
    if let DecisionTree::Switch { edges, .. } = &tree {
        assert_eq!(edges.len(), 2);
    } else {
        panic!("expected Switch");
    }
}

#[test]
fn decision_tree_switch_with_default() {
    // match n { 1 -> a, 2 -> b, _ -> c }
    let tree = DecisionTree::Switch {
        path: Vec::new(),
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
    if let DecisionTree::Switch { default, .. } = &tree {
        assert!(default.is_some());
    } else {
        panic!("expected Switch");
    }
}

#[test]
fn decision_tree_guard() {
    // match x { Some(v) if v > 0 -> pos, Some(v) -> other, None -> none }
    let guard_tree = DecisionTree::Guard {
        arm_index: 0,
        bindings: vec![(Name::from_raw(1), vec![PathInstruction::TagPayload(0)])],
        guard: CanId::new(100),
        on_fail: Box::new(DecisionTree::Leaf {
            arm_index: 1,
            bindings: vec![(Name::from_raw(1), vec![PathInstruction::TagPayload(0)])],
        }),
    };
    assert!(matches!(
        guard_tree,
        DecisionTree::Guard { arm_index: 0, .. }
    ));
}

#[test]
fn decision_tree_nested_switch() {
    // match (tag, payload) { Some(Some(x)) -> deep, Some(None) -> inner_none, None -> outer_none }
    let inner_switch = DecisionTree::Switch {
        path: vec![PathInstruction::TagPayload(0)],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: Name::from_raw(10),
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: Name::from_raw(11),
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };

    let tree = DecisionTree::Switch {
        path: Vec::new(),
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: Name::from_raw(10),
                },
                inner_switch,
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: Name::from_raw(11),
                },
                DecisionTree::Leaf {
                    arm_index: 2,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };

    if let DecisionTree::Switch { edges, .. } = &tree {
        assert_eq!(edges.len(), 2);
        // First edge leads to another Switch (nested).
        assert!(matches!(&edges[0].1, DecisionTree::Switch { .. }));
    } else {
        panic!("expected Switch");
    }
}

// ── FlatPattern ─────────────────────────────────────────────

#[test]
fn flat_pattern_wildcard_like() {
    assert!(FlatPattern::Wildcard.is_wildcard_like());
    assert!(FlatPattern::Binding(Name::from_raw(1)).is_wildcard_like());
    assert!(!FlatPattern::LitInt(42).is_wildcard_like());
    assert!(!FlatPattern::LitBool(true).is_wildcard_like());
}

#[test]
fn flat_pattern_is_constructor() {
    assert!(FlatPattern::LitInt(42).is_constructor());
    assert!(FlatPattern::LitBool(true).is_constructor());
    assert!(FlatPattern::Tuple(vec![]).is_constructor());
    assert!(FlatPattern::Variant {
        variant_name: Name::from_raw(1),
        variant_index: 0,
        fields: vec![],
    }
    .is_constructor());
    assert!(!FlatPattern::Wildcard.is_constructor());
    assert!(!FlatPattern::Binding(Name::from_raw(1)).is_constructor());
}

#[test]
fn extract_bindings_wildcard() {
    let path: ScrutineePath = Vec::new();
    let bindings = FlatPattern::Wildcard.extract_bindings(&path);
    assert!(bindings.is_empty());
}

#[test]
fn extract_bindings_binding() {
    let path: ScrutineePath = Vec::new();
    let name = Name::from_raw(42);
    let bindings = FlatPattern::Binding(name).extract_bindings(&path);
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, name);
    assert!(bindings[0].1.is_empty());
}

#[test]
fn extract_bindings_variant() {
    // Variant { Some(x) } at root path
    let path: ScrutineePath = Vec::new();
    let name_x = Name::from_raw(1);
    let pat = FlatPattern::Variant {
        variant_name: Name::from_raw(10),
        variant_index: 1,
        fields: vec![FlatPattern::Binding(name_x)],
    };
    let bindings = pat.extract_bindings(&path);
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, name_x);
    // Path should be [TagPayload(0)] — first payload field.
    assert_eq!(bindings[0].1.len(), 1);
    assert_eq!(bindings[0].1[0], PathInstruction::TagPayload(0));
}

#[test]
fn extract_bindings_nested_tuple() {
    // (a, (b, c))
    let name_a = Name::from_raw(1);
    let name_b = Name::from_raw(2);
    let name_c = Name::from_raw(3);
    let pat = FlatPattern::Tuple(vec![
        FlatPattern::Binding(name_a),
        FlatPattern::Tuple(vec![
            FlatPattern::Binding(name_b),
            FlatPattern::Binding(name_c),
        ]),
    ]);
    let path: ScrutineePath = Vec::new();
    let bindings = pat.extract_bindings(&path);
    assert_eq!(bindings.len(), 3);
    // a at [TupleIndex(0)]
    assert_eq!(bindings[0].0, name_a);
    assert_eq!(bindings[0].1.as_slice(), &[PathInstruction::TupleIndex(0)]);
    // b at [TupleIndex(1), TupleIndex(0)]
    assert_eq!(bindings[1].0, name_b);
    assert_eq!(
        bindings[1].1.as_slice(),
        &[
            PathInstruction::TupleIndex(1),
            PathInstruction::TupleIndex(0)
        ]
    );
    // c at [TupleIndex(1), TupleIndex(1)]
    assert_eq!(bindings[2].0, name_c);
    assert_eq!(
        bindings[2].1.as_slice(),
        &[
            PathInstruction::TupleIndex(1),
            PathInstruction::TupleIndex(1)
        ]
    );
}

#[test]
fn extract_bindings_at_pattern() {
    // x @ Some(y)
    let name_x = Name::from_raw(1);
    let name_y = Name::from_raw(2);
    let pat = FlatPattern::At {
        name: name_x,
        inner: Box::new(FlatPattern::Variant {
            variant_name: Name::from_raw(10),
            variant_index: 1,
            fields: vec![FlatPattern::Binding(name_y)],
        }),
    };
    let path: ScrutineePath = Vec::new();
    let bindings = pat.extract_bindings(&path);
    assert_eq!(bindings.len(), 2);
    // x binds at root path (the whole scrutinee).
    assert_eq!(bindings[0].0, name_x);
    assert!(bindings[0].1.is_empty());
    // y binds at [TagPayload(0)].
    assert_eq!(bindings[1].0, name_y);
    assert_eq!(bindings[1].1.as_slice(), &[PathInstruction::TagPayload(0)]);
}

#[test]
fn extract_bindings_or_uses_first_alternative() {
    // A(x) | B(y) — both bind at the same path
    let name_x = Name::from_raw(1);
    let name_y = Name::from_raw(2);
    let pat = FlatPattern::Or(vec![
        FlatPattern::Variant {
            variant_name: Name::from_raw(10),
            variant_index: 0,
            fields: vec![FlatPattern::Binding(name_x)],
        },
        FlatPattern::Variant {
            variant_name: Name::from_raw(11),
            variant_index: 1,
            fields: vec![FlatPattern::Binding(name_y)],
        },
    ]);
    let path: ScrutineePath = Vec::new();
    let bindings = pat.extract_bindings(&path);
    // Uses first alternative's bindings.
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].0, name_x);
}

#[test]
fn extract_bindings_struct() {
    // { x, y: (a, _) }
    let name_x = Name::from_raw(1);
    let name_y = Name::from_raw(2);
    let name_a = Name::from_raw(3);
    let pat = FlatPattern::Struct {
        fields: vec![
            (Name::from_raw(10), FlatPattern::Binding(name_x)),
            (
                name_y,
                FlatPattern::Tuple(vec![FlatPattern::Binding(name_a), FlatPattern::Wildcard]),
            ),
        ],
    };
    let path: ScrutineePath = Vec::new();
    let bindings = pat.extract_bindings(&path);
    assert_eq!(bindings.len(), 2);
    // x at [StructField(0)]
    assert_eq!(bindings[0].0, name_x);
    assert_eq!(bindings[0].1.as_slice(), &[PathInstruction::StructField(0)]);
    // a at [StructField(1), TupleIndex(0)]
    assert_eq!(bindings[1].0, name_a);
    assert_eq!(
        bindings[1].1.as_slice(),
        &[
            PathInstruction::StructField(1),
            PathInstruction::TupleIndex(0)
        ]
    );
}

#[test]
fn extract_bindings_list_with_rest() {
    // [head, ..rest]
    let name_head = Name::from_raw(1);
    let name_rest = Name::from_raw(2);
    let pat = FlatPattern::List {
        elements: vec![FlatPattern::Binding(name_head)],
        rest: Some(name_rest),
    };
    let path: ScrutineePath = Vec::new();
    let bindings = pat.extract_bindings(&path);
    assert_eq!(bindings.len(), 2);
    // head at [ListElement(0)]
    assert_eq!(bindings[0].0, name_head);
    assert_eq!(bindings[0].1.as_slice(), &[PathInstruction::ListElement(0)]);
    // rest at [ListRest(1)] — slice from index 1 onwards.
    assert_eq!(bindings[1].0, name_rest);
    assert_eq!(bindings[1].1.as_slice(), &[PathInstruction::ListRest(1)]);
}

// ── PatternRow ──────────────────────────────────────────────

#[test]
fn pattern_row_construction() {
    let row = PatternRow {
        patterns: vec![FlatPattern::Wildcard, FlatPattern::LitInt(42)],
        arm_index: 0,
        guard: None,
        bindings: vec![],
    };
    assert_eq!(row.patterns.len(), 2);
    assert_eq!(row.arm_index, 0);
    assert!(row.guard.is_none());
}

#[test]
fn pattern_row_with_guard() {
    let row = PatternRow {
        patterns: vec![FlatPattern::Binding(Name::from_raw(1))],
        arm_index: 1,
        guard: Some(CanId::new(50)),
        bindings: vec![],
    };
    assert!(row.guard.is_some());
}
