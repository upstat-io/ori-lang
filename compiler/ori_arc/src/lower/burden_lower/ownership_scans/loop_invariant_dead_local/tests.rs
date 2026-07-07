//! Unit pins for the RL-5 loop-invariant dead-local release scan
//! `compute_loop_invariant_dead_local_releases`. Positive pins — the
//! purely-dead canonical shape (fresh collection threaded UNCHANGED through
//! loop block-params, never read) and the borrow-only-read sibling each emit
//! EXACTLY ONE release at the terminal dead block-param. Negative pins — a
//! value-read member, a non-collection root, a reassigned loop slot, an
//! owned-position consume, a multi-terminal fork, and an RC-free terminal all
//! decline (the double-free / over-fire guards). Spec: Annex E §AIMS RL-5.

use ori_ir::Name;

use crate::LitValue;
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind,
};

use super::compute_loop_invariant_dead_local_releases;

/// Build the canonical purely-dead loop-invariant shape:
/// ```text
/// bb0: %0 = Construct <ctor>; %1 = Let Literal(true); Jump bb1(%0)
/// bb1(params: [%2]): Branch %1 -> bb2 / bb3
/// bb2: Jump bb1(%2)                    <- back edge, threads %2 unchanged
/// bb3: Jump bb4(%2)                    <- loop exit hand-off
/// bb4(params: [%3]): Return %1         <- %3 is the terminal DEAD param
/// ```
/// Members: {%0, %2, %3}; every use is a Jump-arg feeding a member.
fn loop_invariant_func(ctor: CtorKind) -> ArcFunction {
    let v = ArcVarId::new;
    let ty = Idx::from_raw(1);
    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Construct {
                dst: v(0),
                ty,
                ctor,
                args: Vec::new(),
            },
            ArcInstr::Let {
                dst: v(1),
                ty: Idx::BOOL,
                value: ArcValue::Literal(LitValue::Bool(true)),
            },
        ],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v(0)],
        },
    };
    let bb1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![(v(2), ty)],
        body: Vec::new(),
        terminator: ArcTerminator::Branch {
            cond: v(1),
            then_block: ArcBlockId::new(2),
            else_block: ArcBlockId::new(3),
        },
    };
    let bb2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v(2)],
        },
    };
    let bb3 = ArcBlock {
        id: ArcBlockId::new(3),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(4),
            args: vec![v(2)],
        },
    };
    let bb4 = ArcBlock {
        id: ArcBlockId::new(4),
        params: vec![(v(3), ty)],
        body: Vec::new(),
        terminator: ArcTerminator::Return { value: v(1) },
    };
    ArcFunction {
        var_types: (0..6).map(|_| ty).collect(),
        blocks: vec![bb0, bb1, bb2, bb3, bb4],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

/// The lineage's RC-carrying vars: root + threaded loop param + terminal param.
fn owned_lineage() -> FxHashSet<ArcVarId> {
    [ArcVarId::new(0), ArcVarId::new(2), ArcVarId::new(3)]
        .into_iter()
        .collect()
}

fn run(func: &ArcFunction, owned: &FxHashSet<ArcVarId>) -> FxHashMap<usize, Vec<ArcVarId>> {
    compute_loop_invariant_dead_local_releases(func, owned)
}

/// Walking skeleton (semantic pin): the canonical purely-dead list shape emits
/// EXACTLY ONE release at the terminal dead block-param's block entry.
/// Reverting the scan drops this release -> the loop-invariant buffer leak.
#[test]
fn purely_dead_loop_invariant_list_releases_at_terminal_dead_param() {
    let func = loop_invariant_func(CtorKind::ListLiteral);
    let releases = run(&func, &owned_lineage());

    assert_eq!(
        releases.get(&4).map(Vec::as_slice),
        Some([ArcVarId::new(3)].as_slice()),
        "purely-dead loop-invariant list must release ONCE at the terminal dead param"
    );
    assert_eq!(releases.len(), 1, "exactly one release block expected");
}

/// Matrix (type rows at the scan's grain): every collection-literal ctor is
/// admitted; every non-collection ctor declines. The count assertion proves
/// no cell was skipped.
#[test]
fn root_ctor_matrix_admits_collections_and_declines_aggregates() {
    let admitted = [
        CtorKind::ListLiteral,
        CtorKind::MapLiteral,
        CtorKind::SetLiteral,
    ];
    let declined = [
        CtorKind::Tuple,
        CtorKind::Struct(Name::from_raw(9)),
        CtorKind::EnumVariant {
            enum_name: Name::from_raw(9),
            variant: 0,
        },
        CtorKind::Closure {
            func: Name::from_raw(9),
        },
    ];
    let mut cells = 0;
    for ctor in admitted {
        let func = loop_invariant_func(ctor);
        assert_eq!(
            run(&func, &owned_lineage()).len(),
            1,
            "collection-literal root must be admitted"
        );
        cells += 1;
    }
    for ctor in declined {
        let func = loop_invariant_func(ctor);
        assert!(
            run(&func, &owned_lineage()).is_empty(),
            "non-collection root must decline"
        );
        cells += 1;
    }
    assert_eq!(cells, 7, "every ctor matrix cell must be visited");
}

/// Borrow-only sibling (semantic pin): a threaded param whose ONLY non-thread
/// use is a borrowed call arg (via a `Let`-Var alias) still releases ONCE at
/// the terminal dead param — the borrow-read does not consume the buffer.
#[test]
fn borrow_only_loop_read_releases_at_terminal_dead_param() {
    let v = ArcVarId::new;
    let ty = Idx::from_raw(1);
    let mut func = loop_invariant_func(CtorKind::ListLiteral);
    func.blocks[2].body = vec![
        ArcInstr::Let {
            dst: v(4),
            ty,
            value: ArcValue::Var(v(2)),
        },
        ArcInstr::Apply {
            dst: v(5),
            ty: Idx::BOOL,
            func: Name::from_raw(1),
            args: vec![v(4)],
            arg_ownership: vec![crate::ir::ArgOwnership::Borrowed],
            mono_instance_id: None,
        },
    ];
    let releases = run(&func, &owned_lineage());

    assert_eq!(
        releases.get(&4).map(Vec::as_slice),
        Some([ArcVarId::new(3)].as_slice()),
        "borrow-only loop-invariant lineage must release ONCE at the terminal dead param"
    );
    assert_eq!(releases.len(), 1, "exactly one release block expected");
}

/// Negative pin (no dec on a consumed lineage): the same alias consumed at an
/// OWNED call-arg position is a real transfer-out — the scan must decline
/// (emitting the RL-5 dec would double-free the transferred buffer).
#[test]
fn owned_position_consume_declines() {
    let v = ArcVarId::new;
    let ty = Idx::from_raw(1);
    let mut func = loop_invariant_func(CtorKind::ListLiteral);
    func.blocks[2].body = vec![
        ArcInstr::Let {
            dst: v(4),
            ty,
            value: ArcValue::Var(v(2)),
        },
        ArcInstr::Apply {
            dst: v(5),
            ty: Idx::BOOL,
            func: Name::from_raw(1),
            args: vec![v(4)],
            arg_ownership: vec![crate::ir::ArgOwnership::Owned],
            mono_instance_id: None,
        },
    ];
    assert!(
        run(&func, &owned_lineage()).is_empty(),
        "an owned-position consume is a transfer-out; the scan must decline"
    );
}

/// Negative pin: a member read as a value (a `Project` off the threaded param)
/// is outside BOTH families — the value has a real last use the base walk
/// releases; an RL-5 dec on top would double-free.
#[test]
fn member_value_read_declines() {
    let v = ArcVarId::new;
    let mut func = loop_invariant_func(CtorKind::ListLiteral);
    func.blocks[2].body = vec![ArcInstr::Project {
        dst: v(4),
        ty: Idx::from_raw(1),
        value: v(2),
        field: 0,
    }];
    assert!(
        run(&func, &owned_lineage()).is_empty(),
        "a value-read member must decline both families"
    );
}

/// Negative pin (closure gate): a loop slot REASSIGNED per iteration — the
/// back-edge feeds a fresh non-member `Construct` into the member param — is
/// NOT a single loop-invariant allocation; the borrow-only family must
/// decline (each fresh value's release is the base walk's job).
#[test]
fn reassigned_loop_slot_declines_borrow_only() {
    let v = ArcVarId::new;
    let ty = Idx::from_raw(1);
    let mut func = loop_invariant_func(CtorKind::ListLiteral);
    func.blocks[2].body = vec![
        // The borrow-read engaging the borrow-only family.
        ArcInstr::Let {
            dst: v(4),
            ty,
            value: ArcValue::Var(v(2)),
        },
        ArcInstr::Apply {
            dst: v(5),
            ty: Idx::BOOL,
            func: Name::from_raw(1),
            args: vec![v(4)],
            arg_ownership: vec![crate::ir::ArgOwnership::Borrowed],
            mono_instance_id: None,
        },
        // The per-iteration reassignment (a fresh non-member feeder).
        ArcInstr::Construct {
            dst: v(6),
            ty,
            ctor: CtorKind::ListLiteral,
            args: Vec::new(),
        },
    ];
    func.blocks[2].terminator = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![v(6)],
    };
    func.var_types.push(ty);
    assert!(
        run(&func, &owned_lineage()).is_empty(),
        "a reassigned loop slot fractures loop-invariance; the closure gate must decline"
    );
}

