use ori_ir::Name;
use ori_types::{EnumVariant, Idx, Pool};
use pretty_assertions::assert_eq;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, RcStrategy,
};
use crate::{ArcClassifier, Ownership};

use super::*;

// Helper

fn cls(pool: &Pool) -> ArcClassifier<'_> {
    ArcClassifier::new(pool)
}

// Scalar types -> None

#[test]
fn scalar_returns_none() {
    let pool = Pool::new();
    let c = cls(&pool);

    assert!(compute_drop_info(Idx::INT, &c, &pool).is_none());
    assert!(compute_drop_info(Idx::FLOAT, &c, &pool).is_none());
    assert!(compute_drop_info(Idx::BOOL, &c, &pool).is_none());
    assert!(compute_drop_info(Idx::CHAR, &c, &pool).is_none());
    assert!(compute_drop_info(Idx::UNIT, &c, &pool).is_none());
}

#[test]
fn option_of_scalar_returns_none() {
    let mut pool = Pool::new();
    let opt_int = pool.option(Idx::INT);
    let c = cls(&pool);

    assert!(compute_drop_info(opt_int, &c, &pool).is_none());
}

#[test]
fn tuple_of_scalars_returns_none() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::BOOL]);
    let c = cls(&pool);

    assert!(compute_drop_info(tup, &c, &pool).is_none());
}

#[test]
fn struct_all_scalar_returns_none() {
    let mut pool = Pool::new();
    let s = pool.struct_type(
        Name::from_raw(10),
        &[
            (Name::from_raw(11), Idx::INT),
            (Name::from_raw(12), Idx::FLOAT),
        ],
    );
    let c = cls(&pool);

    assert!(compute_drop_info(s, &c, &pool).is_none());
}

#[test]
fn enum_all_unit_variants_returns_none() {
    let mut pool = Pool::new();
    let e = pool.enum_type(
        Name::from_raw(20),
        &[
            EnumVariant {
                name: Name::from_raw(21),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(22),
                field_types: vec![],
            },
        ],
    );
    let c = cls(&pool);

    assert!(compute_drop_info(e, &c, &pool).is_none());
}

// str -> Trivial

#[test]
fn str_is_trivial() {
    let pool = Pool::new();
    let c = cls(&pool);

    let info = compute_drop_info(Idx::STR, &c, &pool).unwrap();
    assert_eq!(info.ty, Idx::STR);
    assert_eq!(info.kind, DropKind::Trivial);
}

// List

#[test]
fn list_of_scalar_is_trivial() {
    let mut pool = Pool::new();
    let list = pool.list(Idx::INT);
    let c = cls(&pool);

    let info = compute_drop_info(list, &c, &pool).unwrap();
    assert_eq!(info.kind, DropKind::Trivial);
}

#[test]
fn list_of_str_is_collection() {
    let mut pool = Pool::new();
    let list = pool.list(Idx::STR);
    let c = cls(&pool);

    let info = compute_drop_info(list, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Collection {
            element_type: Idx::STR
        }
    );
}

#[test]
fn list_of_list_is_collection() {
    let mut pool = Pool::new();
    let inner = pool.list(Idx::INT);
    let outer = pool.list(inner);
    let c = cls(&pool);

    let info = compute_drop_info(outer, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Collection {
            element_type: inner
        }
    );
}

// Set

#[test]
fn set_of_scalar_is_trivial() {
    let mut pool = Pool::new();
    let set = pool.set(Idx::INT);
    let c = cls(&pool);

    let info = compute_drop_info(set, &c, &pool).unwrap();
    assert_eq!(info.kind, DropKind::Trivial);
}

#[test]
fn set_of_str_is_collection() {
    let mut pool = Pool::new();
    let set = pool.set(Idx::STR);
    let c = cls(&pool);

    let info = compute_drop_info(set, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Collection {
            element_type: Idx::STR
        }
    );
}

// Map

