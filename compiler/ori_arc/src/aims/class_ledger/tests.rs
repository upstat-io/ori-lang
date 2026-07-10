use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, FieldPath, NodeIdx};
use crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition;
use crate::aims::intraprocedural::ledger_events::{
    classify_function, BoundaryFacts, ClassOrigin, EventSite,
};
use crate::aims::intraprocedural::AimsStateMap;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind,
};
use crate::ownership::Ownership;

use super::apply::apply_plan;
use super::events::{ClassEvent, ClassEvents, EventKind};
use super::verify::verify_class;
use super::*;

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn ty(n: u32) -> Idx {
    Idx::from_raw(n)
}

fn test_interner() -> ori_ir::StringInterner {
    ori_ir::StringInterner::new()
}

fn construct(dst: u32, args: Vec<u32>) -> ArcInstr {
    ArcInstr::Construct {
        dst: v(dst),
        ty: ty(0),
        ctor: CtorKind::Tuple,
        args: args.into_iter().map(v).collect(),
    }
}

fn is_shared(dst: u32, var: u32) -> ArcInstr {
    ArcInstr::IsShared {
        dst: v(dst),
        var: v(var),
    }
}

/// Build an `Apply` with the fixture-standard callee (`Name::from_raw(7)`)
/// and no mono-instance dispatch. `args` pairs each argument var with its
/// ownership.
fn apply(dst: u32, args: Vec<(u32, ArgOwnership)>) -> ArcInstr {
    apply_to(dst, Name::from_raw(7), args)
}

/// Same as `apply`, with an explicit callee `Name`.
fn apply_to(dst: u32, func: Name, args: Vec<(u32, ArgOwnership)>) -> ArcInstr {
    let (vars, ownership): (Vec<u32>, Vec<ArgOwnership>) = args.into_iter().unzip();
    ArcInstr::Apply {
        dst: v(dst),
        ty: ty(0),
        func,
        args: vars.into_iter().map(v).collect(),
        arg_ownership: ownership,
        mono_instance_id: None,
    }
}

/// Build an `Invoke` with the fixture-standard callee (`Name::from_raw(7)`)
/// and no mono-instance dispatch.
fn invoke(dst: u32, args: Vec<(u32, ArgOwnership)>, normal: u32, unwind: u32) -> ArcTerminator {
    let (vars, ownership): (Vec<u32>, Vec<ArgOwnership>) = args.into_iter().unzip();
    ArcTerminator::Invoke {
        dst: v(dst),
        ty: ty(0),
        func: Name::from_raw(7),
        args: vars.into_iter().map(v).collect(),
        arg_ownership: ownership,
        mono_instance_id: None,
        normal: ArcBlockId::new(normal),
        unwind: ArcBlockId::new(unwind),
    }
}

fn block(id: u32, params: Vec<u32>, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: params.into_iter().map(|p| (v(p), ty(0))).collect(),
        body,
        terminator,
    }
}

fn jump(target: u32, args: Vec<u32>) -> ArcTerminator {
    ArcTerminator::Jump {
        target: ArcBlockId::new(target),
        args: args.into_iter().map(v).collect(),
    }
}

fn branch(cond: u32, then_block: u32, else_block: u32) -> ArcTerminator {
    ArcTerminator::Branch {
        cond: v(cond),
        then_block: ArcBlockId::new(then_block),
        else_block: ArcBlockId::new(else_block),
    }
}

fn ret(value: u32) -> ArcTerminator {
    ArcTerminator::Return { value: v(value) }
}

fn func_with_blocks(num_vars: u32, blocks: Vec<ArcBlock>) -> ArcFunction {
    ArcFunction {
        var_types: (0..num_vars).map(ty).collect(),
        blocks,
        ..Default::default()
    }
}

fn one_block_func(num_vars: u32, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcFunction {
    func_with_blocks(num_vars, vec![block(0, vec![], body, terminator)])
}

/// Classify and analyze with no callee contracts, handing back the
/// partition for class-rep lookups.
fn analyze(
    func: &ArcFunction,
    state_map: &AimsStateMap,
) -> (ClassLedgerAnalysis, BirthSitePartition) {
    analyze_with_registry(func, state_map, &ori_types::TypeRegistry::default())
}

fn analyze_with_registry(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    registry: &ori_types::TypeRegistry,
) -> (ClassLedgerAnalysis, BirthSitePartition) {
    let facts: FxHashMap<Name, BoundaryFacts> = FxHashMap::default();
    let mut partition = compute_birth_site_partition(func, state_map);
    let interner = test_interner();
    let classification = classify_function(func, state_map, &mut partition, &facts, &interner);
    let analysis = analyze_class_ledger(func, &classification, &mut partition, registry, &interner);
    (analysis, partition)
}

fn class_rep(partition: &mut BirthSitePartition, var: u32) -> NodeIdx {
    let node = partition.register_node(v(var), FieldPath::whole_var());
    partition.rep_of(node)
}

fn ops_for(analysis: &ClassLedgerAnalysis, class: NodeIdx) -> Vec<PlannedOp> {
    for plan in &analysis.plan.classes {
        if plan.class != class {
            continue;
        }
        return match &plan.outcome {
            ClassOutcome::Planned(ops) => ops.clone(),
            ClassOutcome::Declined(reason) => panic!("class unexpectedly declined: {reason:?}"),
        };
    }
    panic!("class not present in the plan");
}

fn decline_for(analysis: &ClassLedgerAnalysis, class: NodeIdx) -> DeclineReason {
    for plan in &analysis.plan.classes {
        if plan.class != class {
            continue;
        }
        return match &plan.outcome {
            ClassOutcome::Planned(ops) => panic!("class unexpectedly planned: {ops:?}"),
            ClassOutcome::Declined(reason) => *reason,
        };
    }
    panic!("class not present in the plan");
}

fn verdict_for(analysis: &ClassLedgerAnalysis, class: NodeIdx) -> ClassVerdict {
    for &(verdict_class, verdict) in &analysis.readiness.verdicts {
        if verdict_class == class {
            return verdict;
        }
    }
    panic!("class not present in the readiness verdicts");
}

/// Every planned op of every class, plan order.
fn planned_ops(analysis: &ClassLedgerAnalysis) -> Vec<PlannedOp> {
    analysis
        .plan
        .classes
        .iter()
        .flat_map(|plan| match &plan.outcome {
            ClassOutcome::Planned(ops) => ops.clone(),
            ClassOutcome::Declined(_) => Vec::new(),
        })
        .collect()
}

fn inc(slot: PlanSlot, var: u32) -> PlannedOp {
    PlannedOp {
        slot,
        kind: PlannedOpKind::Inc,
        var: v(var),
    }
}

fn dec(slot: PlanSlot, var: u32) -> PlannedOp {
    PlannedOp {
        slot,
        kind: PlannedOpKind::Dec,
        var: v(var),
    }
}

// Unconditional emitter

/// Fresh `str` construct, read once, dead — the fully-clean class-ledger
/// skeleton: the analysis produces a non-empty plan (the emitter is
/// UNCONDITIONAL; the legacy opt-out toggle no longer exists).
#[test]
fn unconditional_emitter_produces_plan() {
    let func = one_block_func(1, vec![construct(0, vec![])], ret(0));
    let state_map = AimsStateMap::new(&func);
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();
    let analysis =
        analyze_from_state_map(&func, &state_map, &contracts, &registry, &test_interner());
    assert!(!analysis.plan.classes.is_empty());
}

// Straight-line shapes

/// A fresh Construct returned is a move: birth consumed by the transfer —
/// net 0 with NO planned ops.
#[test]
fn fresh_construct_returned_moves_with_no_ops() {
    let func = one_block_func(1, vec![construct(0, vec![])], ret(0));
    let state_map = AimsStateMap::new(&func);
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 0);
    assert!(ops_for(&analysis, class).is_empty());
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
    assert!(analysis.readiness.declined.is_empty());
}

/// A non-empty string literal returned is the literal analog of the fresh
/// move: birth at the `Let`, consumed by the Return transfer — Clean with
/// NO planned ops (the legacy path emits nothing for it either).
#[test]
fn str_literal_returned_moves_clean_with_no_ops() {
    let func = one_block_func(
        1,
        vec![ArcInstr::Let {
            dst: v(0),
            ty: ty(0),
            value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
        }],
        ret(0),
    );
    let state_map = AimsStateMap::new(&func);
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 0);
    assert!(ops_for(&analysis, class).is_empty());
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
}

/// A string literal read then dead: the birth funds the read's floor and
/// one planned dec releases it — Clean, mirroring the fresh-read-dead
/// Construct shape.
#[test]
fn str_literal_read_then_dead_verifies_clean() {
    let func = one_block_func(
        2,
        vec![
            ArcInstr::Let {
                dst: v(0),
                ty: ty(0),
                value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
            },
            is_shared(1, 0),
        ],
        ret(1),
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 0);
    assert_eq!(
        ops_for(&analysis, class),
        vec![dec(PlanSlot::AfterBody { block: 0, index: 1 }, 0)]
    );
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
}

/// Fresh + read + dead: exactly one `BurdenDec` after the last read, in the
/// same block; the applied plan lands it right after the reading
/// instruction.
#[test]
fn fresh_read_dead_places_one_dec_after_last_read() {
    let func = one_block_func(2, vec![construct(0, vec![]), is_shared(1, 0)], ret(1));
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 0);
    assert_eq!(
        ops_for(&analysis, class),
        vec![dec(PlanSlot::AfterBody { block: 0, index: 1 }, 0)]
    );
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);

    let mut applied = func;
    apply_plan(&mut applied, &planned_ops(&analysis));
    assert_eq!(applied.blocks[0].body.len(), 3);
    assert_eq!(applied.blocks[0].body[2], ArcInstr::BurdenDec { var: v(0) });
}

/// Fresh aggregate with a funded field: the consumed field class emits no
/// separate release (the container inherits the obligation); the aggregate
/// class releases after its last read.
#[test]
fn funded_field_class_silent_aggregate_released_after_read() {
    let func = one_block_func(
        3,
        vec![construct(1, vec![]), construct(0, vec![1]), is_shared(2, 0)],
        ret(2),
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let buffer = class_rep(&mut partition, 1);
    let aggregate = class_rep(&mut partition, 0);
    assert_ne!(buffer, aggregate);
    assert!(ops_for(&analysis, buffer).is_empty());
    assert_eq!(verdict_for(&analysis, buffer), ClassVerdict::Clean);
    assert_eq!(
        ops_for(&analysis, aggregate),
        vec![dec(PlanSlot::AfterBody { block: 0, index: 2 }, 0)]
    );
    assert_eq!(verdict_for(&analysis, aggregate), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
}

/// RL-5: a dead-on-arrival owned param owes one immediate release at block
/// entry.
#[test]
fn dead_on_arrival_owned_param_released_at_entry() {
    let mut func = one_block_func(2, vec![], ret(1));
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Owned,
    }];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 0);
    assert_eq!(
        ops_for(&analysis, class),
        vec![dec(PlanSlot::BlockFront { block: 0 }, 0)]
    );
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);

    let mut applied = func;
    apply_plan(&mut applied, &planned_ops(&analysis));
    assert_eq!(
        applied.blocks[0].body,
        vec![ArcInstr::BurdenDec { var: v(0) }]
    );
}

/// A borrowed-rooted class consumed at a Construct store gets a funding
/// `BurdenInc` before the consume (the caller retains ownership).
#[test]
fn borrowed_rooted_consume_gets_inc_before_store() {
    let mut func = one_block_func(2, vec![construct(1, vec![0])], ret(1));
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    let state_map = AimsStateMap::new(&func);
    let (analysis, mut partition) = analyze(&func, &state_map);

    let borrowed = class_rep(&mut partition, 0);
    let aggregate = class_rep(&mut partition, 1);
    assert_eq!(
        ops_for(&analysis, borrowed),
        vec![inc(PlanSlot::BeforeBody { block: 0, index: 0 }, 0)]
    );
    assert_eq!(verdict_for(&analysis, borrowed), ClassVerdict::Clean);
    assert!(ops_for(&analysis, aggregate).is_empty());
    assert!(analysis.readiness.all_classes_clean);

    let mut applied = func;
    apply_plan(&mut applied, &planned_ops(&analysis));
    assert_eq!(applied.blocks[0].body[0], ArcInstr::BurdenInc { var: v(0) });
    assert_eq!(applied.blocks[0].body.len(), 2);
}

