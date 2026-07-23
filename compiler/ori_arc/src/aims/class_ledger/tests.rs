use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

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
use super::events::{mutate_floor, ClassEvent, ClassEvents, EventKind};
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

#[test]
fn mutate_floor_saturates_when_sibling_read_count_reaches_carrier_limit() {
    assert_eq!(mutate_floor(1, 2), 3, "ordinary owned mutate floor");
    assert_eq!(
        mutate_floor(1, i64::MAX),
        i64::MAX,
        "owned-unit funding must not overflow the i64 floor carrier"
    );
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

fn invoke_indirect(dst: u32, closure: u32, normal: u32, unwind: u32) -> ArcTerminator {
    ArcTerminator::InvokeIndirect {
        dst: v(dst),
        ty: ty(0),
        closure: v(closure),
        args: vec![],
        arg_ownership: vec![],
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
    let interner = test_interner();
    analyze_with_registry_and_interner(func, state_map, registry, &interner)
}

fn analyze_with_registry_and_interner(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
) -> (ClassLedgerAnalysis, BirthSitePartition) {
    analyze_with_registry_interner_and_exact(
        func,
        state_map,
        registry,
        interner,
        &FxHashSet::default(),
    )
}

fn analyze_with_registry_interner_and_exact(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    exact_callables: &FxHashSet<Name>,
) -> (ClassLedgerAnalysis, BirthSitePartition) {
    // These unit fixtures bypass whole-program AIMS setup. Route
    // their synthetic bodies through the same strict primitive-fact producer
    // before any ledger consumer reads the frozen table.
    let mut func = func.clone();
    let pool = ori_types::Pool::new();
    let classifier = crate::ArcClassifier::new(&pool);
    crate::aims::freeze_primitive_facts(std::slice::from_mut(&mut func), &classifier)
        .unwrap_or_else(|errors| panic!("class-ledger primitive facts should freeze: {errors:?}"));
    let facts: FxHashMap<Name, BoundaryFacts> = FxHashMap::default();
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let mut contracts = FxHashMap::default();
    crate::aims::builtins::seed_builtin_contracts(&mut contracts, &builtins, interner);
    let mut partition = compute_birth_site_partition(&func, state_map);
    let classification = classify_function(&func, state_map, &mut partition, &facts, interner);
    let analysis = super::analysis::analyze_class_ledger_with_exact(
        &func,
        &classification,
        &mut partition,
        &contracts,
        exact_callables,
        registry,
        interner,
    );
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
/// unconditional and has no runtime opt-out).
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

/// Transfer sites and release slots in one block use instruction ordering,
/// not block-level dominance alone.
#[test]
fn boundary_field_consume_respects_same_block_ordering() {
    use super::hazard::{SiteVerdict, TransferFlowContext};

    let func = one_block_func(1, vec![construct(0, vec![])], ret(0));
    let ctx = TransferFlowContext::from_transfer_sites(&func, &[(0, EventSite::Body(0))]);
    let before = PlannedOp {
        slot: PlanSlot::BeforeBody { block: 0, index: 0 },
        kind: PlannedOpKind::Dec,
        var: v(0),
    };
    let after = PlannedOp {
        slot: PlanSlot::AfterBody { block: 0, index: 0 },
        kind: PlannedOpKind::Dec,
        var: v(0),
    };

    assert_eq!(ctx.classify(&before), Some(SiteVerdict::Whole));
    assert_eq!(ctx.classify(&after), Some(SiteVerdict::Skip));
}

/// A release before a loop-local transfer is reached both before any
/// transfer and after the loop backedge has crossed one. The site is mixed
/// and cannot safely take either a uniform skip or whole verdict.
#[test]
fn boundary_field_consume_in_cycle_declines_when_not_uniform() {
    use super::hazard::TransferFlowContext;

    let func = func_with_blocks(
        2,
        vec![
            block(0, vec![], vec![], jump(1, vec![])),
            block(1, vec![], vec![construct(0, vec![])], branch(1, 1, 2)),
            block(2, vec![], vec![], ret(0)),
        ],
    );
    let ctx = TransferFlowContext::from_transfer_sites(&func, &[(1, EventSite::Body(0))]);
    let release = PlannedOp {
        slot: PlanSlot::BeforeBody { block: 1, index: 0 },
        kind: PlannedOpKind::Dec,
        var: v(0),
    };

    assert_eq!(ctx.classify(&release), None);
}

/// `Invoke` transfers ownership before either successor executes. Normal and
/// unwind release sites therefore share the boundary skip verdict.
#[test]
fn boundary_field_consume_invoke_covers_normal_and_unwind() {
    use super::hazard::{SiteVerdict, TransferFlowContext};

    let func = func_with_blocks(
        2,
        vec![
            block(
                0,
                vec![],
                vec![construct(0, vec![])],
                invoke(1, vec![], 1, 2),
            ),
            block(1, vec![], vec![], ret(1)),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    let ctx = TransferFlowContext::from_transfer_sites(&func, &[(0, EventSite::Terminator)]);
    for block in [1usize, 2] {
        let release = PlannedOp {
            slot: PlanSlot::BlockFront { block },
            kind: PlannedOpKind::Dec,
            var: v(0),
        };
        assert_eq!(ctx.classify(&release), Some(SiteVerdict::Skip));
    }
}

/// One transfer can govern multiple release sites, but each site receives an
/// independent verdict rather than inheriting a class-global skip.
#[test]
fn boundary_field_consume_classifies_each_release_site() {
    use super::hazard::{SiteVerdict, TransferFlowContext};

    let func = func_with_blocks(
        2,
        vec![
            block(0, vec![], vec![construct(0, vec![])], branch(1, 1, 2)),
            block(1, vec![], vec![], ret(0)),
            block(2, vec![], vec![], ret(0)),
        ],
    );
    let ctx = TransferFlowContext::from_transfer_sites(&func, &[(0, EventSite::Body(0))]);
    for block in [1usize, 2] {
        let release = PlannedOp {
            slot: PlanSlot::BlockFront { block },
            kind: PlannedOpKind::Dec,
            var: v(0),
        };
        assert_eq!(ctx.classify(&release), Some(SiteVerdict::Skip));
    }
}

/// A joined release reached through both a transfer path and a bypass path
/// is mixed. A class-global skip would leak the bypass-owned field.
#[test]
fn boundary_field_consume_mixed_join_declines() {
    use super::hazard::TransferFlowContext;

    let func = func_with_blocks(
        2,
        vec![
            block(0, vec![], vec![], branch(1, 1, 2)),
            block(1, vec![], vec![construct(0, vec![])], jump(3, vec![])),
            block(2, vec![], vec![], jump(3, vec![])),
            block(3, vec![], vec![], ret(0)),
        ],
    );
    let ctx = TransferFlowContext::from_transfer_sites(&func, &[(1, EventSite::Body(0))]);
    let release = PlannedOp {
        slot: PlanSlot::BeforeTerminator { block: 3 },
        kind: PlannedOpKind::Dec,
        var: v(0),
    };

    assert_eq!(ctx.classify(&release), None);
}

/// A borrowed user-call boundary whose callee consumes a COW owner needs two
/// simultaneous credits: the caller retains its original owner across the
/// call, while a separately funded owner transfers into the callee. The
/// class ledger must place that funding before the invoke and release the
/// retained owner on both normal and unwind continuations.
#[test]
fn borrowed_cow_consumed_invoke_funds_callee_and_releases_retained_owner() {
    let func = func_with_blocks(
        2,
        vec![
            block(
                0,
                vec![],
                vec![construct(0, vec![])],
                invoke(1, vec![(0, ArgOwnership::Borrowed)], 1, 2),
            ),
            block(1, vec![], vec![], ret(1)),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let mut callee_contract = MemoryContract::conservative(1);
    callee_contract.params[0].access = crate::aims::lattice::AccessClass::Borrowed;
    callee_contract.params[0].borrowed_cow_consumed = true;
    let contracts = FxHashMap::from_iter([(Name::from_raw(7), callee_contract)]);
    let registry = ori_types::TypeRegistry::default();
    let analysis =
        analyze_from_state_map(&func, &state_map, &contracts, &registry, &test_interner());
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let class = class_rep(&mut partition, 0);
    let ops = ops_for(&analysis, class);

    assert_eq!(
        ops.iter()
            .filter(|op| op.kind == PlannedOpKind::Inc)
            .collect::<Vec<_>>(),
        vec![&inc(PlanSlot::BeforeTerminator { block: 0 }, 0)],
        "the borrowed boundary must fund exactly one callee owner: {ops:?}"
    );
    assert!(
        ops.contains(&dec(PlanSlot::BlockFront { block: 1 }, 0)),
        "the retained caller owner must release on the normal edge: {ops:?}"
    );
    assert!(
        ops.contains(&dec(PlanSlot::BlockFront { block: 2 }, 0)),
        "the retained caller owner must release on the unwind edge: {ops:?}"
    );
    assert_eq!(verdict_for(&analysis, class), ClassVerdict::Clean);
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
/// NO planned ops (a moved value owes no release).
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

/// A BORROWED param stored into a LOCALLY-RELEASED container while the
/// param stays demanded (read after the store): the store hand-off is
/// funded by the borrowed-rooted duplication inc (`plan_incs`), and the
/// param's demand rides the CALLER's reference (RL-2 borrowed-param
/// discipline — the caller retains + releases after the call), which no
/// callee-local container release can strand. NO hazard; every class
/// Clean. The `[value, ...recurse(value:)]` list-build shape.
#[test]
fn borrowed_store_into_released_container_with_later_read_no_hazard() {
    let mut func = one_block_func(
        4,
        vec![
            ArcInstr::Let {
                dst: v(1),
                ty: ty(0),
                value: ArcValue::Var(v(0)),
            },
            construct(2, vec![1]),
            is_shared(3, 0),
        ],
        ret(3),
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    func.var_types[0] = Idx::STR;
    func.var_types[1] = Idx::STR;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    let (analysis, mut partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "a Clean borrowed-rooted class's demand rides the caller's reference; \
         the funded store into a released container endangers nothing"
    );
    assert!(analysis.readiness.all_classes_clean);
    let borrowed = class_rep(&mut partition, 0);
    assert_eq!(verdict_for(&analysis, borrowed), ClassVerdict::Clean);
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

/// A class consumed at a jump-arg hand-off on ONE merge edge and dying
/// UNPASSED on the other exits the two predecessor edges with divergent
/// owed counts — the completed-plan merge gate equalizes the surplus edge
/// with an RL-4-style release at the predecessor's end instead of
/// declining (the while-loop closure-reassignment shape: the overwritten
/// binding's reference dies on the reassigned arm).
#[test]
fn merge_disagree_equalizes_with_per_edge_release() {
    // The loop either replaces its closure merge parameter with a fresh value
    // or passes it through before taking the back edge.
    let lit = |dst: u32| ArcInstr::Let {
        dst: v(dst),
        ty: ty(0),
        value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
    };
    let func = func_with_blocks(
        7,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(1, vec![1], vec![lit(2)], branch(2, 2, 3)),
            block(2, vec![], vec![is_shared(3, 1)], ret(3)),
            block(3, vec![], vec![lit(4)], branch(4, 4, 5)),
            block(4, vec![], vec![construct(5, vec![])], jump(6, vec![5])),
            block(5, vec![], vec![], jump(6, vec![1])),
            block(6, vec![6], vec![], jump(1, vec![6])),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    state_map.set_permanent_scalar(v(3));
    state_map.set_permanent_scalar(v(4));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let class = class_rep(&mut partition, 1);
    assert_eq!(
        verdict_for(&analysis, class),
        ClassVerdict::Clean,
        "the surplus-owing unpassed edge gets the equalizing release"
    );
    assert!(
        ops_for(&analysis, class)
            .iter()
            .any(|op| matches!(op.kind, PlannedOpKind::Dec)
                && op.slot.block() == 4
                && op.var == v(1)),
        "the equalizing release lands on the reassignment arm"
    );
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
        threads_back_edge: false,
        container_held: false,
        externally_funded: false,
        books_runtime_grounded: true,
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
        per_block: vec![vec![ClassEvent {
            site: EventSite::Body(0),
            kind: EventKind::Consume,
            var: None,
            delta: -1,
            floor: 1,
        }]],
        threads_back_edge: false,
        container_held: false,
        externally_funded: true,
        books_runtime_grounded: true,
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
        per_block: vec![vec![ClassEvent {
            site: EventSite::BlockEntry,
            kind: EventKind::Consume,
            var: Some(v(0)),
            delta: -1,
            floor: 1,
        }]],
        threads_back_edge: false,
        container_held: false,
        externally_funded: true,
        books_runtime_grounded: true,
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

/// A sharing-view result enters through a CREDIT, survives COW consumption,
/// and is stored in the returned tuple. Tuple field membership starts at the
/// store and cannot fund the preceding COW hand-off.
#[test]
fn credit_only_view_surviving_consume_is_duplicated_before_handoff() {
    let slice = Name::from_raw(11);
    let push = Name::from_raw(12);
    let mut func = one_block_func(
        4,
        vec![
            construct(0, vec![]),
            apply_to(1, slice, vec![(0, ArgOwnership::Borrowed)]),
            apply_to(2, push, vec![(1, ArgOwnership::Owned)]),
            construct(3, vec![1, 2]),
        ],
        ret(3),
    );
    let state_map = AimsStateMap::new(&func);
    let pool = ori_types::Pool::new();
    let classifier = crate::ArcClassifier::new(&pool);
    crate::aims::freeze_primitive_facts(std::slice::from_mut(&mut func), &classifier)
        .unwrap_or_else(|errors| panic!("class-ledger primitive facts should freeze: {errors:?}"));
    let mut facts: FxHashMap<Name, BoundaryFacts> = FxHashMap::default();
    facts.insert(
        slice,
        BoundaryFacts {
            returns_sharing_view: true,
            ..BoundaryFacts::default()
        },
    );
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let classification =
        classify_function(&func, &state_map, &mut partition, &facts, &test_interner());
    let view = class_rep(&mut partition, 1);
    assert!(
        partition.site(view).is_some(),
        "the direct call result carries its own birth-site witness"
    );
    let events = super::events::extract_class_events(&func, &classification, &mut partition, view);
    assert!(
        !events.container_held && !events.is_externally_funded(),
        "a later tuple field must not retroactively fund the view"
    );
    let preds = crate::graph::compute_predecessors(&func);
    let regions = super::emit::CycleRegions::compute(&func);
    let ClassOutcome::Planned(ops) = super::emit::plan_class(&func, &preds, &regions, &events, &[])
    else {
        panic!("credit-only view plan unexpectedly declined");
    };

    assert!(
        ops.iter().any(|op| {
            op.kind == PlannedOpKind::Inc
                && op.slot == PlanSlot::BeforeBody { block: 0, index: 2 }
                && op.var == v(1)
        }),
        "the sole sharing-view credit must survive the first hand-off: {ops:?}"
    );
    assert_eq!(
        verify_class(&func, &preds, &events, &ops),
        ClassVerdict::Clean
    );
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
        Some(FallbackReason::BurdenEmissionDisabled)
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

/// Returning a scalar-repr user-`@drop` value transfers its drop obligation to
/// the caller, so RL-DROP and RL-2 admit the clean empty-surface plan.
#[test]
fn replacement_admits_returned_scalar_user_drop_value() {
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

    let outcome = attempt_replacement(
        &mut func,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(
        outcome.mode,
        EmissionMode::Replaced,
        "the returned scalar user-drop value transfers its obligation to \
         the caller — Clean books, admitted"
    );
}

/// An ADMITTED scalar user-drop var whose lineage mints NO ledger event (a
/// scalar-literal alias chain read only through a non-admitted scalar
/// alias — the scalar-newtype shape) is excluded-equivalent for the
/// empty-surface admission: the empty plan correctly emits nothing for it.
#[test]
fn replacement_admits_unbooked_scalar_user_drop_newtype_lineage() {
    use core::num::NonZeroU32;
    use ori_registry::burden::FnSym;
    use ori_types::burden::UserBurdenSpec;

    let newtype_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Handle",
        newtype_idx,
        Some(UserBurdenSpec {
            user_drop: Some(FnSym::new(NonZeroU32::MIN)),
            ..UserBurdenSpec::default()
        }),
    );

    // %0: int = 99 (excluded scalar); %1: Handle = %0 (admitted alias DST —
    // no birth of its own); %2: int = %1 (excluded alias); ret %2 — the
    // admitted var never events (the scalar-newtype corpus shape).
    let mut func = one_block_func(
        3,
        vec![
            ArcInstr::Let {
                dst: v(0),
                ty: ty(0),
                value: ArcValue::Literal(crate::ir::LitValue::Int(99)),
            },
            ArcInstr::Let {
                dst: v(1),
                ty: newtype_idx,
                value: ArcValue::Var(v(0)),
            },
            ArcInstr::Let {
                dst: v(2),
                ty: ty(0),
                value: ArcValue::Var(v(1)),
            },
        ],
        ret(2),
    );
    func.var_types = vec![ty(0), newtype_idx, ty(0)];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(0));
    state_map.set_permanent_scalar(v(1));
    state_map.set_permanent_scalar(v(2));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let outcome = attempt_replacement(
        &mut func,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(
        outcome.mode,
        EmissionMode::Replaced,
        "an unbooked admitted scalar user-drop lineage is empty-surface"
    );
    assert!(
        planned_ops(&outcome.analysis).is_empty(),
        "the empty plan correctly emits nothing for an excluded-equivalent lineage"
    );
}

/// A SCALAR user-drop container whose field-path view is read after the
/// container's planned release is NOT a field-view hazard: the scalar
/// release lowers to the balance-neutral `@drop` call — nothing is freed,
/// so the view's post-release read stays valid on the nested-destructure
/// shape.
#[test]
fn replacement_admits_scalar_user_drop_container_with_post_release_view() {
    use core::num::NonZeroU32;
    use ori_registry::burden::FnSym;
    use ori_types::burden::UserBurdenSpec;

    let outer_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Outer",
        outer_idx,
        Some(UserBurdenSpec {
            user_drop: Some(FnSym::new(NonZeroU32::MIN)),
            ..UserBurdenSpec::default()
        }),
    );

    let inner_idx = ty(65);
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Inner",
        inner_idx,
        Some(UserBurdenSpec {
            user_drop: Some(FnSym::new(NonZeroU32::MIN)),
            ..UserBurdenSpec::default()
        }),
    );

    // Two nested views of an owned outer value straddle the container's
    // planned release, with scalar user-drop types at both aggregate levels.
    let mut func = one_block_func(
        6,
        vec![
            ArcInstr::Let {
                dst: v(1),
                ty: outer_idx,
                value: ArcValue::Var(v(0)),
            },
            ArcInstr::Project {
                dst: v(2),
                ty: inner_idx,
                value: v(1),
                field: 0,
            },
            ArcInstr::Project {
                dst: v(3),
                ty: ty(0),
                value: v(2),
                field: 0,
            },
            ArcInstr::Project {
                dst: v(4),
                ty: inner_idx,
                value: v(1),
                field: 0,
            },
            ArcInstr::Project {
                dst: v(5),
                ty: ty(0),
                value: v(4),
                field: 1,
            },
        ],
        ret(5),
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: outer_idx,
        ownership: Ownership::Owned,
    }];
    func.var_types = vec![outer_idx, outer_idx, inner_idx, ty(0), inner_idx, ty(0)];
    let mut state_map = AimsStateMap::new(&func);
    for raw in 0..6 {
        state_map.set_permanent_scalar(v(raw));
    }
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let outcome = attempt_replacement(
        &mut func,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(
        outcome.mode,
        EmissionMode::Replaced,
        "a scalar container's release frees nothing — the post-release view \
         read is not a field-view hazard"
    );
}

/// A user-`@drop` value whose class plans its own WHOLE-VAR release is
/// ADMITTED: the planned dec lowers to the standard drop glue, which runs
/// the user `@drop` exactly once at the death point (RL-DROP discipline).
/// Only field-grain releases or plan-uncovered user-drop vars decline.
#[test]
fn replacement_admits_user_drop_value_with_whole_var_release() {
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
    assert_eq!(outcome.mode, EmissionMode::Replaced);
    assert!(
        gated.blocks[0]
            .body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::BurdenDec { var } if *var == v(0))),
        "the admitted user-drop value carries its whole-var release (the \
         drop glue runs the user @drop exactly once)"
    );
}

// Op-placement guard (`placement::ops_placeable`)

/// A planned op whose variable's definition dominates the slot is
/// placeable; a definition on a disjoint branch is not.
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
    assert!(super::placement::ops_placeable(&func, &dominated));

    let off_path = vec![dec(PlanSlot::BlockFront { block: 1 }, 2)];
    assert!(!super::placement::ops_placeable(&func, &off_path));

    let before_own_def = vec![dec(PlanSlot::BlockFront { block: 0 }, 1)];
    assert!(!super::placement::ops_placeable(&func, &before_own_def));

    let after_own_def = vec![dec(PlanSlot::AfterBody { block: 0, index: 0 }, 1)];
    assert!(super::placement::ops_placeable(&func, &after_own_def));
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
        super::placement::ops_placeable(&func, &ops),
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
    // The merge receives an alias of one allocation on one arm and a distinct
    // allocation on the other, so its parameter must remain refused.
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
    // A dead-on-creation aggregate is born inside the loop body and never
    // escapes the cycle.
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

/// A TRMC `ContextHole`-shaped variable qualifies for replacement because the
/// recursive-call fill is modeled as `mutate(context) + consume(filled value)`
/// and supplies the filled value's release per `holeFill_is_the_release`.
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

crate::test_helpers::ablation_env_event_test!(
    trmc_context_ledger_reproduces_context_hole_decline,
    "ORI_DISABLE_TRMC_CONTEXT_LEDGER",
    "decline class-ledger replacement for TRMC context-hole functions",
    || {
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

        assert_eq!(outcome.mode, EmissionMode::Fallback);
        assert_eq!(outcome.fallback_reason, Some(FallbackReason::TrmcContext));
        true
    },
);

// Field-view hazard cures across loop / merge / select liveness shapes

/// A live field view of a released container funds its independent reference
/// at extraction and releases it after the view's last use.
#[test]
fn live_field_view_of_released_container_funds_itself_at_extraction() {
    // A field view of an opaque call result remains live after the container's
    // last use, but no field funding is known.
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
    // The loop merge starts with Nil and receives a freshly constructed Cons
    // on each back edge before returning the merge value.
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
    // An immortal seed enters the loop merge, whose back edge supplies fresh
    // accumulator values.
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
    // Two fresh allocations merge into a parameter that is never read.
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
    // A value crosses an eventless loop and is consumed only on the exit path.
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
    // A value consumed into a container is subsequently borrowed by an invoke
    // whose normal and unwind edges diverge.
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

/// A fresh capture-bearing closure is caller-owned and only borrowed by an
/// indirect invoke. Its final READ precedes successor selection, so the one
/// retained owner must be released on both the normal and unwind successors.
#[test]
fn partial_apply_invoke_indirect_releases_closure_on_both_successors() {
    let func = func_with_blocks(
        4,
        vec![
            block(
                0,
                vec![],
                vec![
                    construct(0, vec![]),
                    ArcInstr::PartialApply {
                        dst: v(1),
                        ty: ty(0),
                        func: Name::from_raw(8),
                        args: vec![v(0)],
                    },
                ],
                invoke_indirect(2, 1, 1, 2),
            ),
            block(1, vec![], vec![], ret(3)),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    state_map.set_permanent_scalar(v(3));
    let (analysis, mut partition) = analyze(&func, &state_map);

    let closure = class_rep(&mut partition, 1);
    assert_eq!(verdict_for(&analysis, closure), ClassVerdict::Clean);
    let ops = ops_for(&analysis, closure);
    let successor_decs = ops
        .iter()
        .filter(|op| {
            op.kind == PlannedOpKind::Dec
                && matches!(op.slot, PlanSlot::BlockFront { block } if block == 1 || block == 2)
        })
        .count();
    assert_eq!(
        successor_decs, 2,
        "one closure release on each exit: {ops:?}"
    );
    assert!(
        ops.iter().all(|op| op.kind != PlannedOpKind::Inc),
        "a borrowed closure receiver must not acquire a second owner: {ops:?}"
    );
}

/// A `Select` over two REAL allocations acquires the selected reference:
/// the planner realizes the acquisition with an RL-1 duplication inc after
/// the select, each operand class balances via its own birth + release,
/// and the select class's hand-off consume is funded — every class Clean.
#[test]
fn select_of_real_allocations_funds_the_selected_reference() {
    // A select aliases one of two fresh allocations before an unread merge.
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
    // An owned parameter supplies the cross-class credit into a refused loop
    // merge; its same-class back edge is silent.
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
    // A loop-body invoke borrows the pre-loop allocation, returning on its
    // normal edge and resuming on unwind.
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

/// Verifies clean per-field classes for heap fields read through an aggregate alias.
#[test]
fn struct_list_field_flagship_per_field_classes_replace() {
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

    assert!(analysis.readiness.all_classes_clean);
    assert!(
        !analysis.field_view_hazard,
        "extraction funding cures every endangered view"
    );
    assert_eq!(verdict_for(&analysis, items_class), ClassVerdict::Clean);
    assert_eq!(verdict_for(&analysis, label_class), ClassVerdict::Clean);
}

/// Moving an extracted member into a second released container uses the base
/// plan's duplication credit, leaving the original whole-container release
/// intact and every class clean.
#[test]
fn extract_then_move_out_via_second_container_funds_itself_at_extraction() {
    // A projected member moves from the released tuple into a second
    // container while an alias keeps the first container eventful.
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

/// A fresh managed value stored in a released `Option` and also handed to an
/// owned call has two independently funded owners. The class plan's duplicate
/// credit funds the call; it is not a field move-out that may steal the
/// borrowed option receiver's retained payload credit.
#[test]
fn funded_alias_owned_call_preserves_released_option_payload_credit() {
    let interner = test_interner();
    let option_name = interner.intern("Option");
    let mut pool = ori_types::Pool::new();
    let option_str = pool.option(Idx::STR);
    let mut registry = ori_types::TypeRegistry::new();
    ori_types::register_resolved_collection_burdens(&pool, &mut registry);
    let mut func = one_block_func(
        5,
        vec![
            ArcInstr::Construct {
                dst: v(0),
                ty: Idx::STR,
                ctor: CtorKind::Tuple,
                args: vec![],
            },
            ArcInstr::Construct {
                dst: v(1),
                ty: option_str,
                ctor: CtorKind::EnumVariant {
                    enum_name: option_name,
                    variant: 1,
                },
                args: vec![v(0)],
            },
            apply(2, vec![(0, ArgOwnership::Owned)]),
            ArcInstr::Let {
                dst: v(3),
                ty: option_str,
                value: ArcValue::Var(v(1)),
            },
            is_shared(4, 3),
        ],
        ret(4),
    );
    func.params = vec![];
    func.var_types = vec![Idx::STR, option_str, Idx::UNIT, option_str, Idx::BOOL];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    state_map.set_permanent_scalar(v(4));

    let (analysis, mut partition) =
        analyze_with_registry_and_interner(&func, &state_map, &registry, &interner);

    assert!(
        !analysis.field_view_hazard,
        "the plan-funded owned call must not become a field move-out hazard"
    );
    assert!(analysis.readiness.all_classes_clean);

    let payload = class_rep(&mut partition, 0);
    let payload_ops = ops_for(&analysis, payload);
    assert_eq!(
        payload_ops
            .iter()
            .filter(|op| op.kind == PlannedOpKind::Inc)
            .count(),
        1,
        "one duplicate credit funds the owned call: {payload_ops:?}"
    );

    let container = class_rep(&mut partition, 1);
    let container_ops = ops_for(&analysis, container);
    assert!(
        container_ops.iter().any(|op| op.kind == PlannedOpKind::Dec),
        "the option retains its whole-container release: {container_ops:?}"
    );
    assert!(
        container_ops
            .iter()
            .all(|op| !matches!(op.kind, PlannedOpKind::DecPartial { .. })),
        "the retained payload credit must not be decomposed away: {container_ops:?}"
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
            self_owned_identity: true,
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

fn projected_cow_reconstruction_loop_body(push: Name, pair_ty: Idx, list_ty: Idx) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(2),
        params: vec![],
        body: vec![
            ArcInstr::Project {
                dst: v(6),
                ty: list_ty,
                value: v(4),
                field: 0,
            },
            ArcInstr::Let {
                dst: v(7),
                ty: Idx::INT,
                value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
            },
            ArcInstr::Apply {
                dst: v(8),
                ty: list_ty,
                func: push,
                args: vec![v(6), v(7)],
                arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                mono_instance_id: None,
            },
            ArcInstr::Project {
                dst: v(9),
                ty: Idx::STR,
                value: v(4),
                field: 1,
            },
            ArcInstr::Construct {
                dst: v(10),
                ty: pair_ty,
                ctor: CtorKind::Struct(Name::from_raw(82)),
                args: vec![v(8), v(9)],
            },
            ArcInstr::Let {
                dst: v(11),
                ty: Idx::BOOL,
                value: ArcValue::Literal(crate::ir::LitValue::Bool(false)),
            },
        ],
        terminator: jump(1, vec![10, 11]),
    }
}

fn projected_cow_reconstruction_func(push: Name, pair_ty: Idx, list_ty: Idx) -> ArcFunction {
    ArcFunction {
        var_types: vec![
            list_ty,
            Idx::STR,
            pair_ty,
            Idx::BOOL,
            pair_ty,
            Idx::BOOL,
            list_ty,
            Idx::INT,
            list_ty,
            Idx::STR,
            pair_ty,
            Idx::BOOL,
        ],
        blocks: vec![
            block(
                0,
                vec![],
                vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: list_ty,
                        ctor: CtorKind::ListLiteral,
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::STR,
                        value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(81))),
                    },
                    ArcInstr::Construct {
                        dst: v(2),
                        ty: pair_ty,
                        ctor: CtorKind::Struct(Name::from_raw(82)),
                        args: vec![v(0), v(1)],
                    },
                    ArcInstr::Let {
                        dst: v(3),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                    },
                ],
                jump(1, vec![2, 3]),
            ),
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(v(4), pair_ty), (v(5), Idx::BOOL)],
                body: vec![],
                terminator: branch(5, 2, 3),
            },
            projected_cow_reconstruction_loop_body(push, pair_ty, list_ty),
            ArcBlock {
                id: ArcBlockId::new(3),
                params: vec![],
                body: vec![],
                terminator: ret(4),
            },
        ],
        ..Default::default()
    }
}

#[test]
fn projected_cow_reconstruction_rebooks_the_existing_field_credit() {
    use crate::lower::test_utils::registered_struct_with_burden;
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};

    let interner = test_interner();
    let push = interner.intern("push");
    let pair_ty = ty(64);
    let list_ty = ty(70);
    let func = projected_cow_reconstruction_func(push, pair_ty, list_ty);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_burden(
        &mut registry,
        "ProjectedCowPair",
        pair_ty,
        Some(UserBurdenSpec {
            self_owned_identity: true,
            owned_fields: vec![
                UserOwnedField {
                    field_path: vec![0],
                    field_type: list_ty,
                },
                UserOwnedField {
                    field_path: vec![1],
                    field_type: Idx::STR,
                },
            ],
            ..UserBurdenSpec::default()
        }),
    );
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [3, 5, 7, 11] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, mut partition) =
        analyze_with_registry_and_interner(&func, &state_map, &registry, &interner);

    assert!(
        !analysis.field_view_hazard && analysis.readiness.all_classes_clean,
        "the exact reconstruction must verify cleanly: field_view_hazard={} \
         all_classes_clean={} declined={:?} verdicts={:?}",
        analysis.field_view_hazard,
        analysis.readiness.all_classes_clean,
        analysis.readiness.declined,
        analysis.readiness.verdicts
    );
    let projected_list = class_rep(&mut partition, 6);
    let projected_ops = ops_for(&analysis, projected_list);
    assert!(
        projected_ops.iter().all(|op| op.kind != PlannedOpKind::Inc),
        "the projected list's existing owner credit transfers through push into \
         the rebuilt aggregate; no retain may inflate dynamic COW: {projected_ops:?}"
    );
}

#[test]
fn canonical_exact_transfer_witness_drives_production_ledger_rebooking() {
    use crate::lower::test_utils::registered_struct_with_burden;
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};

    struct WitnessClassifier;
    impl crate::ArcClassification for WitnessClassifier {
        fn arc_class(&self, idx: Idx) -> crate::ArcClass {
            if matches!(idx, Idx::INT | Idx::BOOL) {
                crate::ArcClass::Scalar
            } else {
                crate::ArcClass::DefiniteRef
            }
        }
    }

    let interner = test_interner();
    let push = interner.intern("push");
    let pair_ty = ty(64);
    let list_ty = ty(70);
    let func = projected_cow_reconstruction_func(push, pair_ty, list_ty);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_burden(
        &mut registry,
        "CanonicalWitnessPair",
        pair_ty,
        Some(UserBurdenSpec {
            self_owned_identity: true,
            owned_fields: vec![
                UserOwnedField {
                    field_path: vec![0],
                    field_type: list_ty,
                },
                UserOwnedField {
                    field_path: vec![1],
                    field_type: Idx::STR,
                },
            ],
            ..UserBurdenSpec::default()
        }),
    );
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [3, 5, 7, 11] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let classifier = WitnessClassifier;
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let mut contracts = FxHashMap::default();
    crate::aims::builtins::seed_builtin_contracts(&mut contracts, &builtins, &interner);
    let exact_callables = FxHashSet::default();
    let extraction =
        crate::aims::interprocedural::extract_contract_and_transfers_with_call_ownership(
            &crate::aims::interprocedural::ContractExtractionInput {
                func: &func,
                state_map: &state_map,
                classifier: &classifier,
                sigs: &contracts,
                scc_peers: &FxHashSet::default(),
                context_regions: &[],
                interner: &interner,
                builtins: &builtins,
                exact_callables: &exact_callables,
                type_registry: Some(&registry),
            },
        );
    let [witness] = extraction.exact_transfer_witnesses.as_slice() else {
        panic!("the canonical producer must publish the loop reconstruction witness");
    };
    assert_eq!(witness.param, None);
    assert_eq!(witness.block, ArcBlockId::new(2));

    let analysis = super::analysis::analyze_from_state_map_with_exact(
        &func,
        &state_map,
        &contracts,
        &exact_callables,
        Some(&extraction.exact_transfer_witnesses),
        &registry,
        &interner,
    );
    assert!(
        !analysis.field_view_hazard && analysis.readiness.all_classes_clean,
        "the production witness consumer must rebook without a second recognizer: \
         hazard={} declined={:?} verdicts={:?}",
        analysis.field_view_hazard,
        analysis.readiness.declined,
        analysis.readiness.verdicts,
    );
}

#[test]
fn paramless_container_user_drop_declines_canonical_producer_and_consumer() {
    use crate::lower::test_utils::registered_struct_with_burden;
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};

    struct WitnessClassifier;
    impl crate::ArcClassification for WitnessClassifier {
        fn arc_class(&self, idx: Idx) -> crate::ArcClass {
            if matches!(idx, Idx::INT | Idx::BOOL) {
                crate::ArcClass::Scalar
            } else {
                crate::ArcClass::DefiniteRef
            }
        }
    }

    let interner = test_interner();
    let push = interner.intern("push");
    let pair_ty = ty(64);
    let list_ty = ty(70);
    let func = projected_cow_reconstruction_func(push, pair_ty, list_ty);
    let registry = |user_drop| {
        let mut registry = ori_types::TypeRegistry::new();
        registered_struct_with_burden(
            &mut registry,
            "ParamlessWitnessPair",
            pair_ty,
            Some(UserBurdenSpec {
                self_owned_identity: true,
                user_drop,
                owned_fields: vec![
                    UserOwnedField {
                        field_path: vec![0],
                        field_type: list_ty,
                    },
                    UserOwnedField {
                        field_path: vec![1],
                        field_type: Idx::STR,
                    },
                ],
                ..UserBurdenSpec::default()
            }),
        );
        registry
    };
    let ordinary_registry = registry(None);
    let user_drop_registry = registry(Some(ori_registry::burden::FnSym::new(
        core::num::NonZeroU32::MIN,
    )));
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [3, 5, 7, 11] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let classifier = WitnessClassifier;
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let mut contracts = FxHashMap::default();
    crate::aims::builtins::seed_builtin_contracts(&mut contracts, &builtins, &interner);
    let exact_callables = FxHashSet::default();
    let extract = |type_registry| {
        crate::aims::interprocedural::extract_contract_and_transfers_with_call_ownership(
            &crate::aims::interprocedural::ContractExtractionInput {
                func: &func,
                state_map: &state_map,
                classifier: &classifier,
                sigs: &contracts,
                scc_peers: &FxHashSet::default(),
                context_regions: &[],
                interner: &interner,
                builtins: &builtins,
                exact_callables: &exact_callables,
                type_registry: Some(type_registry),
            },
        )
    };

    let ordinary = extract(&ordinary_registry);
    let [witness] = ordinary.exact_transfer_witnesses.as_slice() else {
        panic!("the ordinary local reconstruction must publish one witness");
    };
    assert_eq!(witness.param, None);

    let rejected = extract(&user_drop_registry);
    assert!(
        rejected.exact_transfer_witnesses.is_empty(),
        "the canonical producer must reject a param-less user-drop container"
    );

    let mut partition = compute_birth_site_partition(&func, &state_map);
    let arms = super::events::full_move_arms_from_exact_transfer_witnesses(
        &func,
        &mut partition,
        &user_drop_registry,
        &ordinary.exact_transfer_witnesses,
    );
    assert!(
        arms.is_empty(),
        "the consumer must not materialize a full-move arm from a stale \
         param-less witness when cleanup authority changes to user drop"
    );
}

#[test]
fn opaque_owned_relay_does_not_authorize_projected_full_move() {
    use crate::lower::test_utils::registered_struct_with_burden;
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};

    let interner = test_interner();
    let opaque = interner.intern("opaque_owned_relay");
    let pair_ty = ty(64);
    let list_ty = ty(70);
    let func = projected_cow_reconstruction_func(opaque, pair_ty, list_ty);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_burden(
        &mut registry,
        "OpaqueRelayPair",
        pair_ty,
        Some(UserBurdenSpec {
            self_owned_identity: true,
            owned_fields: vec![
                UserOwnedField {
                    field_path: vec![0],
                    field_type: list_ty,
                },
                UserOwnedField {
                    field_path: vec![1],
                    field_type: Idx::STR,
                },
            ],
            ..UserBurdenSpec::default()
        }),
    );
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [3, 5, 7, 11] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, _) = analyze_with_registry_and_interner(&func, &state_map, &registry, &interner);

    assert!(
        analysis.field_view_hazard,
        "an unregistered Owned call is conservative authority, not proof that \
         the call result linearly reconstructs the projected field"
    );
}

#[test]
fn exact_push_name_collision_does_not_authorize_projected_full_move() {
    use crate::lower::test_utils::registered_struct_with_burden;
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};

    let interner = test_interner();
    let push = interner.intern("push");
    let pair_ty = ty(64);
    let list_ty = ty(70);
    let func = projected_cow_reconstruction_func(push, pair_ty, list_ty);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_burden(
        &mut registry,
        "ExactPushCollisionPair",
        pair_ty,
        Some(UserBurdenSpec {
            self_owned_identity: true,
            owned_fields: vec![
                UserOwnedField {
                    field_path: vec![0],
                    field_type: list_ty,
                },
                UserOwnedField {
                    field_path: vec![1],
                    field_type: Idx::STR,
                },
            ],
            ..UserBurdenSpec::default()
        }),
    );
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [3, 5, 7, 11] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let exact_callables = FxHashSet::from_iter([push]);
    let (analysis, _) = analyze_with_registry_interner_and_exact(
        &func,
        &state_map,
        &registry,
        &interner,
        &exact_callables,
    );

    assert!(
        analysis.field_view_hazard,
        "an exact user callable named `push` must not inherit registry builtin authority"
    );
}

#[test]
fn registered_receiver_only_relay_authorizes_projected_full_move() {
    use crate::lower::test_utils::registered_struct_with_burden;
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};

    let interner = test_interner();
    let remove = interner.intern("remove");
    let pair_ty = ty(64);
    let list_ty = ty(70);
    let func = projected_cow_reconstruction_func(remove, pair_ty, list_ty);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_burden(
        &mut registry,
        "ReceiverOnlyRelayPair",
        pair_ty,
        Some(UserBurdenSpec {
            self_owned_identity: true,
            owned_fields: vec![
                UserOwnedField {
                    field_path: vec![0],
                    field_type: list_ty,
                },
                UserOwnedField {
                    field_path: vec![1],
                    field_type: Idx::STR,
                },
            ],
            ..UserBurdenSpec::default()
        }),
    );
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [3, 5, 7, 11] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, _) = analyze_with_registry_and_interner(&func, &state_map, &registry, &interner);

    assert!(
        !analysis.field_view_hazard && analysis.readiness.all_classes_clean,
        "the frozen Owned receiver contract proves one-in/one-out conservation"
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
        threads_back_edge: false,
        container_held: false,
        externally_funded: false,
        books_runtime_grounded: grounded,
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
/// `moved_class_shares_edge_source` must decline the arm; rebooking that arm
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
            self_owned_identity: true,
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
    let interner = test_interner();

    let arms = super::events::detect_full_move_arms(
        &func,
        &mut partition,
        &registry,
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        arms.is_empty(),
        "a Jump edge feeding two params from one class must decline the \
         full-move arm (runtime aliasing across per-source lineages)"
    );
}