#[test]
fn map_scalar_keys_and_values_is_trivial() {
    let mut pool = Pool::new();
    let map = pool.map(Idx::INT, Idx::FLOAT);
    let c = cls(&pool);

    let info = compute_drop_info(map, &c, &pool).unwrap();
    assert_eq!(info.kind, DropKind::Trivial);
}

#[test]
fn map_str_keys_scalar_values() {
    let mut pool = Pool::new();
    let map = pool.map(Idx::STR, Idx::INT);
    let c = cls(&pool);

    let info = compute_drop_info(map, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Map {
            key_type: Idx::STR,
            value_type: Idx::INT,
            dec_keys: true,
            dec_values: false,
        }
    );
}

#[test]
fn map_scalar_keys_str_values() {
    let mut pool = Pool::new();
    let map = pool.map(Idx::INT, Idx::STR);
    let c = cls(&pool);

    let info = compute_drop_info(map, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Map {
            key_type: Idx::INT,
            value_type: Idx::STR,
            dec_keys: false,
            dec_values: true,
        }
    );
}

#[test]
fn map_str_keys_str_values() {
    let mut pool = Pool::new();
    let map = pool.map(Idx::STR, Idx::STR);
    let c = cls(&pool);

    let info = compute_drop_info(map, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Map {
            key_type: Idx::STR,
            value_type: Idx::STR,
            dec_keys: true,
            dec_values: true,
        }
    );
}

// Struct

#[test]
fn struct_with_one_rc_field() {
    let mut pool = Pool::new();
    let s = pool.struct_type(
        Name::from_raw(30),
        &[
            (Name::from_raw(31), Idx::INT),
            (Name::from_raw(32), Idx::STR),
        ],
    );
    let c = cls(&pool);

    let info = compute_drop_info(s, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Fields {
            fields: vec![(1, Idx::STR)],
            user_drop: None
        }
    );
}

#[test]
fn struct_with_multiple_rc_fields() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let s = pool.struct_type(
        Name::from_raw(40),
        &[
            (Name::from_raw(41), Idx::STR),
            (Name::from_raw(42), Idx::INT),
            (Name::from_raw(43), list_int),
        ],
    );
    let c = cls(&pool);

    let info = compute_drop_info(s, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Fields {
            fields: vec![(0, Idx::STR), (2, list_int)],
            user_drop: None
        }
    );
}

// Tuple

#[test]
fn tuple_with_rc_element() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::STR]);
    let c = cls(&pool);

    let info = compute_drop_info(tup, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Fields {
            fields: vec![(1, Idx::STR)],
            user_drop: None
        }
    );
}

#[test]
fn tuple_all_rc_elements() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let tup = pool.tuple(&[Idx::STR, list_int]);
    let c = cls(&pool);

    let info = compute_drop_info(tup, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Fields {
            fields: vec![(0, Idx::STR), (1, list_int)],
            user_drop: None
        }
    );
}

// Enum

