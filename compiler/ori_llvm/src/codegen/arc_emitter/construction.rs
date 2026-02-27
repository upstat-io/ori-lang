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
                // sized for the largest variant. Fields are stored at i64-
                // aligned slots within the payload array.
                // Use alloca + GEP + store; mem2reg eliminates the alloca.
                let tag_val = self.builder.const_i64(i64::from(*variant));
                let alloca = self.builder.alloca(llvm_ty, "variant");
                let tag_gep = self.builder.struct_gep(llvm_ty, alloca, 0, "variant.tag");
                self.builder.store(tag_val, tag_gep);
                if !arg_vals.is_empty() {
                    let payload_ptr =
                        self.builder
                            .struct_gep(llvm_ty, alloca, 1, "variant.payload");
                    let i64_ty = self.builder.i64_type();

                    // Look up variant field types for recursive field detection.
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

                    for (i, &val) in arg_vals.iter().enumerate() {
                        let idx = self.builder.const_i64(i as i64);
                        let slot = self
                            .builder
                            .gep(i64_ty, payload_ptr, &[idx], "variant.field");

                        let field_ty = variant_field_types.get(i).copied();
                        if field_ty.is_some_and(|ft| is_boxed_enum_field(self.pool, ty, ft)) {
                            // Recursive field: RC-allocate and store pointer.
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
                let key_esize = self.builder.const_i64(key_size as i64);
                let val_esize = self.builder.const_i64(val_size as i64);

                let alloc_fn = self.builder.runtime_fn("ori_list_alloc_data");

                let keys_ptr = self
                    .builder
                    .call(alloc_fn, &[count_val, key_esize], "map.keys")
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                let vals_ptr = self
                    .builder
                    .call(alloc_fn, &[count_val, val_esize], "map.vals")
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                // Store keys and values into their respective arrays
                for i in 0..count {
                    let idx = self.builder.const_i64(i as i64);
                    let key_ptr = self
                        .builder
                        .gep(key_llvm_ty, keys_ptr, &[idx], "map.key_ptr");
                    self.builder.store(arg_vals[i * 2], key_ptr);

                    let val_ptr = self
                        .builder
                        .gep(val_llvm_ty, vals_ptr, &[idx], "map.val_ptr");
                    self.builder.store(arg_vals[i * 2 + 1], val_ptr);
                }

                // Build map struct: {i64 count, i64 cap, ptr keys, ptr vals}
                self.builder.build_struct(
                    llvm_ty,
                    &[count_val, count_val, keys_ptr, vals_ptr],
                    "map",
                )
            }

            CtorKind::SetLiteral => {
                // Set literal: same layout as list {i64 len, i64 cap, ptr data}
                let count = arg_vals.len();
                let type_info = self.type_info.get(ty);
                let elem_idx = match &type_info {
                    super::super::type_info::TypeInfo::Set { element } => *element,
                    _ => Idx::INT,
                };
                let elem_llvm_ty = self.resolve_type(elem_idx);
                let elem_size = self.element_store_size(elem_idx);

                let cap_val = self.builder.const_i64(count as i64);
                let esize_val = self.builder.const_i64(elem_size as i64);

                let alloc_fn = self.builder.runtime_fn("ori_list_alloc_data");
                let data_ptr = self
                    .builder
                    .call(alloc_fn, &[cap_val, esize_val], "set.data")
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                for (i, &val) in arg_vals.iter().enumerate() {
                    let idx = self.builder.const_i64(i as i64);
                    let elem_ptr = self
                        .builder
                        .gep(elem_llvm_ty, data_ptr, &[idx], "set.elem_ptr");
                    self.builder.store(val, elem_ptr);
                }

                // Build set struct: {i64 len, i64 cap, ptr data}
                self.builder
                    .build_struct(llvm_ty, &[cap_val, cap_val, data_ptr], "set")
            }

            CtorKind::Closure { .. } => {
                // Closures are always emitted via `PartialApply` in ARC IR,
                // which calls `emit_partial_apply()` → `build_closure_env()`.
                // `Construct { ctor: Closure }` is never produced by the lowerer.
                unreachable!("closures use PartialApply, not Construct")
            }
        }
    }
}
