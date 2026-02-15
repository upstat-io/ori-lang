use ori_ir::Name;
use ori_types::{EnumVariant, Idx, Pool};
use pretty_assertions::assert_eq;

use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId};
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
    assert_eq!(info.kind, DropKind::Fields(vec![(1, Idx::STR)]));
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
        DropKind::Fields(vec![(0, Idx::STR), (2, list_int)])
    );
}

// Tuple

#[test]
fn tuple_with_rc_element() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::STR]);
    let c = cls(&pool);

    let info = compute_drop_info(tup, &c, &pool).unwrap();
    assert_eq!(info.kind, DropKind::Fields(vec![(1, Idx::STR)]));
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
        DropKind::Fields(vec![(0, Idx::STR), (1, list_int)])
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
        DropKind::Enum(vec![
            vec![],              // variant 0: int field — scalar
            vec![(0, Idx::STR)], // variant 1: str field — needs Dec
        ])
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
        DropKind::Enum(vec![
            vec![],              // variant 0: unit
            vec![(0, Idx::STR)], // variant 1: str field at 0
            vec![(0, list_str)], // variant 2: list field at 0
        ])
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
        DropKind::Enum(vec![
            vec![],              // None
            vec![(0, Idx::STR)], // Some(str)
        ])
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
        DropKind::Enum(vec![
            vec![(0, Idx::STR)], // Ok(str) — needs Dec
            vec![],              // Err(int) — scalar
        ])
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
        DropKind::Enum(vec![
            vec![],              // Ok(int) — scalar
            vec![(0, Idx::STR)], // Err(str) — needs Dec
        ])
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
        DropKind::Enum(vec![
            vec![(0, Idx::STR)], // Ok(str)
            vec![(0, Idx::STR)], // Err(str)
        ])
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
    assert_eq!(info.kind, DropKind::Fields(vec![(0, Idx::STR)]));
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
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(0),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR],
        spans: vec![vec![None, None]],
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
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(1),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, list_str],
        spans: vec![vec![None, None]],
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
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT],
        spans: vec![vec![None]],
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
    assert_eq!(info.kind, DropKind::Fields(vec![(1, opt_str)]));
}

#[test]
fn result_of_scalars_returns_none() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::FLOAT);
    let c = cls(&pool);

    // result[int, float] is Scalar → no drop needed.
    assert!(compute_drop_info(res, &c, &pool).is_none());
}
