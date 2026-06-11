//! Unit pins for the match-handoff extract-transfer verdict.
//!
//! Positive: the pin-2 shape (sum payload extracted in one arm, transferred
//! through the merge block-param into the rebuild construct; payload-less
//! other arm) widens the carrier to a FULL move. Negatives: a conditional
//! transfer inside the payload arm DECLINES (pin-4 precision gate); a boxed
//! (RcPointer-repr) view DECLINES; the injected toggle is a no-op.

use std::num::NonZeroU32;

use ori_ir::Span;
use ori_registry::burden::{TransferKind, VariantId};
use ori_types::burden::{UserBurdenSpec, UserOwnedField, UserTransferRule, UserVariantBurden};
use ori_types::{FieldDef, Idx, TypeRegistry, Visibility};
use rustc_hash::{FxHashMap, FxHashSet};

use super::apply_match_handoff_extract_transfer_with;
use crate::aims::intraprocedural::project_aliases::{
    compute_param_edge_args, compute_project_alias_table,
};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind,
    ValueRepr,
};
use crate::lower::test_utils::test_name;

const PAIR_IDX: Idx = Idx::from_raw(100);
const SUM_IDX: Idx = Idx::from_raw(101);

fn vid(raw: u32) -> VariantId {
    match NonZeroU32::new(raw) {
        Some(nz) => VariantId::new(nz),
        None => panic!("variant id must be non-zero"),
    }
}

/// Register `Pair { o: Sum, b: str }` (both fields owned) + the two-variant
/// single-payload `Sum` (variant 1 payload-less; variant 2 one str payload —
/// the user-enum shape carrying BOTH `transfers_on_match` and `retained_owned`).
fn register_pair_with_sum_field(registry: &mut TypeRegistry) {
    let sum_burden = UserBurdenSpec {
        self_heap_alloc: false,
        variant_burdens: vec![
            UserVariantBurden {
                variant_id: vid(1),
                transfers_on_match: Vec::new(),
                retained_owned: Vec::new(),
            },
            UserVariantBurden {
                variant_id: vid(2),
                transfers_on_match: vec![UserTransferRule {
                    source_field_path: vec![0],
                    binding_index: 0,
                    field_type: Idx::STR,
                    transfer_kind: TransferKind::Move,
                }],
                retained_owned: vec![UserOwnedField {
                    field_path: vec![0],
                    field_type: Idx::STR,
                }],
            },
        ],
        ..UserBurdenSpec::default()
    };
    registry.register_struct(
        test_name("Sum"),
        SUM_IDX,
        vec![],
        vec![FieldDef {
            name: test_name("payload"),
            ty: Idx::STR,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        }],
        Span::DUMMY,
        Visibility::Public,
        0,
        None,
        Some(sum_burden),
    );
    let pair_burden = UserBurdenSpec {
        self_heap_alloc: false,
        owned_fields: vec![
            UserOwnedField {
                field_path: vec![0],
                field_type: SUM_IDX,
            },
            UserOwnedField {
                field_path: vec![1],
                field_type: Idx::STR,
            },
        ],
        ..UserBurdenSpec::default()
    };
    registry.register_struct(
        test_name("Pair"),
        PAIR_IDX,
        vec![],
        vec![
            FieldDef {
                name: test_name("o"),
                ty: SUM_IDX,
                span: Span::DUMMY,
                visibility: Visibility::Public,
            },
            FieldDef {
                name: test_name("b"),
                ty: Idx::STR,
                span: Span::DUMMY,
                visibility: Visibility::Public,
            },
        ],
        Span::DUMMY,
        Visibility::Public,
        0,
        None,
        Some(pair_burden),
    );
}

