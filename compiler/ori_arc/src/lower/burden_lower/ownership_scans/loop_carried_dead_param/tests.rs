//! Walking-skeleton pins for the RL-5 loop-carried dead-param scans: the
//! closure-scan admission (fresh `PartialApply` threaded through a loop,
//! borrow-used, dead at the post-loop param) and the shared skeleton's gate
//! declines (not-loop-carried, root not RC-tracked, repr mismatch), plus the
//! `ori_list_take` collection-scan admission.

use ori_ir::{Name, StringInterner};
use ori_types::Idx;
use rustc_hash::FxHashSet;

use super::{
    compute_loop_carried_dead_collection_param_lineage, compute_loop_closure_dead_param_lineage,
};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue,
    ValueRepr,
};
use crate::lower::burden_lower::ownership_scans::ForwarderReleasePos;

fn vv(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn func(reprs: Vec<ValueRepr>, blocks: Vec<ArcBlock>) -> ArcFunction {
    ArcFunction {
        var_types: (0..u32::try_from(reprs.len()).unwrap_or(u32::MAX))
            .map(|i| Idx::from_raw(i + 1))
            .collect(),
        var_reprs: reprs,
        blocks,
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

fn block(id: u32, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body,
        terminator,
    }
}

fn block_with_param(
    id: u32,
    param: u32,
    body: Vec<ArcInstr>,
    terminator: ArcTerminator,
) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: vec![(vv(param), Idx::from_raw(80))],
        body,
        terminator,
    }
}

fn jump(target: u32, args: Vec<ArcVarId>) -> ArcTerminator {
    ArcTerminator::Jump {
        target: ArcBlockId::new(target),
        args,
    }
}

fn int_literal(dst: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: vv(dst),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(0)),
    }
}

/// The canonical admission CFG: b0 mints the root and jumps into the loop
/// header b1 (param = the carried lineage var); b1 borrow-uses it via an
/// `ApplyIndirect` receiver and branches to the loop body b2 (back-edge to b1
/// re-threading the member) or the pre-exit b3, which forwards the member to
/// the dead post-loop param in b4.
///
/// Vars: v0 root (def in b0), v1 loop param (member), v2 call result (scalar),
/// v3 branch cond (scalar), v4 dead post-loop param (member), v5 return value.
fn loop_fixture(root_instr: ArcInstr, member_repr: ValueRepr) -> ArcFunction {
    func(
        vec![
            member_repr, // v0 root
            member_repr, // v1 loop param
            ValueRepr::Scalar,
            ValueRepr::Scalar,
            member_repr, // v4 dead post-loop param
            ValueRepr::Scalar,
        ],
        vec![
            block(0, vec![root_instr], jump(1, vec![vv(0)])),
            block_with_param(
                1,
                1,
                vec![
                    ArcInstr::ApplyIndirect {
                        dst: vv(2),
                        ty: Idx::INT,
                        closure: vv(1),
                        args: vec![],
                        arg_ownership: vec![],
                    },
                    int_literal(3),
                ],
                ArcTerminator::Branch {
                    cond: vv(3),
                    then_block: ArcBlockId::new(2),
                    else_block: ArcBlockId::new(3),
                },
            ),
            block(2, vec![], jump(1, vec![vv(1)])), // back-edge: the loop carry
            block(3, vec![], jump(4, vec![vv(1)])), // forward exit edge
            block_with_param(
                4,
                4,
                vec![int_literal(5)],
                ArcTerminator::Return { value: vv(5) },
            ),
        ],
    )
}

fn partial_apply_root() -> ArcInstr {
    ArcInstr::PartialApply {
        dst: vv(0),
        ty: Idx::from_raw(80),
        func: Name::from_raw(40),
        args: vec![],
    }
}

#[test]
fn loop_carried_closure_dead_param_admits() {
    let f = loop_fixture(partial_apply_root(), ValueRepr::FatValue);
    let owned: FxHashSet<ArcVarId> = [vv(0)].into_iter().collect();
    let out = compute_loop_closure_dead_param_lineage(&f, &owned);
    assert!(
        out.suppressed_lineage_vars.contains(&vv(0))
            && out.suppressed_lineage_vars.contains(&vv(1))
            && out.suppressed_lineage_vars.contains(&vv(4)),
        "the whole same-alloc lineage is suppressed: {:?}",
        out.suppressed_lineage_vars
    );
    let releases = out
        .releases
        .get(&(4, ForwarderReleasePos::BlockEntry))
        .unwrap_or_else(|| panic!("no RL-5 release at the dead post-loop param entry"));
    assert_eq!(releases, &vec![vv(4)]);
}

#[test]
fn not_loop_carried_declines() {
    // Same shape but the "loop body" b2 falls through forward (no back-edge):
    // gate (d) declines and nothing is suppressed or placed.
    let mut f = loop_fixture(partial_apply_root(), ValueRepr::FatValue);
    f.blocks[2].terminator = jump(3, vec![]);
    let owned: FxHashSet<ArcVarId> = [vv(0)].into_iter().collect();
    let out = compute_loop_closure_dead_param_lineage(&f, &owned);
    assert!(out.suppressed_lineage_vars.is_empty());
    assert!(out.releases.is_empty());
}

#[test]
fn root_not_rc_tracked_declines() {
    let f = loop_fixture(partial_apply_root(), ValueRepr::FatValue);
    let owned: FxHashSet<ArcVarId> = FxHashSet::default();
    let out = compute_loop_closure_dead_param_lineage(&f, &owned);
    assert!(out.suppressed_lineage_vars.is_empty());
    assert!(out.releases.is_empty());
}

#[test]
fn closure_repr_mismatch_declines() {
    // Gate (b): the closure scan requires FatValue; an RcPointer root declines.
    let f = loop_fixture(partial_apply_root(), ValueRepr::RcPointer);
    let owned: FxHashSet<ArcVarId> = [vv(0)].into_iter().collect();
    let out = compute_loop_closure_dead_param_lineage(&f, &owned);
    assert!(out.suppressed_lineage_vars.is_empty());
}

#[test]
fn loop_carried_list_take_collection_admits() {
    let interner = StringInterner::new();
    let list_take = interner.intern("ori_list_take");
    let root = ArcInstr::Apply {
        dst: vv(0),
        ty: Idx::from_raw(80),
        func: list_take,
        args: vec![],
        arg_ownership: vec![],
        mono_instance_id: None,
    };
    let f = loop_fixture(root, ValueRepr::RcPointer);
    // The collection vet rejects an ApplyIndirect borrow-receiver (that is the
    // closure scan's shape) — replace b1's body use with a wholly-dead thread
    // (the collection vet admits never-used members).
    let mut f = f;
    f.blocks[1].body = vec![int_literal(3)];
    let owned: FxHashSet<ArcVarId> = [vv(0)].into_iter().collect();
    let out = compute_loop_carried_dead_collection_param_lineage(&f, &owned, &interner);
    assert!(
        out.suppressed_lineage_vars.contains(&vv(0)),
        "list_take root lineage suppressed: {:?}",
        out.suppressed_lineage_vars
    );
    let releases = out
        .releases
        .get(&(4, ForwarderReleasePos::BlockEntry))
        .unwrap_or_else(|| panic!("no RL-5 release at the dead post-loop param entry"));
    assert_eq!(releases, &vec![vv(4)]);
}
