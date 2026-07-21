//! Instruction dispatch for the ARC IR emitter.
//!
//! Contains [`ArcIrEmitter::emit_instr`] which dispatches each `ArcInstr`
//! variant to the appropriate emission handler, and [`ArcIrEmitter::emit_project`]
//! for field extraction from structs and enums.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, CtorKind, RcStrategy, ValueRepr};
use ori_types::{Idx, Tag};

use super::context::{is_boxed_enum_field, EmittedValue};
use super::drop_enum::{compute_variant_field_offsets, variant_field_offset};
use super::ArcIrEmitter;
use crate::codegen::value_id::{LLVMTypeId, ValueId};

/// Compute the byte offset for a given payload field in an enum variant.
///
/// Searches all variants of the enum to find one where the field at
/// `payload_field_idx` (0-based) has type matching `field_type`. Returns
/// the byte offset within the `[M x i64]` payload area.
///
/// Falls back to `payload_field_idx * 8` if no matching variant is found
/// (single-slot fields at sequential positions — the legacy behavior).
fn enum_payload_byte_offset(
    emitter: &ArcIrEmitter<'_, '_, '_, '_>,
    enum_ty: Idx,
    payload_field_idx: u32,
    field_type: Idx,
) -> u64 {
    let resolved = emitter.pool.resolve_fully(enum_ty);
    if emitter.pool.tag(resolved) != Tag::Enum {
        return u64::from(payload_field_idx) * 8;
    }
    let variants = emitter.pool.enum_variants(resolved);
    let fi = payload_field_idx as usize;
    let resolved_ft = emitter.pool.resolve_fully(field_type);

    for (_, fields) in &variants {
        if fi < fields.len() && emitter.pool.resolve_fully(fields[fi]) == resolved_ft {
            let offsets = compute_variant_field_offsets(fields, resolved, emitter);
            return variant_field_offset(&offsets, fi);
        }
    }
    // Fallback: assume single-slot fields
    u64::from(payload_field_idx) * 8
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Handle `Project` on a decomposed `__iter_next` result.
    ///
    /// Field 0 returns the tag (already an `i64` in the `var_map`).
    /// Field 1 loads the element from the scratch buffer and registers the
    /// scratch pointer in `borrowed_param_ptrs` for downstream forwarding.
    fn emit_project_iter_next(
        &mut self,
        dst: ArcVarId,
        field: u32,
        tag: ValueId,
        scratch_ptr: ValueId,
        elem_llvm_ty: LLVMTypeId,
        func: &ArcFunction,
    ) {
        if field == 0 {
            self.def_var_repr(dst, tag, func);
        } else {
            // Why: Tuple reordering excludes map iterator elements because the
            // runtime writes `(key, value)` in declaration order.
            let elem = self
                .builder
                .load(elem_llvm_ty, scratch_ptr, &format!("proj.{field}"));
            self.def_var_repr(dst, elem, func);
            // Register scratch pointer for borrowed-parameter forwarding:
            // downstream calls (e.g., ori_str_len) can forward the scratch
            // pointer directly instead of alloca+store round-trip.
            self.borrowed_param_ptrs.insert(dst, scratch_ptr);
        }
    }

    /// Try to emit a `Project` for an enum/Result payload field.
    ///
    /// Returns `true` if the field was an enum/Result payload and was handled,
    /// `false` if it should go through the normal `extractvalue` path.
    #[expect(
        clippy::too_many_arguments,
        reason = "payload projection carries source and destination representations"
    )]
    fn try_emit_project_enum_payload(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        val: ValueId,
        result_ty: LLVMTypeId,
        func: &ArcFunction,
    ) -> bool {
        let val_ty = func.var_type(value);
        let val_type_info = self.type_info.get(val_ty);
        if !matches!(
            val_type_info,
            super::super::type_info::TypeInfo::Result { .. }
                | super::super::type_info::TypeInfo::Enum { .. }
        ) {
            return false;
        }

        // If the projected field is Unit/Never, it's zero-sized
        // and doesn't exist in the LLVM payload. Return a zero constant.
        let resolved_field_ty = self.pool.resolve_fully(ty);
        if matches!(self.pool.tag(resolved_field_ty), Tag::Unit | Tag::Never) {
            let zero = self.builder.const_zero_ty(result_ty);
            self.def_var_repr(dst, zero, func);
            return true;
        }

        // Tagless single-variant enum — struct-like layout (no tag,
        // no `[M x i64]` payload). Project directly from the field slot.
        if self.is_tagless_enum(val_ty) {
            self.emit_project_tagless_field(dst, ty, val_ty, value, field, result_ty, func);
            return true;
        }

        let is_general_enum = matches!(
            val_type_info,
            super::super::type_info::TypeInfo::Enum { .. }
        );

        // Compute byte offset for this field within the payload.
        let byte_offset = if is_general_enum {
            enum_payload_byte_offset(self, val_ty, field - 1, ty)
        } else {
            0
        };

        // Fast path: extractvalue chain for general enum scalar fields.
        let slot_index = byte_offset / 8;
        if is_general_enum
            && !is_boxed_enum_field(self.pool, val_ty, ty)
            && self.builder.is_struct_value(val)
            && self.builder.is_single_slot_type(result_ty)
            && byte_offset % 8 == 0
        {
            let payload = self
                .builder
                .extract_value(val, 1, "proj.payload")
                .expect("enum value should be a struct");
            #[expect(clippy::cast_possible_truncation, reason = "slot index fits u32")]
            let raw = self.builder.extract_value_any(
                payload,
                slot_index as u32,
                &format!("proj.{field}.raw"),
            );
            let converted =
                self.builder
                    .reinterpret_from_i64(raw, result_ty, &format!("proj.{field}"));
            self.def_var_repr(dst, converted, func);
            return true;
        }

        // Slow path: alloca+store+GEP+load for Result types, boxed fields,
        // multi-word types, and pointer-sourced values.
        let llvm_val_ty = self.resolve_type(val_ty);
        let alloca = self.builder.alloca(llvm_val_ty, "proj.alloca");
        self.builder.store(val, alloca);
        if is_general_enum {
            let payload_ptr = self
                .builder
                .struct_gep(llvm_val_ty, alloca, 1, "proj.payload");
            let i8_ty = self.builder.i8_type();
            let offset_val = self.builder.const_i64(byte_offset as i64);
            let slot_ptr = self.builder.gep(
                i8_ty,
                payload_ptr,
                &[offset_val],
                &format!("proj.{field}.gep"),
            );

            if is_boxed_enum_field(self.pool, val_ty, ty) {
                let ptr_ty = self.builder.ptr_type();
                let rc_ptr = self
                    .builder
                    .load(ptr_ty, slot_ptr, &format!("proj.{field}.ptr"));
                let loaded = self
                    .builder
                    .load(result_ty, rc_ptr, &format!("proj.{field}"));
                self.def_var_repr(dst, loaded, func);
            } else {
                let loaded = self
                    .builder
                    .load(result_ty, slot_ptr, &format!("proj.{field}"));
                self.def_var_repr(dst, loaded, func);
            }
        } else {
            self.emit_project_tagged_union_field(
                dst,
                ty,
                val_ty,
                llvm_val_ty,
                alloca,
                field,
                result_ty,
                func,
            );
        }
        true
    }

    /// Project a `Result`/`Option` payload field via alloca + GEP + load. The
    /// payload is at struct index `field` (explicit tag) or `field - 1` (niche,
    /// no tag field). When the payload is a boxed recursive back-edge, the slot
    /// holds an RC `ptr` — load the box pointer then load the inner value
    /// through it (mirrors the general-enum boxed-field path).
    #[expect(
        clippy::too_many_arguments,
        reason = "projection threads dst/types/alloca/field"
    )]
    fn emit_project_tagged_union_field(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        val_ty: Idx,
        llvm_val_ty: super::super::value_id::LLVMTypeId,
        alloca: ValueId,
        field: u32,
        result_ty: super::super::value_id::LLVMTypeId,
        func: &ArcFunction,
    ) {
        let struct_idx = if self.get_niche_encoding(val_ty).is_some() {
            field - 1 // niche: no tag field
        } else {
            field // explicit: tag at 0, payload at 1+
        };
        let gep = self.builder.struct_gep(
            llvm_val_ty,
            alloca,
            struct_idx,
            &format!("proj.{field}.gep"),
        );
        if is_boxed_enum_field(self.pool, val_ty, ty) {
            let ptr_ty = self.builder.ptr_type();
            let box_ptr = self.builder.load(ptr_ty, gep, &format!("proj.{field}.box"));
            let loaded = self
                .builder
                .load(result_ty, box_ptr, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        } else {
            let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        }
    }

    /// Try to emit a niche-encoded enum tag extraction for `Project { field: 0 }`.
    ///
    /// Returns `true` when `val_ty` is niche-encoded and the projection was
    /// emitted (the niche field value becomes the switch scrutinee, recorded in
    /// [`niche_scrutinees`](Self::niche_scrutinees)); `false` otherwise.
    fn try_emit_project_niche_tag(&mut self, dst: ArcVarId, value: ArcVarId, val_ty: Idx) -> bool {
        let Some(encoding) = self.get_niche_encoding(val_ty) else {
            return false;
        };
        let niche_idx = encoding.niche_field_index().unwrap();
        let v = self.var(value);
        let llvm_ty = self.resolve_type(val_ty);
        let niche_val =
            if let Some(extracted) = self.builder.extract_value(v, niche_idx, "niche.field") {
                extracted
            } else {
                // Pointer-based access: GEP + load.
                let field_ty = self
                    .builder
                    .struct_field_type(llvm_ty, niche_idx)
                    .unwrap_or_else(|| self.builder.i64_type());
                let gep = self
                    .builder
                    .struct_gep(llvm_ty, v, niche_idx, "niche.field.gep");
                self.builder.load(field_ty, gep, "niche.field")
            };
        self.niche_scrutinees.insert(dst, val_ty);
        self.def_var(dst, super::EmittedValue::Immediate(niche_val));
        true
    }

    /// Try to emit a scalar-integer sum tag extraction for `Project { field: 0 }`.
    ///
    /// Returns `true` when `val_ty` lowers to a bare non-aggregate integer
    /// (e.g. `Ordering` = i8) — the value IS the discriminant, so it is read
    /// directly as the switch/comparison scrutinee; `false` otherwise.
    ///
    /// Symmetric to the scalar-int guard in `emit_construct`; without it, the
    /// fall-through path would `extract_value(i8, 0)` (the `extract_value on
    /// non-struct value` malformed IR). Keys on the resolved LLVM type via the
    /// type-introspection SSOT, so aggregate struct/enum dst keep their path.
    ///
    /// The dst var carries the discriminant as `Tag::Int` (i64) in ARC IR, so
    /// zero-extend the narrow tag (discriminants are non-negative 0..N-1) to
    /// the dst's resolved width — otherwise the downstream decision-tree
    /// `icmp eq` / `Switch` compares an i8 against i64 constants (`Both
    /// operands to ICmp ... not of the same type`).
    fn try_emit_project_scalar_tag(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        val_ty: Idx,
    ) -> bool {
        let val_llvm_ty = self.resolve_type(val_ty);
        if !self.builder.is_scalar_int_type(val_llvm_ty) {
            return false;
        }
        let tag = self.var(value);
        let dst_ty = self.resolve_type(ty);
        let widened = if dst_ty == val_llvm_ty {
            tag
        } else {
            self.builder.zext(tag, dst_ty, "scalar.tag.zext")
        };
        self.def_var(dst, super::EmittedValue::Immediate(widened));
        true
    }

    /// Emit a `Project` instruction (field extraction).
    ///
    /// For tagged union payload fields (Result, Enum), delegates to
    /// [`try_emit_project_enum_payload`](Self::try_emit_project_enum_payload).
    /// For decomposed `__iter_next` results, delegates to
    /// [`emit_project_iter_next`](Self::emit_project_iter_next).
    pub(super) fn emit_project(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        func: &ArcFunction,
    ) {
        // Fast path: decomposed iter_next result — extract tag or element
        // directly without going through the {i64, T} wrapper struct.
        if let Some(&(tag, scratch_ptr, elem_llvm_ty)) = self.iter_next_decomposed.get(&value) {
            self.emit_project_iter_next(dst, field, tag, scratch_ptr, elem_llvm_ty, func);
            return;
        }

        // Tagged-pointer enum projection.
        // The entire enum is a single 64-bit slot encoded as `(ptr | tag)`.
        // Field 0 decodes the tag (low 3 bits) — this becomes the switch
        // scrutinee directly, no `tagged_ptr_scrutinees` map needed because
        // the decoded i64 tag works with the standard `Switch` path.
        // Field > 0 decodes the payload pointer (high 61 bits) — every
        // variant carries at most one pointer field, so the field index
        // beyond 0 always means "the payload pointer".
        let val_ty = func.var_type(value);
        if self.get_tagged_ptr_encoding(val_ty).is_some() {
            let v = self.var(value);
            if field == 0 {
                let tag = self.tagged_ptr_decode_tag(v, "tagged.tag");
                self.def_var(dst, super::EmittedValue::Immediate(tag));
            } else {
                let ptr = self.tagged_ptr_decode_ptr(v, "tagged.ptr");
                self.def_var_repr(dst, ptr, func);
            }
            return;
        }

        // Tagless single-variant enum tag extraction — the discriminant
        // is always 0 (one variant). No niche field to read.
        if field == 0 && self.is_tagless_enum(val_ty) {
            let zero = self.builder.const_i64(0);
            self.def_var_repr(dst, zero, func);
            return;
        }

        // Scalar-integer sum tag extraction (e.g. `Ordering` = i8): the value
        // IS the discriminant. Extracted to a helper so the dispatch body
        // stays under the `too_many_lines` cap.
        if field == 0 && self.try_emit_project_scalar_tag(dst, ty, value, val_ty) {
            return;
        }

        // Niche-encoded enum tag extraction.
        // When Project { field: 0 } targets a niche-encoded enum, extract the
        // niche field value (not a logical variant index). The raw niche field
        // value is recorded in `niche_scrutinees` so Switch can emit the
        // correct comparison.
        if field == 0 && self.try_emit_project_niche_tag(dst, value, val_ty) {
            return;
        }

        let val = self.var(value);
        let result_ty = self.resolve_type(ty);

        // For enum/Result payload fields (index > 0), the storage type may
        // differ from the variant's actual type. Use alloca + GEP + load to
        // reinterpret the bytes correctly through pointer casting.
        if field > 0
            && self.try_emit_project_enum_payload(dst, ty, value, field, val, result_ty, func)
        {
            return;
        }

        // Remap declaration-order field index to memory-order for LLVM.
        let val_ty = func.var_type(value);
        let mem_field = self.remap_struct_field(val_ty, field);

        // Boxed recursive struct/tuple field: the slot holds an RC pointer to
        // the heap-boxed child, not the inline aggregate. Extract the box
        // pointer, then load through it to recover the child value.
        if is_boxed_enum_field(self.pool, val_ty, ty) {
            let ptr_ty = self.builder.ptr_type();
            let rc_ptr = if let Some(extracted) =
                self.builder
                    .extract_value(val, mem_field, &format!("proj.{field}.ptr"))
            {
                extracted
            } else {
                let llvm_val_ty = self.resolve_type(val_ty);
                let gep = self.builder.struct_gep(
                    llvm_val_ty,
                    val,
                    mem_field,
                    &format!("proj.{field}.gep"),
                );
                self.builder.load(ptr_ty, gep, &format!("proj.{field}.ptr"))
            };
            let loaded = self
                .builder
                .load(result_ty, rc_ptr, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
            return;
        }

        if let Some(extracted) =
            self.builder
                .extract_value(val, mem_field, &format!("proj.{field}"))
        {
            // Sign-extend narrowed int fields (i8/i16/i32) back to
            // canonical width (i64) for computation. Only applies when the
            // ARC IR destination expects i64 (Tag::Int) but the struct field
            // is narrower due to integer narrowing.
            let dst_ty = func.var_type(dst);
            let widened = self.sext_narrowed_field(extracted, field, dst_ty);
            self.def_var_repr(dst, widened, func);
        } else {
            // Fallback: GEP-based field access for heap-allocated types
            let llvm_val_ty = self.resolve_type(val_ty);
            let gep =
                self.builder
                    .struct_gep(llvm_val_ty, val, mem_field, &format!("proj.{field}.gep"));
            let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        }
    }

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
