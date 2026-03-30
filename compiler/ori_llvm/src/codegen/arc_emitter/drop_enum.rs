//! Enum drop function generation for ARC reference counting.
//!
//! Handles the most complex case of drop generation: enum types with
//! per-variant RC'd fields. Generates a tag-switch with per-variant
//! cleanup blocks.
//!
//! ## Enum layout
//!
//! - **Option/Result**: `{i64 tag, T payload}` — payload at struct index 1
//! - **General enum**: `{tag, [M x i64] payload}` — tag is narrowed via
//!   `min_tag_width()` (i8 for ≤256 variants, §07.1); fields at byte offsets
//!   within the payload array

use ori_types::{Idx, Pool, Tag};

use super::context::is_boxed_enum_field;
use super::ArcIrEmitter;
use crate::codegen::type_info::TypeLayoutResolver;
use crate::codegen::value_id::{FunctionId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit drop body for an enum type (switch on tag, per-variant cleanup).
    pub(super) fn emit_drop_enum(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        ty: Idx,
        variants: &[Vec<(u32, Idx)>],
    ) {
        let enum_llvm_ty = self.resolve_type(ty);

        // Load tag (narrowed type at field 0 — §07.1 discriminant narrowing)
        let tag_ty = self
            .builder
            .struct_field_type(enum_llvm_ty, 0)
            .unwrap_or_else(|| self.builder.i64_type());
        let tag_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, data_ptr, 0, "tag.ptr");
        let tag_val = self.builder.load(tag_ty, tag_ptr, "tag");

        // Convergence block: rc_free + ret
        let drop_done = self.builder.append_block(func_id, "drop.done");

        // Build switch cases (only for variants with RC'd fields)
        let mut case_tags = Vec::new();
        let mut case_blocks = Vec::new();
        let mut case_fields: Vec<&[(u32, Idx)]> = Vec::new();
        let mut case_variant_indices: Vec<usize> = Vec::new();

        for (i, variant_fields) in variants.iter().enumerate() {
            if variant_fields.is_empty() {
                continue;
            }
            let block = self.builder.append_block(func_id, &format!("variant.{i}"));
            let tag_const = self.builder.const_int_matching(tag_val, i as u64);
            case_tags.push(tag_const);
            case_blocks.push(block);
            case_fields.push(variant_fields.as_slice());
            case_variant_indices.push(i);
        }

        // Emit switch (default = drop.done for variants without RC'd fields)
        let switch_cases: Vec<_> = case_tags
            .iter()
            .zip(&case_blocks)
            .map(|(&tag, &block)| (tag, block))
            .collect();
        self.builder.switch(tag_val, drop_done, &switch_cases);

        // Determine access strategy from Pool tag
        let pool_tag = resolve_pool_tag(ty, self.pool);

        // Emit per-variant cleanup blocks
        for (idx, &block) in case_blocks.iter().enumerate() {
            let variant_fields = case_fields[idx];
            self.builder.position_at_end(block);

            match pool_tag {
                // Option/Result: payload is a typed field at struct index 1
                Tag::Option | Tag::Result => {
                    for &(field_index, field_type) in variant_fields {
                        let struct_idx = 1 + field_index; // field 0 = tag
                        let field_llvm_ty = self.resolve_type(field_type);
                        let field_ptr = self.builder.struct_gep(
                            enum_llvm_ty,
                            data_ptr,
                            struct_idx,
                            "payload.ptr",
                        );
                        let field_val = self.builder.load(field_llvm_ty, field_ptr, "payload");
                        self.emit_drop_rc_dec(field_val, field_type);
                    }
                }

                // General enum: payload is [M x i64] at struct field 1
                _ => {
                    self.emit_drop_enum_variant_fields(
                        data_ptr,
                        ty,
                        variant_fields,
                        case_variant_indices[idx],
                    );
                }
            }

            self.builder.br(drop_done);
        }

        // drop.done: free + ret
        self.builder.position_at_end(drop_done);
        self.emit_drop_rc_free(data_ptr, ty);
        self.builder.ret_void();
    }

    /// Emit RC dec for fields within a general enum variant.
    ///
    /// General enums store payload as `[M x i64]` at struct field 1.
    /// Fields are accessed via byte-offset GEP into the payload area.
    ///
    /// `variant_idx` identifies which variant in the Pool's variant list
    /// this cleanup block corresponds to, so field offsets are computed
    /// from the correct variant's field types.
    pub(super) fn emit_drop_enum_variant_fields(
        &mut self,
        data_ptr: ValueId,
        ty: Idx,
        rc_fields: &[(u32, Idx)],
        variant_idx: usize,
    ) {
        let enum_llvm_ty = self.resolve_type(ty);
        let payload_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, data_ptr, 1, "payload");

        // Try to get full variant field types from Pool for offset computation
        let (resolved_ty, resolved_tag) = resolve_type_through_aliases(ty, self.pool);

        if resolved_tag == Tag::Enum {
            // Get all variant field types to compute byte offsets
            let all_variants = self.pool.enum_variants(resolved_ty);
            let all_field_lists: Vec<Vec<Idx>> =
                all_variants.into_iter().map(|(_, fields)| fields).collect();

            // Use the correct variant's field list for offset computation
            let variant_fields = all_field_lists
                .get(variant_idx)
                .map_or(&[] as &[Idx], Vec::as_slice);

            // Compute byte offsets (fields packed at i64 alignment)
            let offsets = compute_variant_field_offsets(variant_fields, resolved_ty, self);

            let i8_ty = self.builder.i8_type();
            for &(field_index, field_type) in rc_fields {
                let byte_offset = offsets.get(field_index as usize).copied().unwrap_or(0);
                let offset_val = self.builder.const_i64(byte_offset as i64);
                let field_ptr = self.builder.gep(
                    i8_ty,
                    payload_ptr,
                    &[offset_val],
                    &format!("f{field_index}.ptr"),
                );

                if is_boxed_enum_field(self.pool, resolved_ty, field_type) {
                    // Recursive field: stored as RC pointer. Load and dec.
                    let ptr_ty = self.builder.ptr_type();
                    let rc_ptr =
                        self.builder
                            .load(ptr_ty, field_ptr, &format!("f{field_index}.rc"));
                    let drop_fn = self.get_or_generate_drop_fn(field_type);
                    self.emit_drop_rc_dec_with_fn(rc_ptr, drop_fn);
                } else {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let field_val =
                        self.builder
                            .load(field_llvm_ty, field_ptr, &format!("f{field_index}"));
                    self.emit_drop_rc_dec(field_val, field_type);
                }
            }
        } else {
            // Fallback: treat field_index as i64 slot offset
            tracing::warn!(
                ?resolved_tag,
                "drop_gen: non-Enum tag for general enum — using slot access"
            );
            let i64_ty = self.builder.i64_type();
            for &(field_index, field_type) in rc_fields {
                let field_llvm_ty = self.resolve_type(field_type);
                let slot_val = self.builder.const_i64(i64::from(field_index));
                let field_ptr = self.builder.gep(
                    i64_ty,
                    payload_ptr,
                    &[slot_val],
                    &format!("f{field_index}.ptr"),
                );
                let field_val =
                    self.builder
                        .load(field_llvm_ty, field_ptr, &format!("f{field_index}"));
                self.emit_drop_rc_dec(field_val, field_type);
            }
        }
    }
}

