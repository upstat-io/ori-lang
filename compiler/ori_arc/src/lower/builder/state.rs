//! ARC IR builder state, allocation, and finalization.
//!
//! [`ArcIrBuilder`] owns block and variable state while a function is being
//! lowered. Follows the same "position at a block, emit instructions,
//! terminate" pattern as LLVM's `IRBuilder`, but uses block parameters
//! instead of phi nodes for SSA merge.

use crate::ir::{
    AllocationSiteId, ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator,
    ArcVarId, MethodCallFact, MethodCallForm, YieldAllocationExecution, YieldAllocationFact,
    YieldAllocationLocality, YieldExtent,
};
use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, Span};
use ori_types::Idx;

/// Groups CFG successors with the abstract dispatch index for an invoke.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct InvokeTargets {
    pub(crate) normal: ArcBlockId,
    pub(crate) unwind: ArcBlockId,
    pub(crate) mono_instance_id: Option<MonoInstanceId>,
}

/// In-progress basic block being constructed.
pub(in crate::lower) struct BlockBuilder {
    id: ArcBlockId,
    params: Vec<(ArcVarId, Idx)>,
    pub(in crate::lower) body: Vec<ArcInstr>,
    pub(in crate::lower) spans: Vec<Option<Span>>,
    pub(in crate::lower) terminator: Option<ArcTerminator>,
}

impl BlockBuilder {
    fn new(id: ArcBlockId) -> Self {
        Self {
            id,
            params: Vec::new(),
            body: Vec::new(),
            spans: Vec::new(),
            terminator: None,
        }
    }
}

/// Identifies one instruction within the builder's block table.
#[derive(Clone, Copy)]
pub(super) struct InstructionLocation {
    /// Owning block.
    pub(super) block: ArcBlockId,
    /// Zero-based position within the block body.
    pub(super) instruction: u32,
}

impl InstructionLocation {
    const UNDEFINED: Self = Self {
        block: ArcBlockId::INVALID,
        instruction: u32::MAX,
    };
}

/// Owns block and variable state while the function is being lowered.
/// Consumed by [`finish`](ArcIrBuilder::finish) to produce the final
/// [`ArcFunction`].
pub(crate) struct ArcIrBuilder {
    pub(in crate::lower) blocks: Vec<BlockBuilder>,
    pub(in crate::lower) current_block: ArcBlockId,
    pub(super) next_var: u32,
    pub(in crate::lower) var_types: Vec<Idx>,
    pub(super) definitions: Vec<InstructionLocation>,
    /// When set, `emit_invoke` creates unwind blocks that `Jump` to this
    /// target instead of `Resume`. Used by `catch(expr:)` lowering to
    /// redirect panics to a shared catch handler block.
    pub(in crate::lower) catch_unwind_target: Option<ArcBlockId>,
    /// Mutable-identifier reassignment pairs requiring old-binding release.
    pub(in crate::lower) reassign_deaths: Vec<(ArcVarId, ArcVarId)>,
    /// Checked-operation results paired with their active catch handler.
    pub(in crate::lower) catch_scoped_checked_ops: Vec<(ArcVarId, ArcBlockId)>,
    /// Exact owner/form facts for direct method calls, keyed by result register.
    pub(in crate::lower) method_call_facts: Vec<MethodCallFact>,
    /// User-defined operator calls awaiting exact pre-AIMS target closure.
    pub(in crate::lower) operator_call_facts: Vec<crate::ir::OperatorCallFact>,
    /// Stable yield-comprehension allocations in lowering order.
    pub(in crate::lower) yield_allocations: Vec<YieldAllocationFact>,
}

