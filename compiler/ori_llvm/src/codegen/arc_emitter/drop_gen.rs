//! Drop function IR generation for ARC reference counting.
//!
//! Generates specialized LLVM IR drop functions from [`ori_arc::DropInfo`]
//! descriptors. Each drop function is `extern "C" fn(*mut u8)` — called by
//! `ori_rc_dec` when a refcount reaches zero. The function decrements RC'd
//! children, then frees the allocation via `ori_rc_free`.
//!
//! # Drop function variants
//!
//! | `DropKind`     | IR pattern                                       |
//! |----------------|--------------------------------------------------|
//! | `Trivial`      | `ori_rc_free(ptr, size, align)` + ret             |
//! | `Fields`       | GEP+load+`ori_rc_dec` per RC'd field, then free  |
//! | `ClosureEnv`   | Same as Fields (different naming)                 |
//! | `Enum`         | Switch on tag → per-variant field drops, then free|
//! | `Collection`   | Loop: dec each element, free buffer, then free    |
//! | `Map`          | Loop: dec keys/values, free buffer, then free     |
//!
//! # Cycle safety
//!
//! Drop functions are cached in `ArcIrEmitter::drop_fn_cache` and the
//! `FunctionId` is inserted **before** generating the body. This handles
//! recursive types (e.g., linked lists) where the drop function references
//! itself for child decrements.

use ori_arc::{DropInfo, DropKind};
use ori_types::{Idx, Pool, Tag};

use super::ArcIrEmitter;
use crate::codegen::type_info::TypeLayoutResolver;
use crate::codegen::value_id::{FunctionId, ValueId};

/// Generate an LLVM drop function for the given type.
///
/// Declares a `void (ptr)` function with `nounwind` + `cold` attributes,
/// caches the `FunctionId` immediately (cycle safety), then generates the
/// body based on `DropKind`.
///
/// Naming: `_ori_drop$<idx_raw>` — unique per type pool index.
pub(super) fn generate_drop_fn<'a, 'scx: 'ctx, 'ctx, 'tcx>(
    emitter: &mut ArcIrEmitter<'a, 'scx, 'ctx, 'tcx>,
    ty: Idx,
    drop_info: &DropInfo,
) -> FunctionId {
    let func_name = format!("_ori_drop${}", ty.raw());

    // Declare: void @_ori_drop$N(ptr %data)
    let ptr_ty = emitter.builder.ptr_type();
    let func_id = emitter.builder.declare_void_function(&func_name, &[ptr_ty]);
    emitter.builder.set_ccc(func_id);
    emitter.builder.add_nounwind_attribute(func_id);
    emitter.builder.add_cold_attribute(func_id);

    // Cache before body generation (cycle safety for recursive types)
    emitter.drop_fn_cache.insert(ty, func_id);

    // Create entry block and position builder
    let entry = emitter.builder.append_block(func_id, "entry");
    emitter.builder.position_at_end(entry);
    emitter.builder.set_current_function(func_id);

    // Data pointer parameter
    let data_ptr = emitter.builder.get_param(func_id, 0);

    // Generate body based on drop kind
    match &drop_info.kind {
        DropKind::Trivial => {
            emitter.emit_drop_rc_free(data_ptr, ty);
            emitter.builder.ret_void();
        }

        DropKind::Fields(fields) | DropKind::ClosureEnv(fields) => {
            emitter.emit_drop_fields(data_ptr, ty, fields);
        }

        DropKind::Enum(variants) => {
            emitter.emit_drop_enum(func_id, data_ptr, ty, variants);
        }

        DropKind::Collection { element_type } => {
            emitter.emit_drop_collection(func_id, data_ptr, ty, *element_type);
        }

        DropKind::Map {
            key_type,
            value_type,
            dec_keys,
            dec_values,
        } => {
            emitter.emit_drop_map(
                func_id,
                data_ptr,
                ty,
                *key_type,
                *value_type,
                *dec_keys,
                *dec_values,
            );
        }
    }

    func_id
}

