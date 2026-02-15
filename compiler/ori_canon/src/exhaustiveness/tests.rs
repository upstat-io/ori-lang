use super::*;
use ori_ir::canon::tree::DecisionTree;
use ori_ir::{Name, SharedInterner};

/// Helper: dummy span for tests.
fn span() -> Span {
    Span::new(0, 10)
}

/// Helper: build arm spans for N arms.
fn arm_spans(n: usize) -> Vec<Span> {
    (0..n)
        .map(|i| {
            let start = u32::try_from(i).unwrap() * 10;
            let end = start + 10;
            Span::new(start, end)
        })
        .collect()
}

/// Helper: default pool and interner for tests that don't need enum types.
fn default_ctx() -> (ori_types::Pool, SharedInterner) {
    (ori_types::Pool::new(), SharedInterner::new())
}

/// Helper: shorthand check with default pool (no enum info needed).
fn check(tree: &DecisionTree, arm_count: usize) -> CheckResult {
    let (pool, interner) = default_ctx();
    check_exhaustiveness(
        tree,
        arm_count,
        span(),
        &arm_spans(arm_count),
        ori_types::Idx::UNIT,
        &pool,
        &interner,
    )
}

// ── Exhaustive cases — should produce NO problems ─────────────

#[test]
fn bool_exhaustive_both() {
    // match b { true -> 0, false -> 1 }
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
    let result = check(&tree, 2);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn bool_with_default() {
    // match b { true -> 0, _ -> 1 }
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::BoolEq,
        edges: vec![(
            TestValue::Bool(true),
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: Some(Box::new(DecisionTree::Leaf {
            arm_index: 1,
            bindings: vec![],
        })),
    };
    let result = check(&tree, 2);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn wildcard_match() {
    // A single wildcard arm covers everything.
    let tree = DecisionTree::Leaf {
        arm_index: 0,
        bindings: vec![],
    };
    let result = check(&tree, 1);
    assert!(result.problems.is_empty());
}

#[test]
fn int_with_default() {
    // match n { 1 -> "one", _ -> "other" }
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::IntEq,
        edges: vec![(
            TestValue::Int(1),
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: Some(Box::new(DecisionTree::Leaf {
            arm_index: 1,
            bindings: vec![],
        })),
    };
    let result = check(&tree, 2);
    assert!(result.problems.is_empty());
}

// ── Non-exhaustive cases ──────────────────────────────────────

#[test]
fn bool_missing_false() {
    // match b { true -> 0 }
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::BoolEq,
        edges: vec![(
            TestValue::Bool(true),
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check(&tree, 1);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["false"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn bool_missing_true() {
    // match b { false -> 0 }
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::BoolEq,
        edges: vec![(
            TestValue::Bool(false),
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check(&tree, 1);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["true"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn int_no_default() {
    // match n { 1 -> "one" } — no wildcard, infinite type
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::IntEq,
        edges: vec![(
            TestValue::Int(1),
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check(&tree, 1);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert!(missing.contains(&"_".to_string()));
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn fail_node() {
    // Bare Fail = nothing matches.
    let tree = DecisionTree::Fail;
    let result = check(&tree, 0);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["_"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn guard_fallthrough_fail() {
    // match x { n if n > 0 -> n } — guard may fail, on_fail = Fail
    let tree = DecisionTree::Guard {
        arm_index: 0,
        bindings: vec![],
        guard: ori_ir::canon::CanId::INVALID, // doesn't matter for checking
        on_fail: Box::new(DecisionTree::Fail),
    };
    let result = check(&tree, 1);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["_"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

// ── Redundancy cases ──────────────────────────────────────────

#[test]
fn redundant_arm() {
    // match b { true -> 0, false -> 1, _ -> 2 }
    // The wildcard arm (index 2) is redundant because bool is fully covered.
    // Tree: Switch(BoolEq, [true→Leaf(0), false→Leaf(1)], None)
    // arm_count=3, but arm 2 is never referenced.
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
    let result = check(&tree, 3);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::RedundantArm { arm_index, .. } => {
            assert_eq!(*arm_index, 2);
        }
        other @ PatternProblem::NonExhaustive { .. } => {
            panic!("expected RedundantArm, got: {other:?}")
        }
    }
}

#[test]
fn nested_switch_detects_inner_non_exhaustive() {
    // Outer switch on enum tag with a default, inner switch on BoolEq
    // missing false.
    //
    // This simulates: match (tag, b) where the bool branch is incomplete.
    let inner = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::BoolEq,
        edges: vec![(
            TestValue::Bool(true),
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None, // missing false!
    };
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 0,
                variant_name: Name::EMPTY,
            },
            inner,
        )],
        default: Some(Box::new(DecisionTree::Leaf {
            arm_index: 1,
            bindings: vec![],
        })),
    };
    let result = check(&tree, 2);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert!(missing.contains(&"false".to_string()));
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn str_no_default() {
    // match s { "hello" -> 1 } — infinite type, no wildcard
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::StrEq,
        edges: vec![(
            TestValue::Str(Name::EMPTY),
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check(&tree, 1);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert!(missing.contains(&"_".to_string()));
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn guard_with_fallback_exhaustive() {
    // match x { n if n > 0 -> "pos", _ -> "other" }
    // Guard on arm 0 falls through to Leaf(1) on failure — exhaustive.
    let tree = DecisionTree::Guard {
        arm_index: 0,
        bindings: vec![],
        guard: ori_ir::canon::CanId::INVALID,
        on_fail: Box::new(DecisionTree::Leaf {
            arm_index: 1,
            bindings: vec![],
        }),
    };
    let result = check(&tree, 2);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn guard_chain_all_fail_non_exhaustive() {
    // match x { n if n > 0 -> "pos", n if n < 0 -> "neg" }
    // Both arms are guarded. If both guards fail, we reach Fail.
    // Exhaustiveness: non-exhaustive (both guards can fail simultaneously).
    let tree = DecisionTree::Guard {
        arm_index: 0,
        bindings: vec![],
        guard: ori_ir::canon::CanId::INVALID,
        on_fail: Box::new(DecisionTree::Guard {
            arm_index: 1,
            bindings: vec![],
            guard: ori_ir::canon::CanId::INVALID,
            on_fail: Box::new(DecisionTree::Fail),
        }),
    };
    let result = check(&tree, 2);
    // Both arms are reachable (guards may pass), but the match is non-exhaustive.
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["_"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn guard_chain_ends_with_leaf_exhaustive() {
    // match x { n if n > 0 -> "pos", n if n < 0 -> "neg", _ -> "zero" }
    // Last arm is unguarded — exhaustive.
    let tree = DecisionTree::Guard {
        arm_index: 0,
        bindings: vec![],
        guard: ori_ir::canon::CanId::INVALID,
        on_fail: Box::new(DecisionTree::Guard {
            arm_index: 1,
            bindings: vec![],
            guard: ori_ir::canon::CanId::INVALID,
            on_fail: Box::new(DecisionTree::Leaf {
                arm_index: 2,
                bindings: vec![],
            }),
        }),
    };
    let result = check(&tree, 3);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn guard_on_enum_does_not_count_as_covering() {
    // match opt { Some(v) if v > 0 -> "pos" }
    // The guard on arm 0 doesn't cover Some — if the guard fails, Fail.
    // Also missing None entirely. Both problems should be detected.
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::BoolEq,
        edges: vec![(
            TestValue::Bool(true),
            DecisionTree::Guard {
                arm_index: 0,
                bindings: vec![],
                guard: ori_ir::canon::CanId::INVALID,
                on_fail: Box::new(DecisionTree::Fail),
            },
        )],
        default: None,
    };
    let result = check(&tree, 1);
    // Missing false (no edge), and guard on true can fail (Fail node).
    let non_exhaustive: Vec<_> = result
        .problems
        .iter()
        .filter(|p| matches!(p, PatternProblem::NonExhaustive { .. }))
        .collect();
    assert!(
        !non_exhaustive.is_empty(),
        "expected NonExhaustive, got: {:?}",
        result.problems
    );
}

#[test]
fn multiple_missing_bool_and_redundant() {
    // Empty switch with no edges and no default — both true and false missing.
    // Also arm 0 is never reached.
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::BoolEq,
        edges: vec![],
        default: None,
    };
    let result = check(&tree, 1);
    // Should have NonExhaustive (missing true and false) + RedundantArm (arm 0).
    let non_exhaustive = result
        .problems
        .iter()
        .find(|p| matches!(p, PatternProblem::NonExhaustive { .. }));
    let redundant = result
        .problems
        .iter()
        .find(|p| matches!(p, PatternProblem::RedundantArm { .. }));
    assert!(non_exhaustive.is_some(), "expected NonExhaustive problem");
    assert!(redundant.is_some(), "expected RedundantArm problem");
    if let PatternProblem::NonExhaustive { missing, .. } = non_exhaustive.unwrap() {
        assert!(missing.contains(&"true".to_string()));
        assert!(missing.contains(&"false".to_string()));
    }
}

// ── Phase 2: Enum exhaustiveness ──────────────────────────────

#[test]
fn option_exhaustive_both_variants() {
    // match opt { Some(x) -> ..., None -> ... }
    let interner = SharedInterner::new();
    let name_none = interner.intern("None");
    let name_some = interner.intern("Some");

    let mut pool = ori_types::Pool::new();
    let opt_ty = pool.option(ori_types::Idx::INT);

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_some,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 2, span(), &arm_spans(2), opt_ty, &pool, &interner);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn option_missing_none() {
    // match opt { Some(x) -> ... } — missing None
    let interner = SharedInterner::new();
    let name_some = interner.intern("Some");

    let mut pool = ori_types::Pool::new();
    let opt_ty = pool.option(ori_types::Idx::INT);

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 1,
                variant_name: name_some,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 1, span(), &arm_spans(1), opt_ty, &pool, &interner);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["None"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn option_missing_some() {
    // match opt { None -> ... } — missing Some(_)
    let interner = SharedInterner::new();
    let name_none = interner.intern("None");

    let mut pool = ori_types::Pool::new();
    let opt_ty = pool.option(ori_types::Idx::INT);

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 0,
                variant_name: name_none,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 1, span(), &arm_spans(1), opt_ty, &pool, &interner);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["Some(_)"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn result_exhaustive() {
    // match res { Ok(v) -> ..., Err(e) -> ... }
    let interner = SharedInterner::new();
    let name_ok = interner.intern("Ok");
    let name_err = interner.intern("Err");

    let mut pool = ori_types::Pool::new();
    let res_ty = pool.result(ori_types::Idx::INT, ori_types::Idx::STR);

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_ok,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_err,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 2, span(), &arm_spans(2), res_ty, &pool, &interner);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn result_missing_err() {
    // match res { Ok(v) -> ... } — missing Err(_)
    let interner = SharedInterner::new();
    let name_ok = interner.intern("Ok");

    let mut pool = ori_types::Pool::new();
    let res_ty = pool.result(ori_types::Idx::INT, ori_types::Idx::STR);

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 0,
                variant_name: name_ok,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 1, span(), &arm_spans(1), res_ty, &pool, &interner);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["Err(_)"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn user_enum_exhaustive() {
    // enum Color { Red, Green, Blue }
    // match c { Red -> ..., Green -> ..., Blue -> ... }
    let interner = SharedInterner::new();
    let name_red = interner.intern("Red");
    let name_green = interner.intern("Green");
    let name_blue = interner.intern("Blue");
    let name_color = interner.intern("Color");

    let mut pool = ori_types::Pool::new();
    let enum_ty = pool.enum_type(
        name_color,
        &[
            ori_types::EnumVariant {
                name: name_red,
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: name_green,
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: name_blue,
                field_types: vec![],
            },
        ],
    );

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_red,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_green,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
            (
                TestValue::Tag {
                    variant_index: 2,
                    variant_name: name_blue,
                },
                DecisionTree::Leaf {
                    arm_index: 2,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 3, span(), &arm_spans(3), enum_ty, &pool, &interner);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn user_enum_missing_one() {
    // enum Color { Red, Green, Blue }
    // match c { Red -> ..., Green -> ... } — missing Blue
    let interner = SharedInterner::new();
    let name_red = interner.intern("Red");
    let name_green = interner.intern("Green");
    let name_blue = interner.intern("Blue");
    let name_color = interner.intern("Color");

    let mut pool = ori_types::Pool::new();
    let enum_ty = pool.enum_type(
        name_color,
        &[
            ori_types::EnumVariant {
                name: name_red,
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: name_green,
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: name_blue,
                field_types: vec![],
            },
        ],
    );

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_red,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_green,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 2, span(), &arm_spans(2), enum_ty, &pool, &interner);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["Blue"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn user_enum_missing_multiple() {
    // enum Color { Red, Green, Blue }
    // match c { Red -> ... } — missing Green and Blue
    let interner = SharedInterner::new();
    let name_red = interner.intern("Red");
    let name_green = interner.intern("Green");
    let name_blue = interner.intern("Blue");
    let name_color = interner.intern("Color");

    let mut pool = ori_types::Pool::new();
    let enum_ty = pool.enum_type(
        name_color,
        &[
            ori_types::EnumVariant {
                name: name_red,
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: name_green,
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: name_blue,
                field_types: vec![],
            },
        ],
    );

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 0,
                variant_name: name_red,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 1, span(), &arm_spans(1), enum_ty, &pool, &interner);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert!(missing.contains(&"Blue".to_string()));
            assert!(missing.contains(&"Green".to_string()));
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn user_enum_variant_with_fields() {
    // enum Shape { Circle(float), Rect(float, float) }
    // match s { Circle(r) -> ... } — missing Rect(_, _)
    let interner = SharedInterner::new();
    let name_circle = interner.intern("Circle");
    let name_rect = interner.intern("Rect");
    let name_shape = interner.intern("Shape");

    let mut pool = ori_types::Pool::new();
    let enum_ty = pool.enum_type(
        name_shape,
        &[
            ori_types::EnumVariant {
                name: name_circle,
                field_types: vec![ori_types::Idx::FLOAT],
            },
            ori_types::EnumVariant {
                name: name_rect,
                field_types: vec![ori_types::Idx::FLOAT, ori_types::Idx::FLOAT],
            },
        ],
    );

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 0,
                variant_name: name_circle,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 1, span(), &arm_spans(1), enum_ty, &pool, &interner);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["Rect(_, _)"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn enum_with_default_is_exhaustive() {
    // match opt { Some(x) -> ..., _ -> ... }
    // Default covers None implicitly.
    let interner = SharedInterner::new();
    let name_some = interner.intern("Some");

    let mut pool = ori_types::Pool::new();
    let opt_ty = pool.option(ori_types::Idx::INT);

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 1,
                variant_name: name_some,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: Some(Box::new(DecisionTree::Leaf {
            arm_index: 1,
            bindings: vec![],
        })),
    };
    let result = check_exhaustiveness(&tree, 2, span(), &arm_spans(2), opt_ty, &pool, &interner);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn nested_non_enum_type_skipped() {
    // Option<int>: Some's payload is int (not an enum).
    // A nested EnumTag switch on the int payload has no type info —
    // the checker skips it gracefully (no false positive).
    let interner = SharedInterner::new();
    let name_none = interner.intern("None");

    let mut pool = ori_types::Pool::new();
    let opt_ty = pool.option(ori_types::Idx::INT);

    // Inner switch at non-empty path with int payload type — skipped.
    let inner = DecisionTree::Switch {
        path: vec![PathInstruction::TagPayload(0)],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 0,
                variant_name: name_none,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: interner.intern("Some"),
                },
                inner,
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 2, span(), &arm_spans(2), opt_ty, &pool, &interner);
    // Root-level Option is exhaustive. Inner switch on int type is skipped.
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

// ── Phase 3: Nested enum exhaustiveness ─────────────────────

#[test]
fn nested_option_exhaustive() {
    // Option<Option<int>>
    // match opt { Some(Some(x)) -> ..., Some(None) -> ..., None -> ... }
    let interner = SharedInterner::new();
    let name_none = interner.intern("None");
    let name_some = interner.intern("Some");

    let mut pool = ori_types::Pool::new();
    let inner_opt = pool.option(ori_types::Idx::INT);
    let outer_opt = pool.option(inner_opt);

    let inner = DecisionTree::Switch {
        path: vec![PathInstruction::TagPayload(0)],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_some,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
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
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_some,
                },
                inner,
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
                },
                DecisionTree::Leaf {
                    arm_index: 2,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 3, span(), &arm_spans(3), outer_opt, &pool, &interner);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn nested_option_missing_inner_some() {
    // Option<Option<int>>
    // match opt { Some(None) -> ..., None -> ... }
    // Missing: Some(Some(_))
    let interner = SharedInterner::new();
    let name_none = interner.intern("None");
    let name_some = interner.intern("Some");

    let mut pool = ori_types::Pool::new();
    let inner_opt = pool.option(ori_types::Idx::INT);
    let outer_opt = pool.option(inner_opt);

    let inner = DecisionTree::Switch {
        path: vec![PathInstruction::TagPayload(0)],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 0,
                variant_name: name_none,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None, // Missing Some!
    };
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_some,
                },
                inner,
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 2, span(), &arm_spans(2), outer_opt, &pool, &interner);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["Some(Some(_))"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn nested_option_missing_inner_none() {
    // Option<Option<int>>
    // match opt { Some(Some(x)) -> ..., None -> ... }
    // Missing: Some(None)
    let interner = SharedInterner::new();
    let name_none = interner.intern("None");
    let name_some = interner.intern("Some");

    let mut pool = ori_types::Pool::new();
    let inner_opt = pool.option(ori_types::Idx::INT);
    let outer_opt = pool.option(inner_opt);

    let inner = DecisionTree::Switch {
        path: vec![PathInstruction::TagPayload(0)],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 1,
                variant_name: name_some,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None, // Missing None!
    };
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_some,
                },
                inner,
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 2, span(), &arm_spans(2), outer_opt, &pool, &interner);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["Some(None)"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn nested_result_option_missing() {
    // Result<Option<int>, str>
    // match res { Ok(Some(x)) -> ..., Err(e) -> ... }
    // Missing: Ok(None)
    let interner = SharedInterner::new();
    let name_some = interner.intern("Some");
    let name_ok = interner.intern("Ok");
    let name_err = interner.intern("Err");

    let mut pool = ori_types::Pool::new();
    let opt_ty = pool.option(ori_types::Idx::INT);
    let res_ty = pool.result(opt_ty, ori_types::Idx::STR);

    // Inside Ok: only matches Some, missing None
    let inner = DecisionTree::Switch {
        path: vec![PathInstruction::TagPayload(0)],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 1,
                variant_name: name_some,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None, // Missing None!
    };
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_ok,
                },
                inner,
            ),
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_err,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 2, span(), &arm_spans(2), res_ty, &pool, &interner);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["Ok(None)"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn deeply_nested_option_missing() {
    // Option<Option<Option<int>>>
    // match opt { Some(Some(Some(x))) -> ..., Some(Some(None)) -> ...,
    //             Some(None) -> ..., None -> ... }
    // This is exhaustive.
    let interner = SharedInterner::new();
    let name_none = interner.intern("None");
    let name_some = interner.intern("Some");

    let mut pool = ori_types::Pool::new();
    let opt1 = pool.option(ori_types::Idx::INT);
    let opt2 = pool.option(opt1);
    let opt3 = pool.option(opt2);

    let innermost = DecisionTree::Switch {
        path: vec![
            PathInstruction::TagPayload(0),
            PathInstruction::TagPayload(0),
        ],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_some,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let middle = DecisionTree::Switch {
        path: vec![PathInstruction::TagPayload(0)],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_some,
                },
                innermost,
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
                },
                DecisionTree::Leaf {
                    arm_index: 2,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_some,
                },
                middle,
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
                },
                DecisionTree::Leaf {
                    arm_index: 3,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 4, span(), &arm_spans(4), opt3, &pool, &interner);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn deeply_nested_option_missing_innermost() {
    // Option<Option<Option<int>>>
    // match opt { Some(Some(Some(x))) -> ..., Some(None) -> ..., None -> ... }
    // Missing: Some(Some(None))
    let interner = SharedInterner::new();
    let name_none = interner.intern("None");
    let name_some = interner.intern("Some");

    let mut pool = ori_types::Pool::new();
    let opt1 = pool.option(ori_types::Idx::INT);
    let opt2 = pool.option(opt1);
    let opt3 = pool.option(opt2);

    // Innermost: only Some, missing None
    let innermost = DecisionTree::Switch {
        path: vec![
            PathInstruction::TagPayload(0),
            PathInstruction::TagPayload(0),
        ],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 1,
                variant_name: name_some,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let middle = DecisionTree::Switch {
        path: vec![PathInstruction::TagPayload(0)],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_some,
                },
                innermost,
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
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
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_some,
                },
                middle,
            ),
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_none,
                },
                DecisionTree::Leaf {
                    arm_index: 2,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 3, span(), &arm_spans(3), opt3, &pool, &interner);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["Some(Some(None))"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

// ── Never variant exhaustiveness ─────────────────────────────

#[test]
fn user_enum_never_variant_omittable() {
    // type MaybeNever = Value(int) | Impossible(Never)
    // match m { Value(v) -> ... }
    // Impossible has a Never field → uninhabited → not required in match
    let interner = SharedInterner::new();
    let name_value = interner.intern("Value");
    let name_impossible = interner.intern("Impossible");
    let name_type = interner.intern("MaybeNever");

    let mut pool = ori_types::Pool::new();
    let enum_ty = pool.enum_type(
        name_type,
        &[
            ori_types::EnumVariant {
                name: name_value,
                field_types: vec![ori_types::Idx::INT],
            },
            ori_types::EnumVariant {
                name: name_impossible,
                field_types: vec![ori_types::Idx::NEVER],
            },
        ],
    );

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![(
            TestValue::Tag {
                variant_index: 0,
                variant_name: name_value,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 1, span(), &arm_spans(1), enum_ty, &pool, &interner);
    assert!(
        result.problems.is_empty(),
        "Never variant should be omittable, got: {:?}",
        result.problems
    );
}

#[test]
fn user_enum_never_variant_still_matchable() {
    // type MaybeNever = Value(int) | Impossible(Never)
    // match m { Value(v) -> ..., Impossible(_) -> ... }
    // Matching the Never variant is allowed (arm is redundant but accepted)
    let interner = SharedInterner::new();
    let name_value = interner.intern("Value");
    let name_impossible = interner.intern("Impossible");
    let name_type = interner.intern("MaybeNever");

    let mut pool = ori_types::Pool::new();
    let enum_ty = pool.enum_type(
        name_type,
        &[
            ori_types::EnumVariant {
                name: name_value,
                field_types: vec![ori_types::Idx::INT],
            },
            ori_types::EnumVariant {
                name: name_impossible,
                field_types: vec![ori_types::Idx::NEVER],
            },
        ],
    );

    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![
            (
                TestValue::Tag {
                    variant_index: 0,
                    variant_name: name_value,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::Tag {
                    variant_index: 1,
                    variant_name: name_impossible,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 2, span(), &arm_spans(2), enum_ty, &pool, &interner);
    // No non-exhaustive error — both variants explicitly covered
    let non_exhaustive: Vec<_> = result
        .problems
        .iter()
        .filter(|p| matches!(p, PatternProblem::NonExhaustive { .. }))
        .collect();
    assert!(
        non_exhaustive.is_empty(),
        "expected no non-exhaustive problem, got: {non_exhaustive:?}",
    );
}

#[test]
fn user_enum_all_never_variants_exhaustive() {
    // type AllNever = A(Never) | B(Never)
    // match m { } — empty match should be exhaustive (all variants uninhabited)
    // However the tree would be a Fail in practice. For this test,
    // we verify that check_user_enum skips all Never variants.
    let interner = SharedInterner::new();
    let name_a = interner.intern("A");
    let name_b = interner.intern("B");
    let name_type = interner.intern("AllNever");

    let mut pool = ori_types::Pool::new();
    let enum_ty = pool.enum_type(
        name_type,
        &[
            ori_types::EnumVariant {
                name: name_a,
                field_types: vec![ori_types::Idx::NEVER],
            },
            ori_types::EnumVariant {
                name: name_b,
                field_types: vec![ori_types::Idx::NEVER],
            },
        ],
    );

    // Switch with no edges (nothing matched)
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::EnumTag,
        edges: vec![],
        default: None,
    };
    let result = check_exhaustiveness(&tree, 0, span(), &arm_spans(0), enum_ty, &pool, &interner);
    assert!(
        result.problems.is_empty(),
        "all-Never enum should need no arms, got: {:?}",
        result.problems
    );
}

// ── Phase 3: List pattern exhaustiveness ─────────────────────

#[test]
fn list_rest_zero_covers_all() {
    // match lst { [..rest] -> ... }
    // Rest pattern with min length 0 covers ALL lists — exhaustive.
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::ListLen,
        edges: vec![(
            TestValue::ListLen {
                len: 0,
                is_exact: false,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check(&tree, 1);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn list_empty_plus_rest_exhaustive() {
    // match lst { [] -> ..., [x, ..rest] -> ... }
    // Exact len=0 + rest min=1 covers everything: 0 exactly, >= 1.
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::ListLen,
        edges: vec![
            (
                TestValue::ListLen {
                    len: 0,
                    is_exact: true,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::ListLen {
                    len: 1,
                    is_exact: false,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check(&tree, 2);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn list_multiple_exact_plus_rest_exhaustive() {
    // match lst { [] -> ..., [x] -> ..., [a, b, ..rest] -> ... }
    // Exact 0, exact 1, rest min=2 → covers 0, 1, >= 2 — exhaustive.
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::ListLen,
        edges: vec![
            (
                TestValue::ListLen {
                    len: 0,
                    is_exact: true,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::ListLen {
                    len: 1,
                    is_exact: true,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
            (
                TestValue::ListLen {
                    len: 2,
                    is_exact: false,
                },
                DecisionTree::Leaf {
                    arm_index: 2,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check(&tree, 3);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}

#[test]
fn list_gap_missing_empty() {
    // match lst { [x] -> ..., [a, b, ..rest] -> ... }
    // Exact 1 + rest min=2 — missing len=0 (empty list).
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::ListLen,
        edges: vec![
            (
                TestValue::ListLen {
                    len: 1,
                    is_exact: true,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::ListLen {
                    len: 2,
                    is_exact: false,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check(&tree, 2);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["[]"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn list_gap_missing_single() {
    // match lst { [] -> ..., [a, b, ..rest] -> ... }
    // Exact 0 + rest min=2 — missing len=1.
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::ListLen,
        edges: vec![
            (
                TestValue::ListLen {
                    len: 0,
                    is_exact: true,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::ListLen {
                    len: 2,
                    is_exact: false,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check(&tree, 2);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["[_]"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn list_exact_only_non_exhaustive() {
    // match lst { [] -> ..., [x] -> ..., [a, b] -> ... }
    // All exact — no rest pattern, cannot cover all lengths.
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::ListLen,
        edges: vec![
            (
                TestValue::ListLen {
                    len: 0,
                    is_exact: true,
                },
                DecisionTree::Leaf {
                    arm_index: 0,
                    bindings: vec![],
                },
            ),
            (
                TestValue::ListLen {
                    len: 1,
                    is_exact: true,
                },
                DecisionTree::Leaf {
                    arm_index: 1,
                    bindings: vec![],
                },
            ),
            (
                TestValue::ListLen {
                    len: 2,
                    is_exact: true,
                },
                DecisionTree::Leaf {
                    arm_index: 2,
                    bindings: vec![],
                },
            ),
        ],
        default: None,
    };
    let result = check(&tree, 3);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert_eq!(missing, &["_"]);
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn list_rest_missing_multiple_gaps() {
    // match lst { [a, b, c, ..rest] -> ... }
    // Rest min=3 — missing 0, 1, 2.
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::ListLen,
        edges: vec![(
            TestValue::ListLen {
                len: 3,
                is_exact: false,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: None,
    };
    let result = check(&tree, 1);
    assert_eq!(result.problems.len(), 1);
    match &result.problems[0] {
        PatternProblem::NonExhaustive { missing, .. } => {
            assert!(
                missing.contains(&"[]".to_string()),
                "should contain [] but got: {missing:?}"
            );
            assert!(
                missing.contains(&"[_]".to_string()),
                "should contain [_] but got: {missing:?}"
            );
            assert!(
                missing.contains(&"[_, _]".to_string()),
                "should contain [_, _] but got: {missing:?}"
            );
        }
        other @ PatternProblem::RedundantArm { .. } => {
            panic!("expected NonExhaustive, got: {other:?}")
        }
    }
}

#[test]
fn list_with_default_exhaustive() {
    // match lst { [x] -> ..., _ -> ... }
    // Default branch covers everything else — exhaustive.
    let tree = DecisionTree::Switch {
        path: vec![],
        test_kind: TestKind::ListLen,
        edges: vec![(
            TestValue::ListLen {
                len: 1,
                is_exact: true,
            },
            DecisionTree::Leaf {
                arm_index: 0,
                bindings: vec![],
            },
        )],
        default: Some(Box::new(DecisionTree::Leaf {
            arm_index: 1,
            bindings: vec![],
        })),
    };
    let result = check(&tree, 2);
    assert!(
        result.problems.is_empty(),
        "expected no problems, got: {:?}",
        result.problems
    );
}