impl Default for ArcIrBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ArcIrBuilder {
    /// Create a builder with an entry block already allocated.
    pub(crate) fn new() -> Self {
        let entry = BlockBuilder::new(ArcBlockId::new(0));
        Self {
            blocks: vec![entry],
            current_block: ArcBlockId::new(0),
            next_var: 0,
            var_types: Vec::new(),
            definitions: Vec::new(),
            catch_unwind_target: None,
            reassign_deaths: Vec::new(),
            catch_scoped_checked_ops: Vec::new(),
            method_call_facts: Vec::new(),
            operator_call_facts: Vec::new(),
            yield_allocations: Vec::new(),
        }
    }

    // Block management.

    /// Allocate a new empty block and return its ID.
    pub(crate) fn new_block(&mut self) -> ArcBlockId {
        // Why: Allocating enough blocks to exhaust `ArcBlockId` is not
        // representable in memory.
        let Ok(raw) = u32::try_from(self.blocks.len()) else {
            unreachable!("ARC block table exceeded ArcBlockId capacity");
        };
        let id = ArcBlockId::new(raw);
        self.blocks.push(BlockBuilder::new(id));
        id
    }

    /// Set the current insertion point to the given block.
    pub(crate) fn position_at(&mut self, block: ArcBlockId) {
        assert!(
            (block.index()) < self.blocks.len(),
            "ArcBlockId {} out of bounds (have {} blocks)",
            block.raw(),
            self.blocks.len(),
        );
        self.current_block = block;
    }

    /// Get the current block being built.
    #[inline]
    pub(crate) fn current_block(&self) -> ArcBlockId {
        self.current_block
    }

    /// Check whether the current block already has a terminator.
    #[inline]
    pub(crate) fn is_terminated(&self) -> bool {
        self.blocks[self.current_block.index()].terminator.is_some()
    }

