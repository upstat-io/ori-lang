//! Enum variant construction for [`ArcIrEmitter`].
//!
//! Handles two emission strategies for enum variant values:
//! - `insertvalue` chain (fast path, no memory roundtrip)
//! - `alloca` + GEP + store + load (fallback for recursive/boxed fields)

use ori_arc::ir::ArcVarId;
use ori_types::{Idx, Tag};

use super::context::is_boxed_enum_field;
use super::drop_enum::compute_variant_field_offsets;
use super::ArcIrEmitter;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a variant via pure `insertvalue` chain (no memory roundtrip).
    ///
    /// Handles two enum layouts:
    /// - `{ i64, [M x i64] }` (user-defined enums) — nested insert at `[1, i]`
    /// - `{ i64, T }` (Option/Result) — direct insert at field `1`
    ///
    /// Falls back to alloca if any field isn't single-word (can't insert
    /// a multi-word value into an i64 array element).
    pub(super) fn emit_variant_via_insertvalue(
        &mut self,
        llvm_ty: super::super::value_id::LLVMTypeId,
        ty: Idx,
        tag_val: ValueId,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
        variant_field_types: &[Idx],
    ) -> ValueId {
        let mut result = self.builder.const_zero_ty(llvm_ty);
        // Insert tag at index 0
        result = self.builder.insert_value(result, tag_val, 0, "variant.tag");

        if arg_vals.is_empty() {
            return result;
        }

        // Check payload layout: array-wrapped vs direct.
        let has_array_payload = self.builder.is_struct_field_array(llvm_ty, 1);

        if has_array_payload {
            // User-defined enum: `{ i64, [M x i64] }` — fields must be single-word.
            if arg_vals.iter().all(|&v| self.builder.is_single_word(v)) {
                for (i, &val) in arg_vals.iter().enumerate() {
                    result = self.builder.insert_value_nested(
                        result,
                        val,
                        &[1, i as u32],
                        &format!("variant.f{i}"),
                    );
                }
            } else {
                // Multi-word fields (e.g. struct payloads like `Wrapper { value: int }`)
                // can't be inserted into [M x i64] array slots via insertvalue.
                // Delegate to alloca path which stores each field through GEP.
                return self.emit_variant_via_alloca(
                    llvm_ty,
                    ty,
                    tag_val,
                    arg_vals,
                    arc_args,
                    variant_field_types,
                );
            }
        } else {
            // Option/Result layout: `{ i64, T }` — insert payload directly.
            // This only works when the value's LLVM type matches the struct
            // field's type. E.g., `Ok(42)` for `Result<int, str>` carries i64
            // but the payload slot is `{ i64, i64, ptr }` — type mismatch.
            let types_match = arg_vals.iter().enumerate().all(|(i, &v)| {
                self.builder
                    .value_type_matches_struct_field(llvm_ty, 1 + i as u32, v)
            });
            if types_match {
                for (i, &val) in arg_vals.iter().enumerate() {
                    result = self.builder.insert_value(
                        result,
                        val,
                        1 + i as u32,
                        &format!("variant.f{i}"),
                    );
                }
            } else {
                // Type mismatch: fall back to alloca for correct byte layout.
                return self.emit_variant_via_alloca(
                    llvm_ty,
                    ty,
                    tag_val,
                    arg_vals,
                    arc_args,
                    variant_field_types,
                );
            }
        }

        result
    }

    /// Emit a variant via alloca+GEP+store+load (fallback for recursive fields).
    ///
    /// Used when any field is a boxed recursive reference. The RC-allocated
    /// pointer must be stored through memory, then loaded back as i64.
    pub(super) fn emit_variant_via_alloca(
        &mut self,
        llvm_ty: super::super::value_id::LLVMTypeId,
        ty: Idx,
        tag_val: ValueId,
        arg_vals: &[ValueId],
        arc_args: &[ArcVarId],
        variant_field_types: &[Idx],
    ) -> ValueId {
        let alloca = self.builder.alloca(llvm_ty, "variant");

        if arg_vals.is_empty() {
            let zero = self.builder.const_zero_ty(llvm_ty);
            self.builder.store(zero, alloca);
        }

        let tag_gep = self.builder.struct_gep(llvm_ty, alloca, 0, "variant.tag");
        self.builder.store(tag_val, tag_gep);

        if !arg_vals.is_empty() {
            let payload_ptr = self
                .builder
                .struct_gep(llvm_ty, alloca, 1, "variant.payload");
            let i8_ty = self.builder.i8_type();

            // Compute byte offsets accounting for multi-slot fields (e.g. str = 24 bytes)
            let offsets = compute_variant_field_offsets(variant_field_types, ty, self);

            for (i, &val) in arg_vals.iter().enumerate() {
                let byte_offset = offsets.get(i).copied().unwrap_or(0);
                let idx = self.builder.const_i64(byte_offset as i64);
                let slot = self
                    .builder
                    .gep(i8_ty, payload_ptr, &[idx], "variant.field");

                let field_ty = variant_field_types.get(i).copied();
                if field_ty.is_some_and(|ft| is_boxed_enum_field(self.pool, ty, ft)) {
                    let size = self.element_store_size(ty);
                    let rc_ptr = self.rc_alloc(size, 8);
                    self.builder.store(val, rc_ptr);
                    self.builder.store(rc_ptr, slot);
                    // When storing an inline enum value to a boxed field, we
                    // need to increment its sub-pointers ONLY if the source
                    // value has other live references (i.e., it's borrowed —
                    // the caller retains a reference). For consumed values
                    // (owned, last use), this is a move: no inc needed.
                    if let Some(&arc_var) = arc_args.get(i) {
                        if self.is_var_borrowed_rooted(arc_var) {
                            let ft = field_ty.expect("field type present");
                            let resolved = self.pool.resolve_fully(ft);
                            let pool_tag = self.pool.tag(resolved);
                            if pool_tag == Tag::Enum {
                                self.emit_inline_enum_inc(val, resolved, pool_tag, 1);
                            }
                        }
                    }
                } else {
                    self.builder.store(val, slot);
                }
            }
        }
        self.builder.load(llvm_ty, alloca, "variant")
    }
}