/// An owned class consumed at its last use is a move: NO inc.
#[test]
fn owned_class_moved_at_last_use_gets_no_inc() {
    let func = one_block_func(2, vec![construct(0, vec![]), construct(1, vec![0])], ret(1));
    let state_map = AimsStateMap::new(&func);
    let (analysis, mut partition) = analyze(&func, &state_map);

    let moved = class_rep(&mut partition, 0);
    assert!(ops_for(&analysis, moved).is_empty());
    assert_eq!(verdict_for(&analysis, moved), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
}

// Branch / merge shapes

/// RL-4: a class dying on one arm only gets the per-edge dec on the dying
/// arm's front; the live arm releases after its last read; merge owed
/// equality holds.
#[test]
fn branch_death_gets_edge_dec_on_dying_arm_only() {
    let func = func_with_blocks(
        3,
        vec![
            block(0, vec![], vec![construct(0, vec![])], branch(1, 1, 2)),
            block(1, vec![], vec![is_shared(2, 0)], jump(3, vec![])),
            block(2, vec![], vec![], jump(3, vec![])),
            block(3, vec![], vec![], ret(1)),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    state_map.set_permanent_scalar(v(2));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 0);
    assert_eq!(
        ops_for(&analysis, class),
        vec![
            dec(PlanSlot::BlockFront { block: 2 }, 0),
            dec(PlanSlot::AfterBody { block: 1, index: 0 }, 0),
        ]
    );
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
}

// Verifier soundness (`verify::verify_class`)

/// A residual owed reference reaching a `Resume` terminal is a leak exactly
/// like one reaching `Return` — `verify_class` must catch it even when the
/// planner supplies no releasing ops (the defense-in-depth case: a planner
/// bug that fails to place a release on the unwind arm). Hand-built events
/// bypass `plan_class` so the verifier's own terminal-net check is pinned in
/// isolation, matching the uniform Return/Resume/Unreachable treatment in
/// `aims::verify::burden_delta::balance_verdict_from_nets`.
#[test]
fn resume_terminal_residual_is_flagged_leak_not_silently_clean() {
    let func = func_with_blocks(
        1,
        vec![
            block(0, vec![], vec![construct(0, vec![])], branch(0, 1, 2)),
            block(1, vec![], vec![], ArcTerminator::Resume),
            block(2, vec![], vec![], ret(0)),
        ],
    );
    let preds = crate::graph::compute_predecessors(&func);

    // block 0: birth (+1, floor 0); block 1 (Resume): nothing — the class's
    // sole owed reference is never released on this arm; block 2 (Return):
    // consumed (-1, floor 1) so the Return arm alone nets 0.
    let events = ClassEvents {
        origin: Some(ClassOrigin::Fresh),
        container_held: false,
        externally_funded: false,
        threads_back_edge: false,
        books_runtime_grounded: true,
        per_block: vec![
            vec![ClassEvent {
                site: EventSite::Body(0),
                kind: EventKind::Birth,
                var: Some(v(0)),
                delta: 1,
                floor: 0,
            }],
            vec![],
            vec![ClassEvent {
                site: EventSite::Terminator,
                kind: EventKind::Consume,
                var: Some(v(0)),
                delta: -1,
                floor: 1,
            }],
        ],
    };

    assert_eq!(
        verify_class(&func, &preds, &events, &[]),
        ClassVerdict::LeakOnly
    );
}

// Loop shapes

/// Same-class loop threading (jump-arg silent): no per-iteration ops; the
/// single release lands on the loop-exit edge.
#[test]
fn loop_threading_is_silent_and_releases_on_the_exit_edge() {
    let func = func_with_blocks(
        4,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(1, vec![1], vec![is_shared(2, 1)], branch(3, 2, 3)),
            block(2, vec![], vec![], jump(1, vec![1])),
            block(3, vec![], vec![], ret(3)),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    state_map.set_permanent_scalar(v(3));
    let (analysis, mut partition) = analyze(&func, &state_map);

    // The loop param unified with the birth class (singleton witness).
    let class = class_rep(&mut partition, 0);
    assert_eq!(class, class_rep(&mut partition, 1));
    assert_eq!(
        ops_for(&analysis, class),
        vec![dec(PlanSlot::BlockFront { block: 3 }, 1)]
    );
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
}

/// A per-iteration non-zero class delta (a credit inside the loop with no
/// matching consume) cannot be proven: the emitter DECLINES the class and
/// the verify core reports Unprovable — never a wrong placement, never a
/// false Clean.
#[test]
fn cyclic_nonzero_iteration_delta_declines_and_verifies_unprovable() {
    let func = func_with_blocks(
        4,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(
                1,
                vec![1],
                vec![ArcInstr::BurdenInc { var: v(1) }],
                branch(3, 2, 3),
            ),
            block(2, vec![], vec![], jump(1, vec![1])),
            block(3, vec![], vec![], ret(3)),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 0);
    assert_eq!(decline_for(&analysis, class), DeclineReason::MergeDisagree);
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Unprovable);
    assert!(!analysis.readiness.all_classes_clean);
    assert_eq!(
        analysis.readiness.declined,
        vec![(class, DeclineReason::MergeDisagree)]
    );
    assert!(planned_ops(&analysis).is_empty());
}

// Decline-path coverage (`emit::DeclineReason`)

/// An over-owed class (a birth plus a placed credit, no consume) reaching
/// the sole block's end with no live successor is a within-block release
/// the emitter cannot place (`exit != 1`) — declined `UnplaceableRelease`,
/// never silently planned; the verifier still walks the bare event stream
/// and reports the leak honestly.
#[test]
fn over_owed_class_declines_unplaceable_release_and_verifies_leak() {
    let func = one_block_func(
        2,
        vec![
            construct(0, vec![]),
            is_shared(1, 0),
            ArcInstr::BurdenInc { var: v(0) },
        ],
        ret(1),
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 0);
    assert_eq!(
        decline_for(&analysis, class),
        DeclineReason::UnplaceableRelease
    );
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::LeakOnly);
    assert!(!analysis.readiness.all_classes_clean);
}

/// A borrowed-rooted class's CONSUME event with no resolvable member
/// variable cannot be funded — `plan_incs` cannot emit the mandatory
/// `BurdenInc` without a subject, so the class declines `UnresolvedOpVar`
/// (fail-closed; never a wrong placement synthesized from a missing var).
#[test]
fn unresolved_consume_var_declines_unresolved_op_var() {
    let func = one_block_func(1, vec![], ret(0));
    let preds = crate::graph::compute_predecessors(&func);
    let events = ClassEvents {
        origin: Some(ClassOrigin::Borrowed),
        container_held: false,
        externally_funded: true,
        threads_back_edge: false,
        books_runtime_grounded: true,
        per_block: vec![vec![ClassEvent {
            site: EventSite::Body(0),
            kind: EventKind::Consume,
            var: None,
            delta: -1,
            floor: 1,
        }]],
    };
    let regions = super::emit::CycleRegions::compute(&func);
    assert!(matches!(
        super::emit::plan_class(&func, &preds, &regions, &events, &[]),
        ClassOutcome::Declined(DeclineReason::UnresolvedOpVar)
    ));
}

/// A borrowed-rooted class's CONSUME event recorded at BLOCK ENTRY has no
/// instruction slot to insert the mandatory funding `BurdenInc` before —
/// `plan_incs` has no expressible pre-consume site for a block-entry
/// consume, so the class declines `UnplaceableInc` (fail-closed; never a
/// wrong placement synthesized from a slot that doesn't exist).
#[test]
fn consume_at_block_entry_declines_unplaceable_inc() {
    let func = one_block_func(1, vec![], ret(0));
    let preds = crate::graph::compute_predecessors(&func);
    let events = ClassEvents {
        origin: Some(ClassOrigin::Borrowed),
        container_held: false,
        externally_funded: true,
        threads_back_edge: false,
        books_runtime_grounded: true,
        per_block: vec![vec![ClassEvent {
            site: EventSite::BlockEntry,
            kind: EventKind::Consume,
            var: Some(v(0)),
            delta: -1,
            floor: 1,
        }]],
    };
    let regions = super::emit::CycleRegions::compute(&func);
    assert!(matches!(
        super::emit::plan_class(&func, &preds, &regions, &events, &[]),
        ClassOutcome::Declined(DeclineReason::UnplaceableInc)
    ));
}

// Passthrough refund

/// An RL-34 passthrough (consume at the call refunded by the same-site
/// credit) transfers the existing reference: no inc, net 0.
#[test]
fn passthrough_refund_needs_no_inc() {
    let func = one_block_func(
        2,
        vec![
            construct(0, vec![]),
            apply_to(1, Name::from_raw(11), vec![(0, ArgOwnership::Owned)]),
        ],
        ret(1),
    );
    let mut state_map = AimsStateMap::new(&func);
    let mut aliases: FxHashMap<
        ArcVarId,
        crate::aims::intraprocedural::state_map::ApplyAliasSource,
    > = FxHashMap::default();
    aliases.insert(
        v(1),
        crate::aims::intraprocedural::state_map::ApplyAliasSource::Direct(v(0)),
    );
    state_map.set_apply_result_aliases(aliases);
    let (analysis, mut partition) = analyze(&func, &state_map);

    // Direct alias unified arg and result into ONE class.
    let class = class_rep(&mut partition, 0);
    assert_eq!(class, class_rep(&mut partition, 1));
    assert!(ops_for(&analysis, class).is_empty());
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
}

// Replacement seam (`replace::attempt_replacement`)

/// The fully-clean skeleton commits: plan applied, emission flag set,
/// `burden_emitted` unmarked (the edge machinery stays inert).
#[test]
fn replacement_commits_clean_plan_and_sets_emission_flag() {
    let func = one_block_func(2, vec![construct(0, vec![]), is_shared(1, 0)], ret(1));
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();

    let mut replaced = func;
    let outcome = attempt_replacement(
        &mut replaced,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(outcome.mode, EmissionMode::Replaced);
    assert!(outcome.fallback_reason.is_none());
    assert!(replaced.class_ledger_emission);
    assert!(replaced.burden_emitted.iter().all(|marked| !marked));
    assert_eq!(replaced.blocks[0].body.len(), 3);
    assert_eq!(
        replaced.blocks[0].body[2],
        ArcInstr::BurdenDec { var: v(0) }
    );
}

/// `allow_replacement = false` (Step-4b emission disabled) keeps the
/// analysis reportable and never mutates the function.
#[test]
fn replacement_disallowed_reports_analysis_and_leaves_function_untouched() {
    let func = one_block_func(2, vec![construct(0, vec![]), is_shared(1, 0)], ret(1));
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();

    let mut gated = func.clone();
    let outcome = attempt_replacement(
        &mut gated,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        false,
    );
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(
        outcome.fallback_reason,
        Some(FallbackReason::LegacyEmissionDisabled)
    );
    assert!(outcome.analysis.readiness.all_classes_clean);
    assert_eq!(gated, func);
}

/// A declined class (non-clean readiness) falls back untouched.
#[test]
fn replacement_declines_non_clean_readiness() {
    let func = func_with_blocks(
        4,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(
                1,
                vec![1],
                vec![ArcInstr::BurdenInc { var: v(1) }],
                branch(3, 2, 3),
            ),
            block(2, vec![], vec![], jump(1, vec![1])),
            block(3, vec![], vec![], ret(3)),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();

    let mut gated = func.clone();
    let outcome = attempt_replacement(
        &mut gated,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(
        outcome.fallback_reason,
        Some(FallbackReason::ReadinessNotClean)
    );
    assert!(!gated.class_ledger_emission);
    assert_eq!(gated, func);
}

/// A zero-class function with a NON-excluded variable falls back: the class
/// model proves nothing about a live value it never evented.
#[test]
fn replacement_declines_zero_class_function_with_unexcluded_var() {
    let func = one_block_func(
        2,
        vec![ArcInstr::Let {
            dst: v(0),
            ty: ty(0),
            value: ArcValue::Literal(crate::ir::LitValue::Int(5)),
        }],
        ret(0),
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(0));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();

    let mut gated = func.clone();
    let outcome = attempt_replacement(
        &mut gated,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(outcome.fallback_reason, Some(FallbackReason::ZeroClasses));
    assert_eq!(gated, func);
}

/// Empty-surface admission: every variable excluded (scalar) means no
/// RC-bearing value exists anywhere, so the zero-class function commits
/// with an EMPTY plan — no ops added, emission flag set.
#[test]
fn replacement_admits_all_scalar_function_with_empty_plan() {
    let func = one_block_func(
        1,
        vec![ArcInstr::Let {
            dst: v(0),
            ty: ty(0),
            value: ArcValue::Literal(crate::ir::LitValue::Int(5)),
        }],
        ret(0),
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(0));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();

    let mut replaced = func.clone();
    let outcome = attempt_replacement(
        &mut replaced,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(outcome.mode, EmissionMode::Replaced);
    assert!(outcome.fallback_reason.is_none());
    assert!(outcome.analysis.plan.classes.is_empty());
    assert!(replaced.class_ledger_emission);
    assert_eq!(replaced.blocks, func.blocks);
}

/// The empty-surface admission never bypasses the user-drop gate: a
/// scalar-repr variable whose TYPE carries a user `@drop` (the RL-DROP
/// shape) still falls back to the legacy walk's user-drop completeness.
#[test]
fn replacement_declines_all_scalar_function_with_user_drop_type() {
    use core::num::NonZeroU32;
    use ori_registry::burden::FnSym;
    use ori_types::burden::UserBurdenSpec;

    let struct_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Guarded",
        struct_idx,
        Some(UserBurdenSpec {
            user_drop: Some(FnSym::new(NonZeroU32::MIN)),
            ..UserBurdenSpec::default()
        }),
    );

    let mut func = one_block_func(
        1,
        vec![ArcInstr::Let {
            dst: v(0),
            ty: struct_idx,
            value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
        }],
        ret(0),
    );
    func.var_types = vec![struct_idx];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(0));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let mut gated = func.clone();
    let outcome = attempt_replacement(
        &mut gated,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(outcome.fallback_reason, Some(FallbackReason::UserDropGlue));
    assert_eq!(gated, func);
}

/// A function whose type surface carries a user `@drop` falls back — the
/// RL-DROP user-drop completeness pass belongs to the legacy walk.
#[test]
fn replacement_declines_user_drop_glue_function() {
    use core::num::NonZeroU32;
    use ori_registry::burden::FnSym;
    use ori_types::burden::UserBurdenSpec;

    let struct_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Guarded",
        struct_idx,
        Some(UserBurdenSpec {
            user_drop: Some(FnSym::new(NonZeroU32::MIN)),
            ..UserBurdenSpec::default()
        }),
    );

    let mut func = one_block_func(2, vec![construct(0, vec![]), is_shared(1, 0)], ret(1));
    func.var_types = vec![struct_idx, ty(0)];
    if let Some(ArcInstr::Construct { ty: ctor_ty, .. }) = func.blocks[0].body.first_mut() {
        *ctor_ty = struct_idx;
    }
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let mut gated = func.clone();
    let outcome = attempt_replacement(
        &mut gated,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(outcome.fallback_reason, Some(FallbackReason::UserDropGlue));
    assert_eq!(gated, func);
}

// Op-placement guard (`replace::ops_placeable`)

/// A planned op whose variable's definition dominates the slot is
/// placeable; a definition on a sibling branch is not.
#[test]
fn op_var_placement_requires_dominating_definition() {
    let func = func_with_blocks(
        3,
        vec![
            block(0, vec![], vec![construct(1, vec![])], branch(0, 1, 2)),
            block(1, vec![], vec![], ret(1)),
            block(2, vec![], vec![construct(2, vec![])], ret(2)),
        ],
    );

    let dominated = vec![dec(PlanSlot::BlockFront { block: 1 }, 1)];
    assert!(super::replace::ops_placeable(&func, &dominated));

    let off_path = vec![dec(PlanSlot::BlockFront { block: 1 }, 2)];
    assert!(!super::replace::ops_placeable(&func, &off_path));

    let before_own_def = vec![dec(PlanSlot::BlockFront { block: 0 }, 1)];
    assert!(!super::replace::ops_placeable(&func, &before_own_def));

    let after_own_def = vec![dec(PlanSlot::AfterBody { block: 0, index: 0 }, 1)];
    assert!(super::replace::ops_placeable(&func, &after_own_def));
}

// Invoke-with-unwind shapes

/// An `Invoke` result read on the normal path then dead: the result's class
/// births at the NORMAL successor's entry (never the invoking block), the
/// unwind path owes nothing for it, every class verifies Clean, and every
/// planned op is placeable (no release of a never-materialized value on the
/// unwind edge).
#[test]
fn invoke_result_class_clean_and_placeable_across_unwind() {
    // bb0: %0 = Construct; Invoke f(%0 owned) -> %1, normal bb1, unwind bb2
    // bb1: %2 = IsShared %1; Return %2
    // bb2: Resume
    let func = func_with_blocks(
        3,
        vec![
            block(
                0,
                vec![],
                vec![construct(0, vec![])],
                invoke(1, vec![(0, ArgOwnership::Owned)], 1, 2),
            ),
            block(1, vec![], vec![is_shared(2, 1)], ret(2)),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let result = class_rep(&mut partition, 1);
    assert_eq!(verdict_for(&analysis, result), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
    assert!(analysis.readiness.declined.is_empty());

    let ops = planned_ops(&analysis);
    assert!(
        super::replace::ops_placeable(&func, &ops),
        "no planned op may land where its variable never materializes: {ops:?}"
    );
    assert!(
        ops.iter().all(|op| op.slot.block() != 2),
        "the unwind path owes nothing for the invoke result: {ops:?}"
    );
}

// Branch-exclusive edge death (RL-4)

/// A value threaded through ONE arm of a branch and dead on the other: the
/// merge param is funded per-edge (cross-class credits), and the dying
/// class takes exactly one RL-4 front dec on its dead arm — the edge
/// release resolves the merge's owed-count agreement, so the class
/// verifies Clean (never `MergeDisagree`).
#[test]
fn branch_exclusive_death_places_edge_dec_and_verifies_clean() {
    // bb0: %1 = Construct; branch %0 ? bb1 : bb2   (%0 scalar cond)
    // bb1: %4 = Let Var(%1); Jump bb3(%4)          (same class as %1)
    // bb2: %5 = Construct;   Jump bb3(%5)          (distinct class)
    // bb3(%6): Return %6                            (refused merge param)
    let func = func_with_blocks(
        7,
        vec![
            block(0, vec![], vec![construct(1, vec![])], branch(0, 1, 2)),
            block(
                1,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(4),
                    ty: ty(0),
                    value: ArcValue::Var(v(1)),
                }],
                jump(3, vec![4]),
            ),
            block(2, vec![], vec![construct(5, vec![])], jump(3, vec![5])),
            block(3, vec![6], vec![], ret(6)),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(0));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let threaded = class_rep(&mut partition, 1);
    assert_eq!(verdict_for(&analysis, threaded), ClassVerdict::Clean);
    assert!(analysis.readiness.declined.is_empty());
    assert!(analysis.readiness.all_classes_clean);

    let threaded_ops = ops_for(&analysis, threaded);
    assert_eq!(
        threaded_ops
            .iter()
            .filter(|op| op.kind == PlannedOpKind::Dec
                && op.slot == (PlanSlot::BlockFront { block: 2 }))
            .count(),
        1,
        "exactly one RL-4 front dec on the dead arm: {threaded_ops:?}"
    );
}

// Container-held field classes

/// A field view projected out of a param aggregate is CONTAINER-HELD: the
/// aggregate owns the field's reference, so the field class owes nothing at
/// entry and a hand-off (store into a fresh Construct) funds its own inc —
/// exactly the borrowed-rooted discipline, with the container in the
/// caller's role.
#[test]
fn param_field_view_consume_gets_funding_inc_and_verifies_clean() {
    // %0: borrowed param aggregate; %1 = Project %0.0; %2 = Construct(%1)
    let mut func = one_block_func(
        3,
        vec![
            ArcInstr::Project {
                dst: v(1),
                ty: ty(0),
                value: v(0),
                field: 0,
            },
            construct(2, vec![1]),
        ],
        ret(2),
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    let state_map = AimsStateMap::new(&func);
    let (analysis, mut partition) = analyze(&func, &state_map);

    let field_class = {
        let node = partition.register_node(v(1), FieldPath::whole_var());
        partition.rep_of(node)
    };
    assert_eq!(
        ops_for(&analysis, field_class),
        vec![inc(PlanSlot::BeforeBody { block: 0, index: 1 }, 1)]
    );
    assert_eq!(verdict_for(&analysis, field_class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
}

/// A projected field view of an OWNED param aggregate read past the
/// container's own last use: the container's planned release (recursive,
/// after ITS last event) precedes the view's read, so the view funds itself
/// at extraction — inc at the Project, dec after the view's last read.
#[test]
fn owned_param_field_view_read_past_container_release_funds_itself() {
    // %0: owned param aggregate; %1 = Project %0.0; %2 = IsShared %1
    let mut func = one_block_func(
        3,
        vec![
            ArcInstr::Project {
                dst: v(1),
                ty: ty(0),
                value: v(0),
                field: 0,
            },
            is_shared(2, 1),
        ],
        ret(2),
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Owned,
    }];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let field_class = {
        let node = partition.register_node(v(1), FieldPath::whole_var());
        partition.rep_of(node)
    };
    assert_eq!(
        ops_for(&analysis, field_class),
        vec![
            inc(PlanSlot::AfterBody { block: 0, index: 0 }, 1),
            dec(PlanSlot::AfterBody { block: 0, index: 1 }, 1),
        ]
    );
    assert_eq!(verdict_for(&analysis, field_class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
}

// Fully-dead classes (RL-2 unused-owned)

/// A fresh Construct born inside a loop body and never used: the class has
/// zero demand events, so each iteration's reference releases immediately
/// after its birth (RL-2 unused-owned) — the loop header's owed count
/// agrees at 0 across the back-edge and the class verifies Clean.
#[test]
fn unused_loop_body_construct_releases_immediately_and_verifies_clean() {
    // bb0: Jump bb1
    // bb1: Branch %0 ? bb2 : bb3     (%0 scalar cond)
    // bb2: %1 = Construct; Jump bb1  (unused, dead-on-creation, in-cycle)
    // bb3: Return %0
    let func = func_with_blocks(
        2,
        vec![
            block(0, vec![], vec![], jump(1, vec![])),
            block(1, vec![], vec![], branch(0, 2, 3)),
            block(2, vec![], vec![construct(1, vec![])], jump(1, vec![])),
            block(3, vec![], vec![], ret(0)),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(0));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 1);
    assert_eq!(
        ops_for(&analysis, class),
        vec![dec(PlanSlot::AfterBody { block: 2, index: 0 }, 1)]
    );
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
    assert!(analysis.readiness.declined.is_empty());
}

/// A class born and last-used inside ONE loop block (a per-iteration heap
/// literal read then dead): the block-local release lands right after the
/// class's last event, so each iteration balances and the loop header's
/// owed count agrees across the back-edge.
#[test]
fn per_iteration_block_local_class_releases_after_last_use() {
    // bb0: Jump bb1
    // bb1: %1 = "lit"; %2 = IsShared %1; Branch %0 ? bb1' : bb2  (loop on bb1)
    // bb2: Return %0
    let func = func_with_blocks(
        3,
        vec![
            block(0, vec![], vec![], jump(1, vec![])),
            block(
                1,
                vec![],
                vec![
                    ArcInstr::Let {
                        dst: v(1),
                        ty: ty(0),
                        value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
                    },
                    is_shared(2, 1),
                ],
                branch(0, 1, 2),
            ),
            block(2, vec![], vec![], ret(0)),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(0));
    state_map.set_permanent_scalar(v(2));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 1);
    assert_eq!(
        ops_for(&analysis, class),
        vec![dec(PlanSlot::AfterBody { block: 1, index: 1 }, 1)]
    );
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
}

// Unmodeled-shape gates (TRMC / reuse)

/// A function carrying a reuse-token shape (`Reset`/`Reuse` pairing) falls
/// back: the rebirth reuses the DYING value's allocation, which the
/// fresh-birth-site class model does not represent yet.
#[test]
fn replacement_declines_reuse_shapes() {
    let func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            ArcInstr::Reset {
                var: v(0),
                token: v(1),
            },
            ArcInstr::Reuse {
                token: v(1),
                dst: v(2),
                ty: ty(0),
                ctor: CtorKind::Tuple,
                args: vec![],
            },
        ],
        ret(2),
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();

    let mut gated = func.clone();
    let outcome = attempt_replacement(
        &mut gated,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(outcome.fallback_reason, Some(FallbackReason::ReuseShape));
    assert_eq!(gated, func);
}

/// A TRMC `ContextHole`-shaped variable no longer declines replacement: the
/// fill-at-recursive-call is modeled (the fill's `Set` classifies as
/// mutate(context) + consume(filled value) — the K3 derivation; the fill IS
/// the filled value's release per `holeFill_is_the_release`). The fixture
/// replaces on its own merits — the `TrmcContext` reason never fires.
#[test]
fn trmc_context_hole_admitted_post_k3() {
    let func = one_block_func(1, vec![construct(0, vec![])], ret(0));
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_var_shape(v(0), crate::aims::lattice::ShapeClass::ContextHole);
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();

    let mut gated = func;
    let outcome = attempt_replacement(
        &mut gated,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_ne!(outcome.fallback_reason, Some(FallbackReason::TrmcContext));
}

// Field-view hazard cures across loop / merge / select liveness shapes

/// A container released by the plan (whole-var dec / consume) whose
/// field-path view class carries its OWN events: the view funds itself with
/// an inc at its extraction site (RL-1 dup — the container's recursive
/// release and the view's independent reference each balance), so the
/// function REPLACES with inc-at-project + dec-at-last-use on the view.
#[test]
fn live_field_view_of_released_container_funds_itself_at_extraction() {
    // %0 = Apply f()          (opaque container — no field funding known)
    // %1 = Project %0.0       (extracted view; container-held field class)
    // %2 = IsShared %0        (container read; container dies after)
    // %3 = IsShared %1        (view read AFTER the container's last use)
    // Return %3 (scalar)
    let func = one_block_func(
        4,
        vec![
            apply(0, vec![]),
            ArcInstr::Project {
                dst: v(1),
                ty: ty(0),
                value: v(0),
                field: 0,
            },
            is_shared(2, 0),
            is_shared(3, 1),
        ],
        ret(3),
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    state_map.set_permanent_scalar(v(3));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();

    let mut gated = func.clone();
    let outcome = attempt_replacement(
        &mut gated,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(
        outcome.mode,
        EmissionMode::Replaced,
        "fallback_reason={:?} readiness={:?}",
        outcome.fallback_reason,
        outcome.analysis.readiness
    );
    assert!(outcome.fallback_reason.is_none());

    let mut partition = {
        let sm = AimsStateMap::new(&func);
        compute_birth_site_partition(&func, &sm)
    };
    let view = class_rep(&mut partition, 1);
    let view_ops = ops_for(&outcome.analysis, view);
    assert_eq!(
        view_ops,
        vec![
            inc(PlanSlot::AfterBody { block: 0, index: 1 }, 1),
            dec(PlanSlot::AfterBody { block: 0, index: 3 }, 1),
        ],
        "view class funds at extraction and releases at last use: {view_ops:?}"
    );
}

/// An RL-34 passthrough through an INVOKE: the consume sits at the invoking
/// block's terminator and its refund credit at the NORMAL successor's entry
/// — the same call boundary, so no funding inc is planned and the class
/// nets zero with no ops (the transfer moves the existing reference).
#[test]
fn ttr_refund_across_invoke_normal_edge_needs_no_inc() {
    // bb0: Invoke f(%0 owned) -> %1, normal bb1, unwind bb2
    // bb1: Return %1
    // bb2: Resume
    let mut func = func_with_blocks(
        2,
        vec![
            block(
                0,
                vec![],
                vec![],
                invoke(1, vec![(0, ArgOwnership::Owned)], 1, 2),
            ),
            block(1, vec![], vec![], ret(1)),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Owned,
    }];
    let mut state_map = AimsStateMap::new(&func);
    let mut aliases: FxHashMap<
        ArcVarId,
        crate::aims::intraprocedural::state_map::ApplyAliasSource,
    > = FxHashMap::default();
    aliases.insert(
        v(1),
        crate::aims::intraprocedural::state_map::ApplyAliasSource::Direct(v(0)),
    );
    state_map.set_apply_result_aliases(aliases);
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 0);
    assert_eq!(class, class_rep(&mut partition, 1), "ttr unifies the class");
    assert!(
        ops_for(&analysis, class).is_empty(),
        "the passthrough moves the existing reference: {:?}",
        ops_for(&analysis, class)
    );
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
    assert!(analysis.readiness.all_classes_clean);
    assert!(analysis.readiness.declined.is_empty());
}

/// The rebind loop (`acc = Cons(x, acc)`): each iteration's fresh node is
/// consumed by the jump hand-off into the loop param — a genuine transfer,
/// not a duplication. Demand liveness must not wrap the back-edge (the
/// "later demand" it would see is the NEXT iteration's own events), so no
/// funding inc is planted and every class nets zero per iteration.
#[test]
fn rebind_loop_hand_off_plants_no_spurious_inc() {
    // bb0: %0 = Construct (Nil); Jump bb1(%0)
    // bb1(%1): Branch %4 ? bb2 : bb3
    // bb2: %2 = Construct(%1) (Cons funding the param); Jump bb1(%2)
    // bb3: Return %1
    let func = func_with_blocks(
        5,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(1, vec![1], vec![], branch(4, 2, 3)),
            block(2, vec![], vec![construct(2, vec![1])], jump(1, vec![2])),
            block(3, vec![], vec![], ret(1)),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(4));
    let (analysis, mut partition) = analyze(&func, &state_map);

    // The loop param is a refused merge (Nil site vs Cons site) funded
    // per-edge; the Cons node transfers into it each iteration. No class
    // declines and every verdict is Clean.
    assert!(
        analysis.readiness.declined.is_empty(),
        "declined: {:?}",
        analysis.readiness.declined
    );
    assert!(analysis.readiness.all_classes_clean);

    // No inc lands in the loop body: the hand-off transfers the reference.
    let cons = class_rep(&mut partition, 2);
    let cons_ops = ops_for(&analysis, cons);
    assert!(
        cons_ops.iter().all(|op| op.kind != PlannedOpKind::Inc),
        "no funding inc for a pure transfer: {cons_ops:?}"
    );
}

/// A loop accumulator seeded with an EXCLUDED initial value (an immortal
/// empty string): the entry edge still credits the param class — the slot
/// holds a reference whose eventual dec is a runtime no-op on the immortal
/// — so both loop-header edges agree at one owed reference and the class
/// verifies Clean.
#[test]
fn immortal_seeded_accumulator_credits_the_entry_edge() {
    // bb0: %0 = "" (immortal, excluded); Jump bb1(%0)
    // bb1(%1): Branch %4 ? bb2 : bb3
    // bb2: %2 = Construct(); Jump bb1(%2)     (fresh accumulator value)
    // bb3: Return %1
    let func = func_with_blocks(
        5,
        vec![
            block(
                0,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(0),
                    ty: ty(0),
                    value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
                }],
                jump(1, vec![0]),
            ),
            block(1, vec![1], vec![], branch(4, 2, 3)),
            block(2, vec![], vec![construct(2, vec![])], jump(1, vec![2])),
            block(3, vec![], vec![], ret(1)),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_immortals(vec![true, false, false, false, false]);
    state_map.set_permanent_scalar(v(4));
    let (analysis, _partition) = analyze(&func, &state_map);

    assert!(
        analysis.readiness.declined.is_empty(),
        "declined: {:?}",
        analysis.readiness.declined
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// A cross-class Jump credit into a MERGE param the function never reads
/// (a distinct-birth-site merge the partition refuses) releases at the
/// receiving block's front: the credited reference dies on arrival, so the
/// merge class verifies Clean with one placed front dec.
#[test]
fn dead_cross_class_merge_credit_releases_at_receiving_front() {
    // bb0: %0 = Construct []; %1 = Construct []; Branch %3 ? bb1 : bb2
    // bb1: jump bb3(%0)
    // bb2: jump bb3(%1)
    // bb3(%2): Return %4          (the merged str is never read)
    let func = func_with_blocks(
        5,
        vec![
            block(
                0,
                vec![],
                vec![construct(0, vec![]), construct(1, vec![])],
                branch(3, 1, 2),
            ),
            block(1, vec![], vec![], jump(3, vec![0])),
            block(2, vec![], vec![], jump(3, vec![1])),
            block(3, vec![2], vec![], ArcTerminator::Return { value: v(4) }),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    state_map.set_permanent_scalar(v(4));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let merged = class_rep(&mut partition, 2);
    assert_eq!(verdict_for(&analysis, merged), ClassVerdict::Clean);
    let ops = ops_for(&analysis, merged);
    assert_eq!(ops.len(), 1, "one front dec: {ops:?}");
    assert!(
        matches!(
            ops[0],
            PlannedOp {
                slot: PlanSlot::BlockFront { block: 3 },
                kind: PlannedOpKind::Dec,
                ..
            }
        ),
        "front dec at the receiving block: {ops:?}"
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// A LOOP-INVARIANT class (no events inside any CFG cycle; the value
/// crosses the loop by dominance, not threading) keeps full-closure
/// liveness: no release lands inside the loop, and the post-loop consume
/// is the single balanced hand-off.
#[test]
fn loop_invariant_dominance_class_survives_the_loop_unreleased() {
    // bb0: %0 = Construct []; jump bb1
    // bb1: Branch %1 ? bb2 : bb3     (loop header)
    // bb2: jump bb1                  (loop body, no class events)
    // bb3: %2 = Apply @f(%0 [own]); Return %3
    let func = func_with_blocks(
        4,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![])),
            block(1, vec![], vec![], branch(1, 2, 3)),
            block(2, vec![], vec![], jump(1, vec![])),
            block(
                3,
                vec![],
                vec![apply(2, vec![(0, ArgOwnership::Owned)])],
                ArcTerminator::Return { value: v(3) },
            ),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    state_map.set_permanent_scalar(v(3));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let alloc = class_rep(&mut partition, 0);
    assert_eq!(verdict_for(&analysis, alloc), ClassVerdict::Clean);
    let ops = ops_for(&analysis, alloc);
    assert!(
        ops.iter()
            .all(|op| op.slot.block() != 2 && op.slot.block() != 1),
        "no release inside the loop: {ops:?}"
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// A consume FUNDED by a planned inc, followed by a terminator borrow-read
/// in the SAME block, still owes one release: the funded reference survives
/// the read and dies on the outgoing edges, so the plan places front decs
/// at both successors and the class verifies Clean.
#[test]
fn funded_consume_then_terminator_read_releases_at_successor_fronts() {
    // bb0: %0 = Construct []; %1 = Construct [%0]   (consumes %0, %0 live after)
    //      Invoke @f(%0 [borrow]) normal bb1 unwind bb2
    // bb1: Return %3 (scalar)
    // bb2: Resume
    let func = func_with_blocks(
        4,
        vec![
            block(
                0,
                vec![],
                vec![construct(0, vec![]), construct(1, vec![0])],
                invoke(2, vec![(0, ArgOwnership::Borrowed)], 1, 2),
            ),
            block(1, vec![], vec![], ArcTerminator::Return { value: v(3) }),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    state_map.set_permanent_scalar(v(3));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let stored = class_rep(&mut partition, 0);
    assert_eq!(verdict_for(&analysis, stored), ClassVerdict::Clean);
    let ops = ops_for(&analysis, stored);
    let incs = ops
        .iter()
        .filter(|op| op.kind == PlannedOpKind::Inc)
        .count();
    let front_decs = ops
        .iter()
        .filter(|op| {
            op.kind == PlannedOpKind::Dec
                && matches!(op.slot, PlanSlot::BlockFront { block } if block == 1 || block == 2)
        })
        .count();
    assert_eq!(incs, 1, "one funding inc: {ops:?}");
    assert_eq!(front_decs, 2, "front dec at each successor: {ops:?}");
}

/// A `Select` over two REAL allocations acquires the selected reference:
/// the planner realizes the acquisition with an RL-1 duplication inc after
/// the select, each operand class balances via its own birth + release,
/// and the select class's hand-off consume is funded — every class Clean.
#[test]
fn select_of_real_allocations_funds_the_selected_reference() {
    // bb0: %0 = Construct []; %1 = Construct []
    //      %2 = Select %3 ? %0 : %1
    //      jump bb1(%2)
    // bb1(%4): Return %5 (scalar)
    let func = func_with_blocks(
        6,
        vec![
            block(
                0,
                vec![],
                vec![
                    construct(0, vec![]),
                    construct(1, vec![]),
                    ArcInstr::Select {
                        dst: v(2),
                        ty: ty(0),
                        cond: v(3),
                        true_val: v(0),
                        false_val: v(1),
                    },
                ],
                jump(1, vec![2]),
            ),
            block(1, vec![4], vec![], ArcTerminator::Return { value: v(5) }),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    state_map.set_permanent_scalar(v(5));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let selected = class_rep(&mut partition, 2);
    assert_eq!(verdict_for(&analysis, selected), ClassVerdict::Clean);
    let ops = ops_for(&analysis, selected);
    assert!(
        ops.iter().any(|op| op.kind == PlannedOpKind::Inc
            && matches!(op.slot, PlanSlot::AfterBody { block: 0, index: 2 })
            && op.var == v(2)),
        "realizing inc after the select: {ops:?}"
    );
    for operand in [0u32, 1] {
        let class = class_rep(&mut partition, operand);
        assert_eq!(
            verdict_for(&analysis, class),
            ClassVerdict::Clean,
            "operand %{operand} class"
        );
    }
    assert!(analysis.readiness.all_classes_clean);
}

/// A cross-class credit entering a loop-THREADED class releases at the
/// CYCLE EXIT, never inside the loop: a header-front dec would fire again
/// on every back-edge re-entry (double-free), so the credited reference
/// stays live through the cycle and dies at the single-pred exit block.
#[test]
fn loop_threaded_credit_releases_at_the_cycle_exit() {
    // %0: owned param (no birth site) -> jump bb1(%0) is a CROSS-CLASS
    // credit into the refused merge param %1.
    // bb1(%1): Branch %4 ? bb2 : bb3
    // bb2: jump bb1(%1)               (back-edge, same-class silent)
    // bb3: Return %5 (scalar)         (single-pred cycle exit)
    let mut func = func_with_blocks(
        6,
        vec![
            block(0, vec![], vec![], jump(1, vec![0])),
            block(1, vec![1], vec![], branch(4, 2, 3)),
            block(2, vec![], vec![], jump(1, vec![1])),
            block(3, vec![], vec![], ArcTerminator::Return { value: v(5) }),
        ],
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Owned,
    }];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(4));
    state_map.set_permanent_scalar(v(5));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let threaded = class_rep(&mut partition, 1);
    assert_eq!(verdict_for(&analysis, threaded), ClassVerdict::Clean);
    let ops = ops_for(&analysis, threaded);
    assert_eq!(ops.len(), 1, "one exit release: {ops:?}");
    assert!(
        matches!(
            ops[0],
            PlannedOp {
                slot: PlanSlot::BlockFront { block: 3 },
                kind: PlannedOpKind::Dec,
                ..
            }
        ),
        "front dec at the cycle exit: {ops:?}"
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// A loop-invariant class READ inside the cycle (born outside, borrowed
/// by an in-loop call every iteration) keeps full-closure liveness: no
/// release lands on the read's in-loop successors; the value dies on the
/// loop-exit edge.
#[test]
fn loop_invariant_class_read_inside_the_cycle_survives_iterations() {
    // bb0: %0 = Construct []; jump bb1
    // bb1: Branch %1 ? bb2 : bb3        (loop header)
    // bb2: Invoke @f(%0 [borrow]) normal bb4 unwind bb5
    // bb4: jump bb1                     (back-edge)
    // bb5: Resume
    // bb3: Return %3                    (loop exit)
    let func = func_with_blocks(
        4,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![])),
            block(1, vec![], vec![], branch(1, 2, 3)),
            block(
                2,
                vec![],
                vec![],
                invoke(2, vec![(0, ArgOwnership::Borrowed)], 4, 5),
            ),
            block(3, vec![], vec![], ArcTerminator::Return { value: v(3) }),
            block(4, vec![], vec![], jump(1, vec![])),
            block(5, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    state_map.set_permanent_scalar(v(2));
    state_map.set_permanent_scalar(v(3));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let invariant = class_rep(&mut partition, 0);
    assert_eq!(verdict_for(&analysis, invariant), ClassVerdict::Clean);
    let ops = ops_for(&analysis, invariant);
    assert!(
        ops.iter()
            .all(|op| op.slot.block() != 4 && op.slot.block() != 1),
        "no release inside the loop: {ops:?}"
    );
    assert!(analysis.readiness.all_classes_clean);
}

// struct_list_field flagship — the NEW per-field-class mechanism carries the
// 2+-owned-field aggregate-move cluster (the evidence-bar positive pin: the
// per-field classes verify Clean with extraction funding and the hazard set
// empties, so the replacement gate accepts; the legacy whole-var admission
// never runs on the replaced path).

/// The flagship shape reduced: two fresh heap fields moved into an aggregate
/// container, each read back through an alias-hop Project. Field congruence
/// joins the Project dsts with their field-source classes; the extraction
/// funding cure plans a seed inc per member read; every class verifies
/// Clean and no field-view hazard survives.
#[test]
fn struct_list_field_flagship_per_field_classes_replace() {
    // %0 = Construct List(...)      (items buffer)
    // %1 = Construct Str-ish        (label allocation, modeled as Construct)
    // %2 = Construct Struct(%0, %1) (container)
    // %3 = Let %2 (alias)
    // %4 = Project %3.0 (items view)  %5 = IsShared %4 (read)
    // %6 = Project %3.1 (label view)  %7 = IsShared %6 (read)
    let mut func = one_block_func(
        8,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            construct(2, vec![0, 1]),
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::Var(v(2)),
            },
            ArcInstr::Project {
                dst: v(4),
                ty: ty(0),
                value: v(3),
                field: 0,
            },
            is_shared(5, 4),
            ArcInstr::Project {
                dst: v(6),
                ty: ty(0),
                value: v(3),
                field: 1,
            },
            is_shared(7, 6),
        ],
        ret(5),
    );
    func.params = vec![];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(5));
    state_map.set_permanent_scalar(v(7));
    let (analysis, mut partition) = analyze(&func, &state_map);

    // Field congruence: each Project dst joins its field-source class.
    let items_class = class_rep(&mut partition, 0);
    let view_class = class_rep(&mut partition, 4);
    assert_eq!(
        items_class, view_class,
        "alias-hop Project composes into the field class"
    );
    let label_class = class_rep(&mut partition, 1);
    let label_view = class_rep(&mut partition, 6);
    assert_eq!(label_class, label_view);

    // Every per-field class verifies Clean and the hazard set is empty —
    // the replacement gate accepts on these terms (the positive pin that
    // the NEW mechanism, not the legacy whole-var admission, carries the
    // flagship: on replaced functions the legacy emission is skipped).
    assert!(analysis.readiness.all_classes_clean);
    assert!(
        !analysis.field_view_hazard,
        "extraction funding cures every endangered view"
    );
    assert_eq!(verdict_for(&analysis, items_class), ClassVerdict::Clean);
    assert_eq!(verdict_for(&analysis, label_class), ClassVerdict::Clean);
}

/// A member EXTRACTED from a released container and moved OUT via a SECOND
/// container's `Construct` arg (a `ConstructArg` transferring terminal use,
/// distinct from the sibling test's `Return` sink): the base plan ALREADY
/// funds the second hand-off with a duplication `Inc` (one birth + one
/// planned inc = two references, one per released container's drop), so the
/// funded-move-in refinement recognizes the class as covered — NO hazard,
/// NO cure re-book — and the ORIGINAL container's release stays a plain
/// whole-var `Dec`. Every class verifies Clean.
#[test]
fn extract_then_move_out_via_second_container_funds_itself_at_extraction() {
    // %0 = Construct payload
    // %1 = Construct tuple(%0)      (first container — the one released)
    // %2 = Project %1.0             (extract member)
    // %3 = Construct holder(%2)     (move member OUT to a second container)
    // %4 = Let %1 (alias keeping the first container eventful)
    // ret %5 (scalar)
    let mut func = one_block_func(
        6,
        vec![
            construct(0, vec![]),
            construct(1, vec![0]),
            ArcInstr::Project {
                dst: v(2),
                ty: ty(0),
                value: v(1),
                field: 0,
            },
            construct(3, vec![2]),
            ArcInstr::Let {
                dst: v(4),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
            is_shared(5, 4),
        ],
        ret(5),
    );
    func.params = vec![];
    func.var_types[2] = Idx::STR;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(5));
    let (analysis, mut partition) = analyze(&func, &state_map);

    // Deterministic outcome (never the fail-closed decline for this shape):
    // pin the exact verdict rather than the permissive "hazard OR clean"
    // disjunction the prior version of this test used, which silently
    // accepted a regression that stopped the funding from landing.
    assert!(
        !analysis.field_view_hazard,
        "the funded ConstructArg-sink move-out must pass unmarked, not decline"
    );
    assert!(analysis.readiness.all_classes_clean);

    let container = class_rep(&mut partition, 1);
    let payload = class_rep(&mut partition, 0);
    assert_eq!(
        ops_for(&analysis, payload),
        vec![inc(PlanSlot::BeforeBody { block: 0, index: 1 }, 0)],
        "the base plan's duplication inc funds the second hand-off"
    );
    assert_eq!(
        ops_for(&analysis, container),
        vec![dec(PlanSlot::AfterBody { block: 0, index: 5 }, 4)],
        "the container's own release stays a plain whole-var Dec"
    );
}

/// The branch-exclusive FULL MOVE: one arm projects the aggregate's ONLY
/// owned field into a new `Construct` (the rebuild), the other arm hands
/// the aggregate itself to an owned consumer. Without the full-move rebook
/// the per-path owed counts disagree at the merge (the move arm's Reads
/// leave the count at 1); with it, the move arm's Reads become the
/// aggregate's move-out Consume (RL-2 `ConstructArg` transfer, the
/// full-skip cell of `FD_skipset_sound`) and the field view takes an
/// extraction CREDIT — no duplication inc, no hazard, every class Clean.
/// Builder for the branch-exclusive full-move diamond: bb1 projects the
/// pair's only owned field into a new `Construct`; bb2 hands the pair to an
/// owned consumer; both merge at bb3.
fn branch_exclusive_full_move_func() -> ArcFunction {
    let mut func = func_with_blocks(
        8,
        vec![
            block(
                0,
                vec![],
                vec![
                    construct(0, vec![]),
                    construct(1, vec![0]),
                    ArcInstr::Let {
                        dst: v(2),
                        ty: ty(0),
                        value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                    },
                ],
                branch(2, 1, 2),
            ),
            block(
                1,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(3),
                        ty: ty(70),
                        value: v(1),
                        field: 0,
                    },
                    construct(4, vec![3]),
                ],
                jump(3, vec![]),
            ),
            block(
                2,
                vec![],
                vec![apply(5, vec![(1, ArgOwnership::Owned)])],
                jump(3, vec![]),
            ),
            block(
                3,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(6),
                    ty: ty(0),
                    value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                }],
                ret(6),
            ),
        ],
    );
    func.var_types[3] = ty(70);

    func
}

#[test]
fn branch_exclusive_full_move_rebooks_aggregate_consume() {
    use crate::lower::test_utils::registered_struct_with_burden;
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};

    let mut func = branch_exclusive_full_move_func();
    let pair_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_burden(
        &mut registry,
        "MovedPair",
        pair_idx,
        Some(UserBurdenSpec {
            self_heap_alloc: true,
            owned_fields: vec![UserOwnedField {
                field_path: vec![0],
                field_type: ty(70),
            }],
            ..UserBurdenSpec::default()
        }),
    );
    func.var_types[1] = pair_idx;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    state_map.set_permanent_scalar(v(5));
    state_map.set_permanent_scalar(v(6));
    let (analysis, mut partition) = analyze_with_registry(&func, &state_map, &registry);

    assert!(
        !analysis.field_view_hazard,
        "the full-move rebook + extraction credit must leave no hazard; \
         declined={:?} verdicts={:?}",
        analysis.readiness.declined, analysis.readiness.verdicts,
    );
    assert!(analysis.readiness.all_classes_clean);

    let pair = class_rep(&mut partition, 1);
    let field = class_rep(&mut partition, 0);
    let pair_move_arm_release = ops_for(&analysis, pair).iter().any(|op| {
        op.kind == PlannedOpKind::Dec
            && matches!(
                op.slot,
                PlanSlot::BlockFront { block: 1 }
                    | PlanSlot::BeforeBody { block: 1, .. }
                    | PlanSlot::AfterBody { block: 1, .. }
            )
    });
    assert!(
        !pair_move_arm_release,
        "the moved aggregate takes NO release on the full-move arm (its \
         reference transferred into the rebuild construct)"
    );
    assert!(
        ops_for(&analysis, field)
            .iter()
            .all(|op| op.kind != PlannedOpKind::Inc),
        "the moved field takes NO duplication inc (the extraction credit \
         re-acquires the transferred reference for free)"
    );
}

/// Builder for the multi-owed diamond: a call result re-acquires the SAME
/// allocation (RL-34 Credit) past which the class's books owe two
/// references (birth + credit), both dying past the final terminator read.
fn multi_owed_func() -> ArcFunction {
    let mut func = func_with_blocks(
        3,
        vec![
            block(
                0,
                vec![],
                vec![construct(0, vec![])],
                invoke(1, vec![(0, ArgOwnership::Borrowed)], 1, 2),
            ),
            block(
                1,
                vec![],
                vec![],
                invoke(2, vec![(1, ArgOwnership::Borrowed)], 3, 4),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(3, vec![], vec![], ret(2)),
            block(4, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.params = vec![];

    func
}

/// The credited event stream the multi-owed diamond's classifier books
/// (the RL-34 result re-acquisition); `grounded` sets whether every
/// positive entry is certified a REAL runtime acquisition.
fn multi_owed_events(grounded: bool) -> ClassEvents {
    ClassEvents {
        origin: Some(ClassOrigin::Fresh),
        threads_back_edge: false,
        container_held: false,
        externally_funded: false,
        books_runtime_grounded: grounded,
        per_block: vec![
            vec![
                ClassEvent {
                    site: EventSite::Body(0),
                    kind: EventKind::Birth,
                    var: Some(v(0)),
                    delta: 1,
                    floor: 0,
                },
                ClassEvent {
                    site: EventSite::Terminator,
                    kind: EventKind::Read,
                    var: Some(v(0)),
                    delta: 0,
                    floor: 1,
                },
            ],
            vec![
                ClassEvent {
                    site: EventSite::BlockEntry,
                    kind: EventKind::Credit,
                    var: Some(v(1)),
                    delta: 1,
                    floor: 0,
                },
                ClassEvent {
                    site: EventSite::Terminator,
                    kind: EventKind::Read,
                    var: Some(v(1)),
                    delta: 0,
                    floor: 1,
                },
            ],
            vec![],
            vec![],
            vec![],
        ],
    }
}

/// Runtime-GROUNDED books owing two REAL references (birth + RL-34
/// credit): the release planner places one front dec per owed reference
/// at each successor past the terminator read, and the plan verifies
/// Clean — the panic-always ttr callee shape.
#[test]
fn multi_owed_grounded_books_place_one_dec_per_reference() {
    let func = multi_owed_func();
    let credited = multi_owed_events(true);
    let preds = crate::graph::compute_predecessors(&func);
    let regions = super::emit::CycleRegions::compute(&func);
    let outcome = super::emit::plan_class(&func, &preds, &regions, &credited, &[]);
    let ClassOutcome::Planned(ops) = &outcome else {
        panic!("grounded multi-owed books must plan, got {outcome:?}");
    };
    let verdict = verify_class(&func, &preds, &credited, ops);
    assert_eq!(verdict, ClassVerdict::Clean, "not clean: ops={ops:?}");
    let normal_front_decs = ops
        .iter()
        .filter(|op| {
            op.kind == PlannedOpKind::Dec && matches!(op.slot, PlanSlot::BlockFront { block: 3 })
        })
        .count();
    assert_eq!(
        normal_front_decs, 2,
        "TWO owed references each take a front dec on the normal-path \
         successor: ops={ops:?}"
    );
}

/// UNGROUNDED (cure-inflated) books owing two references DECLINE
/// fail-closed (`UnplaceableRelease`): a cure / force-owned re-extraction
/// deliberately inflates the books past the runtime count, so counting
/// book residue as runtime references over-releases (the stash-and-return
/// double-free).
#[test]
fn multi_owed_ungrounded_books_decline_fail_closed() {
    let func = multi_owed_func();
    let credited = multi_owed_events(false);
    let preds = crate::graph::compute_predecessors(&func);
    let regions = super::emit::CycleRegions::compute(&func);
    let outcome = super::emit::plan_class(&func, &preds, &regions, &credited, &[]);
    let ClassOutcome::Declined(reason) = &outcome else {
        panic!("ungrounded books-owe-two must decline fail-closed, got {outcome:?}");
    };
    assert_eq!(
        *reason,
        DeclineReason::UnplaceableRelease,
        "the ungrounded multi-owed decline is the UnplaceableRelease gate"
    );
}

/// Over-fire negative for the full-move arm (the loop-header-merge-read
/// shape): a `Jump` edge feeding TWO params from ONE class (the aggregate
/// and its alias) means the lineages may alias one runtime allocation —
/// `moved_class_shares_edge_source` must DECLINE the arm; rebooking there
/// releases a field the surviving lineage still reads (use-after-free).
#[test]
fn shared_edge_source_declines_full_move_arm() {
    use crate::lower::test_utils::registered_struct_with_burden;
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};

    let mut func = func_with_blocks(
        9,
        vec![
            block(
                0,
                vec![],
                vec![
                    construct(0, vec![]),
                    construct(1, vec![0]),
                    ArcInstr::Let {
                        dst: v(2),
                        ty: ty(0),
                        value: ArcValue::Var(v(1)),
                    },
                ],
                jump(1, vec![1, 2]),
            ),
            block(
                1,
                vec![3, 4],
                vec![
                    ArcInstr::Project {
                        dst: v(5),
                        ty: ty(70),
                        value: v(3),
                        field: 0,
                    },
                    construct(6, vec![5]),
                ],
                jump(2, vec![4]),
            ),
            block(
                2,
                vec![7],
                vec![apply(8, vec![(7, ArgOwnership::Owned)])],
                ret(8),
            ),
        ],
    );
    func.var_types[5] = ty(70);
    let pair_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_burden(
        &mut registry,
        "AliasedPair",
        pair_idx,
        Some(UserBurdenSpec {
            self_heap_alloc: true,
            owned_fields: vec![UserOwnedField {
                field_path: vec![0],
                field_type: ty(70),
            }],
            ..UserBurdenSpec::default()
        }),
    );
    for var in [1u32, 2, 3, 4, 7] {
        func.var_types[var as usize] = pair_idx;
    }
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(8));
    let (_analysis, mut partition) = analyze_with_registry(&func, &state_map, &registry);

    let arms = super::events::detect_full_move_arms(&func, &mut partition, &registry);
    assert!(
        arms.is_empty(),
        "a Jump edge feeding two params from one class must decline the \
         full-move arm (runtime aliasing across per-source lineages)"
    );
}

/// One inner shared by TWO released containers with NO extraction (the
/// two-wrappers-share-one-inner shape): both stores are move-ins — the
/// first funded by the birth, the second by the base plan's duplication
/// `Inc` — and each wrapper's own drop is the matched release. The
/// funded-move-in refinement leaves the view unmarked: no hazard, no cure,
/// every class Clean.
#[test]
fn shared_inner_two_released_containers_funded_no_hazard() {
    let mut func = one_block_func(
        7,
        vec![
            construct(0, vec![]),
            construct(1, vec![0]),
            construct(2, vec![0]),
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
            is_shared(4, 3),
            ArcInstr::Let {
                dst: v(5),
                ty: ty(0),
                value: ArcValue::Var(v(2)),
            },
            is_shared(6, 5),
        ],
        ret(6),
    );
    func.params = vec![];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(4));
    state_map.set_permanent_scalar(v(6));
    let (analysis, mut partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "both move-ins are funded (birth + planned dup inc); declined={:?} \
         verdicts={:?}",
        analysis.readiness.declined, analysis.readiness.verdicts,
    );
    assert!(analysis.readiness.all_classes_clean);

    let payload = class_rep(&mut partition, 0);
    let payload_incs = ops_for(&analysis, payload)
        .iter()
        .filter(|op| op.kind == PlannedOpKind::Inc)
        .count();
    assert_eq!(
        payload_incs, 1,
        "exactly one duplication inc funds the second wrapper's store"
    );
}

#[test]
fn extract_then_move_out_decomposes_container_release() {
    // The member is extracted then moved out through the Return — a consume
    // mark under exactly ONE released container. The decomposition cure
    // flips the container's Dec to DecPartial(skip = [0]) and re-books the
    // view without its move-in store (PV-6 / IA-T6): the transferee owns
    // the payload, the container's glue skips it.
    let mut func = one_block_func(
        5,
        vec![
            construct(0, vec![]),
            construct(1, vec![0]),
            ArcInstr::Project {
                dst: v(2),
                ty: ty(0),
                value: v(1),
                field: 0,
            },
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
            is_shared(4, 3),
        ],
        ret(2),
    );
    func.params = vec![];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(4));
    let (analysis, mut partition) = analyze(&func, &state_map);

    // Fail-closed floor: never a silent acceptance with non-clean classes.
    assert!(
        analysis.field_view_hazard || analysis.readiness.all_classes_clean,
        "cured acceptance without clean verdicts"
    );
    assert!(
        !analysis.field_view_hazard,
        "single-container consume-marked view must decompose, not decline"
    );
    // The decomposition must be visible: a DecPartial on the container's
    // class skipping exactly the consume-marked field index 0.
    let container_node = partition.register_node(v(1), FieldPath::whole_var());
    let container_rep = partition.rep_of(container_node);
    let mut saw_partial = false;
    for plan in &analysis.plan.classes {
        let ClassOutcome::Planned(ops) = &plan.outcome else {
            continue;
        };
        for op in ops {
            if let PlannedOpKind::DecPartial { skip_fields } = &op.kind {
                saw_partial = true;
                assert_eq!(skip_fields.as_slice(), &[0], "skip set != consume marks");
                assert_eq!(
                    partition.rep_of(plan.class),
                    container_rep,
                    "DecPartial planned off the container class"
                );
            }
        }
    }
    assert!(
        saw_partial,
        "consume-marked view cleared without the per-field decomposition"
    );
}

/// A demand-endangered view (field borrowed and READ while the container
/// is locally released) cures via extraction funding: the seed inc after
/// the `Project` funds the read, and the view's single owed reference
/// releases after its last read.
#[test]
fn demand_endangered_view_cures_via_extraction_funding() {
    // %0 = Let "first" (str literal, moved into the pair)
    // %1 = Construct pair(%0)
    // %2 = Project %1.0            (the borrowed view)
    // %3 = Apply eq(%2 [borrow])   (the read)
    // %4 = IsShared %1             (container last use; container then dies)
    // ret %3
    let mut func = one_block_func(
        5,
        vec![
            ArcInstr::Let {
                dst: v(0),
                ty: Idx::STR,
                value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
            },
            construct(1, vec![0]),
            ArcInstr::Project {
                dst: v(2),
                ty: Idx::STR,
                value: v(1),
                field: 0,
            },
            apply(3, vec![(2, ArgOwnership::Borrowed)]),
            is_shared(4, 1),
        ],
        ret(3),
    );
    func.var_types[0] = Idx::STR;
    func.var_types[2] = Idx::STR;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    state_map.set_permanent_scalar(v(4));
    let (analysis, _partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "a demand-endangered borrow view must cure via extraction funding"
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// Same demand-endangered shape, but the read goes through a `Let` ALIAS of
/// the extracted view: the seed still funds that read (the alias names the
/// seed-funded reference), so the move-in consume takes NO second funding
/// inc and the single owed reference releases after the aliased read.
#[test]
fn demand_endangered_view_alias_read_cures_without_double_funding() {
    // %0 = Let "first" (str literal, moved into the pair)
    // %1 = Construct pair(%0)
    // %2 = Project %1.0            (the borrowed view)
    // %3 = Let Var(%2)             (alias of the view)
    // %4 = Apply eq(%3 [borrow])   (the read, through the alias)
    // %5 = IsShared %1             (container last use)
    // ret %4
    let mut func = one_block_func(
        6,
        vec![
            ArcInstr::Let {
                dst: v(0),
                ty: Idx::STR,
                value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
            },
            construct(1, vec![0]),
            ArcInstr::Project {
                dst: v(2),
                ty: Idx::STR,
                value: v(1),
                field: 0,
            },
            ArcInstr::Let {
                dst: v(3),
                ty: Idx::STR,
                value: ArcValue::Var(v(2)),
            },
            apply(4, vec![(3, ArgOwnership::Borrowed)]),
            is_shared(5, 1),
        ],
        ret(4),
    );
    func.var_types[0] = Idx::STR;
    func.var_types[2] = Idx::STR;
    func.var_types[3] = Idx::STR;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(4));
    state_map.set_permanent_scalar(v(5));
    let (analysis, _partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "an alias read of the seeded view is seed-funded; the cure must land"
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// A DIVERGING function (every reachable terminal is Resume/Unreachable —
/// the uncaught-panic shape) verifies by its reachable terminals alone:
/// the message births and transfers into the panic call on both paths
/// (net 0 at every terminal), so the class is Clean — never Unprovable
/// for the mere absence of a reachable Return.
#[test]
fn diverging_function_verifies_by_reachable_terminals() {
    // bb0: %0 = Let "msg"; Invoke panic(%0 [own]) normal bb1 unwind bb2
    // bb1: Unreachable   bb2: Resume   bb3 (unreachable): Return %1
    let mut func = func_with_blocks(
        2,
        vec![
            block(
                0,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
                }],
                invoke(1, vec![(0, ArgOwnership::Owned)], 1, 2),
            ),
            block(1, vec![], vec![], ArcTerminator::Unreachable),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(3, vec![], vec![], ret(1)),
        ],
    );
    func.var_types[0] = Idx::STR;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let msg = class_rep(&mut partition, 0);
    assert_eq!(
        verdict_for(&analysis, msg),
        ClassVerdict::Clean,
        "a diverging function's balanced class verifies by its reachable terminals"
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// An apply-alias projection through a BORROWED call (`@unwrap(b) = b.inner`;
/// the caller re-acquires the box's field allocation as a same-allocation
/// CREDIT at the call's normal successor) releases the credited reference
/// after its last read.
#[test]
fn apply_alias_credit_releases_after_terminator_read() {
    // bb0: %0 = "boxed str"; %1 = Construct Box(%0); %3 = %1;
    //      Invoke unwrap(%3 [borrow]) -> %4, normal bb1, unwind bb2
    // bb1: %6 = %4; Invoke len(%6 [borrow]) -> %7, normal bb3, unwind bb4
    // bb2: Resume    bb3: Return %7    bb4: Resume
    let mut func = func_with_blocks(
        8,
        vec![
            block(
                0,
                vec![],
                vec![
                    ArcInstr::Let {
                        dst: v(0),
                        ty: Idx::STR,
                        value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
                    },
                    construct(1, vec![0]),
                    ArcInstr::Let {
                        dst: v(3),
                        ty: ty(0),
                        value: ArcValue::Var(v(1)),
                    },
                ],
                invoke(4, vec![(3, ArgOwnership::Borrowed)], 1, 2),
            ),
            block(
                1,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(6),
                    ty: Idx::STR,
                    value: ArcValue::Var(v(4)),
                }],
                invoke(7, vec![(6, ArgOwnership::Borrowed)], 3, 4),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(3, vec![], vec![], ret(7)),
            block(4, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.var_types[0] = Idx::STR;
    func.var_types[4] = Idx::STR;
    func.var_types[6] = Idx::STR;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(7));
    let mut aliases: FxHashMap<
        ArcVarId,
        crate::aims::intraprocedural::state_map::ApplyAliasSource,
    > = FxHashMap::default();
    aliases.insert(
        v(4),
        crate::aims::intraprocedural::state_map::ApplyAliasSource::Project {
            arg: v(3),
            field: 0,
        },
    );
    state_map.set_apply_result_aliases(aliases);
    let (analysis, _partition) = analyze(&func, &state_map);

    assert!(
        analysis.readiness.all_classes_clean,
        "the credited apply-alias reference must plan a post-read release; \
         declined={:?} verdicts={:?} plans={:?}",
        analysis.readiness.declined, analysis.readiness.verdicts, analysis.plan.classes,
    );
    assert!(
        !analysis.field_view_hazard,
        "a SELF-funded view (real floors, verified Clean) is not endangered \
         by the container's release - its credited reference survives it"
    );
}

fn eq_read(dst: u32, lhs: u32, rhs: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: v(dst),
        ty: ty(0),
        value: ArcValue::PrimOp {
            op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Eq),
            args: vec![v(lhs), v(rhs)],
        },
    }
}

fn str_lit(dst: u32, name: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: v(dst),
        ty: Idx::STR,
        value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(name))),
    }
}

/// A constructless container (call result) destructured into TWO field
/// views in bb1; the second view is read (`PrimOp` eq via a bb3-local alias)
/// only on the taken branch of the first comparison.
fn two_field_branch_read_func() -> ArcFunction {
    let mut func = func_with_blocks(
        17,
        vec![
            block(0, vec![], vec![], invoke(0, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![
                    ArcInstr::Let {
                        dst: v(2),
                        ty: ty(0),
                        value: ArcValue::Var(v(0)),
                    },
                    ArcInstr::Project {
                        dst: v(3),
                        ty: Idx::STR,
                        value: v(2),
                        field: 0,
                    },
                    ArcInstr::Project {
                        dst: v(4),
                        ty: Idx::STR,
                        value: v(2),
                        field: 1,
                    },
                    ArcInstr::Let {
                        dst: v(6),
                        ty: Idx::STR,
                        value: ArcValue::Var(v(3)),
                    },
                    str_lit(7, 21),
                    eq_read(8, 6, 7),
                ],
                branch(8, 3, 4),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(
                3,
                vec![],
                vec![
                    ArcInstr::Let {
                        dst: v(9),
                        ty: Idx::STR,
                        value: ArcValue::Var(v(4)),
                    },
                    str_lit(10, 22),
                    eq_read(11, 9, 10),
                ],
                branch(11, 6, 7),
            ),
            block(
                4,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(15),
                    ty: ty(0),
                    value: ArcValue::Literal(crate::ir::LitValue::Int(2)),
                }],
                jump(5, vec![15]),
            ),
            block(5, vec![16], vec![], ret(16)),
            block(
                6,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(12),
                    ty: ty(0),
                    value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                }],
                jump(8, vec![12]),
            ),
            block(
                7,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(13),
                    ty: ty(0),
                    value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                }],
                jump(8, vec![13]),
            ),
            block(8, vec![14], vec![], jump(5, vec![14])),
        ],
    );
    for var in [3u32, 4, 6, 7, 9, 10] {
        func.var_types[var as usize] = Idx::STR;
    }

    func
}

/// A funded view read ONLY on one branch (through a branch-local alias)
/// releases on the untaken edge named by the SEED var: the event var's def
/// is branch-local, but the seeded Project dst dominates both arms — the
/// release chooser must consider planned-op vars, not just event vars.
#[test]
fn funded_view_branch_read_releases_on_dead_edge_via_seed_var() {
    let func = two_field_branch_read_func();
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [8u32, 11, 12, 13, 14, 15, 16] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, _partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "both destructured views must cure; declined={:?} verdicts={:?}",
        analysis.readiness.declined, analysis.readiness.verdicts,
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// TWO same-class extractions funded in DISTINCT short-circuit arms, each
/// read only within its own arm: each seed's release pairs inside its arm
/// (after the last read), so every arm nets zero and the merge agrees with
/// the bypass path that never extracted.
#[test]
fn arm_local_funded_views_in_two_arms_release_within_each_arm() {
    // Constructless container (Invoke result) — the field payload has no
    // in-function birth, so the views are container-held and endangered.
    let mut func = func_with_blocks(
        9,
        vec![
            block(0, vec![], vec![], invoke(0, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    str_lit(2, 31),
                    eq_read(3, 1, 2),
                ],
                branch(3, 3, 4),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(
                3,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(4),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    str_lit(5, 32),
                    eq_read(6, 4, 5),
                ],
                jump(5, vec![6]),
            ),
            block(
                4,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(7),
                    ty: ty(0),
                    value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                }],
                jump(5, vec![7]),
            ),
            block(5, vec![8], vec![], ret(8)),
        ],
    );
    for var in [1u32, 2, 4, 5] {
        func.var_types[var as usize] = Idx::STR;
    }
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [3u32, 6, 7, 8] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, _partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "arm-local funded views must pair their releases per arm; \
         declined={:?} verdicts={:?} plans={:?}",
        analysis.readiness.declined, analysis.readiness.verdicts, analysis.plan.classes,
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// An arm-local funded view whose last read is the block TERMINATOR (a
/// borrowed `Invoke` arg) pairs its release at every successor front — the
/// normal edge too, not just the dead unwind edge — so the funded reference
/// never leaks across the downstream merge.
#[test]
fn arm_local_funded_view_terminator_read_releases_at_successor_fronts() {
    let mut func = func_with_blocks(
        6,
        vec![
            block(0, vec![], vec![], invoke(0, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::STR,
                    value: v(0),
                    field: 0,
                }],
                invoke(2, vec![(1, ArgOwnership::Borrowed)], 3, 4),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(
                3,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(3),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    str_lit(4, 32),
                    eq_read(5, 3, 4),
                ],
                ret(5),
            ),
            block(4, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    for var in [1u32, 3, 4] {
        func.var_types[var as usize] = Idx::STR;
    }
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [2u32, 5] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, _partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "a terminator-read funded view must release at both successor \
         fronts; declined={:?} verdicts={:?}",
        analysis.readiness.declined, analysis.readiness.verdicts,
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// Ops sharing one insertion point apply Inc BEFORE Dec regardless of plan
/// order: a container release and its endangered view's funding inc both
/// land after the extracting `Project`, and dec-first frees the payload the
/// inc then touches (the fund-before-release rule at a shared point).
#[test]
fn apply_plan_orders_inc_before_dec_at_shared_slot() {
    let mut func = one_block_func(
        2,
        vec![
            construct(0, vec![]),
            ArcInstr::Project {
                dst: v(1),
                ty: ty(0),
                value: v(0),
                field: 0,
            },
        ],
        ret(1),
    );
    apply_plan(
        &mut func,
        &[
            dec(PlanSlot::AfterBody { block: 0, index: 1 }, 0),
            inc(PlanSlot::AfterBody { block: 0, index: 1 }, 1),
        ],
    );
    assert_eq!(
        &func.blocks[0].body[2..4],
        &[
            ArcInstr::BurdenInc { var: v(1) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
        "the view's funding inc must precede the container's release at a shared slot"
    );
}

/// A CONSTRUCTLESS container (a call result) whose field view is extracted
/// then RETURNED: field-decomposition has no move-in store to re-book, so
/// the extraction-funding rung cures — the seed inc after the `Project` is
/// the view's own reference and the Return consume MOVES it (RL-2 transfer,
/// no borrowed-rooted duplication inc on top). The container's release
/// stays whole and no field-view hazard survives.
#[test]
fn extract_then_return_from_call_result_container_funds_at_extraction() {
    // bb0: Invoke @f() -> %0, normal bb1, unwind bb2
    // bb1: %1 = Project %0.0; Return %1
    // bb2: Resume
    let mut func = func_with_blocks(
        2,
        vec![
            block(0, vec![], vec![], invoke(0, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::STR,
                    value: v(0),
                    field: 0,
                }],
                ret(1),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.var_types[1] = Idx::STR;
    let state_map = AimsStateMap::new(&func);
    let (analysis, mut partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "constructless extract-then-return must cure via extraction funding"
    );
    assert!(analysis.readiness.all_classes_clean);

    let view_node = partition.register_node(v(1), FieldPath::whole_var());
    let view = partition.rep_of(view_node);
    assert_eq!(
        ops_for(&analysis, view),
        vec![inc(PlanSlot::AfterBody { block: 1, index: 0 }, 1)],
        "the view funds itself right after the Project; the Return transfer needs no further inc"
    );
}

/// Same shape as `extract_then_move_out_decomposes_container_release`, but
/// run through `attempt_replacement` with a container type REGISTERED in the
/// `TypeRegistry` with a real owned-field surface (field 0 named, per
/// `registered_struct_with_two_owned_str_fields`). The prior test proves the
/// cure COMPUTES a `DecPartial(skip=[0])`; this test proves that computed
/// skip set actually clears `replace::dec_partial_skips_valid` and drives a
/// real end-to-end replacement — the flagship claim
/// (`struct_list_field_flagship_per_field_classes_replace`'s "the replacement
/// gate accepts on these terms") was previously asserted only via the
/// lower-level `analyze()` helper, which never calls the gate at all.
#[test]
fn field_decomposition_cure_replaces_end_to_end_with_registered_burden() {
    use crate::lower::test_utils::registered_struct_with_two_owned_str_fields;

    let struct_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_two_owned_str_fields(&mut registry, "Pair", struct_idx);

    let mut func = one_block_func(
        5,
        vec![
            construct(0, vec![]),
            construct(1, vec![0]),
            ArcInstr::Project {
                dst: v(2),
                ty: ty(0),
                value: v(1),
                field: 0,
            },
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
            is_shared(4, 3),
        ],
        ret(2),
    );
    func.params = vec![];
    // The container's real type flows to every alias sharing its allocation
    // (a `Let { Var }` rename never changes type) — v(1) the Construct dst
    // AND v(3) the Let-Var alias the last read walks off both carry the
    // struct's Idx; the synthetic per-index `ty(n)` fixture default does not
    // reflect that invariant, so both slots are overridden explicitly.
    func.var_types[1] = struct_idx;
    func.var_types[3] = struct_idx;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(4));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let mut replaced = func;
    let outcome = attempt_replacement(
        &mut replaced,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(
        outcome.mode,
        EmissionMode::Replaced,
        "fallback_reason={:?} readiness={:?}",
        outcome.fallback_reason,
        outcome.analysis.readiness
    );
    assert!(outcome.fallback_reason.is_none());
    assert!(replaced.class_ledger_emission);
    assert!(
        replaced.blocks[0].body.iter().any(|instr| matches!(
            instr,
            ArcInstr::BurdenDecPartial { skip_fields, .. }
                if skip_fields.as_slice() == [0]
        )),
        "the container's whole-var release lowered to a field-skipping partial dec: {:?}",
        replaced.blocks[0].body
    );
}

/// Negative sibling of `field_decomposition_cure_replaces_end_to_end_with_
/// registered_burden`: same consume-marked-then-Return shape, but the
/// container's registered burden names ONLY field 1 as owned
/// (`registered_struct_value_heap_mixed`) while the extracted member is
/// field 0. The cure still computes `DecPartial(skip=[0])` from the
/// partition's consume marks alone (it never consults the registry); the
/// skip index then falls OUTSIDE the container's named owned-field surface,
/// so `replace::dec_partial_skips_valid` rejects it and `attempt_replacement`
/// falls back with `FieldDecompositionShape` rather than committing a plan
/// whose interior field walk would silently mis-skip at runtime.
#[test]
fn field_decomposition_cure_declines_replacement_on_skip_field_mismatch() {
    use crate::lower::test_utils::registered_struct_value_heap_mixed;

    let struct_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_value_heap_mixed(&mut registry, "Mixed", struct_idx);

    let mut func = one_block_func(
        5,
        vec![
            construct(0, vec![]),
            construct(1, vec![0]),
            ArcInstr::Project {
                dst: v(2),
                ty: ty(0),
                value: v(1),
                field: 0,
            },
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
            is_shared(4, 3),
        ],
        ret(2),
    );
    func.params = vec![];
    func.var_types[1] = struct_idx;
    func.var_types[3] = struct_idx;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(4));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let mut gated = func.clone();
    let outcome = attempt_replacement(
        &mut gated,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(
        outcome.fallback_reason,
        Some(FallbackReason::FieldDecompositionShape)
    );
    assert!(!gated.class_ledger_emission);
    assert_eq!(gated, func, "gate rejection leaves the function untouched");
}

#[test]
fn demand_only_view_is_never_skipped() {
    // The member is only READ after extraction — demand-endangered, never
    // consume-marked. Over-skip is the leak cell IA-T6 rejects: no planned
    // DecPartial may exist for this shape whatever cure ran.
    let mut func = one_block_func(
        5,
        vec![
            construct(0, vec![]),
            construct(1, vec![0]),
            ArcInstr::Project {
                dst: v(2),
                ty: ty(0),
                value: v(1),
                field: 0,
            },
            is_shared(3, 2),
            ArcInstr::Let {
                dst: v(4),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
        ],
        ret(3),
    );
    func.params = vec![];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    let (analysis, _partition) = analyze(&func, &state_map);

    for plan in &analysis.plan.classes {
        let ClassOutcome::Planned(ops) = &plan.outcome else {
            continue;
        };
        assert!(
            !ops.iter()
                .any(|op| matches!(op.kind, PlannedOpKind::DecPartial { .. })),
            "a merely-read view was skipped (over-skip = leak)"
        );
    }
}

/// A sum container whose extracted payload type carries NO burden (an
/// iterator handle: freed by destructor, not refcount) cannot be cured by
/// extraction funding — the seed inc is physically inert, so the container's
/// release still destroys the extracted payload. The rung declines and the
/// hazard survives (fail-closed fallback to the legacy walk).
#[test]
fn unfundable_view_type_declines_extraction_funding() {
    let mut func = one_block_func(
        5,
        vec![
            construct(0, vec![]),
            ArcInstr::Construct {
                dst: v(1),
                ty: ty(0),
                ctor: CtorKind::EnumVariant {
                    enum_name: Name::from_raw(9),
                    variant: 0,
                },
                args: vec![v(0)],
            },
            ArcInstr::Project {
                dst: v(2),
                ty: ty(70),
                value: v(1),
                field: 0,
            },
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
            is_shared(4, 3),
        ],
        ret(2),
    );
    func.params = vec![];
    // The view var's TYPE is an unregistered user index: `lookup_burden`
    // resolves no burden, so a `BurdenInc` on it lowers to nothing.
    func.var_types[2] = ty(70);
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(4));
    let (analysis, _partition) = analyze(&func, &state_map);

    assert!(
        analysis.field_view_hazard,
        "an unfundable view type must survive as a hazard, never a fake cure"
    );
}

/// Same consume-marked-then-Return shape as
/// `extract_then_move_out_decomposes_container_release`, but the container is
/// a SUM type (`CtorKind::EnumVariant`) instead of a tuple: the field
/// decomposition cure declines a sum container (`hazard.sum_container`) since
/// a variant's skip is discriminant- and arm-conditional and the per-class
/// walk does not model per-arm variant state. The extraction-funding cure
/// covers the endangered view instead, so the container's own release stays a
/// plain whole-var `Dec` — never a `DecPartial`.
#[test]
fn sum_container_view_declines_field_decomposition() {
    let mut func = one_block_func(
        5,
        vec![
            construct(0, vec![]),
            ArcInstr::Construct {
                dst: v(1),
                ty: ty(0),
                ctor: CtorKind::EnumVariant {
                    enum_name: Name::from_raw(9),
                    variant: 0,
                },
                args: vec![v(0)],
            },
            ArcInstr::Project {
                dst: v(2),
                ty: Idx::STR,
                value: v(1),
                field: 0,
            },
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
            is_shared(4, 3),
        ],
        ret(2),
    );
    func.params = vec![];
    func.var_types[2] = Idx::STR;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(4));
    let (analysis, mut partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "extraction funding must cure a sum-container-endangered view"
    );
    assert!(analysis.readiness.all_classes_clean);

    let container_node = partition.register_node(v(1), FieldPath::whole_var());
    let container_rep = partition.rep_of(container_node);
    let mut saw_container_release = false;
    for plan in &analysis.plan.classes {
        let ClassOutcome::Planned(ops) = &plan.outcome else {
            continue;
        };
        if partition.rep_of(plan.class) != container_rep {
            continue;
        }
        for op in ops {
            if op.kind == PlannedOpKind::Dec {
                saw_container_release = true;
            }
            assert!(
                !matches!(op.kind, PlannedOpKind::DecPartial { .. }),
                "a sum view outside the uniform single-payload slot must \
                 never decompose: variant-conditional books are unmodeled"
            );
        }
    }
    assert!(
        saw_container_release,
        "the sum container must still get its plain whole-var release"
    );
}

/// A sum built at its sole construct site as variant 9 with ONE payload
/// (slot 1; slot 0 is the tag), matched on the tag, the payload extracted
/// and moved out to an owned consumer on the matching arm.
fn uniform_variant_sum_func() -> ArcFunction {
    let mut func = func_with_blocks(
        9,
        vec![
            block(0, vec![], vec![], invoke(0, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: ty(0),
                        ctor: CtorKind::EnumVariant {
                            enum_name: Name::from_raw(9),
                            variant: 9,
                        },
                        args: vec![v(0)],
                    },
                    ArcInstr::Project {
                        dst: v(2),
                        ty: ty(0),
                        value: v(1),
                        field: 0,
                    },
                ],
                ArcTerminator::Switch {
                    scrutinee: v(2),
                    cases: vec![(9, ArcBlockId::new(3))],
                    default: ArcBlockId::new(4),
                },
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(
                3,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(3),
                        ty: ty(70),
                        value: v(1),
                        field: 1,
                    },
                    apply(4, vec![(3, ArgOwnership::Owned)]),
                    ArcInstr::Let {
                        dst: v(5),
                        ty: ty(0),
                        value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                    },
                ],
                jump(5, vec![5]),
            ),
            block(
                4,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(6),
                    ty: ty(0),
                    value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                }],
                jump(5, vec![6]),
            ),
            block(5, vec![7], vec![], ret(7)),
        ],
    );
    // The payload type is an unregistered user index: no burden, so
    // extraction funding cannot fire — decomposition is the only cure.
    func.var_types[0] = ty(70);
    func.var_types[3] = ty(70);

    func
}

/// The explicit-tag iterator-payload shape: a sum whose EVERY construct site
/// builds the SAME single-payload variant, the payload extracted through the
/// tag match and moved out to an owned consumer. The extraction-funding cure
/// cannot fire (the payload type carries no burden), but the field
/// decomposition CAN: the container's release decomposes to
/// `DecPartial(skip = [variant ordinal])` — the tag-switched glue skips
/// exactly the moved-out variant's payload.
#[test]
fn uniform_variant_sum_payload_decomposes_container_release() {
    use crate::lower::test_utils::registered_enum_with_single_payload_variant;

    let mut func = uniform_variant_sum_func();
    let enum_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registered_enum_with_single_payload_variant(&mut registry, "BigEnum", enum_idx, 9, ty(70));
    func.var_types[1] = enum_idx;
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [2u32, 4, 5, 6, 7] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, mut partition) = analyze_with_registry(&func, &state_map, &registry);

    assert!(
        !analysis.field_view_hazard,
        "a uniform single-payload-variant sum's moved-out payload must cure \
         via decomposition; declined={:?} verdicts={:?}",
        analysis.readiness.declined, analysis.readiness.verdicts,
    );
    assert!(analysis.readiness.all_classes_clean);

    let container_node = partition.register_node(v(1), FieldPath::whole_var());
    let container_rep = partition.rep_of(container_node);
    let mut saw_variant_skip = false;
    for plan in &analysis.plan.classes {
        let ClassOutcome::Planned(ops) = &plan.outcome else {
            continue;
        };
        if partition.rep_of(plan.class) != container_rep {
            continue;
        }
        for op in ops {
            if let PlannedOpKind::DecPartial { skip_fields } = &op.kind {
                assert_eq!(
                    skip_fields,
                    &vec![9u32],
                    "the sum skip set names the VARIANT ordinal, not the slot"
                );
                saw_variant_skip = true;
            }
        }
    }
    assert!(
        saw_variant_skip,
        "the container's release must decompose to skip the moved variant"
    );
}

/// The CONSTRUCTLESS sum container: the enum arrives as a callee `Invoke`
/// result (no `Construct` in the container class to inspect), tag-switched,
/// its payload extracted and moved out on the payload arm. The variant
/// identity derives from the TYPE's burden table — exactly one
/// payload-bearing variant (`derive_constructless_enum_variant`,
/// `FD_skipset_sound` with the moved mark variant-unique by type
/// structure) — and the container's release decomposes to
/// `DecPartial(skip = [variant ordinal])` exactly as the construct-uniform
/// shape does.
/// Builder for the constructless shape: v(1) is the callee `Invoke` result
/// (no `Construct`), tag-read at field 0, payload extracted at field 1 on
/// the matched arm and moved to an owned consumer.
fn constructless_invoke_result_sum_func() -> ArcFunction {
    let mut func = func_with_blocks(
        9,
        vec![
            block(0, vec![], vec![], invoke(1, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![ArcInstr::Project {
                    dst: v(2),
                    ty: ty(0),
                    value: v(1),
                    field: 0,
                }],
                ArcTerminator::Switch {
                    scrutinee: v(2),
                    cases: vec![(9, ArcBlockId::new(3))],
                    default: ArcBlockId::new(4),
                },
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(
                3,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(3),
                        ty: ty(70),
                        value: v(1),
                        field: 1,
                    },
                    apply(4, vec![(3, ArgOwnership::Owned)]),
                    ArcInstr::Let {
                        dst: v(5),
                        ty: ty(0),
                        value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                    },
                ],
                jump(5, vec![5]),
            ),
            block(
                4,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(6),
                    ty: ty(0),
                    value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                }],
                jump(5, vec![6]),
            ),
            block(5, vec![7], vec![], ret(7)),
        ],
    );
    // The payload type is an unregistered user index: no burden, so
    // extraction funding cannot fire — decomposition is the only cure.
    func.var_types[3] = ty(70);

    func
}

#[test]
fn constructless_invoke_result_sum_decomposes_container_release() {
    use crate::lower::test_utils::registered_tagged_enum_with_unique_payload_variant;

    let mut func = constructless_invoke_result_sum_func();
    let enum_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registered_tagged_enum_with_unique_payload_variant(
        &mut registry,
        "MaybePayload",
        enum_idx,
        9,
        ty(70),
    );
    func.var_types[1] = enum_idx;
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [2u32, 4, 5, 6, 7] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, mut partition) = analyze_with_registry(&func, &state_map, &registry);

    assert!(
        !analysis.field_view_hazard,
        "a constructless unique-payload-variant sum's moved-out payload must \
         cure via the type-derived decomposition; declined={:?} verdicts={:?}",
        analysis.readiness.declined, analysis.readiness.verdicts,
    );
    assert!(analysis.readiness.all_classes_clean);

    let container_node = partition.register_node(v(1), FieldPath::whole_var());
    let container_rep = partition.rep_of(container_node);
    let mut saw_variant_skip = false;
    for plan in &analysis.plan.classes {
        let ClassOutcome::Planned(ops) = &plan.outcome else {
            continue;
        };
        if partition.rep_of(plan.class) != container_rep {
            continue;
        }
        for op in ops {
            if let PlannedOpKind::DecPartial { skip_fields } = &op.kind {
                assert_eq!(
                    skip_fields,
                    &vec![9u32],
                    "the type-derived sum skip names the VARIANT ordinal"
                );
                saw_variant_skip = true;
            }
        }
    }
    assert!(
        saw_variant_skip,
        "the constructless container's release must decompose to skip the \
         unique payload-bearing variant"
    );
}

/// The extraction guarded by an UNRELATED branch (not the container's own
/// tag): the bypass path reaches its own release with the payload never
/// extracted while the take path extracts it — the PER-SITE decomposition
/// (`FD_site_uniform_projection`) cures it: the bypass release keeps the
/// whole-var `Dec` (the recursive drop of the unmoved payload) and the
/// extraction-dominated release takes the variant skip; the view books the
/// kept store consume plus a credit at the extraction.
#[test]
fn bypassable_extraction_sum_cures_per_site() {
    let mut func = func_with_blocks(
        10,
        vec![
            block(0, vec![], vec![], invoke(0, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: ty(0),
                        ctor: CtorKind::EnumVariant {
                            enum_name: Name::from_raw(9),
                            variant: 9,
                        },
                        args: vec![v(0)],
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: ty(0),
                        value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                    },
                ],
                branch(2, 3, 4),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(
                3,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(3),
                        ty: ty(70),
                        value: v(1),
                        field: 1,
                    },
                    apply(4, vec![(3, ArgOwnership::Owned)]),
                    ArcInstr::Let {
                        dst: v(5),
                        ty: ty(0),
                        value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                    },
                ],
                jump(5, vec![5]),
            ),
            block(
                4,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(6),
                    ty: ty(0),
                    value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                }],
                jump(5, vec![6]),
            ),
            block(5, vec![7], vec![], ret(7)),
        ],
    );
    func.var_types[0] = ty(70);
    func.var_types[3] = ty(70);
    let enum_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    {
        use crate::lower::test_utils::registered_enum_with_single_payload_variant;
        registered_enum_with_single_payload_variant(&mut registry, "BigEnum", enum_idx, 9, ty(70));
    }
    func.var_types[1] = enum_idx;
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [2u32, 4, 5, 6, 7] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, mut partition) = analyze_with_registry(&func, &state_map, &registry);

    assert!(
        !analysis.field_view_hazard,
        "the per-site decomposition must cure the bypassable extraction: {:?}",
        analysis.readiness.declined
    );
    assert!(analysis.readiness.all_classes_clean);
    // Container ops split per site: at least one whole-var Dec (the bypass
    // arm) AND at least one variant-skip DecPartial (extraction-dominated).
    let container = class_rep(&mut partition, 1);
    let ops = ops_for(&analysis, container);
    let whole = ops
        .iter()
        .filter(|op| op.kind == PlannedOpKind::Dec)
        .count();
    let skipped = ops
        .iter()
        .filter(|op| matches!(&op.kind, PlannedOpKind::DecPartial { skip_fields } if skip_fields == &vec![9u32]))
        .count();
    assert!(
        whole >= 1,
        "the bypass release keeps the whole-var Dec: {ops:?}"
    );
    assert!(
        skipped >= 1,
        "the extraction release takes the variant skip: {ops:?}"
    );
}

/// A single payload extracted from one released container and re-stored into
/// a SECOND released container: the payload's view class is endangered by
/// BOTH containers (`multi_container`), so the field-decomposition cure is
/// skipped entirely for it in favor of extraction funding — a `DecPartial`
/// must never be planned off EITHER container's release for this view.
#[test]
fn multi_container_view_declines_field_decomposition() {
    // %0 = Construct payload
    // %1 = Construct tuple_a(%0)      (container A — released)
    // %2 = Project %1.0               (extract member from A)
    // %3 = Construct tuple_b(%2)      (container B — released; re-stores the
    //                                  extracted member as its own field 0,
    //                                  congruence-unioning the SAME view
    //                                  class into B's field slot too)
    // %4 = Project %3.0               (move the member out again, endangering
    //                                  the shared view under BOTH containers)
    // %5 = Let %1 (alias keeping A eventful)   %6 = IsShared %5
    // %7 = Let %3 (alias keeping B eventful)   %8 = IsShared %7
    let mut func = one_block_func(
        9,
        vec![
            construct(0, vec![]),
            construct(1, vec![0]),
            ArcInstr::Project {
                dst: v(2),
                ty: ty(0),
                value: v(1),
                field: 0,
            },
            construct(3, vec![2]),
            ArcInstr::Project {
                dst: v(4),
                ty: ty(0),
                value: v(3),
                field: 0,
            },
            ArcInstr::Let {
                dst: v(5),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
            is_shared(6, 5),
            ArcInstr::Let {
                dst: v(7),
                ty: ty(0),
                value: ArcValue::Var(v(3)),
            },
            is_shared(8, 7),
        ],
        ret(4),
    );
    func.params = vec![];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(6));
    state_map.set_permanent_scalar(v(8));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let container_a = class_rep(&mut partition, 1);
    let container_b = class_rep(&mut partition, 3);
    let view = class_rep(&mut partition, 0);
    assert_ne!(
        container_a, container_b,
        "the two containers stay distinct classes"
    );
    assert_eq!(
        class_rep(&mut partition, 2),
        view,
        "the re-extracted member composes into the shared payload class"
    );

    assert!(analysis.readiness.all_classes_clean);
    assert!(
        !analysis.field_view_hazard,
        "extraction funding must cure the shared multi-container view"
    );
    assert!(
        ops_for(&analysis, view)
            .iter()
            .any(|op| op.kind == PlannedOpKind::Inc),
        "the shared view must fund itself at an extraction site, proving \
         extraction funding (not a lucky non-hazard) cured it"
    );
    for plan in &analysis.plan.classes {
        let ClassOutcome::Planned(ops) = &plan.outcome else {
            continue;
        };
        let rep = partition.rep_of(plan.class);
        if rep != container_a && rep != container_b {
            continue;
        }
        assert!(
            ops.iter()
                .all(|op| !matches!(op.kind, PlannedOpKind::DecPartial { .. })),
            "a multi-container view must never decompose off either \
             container's release: {ops:?}"
        );
    }
}

/// A release planned for a class containing a BORROWED function param names
/// a same-class alias, never the param var itself (VF-1 rejects an `RcDec` on
/// a borrowed param; the alias is the same allocation).
#[test]
fn release_never_names_borrowed_param_var() {
    // %0: borrowed param, iter-consuming (Foreign origin — owed +1)
    // bb0: %1 = Let Var(%0); Jump bb1
    // bb1: Invoke @f() normal bb2 unwind bb3   (no class event in bb1 — the
    //      unwind release must fall back to the class-wide var scan)
    // bb2: %3 = IsShared %1; Return %3
    // bb3: Resume
    let mut func = func_with_blocks(
        4,
        vec![
            block(
                0,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(1),
                    ty: ty(0),
                    value: ArcValue::Var(v(0)),
                }],
                jump(1, vec![]),
            ),
            block(1, vec![], vec![], invoke(2, vec![], 2, 3)),
            block(2, vec![], vec![is_shared(3, 1)], ret(3)),
            block(3, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    let mut facts: FxHashMap<Name, BoundaryFacts> = FxHashMap::default();
    facts.insert(
        func.name,
        BoundaryFacts {
            param_iter_consumes: vec![true],
            param_transfers_through_return: vec![false],
            ..BoundaryFacts::default()
        },
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    state_map.set_permanent_scalar(v(3));
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let interner = test_interner();
    let classification = classify_function(&func, &state_map, &mut partition, &facts, &interner);
    let analysis = analyze_class_ledger(
        &func,
        &classification,
        &mut partition,
        &ori_types::TypeRegistry::default(),
        &interner,
    );

    let class = class_rep(&mut partition, 0);
    let ops = ops_for(&analysis, class);
    for op in &ops {
        if op.kind == PlannedOpKind::Dec {
            assert_ne!(op.var, v(0), "dec names the borrowed param: {ops:?}");
        }
    }
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
}

/// A loop-carried rebuild: the container is re-Constructed each iteration
/// from a view of the loop block-param, and the view's back-edge hand-off
/// is its LAST use — the extraction-funding seed IS the hand-off's funding,
/// so the planner adds no duplication inc (per-reference forward-only
/// pricing; another iteration's extraction is a different reference).
/// Before the pricing fix the completed plan double-funded the hand-off and
/// declined `MergeDisagree` at the loop header.
#[test]
fn loop_carried_rebuild_seeded_handoff_not_double_funded() {
    // bb0: %0 = Construct(); Jump bb1(%0)
    // bb1(%1): Branch %6 ? bb2 : bb3
    // bb2: %2 = Project %1.0 ; %3 = Construct(%2) ; Jump bb1(%3)
    // bb3: %4 = Project %1.0 ; %5 = IsShared %4 ; Return %7
    let mut func = func_with_blocks(
        8,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(1, vec![1], vec![], branch(6, 2, 3)),
            block(
                2,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(2),
                        ty: ty(0),
                        value: v(1),
                        field: 0,
                    },
                    construct(3, vec![2]),
                ],
                jump(1, vec![3]),
            ),
            block(
                3,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(4),
                        ty: ty(0),
                        value: v(1),
                        field: 0,
                    },
                    is_shared(5, 4),
                ],
                ret(7),
            ),
        ],
    );
    func.var_types[2] = ty(0);
    func.var_types[4] = ty(0);
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [5u32, 6, 7] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, _partition) = analyze(&func, &state_map);
    assert!(
        analysis.readiness.all_classes_clean,
        "loop-carried rebuild must verify clean: {:?}",
        analysis.readiness.verdicts
    );
    assert!(analysis.readiness.declined.is_empty());
    assert!(!analysis.field_view_hazard, "views must be cured");
}

