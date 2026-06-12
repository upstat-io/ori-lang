//! Tests for the post-emission RC-incremented variable tracking.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashSet;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, RcAtomicity,
    RcStrategy,
};

use super::collect_rc_incremented_vars;

fn alias_of(dst: u32, src: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: ArcVarId::new(dst),
        ty: Idx::STR,
        value: ArcValue::Var(ArcVarId::new(src)),
    }
}

fn rc_inc_of(var: u32) -> ArcInstr {
    ArcInstr::RcInc {
        var: ArcVarId::new(var),
        count: 1,
        strategy: RcStrategy::HeapPointer,
        atomicity: RcAtomicity::default_atomic(),
    }
}

fn one_block_func(n_vars: u32, body: Vec<ArcInstr>) -> ArcFunction {
    ArcFunction {
        var_types: (0..n_vars).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

#[test]
fn rc_incremented_propagates_forward_through_alias_chain() {
    // RcInc on the root %0; aliases %1 = %0 and %2 = %1 share the object.
    let func = one_block_func(3, vec![rc_inc_of(0), alias_of(1, 0), alias_of(2, 1)]);
    let set = collect_rc_incremented_vars(&func);
    let expected: FxHashSet<ArcVarId> = (0..3).map(ArcVarId::new).collect();
    assert_eq!(
        set, expected,
        "an inc on the root marks every alias of the same object"
    );
}

#[test]
fn rc_incremented_propagates_backward_from_alias_to_root_and_siblings() {
    // The kept duplication-alias inc shape: %1 = %0 (inc'd), %2 = %0 (sibling).
    // The inc on %1 bumps the SHARED object's refcount — %0 and %2 are
    // physically incremented too; classifying them un-incremented would
    // promote a later COW site to StaticUnique and mutate the shared buffer
    // in place (the store-then-`.set` holder-view corruption).
    let func = one_block_func(3, vec![alias_of(1, 0), rc_inc_of(1), alias_of(2, 0)]);
    let set = collect_rc_incremented_vars(&func);
    let expected: FxHashSet<ArcVarId> = (0..3).map(ArcVarId::new).collect();
    assert_eq!(
        set, expected,
        "an inc on a duplication alias marks the root and every sibling alias"
    );
}

#[test]
fn rc_incremented_empty_without_incs() {
    let func = one_block_func(3, vec![alias_of(1, 0), alias_of(2, 0)]);
    assert!(
        collect_rc_incremented_vars(&func).is_empty(),
        "no RcInc anywhere — no member is physically incremented"
    );
}
