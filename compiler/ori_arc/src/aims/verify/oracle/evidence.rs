//! Subject-independent realized evidence for the VF-3 coherence oracle.

use ori_ir::{Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::interprocedural::{
    build_subject_independent_alias_to_param_map, find_borrowed_cow_consumed_params,
    find_iter_consume_call_args, CowConsumeScope,
};
use crate::aims::lattice::AccessClass;
use crate::graph::successor_block_ids;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};

use super::{demand::derive_param_consumptions, RealizedParamContract};

#[derive(Clone, Copy, Debug)]
enum FundingEvent {
    Credit(u32),
    Consume,
    IterTransfer,
}

#[derive(Clone, Debug)]
struct ParamEvidence {
    block_events: Vec<Vec<FundingEvent>>,
    iter_transfers: bool,
    may_share: bool,
}

impl ParamEvidence {
    fn new(block_count: usize) -> Self {
        Self {
            block_events: vec![Vec::new(); block_count],
            iter_transfers: false,
            may_share: false,
        }
    }
}

pub(super) fn derive_param_contracts(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &StringInterner,
) -> Vec<RealizedParamContract> {
    let param_vars = func
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| (param.var, index))
        .collect();
    let aliases = build_subject_independent_alias_to_param_map(func, &param_vars, contracts);
    let incoming_borrowed_cow_credits = find_borrowed_cow_consumed_params(
        func,
        contracts,
        &aliases,
        interner,
        CowConsumeScope::AnyConsume,
    );
    let iter_calls = find_iter_consume_call_args(func, contracts, interner, Some(func.name));
    let reachable = reachable_blocks(func);
    let consumptions = derive_param_consumptions(func);
    let mut evidence = (0..func.params.len())
        .map(|_| ParamEvidence::new(func.blocks.len()))
        .collect::<Vec<_>>();

    for (block_index, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            observe_instruction(
                func,
                block_index,
                instr,
                &aliases,
                &iter_calls,
                contracts,
                reachable[block_index],
                &mut evidence,
            );
        }
        observe_terminator(
            func,
            block_index,
            &block.terminator,
            &aliases,
            &iter_calls,
            contracts,
            reachable[block_index],
            &mut evidence,
        );
    }

    evidence
        .iter()
        .enumerate()
        .map(|(index, param)| {
            // The closure ABI stores captures in the leading parameter prefix;
            // that environment edge remains live while the target executes.
            let has_boundary_credit =
                index < func.num_captures || incoming_borrowed_cow_credits.contains(&index);
            RealizedParamContract {
                access: if locally_funded(func, &param.block_events, has_boundary_credit) {
                    AccessClass::Borrowed
                } else {
                    AccessClass::Owned
                },
                consumption: consumptions[index],
                may_share: param.may_share,
                iter_transfers: param.iter_transfers,
            }
        })
        .collect()
}

