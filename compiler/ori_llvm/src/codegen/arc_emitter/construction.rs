//! Value construction emission for [`ArcIrEmitter`].
//!
//! Handles `Construct` instructions: building structs/tuples, enum variants
//! (with recursive field boxing), list literals, map literals, and set literals.

use ori_arc::ir::{ArcVarId, CtorKind};
use ori_types::{Idx, Tag};

use super::context::is_boxed_enum_field;
use super::ArcIrEmitter;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a `Construct` instruction.
    pub(super) fn emit_construct(
        &mut self,
        ty: Idx,
        ctor: &CtorKind,
        args: &[ArcVarId],
    ) -> ValueId {
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();
        let llvm_ty = self.resolve_type(ty);

        match ctor {
            CtorKind::Struct(_) | CtorKind::Tuple => {
                // Build a struct value from fields
                self.builder.build_struct(llvm_ty, &arg_vals, "ctor")
            }

            CtorKind::EnumVariant { variant, .. } => {
                // Enum layout is { i64 tag, [M x i64] payload } where M is
                // sized for the largest variant.
                let tag_val = self.builder.const_i64(i64::from(*variant));

                // Check for recursive enum fields that need RC allocation.
                // These require the alloca roundtrip because we need to store
                // the RC pointer into memory, then load back as i64.
                let resolved_enum = self.pool.resolve_fully(ty);
                let variant_field_types = if self.pool.tag(resolved_enum) == Tag::Enum {
                    let all_variants = self.pool.enum_variants(resolved_enum);
                    all_variants
                        .get(*variant as usize)
                        .map(|(_, fields)| fields.clone())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let has_boxed_fields = variant_field_types
                    .iter()
                    .any(|&ft| is_boxed_enum_field(self.pool, ty, ft));

                if has_boxed_fields {
                    // Recursive variant: fall back to alloca+GEP+store+load
                    // because RC-allocated pointers must be stored through memory.
                    self.emit_variant_via_alloca(
                        llvm_ty,
                        ty,
                        tag_val,
                        &arg_vals,
                        &variant_field_types,
                    )
                } else {
                    // Optimized case: pure insertvalue chain (no memory roundtrip).
                    // Start from zeroinitializer (unused payload slots are zero,
                    // safe for hashing/comparison).
                    self.emit_variant_via_insertvalue(
                        llvm_ty,
                        ty,
                        tag_val,
                        &arg_vals,
                        &variant_field_types,
                    )
                }
            }

            CtorKind::ListLiteral => {
                // List construction: allocate data, store elements, build struct
                let count = arg_vals.len();
                let type_info = self.type_info.get(ty);
                let elem_idx = match &type_info {
                    super::super::type_info::TypeInfo::List { element } => *element,
                    _ => ori_types::Idx::INT,
                };
                let elem_llvm_ty = self.resolve_type(elem_idx);
                let elem_size = self.element_store_size(elem_idx);

                let cap_val = self.builder.const_i64(count as i64);
                let esize_val = self.builder.const_i64(elem_size as i64);

                let alloc_fn = self.builder.runtime_fn("ori_list_alloc_data");
                let data_ptr = self
                    .builder
                    .call(alloc_fn, &[cap_val, esize_val], "list.data")
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                // Store each element into the data buffer
                for (i, &val) in arg_vals.iter().enumerate() {
                    let idx = self.builder.const_i64(i as i64);
                    let elem_ptr =
                        self.builder
                            .gep(elem_llvm_ty, data_ptr, &[idx], "list.elem_ptr");
                    self.builder.store(val, elem_ptr);
                }

                // Build list struct: {i64 len, i64 cap, ptr data}
                self.builder
                    .build_struct(llvm_ty, &[cap_val, cap_val, data_ptr], "list")
            }

            CtorKind::MapLiteral => {
                // Map literal: args are [key0, val0, key1, val1, ...]
                // Hash table layout: [metadata | keys | values]
                let count = arg_vals.len() / 2;
                let type_info = self.type_info.get(ty);
                let (key_idx, val_idx) = match &type_info {
                    super::super::type_info::TypeInfo::Map { key, value } => (*key, *value),
                    _ => (Idx::INT, Idx::INT),
                };
                let key_llvm_ty = self.resolve_type(key_idx);
                let val_llvm_ty = self.resolve_type(val_idx);
                let key_size = self.element_store_size(key_idx);
                let val_size = self.element_store_size(val_idx);

                let count_val = self.builder.const_i64(count as i64);
                let ks_val = self.builder.const_i64(key_size as i64);
                let vs_val = self.builder.const_i64(val_size as i64);

                // Allocate hash table buffer via runtime
                let i64_ty = self.builder.i64_type();
                let out_cap = self.builder.alloca(i64_ty, "map.out_cap");
                let alloc_fn = self.builder.runtime_fn("ori_map_literal_alloc");
                let data_ptr = self
                    .builder
                    .call(alloc_fn, &[count_val, ks_val, vs_val, out_cap], "map.data")
                    .unwrap_or_else(|| self.builder.const_null_ptr());
                let cap_val = self.builder.load(i64_ty, out_cap, "map.cap");

                // Get hash thunk for the key type
                let hash_thunk = self
                    .get_or_create_hash_thunk(key_idx)
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                // Insert each key-value pair via runtime
                let key_tmp = self.builder.alloca(key_llvm_ty, "map.key_tmp");
                let val_tmp = self.builder.alloca(val_llvm_ty, "map.val_tmp");
                let put_fn = self.builder.runtime_fn("ori_map_literal_put");
                for i in 0..count {
                    self.builder.store(arg_vals[i * 2], key_tmp);
                    self.builder.store(arg_vals[i * 2 + 1], val_tmp);
                    self.emit_rt_call(
                        put_fn,
                        &[
                            data_ptr, cap_val, key_tmp, val_tmp, ks_val, vs_val, hash_thunk,
                        ],
                        "map.put",
                    );
                }

                // Build map struct: {i64 count, i64 cap, ptr data}
                self.builder
                    .build_struct(llvm_ty, &[count_val, cap_val, data_ptr], "map")
            }

            CtorKind::SetLiteral => {
                // Set literal: hash table layout [metadata | elements]
                let count = arg_vals.len();
                let type_info = self.type_info.get(ty);
                let elem_idx = match &type_info {
                    super::super::type_info::TypeInfo::Set { element } => *element,
                    _ => Idx::INT,
                };
                let elem_llvm_ty = self.resolve_type(elem_idx);
                let elem_size = self.element_store_size(elem_idx);

                let count_val = self.builder.const_i64(count as i64);
                let esize_val = self.builder.const_i64(elem_size as i64);

                // Allocate hash table buffer via runtime
                let i64_ty = self.builder.i64_type();
                let out_cap = self.builder.alloca(i64_ty, "set.out_cap");
                let alloc_fn = self.builder.runtime_fn("ori_set_literal_alloc");
                let data_ptr = self
                    .builder
                    .call(alloc_fn, &[count_val, esize_val, out_cap], "set.data")
                    .unwrap_or_else(|| self.builder.const_null_ptr());
                let cap_val = self.builder.load(i64_ty, out_cap, "set.cap");

                // Get hash thunk for the element type
                let hash_thunk = self
                    .get_or_create_hash_thunk(elem_idx)
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                // Insert each element via runtime
                let elem_tmp = self.builder.alloca(elem_llvm_ty, "set.elem_tmp");
                let put_fn = self.builder.runtime_fn("ori_set_literal_put");
                for &val in &arg_vals {
                    self.builder.store(val, elem_tmp);
                    self.emit_rt_call(
                        put_fn,
                        &[data_ptr, cap_val, elem_tmp, esize_val, hash_thunk],
                        "set.put",
                    );
                }

                // Build set struct: {i64 len, i64 cap, ptr data}
                self.builder
                    .build_struct(llvm_ty, &[count_val, cap_val, data_ptr], "set")
            }

            CtorKind::Closure { .. } => {
                // Closures are always emitted via `PartialApply` in ARC IR,
                // which calls `emit_partial_apply()` → `build_closure_env()`.
                // `Construct { ctor: Closure }` is never produced by the lowerer.
                unreachable!("closures use PartialApply, not Construct")
            }
        }
    }

    /// Emit a variant via pure `insertvalue` chain (no memory roundtrip).
    ///
    /// Handles two enum layouts:
    /// - `{ i64, [M x i64] }` (user-defined enums) — nested insert at `[1, i]`
    /// - `{ i64, T }` (Option/Result) — direct insert at field `1`
    ///
    /// Falls back to alloca if any field isn't single-word (can't insert
    /// a multi-word value into an i64 array element).
    fn emit_variant_via_insertvalue(
        &mut self,
        llvm_ty: super::super::value_id::LLVMTypeId,
        ty: Idx,
        tag_val: ValueId,
        arg_vals: &[ValueId],
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
    fn emit_variant_via_alloca(
        &mut self,
        llvm_ty: super::super::value_id::LLVMTypeId,
        ty: Idx,
        tag_val: ValueId,
        arg_vals: &[ValueId],
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
            let i64_ty = self.builder.i64_type();

            for (i, &val) in arg_vals.iter().enumerate() {
                let idx = self.builder.const_i64(i as i64);
                let slot = self
                    .builder
                    .gep(i64_ty, payload_ptr, &[idx], "variant.field");

                let field_ty = variant_field_types.get(i).copied();
                if field_ty.is_some_and(|ft| is_boxed_enum_field(self.pool, ty, ft)) {
                    let size = self.element_store_size(ty);
                    let rc_ptr = self.rc_alloc(size, 8);
                    self.builder.store(val, rc_ptr);
                    self.builder.store(rc_ptr, slot);
                } else {
                    self.builder.store(val, slot);
                }
            }
        }
        self.builder.load(llvm_ty, alloca, "variant")
    }

    /// Emit a `CollectionReuse` instruction.
    ///
    /// Calls `ori_list_reset_buffer` to either reuse the old buffer (if
    /// uniquely owned) or allocate fresh (if shared). Then stores new
    /// elements and builds the result struct.
    pub(super) fn emit_collection_reuse(
        &mut self,
        old_var: ori_arc::ir::ArcVarId,
        ty: Idx,
        ctor: &CtorKind,
        args: &[ori_arc::ir::ArcVarId],
    ) -> ValueId {
        let old_val = self.var(old_var);
        let llvm_ty = self.resolve_type(ty);
        let new_len = args.len();

        // Determine element type from the collection type.
        let type_info = self.type_info.get(ty);
        let elem_idx = match (ctor, &type_info) {
            (CtorKind::ListLiteral, super::super::type_info::TypeInfo::List { element })
            | (CtorKind::SetLiteral, super::super::type_info::TypeInfo::Set { element }) => {
                *element
            }
            _ => Idx::INT,
        };

        let elem_llvm_ty = self.resolve_type(elem_idx);
        let elem_size = self.element_store_size(elem_idx);

        // Extract old {len, cap, data} from old_var.
        let old_data = self
            .builder
            .extract_value(old_val, 2, "reuse.old_data")
            .unwrap_or_else(|| self.builder.const_null_ptr());
        let old_len = self
            .builder
            .extract_value(old_val, 0, "reuse.old_len")
            .unwrap_or_else(|| self.builder.const_i64(0));
        let old_cap = self
            .builder
            .extract_value(old_val, 1, "reuse.old_cap")
            .unwrap_or_else(|| self.builder.const_i64(0));

        // Build call args for ori_list_reset_buffer.
        let new_len_val = self.builder.const_i64(new_len as i64);
        let elem_size_val = self.builder.const_i64(elem_size as i64);
        let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_idx);

        // Alloca for out_cap (caller-provided output parameter).
        let i64_ty = self.builder.i64_type();
        let out_cap_alloca = self.builder.alloca(i64_ty, "reuse.out_cap");

        // Call ori_list_reset_buffer.
        let reset_fn = self.builder.runtime_fn("ori_list_reset_buffer");
        let new_data = self
            .builder
            .call(
                reset_fn,
                &[
                    old_data,
                    old_len,
                    old_cap,
                    new_len_val,
                    elem_size_val,
                    elem_dec_fn,
                    out_cap_alloca,
                ],
                "reuse.data",
            )
            .unwrap_or_else(|| self.builder.const_null_ptr());

        // Store each new element into the returned buffer.
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();
        for (i, &val) in arg_vals.iter().enumerate() {
            let idx = self.builder.const_i64(i as i64);
            let elem_ptr = self
                .builder
                .gep(elem_llvm_ty, new_data, &[idx], "reuse.elem_ptr");
            self.builder.store(val, elem_ptr);
        }

        // Load the output capacity.
        let result_cap = self.builder.load(i64_ty, out_cap_alloca, "reuse.cap");

        // Build result struct: {i64 len, i64 cap, ptr data}
        self.builder
            .build_struct(llvm_ty, &[new_len_val, result_cap, new_data], "reuse.list")
    }
}
