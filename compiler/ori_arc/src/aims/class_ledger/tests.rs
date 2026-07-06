use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, FieldPath, NodeIdx};
use crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition;
use crate::aims::intraprocedural::ledger_events::{classify_function, BoundaryFacts};
use crate::aims::intraprocedural::AimsStateMap;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, CtorKind,
};
use crate::ownership::Ownership;

use super::apply::apply_plan;
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