#[test]
fn enum_with_rc_variant_fields() {
    let mut pool = Pool::new();
    let e = pool.enum_type(
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
    let c = cls(&pool);

    let info = compute_drop_info(e, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Enum {
            variants: vec![
                vec![],              // variant 0: int field — scalar
                vec![(0, Idx::STR)], // variant 1: str field — needs Dec
            ],
            user_drop: None
        }
    );
}

#[test]
fn enum_with_mixed_variant_fields() {
    let mut pool = Pool::new();
    let list_str = pool.list(Idx::STR);
    let e = pool.enum_type(
        Name::from_raw(60),
        &[
            EnumVariant {
                name: Name::from_raw(61),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(62),
                field_types: vec![Idx::STR, Idx::INT],
            },
            EnumVariant {
                name: Name::from_raw(63),
                field_types: vec![list_str],
            },
        ],
    );
    let c = cls(&pool);

    let info = compute_drop_info(e, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Enum {
            variants: vec![
                vec![],              // variant 0: unit
                vec![(0, Idx::STR)], // variant 1: str field at 0
                vec![(0, list_str)], // variant 2: list field at 0
            ],
            user_drop: None
        }
    );
}

#[test]
fn enum_all_scalar_payloads_returns_none() {
    let mut pool = Pool::new();
    let e = pool.enum_type(
        Name::from_raw(70),
        &[
            EnumVariant {
                name: Name::from_raw(71),
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: Name::from_raw(72),
                field_types: vec![Idx::FLOAT],
            },
        ],
    );
    let c = cls(&pool);

    // The enum itself is Scalar because all payloads are scalar.
    assert!(compute_drop_info(e, &c, &pool).is_none());
}

// Option

#[test]
fn option_str_is_enum_drop() {
    let mut pool = Pool::new();
    let opt = pool.option(Idx::STR);
    let c = cls(&pool);

    let info = compute_drop_info(opt, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Enum {
            variants: vec![
                vec![],              // None
                vec![(0, Idx::STR)], // Some(str)
            ],
            user_drop: None
        }
    );
}

// Result

#[test]
fn result_str_int_drops_ok_only() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::STR, Idx::INT);
    let c = cls(&pool);

    let info = compute_drop_info(res, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Enum {
            variants: vec![
                vec![(0, Idx::STR)], // Ok(str) — needs Dec
                vec![],              // Err(int) — scalar
            ],
            user_drop: None
        }
    );
}

#[test]
fn result_int_str_drops_err_only() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::STR);
    let c = cls(&pool);

    let info = compute_drop_info(res, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Enum {
            variants: vec![
                vec![],              // Ok(int) — scalar
                vec![(0, Idx::STR)], // Err(str) — needs Dec
            ],
            user_drop: None
        }
    );
}

#[test]
fn result_str_str_drops_both() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::STR, Idx::STR);
    let c = cls(&pool);

    let info = compute_drop_info(res, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Enum {
            variants: vec![
                vec![(0, Idx::STR)], // Ok(str)
                vec![(0, Idx::STR)], // Err(str)
            ],
            user_drop: None
        }
    );
}

// Channel

#[test]
fn channel_is_trivial() {
    let mut pool = Pool::new();
    let chan = pool.channel(Idx::INT);
    let c = cls(&pool);

    let info = compute_drop_info(chan, &c, &pool).unwrap();
    assert_eq!(info.kind, DropKind::Trivial);
}

// Function

#[test]
fn function_is_trivial() {
    let mut pool = Pool::new();
    let func = pool.function(&[Idx::INT], Idx::STR);
    let c = cls(&pool);

    let info = compute_drop_info(func, &c, &pool).unwrap();
    assert_eq!(info.kind, DropKind::Trivial);
}

// Named type resolution

#[test]
fn named_type_resolves_to_struct_drop() {
    let mut pool = Pool::new();
    let name = Name::from_raw(80);
    let named_idx = pool.named(name);
    let struct_idx = pool.struct_type(
        name,
        &[
            (Name::from_raw(81), Idx::STR),
            (Name::from_raw(82), Idx::INT),
        ],
    );
    pool.set_resolution(named_idx, struct_idx);
    let c = cls(&pool);

    let info = compute_drop_info(named_idx, &c, &pool).unwrap();
    assert_eq!(
        info.kind,
        DropKind::Fields {
            fields: vec![(0, Idx::STR)],
            user_drop: None
        }
    );
}

// Closure env drop

#[test]
fn closure_env_all_scalar() {
    let pool = Pool::new();
    let c = cls(&pool);

    let kind = compute_closure_env_drop(&[Idx::INT, Idx::FLOAT], &c);
    assert_eq!(kind, DropKind::Trivial);
}

#[test]
fn closure_env_with_rc_captures() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let c = cls(&pool);

    let kind = compute_closure_env_drop(&[Idx::INT, Idx::STR, list_int], &c);
    assert_eq!(
        kind,
        DropKind::ClosureEnv(vec![(1, Idx::STR), (2, list_int)])
    );
}