/// Two released containers sharing one unextracted inner value balance their
/// stores with the birth credit and one duplication credit.
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
    // Returning the only extracted member transfers its ownership; the cure
    // makes the container release skip that field and removes the view's
    // move-in store.
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

/// A locally released container may also transfer out on another path. The
/// spread shape returns the original container on success while an owned call
/// consumes one projected field; globally re-booking the field's move-in as
/// non-consuming would leave the returned original holding an unfunded stale
/// pointer. Decomposition must decline and extraction funding must retain the
/// projected field before the call.
#[test]
fn transferred_container_with_moved_field_uses_extraction_funding() {
    let mut func = func_with_blocks(
        6,
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
                    ArcInstr::Project {
                        dst: v(2),
                        ty: Idx::STR,
                        value: v(1),
                        field: 0,
                    },
                ],
                invoke(3, vec![(2, ArgOwnership::Owned)], 1, 2),
            ),
            block(
                1,
                vec![],
                vec![construct(4, vec![3]), construct(5, vec![1, 4])],
                ret(5),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.var_types[0] = Idx::STR;
    func.var_types[2] = Idx::STR;
    func.var_types[3] = Idx::STR;
    let state_map = AimsStateMap::new(&func);
    let (analysis, mut partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "extraction funding must cure the transferred-container spread shape"
    );
    assert!(analysis.readiness.all_classes_clean);

    let view = class_rep(&mut partition, 2);
    let view_ops = ops_for(&analysis, view);
    assert!(
        view_ops.iter().any(|op| {
            op.kind == PlannedOpKind::Inc
                && op.var == v(2)
                && op.slot == PlanSlot::AfterBody { block: 0, index: 2 }
        }),
        "the projected field must be retained before the owned Invoke: {view_ops:?}"
    );

    let container = class_rep(&mut partition, 1);
    let container_ops = ops_for(&analysis, container);
    assert!(
        container_ops.iter().any(|op| op.kind == PlannedOpKind::Dec),
        "the unwind path keeps the container's whole release: {container_ops:?}"
    );
    assert!(
        container_ops
            .iter()
            .all(|op| !matches!(op.kind, PlannedOpKind::DecPartial { .. })),
        "a transferred container must not globally decompose: {container_ops:?}"
    );
}

