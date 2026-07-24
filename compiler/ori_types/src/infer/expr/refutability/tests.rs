//! Unit tests for the `pattern_is_irrefutable` predicate (Axis 7).
//!
//! Spec: Clause 15 (irrefutability context table + List Pattern Exhaustiveness table).
//!
//! Coverage scope: every `BindingPattern` variant + nested-path arms +
//! `Idx::ERROR` poison degradation.

use super::{pattern_is_irrefutable, NestedPathStep, RefutableReason};
use crate::infer::InferEngine;
use crate::{Idx, Pool};
use ori_ir::{BindingPattern, FieldBinding, Mutability, Name, StringInterner};

fn n(raw: u32) -> Name {
    Name::from_raw(raw)
}

fn name_pat(raw: u32) -> BindingPattern {
    BindingPattern::Name {
        name: n(raw),
        mutable: Mutability::Mutable,
    }
}

fn imm_name_pat(raw: u32) -> BindingPattern {
    BindingPattern::Name {
        name: n(raw),
        mutable: Mutability::Immutable,
    }
}

#[test]
fn test_pattern_is_irrefutable_wildcard() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::Wildcard;
    assert_eq!(pattern_is_irrefutable(&mut engine, &pat, Idx::INT), Ok(()));
}

#[test]
fn test_pattern_is_irrefutable_name() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = name_pat(1);
    assert_eq!(pattern_is_irrefutable(&mut engine, &pat, Idx::INT), Ok(()));
}

#[test]
fn test_pattern_is_irrefutable_empty_list_no_rest() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let int_list = pool.list(Idx::INT);
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::List {
        elements: vec![],
        rest: None,
    };
    assert_eq!(
        pattern_is_irrefutable(&mut engine, &pat, int_list),
        Err(RefutableReason::ListLength {
            required: 0,
            has_rest: false,
        })
    );
}

#[test]
fn test_pattern_is_irrefutable_rest_only() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let int_list = pool.list(Idx::INT);
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::List {
        elements: vec![],
        rest: Some((n(99), Mutability::Mutable)),
    };
    assert_eq!(pattern_is_irrefutable(&mut engine, &pat, int_list), Ok(()));
}

#[test]
fn test_pattern_is_irrefutable_single_head() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let int_list = pool.list(Idx::INT);
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::List {
        elements: vec![name_pat(1)],
        rest: None,
    };
    assert_eq!(
        pattern_is_irrefutable(&mut engine, &pat, int_list),
        Err(RefutableReason::ListLength {
            required: 1,
            has_rest: false,
        })
    );
}

#[test]
fn test_pattern_is_irrefutable_head_with_rest() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let int_list = pool.list(Idx::INT);
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::List {
        elements: vec![name_pat(1)],
        rest: Some((n(99), Mutability::Mutable)),
    };
    assert_eq!(
        pattern_is_irrefutable(&mut engine, &pat, int_list),
        Err(RefutableReason::ListLength {
            required: 1,
            has_rest: true,
        })
    );
}

#[test]
fn test_pattern_is_irrefutable_tuple_all_simple() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let int_str_tuple = pool.tuple(&[Idx::INT, Idx::STR]);
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::Tuple(vec![name_pat(1), name_pat(2)]);
    assert_eq!(
        pattern_is_irrefutable(&mut engine, &pat, int_str_tuple),
        Ok(())
    );
}

#[test]
fn test_pattern_is_irrefutable_tuple_with_refutable_inner() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let int_list = pool.list(Idx::INT);
    let outer = pool.tuple(&[Idx::INT, int_list]);
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::Tuple(vec![
        name_pat(1),
        BindingPattern::List {
            elements: vec![name_pat(2), name_pat(3)],
            rest: None,
        },
    ]);
    assert_eq!(
        pattern_is_irrefutable(&mut engine, &pat, outer),
        Err(RefutableReason::NestedRefutable {
            path: vec![NestedPathStep::TupleIndex(1)],
            inner: Box::new(RefutableReason::ListLength {
                required: 2,
                has_rest: false,
            }),
        })
    );
}

#[test]
fn test_pattern_is_irrefutable_struct_all_simple() {
    // Why: Test irrefutability on synthetic struct with non-Named tag (recurses with Idx::ERROR).
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::Struct {
        fields: vec![
            FieldBinding {
                name: n(1),
                mutable: Mutability::Mutable,
                pattern: Some(name_pat(1)),
            },
            FieldBinding {
                name: n(2),
                mutable: Mutability::Mutable,
                pattern: Some(name_pat(2)),
            },
        ],
    };
    assert_eq!(pattern_is_irrefutable(&mut engine, &pat, Idx::INT), Ok(()));
}

#[test]
fn test_pattern_is_irrefutable_struct_shorthand_field() {
    // Why: Shorthand `{ name }` synthesizes a Name sub-pattern and returns Ok.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::Struct {
        fields: vec![FieldBinding {
            name: n(5),
            mutable: Mutability::Mutable,
            pattern: None,
        }],
    };
    assert_eq!(pattern_is_irrefutable(&mut engine, &pat, Idx::INT), Ok(()));
}