/// Build the pin-2-shaped match-handoff rebuild:
///
/// ```text
/// bb0: %0 = Construct Pair()                       ; Jump bb1(%0)
/// bb1: (%1: Pair)  Branch %1 ? bb2 : bb6           ; loop header
/// bb2: %2 = %1 ; %3 = Project %2.0 (Sum view)
///      %4 = Project %3.0 (tag)                     ; Switch %4 [0=>bb3, 1=>bb4, def bb7]
/// bb3: %5 = Project %3.1 (payload)                 ; Jump bb5(%5, %1)   ; payload arm
/// bb4: %6 = Construct Str()                        ; Jump bb5(%6, %1)   ; payload-less arm
/// bb5: (%7: str, %8: Pair)
///      %9  = Construct Sum(%7)                     ; wrap
///      %10 = %8                                    ; carrier
///      %11 = Project %10.1
///      %12 = Construct Pair(%9, %11)               ; rebuild
///      Jump bb1(%12)                               ; back-edge
/// bb6: Return %1
/// bb7: Unreachable
/// ```
///
/// `conditional_arm` reroutes bb3 through a Branch (bb8/bb9 both jumping to
/// bb5) — the pin-4 conditional-transfer shape the verdict must decline.
#[expect(
    clippy::too_many_lines,
    reason = "one literal CFG fixture mirroring the real lowering shape; \
              splitting block construction fragments the load-bearing topology"
)]
fn match_handoff_rebuild(conditional_arm: bool, view_repr: ValueRepr) -> ArcFunction {
    let v = ArcVarId::new;
    let b = ArcBlockId::new;
    let mut var_types = vec![
        PAIR_IDX, // 0
        PAIR_IDX, // 1 loop param
        PAIR_IDX, // 2 view-source alias
        SUM_IDX,  // 3 sum view
        Idx::INT, // 4 tag
        Idx::STR, // 5 payload extract
        Idx::STR, // 6 fresh arm value
        Idx::STR, // 7 merge payload param
        PAIR_IDX, // 8 merge struct param
        SUM_IDX,  // 9 wrap
        PAIR_IDX, // 10 carrier
        Idx::STR, // 11 still-moved field projection
        PAIR_IDX, // 12 rebuild
    ];
    let mut var_reprs = vec![
        ValueRepr::Aggregate, // 0
        ValueRepr::Aggregate, // 1
        ValueRepr::Aggregate, // 2
        view_repr,            // 3
        ValueRepr::Scalar,    // 4
        ValueRepr::FatValue,  // 5
        ValueRepr::FatValue,  // 6
        ValueRepr::FatValue,  // 7
        ValueRepr::Aggregate, // 8
        ValueRepr::Aggregate, // 9
        ValueRepr::Aggregate, // 10
        ValueRepr::FatValue,  // 11
        ValueRepr::Aggregate, // 12
    ];

    let payload_arm_terminator = if conditional_arm {
        ArcTerminator::Branch {
            cond: v(4),
            then_block: b(8),
            else_block: b(9),
        }
    } else {
        ArcTerminator::Jump {
            target: b(5),
            args: vec![v(5), v(1)],
        }
    };

    let mut blocks = vec![
        // bb0
        ArcBlock {
            id: b(0),
            params: Vec::new(),
            body: vec![ArcInstr::Construct {
                dst: v(0),
                ty: PAIR_IDX,
                ctor: CtorKind::Struct(test_name("Pair")),
                args: vec![],
            }],
            terminator: ArcTerminator::Jump {
                target: b(1),
                args: vec![v(0)],
            },
        },
        // bb1: loop header
        ArcBlock {
            id: b(1),
            params: vec![(v(1), PAIR_IDX)],
            body: Vec::new(),
            terminator: ArcTerminator::Branch {
                cond: v(1),
                then_block: b(2),
                else_block: b(6),
            },
        },
        // bb2: view + tag + switch
        ArcBlock {
            id: b(2),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: v(2),
                    ty: PAIR_IDX,
                    value: ArcValue::Var(v(1)),
                },
                ArcInstr::Project {
                    dst: v(3),
                    ty: SUM_IDX,
                    value: v(2),
                    field: 0,
                },
                ArcInstr::Project {
                    dst: v(4),
                    ty: Idx::INT,
                    value: v(3),
                    field: 0,
                },
            ],
            terminator: ArcTerminator::Switch {
                scrutinee: v(4),
                cases: vec![(0, b(3)), (1, b(4))],
                default: b(7),
            },
        },
        // bb3: payload arm
        ArcBlock {
            id: b(3),
            params: Vec::new(),
            body: vec![ArcInstr::Project {
                dst: v(5),
                ty: Idx::STR,
                value: v(3),
                field: 1,
            }],
            terminator: payload_arm_terminator,
        },
        // bb4: payload-less arm
        ArcBlock {
            id: b(4),
            params: Vec::new(),
            body: vec![ArcInstr::Construct {
                dst: v(6),
                ty: Idx::STR,
                ctor: CtorKind::Struct(test_name("Str")),
                args: vec![],
            }],
            terminator: ArcTerminator::Jump {
                target: b(5),
                args: vec![v(6), v(1)],
            },
        },
        // bb5: merge + rebuild
        ArcBlock {
            id: b(5),
            params: vec![(v(7), Idx::STR), (v(8), PAIR_IDX)],
            body: vec![
                ArcInstr::Construct {
                    dst: v(9),
                    ty: SUM_IDX,
                    ctor: CtorKind::Struct(test_name("Sum")),
                    args: vec![v(7)],
                },
                ArcInstr::Let {
                    dst: v(10),
                    ty: PAIR_IDX,
                    value: ArcValue::Var(v(8)),
                },
                ArcInstr::Project {
                    dst: v(11),
                    ty: Idx::STR,
                    value: v(10),
                    field: 1,
                },
                ArcInstr::Construct {
                    dst: v(12),
                    ty: PAIR_IDX,
                    ctor: CtorKind::Struct(test_name("Pair")),
                    args: vec![v(9), v(11)],
                },
            ],
            terminator: ArcTerminator::Jump {
                target: b(1),
                args: vec![v(12)],
            },
        },
        // bb6: exit
        ArcBlock {
            id: b(6),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return { value: v(1) },
        },
        // bb7: switch default
        ArcBlock {
            id: b(7),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Unreachable,
        },
    ];

    if conditional_arm {
        // %13 / %14: the two conditional wrap candidates inside the arm.
        var_types.extend([Idx::STR, Idx::STR]);
        var_reprs.extend([ValueRepr::FatValue, ValueRepr::FatValue]);
        blocks.push(ArcBlock {
            id: b(8),
            params: Vec::new(),
            body: vec![ArcInstr::Construct {
                dst: v(13),
                ty: Idx::STR,
                ctor: CtorKind::Struct(test_name("Str")),
                args: vec![],
            }],
            terminator: ArcTerminator::Jump {
                target: b(5),
                args: vec![v(13), v(1)],
            },
        });
        blocks.push(ArcBlock {
            id: b(9),
            params: Vec::new(),
            body: vec![ArcInstr::Let {
                dst: v(14),
                ty: Idx::STR,
                value: ArcValue::Var(v(5)),
            }],
            terminator: ArcTerminator::Jump {
                target: b(5),
                args: vec![v(14), v(1)],
            },
        });
    }

    ArcFunction {
        var_types,
        var_reprs,
        blocks,
        entry: ArcBlockId::new(0),
        name: test_name("main"),
        params: Vec::new(),
        ..ArcFunction::default()
    }
}

