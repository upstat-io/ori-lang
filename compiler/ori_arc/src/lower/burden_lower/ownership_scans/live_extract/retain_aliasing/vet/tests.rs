//! Pins for the gate (d) scalar-decline vet: an accessor-retain member
//! (`retain_aliasing_closure_vetted` closure growth) feeding a FURTHER
//! borrowed read whose result is neither a tracked member nor provably
//! scalar must decline admission (Spec: Annex E §AIMS RL-2 + TF-4).

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashSet;

use super::retain_aliasing_closure_vetted;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArgOwnership, ValueRepr,
};

fn vv(n: u32) -> crate::ir::ArcVarId {
    crate::ir::ArcVarId::new(n)
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

fn borrowed_apply(dst: u32, callee: Name, arg: u32) -> ArcInstr {
    ArcInstr::Apply {
        dst: vv(dst),
        ty: Idx::INT,
        func: callee,
        args: vec![vv(arg)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    }
}

/// v1 = accessor(v0 [borrow]) grows the closure to {v0, v1} (accessor-retain,
/// non-scalar result). v2 = other(v1 [borrow]) is a FURTHER borrowed read
/// whose result v2 is neither a closure member nor provably scalar (an
/// untracked co-owning view, e.g. `.substring(..)`) — the gate (d) vet
/// MUST decline admission (return `None`).
#[test]
fn further_read_nonmember_nonscalar_result_declines() {
    let accessor = Name::from_raw(50);
    let other = Name::from_raw(60);
    let mut accessor_retain_names = FxHashSet::default();
    accessor_retain_names.insert(accessor);

    let f = func(
        vec![
            ValueRepr::Aggregate, // v0: root (niche-sum payload wrapper)
            ValueRepr::RcPointer, // v1: accessor-retain result (non-scalar)
            ValueRepr::RcPointer, // v2: untracked sharing-view result (non-scalar)
        ],
        vec![block(
            0,
            vec![borrowed_apply(1, accessor, 0), borrowed_apply(2, other, 1)],
            ArcTerminator::Return { value: vv(2) },
        )],
    );

    let result = retain_aliasing_closure_vetted(&f, vv(0), &accessor_retain_names);
    assert!(
        result.is_none(),
        "a further borrowed read producing an untracked non-member, non-scalar \
         result must decline gate (d) admission: {result:?}"
    );
}

/// Same shape, but the FURTHER read's result (v2) is provably SCALAR
/// (e.g. `.starts_with(..)` -> `bool`) — the pre-existing, unaffected path:
/// admission still succeeds since a scalar result carries no untracked heap
/// alias.
#[test]
fn further_read_nonmember_scalar_result_admits() {
    let accessor = Name::from_raw(50);
    let other = Name::from_raw(60);
    let mut accessor_retain_names = FxHashSet::default();
    accessor_retain_names.insert(accessor);

    let f = func(
        vec![
            ValueRepr::Aggregate, // v0: root
            ValueRepr::RcPointer, // v1: accessor-retain result (non-scalar)
            ValueRepr::Scalar,    // v2: scalar-result borrowed read (e.g. bool)
        ],
        vec![block(
            0,
            vec![borrowed_apply(1, accessor, 0), borrowed_apply(2, other, 1)],
            ArcTerminator::Return { value: vv(2) },
        )],
    );

    let result = retain_aliasing_closure_vetted(&f, vv(0), &accessor_retain_names);
    let admits = result
        .as_ref()
        .is_some_and(|members| members.contains(&vv(0)) && members.contains(&vv(1)));
    assert!(
        admits,
        "a further borrowed read whose result is provably scalar carries no \
         untracked heap alias and must still admit: {result:?}"
    );
}