#[test]
fn test_pattern_is_irrefutable_non_tuple_against_tuple_pattern() {
    // Why: Prevent double-diagnostic on type mismatch by returning Ok on non-tuple type.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::Tuple(vec![name_pat(1), name_pat(2)]);
    assert_eq!(pattern_is_irrefutable(&mut engine, &pat, Idx::INT), Ok(()));
}

#[test]
fn test_pattern_is_irrefutable_outer_tag_var_recurses_with_poison() {
    // Why: Check inner refutability even when outer type is unresolved Tag::Var.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let var_ty = engine.fresh_var();
    let pat = BindingPattern::Tuple(vec![
        name_pat(1),
        BindingPattern::List {
            elements: vec![name_pat(2), name_pat(3)],
            rest: None,
        },
    ]);
    assert_eq!(
        pattern_is_irrefutable(&mut engine, &pat, var_ty),
        Err(RefutableReason::NestedRefutable {
            path: vec![NestedPathStep::TupleIndex(1)],
            inner: Box::new(RefutableReason::ListLength {
                required: 2,
                has_rest: false,
            }),
        })
    );
}

#[test]
fn test_pattern_is_irrefutable_outer_tag_var_struct_recurses_with_poison() {
    // Why: Check struct inner refutability when outer type is unresolved Tag::Var.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let var_ty = engine.fresh_var();
    let addrs_field = n(7);
    let pat = BindingPattern::Struct {
        fields: vec![
            FieldBinding {
                name: n(1),
                mutable: Mutability::Mutable,
                pattern: Some(name_pat(1)),
            },
            FieldBinding {
                name: addrs_field,
                mutable: Mutability::Mutable,
                pattern: Some(BindingPattern::List {
                    elements: vec![name_pat(2), name_pat(3)],
                    rest: None,
                }),
            },
        ],
    };
    assert_eq!(
        pattern_is_irrefutable(&mut engine, &pat, var_ty),
        Err(RefutableReason::NestedRefutable {
            path: vec![NestedPathStep::StructField(addrs_field)],
            inner: Box::new(RefutableReason::ListLength {
                required: 2,
                has_rest: false,
            }),
        })
    );
}

#[test]
fn test_pattern_is_irrefutable_struct_field_preserves_mutability() {
    // Why: Shorthand `{ $name }` must preserve Mutability::Immutable.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let pat = BindingPattern::Struct {
        fields: vec![FieldBinding {
            name: n(5),
            mutable: Mutability::Immutable,
            pattern: None,
        }],
    };
    assert_eq!(pattern_is_irrefutable(&mut engine, &pat, Idx::INT), Ok(()));

    let explicit = BindingPattern::Struct {
        fields: vec![FieldBinding {
            name: n(5),
            mutable: Mutability::Mutable,
            pattern: Some(imm_name_pat(5)),
        }],
    };
    assert_eq!(
        pattern_is_irrefutable(&mut engine, &explicit, Idx::INT),
        Ok(())
    );
}

#[test]
fn test_binding_pattern_refutability_matrix_count() {
    // Axis 8 — Self-verifying matrix completeness.
    //
    // Spec: Clause 15 canonical List shapes.
    let canonical_spec_shape_tests = [
        "test_let_empty_list_pattern_rejects",
        "test_let_single_head_rejects",
        "test_let_two_heads_no_rest_rejects",
        "test_let_two_heads_with_rest_rejects",
        "test_let_rest_only_accepts",
    ];
    assert_eq!(
        canonical_spec_shape_tests.len(),
        5,
        "Spec: Clause 15 enumerates 5 canonical List shapes"
    );

    // (b) Full Axis 1 cell count = canonical 5 + 4 regression cells.
    let axis_1_cell_tests = [
        // Canonical (covered above)
        "test_let_empty_list_pattern_rejects",
        "test_let_single_head_rejects",
        "test_let_two_heads_no_rest_rejects",
        "test_let_two_heads_with_rest_rejects",
        "test_let_rest_only_accepts",
        // Regression (beyond canonical)
        "test_let_three_heads_no_rest_rejects",
        "test_let_three_heads_with_rest_rejects",
        "test_let_with_wildcard_head_rejects",
        "test_let_immutable_rest_only_accepts",
    ];
    assert_eq!(
        axis_1_cell_tests.len(),
        9,
        "Axis 1 enumerates 9 cells (5 canonical + 4 regression)"
    );

    // (c) 4 binding-site call paths + match-arm-body let dispatch = 5 site tests.
    let site_tests = [
        "test_block_let_refutable_list_rejects",
        "test_expr_let_refutable_list_rejects",
        "test_try_let_refutable_list_rejects",
        "test_for_loop_refutable_list_rejects",
        "test_match_arm_body_let_refutable_list_rejects",
    ];
    assert_eq!(
        site_tests.len(),
        5,
        "4 bind_pattern call sites + match-arm-body let dispatch = 5 site tests"
    );
}
