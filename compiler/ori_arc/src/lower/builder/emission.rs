//! Instruction emission methods for [`ArcIrBuilder`].
//!
//! Contains all `emit_*` methods that push instructions to the current
//! block: `Let`, `Apply`, `ApplyIndirect`, `Construct`, `PartialApply`,
//! `Project`, `Select`, and `Invoke` (which also creates unwind blocks).

use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, Span};
use ori_types::Idx;

use crate::ir::{ArcBlockId, ArcInstr, ArcValue, ArcVarId, ArgOwnership, CtorKind};

use super::state::InstructionLocation;
use super::{ArcIrBuilder, InvokeTargets};

impl ArcIrBuilder {
    // Instruction emission

    /// Push an instruction into the current block, allocating a fresh `dst`
    /// variable of type `ty` and recording `span`. `make` builds the
    /// instruction from the fresh `dst`. Returns the fresh variable.
    fn push_instr(
        &mut self,
        ty: Idx,
        span: Option<Span>,
        make: impl FnOnce(ArcVarId) -> ArcInstr,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block_id = self.current_block;
        let block = &mut self.blocks[self.current_block.index()];
        let Ok(instruction) = u32::try_from(block.body.len()) else {
            unreachable!("ARC block instruction table exceeded u32 capacity");
        };
        block.body.push(make(dst));
        block.spans.push(span);
        self.definitions[dst.index()] = InstructionLocation {
            block: block_id,
            instruction,
        };
        dst
    }

    /// Emit a `Let` instruction binding a value to a fresh variable.
    pub(crate) fn emit_let(&mut self, ty: Idx, value: ArcValue, span: Option<Span>) -> ArcVarId {
        self.push_instr(ty, span, |dst| ArcInstr::Let { dst, ty, value })
    }

    /// Emit an `Apply` (direct function call) instruction.
    ///
    /// `mono_instance_id` is the abstract dispatch index for generic-instantiated
    /// calls (sourced from `CanonResult.mono_dispatch_map_can` during ARC
    /// lowering); pass `None` for non-generic calls and builtin emissions.
    pub(crate) fn emit_apply(
        &mut self,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        span: Option<Span>,
        mono_instance_id: Option<MonoInstanceId>,
    ) -> ArcVarId {
        self.push_instr(ty, span, |dst| {
            let arg_count = args.len();
            ArcInstr::Apply {
                dst,
                ty,
                func,
                args,
                arg_ownership: vec![ArgOwnership::Owned; arg_count],
                mono_instance_id,
            }
        })
    }

    /// Emit an `ApplyIndirect` (closure call) instruction.
    pub(crate) fn emit_apply_indirect(
        &mut self,
        ty: Idx,
        closure: ArcVarId,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        self.push_instr(ty, span, |dst| ArcInstr::ApplyIndirect {
            dst,
            ty,
            closure,
            args,
            arg_ownership: Vec::new(),
        })
    }

    /// Emit a `Construct` instruction.
    pub(crate) fn emit_construct(
        &mut self,
        ty: Idx,
        ctor: CtorKind,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        self.push_instr(ty, span, |dst| ArcInstr::Construct {
            dst,
            ty,
            ctor,
            args,
        })
    }

    /// Emit a `PartialApply` instruction (closure creation with captures).
    pub(crate) fn emit_partial_apply(
        &mut self,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        self.push_instr(ty, span, |dst| ArcInstr::PartialApply {
            dst,
            ty,
            func,
            args,
        })
    }

    /// Emit a `Project` (field access) instruction.
    pub(crate) fn emit_project(
        &mut self,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        span: Option<Span>,
    ) -> ArcVarId {
        self.push_instr(ty, span, |dst| ArcInstr::Project {
            dst,
            ty,
            value,
            field,
        })
    }

