//! RC data pointer extraction and inline enum inc/dec for [`ArcIrEmitter`].
//!
//! Provides methods to extract RC-managed heap data pointers from aggregate
//! values (List, Str, Map, Set, Struct, Tuple, Option) for `ori_rc_inc`/`ori_rc_dec`,
//! and inline tag-based RC traversal for enum-like types (Result, Enum).
//!
//! Both `emit_inline_enum_inc` and `emit_inline_enum_dec` use the same
//! tag-switch pattern with per-variant field traversal, shared via
//! `collect_variant_rc_fields`.

use ori_types::{Idx, Tag};

use super::context::is_boxed_enum_field;
use super::ArcIrEmitter;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Extract the RC-managed data pointer(s) from a value based on its type.
    ///
    /// `ori_rc_inc`/`ori_rc_dec` take raw pointers to RC-allocated heap data.
    /// When a value is an inline aggregate (List, Str, Map, Set, structs with
    /// RC fields), we extract the embedded data pointer(s). For compound types
    /// (struct, tuple, option, result, enum) that contain RC fields, we
    /// recursively extract from each RC'd field.
    ///
    /// | Type   | Layout                         | Data Ptr Field(s)  |
    /// |--------|--------------------------------|--------------------|
    /// | List   | `{i64, i64, ptr}`              | field 2            |
    /// | Set    | `{i64, i64, ptr}`              | field 2            |
    /// | Str    | `{i64, i64, ptr}` (SSO union)  | field 2 (SSO→null) |
    /// | Map    | `{i64, i64, ptr}`              | field 2            |
    /// | Struct | `{field0, field1, ...}`         | recurse per field  |
    /// | Tuple  | `{elem0, elem1, ...}`          | recurse per elem   |
    /// | Option | `{i64 tag, T payload}`         | recurse into inner |
    /// | Result | `{i64 tag, payload}`           | recurse into ok/err|
    /// | Enum   | `{i64 tag, payload}`           | recurse into fields|
    /// | Other  | already a ptr                  | use directly        |
    pub(super) fn extract_rc_data_ptrs(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        // Resolve type variables and Named/Applied/Alias to get the concrete tag.
        // The type checker may leave unresolved Var indices in compound types
        // (e.g., Option<Var(96)> where Var(96) → int via VarState::Link).
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        match tag {
            Tag::List | Tag::Set => {
                // {i64 len, i64 cap, ptr data} — data at field 2
                // NOTE: For RC inc, slices need ori_list_rc_inc(data, cap) instead of
                // ori_rc_inc(data). The HeapPointer strategy's emit_rc_inc_heap calls
                // call_rc_inc_all on these pointers, which is incorrect for slices.
                // The AggregateFields strategy (inc_value_rc) handles this correctly.
                // See emit_rc_inc_heap for the slice-aware override.
                if let Some(ptr) = self.builder.extract_value(val, 2, "rc.data_ptr") {
                    vec![ptr]
                } else {
                    vec![val]
                }
            }
            Tag::Str => {
                // {i64 len, i64 cap, ptr data} — data at field 2.
                // SSO strings have inline bytes in field 2, not a valid pointer.
                // Emit a select that returns null for SSO, so ori_rc_inc/dec
                // safely skip it (both null-check at entry).
                if let Some(ptr) = self.builder.extract_value(val, 2, "rc.data_ptr") {
                    let is_sso = self.emit_sso_check(ptr, "rc_str");
                    let null = self.builder.const_null_ptr();
                    let safe_ptr = self.builder.select(is_sso, null, ptr, "rc.str_safe_ptr");
                    vec![safe_ptr]
                } else {
                    vec![val]
                }
            }
            Tag::Map => {
                // {i64 len, i64 cap, ptr data} — single data buffer at field 2
                if let Some(ptr) = self.builder.extract_value(val, 2, "rc.data_ptr") {
                    vec![ptr]
                } else {
                    vec![val]
                }
            }
            Tag::Struct => self.extract_rc_from_struct_fields(val, resolved),
            Tag::Tuple => self.extract_rc_from_tuple_elems(val, resolved),
            Tag::Option => {
                // {i8 tag, T payload} — recurse into inner type at field 1
                let inner = self.pool.option_inner(resolved);
                if self.classifier.needs_rc(inner) {
                    if let Some(field) = self.builder.extract_value(val, 1, "rc.opt_inner") {
                        return self.extract_rc_data_ptrs(field, inner);
                    }
                }
                vec![] // scalar option — no RC needed
            }
            Tag::Result => {
                // Result has two possible types; we can't statically know which
                // is active. Skip RC here — the ARC pipeline should handle
                // result fields individually.
                vec![]
            }
            Tag::Enum => {
                // Enum variant tag + payload — can't statically know which
                // variant is active. Skip RC at the aggregate level.
                vec![]
            }
            _ => vec![val],
        }
    }

    /// Extract RC data pointers from a struct's fields.
    fn extract_rc_from_struct_fields(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        let fields = self.pool.struct_fields(ty);
        let mut ptrs = Vec::new();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "field count bounded by struct definition, well within u32 range"
        )]
        for (i, (_, field_ty)) in fields.into_iter().enumerate() {
            if self.classifier.needs_rc(field_ty) {
                if let Some(field_val) =
                    self.builder
                        .extract_value(val, i as u32, &format!("rc.field.{i}"))
                {
                    ptrs.extend(self.extract_rc_data_ptrs(field_val, field_ty));
                }
            }
        }
        ptrs
    }

    /// Extract RC data pointers from a tuple's elements.
    fn extract_rc_from_tuple_elems(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        let elems = self.pool.tuple_elems(ty);
        let mut ptrs = Vec::new();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "element count bounded by tuple arity, well within u32 range"
        )]
        for (i, elem_ty) in elems.into_iter().enumerate() {
            if self.classifier.needs_rc(elem_ty) {
                if let Some(elem_val) =
                    self.builder
                        .extract_value(val, i as u32, &format!("rc.elem.{i}"))
                {
                    ptrs.extend(self.extract_rc_data_ptrs(elem_val, elem_ty));
                }
            }
        }
        ptrs
    }

    /// Collect per-variant RC field info for an inline enum.
    ///
    /// Returns a vec-of-vecs: `[variant_idx][field_idx] = (field_position, field_type)`.
    /// Empty inner vec means the variant has no RC fields.
    fn collect_variant_rc_fields(&self, resolved_ty: Idx, pool_tag: Tag) -> Vec<Vec<(u32, Idx)>> {
        match pool_tag {
            Tag::Result => {
                let ok_ty = self.pool.result_ok(resolved_ty);
                let err_ty = self.pool.result_err(resolved_ty);
                let ok_fields = if self.classifier.needs_rc(ok_ty) {
                    vec![(0_u32, ok_ty)]
                } else {
                    vec![]
                };
                let err_fields = if self.classifier.needs_rc(err_ty) {
                    vec![(0_u32, err_ty)]
                } else {
                    vec![]
                };
                vec![ok_fields, err_fields]
            }
            Tag::Option => {
                let inner = self.pool.option_inner(resolved_ty);
                let some_fields = if self.classifier.needs_rc(inner) {
                    vec![(0_u32, inner)]
                } else {
                    vec![]
                };
                // Some=0 (has payload), None=1 (empty)
                vec![some_fields, vec![]]
            }
            Tag::Enum => {
                let variants = self.pool.enum_variants(resolved_ty);
                variants
                    .iter()
                    .map(|(_, field_tys)| {
                        field_tys
                            .iter()
                            .enumerate()
                            .filter(|(_, ty)| self.classifier.needs_rc(**ty))
                            .map(|(i, ty)| {
                                #[expect(
                                    clippy::cast_possible_truncation,
                                    reason = "variant field index fits u32"
                                )]
                                (i as u32, *ty)
                            })
                            .collect()
                    })
                    .collect()
            }
            _ => vec![],
        }
    }

    /// Emit inline tag-based RC inc for enum-like types (Result, Enum).
    ///
    /// These types are stack-allocated (no RC header) but may contain
    /// RC-typed fields in their variants. We store the value to a
    /// temporary alloca, load the tag, switch on it, and Inc the
    /// appropriate variant's RC fields.
    ///
    /// Mirrors `emit_inline_enum_dec` structurally.
    pub(super) fn emit_inline_enum_inc(
        &mut self,
        val: ValueId,
        resolved_ty: Idx,
        pool_tag: Tag,
        count: u32,
    ) {
        let variant_rc_fields = self.collect_variant_rc_fields(resolved_ty, pool_tag);

        if variant_rc_fields.iter().all(Vec::is_empty) {
            return;
        }

        let enum_llvm_ty = self.resolve_type(resolved_ty);
        let alloca = self.builder.alloca(enum_llvm_ty, "rc_inc.enum");
        self.builder.store(val, alloca);

        let i64_ty = self.builder.i64_type();
        let tag_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, alloca, 0, "rc_inc.tag.ptr");
        let tag_val = self.builder.load(i64_ty, tag_ptr, "rc_inc.tag");

        let done_block = self
            .builder
            .append_block(self.current_function, "rc_inc.done");

        let mut cases = Vec::new();
        for (i, fields) in variant_rc_fields.iter().enumerate() {
            if fields.is_empty() {
                continue;
            }
            let block = self
                .builder
                .append_block(self.current_function, &format!("rc_inc.v{i}"));
            let tag_const = self.builder.const_i64(i as i64);
            cases.push((tag_const, block, fields.as_slice()));
        }

        let switch_cases: Vec<_> = cases.iter().map(|(tag, block, _)| (*tag, *block)).collect();
        self.builder.switch(tag_val, done_block, &switch_cases);

        for &(_, block, fields) in &cases {
            self.builder.position_at_end(block);

            for &(field_index, field_type) in fields {
                if matches!(pool_tag, Tag::Result | Tag::Option) {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let struct_idx = 1 + field_index;
                    let field_ptr = self.builder.struct_gep(
                        enum_llvm_ty,
                        alloca,
                        struct_idx,
                        "rc_inc.payload.ptr",
                    );
                    let field_val = self
                        .builder
                        .load(field_llvm_ty, field_ptr, "rc_inc.payload");
                    self.inc_value_rc(field_val, field_type, count);
                } else if is_boxed_enum_field(self.pool, resolved_ty, field_type) {
                    let payload_ptr =
                        self.builder
                            .struct_gep(enum_llvm_ty, alloca, 1, "rc_inc.payload");
                    let i64_ty = self.builder.i64_type();
                    let idx = self.builder.const_i64(i64::from(field_index));
                    let field_ptr =
                        self.builder
                            .gep(i64_ty, payload_ptr, &[idx], "rc_inc.field.ptr");
                    let ptr_ty = self.builder.ptr_type();
                    let rc_ptr = self.builder.load(ptr_ty, field_ptr, "rc_inc.field.rc");
                    self.call_rc_inc_all(&[rc_ptr], count);
                } else {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let payload_ptr =
                        self.builder
                            .struct_gep(enum_llvm_ty, alloca, 1, "rc_inc.payload");
                    let i64_ty = self.builder.i64_type();
                    let idx = self.builder.const_i64(i64::from(field_index));
                    let field_ptr =
                        self.builder
                            .gep(i64_ty, payload_ptr, &[idx], "rc_inc.field.ptr");
                    let field_val = self.builder.load(field_llvm_ty, field_ptr, "rc_inc.field");
                    self.inc_value_rc(field_val, field_type, count);
                }
            }

            self.builder.br(done_block);
        }

        self.builder.position_at_end(done_block);
    }

    /// Emit inline tag-based cleanup for enum-like types (Result, Enum).
    ///
    /// These types are stack-allocated (no RC header) but may contain
    /// RC-typed fields in their variants. We store the value to a
    /// temporary alloca, load the tag, switch on it, and Dec the
    /// appropriate variant's RC fields.
    ///
    /// For `Result<int, str>`: tag 0 (Ok) → nothing; tag 1 (Err) → Dec str.
    pub(super) fn emit_inline_enum_dec(&mut self, val: ValueId, resolved_ty: Idx, pool_tag: Tag) {
        let variant_rc_fields = self.collect_variant_rc_fields(resolved_ty, pool_tag);

        if variant_rc_fields.iter().all(Vec::is_empty) {
            return;
        }

        // Store value to alloca so we can use GEP for field access
        let enum_llvm_ty = self.resolve_type(resolved_ty);
        let alloca = self.builder.alloca(enum_llvm_ty, "rc_dec.enum");
        self.builder.store(val, alloca);

        // Load tag (i64 at field 0)
        let i64_ty = self.builder.i64_type();
        let tag_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, alloca, 0, "rc_dec.tag.ptr");
        let tag_val = self.builder.load(i64_ty, tag_ptr, "rc_dec.tag");

        // Convergence block
        let done_block = self
            .builder
            .append_block(self.current_function, "rc_dec.done");

        // Build switch cases for variants with RC fields
        let mut cases = Vec::new();
        for (i, fields) in variant_rc_fields.iter().enumerate() {
            if fields.is_empty() {
                continue;
            }
            let block = self
                .builder
                .append_block(self.current_function, &format!("rc_dec.v{i}"));
            let tag_const = self.builder.const_i64(i as i64);
            cases.push((tag_const, block, fields.as_slice()));
        }

        let switch_cases: Vec<_> = cases.iter().map(|(tag, block, _)| (*tag, *block)).collect();
        self.builder.switch(tag_val, done_block, &switch_cases);

        // Emit per-variant cleanup
        for &(_, block, fields) in &cases {
            self.builder.position_at_end(block);

            for &(field_index, field_type) in fields {
                // Result/Option: typed payload fields at struct index 1+
                // General Enum: payload is [M x i64] at struct field 1
                if matches!(pool_tag, Tag::Result | Tag::Option) {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let struct_idx = 1 + field_index;
                    let field_ptr = self.builder.struct_gep(
                        enum_llvm_ty,
                        alloca,
                        struct_idx,
                        "rc_dec.payload.ptr",
                    );
                    let field_val = self
                        .builder
                        .load(field_llvm_ty, field_ptr, "rc_dec.payload");
                    self.dec_value_rc(field_val, field_type);
                } else if is_boxed_enum_field(self.pool, resolved_ty, field_type) {
                    // Recursive field: stored as RC pointer in the payload.
                    let payload_ptr =
                        self.builder
                            .struct_gep(enum_llvm_ty, alloca, 1, "rc_dec.payload");
                    let i64_ty = self.builder.i64_type();
                    let idx = self.builder.const_i64(i64::from(field_index));
                    let field_ptr =
                        self.builder
                            .gep(i64_ty, payload_ptr, &[idx], "rc_dec.field.ptr");
                    let ptr_ty = self.builder.ptr_type();
                    let rc_ptr = self.builder.load(ptr_ty, field_ptr, "rc_dec.field.rc");
                    let drop_fn = self.get_or_generate_drop_fn(field_type);
                    self.call_rc_dec_all(&[rc_ptr], drop_fn);
                } else {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let payload_ptr =
                        self.builder
                            .struct_gep(enum_llvm_ty, alloca, 1, "rc_dec.payload");
                    let i64_ty = self.builder.i64_type();
                    let idx = self.builder.const_i64(i64::from(field_index));
                    let field_ptr =
                        self.builder
                            .gep(i64_ty, payload_ptr, &[idx], "rc_dec.field.ptr");
                    let field_val = self.builder.load(field_llvm_ty, field_ptr, "rc_dec.field");
                    self.dec_value_rc(field_val, field_type);
                }
            }

            self.builder.br(done_block);
        }

        self.builder.position_at_end(done_block);
    }
}
