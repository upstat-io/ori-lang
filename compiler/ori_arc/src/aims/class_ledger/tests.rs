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
    let facts: FxHashMap<Name, BoundaryFacts> = FxHashMap::default();
    let mut partition = compute_birth_site_partition(func, state_map);
    let classification = classify_function(func, state_map, &mut partition, &facts);
    let analysis = analyze_class_ledger(func, &classification, &mut partition);
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

// Toggle

/// The toggle changes the code path: default-off makes the Step-4b dispatch
/// a no-op (no analysis, no mutation, no replacement); the enabled path
/// produces a plan on the same inputs.
#[test]
fn default_toggle_off_pipeline_entry_is_noop() {
    let func = one_block_func(1, vec![construct(0, vec![])], ret(0));
    let state_map = AimsStateMap::new(&func);
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();
    let interner = ori_ir::StringInterner::new();

    assert!(!class_ledger_emitter_enabled());
    let mut gated = func.clone();
    let replaced = pipeline_step_4b(
        &mut gated, &state_map, &contracts, &registry, &interner, false, true,
    );
    assert!(!replaced);
    assert_eq!(gated, func);

    let analysis = analyze_from_state_map(&func, &state_map, &contracts);
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
        threads_back_edge: false,
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
        threads_back_edge: false,
        per_block: vec![vec![ClassEvent {
            site: EventSite::Body(0),
            kind: EventKind::Consume,
            var: None,
            delta: -1,
            floor: 1,
        }]],
    };
    assert!(matches!(
        super::emit::plan_class(&func, &preds, &events, &[]),
        ClassOutcome::Declined(DeclineReason::UnresolvedOpVar)
    ));
}

// Passthrough refund

/// An RL-34 passthrough (consume at the call refunded by the same-site
/// credit) transfers the existing reference: no inc, net 0.
#[test]
fn passthrough_refund_needs_no_inc() {
    let callee = Name::from_raw(11);
    let func = one_block_func(
        2,
        vec![
            construct(0, vec![]),
            ArcInstr::Apply {
                dst: v(1),
                ty: ty(0),
                func: callee,
                args: vec![v(0)],
                arg_ownership: vec![crate::ir::ArgOwnership::Owned],
                mono_instance_id: None,
            },
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
    let outcome = attempt_replacement(&mut replaced, &state_map, &contracts, &registry, true);
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
    let outcome = attempt_replacement(&mut gated, &state_map, &contracts, &registry, false);
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(outcome.fallback_reason, Some("legacy-emission-disabled"));
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
    let outcome = attempt_replacement(&mut gated, &state_map, &contracts, &registry, true);
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(outcome.fallback_reason, Some("readiness-not-clean"));
    assert!(!gated.class_ledger_emission);
    assert_eq!(gated, func);
}

/// A zero-class function falls back: the class model proves nothing about
/// variables it never evented.
#[test]
fn replacement_declines_zero_class_function() {
    let func = one_block_func(1, vec![], ret(0));
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(0));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();

    let mut gated = func.clone();
    let outcome = attempt_replacement(&mut gated, &state_map, &contracts, &registry, true);
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(outcome.fallback_reason, Some("zero-classes"));
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
    let outcome = attempt_replacement(&mut gated, &state_map, &contracts, &registry, true);
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(outcome.fallback_reason, Some("user-drop-glue"));
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
                ArcTerminator::Invoke {
                    dst: v(1),
                    ty: ty(0),
                    func: Name::from_raw(7),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
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
    let outcome = attempt_replacement(&mut gated, &state_map, &contracts, &registry, true);
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(outcome.fallback_reason, Some("reuse-shape"));
    assert_eq!(gated, func);
}

/// A function whose state map carries a TRMC `ContextHole`-shaped variable
/// falls back: context-hole threading (fill-at-recursive-call) is the
/// protocol-loop surface the class model does not represent yet.
#[test]
fn replacement_declines_trmc_context_hole() {
    let func = one_block_func(1, vec![construct(0, vec![])], ret(0));
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_var_shape(v(0), crate::aims::lattice::ShapeClass::ContextHole);
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let registry = ori_types::TypeRegistry::default();

    let mut gated = func.clone();
    let outcome = attempt_replacement(&mut gated, &state_map, &contracts, &registry, true);
    assert_eq!(outcome.mode, EmissionMode::Fallback);
    assert_eq!(outcome.fallback_reason, Some("trmc-context"));
    assert_eq!(gated, func);
}

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
            ArcInstr::Apply {
                dst: v(0),
                ty: ty(0),
                func: Name::from_raw(7),
                args: vec![],
                arg_ownership: vec![],
                mono_instance_id: None,
            },
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
    let outcome = attempt_replacement(&mut gated, &state_map, &contracts, &registry, true);
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
                ArcTerminator::Invoke {
                    dst: v(1),
                    ty: ty(0),
                    func: Name::from_raw(7),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
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