/// A `FatValue` view seed stays fundable when the burden lookup cannot
/// resolve its type (a monomorphized-generic pool alias of `str`, the
/// generic-pair tuple-field shape): str/closure fat values are ALWAYS
/// refcount-managed, so the extraction-funding inc lowers unconditionally
/// and the endangered view cures instead of declining the function.
#[test]
fn fat_value_seed_fundable_without_burden_entry() {
    // bb0: Invoke @f() -> %0 (constructless container) normal bb1 unwind bb2
    // bb1: %1 = Project %0.0 (FatValue, unregistered user type); read; ret
    // bb2: Resume
    let mut func = func_with_blocks(
        4,
        vec![
            block(0, vec![], vec![], invoke(0, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: ty(64),
                        value: v(0),
                        field: 0,
                    },
                    is_shared(2, 1),
                ],
                ret(3),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.var_types[1] = ty(64);
    func.var_reprs = vec![
        crate::ir::ValueRepr::Aggregate,
        crate::ir::ValueRepr::FatValue,
        crate::ir::ValueRepr::Scalar,
        crate::ir::ValueRepr::Scalar,
    ];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    state_map.set_permanent_scalar(v(3));
    let (analysis, _partition) = analyze(&func, &state_map);
    assert!(
        !analysis.field_view_hazard,
        "FatValue seed must fund the endangered view: {:?}",
        analysis.readiness.declined
    );
    assert!(analysis.readiness.all_classes_clean);
}

/// PROBE (drain of the derive-Clone family): one Project seed whose
/// Let-aliases are handed off TWICE (two store consumes), each followed by
/// a same-block read — every hand-off of the seeded reference that the
/// reference survives takes its own duplication inc (funded via the
/// owning seed's `close_over_let_aliases` closure).
/// The two-hand-off fixture: one Project seed (`%1`), Let-aliases consumed at
/// two Construct stores (`%2 -> %3`, `%6 -> %7`), each followed by a same-block
/// `IsShared` read (`%4`, `%8`).
fn two_handoff_view_func() -> ArcFunction {
    let mut func = func_with_blocks(
        10,
        vec![
            block(0, vec![], vec![], invoke(0, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: ty(64),
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: ty(64),
                        value: ArcValue::Var(v(1)),
                    },
                    construct(3, vec![2]),
                    ArcInstr::Let {
                        dst: v(4),
                        ty: ty(64),
                        value: ArcValue::Var(v(1)),
                    },
                    is_shared(5, 4),
                ],
                jump(3, vec![]),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(
                3,
                vec![],
                vec![
                    ArcInstr::Let {
                        dst: v(6),
                        ty: ty(64),
                        value: ArcValue::Var(v(1)),
                    },
                    construct(7, vec![6]),
                    ArcInstr::Let {
                        dst: v(8),
                        ty: ty(64),
                        value: ArcValue::Var(v(1)),
                    },
                    is_shared(9, 8),
                ],
                ret(9),
            ),
        ],
    );
    for var in [1u32, 2, 4, 6, 8] {
        func.var_types[var as usize] = ty(64);
    }
    func.var_reprs = vec![
        crate::ir::ValueRepr::Aggregate,
        crate::ir::ValueRepr::FatValue,
        crate::ir::ValueRepr::FatValue,
        crate::ir::ValueRepr::Aggregate,
        crate::ir::ValueRepr::FatValue,
        crate::ir::ValueRepr::Scalar,
        crate::ir::ValueRepr::FatValue,
        crate::ir::ValueRepr::Aggregate,
        crate::ir::ValueRepr::FatValue,
        crate::ir::ValueRepr::Scalar,
    ];
    func
}

#[test]
fn seeded_view_with_two_handoffs_funds_both() {
    let func = two_handoff_view_func();
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(5));
    state_map.set_permanent_scalar(v(9));

    // Drive the cure internals directly for full plan visibility.
    let facts: FxHashMap<Name, BoundaryFacts> = FxHashMap::default();
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let interner = test_interner();
    let classification = classify_function(&func, &state_map, &mut partition, &facts, &interner);
    let view = {
        let node = partition.register_node(v(1), FieldPath::whole_var());
        partition.rep_of(node)
    };
    let funded = super::events::extract_class_events_with(
        &func,
        &classification,
        &mut partition,
        view,
        true,
    );
    let preds = crate::graph::compute_predecessors(&func);
    let regions = super::emit::CycleRegions::compute(&func);
    let seeds = vec![PlannedOp {
        slot: PlanSlot::AfterBody { block: 1, index: 0 },
        kind: PlannedOpKind::Inc,
        var: v(1),
    }];
    let outcome = super::emit::plan_class(&func, &preds, &regions, &funded, &seeds);
    let ClassOutcome::Planned(ops) = &outcome else {
        panic!(
            "cure plan declined: {outcome:?} funded={:?}",
            funded.per_block
        );
    };
    let verdict = verify_class(&func, &preds, &funded, ops);
    assert_eq!(
        verdict,
        ClassVerdict::Clean,
        "cure plan not clean: ops={ops:?} funded={:?}",
        funded.per_block
    );

    let (analysis, _partition) = analyze(&func, &state_map);
    assert!(
        !analysis.field_view_hazard,
        "two-hand-off seeded view must cure: declined={:?} plans={:?}",
        analysis.readiness.declined, analysis.plan.classes
    );
    assert!(analysis.readiness.all_classes_clean);
}
