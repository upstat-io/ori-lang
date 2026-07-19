//! ARC IR function builder.
//!
//! [`ArcIrBuilder`] owns block and variable state while a function is being
//! lowered. Follows the same "position at a block, emit instructions,
//! terminate" pattern as LLVM's `IRBuilder`, but uses block parameters
//! instead of phi nodes for SSA merge.

mod emission;
#[cfg(test)]
mod tests;

use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, Span};
use ori_types::Idx;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    LitValue, MethodCallFact, MethodCallForm,
};

/// Routing metadata for `Invoke`-family terminators: CFG successors plus the
/// abstract dispatch index. Bundled so `terminate_invoke` stays under the
/// `clippy::too_many_arguments` threshold. All fields are `Copy`, so the struct itself
/// is `Copy` — passing by value is zero-cost and satisfies clippy.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct InvokeTargets {
    pub normal: ArcBlockId,
    pub unwind: ArcBlockId,
    pub mono_instance_id: Option<MonoInstanceId>,
}

/// In-progress basic block being constructed.
pub(super) struct BlockBuilder {
    id: ArcBlockId,
    params: Vec<(ArcVarId, Idx)>,
    pub(super) body: Vec<ArcInstr>,
    pub(super) spans: Vec<Option<Span>>,
    pub(super) terminator: Option<ArcTerminator>,
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

/// Builder for an in-progress ARC IR function.
///
/// Owns block and variable state while the function is being lowered.
/// Consumed by [`finish`](ArcIrBuilder::finish) to produce the final
/// [`ArcFunction`].
///
/// # Design
///
/// Follows the same "position at a block, emit instructions, terminate"
/// pattern as LLVM's `IRBuilder`. The key difference is that ARC IR uses
/// block parameters instead of phi nodes for SSA merge.
pub(crate) struct ArcIrBuilder {
    pub(super) blocks: Vec<BlockBuilder>,
    pub(super) current_block: ArcBlockId,
    next_var: u32,
    pub(super) var_types: Vec<Idx>,
    /// When set, `emit_invoke` creates unwind blocks that `Jump` to this
    /// target instead of `Resume`. Used by `catch(expr:)` lowering to
    /// redirect panics to a shared catch handler block.
    pub(super) catch_unwind_target: Option<ArcBlockId>,
    /// Mutable-`Ident` reassignment death points `(old_var, new_var)` recorded
    /// by `lower_assign`. Threaded into `ArcFunction::reassign_deaths` by
    /// [`finish`](Self::finish) for the burden Phase-5 reassign-release scan.
    pub(super) reassign_deaths: Vec<(ArcVarId, ArcVarId)>,
    /// `(checked-op result var, catch handler block)` pairs for inline
    /// checked-op `PrimOp`s lowered inside a `catch(expr:)` body. Threaded into
    /// `ArcFunction::catch_scoped_checked_ops` by [`finish`](Self::finish); see
    /// that field for the full emission-side contract. `note_checked_op` appends
    /// when a `catch_unwind_target` is active (pairing with that target).
    pub(super) catch_scoped_checked_ops: Vec<(ArcVarId, ArcBlockId)>,
    /// Exact owner/form facts for direct method calls, keyed by result register.
    pub(super) method_call_facts: Vec<MethodCallFact>,
    /// User-defined operator calls awaiting exact pre-AIMS target closure.
    pub(super) operator_call_facts: Vec<crate::ir::OperatorCallFact>,
}

impl Default for ArcIrBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ArcIrBuilder {
    /// Create a builder with an entry block already allocated.
    pub fn new() -> Self {
        let entry = BlockBuilder::new(ArcBlockId::new(0));
        Self {
            blocks: vec![entry],
            current_block: ArcBlockId::new(0),
            next_var: 0,
            var_types: Vec::new(),
            catch_unwind_target: None,
            reassign_deaths: Vec::new(),
            catch_scoped_checked_ops: Vec::new(),
            method_call_facts: Vec::new(),
            operator_call_facts: Vec::new(),
        }
    }

    // Block management