/// Negative pin (no inc-only split analog): a fork delivering the lineage to
/// TWO terminal dead params needs more than one release — outside this
/// single-release family; the scan must decline rather than release one arm.
#[test]
fn two_terminal_dead_params_decline() {
    let v = ArcVarId::new;
    let ty = Idx::from_raw(1);
    let mut func = loop_invariant_func(CtorKind::ListLiteral);
    func.blocks[3].terminator = ArcTerminator::Jump {
        target: ArcBlockId::new(4),
        args: vec![v(2), v(2)],
    };
    func.blocks[4].params = vec![(v(3), ty), (v(4), ty)];
    let mut owned = owned_lineage();
    owned.insert(v(4));
    assert!(
        run(&func, &owned).is_empty(),
        "more than one terminal dead param must decline the single-release family"
    );
}

/// Negative pin: a terminal param that carries no RC (not in
/// `owned_vars_needing_rc`) owes no release — the scan must decline.
#[test]
fn terminal_param_without_rc_declines() {
    let func = loop_invariant_func(CtorKind::ListLiteral);
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0), ArcVarId::new(2)].into_iter().collect();
    assert!(
        run(&func, &owned).is_empty(),
        "an RC-free terminal param must decline"
    );
}

/// Negative pin: a root whose dst carries no RC (scalar-element shape the
/// burden admission already excluded) never seeds a lineage.
#[test]
fn root_without_rc_declines() {
    let func = loop_invariant_func(CtorKind::ListLiteral);
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(2), ArcVarId::new(3)].into_iter().collect();
    assert!(
        run(&func, &owned).is_empty(),
        "an RC-free Construct dst must not seed a lineage"
    );
}

/// Negative pin: a member escaping through a non-Jump terminator (`Return`)
/// is a transfer-out, not a dead thread — the scan must decline.
#[test]
fn member_returned_declines() {
    let v = ArcVarId::new;
    let mut func = loop_invariant_func(CtorKind::ListLiteral);
    func.blocks[4].terminator = ArcTerminator::Return { value: v(3) };
    assert!(
        run(&func, &owned_lineage()).is_empty(),
        "a returned member is a transfer-out; the scan must decline"
    );
}