/// A demand-endangered view (field borrowed and READ while the container
/// is locally released) cures via extraction funding: the seed inc after
/// the `Project` funds the read, and the view's single owed reference
/// releases after its last read.
#[test]
fn demand_endangered_view_cures_via_extraction_funding() {
    // A borrowed field view is read before the pair container's last use.
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
    // A borrowed field view is read through an alias before the pair
    // container's last use.
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
    // Two chained invokes borrow through aliases of a boxed string and its
    // unwrapped result, with independent unwind edges.
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
/// never leaks into the merge result.
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

/// Returning a field from a constructless call result funds the view at its
/// extraction and transfers that credit under RL-2.
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

/// A registered constructless call-result container already owns each named
/// field. Moving one field out transfers that credit through the extraction;
/// it must not manufacture the funding increment used by an unregistered
/// container whose field ownership is unknown.
#[test]
fn registered_call_result_field_move_transfers_container_credit() {
    use crate::lower::test_utils::registered_struct_with_two_owned_str_fields;

    let struct_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_two_owned_str_fields(&mut registry, "Pair", struct_idx);

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
    func.var_types[0] = struct_idx;
    func.var_types[1] = Idx::STR;
    let state_map = AimsStateMap::new(&func);
    let (analysis, mut partition) = analyze_with_registry(&func, &state_map, &registry);

    assert!(
        !analysis.field_view_hazard,
        "registered constructless field move must cure via decomposition"
    );
    assert!(analysis.readiness.all_classes_clean);

    let container = class_rep(&mut partition, 0);
    assert!(
        ops_for(&analysis, container).iter().any(|op| matches!(
            &op.kind,
            PlannedOpKind::DecPartial { skip_fields } if skip_fields == &vec![0]
        )),
        "the container release skips the transferred field"
    );

    let view = class_rep(&mut partition, 1);
    assert!(
        ops_for(&analysis, view).is_empty(),
        "the extraction transfers the container-held credit without an increment"
    );
}

/// The constructless container contributes one field credit per path, not one
/// per projection. Two sequential moves therefore retain exactly one real
/// increment for the second transferred reference.
#[test]
fn registered_call_result_two_field_moves_keep_one_duplication_increment() {
    use crate::lower::test_utils::registered_struct_with_two_owned_str_fields;

    let struct_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_two_owned_str_fields(&mut registry, "Pair", struct_idx);

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
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Project {
                        dst: v(2),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    apply(3, vec![(1, ArgOwnership::Owned), (2, ArgOwnership::Owned)]),
                ],
                ret(3),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.var_types[0] = struct_idx;
    func.var_types[1] = Idx::STR;
    func.var_types[2] = Idx::STR;
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    let (analysis, mut partition) = analyze_with_registry(&func, &state_map, &registry);

    assert!(!analysis.field_view_hazard);
    assert!(analysis.readiness.all_classes_clean);
    let view = class_rep(&mut partition, 1);
    let view_ops = ops_for(&analysis, view);
    assert_eq!(
        view_ops
            .iter()
            .filter(|op| op.kind == PlannedOpKind::Inc)
            .count(),
        1,
        "one inherited field credit funds the first move; the duplicate still increments: \
         {view_ops:?}"
    );
}

/// Same shape as `extract_then_move_out_decomposes_container_release`, but
/// Run `attempt_replacement` with a container whose registered owned-field
/// surface includes field zero. The computed `DecPartial(skip=[0])` clears
/// `replace::dec_partial_skips_valid` and admits end-to-end replacement through
/// the real gate, not only the lower-level `analyze()` helper.
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
    // The fixture must give both the constructed value and its Let alias the
    // same struct type, matching the real type-preserving rename invariant.
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

/// Negative case for `field_decomposition_cure_replaces_end_to_end_with_
/// registered_burden`: the same consume-marked-then-Return shape uses a
/// container's registered burden names ONLY field 1 as owned
/// (`registered_struct_scalar_owned_mixed`) while the extracted member is
/// field 0. The cure still computes `DecPartial(skip=[0])` from the
/// partition's consume marks alone (it never consults the registry); the
/// skip index then falls OUTSIDE the container's named owned-field surface,
/// so `replace::dec_partial_skips_valid` rejects it and `attempt_replacement`
/// falls back with `FieldDecompositionShape` rather than committing a plan
/// whose interior field walk would silently mis-skip at runtime.
#[test]
fn field_decomposition_cure_declines_replacement_on_skip_field_mismatch() {
    use crate::lower::test_utils::registered_struct_scalar_owned_mixed;

    let struct_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_scalar_owned_mixed(&mut registry, "Mixed", struct_idx);

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
/// hazard survives (fail-closed decline).
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
/// decomposition cure declines a sum container (`hazard.is_sum_container()`) since
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

/// A uniform single-payload sum moves its payload through a tag match, so its
/// container release skips exactly that variant ordinal.
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

/// Builds a constructless tuple whose handle moves out only on the taken arm;
/// the bypass arm retains the handle inside the call-result container.
fn constructless_invoke_result_tuple_func() -> ArcFunction {
    let mut func = func_with_blocks(
        8,
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
                ArcTerminator::Branch {
                    cond: v(2),
                    then_block: ArcBlockId::new(3),
                    else_block: ArcBlockId::new(4),
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
    // The handle type is an unregistered index with NO burden (the iterator
    // handle: destructor-freed, never refcounted) — extraction funding
    // cannot fire; per-site positional decomposition is the only cure.
    func.var_types[3] = ty(70);

    func
}

/// A constructless tuple decomposes per site: the taken-arm release skips the
/// moved field, while the bypass release remains whole.
#[test]
fn constructless_invoke_result_tuple_decomposes_release_per_site() {
    let mut func = constructless_invoke_result_tuple_func();
    let mut pool = ori_types::Pool::new();
    let option = pool.option(ori_types::Idx::INT);
    let iterator = pool.iterator(ori_types::Idx::INT);
    let tuple_idx = pool.tuple(&[option, iterator]);
    let mut registry = ori_types::TypeRegistry::new();
    ori_types::register_resolved_collection_burdens(&pool, &mut registry);
    func.var_types[1] = tuple_idx;
    func.var_types[3] = iterator;
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [2u32, 4, 5, 6, 7] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let (analysis, mut partition) = analyze_with_registry(&func, &state_map, &registry);

    assert!(
        !analysis.field_view_hazard,
        "the per-site positional skip cures the moved-out tuple field"
    );
    assert!(analysis.readiness.all_classes_clean);
    let container = class_rep(&mut partition, 1);
    let container_plan = analysis
        .plan
        .classes
        .iter()
        .find(|plan| partition.rep_of(plan.class) == container)
        .unwrap_or_else(|| panic!("container class planned"));
    let ClassOutcome::Planned(ops) = &container_plan.outcome else {
        panic!("container plan declined");
    };
    let has_skip_op = ops.iter().any(|op| {
        matches!(&op.kind, super::emit::PlannedOpKind::DecPartial { skip_fields } if skip_fields == &vec![1])
    });
    let has_whole_op = ops
        .iter()
        .any(|op| matches!(op.kind, super::emit::PlannedOpKind::Dec));
    assert!(
        has_skip_op,
        "extraction-dominated release skips the moved-out field: {ops:?}"
    );
    assert!(
        has_whole_op,
        "bypass-arm release keeps the whole field-wise dec: {ops:?}"
    );
}

/// A constructless tuple field is extracted before a may-unwind call and
/// transferred only on the normal edge. The extraction credit is an OWNED
/// transfer from the container: the normal-edge consume needs no synthetic
/// increment, while the unwind edge drops the projected iterator alias.
#[test]
fn constructless_tuple_field_transfer_survives_intervening_unwind() {
    let mut func = func_with_blocks(
        5,
        vec![
            block(0, vec![], vec![], invoke(0, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![ArcInstr::Project {
                    dst: v(1),
                    ty: ty(70),
                    value: v(0),
                    field: 1,
                }],
                invoke(2, vec![], 3, 4),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
            block(
                3,
                vec![],
                vec![
                    apply(3, vec![(1, ArgOwnership::Owned)]),
                    ArcInstr::Let {
                        dst: v(4),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(crate::ir::LitValue::Bool(true)),
                    },
                ],
                ret(4),
            ),
            block(4, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    let mut pool = ori_types::Pool::new();
    let option = pool.option(ori_types::Idx::INT);
    let iterator = pool.iterator(ori_types::Idx::INT);
    let tuple_idx = pool.tuple(&[option, iterator]);
    let mut registry = ori_types::TypeRegistry::new();
    ori_types::register_resolved_collection_burdens(&pool, &mut registry);
    func.var_types[0] = tuple_idx;
    func.var_types[1] = iterator;
    func.var_types[2] = Idx::UNIT;
    func.var_types[3] = Idx::UNIT;
    func.var_types[4] = Idx::BOOL;
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [2u32, 3, 4] {
        state_map.set_permanent_scalar(v(scalar));
    }

    let (analysis, mut partition) = analyze_with_registry(&func, &state_map, &registry);

    assert!(
        !analysis.field_view_hazard,
        "the projected iterator must transfer cleanly across the intervening unwind"
    );
    assert!(analysis.readiness.all_classes_clean);
    let view = class_rep(&mut partition, 1);
    let view_ops = ops_for(&analysis, view);
    assert!(
        view_ops.iter().all(|op| op.kind != PlannedOpKind::Inc),
        "a linear iterator transfer cannot be funded by a no-op increment: {view_ops:?}"
    );
    assert!(
        view_ops.iter().any(|op| {
            op.kind == PlannedOpKind::Dec
                && matches!(op.slot, PlanSlot::BlockFront { block: 4 })
                && op.var == v(1)
        }),
        "the unwind edge must drop the Project result that owns the transferred field: {view_ops:?}"
    );
}

/// Builds a constructless sum whose matched payload moves to an owned consumer.
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

/// A view shared by two released containers uses extraction funding because
/// decomposing either container would not cover both release obligations.
#[test]
fn multi_container_view_declines_field_decomposition() {
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

fn multi_container_unfundable_seed_func() -> ArcFunction {
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
    func.var_types[2] = ty(64);
    func.var_types[4] = ty(64);
    func
}

fn multi_container_unfundable_seed_state(func: &ArcFunction) -> AimsStateMap {
    let mut state_map = AimsStateMap::new(func);
    state_map.set_permanent_scalar(v(6));
    state_map.set_permanent_scalar(v(8));
    state_map
}

#[test]
fn multi_container_view_with_unfundable_seed_is_cured() {
    let func = multi_container_unfundable_seed_func();
    let state_map = multi_container_unfundable_seed_state(&func);
    let (analysis, _partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "a multi-container view whose seed increment carries no burden must \
         still be cured: {:?}",
        analysis.readiness.declined
    );
    assert!(
        analysis.readiness.all_classes_clean,
        "the cured plan must verify Clean: {:?}",
        analysis.readiness.declined
    );
}

#[test]
fn multi_container_view_with_unfundable_seed_keeps_container_releases_whole() {
    let func = multi_container_unfundable_seed_func();
    let state_map = multi_container_unfundable_seed_state(&func);
    let (analysis, mut partition) = analyze(&func, &state_map);

    let container_a = class_rep(&mut partition, 1);
    let container_b = class_rep(&mut partition, 3);
    assert_ne!(
        container_a, container_b,
        "the two containers stay distinct classes"
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
            "unmanaged tuple containers retain whole releases: {ops:?}"
        );
    }
}

#[test]
fn multi_container_tuple_container_with_user_drop_declines_no_release_cure() {
    use core::num::NonZeroU32;
    use ori_registry::burden::FnSym;
    use ori_types::burden::UserBurdenSpec;

    let mut func = multi_container_unfundable_seed_func();
    func.var_types[1] = ty(65);
    func.var_types[3] = ty(65);
    let state_map = multi_container_unfundable_seed_state(&func);
    let mut registry = ori_types::TypeRegistry::new();
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "GuardedContainer",
        ty(65),
        Some(UserBurdenSpec {
            user_drop: Some(FnSym::new(NonZeroU32::MIN)),
            ..UserBurdenSpec::default()
        }),
    );
    assert!(
        crate::lower::type_has_user_drop(ty(65), &registry),
        "the negative pin requires cleanup on both tuple containers"
    );
    assert!(
        !crate::lower::type_has_user_drop(ty(64), &registry),
        "the member type stays unregistered so only the container guard can decline"
    );

    let (analysis, _partition) = analyze_with_registry(&func, &state_map, &registry);
    assert!(
        analysis.field_view_hazard,
        "a released container carrying its own cleanup must retain the hazard"
    );
}

#[test]
fn multi_container_tuple_member_with_user_drop_declines_no_release_cure() {
    use core::num::NonZeroU32;
    use ori_registry::burden::FnSym;
    use ori_types::burden::UserBurdenSpec;

    let func = multi_container_unfundable_seed_func();
    let state_map = multi_container_unfundable_seed_state(&func);
    let mut registry = ori_types::TypeRegistry::new();
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "GuardedMember",
        ty(64),
        Some(UserBurdenSpec {
            user_drop: Some(FnSym::new(NonZeroU32::MIN)),
            ..UserBurdenSpec::default()
        }),
    );
    assert!(
        crate::lower::type_has_user_drop(ty(64), &registry),
        "the negative pin requires cleanup on projected tuple members"
    );
    assert!(
        func.blocks[0]
            .body
            .iter()
            .filter(|instr| matches!(
                instr,
                ArcInstr::Construct {
                    ctor: CtorKind::Tuple,
                    args,
                    ..
                } if !args.is_empty()
            ))
            .count()
            == 2,
        "the negative pin reaches the multi-container non-sum rung"
    );

    let (analysis, _partition) = analyze_with_registry(&func, &state_map, &registry);
    assert!(
        analysis.field_view_hazard,
        "a non-sum tuple chain must retain the hazard when member cleanup is registered"
    );
}

#[test]
fn single_container_unfundable_demand_declines_no_release_cure() {
    let mut func = func_with_blocks(
        6,
        vec![
            block(
                0,
                vec![],
                vec![
                    construct(0, vec![]),
                    construct(1, vec![0]),
                    ArcInstr::Project {
                        dst: v(2),
                        ty: ty(70),
                        value: v(1),
                        field: 0,
                    },
                ],
                invoke(3, vec![(2, ArgOwnership::Owned)], 1, 2),
            ),
            block(
                1,
                vec![],
                vec![construct(4, vec![3]), construct(5, vec![1, 4])],
                ret(5),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.params = vec![];
    func.var_types[2] = ty(70);
    func.var_types[3] = ty(70);
    let state_map = AimsStateMap::new(&func);

    let (analysis, _partition) = analyze(&func, &state_map);
    assert!(
        analysis.field_view_hazard,
        "one non-sum container cannot justify the unmanaged tuple-chain cure"
    );
}

#[test]
fn multi_container_sum_view_declines_no_release_cure() {
    let mut func = multi_container_unfundable_seed_func();
    for instr in &mut func.blocks[0].body {
        let ArcInstr::Construct { ctor, args, .. } = instr else {
            continue;
        };
        if args.is_empty() {
            continue;
        }
        *ctor = CtorKind::EnumVariant {
            enum_name: Name::from_raw(9),
            variant: 0,
        };
    }
    let state_map = multi_container_unfundable_seed_state(&func);

    let (analysis, _partition) = analyze(&func, &state_map);
    assert!(
        analysis.field_view_hazard,
        "recursive sum-container cleanup must retain the shared-view hazard"
    );
}

#[test]
fn production_replacement_covers_multi_container_unfundable_view() {
    let mut func = multi_container_unfundable_seed_func();
    let state_map = multi_container_unfundable_seed_state(&func);

    let pool = ori_types::Pool::new();
    let classifier = crate::ArcClassifier::new(&pool);
    crate::aims::freeze_primitive_facts(std::slice::from_mut(&mut func), &classifier)
        .unwrap_or_else(|errors| panic!("primitive facts should freeze: {errors:?}"));
    let interner = test_interner();
    let registry = ori_types::TypeRegistry::default();
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let replaced = crate::aims::class_ledger::apply_class_ledger_replacement(
        &mut func, &state_map, &contracts, &registry, &interner, true,
    );
    assert!(
        replaced,
        "the production replacement gate must cover a multi-container view \
         whose extraction seed carries no burden"
    );
}

#[test]
fn single_container_view_with_unfundable_seed_is_cured() {
    let mut func = one_block_func(
        7,
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
            is_shared(6, 3),
        ],
        ret(3),
    );
    func.params = vec![];
    func.var_types[2] = ty(64);
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(5));
    state_map.set_permanent_scalar(v(6));
    let (analysis, _partition) = analyze(&func, &state_map);

    assert!(
        !analysis.field_view_hazard,
        "a single-container view is cured by field decomposition even when \
         its extraction seed carries no burden: {:?}",
        analysis.readiness.declined
    );
    assert!(
        analysis.readiness.all_classes_clean,
        "field decomposition must leave every single-container class clean"
    );
}

/// A release planned for a class containing a BORROWED function param names
/// a same-class alias, never the param var itself (VF-1 rejects an `RcDec` on
/// a borrowed param; the alias is the same allocation).
#[test]
fn release_never_names_borrowed_param_var() {
    // A borrowed iter-consuming parameter crosses an eventless invoke block;
    // unwind cleanup must find its alias through the class-wide fallback.
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
        &FxHashMap::default(),
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

/// A borrowed parameter whose contract carries an incoming whole-value credit
/// may need cleanup on an unwind edge before the body creates its first alias.
/// Replacement must materialize an entry alias for that owned credit: the
/// release cannot name the borrowed ABI parameter directly, and a later alias
/// does not dominate the early unwind block.
#[test]
fn replacement_materializes_entry_alias_for_credited_borrowed_param_unwind_cleanup() {
    let mut func = func_with_blocks(
        4,
        vec![
            block(0, vec![], vec![], invoke(1, vec![], 1, 2)),
            block(
                1,
                vec![],
                vec![
                    ArcInstr::Let {
                        dst: v(2),
                        ty: ty(0),
                        value: ArcValue::Var(v(0)),
                    },
                    is_shared(3, 2),
                ],
                ret(3),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    let mut own_contract = MemoryContract::conservative(1);
    own_contract.params[0].access = crate::aims::lattice::AccessClass::Borrowed;
    own_contract.params[0].iter_consumes = true;
    let contracts = FxHashMap::from_iter([(func.name, own_contract)]);
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    state_map.set_permanent_scalar(v(3));

    let outcome = attempt_replacement(
        &mut func,
        &state_map,
        &contracts,
        &ori_types::TypeRegistry::default(),
        &test_interner(),
        true,
    );

    assert_eq!(outcome.mode, EmissionMode::Replaced);
    let ArcInstr::Let {
        dst: entry_alias,
        value: ArcValue::Var(source),
        ..
    } = func.blocks[0]
        .body
        .first()
        .unwrap_or_else(|| panic!("credited borrowed param gets an entry alias"))
    else {
        panic!("entry instruction is not the credited-param alias")
    };
    assert_eq!(*source, v(0));
    assert_ne!(*entry_alias, v(0));
    assert!(func.blocks[2]
        .body
        .iter()
        .any(|instr| { matches!(instr, ArcInstr::BurdenDec { var } if var == entry_alias) }));
    assert!(func.blocks.iter().all(|block| {
        block
            .body
            .iter()
            .all(|instr| !matches!(instr, ArcInstr::BurdenDec { var } if *var == v(0)))
    }));
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
    // A loop projects and rebuilds the merge value on its back edge, then
    // projects it once more on exit.
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
    func.replace_variable_representations(vec![
        crate::ir::ValueRepr::Aggregate,
        crate::ir::ValueRepr::FatValue,
        crate::ir::ValueRepr::Scalar,
        crate::ir::ValueRepr::Scalar,
    ]);
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
    func.replace_variable_representations(vec![
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
    ]);
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
        super::events::EventFunding::ExtractionOwned,
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

/// Books-shape regression: a fresh sum container borrowed into an `Invoke`
/// whose callee contract certifies `return_alias = Project`. The credited
/// call-result arrival unions into the payload view class, the container
/// releases on both successor edges, and the view carries its own planned
/// release — every class verifies `Clean`. (Engagement for this shape is
/// pinned by `pipeline::tests::
/// class_ledger_replaces_contract_certified_payload_view_caller`.)
#[test]
fn credited_call_result_payload_view_books_stay_clean() {
    use crate::lower::test_utils::registered_struct_with_burden;
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};

    let mut func = func_with_blocks(
        5,
        vec![
            block(
                0,
                vec![],
                vec![
                    ArcInstr::Let {
                        dst: v(0),
                        ty: ty(3),
                        value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
                    },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: ty(64),
                        ctor: CtorKind::EnumVariant {
                            enum_name: Name::from_raw(9),
                            variant: 0,
                        },
                        args: vec![v(0)],
                    },
                ],
                invoke(2, vec![(1, ArgOwnership::Borrowed)], 1, 2),
            ),
            block(
                1,
                vec![],
                vec![apply(3, vec![(2, ArgOwnership::Borrowed)])],
                ret(4),
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    let container_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registered_struct_with_burden(
        &mut registry,
        "Wrapper",
        container_idx,
        Some(UserBurdenSpec {
            self_owned_identity: false,
            owned_fields: vec![UserOwnedField {
                field_path: vec![0],
                field_type: ty(3),
            }],
            ..Default::default()
        }),
    );
    func.var_types[1] = container_idx;
    func.var_types[2] = ty(3);
    let mut state_map = AimsStateMap::new(&func);
    for scalar in [3u32, 4] {
        state_map.set_permanent_scalar(v(scalar));
    }
    let mut aliases: FxHashMap<
        ArcVarId,
        crate::aims::intraprocedural::state_map::ApplyAliasSource,
    > = FxHashMap::default();
    aliases.insert(
        v(2),
        crate::aims::intraprocedural::state_map::ApplyAliasSource::Project {
            arg: v(1),
            field: 0,
        },
    );
    state_map.set_apply_result_aliases(aliases);
    let (analysis, mut partition) = analyze_with_registry(&func, &state_map, &registry);

    assert!(!analysis.field_view_hazard);
    assert!(analysis.readiness.all_classes_clean);
    let view = class_rep(&mut partition, 2);
    assert_eq!(verdict_for(&analysis, view), ClassVerdict::Clean);
    // The container releases on both successor edges while the credited
    // result carries its own planned release after its read.
    let container = class_rep(&mut partition, 1);
    assert!(
        ops_for(&analysis, container)
            .iter()
            .any(|op| op.kind == emit::PlannedOpKind::Dec),
        "container class plans its release"
    );
    assert!(
        ops_for(&analysis, view)
            .iter()
            .any(|op| op.kind == emit::PlannedOpKind::Dec && op.var == v(2)),
        "credited call-result carries its own planned release"
    );
}

/// A BORROWED user-`@drop`-typed param ADMITS despite carrying no planned
/// release of its own: the caller owns the release (RL-2 borrowed
/// discipline) — the user `@drop` impl body's own `self` is the canonical
/// case (the drop glue calls the body, then runs the release itself).
#[test]
fn replacement_admits_borrowed_user_drop_param_without_own_dec() {
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

    // The @drop-body shape: borrowed self, a read, no release of self.
    let mut func = one_block_func(2, vec![is_shared(1, 0)], ret(1));
    func.params = vec![ArcParam {
        var: v(0),
        ty: struct_idx,
        ownership: Ownership::Borrowed,
    }];
    func.var_types = vec![struct_idx, ty(0)];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let outcome = attempt_replacement(
        &mut func,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(
        outcome.mode,
        EmissionMode::Replaced,
        "a borrowed user-drop param is glue-released by the caller — the \
         plan owes it nothing"
    );
}

/// The @drop-body ALIAS shape: a `Let {{ Var }}` alias of the borrowed
/// user-drop self shares the caller-released allocation — the exemption
/// follows the alias chain to its borrowed-param root, never just the
/// param var itself.
#[test]
fn replacement_admits_borrowed_user_drop_param_alias() {
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

    // %1 = %0 (alias of borrowed self); %2 = IsShared %1; ret %2.
    let mut func = one_block_func(
        3,
        vec![
            ArcInstr::Let {
                dst: v(1),
                ty: struct_idx,
                value: ArcValue::Var(v(0)),
            },
            is_shared(2, 1),
        ],
        ret(2),
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: struct_idx,
        ownership: Ownership::Borrowed,
    }];
    func.var_types = vec![struct_idx, struct_idx, ty(0)];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let outcome = attempt_replacement(
        &mut func,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(
        outcome.mode,
        EmissionMode::Replaced,
        "the alias shares the borrowed param's caller-released allocation"
    );
}

/// A SCALAR-repr user-`@drop` value that dies normally is ADMITTED with its
/// drop obligation booked: the plan carries one whole-var release (lowered
/// with the scalar user-drop strategy — `@drop` runs exactly once at the
/// death point).
#[test]
fn replacement_admits_scalar_user_drop_value_with_planned_release() {
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

    // Owned scalar user-drop param, read (is_shared), dead at return — the
    // sum_nested/destructure family's core books.
    let mut func = one_block_func(2, vec![is_shared(1, 0)], ret(1));
    func.params = vec![ArcParam {
        var: v(0),
        ty: struct_idx,
        ownership: Ownership::Owned,
    }];
    func.var_types = vec![struct_idx, ty(0)];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(0));
    state_map.set_permanent_scalar(v(1));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let outcome = attempt_replacement(
        &mut func,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(
        outcome.mode,
        EmissionMode::Replaced,
        "a scalar user-drop value books its drop obligation and replaces"
    );
    assert!(
        func.blocks[0]
            .body
            .iter()
            .any(|instr| matches!(instr, ArcInstr::BurdenDec { var } if *var == v(0))),
        "the admitted scalar user-drop value carries its planned whole-var \
         release (the scalar user-drop strategy runs @drop exactly once)"
    );
}

/// The corpus main-fn shape: HEAP user-drop locals CONSUMED INTO an owning
/// `Construct` chain whose collection root carries the whole-var planned
/// release. The root's recursive drop glue runs each consumed value's
/// `@drop` (an RL-2 `Construct`-arg transfer), so the
/// consumed values owe no per-var release of their own.
#[test]
fn replacement_admits_heap_user_drop_locals_consumed_into_owner() {
    use core::num::NonZeroU32;
    use ori_registry::burden::FnSym;
    use ori_types::burden::UserBurdenSpec;

    let logged_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Logged",
        logged_idx,
        Some(UserBurdenSpec {
            user_drop: Some(FnSym::new(NonZeroU32::MIN)),
            ..UserBurdenSpec::default()
        }),
    );
    let resource_idx = ty(65);
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Resource",
        resource_idx,
        Some(UserBurdenSpec {
            user_drop: Some(FnSym::new(NonZeroU32::MIN)),
            ..UserBurdenSpec::default()
        }),
    );

    // %0 = Construct Struct(Logged)(); %1 = Construct Struct(Resource)(%0);
    // %2 = Construct List(%1); %3 = int literal; ret %3.
    let mut func = one_block_func(
        4,
        vec![
            ArcInstr::Construct {
                dst: v(0),
                ty: logged_idx,
                ctor: CtorKind::Struct(Name::from_raw(64)),
                args: vec![],
            },
            ArcInstr::Construct {
                dst: v(1),
                ty: resource_idx,
                ctor: CtorKind::Struct(Name::from_raw(65)),
                args: vec![v(0)],
            },
            ArcInstr::Construct {
                dst: v(2),
                ty: ty(66),
                ctor: CtorKind::ListLiteral,
                args: vec![v(1)],
            },
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
            },
        ],
        ret(3),
    );
    func.var_types = vec![logged_idx, resource_idx, ty(66), ty(0)];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let outcome = attempt_replacement(
        &mut func,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(
        outcome
            .fallback_reason
            .map(super::replace::FallbackReason::as_str),
        None,
        "consumed-into-owner shape declined"
    );
    assert_eq!(
        outcome.mode,
        EmissionMode::Replaced,
        "consumed-into-owner user-drop locals are released by the root's \
         recursive drop glue"
    );
}

/// The recursive-`@drop` corpus shape: the borrowed self's user-drop-typed
/// FIELD VIEW (`Project` of the self alias) is a borrow of the same
/// caller-owned allocation tree — the callee releases nothing for it
/// (TF-4 borrow; the drop glue walking the fields is the caller).
#[test]
fn replacement_admits_borrowed_self_user_drop_field_view() {
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

    // %1 = %0 (alias of borrowed self); %2 = Project %1.2 (user-drop field
    // view); %3 = IsShared %2; ret %3.
    let mut func = one_block_func(
        4,
        vec![
            ArcInstr::Let {
                dst: v(1),
                ty: struct_idx,
                value: ArcValue::Var(v(0)),
            },
            ArcInstr::Project {
                dst: v(2),
                ty: struct_idx,
                value: v(1),
                field: 2,
            },
            is_shared(3, 2),
        ],
        ret(3),
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: struct_idx,
        ownership: Ownership::Borrowed,
    }];
    func.var_types = vec![struct_idx, struct_idx, struct_idx, ty(0)];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let outcome = attempt_replacement(
        &mut func,
        &state_map,
        &contracts,
        &registry,
        &test_interner(),
        true,
    );
    assert_eq!(
        outcome
            .fallback_reason
            .map(super::replace::FallbackReason::as_str),
        None,
        "borrowed-self field view declined"
    );
    assert_eq!(
        outcome.mode,
        EmissionMode::Replaced,
        "a field view of the borrowed self is caller-released"
    );
}

/// The map-insert copy-out shape (RL-DROP §8.1.1): a user-drop local whose
/// alias is a borrowed `insert` arg on a map-literal receiver. The value is
/// runtime-copied into the map (the stored copy's teardown carries the
/// single `@drop`), so the class's placed releases rewrite FIELDS-ONLY
/// (`DecPartial` empty skip) and the shape replaces.
#[test]
fn replacement_admits_map_insert_copy_out_user_drop_local() {
    use core::num::NonZeroU32;
    use ori_registry::burden::FnSym;
    use ori_types::burden::UserBurdenSpec;

    let boom_idx = ty(64);
    let mut registry = ori_types::TypeRegistry::new();
    registry.register_struct(
        crate::lower::test_utils::test_name("Boom"),
        boom_idx,
        vec![],
        vec![ori_types::FieldDef {
            name: crate::lower::test_utils::test_name("tag"),
            ty: Idx::STR,
            span: ori_ir::Span::DUMMY,
            visibility: ori_types::Visibility::Public,
        }],
        ori_ir::Span::DUMMY,
        ori_types::Visibility::Public,
        0,
        None,
        Some(UserBurdenSpec {
            user_drop: Some(FnSym::new(NonZeroU32::MIN)),
            ..UserBurdenSpec::default()
        }),
    );

    let interner = test_interner();
    let insert = interner.intern("insert");
    // bb0: %0 = Construct Struct(Boom)(); %1 = Construct Map(); %2 = %0;
    //      Invoke @insert(%1 [own], %2 [borrow]) -> bb1 / unwind bb2
    // bb1: %4 = int literal; ret %4; bb2: Resume
    let mut func = func_with_blocks(
        5,
        vec![
            block(
                0,
                vec![],
                vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: boom_idx,
                        ctor: CtorKind::Struct(Name::from_raw(64)),
                        args: vec![],
                    },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: ty(66),
                        ctor: CtorKind::MapLiteral,
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: boom_idx,
                        value: ArcValue::Var(v(0)),
                    },
                ],
                ArcTerminator::Invoke {
                    dst: v(3),
                    ty: ty(66),
                    func: insert,
                    args: vec![v(1), v(2)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            ),
            block(
                1,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(4),
                    ty: ty(0),
                    value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                }],
                ArcTerminator::Return { value: v(4) },
            ),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.var_types = vec![boom_idx, ty(66), boom_idx, ty(66), ty(0)];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(4));
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();

    let outcome = attempt_replacement(
        &mut func, &state_map, &contracts, &registry, &interner, true,
    );
    assert_eq!(
        outcome
            .fallback_reason
            .map(super::replace::FallbackReason::as_str),
        None,
        "copy-out shape declined"
    );
    assert_eq!(outcome.mode, EmissionMode::Replaced);
    let partial = func.blocks.iter().flat_map(|b| &b.body).any(|instr| {
        matches!(instr, ArcInstr::BurdenDecPartial { skip_fields, .. } if skip_fields.is_empty())
    });
    assert!(
        partial,
        "the copy-out release is the fields-only empty-skip partial"
    );
}
