//! Walking-skeleton pins for the `ori_catch_recover` recovered-message release
//! scan: admission (recover result borrow-read with a MISSING normal-path
//! release), and the gate declines (root not RC-tracked, `Return` transfer,
//! an already-present normal-path release).

use ori_ir::{Name, StringInterner};
use ori_types::Idx;
use rustc_hash::FxHashSet;

use super::compute_catch_recover_release_lineage;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership,
    LitValue, ValueRepr,
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

fn recover_root(interner: &StringInterner, dst: u32) -> ArcInstr {
    ArcInstr::Apply {
        dst: vv(dst),
        ty: Idx::from_raw(90),
        func: interner.intern("ori_catch_recover"),
        args: vec![],
        arg_ownership: vec![],
        mono_instance_id: None,
    }
}

fn borrowed_scalar_read(dst: u32, arg: u32) -> ArcInstr {
    ArcInstr::Apply {
        dst: vv(dst),
        ty: Idx::INT,
        func: Name::from_raw(60),
        args: vec![vv(arg)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    }
}

fn int_literal(dst: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: vv(dst),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(0)),
    }
}

#[test]
fn recover_result_borrow_read_missing_release_admits() {
    // v0 = ori_catch_recover(); v1 = contains(v0 [borrow]) -> scalar;
    // return v2 (scalar). No dec anywhere: the missing-normal-path-release
    // shape — the scan suppresses the lineage and places the single dec
    // AFTER the final genuine borrow-read.
    let interner = StringInterner::new();
    let f = func(
        vec![ValueRepr::RcPointer, ValueRepr::Scalar, ValueRepr::Scalar],
        vec![block(
            0,
            vec![
                recover_root(&interner, 0),
                borrowed_scalar_read(1, 0),
                int_literal(2),
            ],
            ArcTerminator::Return { value: vv(2) },
        )],
    );
    let owned: FxHashSet<ArcVarId> = [vv(0)].into_iter().collect();
    let out = compute_catch_recover_release_lineage(&f, &owned, &interner);
    assert!(
        out.suppressed_lineage_vars.contains(&vv(0)),
        "the recovered-message lineage is suppressed: {:?}",
        out.suppressed_lineage_vars
    );
    let releases = out
        .releases
        .get(&(0, ForwarderReleasePos::AfterInstr(1)))
        .unwrap_or_else(|| panic!("no dec after the final genuine borrow-read"));
    assert_eq!(releases, &vec![vv(0)]);
}

#[test]
fn root_not_rc_tracked_declines() {
    let interner = StringInterner::new();
    let f = func(
        vec![ValueRepr::RcPointer, ValueRepr::Scalar, ValueRepr::Scalar],
        vec![block(
            0,
            vec![
                recover_root(&interner, 0),
                borrowed_scalar_read(1, 0),
                int_literal(2),
            ],
            ArcTerminator::Return { value: vv(2) },
        )],
    );
    let owned: FxHashSet<ArcVarId> = FxHashSet::default();
    let out = compute_catch_recover_release_lineage(&f, &owned, &interner);
    assert!(out.suppressed_lineage_vars.is_empty());
    assert!(out.releases.is_empty());
}

#[test]
fn return_transfer_declines() {
    // A member reaching `Return` is an owned transfer out of the family —
    // gate (c) declines.
    let interner = StringInterner::new();
    let f = func(
        vec![ValueRepr::RcPointer],
        vec![block(
            0,
            vec![recover_root(&interner, 0)],
            ArcTerminator::Return { value: vv(0) },
        )],
    );
    let owned: FxHashSet<ArcVarId> = [vv(0)].into_iter().collect();
    let out = compute_catch_recover_release_lineage(&f, &owned, &interner);
    assert!(out.suppressed_lineage_vars.is_empty());
    assert!(out.releases.is_empty());
}

#[test]
fn existing_normal_path_release_declines() {
    // Gate (d): a BurdenDec forward-reachable to a Return-terminated normal
    // exit means the lineage is already balanced — no second release.
    let interner = StringInterner::new();
    let f = func(
        vec![ValueRepr::RcPointer, ValueRepr::Scalar, ValueRepr::Scalar],
        vec![block(
            0,
            vec![
                recover_root(&interner, 0),
                borrowed_scalar_read(1, 0),
                ArcInstr::BurdenDec { var: vv(0) },
                int_literal(2),
            ],
            ArcTerminator::Return { value: vv(2) },
        )],
    );
    let owned: FxHashSet<ArcVarId> = [vv(0)].into_iter().collect();
    let out = compute_catch_recover_release_lineage(&f, &owned, &interner);
    assert!(out.suppressed_lineage_vars.is_empty());
    assert!(out.releases.is_empty());
}
