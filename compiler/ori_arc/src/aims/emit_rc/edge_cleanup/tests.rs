//! Unit tests for the RL-4 `deadAtSucc` same-alloc liveness probe
//! [`same_alloc_member_live_at`].
//!
//! The probe decides whether a borrowed-`Invoke` arg's per-edge release is
//! suppressed: a same-allocation alias-class member that is live at the
//! successor's entry suppresses the release (the receiver survives the catch).
//! A member defined ONLY on a sibling branch (a ghost member) is reported
//! `Once`-live at the successor by backward alias-demand bleed but is NOT
//! reachable on this edge — it must NOT suppress, or a genuine release is
//! dropped (leak). The `defined_at_or_before` guard mirrors the Category-1
//! normal-edge gate. Spec: Annex E §AIMS RL-4.

use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::emit_rc::take_project::TakeMoveFacts;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::AimsState;
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcTerminator, ArcVarId, ValueRepr};

use super::invoke::same_alloc_member_live_at;
use super::EdgeCleanupEnv;

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// Single-block function with one successor block-1 the probe queries. The
/// receiver `v(0)` and a same-alloc member `v(1)` both type `[str]`.
fn make_probe_func(pool: &mut Pool) -> ArcFunction {
    let list_str = pool.list(Idx::STR);
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![],
        },
    };
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Return { value: v(0) },
    };
    ArcFunction {
        name: Name::from_raw(1),
        params: vec![],
        return_type: list_str,
        blocks: vec![block0, block1],
        entry: ArcBlockId::new(0),
        var_types: vec![list_str; 2],
        var_reprs: vec![ValueRepr::RcPointer; 2],
        var_rc_strategies: Vec::new(),
        spans: vec![vec![]; 2],
        ..Default::default()
    }
}

/// Wire a state map where `arg` is `Absent` (dead) at block-1 entry and the
/// same-alloc member is `Once` (live) at block-1 entry — the backward
/// alias-demand-bleed shape that reports a ghost member as live.
fn state_map_with_member_live(func: &ArcFunction, arg: ArcVarId, member: ArcVarId) -> AimsStateMap {
    let mut state_map = AimsStateMap::new(func);

    let mut entry: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();
    entry.insert(arg, AimsState::BOTTOM); // Absent
    entry.insert(member, AimsState::FRESH); // Once-live
    state_map.update_block_entry(ArcBlockId::new(1), entry);

    // Union `arg` and `member` into one SSA-alias class (same allocation).
    let class_id = 0u32;
    let mut class_table: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    class_table.insert(arg, class_id);
    class_table.insert(member, class_id);
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(arg);
    members.insert(member);
    let mut class_members: FxHashMap<u32, FxHashSet<ArcVarId>> = FxHashMap::default();
    class_members.insert(class_id, members);
    state_map.set_ssa_alias_output(class_table, class_members, FxHashMap::default());

    state_map
}

fn env<'a>(
    func: &'a ArcFunction,
    state_map: &'a AimsStateMap,
    pool: &'a Pool,
    all_borrowed_defs: &'a FxHashSet<ArcVarId>,
    take_move_facts: &'a TakeMoveFacts,
    same_alloc_reps: &'a FxHashMap<ArcVarId, ArcVarId>,
) -> EdgeCleanupEnv<'a> {
    EdgeCleanupEnv {
        func,
        state_map,
        pool,
        all_borrowed_defs,
        take_move_facts,
        same_alloc_reps,
    }
}

/// Same-allocation reps map uniting `arg` and `member` under one rep so
/// `same_alloc(reps, member, arg)` is true.
fn same_alloc_reps(arg: ArcVarId, member: ArcVarId) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    reps.insert(arg, arg);
    reps.insert(member, arg);
    reps
}

/// Positive pin (the cure): a same-alloc member defined ONLY on a sibling
/// branch — NOT in `defined_at_or_before` for this Invoke block — must NOT
/// suppress `arg`'s per-edge release. Without the `defined_at_or_before`
/// guard the ghost member is reported `Once`-live at the successor and
/// phantom-suppresses the genuine release (leak); the guard excludes it.
#[test]
fn ghost_member_on_sibling_branch_does_not_suppress_edge_dec() {
    let mut pool = Pool::new();
    let func = make_probe_func(&mut pool);
    let arg = v(0);
    let ghost = v(1);

    let state_map = state_map_with_member_live(&func, arg, ghost);
    let all_borrowed_defs = FxHashSet::default();
    let take_move_facts = TakeMoveFacts::empty();
    let reps = same_alloc_reps(arg, ghost);
    let env = env(
        &func,
        &state_map,
        &pool,
        &all_borrowed_defs,
        &take_move_facts,
        &reps,
    );

    // `defined_at_or_before` for the Invoke block does NOT contain the ghost
    // member (it is defined on a sibling branch only).
    let defined_at_or_before: FxHashSet<ArcVarId> = [arg].into_iter().collect();

    // Category 2 (borrowed-arg) discipline: ghost-EXCLUSIVE, var-self included.
    let live = same_alloc_member_live_at(
        env,
        arg,
        ArcBlockId::new(1),
        &defined_at_or_before,
        true,
        true,
    );
    assert!(
        !live,
        "a ghost same-alloc member defined only on a sibling branch must NOT \
         suppress the Category-2 edge release — it is not reachable on the \
         borrowed-arg edge (leak)"
    );
}