#[test]
fn closure_env_single_rc_capture() {
    let pool = Pool::new();
    let c = cls(&pool);

    let kind = compute_closure_env_drop(&[Idx::STR], &c);
    assert_eq!(kind, DropKind::ClosureEnv(vec![(0, Idx::STR)]));
}

// collect_drop_infos

#[test]
fn collect_from_empty_functions() {
    let pool = Pool::new();
    let c = cls(&pool);

    let infos = collect_drop_infos(&[], &c, &pool);
    assert!(infos.is_empty());
}

#[test]
fn collect_deduplicates_types() {
    let pool = Pool::new();
    let c = cls(&pool);

    // Two RcDec instructions for the same type (str) → one DropInfo.
    let func = ArcFunction {
        name: Name::from_raw(100),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec {
                    var: ArcVarId::new(0),
                    strategy: RcStrategy::HeapPointer,
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(0),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR],
        var_reprs: Vec::new(),
        var_rc_strategies: Vec::new(),
        spans: vec![vec![None, None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: Vec::new(),
        burden_emitted: Vec::new(),
    };

    let infos = collect_drop_infos(&[func], &c, &pool);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].ty, Idx::STR);
    assert_eq!(infos[0].kind, DropKind::Trivial);
}

#[test]
fn collect_multiple_types() {
    let mut pool = Pool::new();
    let list_str = pool.list(Idx::STR);
    let c = cls(&pool);

    let func = ArcFunction {
        name: Name::from_raw(110),
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: list_str,
                ownership: Ownership::Owned,
            },
        ],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec {
                    var: ArcVarId::new(0),
                    strategy: RcStrategy::HeapPointer,
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(1),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, list_str],
        var_reprs: Vec::new(),
        var_rc_strategies: Vec::new(),
        spans: vec![vec![None, None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: Vec::new(),
        burden_emitted: Vec::new(),
    };

    let infos = collect_drop_infos(&[func], &c, &pool);
    assert_eq!(infos.len(), 2);

    // First: str (Trivial), Second: [str] (Collection)
    let str_info = infos.iter().find(|i| i.ty == Idx::STR).unwrap();
    assert_eq!(str_info.kind, DropKind::Trivial);

    let list_info = infos.iter().find(|i| i.ty == list_str).unwrap();
    assert_eq!(
        list_info.kind,
        DropKind::Collection {
            element_type: Idx::STR
        }
    );
}

#[test]
fn collect_skips_scalar_rc_dec() {
    let pool = Pool::new();
    let c = cls(&pool);

    // RcDec on an int variable — should be skipped (classifier says no RC).
    let func = ArcFunction {
        name: Name::from_raw(120),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::RcDec {
                var: ArcVarId::new(0),
                strategy: RcStrategy::HeapPointer,
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT],
        var_reprs: Vec::new(),
        var_rc_strategies: Vec::new(),
        spans: vec![vec![None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: Vec::new(),
        burden_emitted: Vec::new(),
    };

    let infos = collect_drop_infos(&[func], &c, &pool);
    assert!(infos.is_empty());
}

// Nested compound types

#[test]
fn struct_with_nested_option_str_field() {
    let mut pool = Pool::new();
    let opt_str = pool.option(Idx::STR);
    let s = pool.struct_type(
        Name::from_raw(130),
        &[
            (Name::from_raw(131), Idx::INT),
            (Name::from_raw(132), opt_str),
        ],
    );
    let c = cls(&pool);

    let info = compute_drop_info(s, &c, &pool).unwrap();
    // Field 1 is option[str] which is DefiniteRef → needs Dec.
    assert_eq!(
        info.kind,
        DropKind::Fields {
            fields: vec![(1, opt_str)],
            user_drop: None
        }
    );
}

#[test]
fn result_of_scalars_returns_none() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::FLOAT);
    let c = cls(&pool);

    // result[int, float] is Scalar → no drop needed.
    assert!(compute_drop_info(res, &c, &pool).is_none());
}

