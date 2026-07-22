//! Closed-program yield allocation projections.

use ori_arc::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, YieldAllocationExecution,
    YieldAllocationFact, YieldAllocationLocality, YieldExtent,
};
use ori_arc::ArcClassification;
use ori_ir::Name;
use ori_registry::TypeTag;
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
    CompiledAllocationDecision, CompiledAllocationMechanism, ReprPlan, YieldAllocationIdentity,
};

/// Runtime-use evidence accepted by compact yield-lineage storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum YieldLineageRuntimeCall {
    /// A receiver read that neither consumes nor exposes the runtime header.
    BorrowedRead,
    /// A statically unique update whose result remains in the same lineage.
    StaticUniqueListSet,
}

#[derive(Clone, Copy)]
struct YieldLineageCall<'a> {
    args: &'a [ArcVarId],
    arg_ownership: &'a [ArgOwnership],
    dst: ArcVarId,
    position: (usize, usize),
}

impl ReprPlan {
    /// Freeze AIMS yield facts into conservative compiled allocation choices.
    pub fn freeze_yield_allocations(
        &mut self,
        facts_by_function: &FxHashMap<Name, Vec<YieldAllocationFact>>,
    ) {
        self.yield_allocations.clear();
        self.yield_allocation_sites.clear();
        for (&function, facts) in facts_by_function {
            for fact in facts {
                self.yield_allocation_sites.insert(
                    (function, YieldAllocationIdentity::Builder(fact.builder)),
                    fact.site,
                );
                self.yield_allocation_sites.insert(
                    (function, YieldAllocationIdentity::Result(fact.result)),
                    fact.site,
                );
                self.yield_allocations.insert(
                    (function, fact.site),
                    CompiledAllocationDecision {
                        site: fact.site,
                        builder: fact.builder,
                        result: fact.result,
                        elem_ty: fact.elem_ty,
                        elem_size: fact.elem_size,
                        extent: fact.extent,
                        mechanism: select_yield_mechanism(*fact),
                        requires_runtime_header: true,
                    },
                );
            }
        }
    }

    /// Close physical header requirements over every realized lineage use.
    pub(crate) fn close_yield_runtime_header_requirements(
        &mut self,
        functions: &[ArcFunction],
        pool: &Pool,
        mut runtime_call: impl FnMut(Name, ArcVarId) -> Option<YieldLineageRuntimeCall>,
    ) {
        let functions_by_name: FxHashMap<_, _> = functions
            .iter()
            .map(|function| {
                let yield_lineages = ori_arc::YieldLineageIndex::for_function(function);
                let lineage_uses = index_yield_lineage_uses(function, &yield_lineages);
                (function.name, (function, yield_lineages, lineage_uses))
            })
            .collect();

        let decisions: Vec<_> = self
            .yield_allocations
            .iter()
            .map(|(&key, &decision)| (key, decision))
            .collect();
        for (key, decision) in decisions {
            let elidable = functions_by_name.get(&key.0).is_some_and(
                |(function, yield_lineages, lineage_uses)| {
                    yield_runtime_header_is_elidable(
                        function,
                        yield_lineages,
                        lineage_uses,
                        decision,
                        pool,
                        &mut runtime_call,
                    )
                },
            );
            if elidable {
                self.yield_allocations
                    .get_mut(&key)
                    .unwrap_or_else(|| unreachable!("compiled allocation disappeared"))
                    .requires_runtime_header = false;
            }
        }
    }

    /// Query the allocation projection by the scratch builder identity.
    #[must_use]
    pub fn yield_allocation_for_builder(
        &self,
        function: Name,
        builder: ArcVarId,
    ) -> Option<CompiledAllocationDecision> {
        let site = self
            .yield_allocation_sites
            .get(&(function, YieldAllocationIdentity::Builder(builder)))?;
        self.yield_allocations.get(&(function, *site)).copied()
    }

    /// Query the allocation projection by the final list result identity.
    #[must_use]
    pub fn yield_allocation_for_result(
        &self,
        function: Name,
        result: ArcVarId,
    ) -> Option<CompiledAllocationDecision> {
        let site = self
            .yield_allocation_sites
            .get(&(function, YieldAllocationIdentity::Result(result)))?;
        self.yield_allocations.get(&(function, *site)).copied()
    }

    /// Freeze the closed program's physical length-only clone decisions.
    pub(crate) fn set_length_projections(
        &mut self,
        calls: FxHashMap<(Name, ArcVarId), Name>,
        yields: FxHashMap<Name, ArcVarId>,
    ) {
        self.length_projection_calls = calls;
        self.length_projection_yields = yields;
    }

    /// Iterate qualified call-site redirects selected by the compiled plan.
    pub fn length_projection_calls(&self) -> impl Iterator<Item = ((Name, ArcVarId), Name)> + '_ {
        self.length_projection_calls
            .iter()
            .map(|(&site, &callee)| (site, callee))
    }

    /// Iterate qualified callees and their virtualized yield result.
    pub fn length_projection_yields(&self) -> impl Iterator<Item = (Name, ArcVarId)> + '_ {
        self.length_projection_yields
            .iter()
            .map(|(&callee, &result)| (callee, result))
    }
}