    /// Allocate a new empty block and return its ID.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "block indices never exceed u32"
    )]
    pub fn new_block(&mut self) -> ArcBlockId {
        let id = ArcBlockId::new(self.blocks.len() as u32);
        self.blocks.push(BlockBuilder::new(id));
        id
    }

    /// Set the current insertion point to the given block.
    pub fn position_at(&mut self, block: ArcBlockId) {
        debug_assert!(
            (block.index()) < self.blocks.len(),
            "ArcBlockId {} out of bounds (have {} blocks)",
            block.raw(),
            self.blocks.len(),
        );
        self.current_block = block;
    }

    /// Get the current block being built.
    #[inline]
    pub fn current_block(&self) -> ArcBlockId {
        self.current_block
    }

    /// Check whether the current block already has a terminator.
    #[inline]
    pub fn is_terminated(&self) -> bool {
        self.blocks[self.current_block.index()].terminator.is_some()
    }

    /// Get the entry block (always block 0).
    #[inline]
    #[expect(
        clippy::unused_self,
        reason = "method API: callers use builder.entry_block()"
    )]
    pub fn entry_block(&self) -> ArcBlockId {
        ArcBlockId::new(0)
    }

    // Variable allocation

    /// Allocate a fresh variable with the given type.
    pub fn fresh_var(&mut self, ty: Idx) -> ArcVarId {
        let id = ArcVarId::new(self.next_var);
        self.next_var += 1;
        self.var_types.push(ty);
        id
    }

    /// Add a block parameter and return the variable bound to it.
    pub fn add_block_param(&mut self, block: ArcBlockId, ty: Idx) -> ArcVarId {
        let var = self.fresh_var(ty);
        self.blocks[block.index()].params.push((var, ty));
        var
    }

    /// Get the type of a variable.
    pub fn var_type(&self, var: ArcVarId) -> Idx {
        self.var_types[var.index()]
    }

    /// Get the type of a variable, returning `Idx::UNIT` if out of bounds.
    ///
    /// Used when looking up mutable variable types from scope, where the
    /// variable may have been created in a context the builder hasn't
    /// registered yet.
    pub fn var_type_or_unit(&self, var: ArcVarId) -> Idx {
        if var.index() < self.var_types.len() {
            self.var_types[var.index()]
        } else {
            Idx::UNIT
        }
    }

    /// Preserve the source-selected owner and form of one emitted direct call.
    pub fn note_method_call(
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
    pub fn note_selected_method_call(
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
    pub fn note_operator_call(
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

    // Literal queries

    /// Look up whether `var` resolves to a literal integer constant.
    ///
    /// Traces through SSA definitions to find the ultimate literal value:
    /// - Direct: `Let { dst: var, value: Literal(Int(n)) }` → `Some(n)`
    /// - Through projection: `Project { dst: var, value: src, field: f }`
    ///   → `Construct { dst: src, args }` → `get_literal_int(args[f])`
    ///
    /// Used by range specialization to detect compile-time-constant step
    /// and inclusive flags, enabling single-instruction bounds checks at -O0.
    pub fn get_literal_int(&self, var: ArcVarId) -> Option<i64> {
        for block in &self.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Literal(LitValue::Int(n)),
                        ..
                    } if *dst == var => return Some(*n),

                    ArcInstr::Project {
                        dst,
                        value: src,
                        field,
                        ..
                    } if *dst == var => {
                        return self.get_construct_arg(*src, *field);
                    }

                    _ => {}
                }
            }
        }
        None
    }

    /// Trace a `Construct` instruction to get the literal int of one of its args.
    fn get_construct_arg(&self, construct_var: ArcVarId, field: u32) -> Option<i64> {
        for block in &self.blocks {
            for instr in &block.body {
                if let ArcInstr::Construct { dst, args, .. } = instr {
                    if *dst == construct_var {
                        let arg = *args.get(field as usize)?;
                        return self.get_literal_int(arg);
                    }
                }
            }
        }
        None
    }

    /// Query whether a field of a constructed aggregate is a literal int,
    /// without emitting a `Project` instruction.
    ///
    /// Traces `base_var → Construct { args }` → `args[field]` → literal check.
    /// Used to detect compile-time constants (e.g., range step/inclusive flags)
    /// before deciding whether to extract the field.
    pub fn get_field_literal_int(&self, base_var: ArcVarId, field: u32) -> Option<i64> {
        self.get_construct_arg(base_var, field)
    }

    // Terminators

    /// Terminate with `Return`.
    pub fn terminate_return(&mut self, value: ArcVarId) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Return { value });
    }

    /// Terminate with unconditional `Jump`.
    pub fn terminate_jump(&mut self, target: ArcBlockId, args: Vec<ArcVarId>) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Jump { target, args });
    }

    /// Terminate with conditional `Branch`.
    pub fn terminate_branch(
        &mut self,
        cond: ArcVarId,
        then_block: ArcBlockId,
        else_block: ArcBlockId,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Branch {
            cond,
            then_block,
            else_block,
        });
    }

    /// Terminate with multi-way `Switch`.
    pub fn terminate_switch(
        &mut self,
        scrutinee: ArcVarId,
        cases: Vec<(u64, ArcBlockId)>,
        default: ArcBlockId,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Switch {
            scrutinee,
            cases,
            default,
        });
    }

    /// Terminate with `Invoke` (function call that may unwind).
    ///
    /// The `dst` variable is defined at the `normal` continuation block's
    /// entry, NOT in the current block. The `unwind` block receives control
    /// if the callee unwinds (panics).
    pub fn terminate_invoke(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        targets: InvokeTargets,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        let arg_count = args.len();
        block.terminator = Some(ArcTerminator::Invoke {
            dst,
            ty,
            func,
            args,
            arg_ownership: vec![crate::ir::ArgOwnership::Owned; arg_count],
            mono_instance_id: targets.mono_instance_id,
            normal: targets.normal,
            unwind: targets.unwind,
        });
    }

    /// Terminate with `InvokeIndirect` (indirect call through closure that may unwind).
    pub fn terminate_invoke_indirect(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        closure: ArcVarId,
        args: Vec<ArcVarId>,
        normal: ArcBlockId,
        unwind: ArcBlockId,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::InvokeIndirect {
            dst,
            ty,
            closure,
            args,
            arg_ownership: Vec::new(),
            normal,
            unwind,
        });
    }

    /// Terminate with `Resume` (re-raise an unwinding panic).
    pub fn terminate_resume(&mut self) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Resume);
    }

    /// Terminate with `Unreachable`.
    pub fn terminate_unreachable(&mut self) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Unreachable);
    }

    // Finalization

    /// Consume the builder and produce a finished [`ArcFunction`].
    ///
    /// Validates that every block has a terminator. Unterminated blocks
    /// get `Unreachable` as a fallback (with a tracing warning).
    pub fn finish(
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
            if bb.terminator.is_none() {
                tracing::warn!(
                    block = bb.id.raw(),
                    "unterminated block in ARC IR — adding Unreachable"
                );
                bb.terminator = Some(ArcTerminator::Unreachable);
            }

            let terminator = bb.terminator.take().unwrap_or(ArcTerminator::Unreachable);
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
            class_ledger_emission: false,
        }
    }
}