// §02.3 regression: trivial compound types get no drop after triviality unification.

#[test]
fn result_int_ordering_returns_none() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::ORDERING);
    let c = cls(&pool);

    // Result<int, Ordering> is trivially composed → Scalar → no drop.
    assert!(compute_drop_info(res, &c, &pool).is_none());
}

#[test]
fn iterator_compute_drop_info_returns_none() {
    let mut pool = Pool::new();
    let iter = pool.iterator(Idx::INT);
    let c = cls(&pool);

    // semantic pin: Iterator<int> is non-trivial (needs
    // `ori_iter_drop` at scope exit), BUT `compute_drop_info` must
    // return `None` so that `collect_drop_infos` does not ask the
    // codegen layer to generate a per-type `_ori_drop$Iterator<int>`
    // function. The ARC emitter dispatches iterator drops inline via
    // `RcStrategy::Iterator` (top-level) and the `Tag::Iterator` arm
    // of `dec_value_rc_inner` (compound traversal); both emit
    // `ori_iter_drop(ptr)` directly. Generating a drop function would
    // call `ori_rc_free` on a Box-allocated pointer and corrupt
    // memory.
    let info = compute_drop_info(iter, &c, &pool);
    assert!(
        info.is_none(),
        "Iterator<int> must not produce a per-type drop function — ori_iter_drop is emitted inline"
    );
}

#[test]
fn double_ended_iterator_compute_drop_info_returns_none() {
    let mut pool = Pool::new();
    let deiter = pool.double_ended_iterator(Idx::INT);
    let c = cls(&pool);

    // semantic pin: DoubleEndedIterator<int> same as
    // `iterator_compute_drop_info_returns_none`.
    let info = compute_drop_info(deiter, &c, &pool);
    assert!(
        info.is_none(),
        "DoubleEndedIterator<int> must not produce a per-type drop function — ori_iter_drop is emitted inline"
    );
}

// §02.3.A structural-equivalence pin
//
// For every concrete type currently exercised by the surrounding tests,
// the BurdenSpec-lifted wrapper at `compute_drop_info` MUST produce the
// same `DropInfo` shape (kind discriminant + nested structure) as the
// pre-migration `compute_drop_kind` would have produced. The pre-migration
// expected shapes are hard-coded here as a snapshot vs reading from a
// separate fixture file: keeping the snapshot inline makes drift
// immediately visible in code review, and the cell count (one per
// existing test cell) is bounded by the matrix that drop/tests.rs already
// pins.
//
// Fails if the wrapper rewrite drops a `Variant` arm, collapses a
// `Fields` to `Trivial`, reorders fields within a struct/tuple/variant,
// or swaps key/value channels on a Map.