// All body generators live in an impl block so they inherit the correct
// lifetime bounds from ArcIrEmitter's declaration (`'scx: 'ctx`).
impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    // -------------------------------------------------------------------
    // Body generators
    // -------------------------------------------------------------------

    /// Emit drop body for struct/tuple/closure-env with specific RC'd fields.
    fn emit_drop_fields(&mut self, data_ptr: ValueId, ty: Idx, fields: &[(u32, Idx)]) {
        let struct_llvm_ty = self.resolve_type(ty);

        for &(field_index, field_type) in fields {
            let field_llvm_ty = self.resolve_type(field_type);
            let field_ptr = self.builder.struct_gep(
                struct_llvm_ty,
                data_ptr,
                field_index,
                &format!("f{field_index}.ptr"),
            );
            let field_val = self
                .builder
                .load(field_llvm_ty, field_ptr, &format!("f{field_index}"));
            self.emit_drop_rc_dec(field_val, field_type);
        }

        self.emit_drop_rc_free(data_ptr, ty);
        self.builder.ret_void();
    }

    /// Emit drop body for an enum type (switch on tag, per-variant cleanup).
    fn emit_drop_enum(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        ty: Idx,
        variants: &[Vec<(u32, Idx)>],
    ) {
        let enum_llvm_ty = self.resolve_type(ty);
        let i8_ty = self.builder.i8_type();

        // Load tag (i8 at field 0 for all enum-like types)
        let tag_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, data_ptr, 0, "tag.ptr");
        let tag_val = self.builder.load(i8_ty, tag_ptr, "tag");

        // Convergence block: rc_free + ret
        let drop_done = self.builder.append_block(func_id, "drop.done");

        // Build switch cases (only for variants with RC'd fields)
        let mut case_tags = Vec::new();
        let mut case_blocks = Vec::new();
        let mut case_fields: Vec<&[(u32, Idx)]> = Vec::new();

        #[expect(
            clippy::cast_possible_truncation,
            reason = "variant count bounded by enum definition, well within i8 range"
        )]
        for (i, variant_fields) in variants.iter().enumerate() {
            if variant_fields.is_empty() {
                continue;
            }
            let block = self.builder.append_block(func_id, &format!("variant.{i}"));
            let tag_const = self.builder.const_i8(i as i8);
            case_tags.push(tag_const);
            case_blocks.push(block);
            case_fields.push(variant_fields.as_slice());
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
                    self.emit_drop_enum_variant_fields(data_ptr, ty, variant_fields);
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
    fn emit_drop_enum_variant_fields(
        &mut self,
        data_ptr: ValueId,
        ty: Idx,
        rc_fields: &[(u32, Idx)],
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

            // Find which variant's field list matches (by first RC'd field index)
            let variant_fields = rc_fields
                .first()
                .and_then(|&(fi, _)| {
                    all_field_lists
                        .iter()
                        .find(|fields| fields.len() > fi as usize)
                })
                .map_or(&[] as &[Idx], Vec::as_slice);

            // Compute byte offsets (fields packed at i64 alignment)
            let offsets = compute_variant_field_offsets(variant_fields, self);

            let i8_ty = self.builder.i8_type();
            for &(field_index, field_type) in rc_fields {
                let byte_offset = offsets.get(field_index as usize).copied().unwrap_or(0);
                let field_llvm_ty = self.resolve_type(field_type);
                let offset_val = self.builder.const_i64(byte_offset as i64);
                let field_ptr = self.builder.gep(
                    i8_ty,
                    payload_ptr,
                    &[offset_val],
                    &format!("f{field_index}.ptr"),
                );
                let field_val =
                    self.builder
                        .load(field_llvm_ty, field_ptr, &format!("f{field_index}"));
                self.emit_drop_rc_dec(field_val, field_type);
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

    /// Emit drop body for a collection type ([T], set[T]).
    fn emit_drop_collection(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        ty: Idx,
        element_type: Idx,
    ) {
        let list_llvm_ty = self.resolve_type(ty);
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();

        // Load len (field 0), cap (field 1), data pointer (field 2)
        let len_ptr = self
            .builder
            .struct_gep(list_llvm_ty, data_ptr, 0, "len.ptr");
        let len = self.builder.load(i64_ty, len_ptr, "len");

        let cap_ptr = self
            .builder
            .struct_gep(list_llvm_ty, data_ptr, 1, "cap.ptr");
        let cap = self.builder.load(i64_ty, cap_ptr, "cap");

        let data_field_ptr = self
            .builder
            .struct_gep(list_llvm_ty, data_ptr, 2, "data.field.ptr");
        let elem_data = self.builder.load(ptr_ty, data_field_ptr, "elem_data");

        // Get element drop function (may recursively generate)
        let elem_drop_fn = self.get_or_generate_drop_fn(element_type);

        // Emit dec loop
        self.emit_drop_element_loop(func_id, elem_data, len, element_type, elem_drop_fn, "elem");

        // Free element buffer + collection struct
        self.emit_drop_list_free_data(elem_data, cap, element_type);
        self.emit_drop_rc_free(data_ptr, ty);
        self.builder.ret_void();
    }

    /// Emit drop body for a map type ({K: V}).
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors DropKind::Map fields; grouping would add indirection"
    )]
    fn emit_drop_map(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        ty: Idx,
        key_type: Idx,
        value_type: Idx,
        dec_keys: bool,
        dec_values: bool,
    ) {
        let map_llvm_ty = self.resolve_type(ty);
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();

        // Map layout: { i64 len, i64 cap, ptr keys, ptr values }
        let len_ptr = self.builder.struct_gep(map_llvm_ty, data_ptr, 0, "len.ptr");
        let len = self.builder.load(i64_ty, len_ptr, "len");

        if dec_keys {
            let keys_ptr = self
                .builder
                .struct_gep(map_llvm_ty, data_ptr, 2, "keys.field.ptr");
            let keys_data = self.builder.load(ptr_ty, keys_ptr, "keys_data");
            let key_drop_fn = self.get_or_generate_drop_fn(key_type);
            self.emit_drop_element_loop(func_id, keys_data, len, key_type, key_drop_fn, "key");
        }

        if dec_values {
            let vals_ptr = self
                .builder
                .struct_gep(map_llvm_ty, data_ptr, 3, "vals.field.ptr");
            let vals_data = self.builder.load(ptr_ty, vals_ptr, "vals_data");
            let val_drop_fn = self.get_or_generate_drop_fn(value_type);
            self.emit_drop_element_loop(func_id, vals_data, len, value_type, val_drop_fn, "val");
        }

        self.emit_drop_rc_free(data_ptr, ty);
        self.builder.ret_void();
    }

    /// Emit a loop that decrements RC for each element in an array.
    ///
    /// Shared between collection and map drop.
    fn emit_drop_element_loop(
        &mut self,
        func_id: FunctionId,
        array_ptr: ValueId,
        len: ValueId,
        element_type: Idx,
        elem_drop_fn: ValueId,
        prefix: &str,
    ) {
        let i64_ty = self.builder.i64_type();
        let elem_llvm_ty = self.resolve_type(element_type);

        let entry_block = self.builder.current_block().expect("current block");
        let loop_header = self
            .builder
            .append_block(func_id, &format!("{prefix}.loop.hdr"));
        let loop_body = self
            .builder
            .append_block(func_id, &format!("{prefix}.loop.body"));
        let loop_done = self
            .builder
            .append_block(func_id, &format!("{prefix}.loop.done"));

        let zero = self.builder.const_i64(0);
        let one = self.builder.const_i64(1);

        // entry → loop.header
        self.builder.br(loop_header);

        // loop.header: phi + cmp + branch
        self.builder.position_at_end(loop_header);
        let i_phi = self.builder.phi(i64_ty, &format!("{prefix}.i"));
        let done = self.builder.icmp_sge(i_phi, len, &format!("{prefix}.done"));
        self.builder.cond_br(done, loop_done, loop_body);

        // loop.body: load element, dec, increment, loop back
        self.builder.position_at_end(loop_body);
        let elem_ptr =
            self.builder
                .gep(elem_llvm_ty, array_ptr, &[i_phi], &format!("{prefix}.ptr"));
        let elem_val = self
            .builder
            .load(elem_llvm_ty, elem_ptr, &format!("{prefix}.val"));
        self.emit_drop_rc_dec_with_fn(elem_val, elem_drop_fn);
        let i_next = self.builder.add(i_phi, one, &format!("{prefix}.i.next"));
        self.builder.br(loop_header);

        // Patch phi: entry → 0, loop.body → i.next
        self.builder
            .add_phi_incoming(i_phi, &[(zero, entry_block), (i_next, loop_body)]);

        // Position at loop.done for caller to continue
        self.builder.position_at_end(loop_done);
    }

    // -------------------------------------------------------------------
    // Runtime call helpers
    // -------------------------------------------------------------------

    /// Emit `ori_rc_dec(val, drop_fn)` for a child field.
    fn emit_drop_rc_dec(&mut self, val: ValueId, field_type: Idx) {
        let drop_fn = self.get_or_generate_drop_fn(field_type);
        self.emit_drop_rc_dec_with_fn(val, drop_fn);
    }

    /// Emit `ori_rc_dec(val, drop_fn_ptr)` with a pre-computed drop function.
    fn emit_drop_rc_dec_with_fn(&mut self, val: ValueId, drop_fn_ptr: ValueId) {
        if let Some(llvm_func) = self.builder.scx().llmod.get_function("ori_rc_dec") {
            let func_id = self.builder.intern_function(llvm_func);
            self.builder.call(func_id, &[val, drop_fn_ptr], "");
        }
    }

    /// Emit `ori_rc_free(data_ptr, size, align)` to deallocate an RC object.
    fn emit_drop_rc_free(&mut self, data_ptr: ValueId, ty: Idx) {
        let size = compute_type_size(self, ty);
        let align = u64::from(self.type_info.get(ty).alignment());

        let size_val = self.builder.const_i64(size as i64);
        let align_val = self.builder.const_i64(align as i64);

        if let Some(llvm_func) = self.builder.scx().llmod.get_function("ori_rc_free") {
            let func_id = self.builder.intern_function(llvm_func);
            self.builder
                .call(func_id, &[data_ptr, size_val, align_val], "");
        }
    }

    /// Emit `ori_list_free_data(data, cap, elem_size)` to free a collection buffer.
    fn emit_drop_list_free_data(&mut self, data: ValueId, cap: ValueId, element_type: Idx) {
        let elem_size = compute_type_size(self, element_type);
        let elem_size_val = self.builder.const_i64(elem_size as i64);

        if let Some(llvm_func) = self.builder.scx().llmod.get_function("ori_list_free_data") {
            let func_id = self.builder.intern_function(llvm_func);
            self.builder.call(func_id, &[data, cap, elem_size_val], "");
        }
    }
}

