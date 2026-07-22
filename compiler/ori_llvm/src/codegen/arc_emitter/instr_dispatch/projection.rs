//! ARC `Project` instruction emission.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::{Idx, Tag};

use crate::codegen::value_id::{LLVMTypeId, ValueId};

use super::super::context::{is_boxed_enum_field, EmittedValue};
use super::super::drop_enum::{compute_variant_field_offsets, variant_field_offset};
use super::super::ArcIrEmitter;

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
            crate::codegen::type_info::TypeInfo::Result { .. }
                | crate::codegen::type_info::TypeInfo::Enum { .. }
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
            crate::codegen::type_info::TypeInfo::Enum { .. }
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
        llvm_val_ty: LLVMTypeId,
        alloca: ValueId,
        field: u32,
        result_ty: LLVMTypeId,
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
        self.def_var(dst, EmittedValue::Immediate(niche_val));
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
        self.def_var(dst, EmittedValue::Immediate(widened));
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
                self.def_var(dst, EmittedValue::Immediate(tag));
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
}
