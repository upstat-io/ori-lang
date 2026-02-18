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
            bindings: vec![],
        })
        .collect()
}

fn paths(n: usize) -> Vec<ScrutineePath> {
    vec![Vec::new(); n]
}

// Empty and trivial

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

// Bool matching

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

// Int matching with default

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

// Enum variant matching

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

// Wildcard mixed with constructors

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

// Multi-column matching

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

// Guard handling

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
            bindings: vec![],
        },
        PatternRow {
            patterns: vec![FlatPattern::Wildcard],
            arm_index: 1,
            guard: None,
            bindings: vec![],
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

// Tuple decomposition

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

// Or-pattern

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

// pick_column heuristic

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

// String matching

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

// Or-pattern with variant bindings

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

// Guards with overlapping patterns

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "multi-step decision tree test with guard combinations"
)]
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
            bindings: vec![],
        },
        PatternRow {
            patterns: vec![FlatPattern::Variant {
                variant_name: some_name,
                variant_index: 1,
                fields: vec![FlatPattern::Binding(name_x)],
            }],
            arm_index: 1,
            guard: Some(guard2),
            bindings: vec![],
        },
        PatternRow {
            patterns: vec![FlatPattern::Variant {
                variant_name: some_name,
                variant_index: 1,
                fields: vec![FlatPattern::Binding(name_x)],
            }],
            arm_index: 2,
            guard: None,
            bindings: vec![],
        },
        PatternRow {
            patterns: vec![FlatPattern::Variant {
                variant_name: none_name,
                variant_index: 0,
                fields: vec![],
            }],
            arm_index: 3,
            guard: None,
            bindings: vec![],
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

// Struct decomposition

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

// Nested enum inside tuple

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
