//! Instruction emission methods for [`ArcIrBuilder`].
//!
//! Contains all `emit_*` methods that push instructions to the current
//! block: `Let`, `Apply`, `ApplyIndirect`, `Construct`, `PartialApply`,
//! `Project`, `Select`, and `Invoke` (which also creates unwind blocks).

use ori_ir::{Name, Span};
use ori_types::Idx;

use crate::ir::{ArcBlockId, ArcInstr, ArcValue, ArcVarId, ArgOwnership, CtorKind};

use super::ArcIrBuilder;

impl ArcIrBuilder {
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

    /// Emit an `InvokeIndirect` terminator for an indirect closure call that may unwind.
    ///
    /// Same pattern as [`emit_invoke`] but calls through a closure fat pointer.
    /// Used when an indirect call is made inside `catch(expr:)`.
    pub fn emit_invoke_indirect(
        &mut self,
        ty: Idx,
        closure: ArcVarId,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let normal = self.new_block();
        let unwind = self.new_block();

        let _ = span;

        self.terminate_invoke_indirect(dst, ty, closure, args, normal, unwind);

        // Unwind block: same as emit_invoke — Jump to catch or Resume
        self.position_at(unwind);
        if let Some(catch_target) = self.catch_unwind_target {
            self.terminate_jump(catch_target, vec![]);
        } else {
            self.terminate_resume();
        }

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
}