    /// Get the entry block (always block 0).
    #[inline]
    #[expect(
        clippy::unused_self,
        reason = "method syntax keeps the builder-independent entry ID with block queries"
    )]
    pub(crate) fn entry_block(&self) -> ArcBlockId {
        ArcBlockId::new(0)
    }

    // Variable allocation.

    /// Allocate a fresh variable with the given type.
    pub(crate) fn fresh_var(&mut self, ty: Idx) -> ArcVarId {
        let id = ArcVarId::new(self.next_var);
        // Why: Allocating enough variables to exhaust `ArcVarId` is not
        // representable in memory.
        let Some(next_var) = self.next_var.checked_add(1) else {
            unreachable!("ARC variable table exceeded ArcVarId capacity");
        };
        self.next_var = next_var;
        self.var_types.push(ty);
        self.definitions.push(InstructionLocation::UNDEFINED);
        id
    }

    /// Add a block parameter and return the variable bound to it.
    pub(crate) fn add_block_param(&mut self, block: ArcBlockId, ty: Idx) -> ArcVarId {
        let var = self.fresh_var(ty);
        self.blocks[block.index()].params.push((var, ty));
        var
    }

    /// Get the type of a variable.
    pub(crate) fn var_type(&self, var: ArcVarId) -> Idx {
        assert!(
            var.index() < self.var_types.len(),
            "ARC variable {} must have a registered type before use",
            var.raw()
        );
        self.var_types[var.index()]
    }

    /// Preserve the source-selected owner and form of one emitted direct call.
    pub(crate) fn note_method_call(
        &mut self,
        destination: ArcVarId,
        receiver_type: Idx,
        form: MethodCallForm,
    ) {
        assert!(
            self.method_call_facts
                .iter()
                .all(|fact| fact.destination != destination),
            "a direct call result may carry only one method provenance fact"
        );
        self.method_call_facts.push(MethodCallFact {
            destination,
            receiver_type,
            form,
            producer: None,
            selected_producer: None,
            derived_position: None,
        });
    }

    /// Preserve a type-checker-selected source method until realization can
    /// resolve its module-local producer handle against `TypedModule`.
    pub(crate) fn note_selected_method_call(
        &mut self,
        destination: ArcVarId,
        receiver_type: Idx,
        form: MethodCallForm,
        producer: ori_ir::canon::MethodProducerId,
    ) {
        assert!(
            self.method_call_facts
                .iter()
                .all(|fact| fact.destination != destination),
            "a direct call result may carry only one method provenance fact"
        );
        self.method_call_facts.push(MethodCallFact {
            destination,
            receiver_type,
            form,
            producer: None,
            selected_producer: Some(producer),
            derived_position: None,
        });
    }

    /// Preserve one source operator's receiver and operation until realization
    /// closes its exact callable identity.
    pub(crate) fn note_operator_call(
        &mut self,
        destination: ArcVarId,
        receiver: ArcVarId,
        operation: crate::ir::PrimOp,
        span: Option<ori_ir::Span>,
    ) {
        assert!(
            self.operator_call_facts
                .iter()
                .all(|fact| fact.destination != destination),
            "an operator call result may carry only one resolution fact"
        );
        self.operator_call_facts.push(crate::ir::OperatorCallFact {
            destination,
            receiver,
            operation,
            span,
        });
    }

    /// Record one completed yield accumulator allocation.
    pub(crate) fn note_yield_allocation(
        &mut self,
        builder: ArcVarId,
        result: ArcVarId,
        elem_ty: Idx,
        elem_size: u64,
        extent: YieldExtent,
    ) {
        let Ok(raw_site) = u32::try_from(self.yield_allocations.len()) else {
            unreachable!("yield allocation table exceeded AllocationSiteId capacity");
        };
        self.yield_allocations.push(YieldAllocationFact {
            site: AllocationSiteId::new(raw_site),
            builder,
            result,
            elem_ty,
            elem_size,
            extent,
            locality: YieldAllocationLocality::Unknown,
            execution: YieldAllocationExecution::RepeatedOrUnknown,
        });
    }

    // Finalization.

    /// Consume the builder and produce a finished [`ArcFunction`].
    ///
    /// Validates that every block has a terminator.
    pub(crate) fn finish(
        mut self,
        name: Name,
        params: Vec<ArcParam>,
        return_type: Idx,
        entry: ArcBlockId,
        is_fbip: bool,
    ) -> ArcFunction {
        let mut blocks = Vec::with_capacity(self.blocks.len());
        let mut spans = Vec::with_capacity(self.blocks.len());

        for bb in &mut self.blocks {
            assert!(
                bb.terminator.is_some(),
                "ARC block {} must be terminated before finish",
                bb.id.raw()
            );
            // Why: The always-on assertion establishes the terminator's presence.
            let Some(terminator) = bb.terminator.take() else {
                unreachable!("validated ARC terminator disappeared before finalization");
            };
            let body = std::mem::take(&mut bb.body);
            let block_spans = std::mem::take(&mut bb.spans);
            let block_params = std::mem::take(&mut bb.params);

            blocks.push(ArcBlock {
                id: bb.id,
                params: block_params,
                body,
                terminator,
            });
            spans.push(block_spans);
        }

        ArcFunction {
            name,
            params,
            return_type,
            blocks,
            entry,
            var_types: self.var_types,
            var_reprs: Vec::new(),
            var_rc_strategies: Vec::new(),
            var_metadata_state: crate::ir::VariableMetadataState::Unrealized,
            spans,
            is_fbip,
            num_captures: 0,
            cow_annotations: crate::uniqueness::CowAnnotations::default(),
            primitive_facts: crate::ir::PrimitiveFacts::default(),
            drop_hints: crate::uniqueness::DropHints::default(),
            tail_calls: Vec::new(),
            burden_emitted: Vec::new(),
            reassign_deaths: self.reassign_deaths,
            catch_scoped_checked_ops: self.catch_scoped_checked_ops,
            method_call_facts: self.method_call_facts,
            operator_call_facts: self.operator_call_facts,
            direct_call_facts: Vec::new(),
            yield_allocations: self.yield_allocations,
            class_ledger_emission: false,
        }
    }
}