/// Category-1 (Invoke normal-edge) discipline: ghost-INCLUSIVE. The SAME ghost
/// member defined only on a sibling arm DOES suppress the normal-edge release —
/// the normal edge merges arms, so the sibling-arm member is genuine cross-path
/// liveness, and that path owns the release. A ghost-EXCLUSIVE guard here would
/// double-free. Clamps the `require_defined_at_or_before = false` selector.
#[test]
fn ghost_member_on_sibling_branch_suppresses_normal_edge_dec() {
    let mut pool = Pool::new();
    let func = make_probe_func(&mut pool);
    let arg = v(0);
    let ghost = v(1);

    let state_map = state_map_with_member_live(&func, arg, ghost);
    let all_borrowed_defs = FxHashSet::default();
    let take_move_facts = TakeMoveFacts::empty();
    let reps = same_alloc_reps(arg, ghost);
    let env = env(
        &func,
        &state_map,
        &pool,
        &all_borrowed_defs,
        &take_move_facts,
        &reps,
    );
    let defined_at_or_before: FxHashSet<ArcVarId> = [arg].into_iter().collect();

    let live = same_alloc_member_live_at(
        env,
        arg,
        ArcBlockId::new(1),
        &defined_at_or_before,
        false,
        false,
    );
    assert!(
        live,
        "the same ghost member MUST suppress the Category-1 normal-edge release \
         (ghost-inclusive): the sibling-arm path is reachable through the merge \
         and owns the release — emitting here double-frees"
    );
}

/// Negative pin: a REAL same-alloc member that IS defined-at-or-before AND
/// live at the successor (the receiver-survives-the-catch case) MUST suppress
/// the edge release — emitting it would double-free the still-live allocation.
#[test]
fn live_same_alloc_member_defined_at_or_before_suppresses_edge_dec() {
    let mut pool = Pool::new();
    let func = make_probe_func(&mut pool);
    let arg = v(0);
    let alias = v(1);

    let state_map = state_map_with_member_live(&func, arg, alias);
    let all_borrowed_defs = FxHashSet::default();
    let take_move_facts = TakeMoveFacts::empty();
    let reps = same_alloc_reps(arg, alias);
    let env = env(
        &func,
        &state_map,
        &pool,
        &all_borrowed_defs,
        &take_move_facts,
        &reps,
    );

    // The alias IS defined at-or-before the Invoke block — a genuine
    // live-across receiver, reachable on this edge.
    let defined_at_or_before: FxHashSet<ArcVarId> = [arg, alias].into_iter().collect();

    let live = same_alloc_member_live_at(
        env,
        arg,
        ArcBlockId::new(1),
        &defined_at_or_before,
        true,
        true,
    );
    assert!(
        live,
        "a live same-alloc member defined at-or-before the Invoke must suppress \
         the edge release — emitting it would double-free the live allocation"
    );
}

/// Wire a state map where the CLASSLESS `arg` is `Once` (live) at block-1
/// entry. No SSA-alias class is set, so the member loop is never reached — this
/// isolates the `include_var_self` axis (the only path that can catch a
/// classless var's own liveness).
fn state_map_with_classless_var_live(func: &ArcFunction, arg: ArcVarId) -> AimsStateMap {
    let mut state_map = AimsStateMap::new(func);

    let mut entry: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();
    entry.insert(arg, AimsState::FRESH); // Once-live
    state_map.update_block_entry(ArcBlockId::new(1), entry);

    // No `set_ssa_alias_output` — `arg` carries no alias class.
    state_map
}

