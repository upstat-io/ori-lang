use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition;
use crate::ir::{ArcBlock, ArcBlockId, ArcParam, CtorKind};

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

/// Populate the partition, classify, and hand back everything a test needs.
fn classify(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    facts: &FxHashMap<Name, BoundaryFacts>,
) -> (LedgerClassification, BirthSitePartition) {
    let mut partition = compute_birth_site_partition(func, state_map);
    let classification = classify_function(func, state_map, &mut partition, facts);
    (classification, partition)
}

fn no_facts() -> FxHashMap<Name, BoundaryFacts> {
    FxHashMap::default()
}

fn rep(partition: &mut BirthSitePartition, var: u32) -> NodeIdx {
    let node = partition.register_node(v(var), FieldPath::whole_var());
    partition.rep_of(node)
}

/// Flatten the per-block streams of a straight-line function into one walk.
fn flat(classification: &LedgerClassification) -> Vec<ClassInstr> {
    classification.blocks.iter().flatten().copied().collect()
}

/// The signed obligation net of a derived ledger: births + credits minus
/// consumes (reads and mutates are floors, not deltas).
fn net(events: &[LedgerEvent]) -> i64 {
    events
        .iter()
        .map(|e| match e {
            LedgerEvent::Birth | LedgerEvent::Credit => 1,
            LedgerEvent::Consume => -1,
            LedgerEvent::Read | LedgerEvent::Mutate { .. } => 0,
        })
        .sum()
}

// The committed RL-2 grid

/// Exhaustive 12-row grid: the transfer partition matches
/// `AimsProof.Realization::rl2_use_transfers_ownership` row-for-row —
/// 9 transfer kinds, 3 non-transfer kinds.
#[test]
fn terminal_use_table_matches_committed_rl2_grid() {
    use TerminalUse::*;
    let expected = [
        (Return, true),
        (ConstructArg, true),
        (ReuseArg, true),
        (CollectionReuseArg, true),
        (SetValue, true),
        (PartialApplyCapture, true),
        (ApplyToOwnedParam, true),
        (JumpArg, true),
        (ApplyToIterConsumingParam, true),
        (LastReadBeforeScopeExit, false),
        (ScopeExit, false),
        (ApplyToBorrowedParam, false),
    ];
    assert_eq!(expected.len(), TerminalUse::ALL.len());
    for (kind, transfers) in expected {
        assert_eq!(
            kind.transfers_ownership(),
            transfers,
            "transfer split diverged from the committed table on {kind:?}"
        );
    }
    let transfer_count = TerminalUse::ALL
        .iter()
        .filter(|k| k.transfers_ownership())
        .count();
    assert_eq!(transfer_count, 9);
}

/// The production adapter projects exactly the classification-relevant
/// contract facts (PV-4 `BoundaryContract.ofParamContract` composed).
#[test]
fn boundary_facts_project_the_contract() {
    let mut contract = MemoryContract::conservative(2);
    contract.params[0].iter_consumes = true;
    contract.params[1].transfers_through_return = true;
    contract.return_info.returns_sharing_view = true;
    contract.return_info.preserves_freshness = false;

    let facts = BoundaryFacts::from_contract(&contract);
    assert_eq!(facts.param_iter_consumes, vec![true, false]);
    assert_eq!(facts.param_transfers_through_return, vec![false, true]);
    assert!(facts.returns_sharing_view);
    assert!(!facts.returns_owned_fresh);
    assert!(facts.iter_consume_transfer(0));
    assert!(!facts.iter_consume_transfer(1));
    assert!(!facts.iter_consume_transfer(9));
}

// derive_ledger — the pure mirror of AimsProof.Ledger::deriveLedger

fn n(raw: u32) -> NodeIdx {
    // Tests mint distinct class reps through a scratch partition.
    let mut partition = BirthSitePartition::new();
    for i in 0..=raw {
        partition.register_node(v(i), FieldPath::whole_var());
    }
    partition.register_node(v(raw), FieldPath::whole_var())
}