#[test]
fn drop_info_via_burden_matches_legacy() {
    let mut pool = Pool::new();

    // Intern every fixture type first, then construct the classifier
    // (the classifier holds an immutable `&Pool` borrow for its lifetime).
    let list_int = pool.list(Idx::INT);
    let list_str = pool.list(Idx::STR);
    let map_int_str = pool.map(Idx::INT, Idx::STR);
    let struct_one_rc = pool.struct_type(
        Name::from_raw(1000),
        &[
            (Name::from_raw(1001), Idx::INT),
            (Name::from_raw(1002), Idx::STR),
        ],
    );
    let tup = pool.tuple(&[Idx::INT, Idx::STR]);
    let enum_mixed = pool.enum_type(
        Name::from_raw(2000),
        &[
            EnumVariant {
                name: Name::from_raw(2001),
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: Name::from_raw(2002),
                field_types: vec![Idx::STR],
            },
        ],
    );
    let opt_str = pool.option(Idx::STR);
    let res_str_int = pool.result(Idx::STR, Idx::INT);

    let c = cls(&pool);

    // Triviality cells — `compute_drop_info` returns `None` (scalar) OR
    // `Some(DropInfo { kind: Trivial })` (heap with no RC children).
    assert!(compute_drop_info(Idx::INT, &c, &pool).is_none());
    assert!(compute_drop_info(Idx::FLOAT, &c, &pool).is_none());
    assert!(compute_drop_info(Idx::BOOL, &c, &pool).is_none());
    assert_eq!(
        compute_drop_info(Idx::STR, &c, &pool).unwrap().kind,
        DropKind::Trivial,
    );

    // List<int> — element scalar → Trivial.
    assert_eq!(
        compute_drop_info(list_int, &c, &pool).unwrap().kind,
        DropKind::Trivial,
    );

    // List<str> — Collection { element_type: str }.
    assert_eq!(
        compute_drop_info(list_str, &c, &pool).unwrap().kind,
        DropKind::Collection {
            element_type: Idx::STR,
        },
    );

    // Map<int, str> — Map { dec_keys: false, dec_values: true }.
    assert_eq!(
        compute_drop_info(map_int_str, &c, &pool).unwrap().kind,
        DropKind::Map {
            key_type: Idx::INT,
            value_type: Idx::STR,
            dec_keys: false,
            dec_values: true,
        },
    );

    // Struct with one RC field — Fields(vec![(1, Idx::STR)]).
    assert_eq!(
        compute_drop_info(struct_one_rc, &c, &pool).unwrap().kind,
        DropKind::Fields {
            fields: vec![(1, Idx::STR)],
            user_drop: None
        },
    );

    // Tuple with mixed scalar + RC — Fields(vec![(1, Idx::STR)]).
    assert_eq!(
        compute_drop_info(tup, &c, &pool).unwrap().kind,
        DropKind::Fields {
            fields: vec![(1, Idx::STR)],
            user_drop: None
        },
    );

    // Enum with mixed variant RC fields — Enum([[], [(0, STR)]]).
    assert_eq!(
        compute_drop_info(enum_mixed, &c, &pool).unwrap().kind,
        DropKind::Enum {
            variants: vec![vec![], vec![(0, Idx::STR)]],
            user_drop: None
        },
    );

    // Option<str> — Enum([[], [(0, STR)]]).
    assert_eq!(
        compute_drop_info(opt_str, &c, &pool).unwrap().kind,
        DropKind::Enum {
            variants: vec![vec![], vec![(0, Idx::STR)]],
            user_drop: None
        },
    );

    // Result<str, int> — Enum([[(0, STR)], []]).
    assert_eq!(
        compute_drop_info(res_str_int, &c, &pool).unwrap().kind,
        DropKind::Enum {
            variants: vec![vec![(0, Idx::STR)], vec![]],
            user_drop: None
        },
    );

    // Closure environment with mixed captures — ClosureEnv(vec![(1, STR)]).
    let env_kind = compute_closure_env_drop(&[Idx::INT, Idx::STR], &c);
    assert_eq!(env_kind, DropKind::ClosureEnv(vec![(1, Idx::STR)]));

    // Closure environment with no RC captures — Trivial.
    let env_trivial = compute_closure_env_drop(&[Idx::INT, Idx::FLOAT], &c);
    assert_eq!(env_trivial, DropKind::Trivial);
}

// §02.3.A positive pin — `Option<str>` regression
//
// The same observation as `option_str_is_enum_drop` above, but written
// against the BurdenSpec-lifted wrapper specifically. Co-existing with
// the original test makes accidental rewrites to the wrapper visible —
// dropping the positive pin would also delete this test name from the
// suite roster, surfacing the regression at audit time.

#[test]
fn drop_info_via_burden_for_option_str() {
    let mut pool = Pool::new();
    let opt = pool.option(Idx::STR);
    let c = cls(&pool);

    let Some(info) = compute_drop_info(opt, &c, &pool) else {
        panic!("Option<str> has non-trivial drop");
    };
    assert_eq!(info.ty, opt);
    assert_eq!(
        info.kind,
        DropKind::Enum {
            variants: vec![
                vec![],              // None
                vec![(0, Idx::STR)], // Some(str)
            ],
            user_drop: None
        }
    );
}

