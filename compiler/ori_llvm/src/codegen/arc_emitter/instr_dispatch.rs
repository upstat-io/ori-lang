//! Instruction dispatch for the ARC IR emitter.
//!
//! Contains [`ArcIrEmitter::emit_instr`] which dispatches each `ArcInstr`
//! variant to the appropriate emission handler, and [`ArcIrEmitter::emit_project`]
//! for field extraction from structs and enums.

mod mutation;
mod projection;

use super::context::EmittedValue;
use super::ArcIrEmitter;
use ori_arc::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, CtorKind};
use ori_types::Idx;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a single `ArcInstr` as LLVM IR.
    pub(super) fn emit_instr(&mut self, instr: &ArcInstr, func: &ArcFunction) {
        tracing::trace!(?instr, "emit_instr");
        match instr {
            ArcInstr::Let { dst, ty, value } => self.emit_let_instr(*dst, *ty, value, func),

            ArcInstr::Apply {
                dst,
                ty: _,
                func: callee,
                args,
                arg_ownership: _,
                mono_instance_id,
            } => self.emit_apply(*dst, *callee, args, func, *mono_instance_id),

            ArcInstr::ApplyIndirect {
                dst,
                ty,
                closure,
                args,
                arg_ownership: _,
            } => self.emit_apply_indirect(*dst, *ty, *closure, args, func),

            ArcInstr::PartialApply {
                dst,
                ty,
                func: callee,
                args,
            } => self.emit_partial_apply(*dst, *ty, *callee, args, func),

            ArcInstr::Project {
                dst,
                ty,
                value,
                field,
            } => self.emit_project(*dst, *ty, *value, *field, func),

            ArcInstr::Construct {
                dst,
                ty,
                ctor,
                args,
            } => self.emit_construct_instr(*dst, *ty, ctor, args, func),

            ArcInstr::CollectionReuse {
                old_var,
                dst,
                ty,
                ctor,
                args,
            } => self.emit_collection_reuse_instr(*old_var, *dst, *ty, ctor, args, func),

            ArcInstr::RcInc {
                var,
                count,
                strategy,
                atomicity: _,
            } => {
                // Why: Production ARC emission and the compiled runtime expose only atomic RC.
                self.emit_rc_inc(*var, *count, *strategy, func);
            }

            ArcInstr::RcDec {
                var,
                strategy,
                atomicity: _,
            } => {
                self.emit_rc_dec(*var, *strategy, func);
            }

            // INVARIANT: Unrealized burden markers have no compiled runtime effect.
            ArcInstr::BurdenInc { var: _ } | ArcInstr::BurdenDec { var: _ } => {}

            // INVARIANT: Burden and realized RC spellings share partial-drop glue.
            ArcInstr::BurdenDecPartial { var, skip_fields }
            | ArcInstr::RcDecPartial { var, skip_fields } => {
                self.emit_burden_dec_partial(*var, skip_fields, func);
            }

            ArcInstr::BurdenDecVariant { var } | ArcInstr::RcDecVariant { var } => {
                self.emit_burden_dec_variant(*var, func);
            }

            ArcInstr::BurdenDecField { base, field } | ArcInstr::RcDecField { base, field } => {
                self.emit_burden_dec_field(*base, *field, func);
            }

            ArcInstr::IsShared { dst, var } => self.emit_is_shared(*dst, *var, func),

            ArcInstr::Reset { var, token } => {
                // INVARIANT: A reset token aliases its source until reuse expansion.
                let emitted = self.var_emitted(*var);
                self.def_var(*token, emitted);
            }

            ArcInstr::Reuse {
                token,
                dst,
                ty,
                ctor,
                args,
            } => self.emit_reuse_fallback(*token, *dst, *ty, ctor, args, func),

            ArcInstr::Set { base, field, value } => {
                self.emit_set_field(*base, *field, *value, func);
            }

            ArcInstr::SetTag { base, tag } => self.emit_set_tag(*base, *tag, func),

            ArcInstr::Select {
                dst,
                cond,
                true_val,
                false_val,
                ..
            } => self.emit_select(*dst, *cond, *true_val, *false_val, func),
        }
    }

    fn emit_construct_instr(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        ctor: &CtorKind,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        let value = self.emit_construct(ty, ctor, args);
        self.def_var_repr(dst, value, func);
    }

    fn emit_collection_reuse_instr(
        &mut self,
        old_var: ArcVarId,
        dst: ArcVarId,
        ty: Idx,
        ctor: &CtorKind,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        let value = self.emit_collection_reuse(old_var, ty, ctor, args);
        self.def_var_repr(dst, value, func);
    }

    fn emit_select(
        &mut self,
        dst: ArcVarId,
        condition: ArcVarId,
        true_value: ArcVarId,
        false_value: ArcVarId,
        func: &ArcFunction,
    ) {
        let condition = self.var(condition);
        let true_value = self.var(true_value);
        let false_value = self.var(false_value);
        let result = self
            .builder
            .select(condition, true_value, false_value, "sel");
        self.def_var_repr(dst, result, func);
    }

    fn emit_let_instr(&mut self, dst: ArcVarId, ty: Idx, value: &ArcValue, func: &ArcFunction) {
        let emitted = if let Some(&(_, element_type)) = self.yield_types_by_elem_size_var.get(&dst)
        {
            let llvm_size = self.element_store_size(element_type);
            let Ok(llvm_size) = i64::try_from(llvm_size) else {
                self.builder.record_codegen_error_with_msg(format!(
                    "element storage size {llvm_size} exceeds the LLVM i64 ABI"
                ));
                return;
            };
            self.builder.const_i64(llvm_size)
        } else {
            let catch_pad = self.same_frame_catch_landing_pads.get(&dst).copied();
            if catch_pad.is_some() {
                self.builder.set_catch_unwind_target(catch_pad);
            }
            let emitted = self.emit_value(dst, value, ty, func);
            if catch_pad.is_some() {
                self.builder.set_catch_unwind_target(None);
            }
            emitted
        };

        // Why: Narrowing copies creates distinct SSA extensions that inhibit CSE.
        if matches!(value, ArcValue::PrimOp { .. }) {
            self.def_var_repr(dst, emitted, func);
        } else {
            let repr = super::emitter_utils::required_var_repr(dst, func);
            self.def_var(dst, EmittedValue::from_repr(repr, emitted));
        }

        if let ArcValue::Var(source) = value {
            if let Some(&pointer) = self.borrowed_param_ptrs.get(source) {
                self.borrowed_param_ptrs.insert(dst, pointer);
                if self.pointer_only_params.contains(source) {
                    self.pointer_only_params.insert(dst);
                }
            }
        }
    }
}