#[test]
fn derive_ledger_filters_to_the_requested_class() {
    let (a, b) = (n(0), n(1));
    let instrs = [
        ClassInstr::Birth {
            class: a,
            origin: ClassOrigin::Fresh,
        },
        ClassInstr::Birth {
            class: b,
            origin: ClassOrigin::Fresh,
        },
        ClassInstr::Credit { class: b },
        ClassInstr::Consume { class: a },
        ClassInstr::Read {
            class: b,
            value: v(1),
        },
    ];
    assert_eq!(
        derive_ledger(a, &instrs),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    assert_eq!(
        derive_ledger(b, &instrs),
        vec![LedgerEvent::Birth, LedgerEvent::Credit, LedgerEvent::Read]
    );
}

/// Mirrors `sibReadCount`: distinct OTHER same-class values read in the
/// suffix — self-reads excluded, duplicates deduped, other classes ignored.
#[test]
fn derive_ledger_mutate_counts_distinct_suffix_sibling_reads() {
    let (a, b) = (n(0), n(1));
    let instrs = [
        ClassInstr::Mutate {
            class: a,
            value: v(0),
        },
        // Self-read: not a sibling.
        ClassInstr::Read {
            class: a,
            value: v(0),
        },
        // Two reads of ONE sibling: deduped to one.
        ClassInstr::Read {
            class: a,
            value: v(2),
        },
        ClassInstr::Read {
            class: a,
            value: v(2),
        },
        // A second distinct sibling.
        ClassInstr::Read {
            class: a,
            value: v(3),
        },
        // Another class: ignored.
        ClassInstr::Read {
            class: b,
            value: v(4),
        },
    ];
    let events = derive_ledger(a, &instrs);
    assert_eq!(events[0], LedgerEvent::Mutate { live_siblings: 2 });
}

/// A mutate at the end of the stream has zero live siblings; a read BEFORE
/// the mutate never counts (suffix only).
#[test]
fn derive_ledger_mutate_suffix_is_forward_only() {
    let a = n(0);
    let instrs = [
        ClassInstr::Read {
            class: a,
            value: v(2),
        },
        ClassInstr::Mutate {
            class: a,
            value: v(0),
        },
    ];
    let events = derive_ledger(a, &instrs);
    assert_eq!(
        events,
        vec![LedgerEvent::Read, LedgerEvent::Mutate { live_siblings: 0 }]
    );
}

// classify_function — IR walk

/// The walking-skeleton fresh-move shape: a fresh Construct returned. The
/// class births at the Construct and consumes at the Return — net 0, no
/// placed dec needed (the Return is a transfer kind).
#[test]
fn fresh_construct_return_births_and_consumes_net_zero() {
    let func = one_block_func(
        1,
        vec![construct(0, vec![])],
        ArcTerminator::Return { value: v(0) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let class = rep(&mut partition, 0);
    let events = derive_ledger(class, &flat(&classification));
    assert_eq!(events, vec![LedgerEvent::Birth, LedgerEvent::Consume]);
    assert_eq!(net(&events), 0);
    assert_eq!(
        classification.class_origins.get(&class),
        Some(&ClassOrigin::Fresh)
    );
}

/// Constructor arg funding is a `ConstructArg` transfer: the stored buffer's
/// class consumes at the store (the container inherits the obligation), and
/// the aggregate's class births separately.
#[test]
fn construct_arg_funding_consumes_the_stored_class() {
    let func = one_block_func(
        2,
        vec![construct(1, vec![]), construct(0, vec![1])],
        ArcTerminator::Return { value: v(0) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let buffer = rep(&mut partition, 1);
    let aggregate = rep(&mut partition, 0);
    assert_ne!(buffer, aggregate);

    let buffer_events = derive_ledger(buffer, &flat(&classification));
    assert_eq!(
        buffer_events,
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    let aggregate_events = derive_ledger(aggregate, &flat(&classification));
    assert_eq!(
        aggregate_events,
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
}

/// A Project is a borrow-view READ of the source aggregate's class; the
/// view variable joins the field class with no event of its own.
#[test]
fn project_reads_the_aggregate_class() {
    let func = one_block_func(
        3,
        vec![
            construct(1, vec![]),
            construct(0, vec![1]),
            ArcInstr::Project {
                dst: v(2),
                ty: ty(0),
                value: v(0),
                field: 0,
            },
        ],
        ArcTerminator::Return { value: v(0) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let aggregate = rep(&mut partition, 0);
    let events = derive_ledger(aggregate, &flat(&classification));
    assert_eq!(
        events,
        vec![LedgerEvent::Birth, LedgerEvent::Read, LedgerEvent::Consume]
    );
}

/// `Set` is a dynamic-COW MUTATE of the base class plus a `SetValue` transfer
/// CONSUME of the stored value's class; `SetTag` mutates only.
#[test]
fn set_mutates_base_and_consumes_value() {
    let func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            ArcInstr::Set {
                base: v(0),
                field: 0,
                value: v(1),
            },
            ArcInstr::SetTag { base: v(0), tag: 1 },
        ],
        ArcTerminator::Return { value: v(0) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let base = rep(&mut partition, 0);
    let stored = rep(&mut partition, 1);
    let base_events = derive_ledger(base, &flat(&classification));
    assert_eq!(
        base_events,
        vec![
            LedgerEvent::Birth,
            LedgerEvent::Mutate { live_siblings: 0 },
            LedgerEvent::Mutate { live_siblings: 0 },
            LedgerEvent::Consume,
        ]
    );
    let stored_events = derive_ledger(stored, &flat(&classification));
    assert_eq!(
        stored_events,
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
}

/// Owned function params birth FOREIGN; borrowed params birth BORROWED.
#[test]
fn function_params_birth_by_ownership() {
    let mut func = one_block_func(2, vec![], ArcTerminator::Return { value: v(0) });
    func.params = vec![
        ArcParam {
            var: v(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        },
        ArcParam {
            var: v(1),
            ty: ty(0),
            ownership: Ownership::Borrowed,
        },
    ];
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let owned = rep(&mut partition, 0);
    let borrowed = rep(&mut partition, 1);
    assert_eq!(
        classification.class_origins.get(&owned),
        Some(&ClassOrigin::Foreign)
    );
    assert_eq!(
        classification.class_origins.get(&borrowed),
        Some(&ClassOrigin::Borrowed)
    );
    assert_eq!(
        derive_ledger(owned, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    assert_eq!(
        derive_ledger(borrowed, &flat(&classification)),
        vec![LedgerEvent::Birth]
    );
}

/// Apply args classify by ownership annotation: Owned consumes
/// (`ApplyToOwnedParam`), Borrowed reads (`ApplyToBorrowedParam`). A
/// contract-less call result births OPAQUE.
#[test]
fn apply_args_classify_by_ownership_and_result_births_opaque() {
    let func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            ArcInstr::Apply {
                dst: v(2),
                ty: ty(0),
                func: Name::from_raw(7),
                args: vec![v(0), v(1)],
                arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                mono_instance_id: None,
            },
        ],
        ArcTerminator::Return { value: v(2) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let owned_arg = rep(&mut partition, 0);
    let borrowed_arg = rep(&mut partition, 1);
    let result = rep(&mut partition, 2);
    assert_eq!(
        derive_ledger(owned_arg, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    assert_eq!(
        derive_ledger(borrowed_arg, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Read]
    );
    assert_eq!(
        classification.class_origins.get(&result),
        Some(&ClassOrigin::Opaque)
    );
    assert_eq!(
        derive_ledger(result, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
}

/// The iter-consume contract fact (`iter_consumes && !transfers_through_return`)
/// overrides a Borrowed annotation into an RL-2 inward-transfer CONSUME.
#[test]
fn iter_consume_fact_overrides_borrowed_to_consume() {
    let callee = Name::from_raw(9);
    let func = one_block_func(
        2,
        vec![
            construct(0, vec![]),
            ArcInstr::Apply {
                dst: v(1),
                ty: ty(0),
                func: callee,
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            },
        ],
        ArcTerminator::Return { value: v(1) },
    );
    let mut facts = no_facts();
    facts.insert(
        callee,
        BoundaryFacts {
            param_iter_consumes: vec![true],
            param_transfers_through_return: vec![false],
            ..BoundaryFacts::default()
        },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &facts);

    let arg = rep(&mut partition, 0);
    assert_eq!(
        derive_ledger(arg, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
}

/// An RL-34 passthrough (Direct alias contract) credits the result class:
/// consume at the call, credit at the return — net 0 on ONE class.
#[test]
fn ttr_direct_alias_credits_the_result_class() {
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
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            },
        ],
        ArcTerminator::Return { value: v(1) },
    );
    let mut state_map = AimsStateMap::new(&func);
    let mut aliases: FxHashMap<ArcVarId, ApplyAliasSource> = FxHashMap::default();
    aliases.insert(v(1), ApplyAliasSource::Direct(v(0)));
    state_map.set_apply_result_aliases(aliases);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    // The Direct alias unified arg and result into ONE class.
    let class = rep(&mut partition, 0);
    assert_eq!(class, rep(&mut partition, 1));
    let events = derive_ledger(class, &flat(&classification));
    assert_eq!(
        events,
        vec![
            LedgerEvent::Birth,
            LedgerEvent::Consume,
            LedgerEvent::Credit,
            LedgerEvent::Consume,
        ]
    );
    assert_eq!(net(&events), 0);
}

/// A sharing-view producer contract credits the result class.
#[test]
fn sharing_view_producer_credits_the_result() {
    let callee = Name::from_raw(13);
    let func = one_block_func(
        2,
        vec![
            construct(0, vec![]),
            ArcInstr::Apply {
                dst: v(1),
                ty: ty(0),
                func: callee,
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            },
        ],
        ArcTerminator::Return { value: v(1) },
    );
    let mut facts = no_facts();
    facts.insert(
        callee,
        BoundaryFacts {
            param_iter_consumes: vec![false],
            param_transfers_through_return: vec![false],
            returns_sharing_view: true,
            returns_owned_fresh: false,
        },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &facts);

    let result = rep(&mut partition, 1);
    assert_eq!(
        derive_ledger(result, &flat(&classification)),
        vec![LedgerEvent::Credit, LedgerEvent::Consume]
    );
}

/// A same-class jump arg is the RL-4 exemption: silent. A single-pred param
/// is tier-1-unified with its arg, so the hand-off emits nothing.
#[test]
fn same_class_jump_arg_is_silent() {
    let func = func_with_blocks(
        2,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(1, vec![0])),
            block(1, vec![1], vec![], ArcTerminator::Return { value: v(1) }),
        ],
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let class = rep(&mut partition, 0);
    assert_eq!(class, rep(&mut partition, 1));
    let events = derive_ledger(class, &flat(&classification));
    assert_eq!(events, vec![LedgerEvent::Birth, LedgerEvent::Consume]);
    assert_eq!(net(&events), 0);
}

/// A REFUSED merge (two distinct birth sites) is cross-class per edge:
/// each predecessor's jump consumes its source class and credits the
/// param's class; the param class's origin is MERGE with no birth event.
#[test]
fn refused_merge_cross_class_jump_consumes_and_credits() {
    let func = func_with_blocks(
        3,
        vec![
            block(0, vec![], vec![construct(0, vec![])], jump(2, vec![0])),
            block(1, vec![], vec![construct(1, vec![])], jump(2, vec![1])),
            block(2, vec![2], vec![], ArcTerminator::Return { value: v(2) }),
        ],
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let entry_class = rep(&mut partition, 0);
    let latch_class = rep(&mut partition, 1);
    let merge_class = rep(&mut partition, 2);
    assert_ne!(merge_class, entry_class);
    assert_ne!(merge_class, latch_class);

    assert_eq!(
        classification.class_origins.get(&merge_class),
        Some(&ClassOrigin::Merge)
    );
    assert_eq!(
        derive_ledger(entry_class, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    assert_eq!(
        derive_ledger(latch_class, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    // Per WALK exactly one predecessor executes: credit then return-consume.
    let merge_events = derive_ledger(merge_class, &flat(&classification));
    assert_eq!(
        merge_events,
        vec![
            LedgerEvent::Credit,
            LedgerEvent::Credit,
            LedgerEvent::Consume
        ]
    );
}

/// Scalar/immortal-excluded vars produce no events anywhere.
#[test]
fn excluded_vars_produce_no_events() {
    let func = one_block_func(
        2,
        vec![
            construct(0, vec![]),
            ArcInstr::IsShared {
                dst: v(1),
                var: v(0),
            },
        ],
        ArcTerminator::Return { value: v(0) },
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(0));
    state_map.set_permanent_scalar(v(1));
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let classification = classify_function(&func, &state_map, &mut partition, &no_facts());

    assert!(flat(&classification).is_empty());
    assert!(classification.class_origins.is_empty());
}

/// `IsShared` and `PrimOp` operands are READ positions.
#[test]
fn is_shared_and_primop_operands_read() {
    let func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            ArcInstr::IsShared {
                dst: v(1),
                var: v(0),
            },
            ArcInstr::Let {
                dst: v(2),
                ty: ty(0),
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Eq),
                    args: vec![v(0), v(0)],
                },
            },
        ],
        ArcTerminator::Return { value: v(0) },
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(1));
    state_map.set_permanent_scalar(v(2));
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let class = rep(&mut partition, 0);
    let events = derive_ledger(class, &flat(&classification));
    assert_eq!(
        events,
        vec![
            LedgerEvent::Birth,
            LedgerEvent::Read,
            LedgerEvent::Read,
            LedgerEvent::Read,
            LedgerEvent::Consume,
        ]
    );
}

/// `PartialApply` captures transfer (`PartialApplyCapture`) and the closure
/// births FRESH.
#[test]
fn partial_apply_captures_consume_and_closure_births_fresh() {
    let func = one_block_func(
        2,
        vec![
            construct(0, vec![]),
            ArcInstr::PartialApply {
                dst: v(1),
                ty: ty(0),
                func: Name::from_raw(3),
                args: vec![v(0)],
            },
        ],
        ArcTerminator::Return { value: v(1) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let captured = rep(&mut partition, 0);
    let closure = rep(&mut partition, 1);
    assert_eq!(
        derive_ledger(captured, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    assert_eq!(
        classification.class_origins.get(&closure),
        Some(&ClassOrigin::Fresh)
    );
    assert_eq!(
        derive_ledger(closure, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
}

/// Placed burden ops classify per the calculus: `BurdenInc` is the placed
/// dup (CREDIT); `BurdenDec` is the placed release (CONSUME).
#[test]
fn placed_burden_ops_classify_as_credit_and_consume() {
    let func = one_block_func(
        1,
        vec![
            construct(0, vec![]),
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
        ArcTerminator::Return { value: v(0) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let class = rep(&mut partition, 0);
    let events = derive_ledger(class, &flat(&classification));
    assert_eq!(
        events,
        vec![
            LedgerEvent::Birth,
            LedgerEvent::Credit,
            LedgerEvent::Consume,
            LedgerEvent::Consume,
        ]
    );
    assert_eq!(net(&events), 0);
}

/// The walking-skeleton fresh-read shape end-to-end: a fresh allocation
/// read then dead nets +1 (the emitter owes exactly one placed dec after the
/// last READ) — the classifier surfaces the obligation, never places it.
#[test]
fn fresh_read_shape_nets_plus_one_owed_release() {
    let func = one_block_func(
        3,
        vec![
            construct(1, vec![]),
            construct(0, vec![1]),
            ArcInstr::Project {
                dst: v(2),
                ty: ty(0),
                value: v(0),
                field: 0,
            },
        ],
        // The aggregate is NOT returned: its class ends the walk still owed.
        ArcTerminator::Return { value: v(2) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let aggregate = rep(&mut partition, 0);
    let events = derive_ledger(aggregate, &flat(&classification));
    assert_eq!(events, vec![LedgerEvent::Birth, LedgerEvent::Read]);
    assert_eq!(net(&events), 1);
}
