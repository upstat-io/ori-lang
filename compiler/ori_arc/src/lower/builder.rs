//! ARC IR function builder.
//!
//! [`ArcIrBuilder`] owns block and variable state while a function is being
//! lowered. Follows the same "position at a block, emit instructions,
//! terminate" pattern as LLVM's `IRBuilder`, but uses block parameters
//! instead of phi nodes for SSA merge.

use ori_ir::{Name, Span};
use ori_types::Idx;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind,
};

/// In-progress basic block being constructed.
pub(super) struct BlockBuilder {
    id: ArcBlockId,
    params: Vec<(ArcVarId, Idx)>,
    pub(super) body: Vec<ArcInstr>,
    spans: Vec<Option<Span>>,
    terminator: Option<ArcTerminator>,
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
pub struct ArcIrBuilder {
    pub(super) blocks: Vec<BlockBuilder>,
    current_block: ArcBlockId,
    next_var: u32,
    pub(super) var_types: Vec<Idx>,
    /// When set, `emit_invoke` creates unwind blocks that `Jump` to this
    /// target instead of `Resume`. Used by `catch(expr:)` lowering to
    /// redirect panics to a shared catch handler block.
    catch_unwind_target: Option<ArcBlockId>,
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

    // Instruction emission

    /// Emit a `Let` instruction binding a value to a fresh variable.
    pub fn emit_let(&mut self, ty: Idx, value: ArcValue, span: Option<Span>) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::Let { dst, ty, value });
        block.spans.push(span);
        dst
    }

    /// Emit an `Apply` (direct function call) instruction.
    pub fn emit_apply(
        &mut self,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        let arg_count = args.len();
        block.body.push(ArcInstr::Apply {
            dst,
            ty,
            func,
            args,
            arg_ownership: vec![ArgOwnership::Owned; arg_count],
        });
        block.spans.push(span);
        dst
    }

    /// Emit an `ApplyIndirect` (closure call) instruction.
    pub fn emit_apply_indirect(
        &mut self,
        ty: Idx,
        closure: ArcVarId,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::ApplyIndirect {
            dst,
            ty,
            closure,
            args,
        });
        block.spans.push(span);
        dst
    }

    /// Emit a `Construct` instruction.
    pub fn emit_construct(
        &mut self,
        ty: Idx,
        ctor: CtorKind,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::Construct {
            dst,
            ty,
            ctor,
            args,
        });
        block.spans.push(span);
        dst
    }

    /// Emit a `PartialApply` instruction (closure creation with captures).
    pub fn emit_partial_apply(
        &mut self,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::PartialApply {
            dst,
            ty,
            func,
            args,
        });
        block.spans.push(span);
        dst
    }

    /// Emit a `Project` (field access) instruction.
    pub fn emit_project(
        &mut self,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::Project {
            dst,
            ty,
            value,
            field,
        });
        block.spans.push(span);
        dst
    }

    /// Emit a `Select` (branchless conditional value) instruction.
    ///
    /// Maps to LLVM's `select` instruction: returns `true_val` if `cond`
    /// is true, `false_val` otherwise. Used to eliminate basic blocks for
    /// trivial match arms.
    pub fn emit_select(
        &mut self,
        ty: Idx,
        cond: ArcVarId,
        true_val: ArcVarId,
        false_val: ArcVarId,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::Select {
            dst,
            ty,
            cond,
            true_val,
            false_val,
        });
        block.spans.push(span);
        dst
    }

    // Invoke (call that may unwind)

    /// Emit an `Invoke` terminator for a function call that may unwind.
    ///
    /// Creates a normal continuation block and an unwind cleanup block.
    /// The current block is terminated with `Invoke`. The builder is
    /// positioned at the normal block on return. The unwind block is
    /// terminated with `Resume` (cleanup blocks will be filled in later
    /// by the RC insertion pass).
    ///
    /// Returns the `dst` variable holding the call result (defined at
    /// the normal block's entry).
    pub fn emit_invoke(
        &mut self,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let normal = self.new_block();
        let unwind = self.new_block();

        // Track the span on the invoking block (one span per instruction,
        // but Invoke is a terminator so we don't push to spans here —
        // terminators don't have span slots in the current design).
        let _ = span;

        self.terminate_invoke(dst, ty, func, args, normal, unwind);

        // Unwind block terminator depends on context:
        // - Normal code: Resume (re-raise after cleanup)
        // - Inside catch(expr:): Jump to catch handler (catch after cleanup)
        // The RC insertion pass (Phase 3C) adds cleanup RcDec instructions
        // before whichever terminator is present.
        self.position_at(unwind);
        if let Some(catch_target) = self.catch_unwind_target {
            self.terminate_jump(catch_target, vec![]);
        } else {
            self.terminate_resume();
        }

        // Position at the normal continuation block for subsequent lowering.
        self.position_at(normal);
        dst
    }

    /// Set the catch unwind target for `catch(expr:)` lowering.
    ///
    /// When set, [`emit_invoke`](Self::emit_invoke) creates unwind blocks
    /// that `Jump` to this target instead of `Resume`. Returns the previous
    /// target (for nesting).
    pub fn set_catch_target(&mut self, target: ArcBlockId) -> Option<ArcBlockId> {
        self.catch_unwind_target.replace(target)
    }

    /// Clear the catch unwind target. Returns the previous target.
    pub fn clear_catch_target(&mut self) -> Option<ArcBlockId> {
        self.catch_unwind_target.take()
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
        normal: ArcBlockId,
        unwind: ArcBlockId,
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
            arg_ownership: vec![ArgOwnership::Owned; arg_count],
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
            spans,
            is_fbip,
            num_captures: 0,
        }
    }
}
