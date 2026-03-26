//! Tests for [`ValueRepr`] classification, [`RcStrategy`] derivation,
//! and [`compute_var_reprs`].

use ori_ir::Name;
use ori_types::{EnumVariant, Idx, Pool};

use crate::classify::ArcClassifier;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    LitValue,
};
use crate::ownership::Ownership;
use crate::ArcClass;

use super::*;

// from_arc_class: Scalar

#[test]
fn scalar_class_yields_scalar_repr() {
    let pool = Pool::new();

    for idx in [
        Idx::INT,
        Idx::FLOAT,
        Idx::BOOL,
        Idx::CHAR,
        Idx::BYTE,
        Idx::UNIT,
    ] {
        assert_eq!(
            ValueRepr::from_arc_class(ArcClass::Scalar, &pool, idx),
            ValueRepr::Scalar,
            "Scalar class for {} should yield Scalar repr",
            idx.display_name(),
        );
    }
}

// from_arc_class: FatValue (str, function)

#[test]
fn str_yields_fat_value() {
    let pool = Pool::new();
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, Idx::STR),
        ValueRepr::FatValue,
    );
}

#[test]
fn function_yields_fat_value() {
    let mut pool = Pool::new();
    let func_ty = pool.function(&[Idx::INT], Idx::BOOL);
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, func_ty),
        ValueRepr::FatValue,
    );
}

// from_arc_class: RcPointer (list, map, set, channel)

#[test]
fn list_yields_rc_pointer() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, list_int),
        ValueRepr::RcPointer,
    );
}

#[test]
fn map_yields_rc_pointer() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::STR, Idx::INT);
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, map_ty),
        ValueRepr::RcPointer,
    );
}

#[test]
fn set_yields_rc_pointer() {
    let mut pool = Pool::new();
    let set_ty = pool.set(Idx::INT);
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, set_ty),
        ValueRepr::RcPointer,
    );
}

#[test]
fn channel_yields_rc_pointer() {
    let mut pool = Pool::new();
    let chan_ty = pool.channel(Idx::INT);
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, chan_ty),
        ValueRepr::RcPointer,
    );
}

// from_arc_class: Aggregate (tuple, struct, enum, result, option)

#[test]
fn tuple_with_ref_yields_aggregate() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::STR]);
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, tup),
        ValueRepr::Aggregate,
    );
}

#[test]
fn struct_with_ref_yields_aggregate() {
    let mut pool = Pool::new();
    let name = Name::from_raw(10);
    let f1 = Name::from_raw(11);
    let f2 = Name::from_raw(12);
    let st = pool.struct_type(name, &[(f1, Idx::STR), (f2, Idx::INT)]);
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, st),
        ValueRepr::Aggregate,
    );
}

#[test]
fn result_yields_aggregate() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::STR);
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, res),
        ValueRepr::Aggregate,
    );
}

#[test]
fn option_of_ref_yields_aggregate() {
    let mut pool = Pool::new();
    let opt = pool.option(Idx::STR);
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, opt),
        ValueRepr::Aggregate,
    );
}

#[test]
fn enum_with_ref_variant_yields_aggregate() {
    let mut pool = Pool::new();
    let name = Name::from_raw(40);
    let enum_ty = pool.enum_type(
        name,
        &[
            EnumVariant {
                name: Name::from_raw(41),
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: Name::from_raw(42),
                field_types: vec![Idx::STR],
            },
        ],
    );
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::DefiniteRef, &pool, enum_ty),
        ValueRepr::Aggregate,
    );
}

// PossibleRef uses same tag-based classification

#[test]
fn possible_ref_list_yields_rc_pointer() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::PossibleRef, &pool, list_int),
        ValueRepr::RcPointer,
    );
}

#[test]
fn possible_ref_str_yields_fat_value() {
    let pool = Pool::new();
    assert_eq!(
        ValueRepr::from_arc_class(ArcClass::PossibleRef, &pool, Idx::STR),
        ValueRepr::FatValue,
    );
}

// compute_var_reprs

#[test]
fn compute_var_reprs_matches_types() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let func_ty = pool.function(&[Idx::INT], Idx::BOOL);
    let tup = pool.tuple(&[Idx::INT, Idx::STR]);

    let func = ArcFunction {
        name: Name::from_raw(1),
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
        ],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: list_int,
                    value: ArcValue::Literal(LitValue::Unit),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: func_ty,
                    value: ArcValue::Literal(LitValue::Unit),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: tup,
                    value: ArcValue::Literal(LitValue::Unit),
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT, Idx::STR, list_int, func_ty, tup],
        var_reprs: Vec::new(),
        spans: vec![vec![None, None, None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: Vec::new(),
    };

    let classifier = ArcClassifier::new(&pool);
    let reprs = compute_var_reprs(&func, &classifier, &pool);

    assert_eq!(reprs.len(), 5);
    assert_eq!(reprs[0], ValueRepr::Scalar, "int → Scalar");
    assert_eq!(reprs[1], ValueRepr::FatValue, "str → FatValue");
    assert_eq!(reprs[2], ValueRepr::RcPointer, "list[int] → RcPointer");
    assert_eq!(reprs[3], ValueRepr::FatValue, "function → FatValue");
    assert_eq!(reprs[4], ValueRepr::Aggregate, "(int, str) → Aggregate");
}

