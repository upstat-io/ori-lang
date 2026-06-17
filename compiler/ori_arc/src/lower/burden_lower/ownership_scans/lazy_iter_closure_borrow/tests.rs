//! Unit pins for the RL-2 lazy-iterator closure-borrow scan
//! `compute_lazy_iter_closure_borrow_lineage`.
//!
//! Positive pin — the leak signature (`xs.iter().map(f).collect()` with a fresh
//! `PartialApply` closure `f` borrowed into `@map`) suppresses the closure
//! lineage and places exactly one `BurdenDec(closure)` at the terminal `@collect`
//! consumer's normal-successor entry. Negative pins — `@fold` (eager) declines;
//! an owned-position closure consume declines. Spec: Annex E §AIMS RL-2.

use ori_ir::StringInterner;
use ori_types::Idx;
use rustc_hash::FxHashSet;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr,
};

use super::super::forwarder_release::ForwarderReleasePos;
use super::compute_lazy_iter_closure_borrow_lineage;

/// A bodyless block with the given ID and terminator (the trivial return/resume
/// tail blocks of the test fixture).
fn empty_block(id: u32, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body: Vec::new(),
        terminator,
    }
}

/// Build the lazy-HOF leak shape across 5 blocks (each may-unwind call in its
/// own block, mirroring the real ARC IR). Block IDs equal vec positions (the
/// realization-IR invariant the release-map `block_idx` keys rely on):
/// ```text
/// bb0: %0 = "captured"; %2 = PartialApply @lambda(%0); %7 = Construct List;
///      %9 = %7; %10 = Invoke @iter(%9 [own]) normal bb1 unwind bb4
/// bb1: %11 = %2;  %12 = Invoke @<builtin>(%10 [own], %11 [borrow]) normal bb2 unwind bb4
/// bb2: %13 = Invoke @collect(%12 [own]) normal bb3 unwind bb4
/// bb3: Return %13
/// bb4: Resume
/// ```
/// `builtin` is parameterized (`map`/`filter`/`fold`). Closure root = `%2`,
/// alias `%11`. `var_reprs[2]` = `FatValue` (closure two-word value).
fn lazy_hof_func(
    interner: &mut StringInterner,
    builtin: &str,
    closure_owned_at_map: bool,
) -> ArcFunction {
    let v = ArcVarId::new;
    let iter_name = interner.intern("iter");
    let builtin_name = interner.intern(builtin);
    let collect_name = interner.intern("collect");

    let map_ownership = if closure_owned_at_map {
        vec![
            crate::ir::ArgOwnership::Owned,
            crate::ir::ArgOwnership::Owned,
        ]
    } else {
        vec![
            crate::ir::ArgOwnership::Owned,
            crate::ir::ArgOwnership::Borrowed,
        ]
    };

    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: v(0),
                ty: Idx::from_raw(1),
                value: ArcValue::Literal(crate::ir::LitValue::String(interner.intern("captured"))),
            },
            ArcInstr::PartialApply {
                dst: v(2),
                ty: Idx::from_raw(2),
                func: interner.intern("lambda"),
                args: vec![v(0)],
            },
            ArcInstr::Construct {
                dst: v(7),
                ty: Idx::from_raw(3),
                ctor: crate::ir::CtorKind::ListLiteral,
                args: Vec::new(),
            },
            ArcInstr::Let {
                dst: v(9),
                ty: Idx::from_raw(3),
                value: ArcValue::Var(v(7)),
            },
        ],
        terminator: ArcTerminator::Invoke {
            dst: v(10),
            ty: Idx::from_raw(4),
            func: iter_name,
            args: vec![v(9)],
            arg_ownership: vec![crate::ir::ArgOwnership::Owned],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(4),
            mono_instance_id: None,
        },
    };
    let bb1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v(11),
            ty: Idx::from_raw(2),
            value: ArcValue::Var(v(2)),
        }],
        terminator: ArcTerminator::Invoke {
            dst: v(12),
            ty: Idx::from_raw(4),
            func: builtin_name,
            args: vec![v(10), v(11)],
            arg_ownership: map_ownership,
            normal: ArcBlockId::new(2),
            unwind: ArcBlockId::new(4),
            mono_instance_id: None,
        },
    };
    let bb3 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Invoke {
            dst: v(13),
            ty: Idx::from_raw(3),
            func: collect_name,
            args: vec![v(12)],
            arg_ownership: vec![crate::ir::ArgOwnership::Owned],
            normal: ArcBlockId::new(3),
            unwind: ArcBlockId::new(4),
            mono_instance_id: None,
        },
    };
    let bb5 = empty_block(3, ArcTerminator::Return { value: v(13) });
    let bb6 = empty_block(4, ArcTerminator::Resume);

    let mut var_reprs = vec![ValueRepr::RcPointer; 14];
    var_reprs[2] = ValueRepr::FatValue; // closure root
    var_reprs[11] = ValueRepr::FatValue; // closure alias

    ArcFunction {
        var_types: (0..14).map(|i| Idx::from_raw(i + 1)).collect(),
        var_reprs,
        blocks: vec![bb0, bb1, bb3, bb5, bb6],
        entry: ArcBlockId::new(0),
        name: interner.intern("main"),
        ..ArcFunction::default()
    }
}