    /// Emit a `Select` (branchless conditional value) instruction.
    ///
    /// Maps to LLVM's `select` instruction: returns `true_val` if `cond`
    /// is true, `false_val` otherwise. Used to eliminate basic blocks for
    /// trivial match arms.
    pub(crate) fn emit_select(
        &mut self,
        ty: Idx,
        cond: ArcVarId,
        true_val: ArcVarId,
        false_val: ArcVarId,
        span: Option<Span>,
    ) -> ArcVarId {
        self.push_instr(ty, span, |dst| ArcInstr::Select {
            dst,
            ty,
            cond,
            true_val,
            false_val,
        })
    }

    // Invoke (call that may unwind)

    /// Emit the shared control-flow shape for a call that may unwind.
    fn emit_may_unwind(
        &mut self,
        ty: Idx,
        span: Option<Span>,
        terminate: impl FnOnce(&mut Self, ArcVarId, ArcBlockId, ArcBlockId),
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let normal = self.new_block();
        let unwind = self.new_block();

        // Invoke is a terminator, so it has no entry in the instruction-span
        // sidecar.
        let _ = span;
        terminate(self, dst, normal, unwind);

        // Cleanup is inserted before this terminator by the RC insertion pass.
        self.position_at(unwind);
        if let Some(catch_target) = self.catch_unwind_target {
            self.terminate_jump(catch_target, vec![]);
        } else {
            self.terminate_resume();
        }

        self.position_at(normal);
        dst
    }

    /// Emit an `Invoke` terminator for a function call that may unwind.
    ///
    /// Creates a normal continuation block and an unwind cleanup block.
    /// The current block is terminated with `Invoke`. The builder is
    /// positioned at the normal block on return. The unwind block is
    /// terminated with `Resume`; the RC insertion pass fills its cleanup.
    ///
    /// Returns the `dst` variable holding the call result (defined at
    /// the normal block's entry).
    pub(crate) fn emit_invoke(
        &mut self,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        span: Option<Span>,
        mono_instance_id: Option<MonoInstanceId>,
    ) -> ArcVarId {
        self.emit_may_unwind(ty, span, |builder, dst, normal, unwind| {
            builder.terminate_invoke(
                dst,
                ty,
                func,
                args,
                InvokeTargets {
                    normal,
                    unwind,
                    mono_instance_id,
                },
            );
        })
    }

    /// Emit an `InvokeIndirect` terminator for an indirect closure call that may unwind.
    ///
    /// Same pattern as [`emit_invoke`] but calls through a closure fat pointer.
    /// Used when an indirect call is made inside `catch(expr:)`.
    pub(crate) fn emit_invoke_indirect(
        &mut self,
        ty: Idx,
        closure: ArcVarId,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        self.emit_may_unwind(ty, span, |builder, dst, normal, unwind| {
            builder.terminate_invoke_indirect(dst, ty, closure, args, normal, unwind);
        })
    }

    /// Set the catch unwind target for `catch(expr:)` lowering.
    ///
    /// When set, [`emit_invoke`](Self::emit_invoke) creates unwind blocks
    /// that `Jump` to this target instead of `Resume`. Returns the previous
    /// target (for nesting).
    pub(crate) fn set_catch_target(&mut self, target: ArcBlockId) -> Option<ArcBlockId> {
        self.catch_unwind_target.replace(target)
    }

    /// Clear the catch unwind target. Returns the previous target.
    pub(crate) fn clear_catch_target(&mut self) -> Option<ArcBlockId> {
        self.catch_unwind_target.take()
    }

    /// Record that a may-panic inline checked-op `PrimOp` with result `dst` was
    /// lowered. When a catch target is active (i.e. lowering lexically inside a
    /// `catch(expr:)` body), maps `dst` to that catch's initial handler block.
    /// AIMS unwind cleanup may retarget this metadata through a cleanup-only
    /// landing block before physical projection. The active
    /// `catch_unwind_target` is always the innermost enclosing catch. No-op
    /// outside a catch. Spec: Clause 14.3.
    pub(crate) fn note_checked_op(&mut self, dst: ArcVarId) {
        if let Some(handler) = self.catch_unwind_target {
            // `dst` is a fresh SSA var (defined exactly once), so no dedup is
            // needed — each checked-op result appears at most once.
            self.catch_scoped_checked_ops.push((dst, handler));
        }
    }
}
