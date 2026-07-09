//! Unit pins for the RL-2 mutable-rebind release scan
//! `compute_reassign_rebind_releases`. Positive pin — the leak signature
//! (kept fresh-site inc + suppressed terminal dec) emits exactly one
//! `BurdenDec(old_var)` at the rebind. Negative pins — a genuinely-transferred
//! binding value (inc suppressed) and a full-move binding value emit NOTHING
//! (the double-free guard). Spec: Annex E §AIMS RL-2.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashSet;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr,
};

use super::super::forwarder_release::ForwarderReleasePos;
use super::compute_reassign_rebind_releases;

/// Build a 2-block function modeling the self-referential rebind shape:
/// ```text
/// bb0: %0 = Construct List; %1 = %0 (updated receiver); %2 = %0 (index-read);
///      Invoke @updated(%1 [own], %2 [borrow]) normal bb1 unwind bb2
/// bb1: %3 = %4 (the rebind: xs := updated-result)   <- reassign death recorded
///      Return %3
/// bb2: Resume
/// ```
/// `reassign_deaths = [(old=%0, new=%3)]`. `%4` is the Invoke dst.
fn rebind_func() -> ArcFunction {
    let v = ArcVarId::new;
    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Construct {
                dst: v(0),
                ty: Idx::from_raw(1),
                ctor: crate::ir::CtorKind::ListLiteral,
                args: Vec::new(),
            },
            ArcInstr::Let {
                dst: v(1),
                ty: Idx::from_raw(1),
                value: ArcValue::Var(v(0)),
            },
            ArcInstr::Let {
                dst: v(2),
                ty: Idx::from_raw(1),
                value: ArcValue::Var(v(0)),
            },
        ],
        terminator: ArcTerminator::Invoke {
            dst: v(4),
            ty: Idx::from_raw(1),
            func: Name::from_raw(1),
            args: vec![v(1), v(2)],
            arg_ownership: vec![
                crate::ir::ArgOwnership::Owned,
                crate::ir::ArgOwnership::Borrowed,
            ],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
            mono_instance_id: None,
        },
    };
    let bb1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        // The rebind `Let { dst: %3, value: Var(%4) }` — the new binding value.
        body: vec![ArcInstr::Let {
            dst: v(3),
            ty: Idx::from_raw(1),
            value: ArcValue::Var(v(4)),
        }],
        terminator: ArcTerminator::Return { value: v(3) },
    };
    let bb2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Resume,
    };
    ArcFunction {
        var_types: (0..5).map(|i| Idx::from_raw(i + 1)).collect(),
        // `%0` (the binding's prior value) is a whole-value heap `RcPointer`
        // (the `[int]` list the leak shape rebinds); the rest are immaterial to
        // the scan's repr gate.
        var_reprs: vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
        ],
        blocks: vec![bb0, bb1, bb2],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        reassign_deaths: vec![(v(0), v(3))],
        ..ArcFunction::default()
    }
}

/// Positive pin: the leak signature (`old_var` owned-RC, terminal dec suppressed
/// via `transfer_via_move_alias`, fresh inc kept (∉ `inc_suppressed`), not a full
/// move) emits exactly one `BurdenDec(%0)` at the rebind `Let` (`%3`'s def in
/// bb1, AfterInstr(0)). Reverting the scan drops this release → the +1 leak.
#[test]
fn reassign_release_emits_dec_for_kept_inc_suppressed_dec() {
    let func = rebind_func();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let move_alias: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let inc_suppressed: FxHashSet<ArcVarId> = FxHashSet::default();
    let full_move: FxHashSet<ArcVarId> = FxHashSet::default();

    let releases = compute_reassign_rebind_releases(
        &func,
        &owned,
        &move_alias,
        &inc_suppressed,
        &full_move,
        &FxHashSet::default(),
    );

    let key = (1usize, ForwarderReleasePos::AfterInstr(0));
    assert_eq!(
        releases.get(&key).map(Vec::as_slice),
        Some([ArcVarId::new(0)].as_slice()),
        "leak-signature rebind must place exactly one BurdenDec(old_var) at the rebind Let"
    );
    assert_eq!(
        releases.len(),
        1,
        "exactly one placed-release entry expected"
    );
}