// Free helpers (no &mut self needed)

/// Compute byte offsets for each field within a general enum variant.
///
/// Fields are packed at i64 alignment (8-byte slots) within the `[M x i64]`
/// payload array. Recursive fields (same type as the enclosing enum) are
/// stored as 8-byte RC pointers, not inline.
pub(super) fn compute_variant_field_offsets(
    field_types: &[Idx],
    enum_type: Idx,
    emitter: &ArcIrEmitter<'_, '_, '_, '_>,
) -> Vec<u64> {
    let mut offsets = Vec::with_capacity(field_types.len());
    let mut current: u64 = 0;

    for &field_ty in field_types {
        offsets.push(current);
        let field_size = if is_boxed_enum_field(emitter.pool, enum_type, field_ty) {
            8 // Stored as RC pointer
        } else {
            let llvm_ty = emitter.type_resolver.resolve(field_ty);
            TypeLayoutResolver::type_store_size(llvm_ty)
        };
        // Round up to 8-byte alignment (i64 slot boundary)
        current += field_size.div_ceil(8).saturating_mul(8);
    }

    offsets
}

/// Resolve a type's Pool tag, following Named/Applied/Alias indirections.
fn resolve_pool_tag(ty: Idx, pool: &Pool) -> Tag {
    let (_, tag) = resolve_type_through_aliases(ty, pool);
    tag
}

/// Resolve a type through Var chains and Named/Applied/Alias to its concrete tag.
fn resolve_type_through_aliases(ty: Idx, pool: &Pool) -> (Idx, Tag) {
    // Use resolve_fully to handle both type variables (VarState::Link)
    // and named type aliases in one pass.
    let resolved = pool.resolve_fully(ty);
    (resolved, pool.tag(resolved))
}