#[test]
fn compute_var_reprs_empty_function() {
    let pool = Pool::new();
    let func = ArcFunction {
        name: Name::from_raw(1),
        params: vec![],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![],
        var_reprs: Vec::new(),
        spans: vec![vec![]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: Vec::new(),
    };

    let classifier = ArcClassifier::new(&pool);
    let reprs = compute_var_reprs(&func, &classifier, &pool);
    assert!(reprs.is_empty());
}

// RcStrategy::from_var

#[test]
fn rc_strategy_str_is_fat_pointer() {
    let pool = Pool::new();
    assert_eq!(
        RcStrategy::from_var(ValueRepr::FatValue, &pool, Idx::STR),
        RcStrategy::FatPointer,
    );
}

#[test]
fn rc_strategy_list_is_heap_pointer() {
    let mut pool = Pool::new();
    let list_str = pool.list(Idx::STR);
    assert_eq!(
        RcStrategy::from_var(ValueRepr::RcPointer, &pool, list_str),
        RcStrategy::HeapPointer,
    );
}

#[test]
fn rc_strategy_tuple_is_aggregate_fields() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::STR]);
    assert_eq!(
        RcStrategy::from_var(ValueRepr::Aggregate, &pool, tup),
        RcStrategy::AggregateFields,
    );
}

#[test]
fn rc_strategy_result_is_inline_enum() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::STR);
    assert_eq!(
        RcStrategy::from_var(ValueRepr::Aggregate, &pool, res),
        RcStrategy::InlineEnum,
    );
}

#[test]
fn rc_strategy_option_is_inline_enum() {
    let mut pool = Pool::new();
    let opt = pool.option(Idx::STR);
    assert_eq!(
        RcStrategy::from_var(ValueRepr::Aggregate, &pool, opt),
        RcStrategy::InlineEnum,
    );
}

#[test]
fn rc_strategy_closure_is_closure() {
    let mut pool = Pool::new();
    let func_ty = pool.function(&[Idx::INT], Idx::INT);
    assert_eq!(
        RcStrategy::from_var(ValueRepr::FatValue, &pool, func_ty),
        RcStrategy::Closure,
    );
}

#[test]
fn rc_strategy_map_is_heap_pointer() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::STR, Idx::INT);
    assert_eq!(
        RcStrategy::from_var(ValueRepr::RcPointer, &pool, map_ty),
        RcStrategy::HeapPointer,
    );
}

#[test]
fn rc_strategy_enum_is_inline_enum() {
    let mut pool = Pool::new();
    let enum_ty = pool.enum_type(
        Name::from_raw(50),
        &[
            EnumVariant {
                name: Name::from_raw(51),
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: Name::from_raw(52),
                field_types: vec![Idx::STR],
            },
        ],
    );
    assert_eq!(
        RcStrategy::from_var(ValueRepr::Aggregate, &pool, enum_ty),
        RcStrategy::InlineEnum,
    );
}

#[test]
fn rc_strategy_struct_is_aggregate_fields() {
    let mut pool = Pool::new();
    let st = pool.struct_type(
        Name::from_raw(60),
        &[
            (Name::from_raw(61), Idx::STR),
            (Name::from_raw(62), Idx::INT),
        ],
    );
    assert_eq!(
        RcStrategy::from_var(ValueRepr::Aggregate, &pool, st),
        RcStrategy::AggregateFields,
    );
}

#[test]
fn rc_strategy_set_is_heap_pointer() {
    let mut pool = Pool::new();
    let set_ty = pool.set(Idx::INT);
    assert_eq!(
        RcStrategy::from_var(ValueRepr::RcPointer, &pool, set_ty),
        RcStrategy::HeapPointer,
    );
}

// §02.3 regression: trivial compound types get Scalar repr after triviality unification.

#[test]
fn compute_var_reprs_trivial_compounds_are_scalar() {
    let mut pool = Pool::new();
    let opt_int = pool.option(Idx::INT);
    let tuple_trivial = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::BOOL]);
    let result_trivial = pool.result(Idx::INT, Idx::ORDERING);

    let func = ArcFunction {
        name: Name::from_raw(100),
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: opt_int,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: tuple_trivial,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(2),
                ty: result_trivial,
                ownership: Ownership::Owned,
            },
        ],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![opt_int, tuple_trivial, result_trivial],
        var_reprs: Vec::new(),
        spans: vec![Vec::new()],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: Vec::new(),
    };

    let classifier = ArcClassifier::new(&pool);
    let reprs = compute_var_reprs(&func, &classifier, &pool);

    assert_eq!(reprs.len(), 3);
    assert_eq!(reprs[0], ValueRepr::Scalar, "Option<int> → Scalar");
    assert_eq!(reprs[1], ValueRepr::Scalar, "(int, float, bool) → Scalar");
    assert_eq!(
        reprs[2],
        ValueRepr::Scalar,
        "Result<int, Ordering> → Scalar"
    );
}
