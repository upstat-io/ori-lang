//! Tests for drop hint computation.

use ori_ir::Name;
use ori_types::{Idx, Pool};

use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcValue, CtorKind, RcStrategy};
use crate::test_helpers::{make_func, owned_param, v};

use super::compute_drop_hints;

/// Fresh list constructed and dropped in same block → unique hint.
#[test]
fn fresh_list_dropped_locally() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    // v0: param (int element)
    // v1: Construct ListLiteral [v0] → fresh [int]
    // v2: unit
    // RcDec v1  ← should be unique
    // Return v2
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: v(1),
                    ty: list_int,
                    ctor: CtorKind::ListLiteral,
                    args: vec![v(0)],
                },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(crate::ir::LitValue::Unit),
                },
                ArcInstr::RcDec {
                    var: v(1),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::INT, list_int, Idx::UNIT],
    );

    let hints = compute_drop_hints(&func, &pool);
    assert!(
        hints.is_unique_drop(0, 2),
        "RcDec of fresh local list should be unique"
    );
    assert_eq!(hints.len(), 1);
}

/// Parameter list dropped → NOT unique (may be shared by caller).
#[test]
fn parameter_list_not_unique() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    // v0: param [int] — NOT constructed here
    // v1: unit
    // RcDec v0  ← should NOT be unique
    // Return v1
    let func = make_func(
        vec![owned_param(0, list_int)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(crate::ir::LitValue::Unit),
                },
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![list_int, Idx::UNIT],
    );

    let hints = compute_drop_hints(&func, &pool);
    assert!(
        !hints.is_unique_drop(0, 1),
        "Parameter list should not be unique"
    );
    assert!(hints.is_empty());
}

/// List with `RcInc` → NOT unique (was shared at some point).
#[test]
fn incremented_list_not_unique() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    // v0: param (int)
    // v1: Construct ListLiteral → fresh
    // RcInc v1  ← shared!
    // v2: unit
    // RcDec v1  ← NOT unique
    // Return v2
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: v(1),
                    ty: list_int,
                    ctor: CtorKind::ListLiteral,
                    args: vec![v(0)],
                },
                ArcInstr::RcInc {
                    var: v(1),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(crate::ir::LitValue::Unit),
                },
                ArcInstr::RcDec {
                    var: v(1),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::INT, list_int, Idx::UNIT],
    );

    let hints = compute_drop_hints(&func, &pool);
    assert!(
        !hints.is_unique_drop(0, 3),
        "Incremented list should not be unique"
    );
    assert!(hints.is_empty());
}

/// Struct Construct dropped → NOT unique (not a collection).
#[test]
fn struct_construct_not_collection() {
    // v0: Construct Struct → NOT a collection type
    // v1: unit
    // RcDec v0  ← NOT unique (int type, not collection)
    let func = make_func(
        vec![],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: v(0),
                    ty: Idx::INT, // Not a collection type
                    ctor: CtorKind::Struct(Name::from_raw(42)),
                    args: vec![],
                },
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(crate::ir::LitValue::Unit),
                },
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::AggregateFields,
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::INT, Idx::UNIT],
    );

    let pool = Pool::new();
    let hints = compute_drop_hints(&func, &pool);
    assert!(
        hints.is_empty(),
        "Struct construct should not get unique drop hint"
    );
}

/// Fresh map constructed and dropped → unique hint.
#[test]
fn fresh_map_dropped_locally() {
    let mut pool = Pool::new();
    let map_type = pool.map(Idx::STR, Idx::INT);

    // v0: Construct MapLiteral → fresh {str: int}
    // v1: unit
    // RcDec v0  ← should be unique
    let func = make_func(
        vec![],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: v(0),
                    ty: map_type,
                    ctor: CtorKind::MapLiteral,
                    args: vec![],
                },
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(crate::ir::LitValue::Unit),
                },
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![map_type, Idx::UNIT],
    );

    let hints = compute_drop_hints(&func, &pool);
    assert!(
        hints.is_unique_drop(0, 2),
        "Fresh map should be unique-droppable"
    );
    assert_eq!(hints.len(), 1);
}

/// Multiple independent lists — each gets its own hint.
#[test]
fn multiple_independent_lists() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    // v0: Construct ListLiteral
    // v1: Construct ListLiteral
    // v2: unit
    // RcDec v0  ← unique
    // RcDec v1  ← unique
    let func = make_func(
        vec![],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: v(0),
                    ty: list_int,
                    ctor: CtorKind::ListLiteral,
                    args: vec![],
                },
                ArcInstr::Construct {
                    dst: v(1),
                    ty: list_int,
                    ctor: CtorKind::ListLiteral,
                    args: vec![],
                },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(crate::ir::LitValue::Unit),
                },
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
                ArcInstr::RcDec {
                    var: v(1),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![list_int, list_int, Idx::UNIT],
    );

    let hints = compute_drop_hints(&func, &pool);
    assert!(hints.is_unique_drop(0, 3), "First list should be unique");
    assert!(hints.is_unique_drop(0, 4), "Second list should be unique");
    assert_eq!(hints.len(), 2);
}

/// Apply result dropped → NOT unique (not constructed locally).
#[test]
fn apply_result_not_unique() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    // v0: Apply some_fn() → list_int (result of function call)
    // v1: unit
    // RcDec v0  ← NOT unique (not Construct)
    let func = make_func(
        vec![],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: v(0),
                    ty: list_int,
                    func: Name::from_raw(99),
                    args: vec![],
                    arg_ownership: vec![],
                },
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(crate::ir::LitValue::Unit),
                },
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![list_int, Idx::UNIT],
    );

    let hints = compute_drop_hints(&func, &pool);
    assert!(
        hints.is_empty(),
        "Apply result should not be unique-droppable"
    );
}

/// Empty `DropHints` defaults correctly.
#[test]
fn empty_hints() {
    let hints = super::DropHints::new();
    assert!(hints.is_empty());
    assert_eq!(hints.len(), 0);
    assert!(!hints.is_unique_drop(0, 0));
}
