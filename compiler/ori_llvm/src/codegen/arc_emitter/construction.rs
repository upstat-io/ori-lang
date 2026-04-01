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
    #[expect(
        clippy::too_many_lines,
        reason = "construct dispatch over struct/enum/closure/tuple types"
    )]
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
                // BUG-04-008: Unit/void tuples resolve to i64 in LLVM (not a struct),
                // because LLVM void can't be stored. Return a zero constant directly
                // instead of calling build_struct on a non-struct type.
                let resolved_ty = self.pool.resolve_fully(ty);
                if matches!(self.pool.tag(resolved_ty), Tag::Unit | Tag::Never) {
                    return self.builder.const_zero_ty(llvm_ty);
                }
                // §06: reorder args from declaration order to memory order
                // before truncation and LLVM struct construction.
                let mem_args = self.reorder_args_to_memory_order(&arg_vals, ty);
                // §04.4: truncate canonical-width (i64) values to narrowed
                // field width (i8/i16/i32) when the struct has narrowed fields.
                let narrowed_args = self.trunc_for_narrowed_struct(llvm_ty, &mem_args, ty);
                self.builder.build_struct(llvm_ty, &narrowed_args, "ctor")
            }

            CtorKind::EnumVariant { variant, .. } => {
                // §07.2: Niche-encoded enum — no tag field, payload at index 0.
                if let Some(encoding) = self.get_niche_encoding(ty) {
                    return self
                        .emit_niche_variant_construct(llvm_ty, &arg_vals, &encoding, *variant);
                }

                // Explicit tag layout: { tag, [M x i64] payload } where the tag
                // type is narrowed via min_tag_width (§07.1).
                let tag_val =
                    self.builder
                        .const_int_for_struct_field(llvm_ty, 0, u64::from(*variant));

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

                // BUG-04-008: For user-defined enums (Tag::Enum), filter out
                // Unit/Never args — they are zero-sized and don't occupy payload
                // space. For Option/Result (where variant_field_types is empty
                // because they're not Tag::Enum), use args unchanged.
                let (eff_args, eff_arc_args, eff_field_types);
                if !variant_field_types.is_empty()
                    && variant_field_types.iter().any(|ft| {
                        let r = self.pool.resolve_fully(*ft);
                        matches!(self.pool.tag(r), Tag::Unit | Tag::Never)
                    })
                {
                    let non_void: Vec<usize> = variant_field_types
                        .iter()
                        .enumerate()
                        .filter(|(_, ft)| {
                            let r = self.pool.resolve_fully(**ft);
                            !matches!(self.pool.tag(r), Tag::Unit | Tag::Never)
                        })
                        .map(|(i, _)| i)
                        .collect();
                    eff_args = non_void.iter().map(|&i| arg_vals[i]).collect();
                    eff_arc_args = non_void.iter().map(|&i| args[i]).collect();
                    eff_field_types = non_void.iter().map(|&i| variant_field_types[i]).collect();
                } else {
                    eff_args = arg_vals.clone();
                    eff_arc_args = args.to_vec();
                    eff_field_types = variant_field_types.clone();
                }

                let has_boxed_fields = eff_field_types
                    .iter()
                    .any(|&ft| is_boxed_enum_field(self.pool, ty, ft));

                if has_boxed_fields {
                    // Recursive variant: fall back to alloca+GEP+store+load
                    // because RC-allocated pointers must be stored through memory.
                    self.emit_variant_via_alloca(
                        llvm_ty,
                        ty,
                        tag_val,
                        &eff_args,
                        &eff_arc_args,
                        &eff_field_types,
                    )
                } else {
                    // Optimized case: pure insertvalue chain (no memory roundtrip).
                    // Start from zeroinitializer (unused payload slots are zero,
                    // safe for hashing/comparison).
                    self.emit_variant_via_insertvalue(
                        llvm_ty,
                        ty,
                        tag_val,
                        &eff_args,
                        &eff_arc_args,
                        &eff_field_types,
                    )
                }
            }

            CtorKind::ListLiteral => {
                // List construction: allocate data, store elements, build struct
                let count = arg_vals.len();
                let type_info = self.type_info.get(ty);
                let elem_idx =
                    if let super::super::type_info::TypeInfo::List { element } = &type_info {
                        *element
                    } else {
                        debug_assert!(false, "ListLiteral TypeInfo mismatch: {type_info:?}");
                        ori_types::Idx::INT
                    };

                // §04.4 Phase C: use narrowed element type/size if the ReprPlan
                // has narrowed this collection's int elements (e.g., i8 for [int]
                // with elements in [-128, 127]).
                let collection_idx = self.pool.resolve_fully(ty);
                let elem_llvm_ty = self.collection_elem_llvm_type(collection_idx, elem_idx);
                let elem_size = self.collection_elem_size(collection_idx, elem_idx);

                let cap_val = self.builder.const_i64(count as i64);
                let esize_val = self.builder.const_i64(elem_size as i64);

                let alloc_fn = self.builder.runtime_fn("ori_list_alloc_data");
                let data_ptr = self
                    .builder
                    .call(alloc_fn, &[cap_val, esize_val], "list.data")
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                // Store each element into the data buffer.
                // For narrowed collections, trunc each i64 value to the narrow
                // width (e.g., i8) before storing.
                for (i, &val) in arg_vals.iter().enumerate() {
                    let idx = self.builder.const_i64(i as i64);
                    let elem_ptr =
                        self.builder
                            .gep(elem_llvm_ty, data_ptr, &[idx], "list.elem_ptr");
                    let store_val = self.trunc_for_narrowed_collection_element(
                        val,
                        collection_idx,
                        &format!("list.elem.trunc.{i}"),
                    );
                    self.builder.store(store_val, elem_ptr);
                }

                // Store elem_dec_fn and elem_count in the RC header so that
                // any RcDec reaching zero can perform element cleanup.
                // For scalar elements, elem_dec_fn is null (no RC children) —
                // the call writes null over zero-initialized null (idempotent).
                let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_idx);
                let store_dec_fn = self.builder.runtime_fn("ori_buffer_store_elem_dec");
                self.builder
                    .call(store_dec_fn, &[data_ptr, elem_dec_fn], "");
                let store_count_fn = self.builder.runtime_fn("ori_buffer_store_elem_count");
                self.builder.call(store_count_fn, &[data_ptr, cap_val], "");

                // Build list struct: {i64 len, i64 cap, ptr data}
                self.builder
                    .build_struct(llvm_ty, &[cap_val, cap_val, data_ptr], "list")
            }

            CtorKind::MapLiteral => {
                // Map literal: args are [key0, val0, key1, val1, ...]
                // Hash table layout: [metadata | keys | values]
                let count = arg_vals.len() / 2;
                let type_info = self.type_info.get(ty);
                let (key_idx, val_idx) =
                    if let super::super::type_info::TypeInfo::Map { key, value } = &type_info {
                        (*key, *value)
                    } else {
                        debug_assert!(false, "MapLiteral TypeInfo mismatch: {type_info:?}");
                        (Idx::INT, Idx::INT)
                    };
                let key_llvm_ty = self.resolve_type(key_idx);
                let val_llvm_ty = self.resolve_type(val_idx);
                // §04.4 Phase C: use narrowed element sizes for map buffers.
                let collection_idx = self.pool.resolve_fully(ty);
                let key_size = self.collection_elem_size(collection_idx, key_idx);
                let val_size = self.collection_elem_size(collection_idx, val_idx);

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
                let elem_idx =
                    if let super::super::type_info::TypeInfo::Set { element } = &type_info {
                        *element
                    } else {
                        debug_assert!(false, "SetLiteral TypeInfo mismatch: {type_info:?}");
                        Idx::INT
                    };
                // Sets always use canonical element sizes — eq/hash thunks load
                // full-width values, so narrowing set elements would cause
                // out-of-bounds reads. Phase C collection narrowing is gated
                // to list types only in ori_repr.
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

                // Insert each element via runtime (canonical width).
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

                // Store elem_dec_fn and elem_count in the RC header.
                let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_idx);
                let store_dec_fn = self.builder.runtime_fn("ori_buffer_store_elem_dec");
                self.builder
                    .call(store_dec_fn, &[data_ptr, elem_dec_fn], "");
                let store_count_fn = self.builder.runtime_fn("ori_buffer_store_elem_count");
                self.builder
                    .call(store_count_fn, &[data_ptr, count_val], "");

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
            _ => {
                debug_assert!(
                    false,
                    "collection reuse TypeInfo mismatch: ctor={ctor:?}, info={type_info:?}"
                );
                Idx::INT
            }
        };

        // §04.4 Phase C: narrowed element type/size for collection reuse.
        let collection_idx = self.pool.resolve_fully(ty);
        let elem_llvm_ty = self.collection_elem_llvm_type(collection_idx, elem_idx);
        let elem_size = self.collection_elem_size(collection_idx, elem_idx);

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
        // For narrowed collections, trunc to narrow width before storing.
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();
        for (i, &val) in arg_vals.iter().enumerate() {
            let idx = self.builder.const_i64(i as i64);
            let elem_ptr = self
                .builder
                .gep(elem_llvm_ty, new_data, &[idx], "reuse.elem_ptr");
            let store_val = self.trunc_for_narrowed_collection_element(
                val,
                collection_idx,
                &format!("reuse.elem.trunc.{i}"),
            );
            self.builder.store(store_val, elem_ptr);
        }

        // Store elem_dec_fn and elem_count in the new buffer's RC header.
        // ori_list_reset_buffer does NOT propagate internally — codegen
        // handles it externally after the reset returns.
        let store_dec_fn = self.builder.runtime_fn("ori_buffer_store_elem_dec");
        self.builder
            .call(store_dec_fn, &[new_data, elem_dec_fn], "");
        let store_count_fn = self.builder.runtime_fn("ori_buffer_store_elem_count");
        self.builder
            .call(store_count_fn, &[new_data, new_len_val], "");

        // Load the output capacity.
        let result_cap = self.builder.load(i64_ty, out_cap_alloca, "reuse.cap");

        // Build result struct: {i64 len, i64 cap, ptr data}
        self.builder
            .build_struct(llvm_ty, &[new_len_val, result_cap, new_data], "reuse.list")
    }

    /// §07.2: Construct a niche-encoded enum variant.
    ///
    /// Niche layout has no tag field — payload fields start at struct index 0.
    /// For the niche variant (e.g., None): create a zeroinit struct; `SetTag`
    /// will write the niche value afterward.
    /// For the data variant (e.g., Some(val)): insert payload at index 0.
    fn emit_niche_variant_construct(
        &mut self,
        llvm_ty: super::super::value_id::LLVMTypeId,
        arg_vals: &[ValueId],
        encoding: &super::tag_access::TagEncoding,
        variant: u32,
    ) -> ValueId {
        let mut result = self.builder.const_zero_ty(llvm_ty);
        if encoding.needs_tag_store(variant) {
            // Niche variant (no payload): insert niche_value directly
            // so that `SetTag` is not needed (avoids GEP-on-register issues).
            let niche_idx = encoding.niche_field_index().unwrap();
            let niche_value = encoding.variant_to_tag_value(variant);
            let niche_const =
                self.builder
                    .const_int_for_struct_field(llvm_ty, niche_idx, niche_value);
            return self
                .builder
                .insert_value(result, niche_const, niche_idx, "niche.tag");
        }
        // Data variant: insert payload fields starting at index 0.
        for (i, &val) in arg_vals.iter().enumerate() {
            #[expect(clippy::cast_possible_truncation, reason = "field index fits u32")]
            let idx = i as u32;
            result = self
                .builder
                .insert_value(result, val, idx, &format!("niche.val.{i}"));
        }
        result
    }
}