#[expect(
    clippy::too_many_arguments,
    reason = "the evidence adapter keeps the subject-independent inputs explicit"
)]
fn observe_instruction(
    func: &ArcFunction,
    block: usize,
    instr: &ArcInstr,
    aliases: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    iter_calls: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    contracts: &FxHashMap<Name, MemoryContract>,
    reachable: bool,
    evidence: &mut [ParamEvidence],
) {
    match instr {
        ArcInstr::Apply {
            dst,
            func: callee,
            args,
            arg_ownership,
            ..
        } => observe_direct_call(
            func,
            block,
            *dst,
            *callee,
            args,
            arg_ownership,
            aliases,
            iter_calls,
            contracts,
            reachable,
            evidence,
        ),
        ArcInstr::ApplyIndirect {
            args,
            arg_ownership,
            ..
        } => {
            for (position, &arg) in args.iter().enumerate() {
                if arg_ownership
                    .get(position)
                    .is_some_and(|ownership| *ownership == ArgOwnership::Owned)
                {
                    add_event(arg, block, FundingEvent::Consume, aliases, evidence);
                }
            }
        }
        ArcInstr::PartialApply { args, .. }
        | ArcInstr::Construct { args, .. }
        | ArcInstr::Reuse { args, .. } => {
            add_transfer_events(args, block, aliases, evidence);
        }
        ArcInstr::RcInc { var, count, .. } => {
            add_credit(*var, block, *count, aliases, reachable, evidence);
        }
        ArcInstr::BurdenInc { var } => {
            add_credit(*var, block, 1, aliases, reachable, evidence);
        }
        ArcInstr::RcDec { var, .. }
        | ArcInstr::RcDecPartial { var, .. }
        | ArcInstr::RcDecVariant { var }
        | ArcInstr::BurdenDec { var }
        | ArcInstr::BurdenDecPartial { var, .. }
        | ArcInstr::BurdenDecVariant { var } => {
            add_event(*var, block, FundingEvent::Consume, aliases, evidence);
        }
        ArcInstr::RcDecField { base, .. } | ArcInstr::BurdenDecField { base, .. } => {
            add_event(*base, block, FundingEvent::Consume, aliases, evidence);
        }
        ArcInstr::Set { value, .. } => {
            add_event(*value, block, FundingEvent::Consume, aliases, evidence);
        }
        ArcInstr::CollectionReuse { old_var, args, .. } => {
            add_event(*old_var, block, FundingEvent::Consume, aliases, evidence);
            add_transfer_events(args, block, aliases, evidence);
        }
        ArcInstr::Let { .. }
        | ArcInstr::Project { .. }
        | ArcInstr::IsShared { .. }
        | ArcInstr::Reset { .. }
        | ArcInstr::SetTag { .. }
        | ArcInstr::Select { .. } => {}
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the evidence adapter keeps the subject-independent inputs explicit"
)]
fn observe_terminator(
    func: &ArcFunction,
    block: usize,
    term: &ArcTerminator,
    aliases: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    iter_calls: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    contracts: &FxHashMap<Name, MemoryContract>,
    reachable: bool,
    evidence: &mut [ParamEvidence],
) {
    match term {
        ArcTerminator::Return { value } => {
            add_event(*value, block, FundingEvent::Consume, aliases, evidence);
        }
        ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            arg_ownership,
            ..
        } => observe_direct_call(
            func,
            block,
            *dst,
            *callee,
            args,
            arg_ownership,
            aliases,
            iter_calls,
            contracts,
            reachable,
            evidence,
        ),
        ArcTerminator::InvokeIndirect {
            args,
            arg_ownership,
            ..
        } => {
            for (position, &arg) in args.iter().enumerate() {
                if arg_ownership
                    .get(position)
                    .is_some_and(|ownership| *ownership == ArgOwnership::Owned)
                {
                    add_event(arg, block, FundingEvent::Consume, aliases, evidence);
                }
            }
        }
        ArcTerminator::Jump { .. }
        | ArcTerminator::Branch { .. }
        | ArcTerminator::Switch { .. }
        | ArcTerminator::Resume
        | ArcTerminator::Unreachable => {}
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "direct calls carry the independent ownership and callee-summary inputs"
)]
fn observe_direct_call(
    subject: &ArcFunction,
    block: usize,
    dst: ArcVarId,
    callee: Name,
    args: &[ArcVarId],
    ownerships: &[ArgOwnership],
    aliases: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    iter_calls: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    contracts: &FxHashMap<Name, MemoryContract>,
    reachable: bool,
    evidence: &mut [ParamEvidence],
) {
    let iter_positions = iter_calls.get(&dst);
    let callee_contract = (callee != subject.name)
        .then(|| contracts.get(&callee))
        .flatten();

    for (position, &arg) in args.iter().enumerate() {
        if iter_positions.is_some_and(|positions| positions.contains(&position)) {
            add_event(arg, block, FundingEvent::IterTransfer, aliases, evidence);
            if reachable {
                set_iter_transfer(arg, aliases, evidence);
            }
        } else if ownerships
            .get(position)
            .is_none_or(|ownership| *ownership == ArgOwnership::Owned)
        {
            add_event(arg, block, FundingEvent::Consume, aliases, evidence);
        }

        let explicitly_borrowed = ownerships.get(position) == Some(&ArgOwnership::Borrowed);
        let callee_may_share = callee_contract.is_some_and(|contract| {
            contract
                .params
                .get(position)
                .is_some_and(|param| param.may_share)
        });
        if reachable && explicitly_borrowed && callee_may_share {
            set_may_share(arg, aliases, evidence);
        }
    }
}