/// Negative pin (double-free guard): a genuinely-transferred binding value has
/// its FRESH-site inc SUPPRESSED (`old_var ∈ inc_suppressed_vars`), so the
/// rebind release MUST NOT fire — emitting one would double-free the value the
/// transfer already discharged.
#[test]
fn reassign_release_no_dec_when_inc_suppressed() {
    let func = rebind_func();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let move_alias: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let inc_suppressed: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let full_move: FxHashSet<ArcVarId> = FxHashSet::default();

    let releases = compute_reassign_rebind_releases(
        &func,
        &owned,
        &move_alias,
        &inc_suppressed,
        &full_move,
        &FxHashSet::default(),
    );

    assert!(
        releases.is_empty(),
        "a genuinely-transferred (inc-suppressed) binding value must get no rebind release"
    );
}

/// Negative pin: a full-move binding value (whole owned-field set moved) owns
/// its own field-grain releases — the rebind scan must not add a whole-var dec.
#[test]
fn reassign_release_no_dec_when_full_move() {
    let func = rebind_func();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let move_alias: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let inc_suppressed: FxHashSet<ArcVarId> = FxHashSet::default();
    let full_move: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();

    let releases = compute_reassign_rebind_releases(
        &func,
        &owned,
        &move_alias,
        &inc_suppressed,
        &full_move,
        &FxHashSet::default(),
    );

    assert!(
        releases.is_empty(),
        "a full-move binding value must get no whole-var rebind release"
    );
}

/// Negative pin (double-free guard): an `Aggregate`-repr binding value (a
/// struct / enum whose RC-bearing field is moved out via a `Project` consumed
/// `[own]` — the `r.players[0].score = v` shape) MUST get no whole-var rebind
/// release; the partial/full-move field-grain machinery owns its release. Gate
/// (0) declines on the non-`RcPointer` repr.
#[test]
fn reassign_release_no_dec_for_aggregate_repr() {
    let mut func = rebind_func();
    // Flip `%0`'s repr to Aggregate (a struct binding); every other leak-gate
    // input still satisfied — proves gate (0) alone declines.
    func.var_reprs[0] = ValueRepr::Aggregate;
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let move_alias: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let inc_suppressed: FxHashSet<ArcVarId> = FxHashSet::default();
    let full_move: FxHashSet<ArcVarId> = FxHashSet::default();

    let releases = compute_reassign_rebind_releases(
        &func,
        &owned,
        &move_alias,
        &inc_suppressed,
        &full_move,
        &FxHashSet::default(),
    );

    assert!(
        releases.is_empty(),
        "an Aggregate-repr binding value must get no whole-var rebind release"
    );
}

/// Negative pin: a binding value whose terminal dec was NOT suppressed
/// (`old_var ∉ transfer_via_move_alias`) is already balanced by the normal
/// last-use machinery — the rebind scan must not add a second dec.
#[test]
fn reassign_release_no_dec_when_dec_not_suppressed() {
    let func = rebind_func();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let move_alias: FxHashSet<ArcVarId> = FxHashSet::default();
    let inc_suppressed: FxHashSet<ArcVarId> = FxHashSet::default();
    let full_move: FxHashSet<ArcVarId> = FxHashSet::default();

    let releases = compute_reassign_rebind_releases(
        &func,
        &owned,
        &move_alias,
        &inc_suppressed,
        &full_move,
        &FxHashSet::default(),
    );

    assert!(
        releases.is_empty(),
        "a binding value with a non-suppressed terminal dec must get no rebind release"
    );
}
