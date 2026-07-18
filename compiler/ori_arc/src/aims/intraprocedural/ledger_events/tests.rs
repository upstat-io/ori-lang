//! Tests for the per-class ledger-event classifier.

use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition;
use crate::aims::intraprocedural::state_map::ApplyAliasSource;
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcParam, ArcValue, ArgOwnership, CtorKind};

use super::*;

fn test_interner() -> ori_ir::StringInterner {
    ori_ir::StringInterner::new()
}

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

fn freeze_primitive(func: &mut ArcFunction, dst: u32, strategy: ori_registry::OpStrategy) {
    let Some(fact) = crate::ir::PrimitiveFact::resolve(strategy, 2) else {
        panic!("expected a valid binary primitive descriptor");
    };
    assert!(func.primitive_facts.insert(v(dst), fact).is_none());
}

/// Populate the partition, classify, and hand back everything a test needs.
fn classify(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    facts: &FxHashMap<Name, BoundaryFacts>,
) -> (LedgerClassification, BirthSitePartition) {
    let mut partition = compute_birth_site_partition(func, state_map);
    let classification =
        classify_function(func, state_map, &mut partition, facts, &test_interner());
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
    contract.params[1].borrowed_cow_consumed = true;
    contract.return_info.returns_sharing_view = true;
    contract.return_info.preserves_freshness = false;

    let facts = BoundaryFacts::from_contract(&contract);
    assert_eq!(facts.param_iter_consumes, vec![true, false]);
    assert_eq!(facts.param_borrowed_cow_consumed, vec![false, true]);
    assert_eq!(facts.param_transfers_through_return, vec![false, true]);
    assert!(facts.returns_sharing_view);
    assert!(!facts.returns_owned_fresh);
    assert!(facts.iter_consume_transfer(0));
    assert!(facts.borrowed_cow_consume_funding(1));
    assert!(facts.incoming_whole_value_credit(0));
    assert!(facts.incoming_whole_value_credit(1));
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

/// A non-excluded heap literal (a non-empty string) is a fresh allocation:
/// its class births FRESH at the `Let` and consumes at the Return transfer —
/// net 0, mirroring the Construct shape (TF-3 analog; only `""` is immortal
/// per the immortal pre-pass, so a non-empty literal is RC-carrying).
#[test]
fn str_literal_let_births_fresh_and_return_consumes_net_zero() {
    let func = one_block_func(
        1,
        vec![ArcInstr::Let {
            dst: v(0),
            ty: ty(0),
            value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
        }],
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

/// An EXCLUDED literal (scalar or immortal per the state map) stays out of
/// the event stream entirely — no birth, no class.
#[test]
fn excluded_literal_let_emits_no_events() {
    let func = one_block_func(
        2,
        vec![
            ArcInstr::Let {
                dst: v(0),
                ty: ty(0),
                value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
            },
            construct(1, vec![]),
        ],
        ArcTerminator::Return { value: v(1) },
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_immortals(vec![true, false]);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let literal_class = rep(&mut partition, 0);
    let events = derive_ledger(literal_class, &flat(&classification));
    assert!(events.is_empty());
    assert!(!classification.class_origins.contains_key(&literal_class));
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

/// `trace` renders a FRESH owned str (`_ori_format_error_trace` returns
/// `OriStr::from_owned`), so its call result books an owned arrival
/// (Birth) the Return consume balances — it is NOT a borrow-view accessor.
#[test]
fn trace_result_books_owned_arrival_birth() {
    let interner = test_interner();
    let callee = interner.intern("trace");
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
    let state_map = AimsStateMap::new(&func);
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let classification =
        classify_function(&func, &state_map, &mut partition, &no_facts(), &interner);

    let result = rep(&mut partition, 1);
    assert_eq!(
        derive_ledger(result, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume],
        "the fresh trace render is an owned arrival consumed by Return"
    );
}

/// `trace_entries` loads the receiver's interior trace-list fat pointer with
/// NO retain (a genuine borrow view of an INTERIOR allocation the receiver's
/// own release frees), so its result books a BORROWED-origin arrival: owed 0,
/// floor-0 reads, funded consumes — never an Opaque birth whose planned
/// release would free the interior list out from under the receiver.
#[test]
fn trace_entries_result_books_borrowed_arrival() {
    let interner = test_interner();
    let callee = interner.intern("trace_entries");
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
        ArcTerminator::Return { value: v(0) },
    );
    let state_map = AimsStateMap::new(&func);
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let classification =
        classify_function(&func, &state_map, &mut partition, &no_facts(), &interner);

    let result = rep(&mut partition, 1);
    assert_eq!(
        classification.class_origins.get(&result).copied(),
        Some(ClassOrigin::Borrowed),
        "the retain-less interior view is a borrowed-origin arrival"
    );
    assert_eq!(
        derive_ledger(result, &flat(&classification)),
        vec![LedgerEvent::Birth],
        "borrowed arrival: owed 0, no release planned for the view itself"
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

/// A borrowed-COW-consuming user boundary retains the caller's original
/// owner and transfers a separately funded owner to the callee. The ordered
/// CONSUME+READ pair is the class-ledger shape that plans the funding inc and
/// the caller's post-call release.
#[test]
fn borrowed_cow_consumed_boundary_books_transfer_and_retained_owner() {
    let callee = Name::from_raw(10);
    let func = one_block_func(
        2,
        vec![
            construct(0, vec![]),
            ArcInstr::Apply {
                dst: v(1),
                ty: ty(1),
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
            param_borrowed_cow_consumed: vec![true],
            ..BoundaryFacts::default()
        },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &facts);

    let arg = rep(&mut partition, 0);
    assert_eq!(
        derive_ledger(arg, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume, LedgerEvent::Read,]
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
            param_borrowed_cow_consumed: vec![false],
            param_transfers_through_return: vec![false],
            param_cardinality_absent: vec![false],
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
    let classification = classify_function(
        &func,
        &state_map,
        &mut partition,
        &no_facts(),
        &test_interner(),
    );

    assert!(flat(&classification).is_empty());
    assert!(classification.class_origins.is_empty());
}

/// `IsShared` and `PrimOp` operands are READ positions.
#[test]
fn is_shared_and_primop_operands_read() {
    let mut func = one_block_func(
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
    freeze_primitive(&mut func, 2, ori_registry::OpStrategy::SignedInteger);
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

/// `Select` is a conditional-alias READ of every operand (cond + both
/// branches); the dst is EXCLUDED from partition admission (per
/// `birth_site_partition`'s distinct-site refusal), so it carries no birth
/// of its own — the selected allocation's obligation stays with its source
/// class.
#[test]
fn select_reads_cond_and_both_branch_operands() {
    let func = one_block_func(
        4,
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
        ArcTerminator::Return { value: v(2) },
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(3));
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let true_branch = rep(&mut partition, 0);
    let false_branch = rep(&mut partition, 1);
    assert_eq!(
        derive_ledger(true_branch, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Read]
    );
    assert_eq!(
        derive_ledger(false_branch, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Read]
    );
    assert!(!classification
        .class_origins
        .contains_key(&rep(&mut partition, 2)));
}

/// `BurdenDecField` is the field-grain placed release: it consumes the
/// field-path class (distinct from the whole-var `base` class), never the
/// base's whole-var class.
#[test]
fn burden_dec_field_consumes_the_field_path_class() {
    let func = one_block_func(
        1,
        vec![
            construct(0, vec![]),
            ArcInstr::BurdenDecField {
                base: v(0),
                field: 2,
            },
        ],
        ArcTerminator::Return { value: v(0) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let whole = rep(&mut partition, 0);
    let field_node = partition.register_node(v(0), FieldPath::single(2));
    let field_class = partition.rep_of(field_node);
    assert_ne!(
        whole, field_class,
        "field-grain release targets its own class"
    );

    assert_eq!(
        derive_ledger(whole, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume],
        "the whole-var class still owes its Return-terminator consume"
    );
    assert_eq!(
        derive_ledger(field_class, &flat(&classification)),
        vec![LedgerEvent::Consume],
        "BurdenDecField consumes the FIELD class, not the base's whole-var class"
    );
}

/// Realized (Phase-7-lowered) RC ops are not uses per TF-11 — the classifier
/// runs before placement, and `RcInc`/`RcDec` in view belong to the legacy
/// path the toggle keeps disjoint (per `classify_placed_op`'s fallback arm).
/// A construct followed by a realized inc/dec pair with NO burden ops
/// produces only the construct's Birth (plus the terminator's consume) —
/// zero Credit/extra-Consume events from the realized ops themselves.
#[test]
fn realized_rc_ops_produce_no_ledger_events() {
    let func = one_block_func(
        1,
        vec![
            construct(0, vec![]),
            ArcInstr::RcInc {
                var: v(0),
                count: 1,
                strategy: crate::ir::RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            },
            ArcInstr::RcDec {
                var: v(0),
                strategy: crate::ir::RcStrategy::HeapPointer,
                atomicity: crate::ir::RcAtomicity::default_atomic(),
            },
        ],
        ArcTerminator::Return { value: v(0) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let class = rep(&mut partition, 0);
    assert_eq!(
        derive_ledger(class, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume],
        "realized RcInc/RcDec must classify as no-ops, never Credit/Consume"
    );
}

/// `CollectionReuse` consumes the recycled `old_var`'s class (transfer, per
/// the committed table) AND funds a fresh allocation exactly like
/// `Construct`/`Reuse`: the dst births FRESH and every new-element arg is
/// consumed into it.
#[test]
fn collection_reuse_consumes_old_var_and_funds_new_args() {
    let func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            ArcInstr::CollectionReuse {
                old_var: v(0),
                dst: v(2),
                ty: ty(0),
                ctor: CtorKind::ListLiteral,
                args: vec![v(1)],
            },
        ],
        ArcTerminator::Return { value: v(2) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let old = rep(&mut partition, 0);
    let elem = rep(&mut partition, 1);
    let new_collection = rep(&mut partition, 2);
    assert_eq!(
        derive_ledger(old, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume],
        "the recycled old_var is CONSUMEd (transfer terminal use)"
    );
    assert_eq!(
        derive_ledger(elem, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    assert_eq!(
        classification.class_origins.get(&new_collection),
        Some(&ClassOrigin::Fresh)
    );
    assert_eq!(
        derive_ledger(new_collection, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
}

/// `ApplyIndirect` has no contract: the closure receiver and args are
/// conservative READs; the result BIRTHS OPAQUE (mirrors `Apply`'s
/// contract-less arm, distinct code path via the indirect terminator/instr
/// dispatch).
#[test]
fn apply_indirect_reads_closure_and_args_result_births_opaque() {
    let func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            ArcInstr::ApplyIndirect {
                dst: v(2),
                ty: ty(0),
                closure: v(0),
                args: vec![v(1)],
                arg_ownership: vec![],
            },
        ],
        ArcTerminator::Return { value: v(2) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let closure = rep(&mut partition, 0);
    let arg = rep(&mut partition, 1);
    let result = rep(&mut partition, 2);
    assert_eq!(
        derive_ledger(closure, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Read]
    );
    assert_eq!(
        derive_ledger(arg, &flat(&classification)),
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

/// An `Invoke` result materializes ONLY on the normal edge: its birth event
/// lands in the NORMAL successor's stream (block-entry site), never in the
/// invoking block, so the unwind path inherits no owed count for a value
/// that never existed there (PV-4: the boundary credit lands where the
/// return lands).
#[test]
fn invoke_result_births_in_normal_successor_not_invoking_block() {
    // bb0: %0 = Construct; Invoke f(%0) -> %1, normal bb1, unwind bb2
    // bb1: Return %1
    // bb2: Resume
    let func = func_with_blocks(
        2,
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
            block(1, vec![], vec![], ArcTerminator::Return { value: v(1) }),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let result = rep(&mut partition, 1);
    let births_in = |block: usize| {
        classification.blocks[block]
            .iter()
            .any(|instr| matches!(instr, ClassInstr::Birth { class, .. } if *class == result))
    };
    assert!(
        !births_in(0),
        "invoke result must not birth in the invoking block"
    );
    assert!(
        births_in(1),
        "invoke result births at the normal successor's entry"
    );
    assert!(!births_in(2), "the unwind path never sees the result");
    assert!(classification.blocks[2].is_empty());
}

/// A heap-producing `PrimOp` (list concat) is a fresh allocation whose
/// `RcPointer` operands are CONSUMED: the dual-consuming
/// `ori_list_concat_cow` takes over or releases both inputs, so the
/// non-excluded dst births FRESH and each operand hands its reference in.
/// (`FatValue` str operands are BORROWED and READ instead — pinned by
/// `str_concat_operand_reads_not_consumes`.) Scalar `PrimOp` dsts remain
/// state-map-excluded; their heap operands are comparison borrow-READS.
#[test]
fn heap_primop_dst_births_fresh_and_rcptr_operands_consumed() {
    let mut func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            ArcInstr::Let {
                dst: v(2),
                ty: ty(0),
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![v(0), v(1)],
                },
            },
        ],
        ArcTerminator::Return { value: v(2) },
    );
    func.replace_variable_representations(vec![
        crate::ir::ValueRepr::RcPointer,
        crate::ir::ValueRepr::RcPointer,
        crate::ir::ValueRepr::RcPointer,
    ]);
    freeze_primitive(
        &mut func,
        2,
        ori_registry::OpStrategy::RuntimeCall(ori_registry::RuntimeOperator::ListConcat),
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let result = rep(&mut partition, 2);
    let events = derive_ledger(result, &flat(&classification));
    assert_eq!(events, vec![LedgerEvent::Birth, LedgerEvent::Consume]);
    assert_eq!(
        classification.class_origins.get(&result),
        Some(&ClassOrigin::Fresh)
    );
    let lhs = rep(&mut partition, 0);
    assert_eq!(
        derive_ledger(lhs, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    let rhs = rep(&mut partition, 1);
    assert_eq!(
        derive_ledger(rhs, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    // The dual-consume model is COMPLETE: no poison, no readiness fallback
    // (COW uniqueness selects the runtime strategy, never the refcount
    // contract), so concat-bearing functions stay ledger-replaced.
    assert!(!classification.indirect_arg_handoff);
}

/// A NON-STRING literal under a non-excluded (heap-repr) variable — the
/// iterator-protocol placeholder shape `%n: str = 0` — allocates nothing at
/// runtime: no birth, no events, no release (a planned dec on it would
/// release a non-pointer).
#[test]
fn non_string_literal_under_heap_var_emits_no_events() {
    let func = one_block_func(
        2,
        vec![
            ArcInstr::Let {
                dst: v(0),
                ty: ty(0),
                value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
            },
            construct(1, vec![]),
        ],
        ArcTerminator::Return { value: v(1) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let placeholder = rep(&mut partition, 0);
    assert!(derive_ledger(placeholder, &flat(&classification)).is_empty());
    assert!(!classification.class_origins.contains_key(&placeholder));
}

/// A var defined by a NON-string literal under a heap repr (the iterator
/// placeholder `%n: [str] = 0`) is a placeholder, not an allocation: NO
/// events attach to it anywhere — not even reads (a borrowed-arg read of
/// it would demand an owned reference that cannot exist).
#[test]
fn placeholder_literal_var_is_event_less_everywhere() {
    let callee = Name::from_raw(9);
    let func = one_block_func(
        2,
        vec![
            ArcInstr::Let {
                dst: v(0),
                ty: ty(0),
                value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
            },
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
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let placeholder = rep(&mut partition, 0);
    assert!(
        derive_ledger(placeholder, &flat(&classification)).is_empty(),
        "no events for a placeholder literal"
    );
}

/// A `Select` whose value operands are ALL excluded (placeholders /
/// immortals / scalars) holds an excluded value itself: no events attach
/// to the dst anywhere — its later jump hand-off seeds the receiving param
/// with an immortal credit instead of an unfunded consume.
#[test]
fn select_of_excluded_operands_is_excluded() {
    // %0, %1: non-string literals under heap repr (placeholders)
    // %2 = Select %3 ? %0 : %1
    // jump bb1(%2); bb1(%4): Return %5
    let func = func_with_blocks(
        6,
        vec![
            block(
                0,
                vec![],
                vec![
                    ArcInstr::Let {
                        dst: v(0),
                        ty: ty(0),
                        value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                    },
                    ArcInstr::Let {
                        dst: v(1),
                        ty: ty(0),
                        value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                    },
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
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    // The excluded-select hand-off seeds the receiving class with an
    // immortal Credit — never an unfunded Consume (the pre-fix shape).
    let param = rep(&mut partition, 4);
    assert_eq!(
        derive_ledger(param, &flat(&classification)),
        vec![LedgerEvent::Credit]
    );
    let selected = rep(&mut partition, 2);
    let selected_events = derive_ledger(selected, &flat(&classification));
    assert!(
        !selected_events.contains(&LedgerEvent::Consume),
        "no unfunded consume on the select class: {selected_events:?}"
    );
}

/// A whole-var alias of an EXCLUDED var (an immortal, e.g. the empty
/// string) is excluded itself: reads of the alias are event-less, so no
/// unfunded floor accrues on the alias's class.
#[test]
fn alias_of_excluded_var_is_excluded() {
    // %0: immortal; %1 = Let Var(%0); %2 = PrimOp(%1 == %1)
    let mut func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            ArcInstr::Let {
                dst: v(1),
                ty: ty(0),
                value: ArcValue::Var(v(0)),
            },
            ArcInstr::Let {
                dst: v(2),
                ty: ty(0),
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Eq),
                    args: vec![v(1), v(1)],
                },
            },
        ],
        ArcTerminator::Return { value: v(2) },
    );
    freeze_primitive(&mut func, 2, ori_registry::OpStrategy::SignedInteger);
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_immortals(vec![true, false, false]);
    state_map.set_permanent_scalar(v(2));
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let alias = rep(&mut partition, 1);
    assert!(
        derive_ledger(alias, &flat(&classification)).is_empty(),
        "no events on the excluded-alias class"
    );
}

/// A STR concat operand (`FatValue` repr) is BORROWED by `ori_str_concat`
/// (the runtime reads both inputs and builds a fresh result; the caller's
/// own dec is the operand's release) — the operand's class READs at the
/// concat, keeping its Birth's owed release with the planner. Classifying
/// it Consume plans no release and funds a spurious dup inc on a
/// borrowed-rooted operand (the b003 lazy-iter lambda +2 leak shape).
#[test]
fn str_concat_operand_reads_not_consumes() {
    let mut func = one_block_func(
        3,
        vec![
            ArcInstr::Let {
                dst: v(0),
                ty: ty(0),
                value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
            },
            ArcInstr::Let {
                dst: v(1),
                ty: ty(1),
                value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(4))),
            },
            ArcInstr::Let {
                dst: v(2),
                ty: ty(2),
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![v(0), v(1)],
                },
            },
        ],
        ArcTerminator::Return { value: v(2) },
    );
    func.replace_variable_representations(vec![
        crate::ir::ValueRepr::FatValue,
        crate::ir::ValueRepr::FatValue,
        crate::ir::ValueRepr::FatValue,
    ]);
    freeze_primitive(
        &mut func,
        2,
        ori_registry::OpStrategy::RuntimeCall(ori_registry::RuntimeOperator::StringConcat),
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let operand = rep(&mut partition, 0);
    let events = derive_ledger(operand, &flat(&classification));
    assert_eq!(
        events,
        vec![LedgerEvent::Birth, LedgerEvent::Read],
        "a borrowed str concat operand READs; its Birth keeps the owed release"
    );
    assert_eq!(net(&events), 1, "the operand still owes its own release");
}

/// A LIST concat operand (`RcPointer` repr) transfers into the
/// dual-consuming `ori_list_concat_cow` (unique buffers taken over, shared
/// ones released by the runtime) — the operand's class CONSUMEs at the
/// concat, net 0, no planner-placed release.
#[test]
fn list_concat_operand_consumes() {
    let mut func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            ArcInstr::Let {
                dst: v(2),
                ty: ty(2),
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![v(0), v(1)],
                },
            },
        ],
        ArcTerminator::Return { value: v(2) },
    );
    func.replace_variable_representations(vec![
        crate::ir::ValueRepr::RcPointer,
        crate::ir::ValueRepr::RcPointer,
        crate::ir::ValueRepr::RcPointer,
    ]);
    freeze_primitive(
        &mut func,
        2,
        ori_registry::OpStrategy::RuntimeCall(ori_registry::RuntimeOperator::ListConcat),
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let operand = rep(&mut partition, 0);
    let events = derive_ledger(operand, &flat(&classification));
    assert_eq!(
        events,
        vec![LedgerEvent::Birth, LedgerEvent::Consume],
        "a list concat operand transfers into the dual-consuming runtime concat"
    );
    assert_eq!(net(&events), 0);
}

/// A BORROWED param this function's OWN contract marks iter-consuming
/// (PV-4: the caller classified its arg `ApplyToIterConsumingParam` and
/// transferred the reference in) arrives OWNING that reference — it births
/// FOREIGN like an owned param. Borrowed-origin classification would make
/// the emitter self-fund the internal `@iter [own]` hand-off the caller's
/// transfer already pays for (the double-funded whole-collection leak on
/// the caught-panic `fat_ptr_iter` shape).
#[test]
fn borrowed_iter_consuming_param_births_foreign() {
    let mut func = one_block_func(1, vec![], ArcTerminator::Return { value: v(0) });
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    let mut facts = no_facts();
    facts.insert(
        func.name,
        BoundaryFacts {
            param_iter_consumes: vec![true],
            param_transfers_through_return: vec![false],
            ..BoundaryFacts::default()
        },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &facts);

    let param = rep(&mut partition, 0);
    assert_eq!(
        classification.class_origins.get(&param),
        Some(&ClassOrigin::Foreign),
        "an iter-consuming borrowed param owns its transferred-in reference"
    );
}

/// Borrowed-COW consumption also arrives with a caller-funded whole-value
/// owner. The callee must classify it FOREIGN so its internal owned handoff
/// transfers that credit instead of minting a second one.
#[test]
fn borrowed_cow_consuming_param_births_foreign() {
    let mut func = one_block_func(1, vec![], ArcTerminator::Return { value: v(0) });
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    let mut facts = no_facts();
    facts.insert(
        func.name,
        BoundaryFacts {
            param_borrowed_cow_consumed: vec![true],
            ..BoundaryFacts::default()
        },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &facts);

    let param = rep(&mut partition, 0);
    assert_eq!(
        classification.class_origins.get(&param),
        Some(&ClassOrigin::Foreign),
        "a borrowed-COW-consuming param owns its caller-funded reference"
    );
}

/// A borrowed-COW-consuming param's alias is not a borrowed-root iterator
/// receiver: its caller-funded owner transfers into the iterator.
#[test]
fn funded_borrowed_rooted_iter_arg_classifies_consume() {
    let interner = test_interner();
    let iter_name = interner.intern("iter");
    let mut func = func_with_blocks(
        3,
        vec![
            block(
                0,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(1),
                    ty: ty(0),
                    value: ArcValue::Var(v(0)),
                }],
                ArcTerminator::Invoke {
                    dst: v(2),
                    ty: ty(1),
                    func: iter_name,
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            ),
            block(1, vec![], vec![], ArcTerminator::Return { value: v(2) }),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    let mut facts = no_facts();
    facts.insert(
        func.name,
        BoundaryFacts {
            param_borrowed_cow_consumed: vec![true],
            ..BoundaryFacts::default()
        },
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let classification = classify_function(&func, &state_map, &mut partition, &facts, &interner);

    let param_class = rep(&mut partition, 0);
    assert_eq!(
        derive_ledger(param_class, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume],
        "the funded borrowed root transfers its incoming credit into @iter"
    );
}

/// The same borrowed param WITHOUT the iter-consume contract fact stays
/// BORROWED-origin (the caller retains ownership; reads are free).
#[test]
fn borrowed_non_iter_consuming_param_stays_borrowed() {
    let mut func = one_block_func(1, vec![], ArcTerminator::Return { value: v(0) });
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    let param = rep(&mut partition, 0);
    assert_eq!(
        classification.class_origins.get(&param),
        Some(&ClassOrigin::Borrowed)
    );
}

/// A borrowed-rooted collection handed to the `@iter` protocol builtin at
/// an OWNED arg position classifies READ: the emitter creates a NON-owning
/// iterator for a borrowed-rooted receiver (no source dec inside), so the
/// owner/caller releases the source (`RL2_borrowed_param_emits_caller_dec`)
/// and a CONSUME here would demand funding no runtime release matches.
#[test]
fn borrowed_rooted_iter_arg_classifies_read() {
    let interner = test_interner();
    let iter_name = interner.intern("iter");
    let mut func = func_with_blocks(
        3,
        vec![
            block(
                0,
                vec![],
                vec![ArcInstr::Let {
                    dst: v(1),
                    ty: ty(0),
                    value: ArcValue::Var(v(0)),
                }],
                ArcTerminator::Invoke {
                    dst: v(2),
                    ty: ty(0),
                    func: iter_name,
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            ),
            block(1, vec![], vec![], ArcTerminator::Return { value: v(2) }),
            block(2, vec![], vec![], ArcTerminator::Resume),
        ],
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_permanent_scalar(v(2));
    let mut partition = compute_birth_site_partition(&func, &state_map);
    let classification =
        classify_function(&func, &state_map, &mut partition, &no_facts(), &interner);

    let param_class = rep(&mut partition, 0);
    assert_eq!(
        derive_ledger(param_class, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Read],
        "a borrowed-rooted @iter arg is a READ, never a consume"
    );
}

/// A HEAP arg handed through an indirect call with NO ownership
/// annotation is an UNMODELED hand-off: the classification carries the
/// poison flag so the readiness gate falls back — guessing READ
/// double-frees a consuming callee (the curried-closure capture shape);
/// guessing CONSUME leaks a borrowing one.
#[test]
fn indirect_heap_arg_sets_handoff_flag() {
    let func = one_block_func(
        3,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            ArcInstr::ApplyIndirect {
                dst: v(2),
                ty: ty(0),
                closure: v(1),
                args: vec![v(0)],
                arg_ownership: vec![],
            },
        ],
        ArcTerminator::Return { value: v(2) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, _) = classify(&func, &state_map, &no_facts());
    assert!(
        classification.indirect_arg_handoff,
        "a heap arg through ApplyIndirect poisons the classification"
    );
}

/// A POPULATED `arg_ownership` (the Step-4b prelude runs
/// `emit_arg_ownership` before classification) resolves the hand-off:
/// an Owned indirect arg classifies CONSUME, a Borrowed one READ — the
/// same annotation source direct no-contract calls classify by — and the
/// classification carries no poison.
#[test]
fn indirect_annotated_args_classify_without_handoff_flag() {
    let func = one_block_func(
        4,
        vec![
            construct(0, vec![]),
            construct(1, vec![]),
            construct(2, vec![]),
            ArcInstr::ApplyIndirect {
                dst: v(3),
                ty: ty(0),
                closure: v(2),
                args: vec![v(0), v(1)],
                arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
            },
        ],
        ArcTerminator::Return { value: v(3) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());
    assert!(
        !classification.indirect_arg_handoff,
        "annotated indirect args are a modeled hand-off"
    );
    let owned_arg = rep(&mut partition, 0);
    let borrowed_arg = rep(&mut partition, 1);
    let stream = &classification.blocks[0];
    assert!(
        stream.iter().any(|instr| matches!(instr,
            ClassInstr::Consume { class } if *class == owned_arg)),
        "the Owned indirect arg transfers (CONSUME)"
    );
    assert!(
        stream.iter().any(|instr| matches!(instr,
            ClassInstr::Read { class, .. } if *class == borrowed_arg)),
        "the Borrowed indirect arg is a floor read"
    );
}

/// An indirect call whose only class member is the CLOSURE RECEIVER (pos 0,
/// always borrowed per the ABI) does NOT poison — receiver-only indirect
/// calls (the lazy-iter lambda invocation shape) stay replaceable.
#[test]
fn indirect_receiver_only_does_not_set_handoff_flag() {
    let func = one_block_func(
        2,
        vec![
            construct(0, vec![]),
            ArcInstr::ApplyIndirect {
                dst: v(1),
                ty: ty(0),
                closure: v(0),
                args: vec![],
                arg_ownership: vec![],
            },
        ],
        ArcTerminator::Return { value: v(1) },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, _) = classify(&func, &state_map, &no_facts());
    assert!(!classification.indirect_arg_handoff);
}

/// An OWNED param whose own contract cardinality is `Absent` books NO
/// events: every call site passes it BORROWED (the `contract_to_params`
/// convention maps `Cardinality::Absent` to `Ownership::Borrowed`), so the
/// caller retains and releases and the callee owes nothing (the
/// borrowed-boundary discipline). A param with live-path demand keeps its
/// birth.
#[test]
fn absent_owned_param_books_no_events() {
    let mut func = one_block_func(1, vec![], ArcTerminator::Return { value: v(0) });
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Owned,
    }];
    let mut facts = no_facts();
    facts.insert(
        func.name,
        BoundaryFacts {
            param_iter_consumes: vec![false],
            param_borrowed_cow_consumed: vec![false],
            param_transfers_through_return: vec![false],
            param_cardinality_absent: vec![true],
            returns_sharing_view: false,
            returns_owned_fresh: false,
        },
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &facts);
    let param_rep = {
        use crate::aims::intraprocedural::birth_site_partition::FieldPath;
        let node = partition.register_node(v(0), FieldPath::whole_var());
        partition.rep_of(node)
    };
    let absent_param_evented = classification
        .blocks
        .iter()
        .flatten()
        .any(|instr| matches!(instr, ClassInstr::Birth { class, .. } if *class == param_rep));
    assert!(
        !absent_param_evented,
        "an Absent owned param must book no birth — the caller retains the \
         release obligation per the borrowed call-site convention"
    );

    // The same param with live-path demand keeps its birth.
    let mut live_facts = no_facts();
    live_facts.insert(
        func.name,
        BoundaryFacts {
            param_iter_consumes: vec![false],
            param_borrowed_cow_consumed: vec![false],
            param_transfers_through_return: vec![false],
            param_cardinality_absent: vec![false],
            returns_sharing_view: false,
            returns_owned_fresh: false,
        },
    );
    let (classification, mut partition) = classify(&func, &state_map, &live_facts);
    let param_rep = {
        use crate::aims::intraprocedural::birth_site_partition::FieldPath;
        let node = partition.register_node(v(0), FieldPath::whole_var());
        partition.rep_of(node)
    };
    let live_param_evented = classification
        .blocks
        .iter()
        .flatten()
        .any(|instr| matches!(instr, ClassInstr::Birth { class, .. } if *class == param_rep));
    assert!(live_param_evented, "a live owned param keeps its birth");
}

/// A borrowed-rooted list operand still CONSUMES at the semantic primitive
/// boundary. The borrowed class origin makes AIMS fund that consume with a
/// placed increment; no physical executor owns a hidden protection rule.
#[test]
fn borrowed_rooted_concat_operand_is_explicit_consume() {
    let mut func = one_block_func(
        4,
        vec![
            ArcInstr::Let {
                dst: v(2),
                ty: ty(0),
                value: ArcValue::Var(v(0)),
            },
            construct(1, vec![]),
            ArcInstr::Let {
                dst: v(3),
                ty: ty(0),
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![v(2), v(1)],
                },
            },
        ],
        ArcTerminator::Return { value: v(3) },
    );
    func.params = vec![ArcParam {
        var: v(0),
        ty: ty(0),
        ownership: Ownership::Borrowed,
    }];
    func.replace_variable_representations(vec![
        crate::ir::ValueRepr::RcPointer,
        crate::ir::ValueRepr::RcPointer,
        crate::ir::ValueRepr::RcPointer,
        crate::ir::ValueRepr::RcPointer,
    ]);
    freeze_primitive(
        &mut func,
        3,
        ori_registry::OpStrategy::RuntimeCall(ori_registry::RuntimeOperator::ListConcat),
    );
    let state_map = AimsStateMap::new(&func);
    let (classification, mut partition) = classify(&func, &state_map, &no_facts());

    // The borrowed-rooted operand (%2, alias of borrowed param %0): CONSUME.
    let borrowed = rep(&mut partition, 0);
    assert_eq!(
        derive_ledger(borrowed, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    // The fresh local operand (%1): CONSUMED (its reference hands in).
    let fresh = rep(&mut partition, 1);
    assert_eq!(
        derive_ledger(fresh, &flat(&classification)),
        vec![LedgerEvent::Birth, LedgerEvent::Consume]
    );
    assert!(!classification.indirect_arg_handoff);
}

/// The classifier's all-excluded verdict covers the placeholder
/// alias-closure: a `Let Var` alias of an immortal literal is excluded
/// (the state map marks only the literal's def var), so an
/// immortal-empty-string function admits the empty plan instead of
/// declining zero-classes.
#[test]
fn all_vars_excluded_covers_immortal_alias_closure() {
    let func = one_block_func(
        2,
        vec![
            ArcInstr::Let {
                dst: v(0),
                ty: ty(0),
                value: ArcValue::Literal(crate::ir::LitValue::String(Name::from_raw(3))),
            },
            ArcInstr::Let {
                dst: v(1),
                ty: ty(0),
                value: ArcValue::Var(v(0)),
            },
        ],
        ArcTerminator::Return { value: v(1) },
    );
    let mut state_map = AimsStateMap::new(&func);
    state_map.set_immortals(vec![true, false]);
    let (classification, _partition) = classify(&func, &state_map, &no_facts());
    assert!(classification.all_vars_excluded);
    assert!(classification.blocks.iter().flatten().next().is_none());

    // A live (non-immortal) literal keeps the verdict false via its class.
    let state_map = AimsStateMap::new(&func);
    let (classification, _partition) = classify(&func, &state_map, &no_facts());
    assert!(!classification.all_vars_excluded);
}