// ---------------------------------------------------------------------------
// Free helpers (no &mut self needed)
// ---------------------------------------------------------------------------

/// Compute the store size of a type in bytes.
///
/// Uses `TypeInfo::size()` for well-known types (str=16, list=24, etc.).
/// Falls back to `TypeLayoutResolver::type_store_size()` for compound types
/// (struct, tuple, enum) where the size depends on field types.
fn compute_type_size(emitter: &ArcIrEmitter<'_, '_, '_, '_>, ty: Idx) -> u64 {
    emitter.type_info.get(ty).size().unwrap_or_else(|| {
        let llvm_ty = emitter.type_resolver.resolve(ty);
        TypeLayoutResolver::type_store_size(llvm_ty)
    })
}

/// Compute byte offsets for each field within a general enum variant.
///
/// Fields are packed at i64 alignment (8-byte slots) within the `[M x i64]`
/// payload array.
fn compute_variant_field_offsets(
    field_types: &[Idx],
    emitter: &ArcIrEmitter<'_, '_, '_, '_>,
) -> Vec<u64> {
    let mut offsets = Vec::with_capacity(field_types.len());
    let mut current: u64 = 0;

    for &field_ty in field_types {
        offsets.push(current);
        let llvm_ty = emitter.type_resolver.resolve(field_ty);
        let field_size = TypeLayoutResolver::type_store_size(llvm_ty);
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

/// Resolve a type through Named/Applied/Alias to its concrete tag.
fn resolve_type_through_aliases(ty: Idx, pool: &Pool) -> (Idx, Tag) {
    let tag = pool.tag(ty);
    match tag {
        Tag::Named | Tag::Applied | Tag::Alias => match pool.resolve(ty) {
            Some(resolved) => resolve_type_through_aliases(resolved, pool),
            None => (ty, tag),
        },
        _ => (ty, tag),
    }
}
