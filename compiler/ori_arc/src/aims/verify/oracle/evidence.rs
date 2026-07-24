//! Subject-independent realized evidence for the VF-3 coherence oracle.

use ori_ir::{Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::interprocedural::{
    build_subject_independent_alias_to_param_map, find_borrowed_cow_consumed_params,
    find_iter_consume_call_args, CowConsumeScope,
};
use crate::aims::lattice::AccessClass;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};

use super::{
    demand::derive_param_consumptions,
    local_funding::{locally_funded, reachable_blocks},
    RealizedParamContract,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum FundingEvent {
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

    let mut observer = EvidenceObserver {
        subject: func,
        aliases: &aliases,
        iter_calls: &iter_calls,
        contracts,
        evidence: &mut evidence,
    };
    for (block_index, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            observer.observe_instruction(block_index, instr, reachable[block_index]);
        }
        observer.observe_terminator(block_index, &block.terminator, reachable[block_index]);
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

struct EvidenceObserver<'a> {
    subject: &'a ArcFunction,
    aliases: &'a FxHashMap<ArcVarId, FxHashSet<usize>>,
    iter_calls: &'a FxHashMap<ArcVarId, FxHashSet<usize>>,
    contracts: &'a FxHashMap<Name, MemoryContract>,
    evidence: &'a mut [ParamEvidence],
}

#[derive(Clone, Copy)]
struct DirectCallEvidence<'a> {
    dst: ArcVarId,
    callee: Name,
    args: &'a [ArcVarId],
    ownerships: &'a [ArgOwnership],
}

impl EvidenceObserver<'_> {
    fn observe_instruction(&mut self, block: usize, instr: &ArcInstr, reachable: bool) {
        match instr {
            ArcInstr::Apply {
                dst,
                func: callee,
                args,
                arg_ownership,
                ..
            } => self.observe_direct_call(
                block,
                DirectCallEvidence {
                    dst: *dst,
                    callee: *callee,
                    args,
                    ownerships: arg_ownership,
                },
                reachable,
            ),
            ArcInstr::ApplyIndirect {
                args,
                arg_ownership,
                ..
            } => self.observe_indirect_call(block, args, arg_ownership),
            ArcInstr::PartialApply { args, .. }
            | ArcInstr::Construct { args, .. }
            | ArcInstr::Reuse { args, .. } => {
                add_transfer_events(args, block, self.aliases, self.evidence);
            }
            ArcInstr::RcInc { var, count, .. } => {
                add_credit(*var, block, *count, self.aliases, reachable, self.evidence);
            }
            ArcInstr::BurdenInc { var } => {
                add_credit(*var, block, 1, self.aliases, reachable, self.evidence);
            }
            ArcInstr::RcDec { var, .. }
            | ArcInstr::RcDecPartial { var, .. }
            | ArcInstr::RcDecVariant { var }
            | ArcInstr::BurdenDec { var }
            | ArcInstr::BurdenDecPartial { var, .. }
            | ArcInstr::BurdenDecVariant { var } => {
                add_event(
                    *var,
                    block,
                    FundingEvent::Consume,
                    self.aliases,
                    self.evidence,
                );
            }
            ArcInstr::RcDecField { base, .. } | ArcInstr::BurdenDecField { base, .. } => {
                add_event(
                    *base,
                    block,
                    FundingEvent::Consume,
                    self.aliases,
                    self.evidence,
                );
            }
            ArcInstr::Set { value, .. } => {
                add_event(
                    *value,
                    block,
                    FundingEvent::Consume,
                    self.aliases,
                    self.evidence,
                );
            }
            ArcInstr::CollectionReuse { old_var, args, .. } => {
                add_event(
                    *old_var,
                    block,
                    FundingEvent::Consume,
                    self.aliases,
                    self.evidence,
                );
                add_transfer_events(args, block, self.aliases, self.evidence);
            }
            ArcInstr::Let { .. }
            | ArcInstr::Project { .. }
            | ArcInstr::IsShared { .. }
            | ArcInstr::Reset { .. }
            | ArcInstr::SetTag { .. }
            | ArcInstr::Select { .. } => {}
        }
    }

    fn observe_terminator(&mut self, block: usize, term: &ArcTerminator, reachable: bool) {
        match term {
            ArcTerminator::Return { value } => {
                add_event(
                    *value,
                    block,
                    FundingEvent::Consume,
                    self.aliases,
                    self.evidence,
                );
            }
            ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                arg_ownership,
                ..
            } => self.observe_direct_call(
                block,
                DirectCallEvidence {
                    dst: *dst,
                    callee: *callee,
                    args,
                    ownerships: arg_ownership,
                },
                reachable,
            ),
            ArcTerminator::InvokeIndirect {
                args,
                arg_ownership,
                ..
            } => self.observe_indirect_call(block, args, arg_ownership),
            ArcTerminator::Jump { .. }
            | ArcTerminator::Branch { .. }
            | ArcTerminator::Switch { .. }
            | ArcTerminator::Resume
            | ArcTerminator::Unreachable => {}
        }
    }

    fn observe_direct_call(&mut self, block: usize, call: DirectCallEvidence<'_>, reachable: bool) {
        let iter_positions = self.iter_calls.get(&call.dst);
        let callee_contract = (call.callee != self.subject.name)
            .then(|| self.contracts.get(&call.callee))
            .flatten();

        for (position, &arg) in call.args.iter().enumerate() {
            if iter_positions.is_some_and(|positions| positions.contains(&position)) {
                add_event(
                    arg,
                    block,
                    FundingEvent::IterTransfer,
                    self.aliases,
                    self.evidence,
                );
                if reachable {
                    set_iter_transfer(arg, self.aliases, self.evidence);
                }
            } else if call
                .ownerships
                .get(position)
                .is_none_or(|ownership| *ownership == ArgOwnership::Owned)
            {
                add_event(
                    arg,
                    block,
                    FundingEvent::Consume,
                    self.aliases,
                    self.evidence,
                );
            }

            let explicitly_borrowed =
                call.ownerships.get(position) == Some(&ArgOwnership::Borrowed);
            let callee_may_share = callee_contract.is_some_and(|contract| {
                contract
                    .params
                    .get(position)
                    .is_some_and(|param| param.may_share)
            });
            if reachable && explicitly_borrowed && callee_may_share {
                set_may_share(arg, self.aliases, self.evidence);
            }
        }
    }

    fn observe_indirect_call(
        &mut self,
        block: usize,
        args: &[ArcVarId],
        ownerships: &[ArgOwnership],
    ) {
        for (position, &arg) in args.iter().enumerate() {
            if ownerships
                .get(position)
                .is_some_and(|ownership| *ownership == ArgOwnership::Owned)
            {
                add_event(
                    arg,
                    block,
                    FundingEvent::Consume,
                    self.aliases,
                    self.evidence,
                );
            }
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