fn add_transfer_events(
    vars: &[ArcVarId],
    block: usize,
    aliases: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    evidence: &mut [ParamEvidence],
) {
    for &var in vars {
        add_event(var, block, FundingEvent::Consume, aliases, evidence);
    }
}

fn add_event(
    var: ArcVarId,
    block: usize,
    event: FundingEvent,
    aliases: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    evidence: &mut [ParamEvidence],
) {
    let Some(params) = aliases.get(&var) else {
        return;
    };
    for &param in params {
        evidence[param].block_events[block].push(event);
    }
}

fn add_credit(
    var: ArcVarId,
    block: usize,
    count: u32,
    aliases: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    reachable: bool,
    evidence: &mut [ParamEvidence],
) {
    add_event(var, block, FundingEvent::Credit(count), aliases, evidence);
    if reachable && count > 0 {
        set_may_share(var, aliases, evidence);
    }
}

fn set_iter_transfer(
    var: ArcVarId,
    aliases: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    evidence: &mut [ParamEvidence],
) {
    if let Some(params) = aliases.get(&var) {
        for &param in params {
            evidence[param].iter_transfers = true;
        }
    }
}

fn set_may_share(
    var: ArcVarId,
    aliases: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    evidence: &mut [ParamEvidence],
) {
    if let Some(params) = aliases.get(&var) {
        for &param in params {
            evidence[param].may_share = true;
        }
    }
}

fn reachable_blocks(func: &ArcFunction) -> Vec<bool> {
    let mut reachable = vec![false; func.blocks.len()];
    if func.blocks.is_empty() {
        return reachable;
    }
    let entry = func.entry.index();
    if entry >= func.blocks.len() {
        return reachable;
    }
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        if reachable[block] {
            continue;
        }
        reachable[block] = true;
        pending.extend(
            successor_block_ids(&func.blocks[block].terminator)
                .into_iter()
                .map(crate::ir::ArcBlockId::index)
                .filter(|&successor| successor < func.blocks.len()),
        );
    }
    reachable
}

fn locally_funded(
    func: &ArcFunction,
    events: &[Vec<FundingEvent>],
    incoming_whole_value_credit: bool,
) -> bool {
    if func.blocks.is_empty() {
        return true;
    }

    let mut incoming = vec![None; func.blocks.len()];
    let entry = func.entry.index();
    if entry >= func.blocks.len() {
        return true;
    }
    incoming[entry] = Some(i128::from(incoming_whole_value_credit));
    for round in 0..func.blocks.len() {
        let prior = incoming.clone();
        let mut changed = false;
        for (block, balance) in prior.into_iter().enumerate() {
            let Some(mut balance) = balance else {
                continue;
            };
            for event in &events[block] {
                match event {
                    FundingEvent::Credit(count) => balance += i128::from(*count),
                    FundingEvent::Consume => {
                        if balance == 0 {
                            return false;
                        }
                        balance -= 1;
                    }
                    FundingEvent::IterTransfer => {}
                }
            }
            for successor in successor_block_ids(&func.blocks[block].terminator) {
                let successor = successor.index();
                if successor >= func.blocks.len() {
                    continue;
                }
                let should_update = incoming[successor].is_none_or(|current| balance < current);
                if should_update {
                    incoming[successor] = Some(balance);
                    changed = true;
                }
            }
        }
        if !changed {
            return true;
        }
        if round + 1 == func.blocks.len() {
            // A balance can improve after |V| rounds only through a
            // reachable negative-credit cycle, which eventually underfunds.
            return false;
        }
    }
    true
}