// §02.3.A negative pin — newly-monomorphized generic
//
// Engineered fixture: intern a fresh `Result<List<str>, str>` AFTER the
// pool already contains an unrelated `Result` cell, then call the
// wrapper for the freshly-interned type. The wrapper MUST observe the
// freshly-monomorphized shape (Enum with both RC variants) — NOT a
// stale entry from the earlier interning, NOT a default-empty shape.
//
// Guards against introduce-then-cache regressions where the wrapper
// might short-circuit on a global cache keyed by something other than
// the exact `Idx`.

#[test]
fn drop_info_via_burden_for_newly_monomorphized_generic() {
    let mut pool = Pool::new();

    // Intern both Result cells BEFORE constructing the classifier.
    // Pre-existing Result cell (scalar both sides → Trivial).
    let res_scalar = pool.result(Idx::INT, Idx::INT);
    // NEW Result cell with structurally distinct payloads. Idx must
    // differ from the prior one because the inner types differ.
    let list_str = pool.list(Idx::STR);
    let res_fresh = pool.result(list_str, Idx::STR);
    assert_ne!(
        res_fresh, res_scalar,
        "the fresh monomorphization must intern as a distinct Idx",
    );

    let c = cls(&pool);

    // Pre-existing scalar Result remains Trivial.
    assert!(compute_drop_info(res_scalar, &c, &pool).is_none());

    // Wrapper must observe the freshly-composed shape for the new cell.
    let Some(info) = compute_drop_info(res_fresh, &c, &pool) else {
        panic!("Result<List<str>, str> has non-trivial drop");
    };
    assert_eq!(info.ty, res_fresh);
    assert_eq!(
        info.kind,
        DropKind::Enum {
            variants: vec![
                vec![(0, list_str)], // Ok(List<str>) — needs Dec
                vec![(0, Idx::STR)], // Err(str) — needs Dec
            ],
            user_drop: None
        }
    );
}

// type_drop_may_unwind — may-unwind drop predicate.
//
// The local "@drop" fact is parameterized via `has_user_drop`, so the drop-tree
// recursion + cycle-soundness are exercised white-box here, independent of the
// codegen `method_functions` map (the real local-check SSOT on the codegen
// path). `drop_types` is the set of Idxs that carry a user `@drop`.
//
// KNOWN LIMITATION (consistent with Step-0 emission, NOT a new unsoundness):
// `compute_drop_info` is RC-filtered — an all-scalar-field Drop struct reports
// no RC need and is omitted from a parent's child list, so a NESTED all-scalar
// Drop type is not reached through the structure walk. The top-level local
// check still catches an all-scalar Drop type directly. Realistic Drop types
// own heap/str fields (RC'd) and are fully covered. The nested-all-scalar gap
// is a facet of the burden-registry-not-threaded condition owned by the
// aims-burden-tracking plan (§06); Step-0 `@drop` emission shares the same
// RC-filtered structure, so the predicate matches emission reality.

fn may_unwind(ty: Idx, pool: &Pool, drop_types: &[Idx]) -> bool {
    let c = cls(pool);
    let mut memo: rustc_hash::FxHashMap<Idx, bool> = rustc_hash::FxHashMap::default();
    let has_user_drop = |t: Idx| drop_types.contains(&t);
    type_drop_may_unwind(ty, &c, pool, &has_user_drop, &mut memo)
}

#[test]
fn may_unwind_scalar_and_trivial_never_unwind() {
    let pool = Pool::new();
    assert!(!may_unwind(Idx::INT, &pool, &[]));
    assert!(!may_unwind(Idx::FLOAT, &pool, &[]));
    assert!(!may_unwind(Idx::STR, &pool, &[])); // str: Trivial, no @drop
}