/// Run the verdict over the fixture; returns the mutated
/// (`full_move_vars`, `partial_move_vars`) pair.
fn run_verdict(
    func: &ArcFunction,
    registry: &TypeRegistry,
    disabled: bool,
) -> (FxHashSet<ArcVarId>, FxHashMap<ArcVarId, Vec<u32>>) {
    let carrier = ArcVarId::new(10);
    let mut full_move_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut partial_move_vars: FxHashMap<ArcVarId, Vec<u32>> = FxHashMap::default();
    partial_move_vars.insert(carrier, vec![1]);
    let mut owned: FxHashSet<ArcVarId> = FxHashSet::default();
    owned.insert(carrier);
    let edges = compute_param_edge_args(func);
    let alias_table = compute_project_alias_table(func, &FxHashMap::default());
    apply_match_handoff_extract_transfer_with(
        disabled,
        func,
        registry,
        &owned,
        &alias_table,
        &edges,
        &mut full_move_vars,
        &mut partial_move_vars,
    );
    (full_move_vars, partial_move_vars)
}

#[test]
fn test_all_arms_transfer_widens_carrier_to_full_move() {
    let mut registry = TypeRegistry::new();
    register_pair_with_sum_field(&mut registry);
    let func = match_handoff_rebuild(false, ValueRepr::Aggregate);
    let (full, partial) = run_verdict(&func, &registry, false);
    assert!(
        full.contains(&ArcVarId::new(10)),
        "all-arms transfer absorbs the carrier into full_move_vars"
    );
    assert!(
        !partial.contains_key(&ArcVarId::new(10)),
        "the widened carrier leaves partial_move_vars"
    );
}

#[test]
fn test_conditional_transfer_inside_arm_declines() {
    let mut registry = TypeRegistry::new();
    register_pair_with_sum_field(&mut registry);
    let func = match_handoff_rebuild(true, ValueRepr::Aggregate);
    let (full, partial) = run_verdict(&func, &registry, false);
    assert!(
        !full.contains(&ArcVarId::new(10)),
        "a conditional per-arm transfer keeps the carrier out of full_move_vars"
    );
    assert_eq!(
        partial.get(&ArcVarId::new(10)),
        Some(&vec![1]),
        "the declined carrier keeps its un-widened skip set (the release stays)"
    );
}

#[test]
fn test_boxed_view_repr_declines() {
    let mut registry = TypeRegistry::new();
    register_pair_with_sum_field(&mut registry);
    let func = match_handoff_rebuild(false, ValueRepr::RcPointer);
    let (full, partial) = run_verdict(&func, &registry, false);
    assert!(
        !full.contains(&ArcVarId::new(10)),
        "a boxed (RcPointer) sum view declines — the box's own allocation is \
         not discharged by a payload transfer"
    );
    assert_eq!(partial.get(&ArcVarId::new(10)), Some(&vec![1]));
}

#[test]
fn test_disabled_toggle_is_noop() {
    let mut registry = TypeRegistry::new();
    register_pair_with_sum_field(&mut registry);
    let func = match_handoff_rebuild(false, ValueRepr::Aggregate);
    let (full, partial) = run_verdict(&func, &registry, true);
    assert!(full.is_empty(), "disabled verdict mutates nothing");
    assert_eq!(partial.get(&ArcVarId::new(10)), Some(&vec![1]));
}