/// Positive pin: `xs.iter().map(f).collect()` — the fresh closure `%2` (and alias
/// `%11`) borrowed into `@map`, chain terminated by `@collect`. The scan suppresses
/// the lineage `{%2, %11}` and places exactly one `BurdenDec(%2)` at `@collect`'s
/// normal successor (bb5) entry. Reverting drops this release → the early
/// borrowed-arg dec frees the closure before `@collect` runs it (UAF).
#[test]
fn lazy_iter_map_suppresses_lineage_and_releases_after_collect() {
    let mut interner = StringInterner::new();
    let func = lazy_hof_func(&mut interner, "map", false);
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(2), ArcVarId::new(11)].into_iter().collect();

    let out = compute_lazy_iter_closure_borrow_lineage(&func, &owned, &interner);

    assert!(
        out.suppressed_lineage_vars.contains(&ArcVarId::new(2)),
        "closure root must be suppressed from owned_vars_needing_rc"
    );
    assert!(
        out.suppressed_lineage_vars.contains(&ArcVarId::new(11)),
        "closure alias must be suppressed from owned_vars_needing_rc"
    );
    // collect is in bb3 (index 2 in blocks vec), normal successor = bb5 (index 3).
    let key = (3usize, ForwarderReleasePos::BlockEntry);
    assert_eq!(
        out.releases.get(&key).map(Vec::as_slice),
        Some([ArcVarId::new(2)].as_slice()),
        "exactly one BurdenDec(closure root) at the terminal consumer's normal successor"
    );
    assert_eq!(out.releases.len(), 1, "exactly one placed-release entry");
}

/// Positive pin: `@filter` behaves identically to `@map` (both are lazy).
#[test]
fn lazy_iter_filter_suppresses_and_releases() {
    let mut interner = StringInterner::new();
    let func = lazy_hof_func(&mut interner, "filter", false);
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(2), ArcVarId::new(11)].into_iter().collect();

    let out = compute_lazy_iter_closure_borrow_lineage(&func, &owned, &interner);

    assert!(out.suppressed_lineage_vars.contains(&ArcVarId::new(2)));
    assert_eq!(
        out.releases.len(),
        1,
        "filter must place one release like map"
    );
}

/// Negative pin (eager guard): `@fold` is EAGER — it runs the closure
/// synchronously, so the base walk's early dec is harmless. `@fold` is NOT in
/// `LAZY_CLOSURE_BUILTINS`, so gate (d) finds no lazy-builtin site → decline.
/// Emitting a deferred release here would be wrong (the closure is already
/// released correctly by the base walk).
#[test]
fn lazy_iter_fold_declines() {
    let mut interner = StringInterner::new();
    let func = lazy_hof_func(&mut interner, "fold", false);
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(2), ArcVarId::new(11)].into_iter().collect();

    let out = compute_lazy_iter_closure_borrow_lineage(&func, &owned, &interner);

    assert!(
        out.suppressed_lineage_vars.is_empty(),
        "eager @fold must decline — no lazy-builtin borrowed-arg site"
    );
    assert!(out.releases.is_empty(), "no release placed for eager fold");
}

/// Negative pin (owned-consume guard): when the closure is passed at an OWNED
/// position to `@map` (the runtime would consume it), the borrow-only vet's
/// lazy-site detection requires a BORROWED position — gate (d) finds no borrowed
/// lazy site → decline. (The owned-consume shape is a different ownership
/// contract the scan must not touch.)
#[test]
fn lazy_iter_owned_closure_consume_declines() {
    let mut interner = StringInterner::new();
    let func = lazy_hof_func(&mut interner, "map", true);
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(2), ArcVarId::new(11)].into_iter().collect();

    let out = compute_lazy_iter_closure_borrow_lineage(&func, &owned, &interner);

    assert!(
        out.suppressed_lineage_vars.is_empty(),
        "owned-position closure consume must decline (no borrowed lazy site)"
    );
}
