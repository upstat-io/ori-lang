//! Instruction dispatch for the ARC IR emitter.
//!
//! Contains [`ArcIrEmitter::emit_instr`] which dispatches each `ArcInstr`
//! variant to the appropriate emission handler, and [`ArcIrEmitter::emit_project`]
//! for field extraction from structs and enums.

mod projection;

use super::context::{is_boxed_enum_field, EmittedValue};
use super::ArcIrEmitter;
use ori_arc::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, CtorKind, RcStrategy, ValueRepr};
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

            // RC operations — dispatched by strategy (no Pool queries)
            ArcInstr::RcInc {
                var,
                count,
                strategy,
                atomicity: _,
            } => {
                // `atomicity` is `RcAtomicity::Atomic` at every construction
                // site today (the shipped runtime RC primitives are
                // unconditionally atomic). The atomicity-selecting branch +
                // non-atomic runtime path land with the thread-local-ARC
                // dispatch (RL-19/20/21); until then this arm ignores it.
                self.emit_rc_inc(*var, *count, *strategy, func);
            }

            ArcInstr::RcDec {
                var,
                strategy,
                atomicity: _,
            } => {
                self.emit_rc_dec(*var, *strategy, func);
            }

            // BurdenInc / BurdenDec — no-op codegen markers. Phase 5
            // trivial-burden lowering uses these for IR-level dataflow +
            // emission ordering; the LLVM backend treats them as zero-cost
            // annotations.
            ArcInstr::BurdenInc { var: _ } | ArcInstr::BurdenDec { var: _ } => {
                // No LLVM IR emitted.
            }

            // Burden spelling = legacy pre-lowering survivor
            // (`ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING=1`); Rc spelling = the
            // Phase-7 realized form. One canonical glue body for both.
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
                // Reset marks a value for potential reuse. After reuse expansion,
                // this becomes IsShared + conditional.
                // The token IS the variable (reuse its memory if unique).
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
        let emitted = if let Some(&(_, element_type)) = self.for_yield_elem_size_types.get(&dst) {
            let llvm_size = self.element_store_size(element_type);
            self.builder.const_i64(llvm_size as i64)
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

        // Narrow computations only. Narrowing copies creates distinct SSA
        // extensions that prevent equivalent expressions from sharing CSE keys.
        if matches!(value, ArcValue::PrimOp { .. }) {
            self.def_var_repr(dst, emitted, func);
        } else {
            let repr = func.var_repr(dst).unwrap_or(ValueRepr::Scalar);
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

    fn emit_burden_dec_partial(&mut self, var: ArcVarId, skip_fields: &[u32], func: &ArcFunction) {
        let base_type = func.var_type(var);
        let Some(drop_info) = ori_arc::compute_drop_info(base_type, self.classifier, self.pool)
        else {
            return;
        };
        let base_value = self.var_field_base_ptr(var, base_type);

        match drop_info.kind {
            ori_arc::DropKind::Fields { fields, .. } => self.emit_drop_field_loop(
                base_value,
                base_type,
                &fields,
                Some(skip_fields),
                "burden_dec_partial",
            ),
            ori_arc::DropKind::Enum { variants, .. } => {
                // A projected variant transfers its payload ownership; every
                // other variant retains its release obligation.
                let surviving: Vec<Vec<(u32, Idx)>> = variants
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, fields)| {
                        if skip_fields.contains(&(ordinal as u32)) {
                            Vec::new()
                        } else {
                            fields
                        }
                    })
                    .collect();
                self.emit_variant_burden_walk(
                    self.current_function,
                    base_value,
                    base_type,
                    &surviving,
                );
            }
            other => {
                debug_assert!(
                    false,
                    "BurdenDecPartial on unsupported drop shape: {other:?}"
                );
                self.builder.record_codegen_error_with_msg(format!(
                    "BurdenDecPartial on unsupported drop shape: {other:?}"
                ));
            }
        }
    }

    fn emit_burden_dec_variant(&mut self, var: ArcVarId, func: &ArcFunction) {
        let base_type = func.var_type(var);
        let Some(drop_info) = ori_arc::compute_drop_info(base_type, self.classifier, self.pool)
        else {
            return;
        };
        let ori_arc::DropKind::Enum { variants, .. } = drop_info.kind else {
            debug_assert!(
                false,
                "BurdenDecVariant on non-enum drop shape: {:?}",
                drop_info.kind
            );
            self.builder.record_codegen_error_with_msg(format!(
                "BurdenDecVariant on non-enum drop shape: {:?}",
                drop_info.kind
            ));
            return;
        };

        // SetTag invalidates the old payload, so release it before the tag store.
        let base_value = self.var_field_base_ptr(var, base_type);
        self.emit_variant_burden_walk(self.current_function, base_value, base_type, &variants);
    }

    fn emit_burden_dec_field(&mut self, base: ArcVarId, field: u32, func: &ArcFunction) {
        let repr = func.var_repr(base).unwrap_or(ValueRepr::Scalar);
        if repr == ValueRepr::Scalar {
            tracing::trace!(
                base = base.raw(),
                field,
                ?repr,
                "BurdenDecField on scalar base — skipping (unreachable)"
            );
            return;
        }

        let base_type = func.var_type(base);
        let fields = self.pool.struct_fields(base_type);
        let Some(&(_, field_type)) = fields.get(field as usize) else {
            tracing::trace!(
                base = base.raw(),
                field,
                "BurdenDecField index is outside struct fields; skipping"
            );
            return;
        };
        let base_value = self.var_field_base_ptr(base, base_type);
        self.emit_drop_field_loop(
            base_value,
            base_type,
            &[(field, field_type)],
            None,
            "burden_dec_field",
        );
    }

    fn emit_is_shared(&mut self, dst: ArcVarId, var: ArcVarId, func: &ArcFunction) {
        let repr = func.var_repr(var).unwrap_or(ValueRepr::Scalar);
        if repr != ValueRepr::RcPointer {
            tracing::trace!(
                var = var.raw(),
                ?repr,
                "IsShared on non-pointer value — emitting true"
            );
            let always_shared = self.builder.const_bool(true);
            self.def_var(dst, EmittedValue::Immediate(always_shared));
            return;
        }

        let data_pointer = self.var(var);
        let i8_type = self.builder.i8_type();
        let header_offset = self.builder.const_i64(-8);
        let rc_pointer = self
            .builder
            .gep(i8_type, data_pointer, &[header_offset], "rc_ptr");
        let i64_type = self.builder.i64_type();
        let ref_count = self.builder.load(i64_type, rc_pointer, "rc_val");
        let one = self.builder.const_i64(1);
        let is_shared = self.builder.icmp_sgt(ref_count, one, "is_shared");
        self.def_var(dst, EmittedValue::Immediate(is_shared));
    }

    fn emit_reuse_fallback(
        &mut self,
        token: ArcVarId,
        dst: ArcVarId,
        ty: Idx,
        ctor: &CtorKind,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        tracing::debug!("Reuse was not expanded; using Construct fallback");
        if let Some(repr) = func.var_repr(token) {
            let strategy = RcStrategy::from_repr(repr, self.pool, func.var_type(token));
            self.emit_rc_dec(token, strategy, func);
        }
        let value = self.emit_construct(ty, ctor, args);
        self.def_var_repr(dst, value, func);
    }

    fn emit_set_field(&mut self, base: ArcVarId, field: u32, value: ArcVarId, func: &ArcFunction) {
        let repr = func.var_repr(base).unwrap_or(ValueRepr::Scalar);
        if repr != ValueRepr::RcPointer {
            tracing::trace!(
                base = base.raw(),
                field,
                ?repr,
                "Set on non-pointer value — skipping (unreachable)"
            );
            return;
        }

        let base_value = self.var(base);
        let new_value = self.var(value);
        let base_type = func.var_type(base);
        let llvm_type = self.resolve_type(base_type);
        debug_assert!(
            self.get_tagged_ptr_encoding(base_type).is_none(),
            "compiled layout must resolve tagged-pointer Set before LLVM emission"
        );

        let memory_field = self.remap_struct_field(base_type, field);
        let field_type = self
            .pool
            .struct_fields(base_type)
            .get(field as usize)
            .map(|&(_, ty)| ty);
        let stored_value = match field_type {
            Some(field_type) if is_boxed_enum_field(self.pool, base_type, field_type) => {
                self.box_recursive_field(new_value, field_type, Some(value))
            }
            _ => new_value,
        };
        let field_pointer = self.builder.struct_gep(
            llvm_type,
            base_value,
            memory_field,
            &format!("set.{field}.ptr"),
        );
        self.builder.store(stored_value, field_pointer);
    }

    fn emit_set_tag(&mut self, base: ArcVarId, tag: u64, func: &ArcFunction) {
        let base_value = self.var(base);
        let base_type = func.var_type(base);
        let llvm_type = self.resolve_type(base_type);

        if self.get_tagged_ptr_encoding(base_type).is_some() {
            let pointer = self.tagged_ptr_decode_ptr(base_value, "set_tag.ptr");
            let updated = self.tagged_ptr_encode(pointer, tag as u32, "set_tag");
            self.def_var(base, EmittedValue::Immediate(updated));
            return;
        }
        if self.is_tagless_enum(base_type) {
            return;
        }

        if let Some(encoding) = self.get_niche_encoding(base_type) {
            if !encoding.needs_tag_store(tag as u32) {
                return;
            }
            let Some(niche_index) = encoding.niche_field_index() else {
                self.builder
                    .record_codegen_error_with_msg("niche encoding has no field index");
                return;
            };
            let niche_value = encoding.variant_to_tag_value(tag as u32);
            if self.builder.is_struct_value(base_value) {
                let value =
                    self.builder
                        .const_int_for_struct_field(llvm_type, niche_index, niche_value);

                let updated =
                    self.builder
                        .insert_value(base_value, value, niche_index, "set.niche");
                self.def_var(base, EmittedValue::Aggregate(updated));
            } else {
                let pointer =
                    self.builder
                        .struct_gep(llvm_type, base_value, niche_index, "set.niche.ptr");

                let value =
                    self.builder
                        .const_int_for_struct_field(llvm_type, niche_index, niche_value);
                self.builder.store(value, pointer);
            }
            return;
        }

        let tag_pointer = self
            .builder
            .struct_gep(llvm_type, base_value, 0, "set.tag.ptr");
        let tag_value = self.builder.const_int_for_struct_field(llvm_type, 0, tag);
        self.builder.store(tag_value, tag_pointer);
    }
}