fn yield_runtime_header_is_elidable(
    function: &ArcFunction,
    yield_lineages: &ori_arc::YieldLineageIndex,
    lineage_uses: &FxHashMap<ArcVarId, Vec<(usize, usize)>>,
    decision: CompiledAllocationDecision,
    pool: &Pool,
    runtime_call: &mut impl FnMut(Name, ArcVarId) -> Option<YieldLineageRuntimeCall>,
) -> bool {
    let classifier = ori_arc::ArcClassifier::new(pool);
    if decision.mechanism != CompiledAllocationMechanism::StackSlot
        || !classifier.is_scalar(decision.elem_ty)
        || !matches!(
            classifier.builtin_type_tag(decision.elem_ty),
            Some(
                TypeTag::Int
                    | TypeTag::Float
                    | TypeTag::Bool
                    | TypeTag::Char
                    | TypeTag::Byte
                    | TypeTag::Unit
                    | TypeTag::Never
                    | TypeTag::Duration
                    | TypeTag::Size
                    | TypeTag::Ordering
            )
        )
    {
        return false;
    }

    let in_lineage = |var| {
        yield_lineages
            .result_for_receiver(var)
            .is_some_and(|result| result == decision.result)
    };
    for &(block_idx, instruction_index) in lineage_uses
        .get(&decision.result)
        .map_or(&[][..], Vec::as_slice)
    {
        let block = &function.blocks[block_idx];
        let allowed = if let Some(instruction) = block.body.get(instruction_index) {
            match instruction {
                ArcInstr::Let {
                    value: ori_arc::ir::ArcValue::Var(source),
                    ..
                } => in_lineage(*source),
                ArcInstr::Project {
                    value, field: 0, ..
                } => in_lineage(*value),
                ArcInstr::RcDec { var, .. } => in_lineage(*var),
                ArcInstr::Apply {
                    dst,
                    args,
                    arg_ownership,
                    ..
                } => yield_lineage_call_is_header_independent(
                    function,
                    YieldLineageCall {
                        args,
                        arg_ownership,
                        dst: *dst,
                        position: (block_idx, instruction_index),
                    },
                    runtime_call(function.name, *dst),
                    &in_lineage,
                ),
                _ => false,
            }
        } else {
            match &block.terminator {
                ArcTerminator::Jump { .. } => true,
                ArcTerminator::Invoke {
                    dst,
                    args,
                    arg_ownership,
                    ..
                } => yield_lineage_call_is_header_independent(
                    function,
                    YieldLineageCall {
                        args,
                        arg_ownership,
                        dst: *dst,
                        position: (block_idx, block.body.len()),
                    },
                    runtime_call(function.name, *dst),
                    &in_lineage,
                ),
                _ => false,
            }
        };
        if !allowed {
            return false;
        }
    }
    true
}

fn index_yield_lineage_uses(
    function: &ArcFunction,
    yield_lineages: &ori_arc::YieldLineageIndex,
) -> FxHashMap<ArcVarId, Vec<(usize, usize)>> {
    let mut uses = FxHashMap::default();
    let mut instruction_results = FxHashSet::default();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (instruction_index, instruction) in block.body.iter().enumerate() {
            instruction_results.clear();
            for result in instruction
                .used_vars()
                .iter()
                .filter_map(|&variable| yield_lineages.result_for_receiver(variable))
            {
                if instruction_results.insert(result) {
                    uses.entry(result)
                        .or_insert_with(Vec::new)
                        .push((block_index, instruction_index));
                }
            }
        }

        instruction_results.clear();
        for result in block
            .terminator
            .used_vars()
            .iter()
            .filter_map(|&variable| yield_lineages.result_for_receiver(variable))
        {
            if instruction_results.insert(result) {
                uses.entry(result)
                    .or_insert_with(Vec::new)
                    .push((block_index, block.body.len()));
            }
        }
    }
    uses
}

fn yield_lineage_call_is_header_independent(
    function: &ArcFunction,
    call: YieldLineageCall<'_>,
    operation: Option<YieldLineageRuntimeCall>,
    in_lineage: &impl Fn(ArcVarId) -> bool,
) -> bool {
    if !call.args.first().copied().is_some_and(in_lineage)
        || call.args.iter().skip(1).copied().any(in_lineage)
    {
        return false;
    }
    match operation {
        Some(YieldLineageRuntimeCall::BorrowedRead) => {
            call.arg_ownership.first() == Some(&ArgOwnership::Borrowed)
        }
        Some(YieldLineageRuntimeCall::StaticUniqueListSet) => {
            in_lineage(call.dst)
                && function
                    .cow_annotations
                    .get(call.position.0, call.position.1)
                    == ori_arc::CowMode::StaticUnique
        }
        None => false,
    }
}

fn select_yield_mechanism(fact: YieldAllocationFact) -> CompiledAllocationMechanism {
    let YieldExtent::StaticExact(capacity) = fact.extent else {
        return CompiledAllocationMechanism::RuntimeHeap;
    };
    let Some(bytes) = capacity.checked_mul(fact.elem_size.max(1)) else {
        return CompiledAllocationMechanism::RuntimeHeap;
    };
    match (fact.locality, fact.execution) {
        (YieldAllocationLocality::Local, YieldAllocationExecution::SingleExecution) => {
            if bytes <= CompiledAllocationDecision::MAX_LOCAL_BYTES {
                CompiledAllocationMechanism::StackSlot
            } else {
                CompiledAllocationMechanism::RuntimeHeap
            }
        }
        (
            YieldAllocationLocality::Unknown | YieldAllocationLocality::Escaping,
            YieldAllocationExecution::RepeatedOrUnknown | YieldAllocationExecution::SingleExecution,
        )
        | (YieldAllocationLocality::Local, YieldAllocationExecution::RepeatedOrUnknown) => {
            CompiledAllocationMechanism::RuntimeHeap
        }
    }
}