#[test]
fn may_unwind_list_of_scalar_does_not_unwind() {
    let mut pool = Pool::new();
    let list = pool.list(Idx::INT);
    assert!(!may_unwind(list, &pool, &[]));
}

#[test]
fn may_unwind_top_level_all_scalar_drop_type_is_caught_by_local_check() {
    // An all-scalar Drop struct collapses to Trivial/None in compute_drop_info
    // (user_drop hardcoded None); the local-check-first path catches it.
    let mut pool = Pool::new();
    let s = pool.struct_type(Name::from_raw(700), &[(Name::from_raw(701), Idx::INT)]);
    assert!(may_unwind(s, &pool, &[s])); // s itself carries @drop
    assert!(!may_unwind(s, &pool, &[])); // no @drop anywhere
}

#[test]
fn may_unwind_struct_with_drop_field_unwinds() {
    // Realistic shape: the inner Drop type owns a `str` (RC'd, so it appears in
    // the outer struct's drop-tree). Outer has no @drop of its own.
    let mut pool = Pool::new();
    let inner = pool.struct_type(Name::from_raw(710), &[(Name::from_raw(711), Idx::STR)]);
    let outer = pool.struct_type(Name::from_raw(712), &[(Name::from_raw(713), inner)]);
    assert!(may_unwind(outer, &pool, &[inner])); // reached via the inner field
    assert!(!may_unwind(outer, &pool, &[])); // str-only drop tree, no @drop
}

#[test]
fn may_unwind_collection_of_drop_element_unwinds() {
    // `[DropElem]` where DropElem owns a str and carries @drop.
    let mut pool = Pool::new();
    let elem = pool.struct_type(Name::from_raw(720), &[(Name::from_raw(721), Idx::STR)]);
    let list = pool.list(elem);
    assert!(may_unwind(list, &pool, &[elem]));
    assert!(!may_unwind(list, &pool, &[])); // list of non-@drop structs
}

#[test]
fn may_unwind_map_value_drop_unwinds_keys_do_not_when_scalar() {
    let mut pool = Pool::new();
    let val = pool.struct_type(Name::from_raw(730), &[(Name::from_raw(731), Idx::STR)]);
    let map = pool.map(Idx::INT, val); // scalar key, Drop value
    assert!(may_unwind(map, &pool, &[val])); // reached via dec_values
    assert!(!may_unwind(map, &pool, &[]));
}

#[test]
fn may_unwind_map_key_drop_unwinds() {
    let mut pool = Pool::new();
    let key = pool.struct_type(Name::from_raw(740), &[(Name::from_raw(741), Idx::STR)]);
    let map = pool.map(key, Idx::INT); // Drop key, scalar value
    assert!(may_unwind(map, &pool, &[key])); // reached via dec_keys
}

#[test]
fn may_unwind_enum_variant_payload_drop_unwinds() {
    // Option<DropElem> — the Some payload reaches a @drop type.
    let mut pool = Pool::new();
    let elem = pool.struct_type(Name::from_raw(750), &[(Name::from_raw(751), Idx::STR)]);
    let opt = pool.option(elem);
    assert!(may_unwind(opt, &pool, &[elem]));
    assert!(!may_unwind(opt, &pool, &[]));
}

#[test]
fn may_unwind_negative_pin_plain_str_struct_does_not_unwind() {
    // Negative pin: a struct owning only str/list (Trivial element drops) with
    // NO @drop anywhere must NOT be classified may-unwind — else every
    // collection-bearing function would get spurious landing pads.
    let mut pool = Pool::new();
    let list_str = pool.list(Idx::STR);
    let s = pool.struct_type(
        Name::from_raw(760),
        &[
            (Name::from_raw(761), Idx::STR),
            (Name::from_raw(762), list_str),
        ],
    );
    assert!(!may_unwind(s, &pool, &[]));
}