/// `include_var_self` axis clamp. For a CLASSLESS receiver live at the
/// successor, the Category-2 borrowed-arg gate (`include_var_self = true`)
/// suppresses via the var-self short-circuit (the receiver survives the catch —
/// the per-edge dec would double-free); the Category-1 normal-edge gate
/// (`include_var_self = false`) does NOT — the caller has already established
/// via `InvokeEdgeState.normal` that `arg` is `Absent` on THIS edge, so the
/// merge-level self reading would over-suppress a genuine release. The member
/// loop cannot catch a classless var, so the flag is the sole discriminator.
#[test]
fn classless_var_self_live_suppresses_only_when_include_var_self() {
    let mut pool = Pool::new();
    let func = make_probe_func(&mut pool);
    let arg = v(0);

    let state_map = state_map_with_classless_var_live(&func, arg);
    let all_borrowed_defs = FxHashSet::default();
    let take_move_facts = TakeMoveFacts::empty();
    let reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let env = env(
        &func,
        &state_map,
        &pool,
        &all_borrowed_defs,
        &take_move_facts,
        &reps,
    );
    let defined_at_or_before: FxHashSet<ArcVarId> = [arg].into_iter().collect();

    // Category 2: var-self short-circuit fires — live classless receiver suppresses.
    assert!(
        same_alloc_member_live_at(
            env,
            arg,
            ArcBlockId::new(1),
            &defined_at_or_before,
            true,
            true
        ),
        "Category-2 (include_var_self): a live classless receiver at the \
         successor must suppress the edge release — emitting it would double-free"
    );

    // Negative pin — Category 1: var-self short-circuit OFF + no class ⇒ the
    // member loop is unreachable, so a classless var's merge-level liveness must
    // NOT suppress the normal-edge release (the edge already proved it dead).
    assert!(
        !same_alloc_member_live_at(
            env,
            arg,
            ArcBlockId::new(1),
            &defined_at_or_before,
            false,
            false,
        ),
        "Category-1 (no include_var_self): a classless var's merge-level liveness \
         must NOT suppress the normal-edge release — that would over-suppress a \
         genuine release proved dead on this edge"
    );
}

/// A single-predecessor edge carrying BOTH a plain (`HeapPointer`) release and
/// an aggregate (`AggregateFields`) release. [`super::apply_edge_decs`] MUST
/// place BOTH releases at the successor's END (after the existing body), in
/// `decs` order — no `RcStrategy` shape is a sound proof that a release can
/// never transitively run a user `@drop` (a `HeapPointer` buffer's
/// `elem_dec_fn`, or a boxed type's generated drop function, can invoke one
/// exactly like an inline `AggregateFields`/`InlineEnum` field walk), so
/// entry placement would risk firing a drop BEFORE the successor's own
/// statements, inverting the user-visible drop order (Spec: Annex E §AIMS
/// RL-2 + RL-4 + RL-DROP).
#[test]
fn apply_edge_decs_places_every_release_at_successor_end() {
    use crate::ir::{ArcInstr, ArcValue, LitValue, RcAtomicity, RcStrategy};

    let plain_var = v(0);
    let aggregate_var = v(1);
    let placeholder_dst = v(2);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![],
        },
    };
    let placeholder = ArcInstr::Let {
        dst: placeholder_dst,
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(7)),
    };
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![],
        body: vec![placeholder.clone()],
        terminator: ArcTerminator::Return {
            value: placeholder_dst,
        },
    };
    let mut func = ArcFunction {
        name: Name::from_raw(1),
        params: vec![],
        return_type: Idx::INT,
        blocks: vec![block0, block1],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT; 3],
        var_reprs: vec![ValueRepr::RcPointer; 3],
        var_rc_strategies: Vec::new(),
        spans: vec![vec![], vec![Some(ori_ir::Span::new(5, 6))]],
        ..Default::default()
    };
    let predecessors: Vec<Vec<usize>> = vec![vec![], vec![0]];
    let edge_decs: Vec<(usize, usize, ArcVarId, RcStrategy)> = vec![
        (0, 1, plain_var, RcStrategy::HeapPointer),
        (0, 1, aggregate_var, RcStrategy::AggregateFields),
    ];

    super::apply_edge_decs(&mut func, &predecessors, edge_decs, false);

    let body = &func.blocks[1].body;
    assert_eq!(body.len(), 3, "the original placeholder + two end releases");
    assert_eq!(
        body[0], placeholder,
        "the original body instruction is undisturbed at the front — both \
         releases land after it"
    );
    assert!(
        matches!(
            &body[1],
            ArcInstr::RcDec { var, strategy: RcStrategy::HeapPointer, atomicity }
                if *var == plain_var && *atomicity == RcAtomicity::default_atomic()
        ),
        "the plain (HeapPointer) release lands at the successor's END, in \
         `decs` order; body[1]={:?}",
        body[1]
    );
    assert!(
        matches!(
            &body[2],
            ArcInstr::RcDec { var, strategy: RcStrategy::AggregateFields, atomicity }
                if *var == aggregate_var && *atomicity == RcAtomicity::default_atomic()
        ),
        "the aggregate (AggregateFields) release lands at the successor's END, \
         after the original body; body[2]={:?}",
        body[2]
    );

    // Span lockstep: the original placeholder keeps its span; both synthetic
    // releases carry `None` (no source span for an inserted edge release).
    let spans = &func.spans[1];
    assert_eq!(
        spans,
        &[Some(ori_ir::Span::new(5, 6)), None, None],
        "spans stay in lockstep with the reordered body — the placeholder's \
         span travels with it, synthetic releases carry no span"
    );
}
