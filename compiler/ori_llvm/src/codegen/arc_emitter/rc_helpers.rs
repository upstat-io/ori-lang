//! RC data pointer extraction and inline enum inc/dec for [`ArcIrEmitter`].
//!
//! Provides methods to extract RC-managed heap data pointers from aggregate
//! values (List, Str, Map, Set, Struct, Tuple, Option) for `ori_rc_inc`/`ori_rc_dec`,
//! and inline tag-based RC traversal for enum-like types (Result, Enum).
//!
//! Enum inc/dec share a single parameterized implementation via
//! `emit_inline_enum_rc_core`, called by both `emit_inline_enum_inc`
//! and `emit_inline_enum_dec`.

use ori_ir::{CLOSURE_FIELD_ENV, FIELD_DATA};
use ori_types::{Idx, Tag};

use super::context::is_boxed_enum_field;
use super::drop_enum::compute_variant_field_offsets;
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
    /// | Option  | `{i64 tag, T payload}`        | recurse into inner |
    /// | Result  | `{i64 tag, payload}`          | recurse into ok/err|
    /// | Enum    | `{i64 tag, payload}`          | recurse into fields|
    /// | Function| `{ptr fn, ptr env}`           | env ptr (field 1)  |
    /// | Other   | already a ptr                 | use directly       |
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
                if let Some(ptr) = self.builder.extract_value(val, FIELD_DATA, "rc.data_ptr") {
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
                if let Some(ptr) = self.builder.extract_value(val, FIELD_DATA, "rc.data_ptr") {
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
                if let Some(ptr) = self.builder.extract_value(val, FIELD_DATA, "rc.data_ptr") {
                    vec![ptr]
                } else {
                    vec![val]
                }
            }
            Tag::Struct => self.extract_rc_from_struct_fields(val, resolved),
            Tag::Tuple => self.extract_rc_from_tuple_elems(val, resolved),
            Tag::Option => {
                // {i64 tag, T payload} — recurse into inner type at field 1
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
            Tag::Function => {
                // Closure: { fn_ptr, env_ptr } — only env_ptr (field 1) is RC-managed.
                // The fn_ptr is a code pointer, not heap-allocated.
                if let Some(env_ptr) =
                    self.builder
                        .extract_value(val, CLOSURE_FIELD_ENV, "rc.closure_env")
                {
                    vec![env_ptr]
                } else {
                    vec![]
                }
            }
            _ => vec![val],
        }
    }

    /// Extract RC data pointers from a struct's fields (remap to memory order).
    fn extract_rc_from_struct_fields(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        let fields = self.pool.struct_fields(ty);
        let mut ptrs = Vec::new();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "field count bounded by struct definition, well within u32 range"
        )]
        for (i, (_, field_ty)) in fields.into_iter().enumerate() {
            if self.classifier.needs_rc(field_ty) {
                let mem_i = self.remap_struct_field(ty, i as u32);
                if let Some(field_val) =
                    self.builder
                        .extract_value(val, mem_i, &format!("rc.field.{i}"))
                {
                    ptrs.extend(self.extract_rc_data_ptrs(field_val, field_ty));
                }
            }
        }
        ptrs
    }

    /// Extract RC data pointers from a tuple's elements (remap to memory order).
    fn extract_rc_from_tuple_elems(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        let elems = self.pool.tuple_elems(ty);
        let mut ptrs = Vec::new();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "element count bounded by tuple arity, well within u32 range"
        )]
        for (i, elem_ty) in elems.into_iter().enumerate() {
            if self.classifier.needs_rc(elem_ty) {
                let mem_i = self.remap_struct_field(ty, i as u32);
                if let Some(elem_val) =
                    self.builder
                        .extract_value(val, mem_i, &format!("rc.elem.{i}"))
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
    /// Delegates to shared `emit_inline_enum_rc_core` with inc direction.
    pub(super) fn emit_inline_enum_inc(
        &mut self,
        val: ValueId,
        resolved_ty: Idx,
        pool_tag: Tag,
        count: u32,
    ) {
        self.emit_inline_enum_rc_core(val, resolved_ty, pool_tag, true, count);
    }

    /// Delegates to shared `emit_inline_enum_rc_core` with dec direction.
    pub(super) fn emit_inline_enum_dec(&mut self, val: ValueId, resolved_ty: Idx, pool_tag: Tag) {
        self.emit_inline_enum_rc_core(val, resolved_ty, pool_tag, false, 1);
    }

    /// Shared core for inline enum RC inc/dec.
    ///
    /// Both directions (inc/dec) share identical tag-switch scaffolding:
    /// alloca → load tag → switch on variants → GEP to payload field.
    /// The only difference is which RC operation is applied to each field.
    #[expect(
        clippy::too_many_lines,
        reason = "Tag-switch + per-variant field traversal is one indivisible pattern"
    )]
    fn emit_inline_enum_rc_core(
        &mut self,
        val: ValueId,
        resolved_ty: Idx,
        pool_tag: Tag,
        is_inc: bool,
        count: u32,
    ) {
        let variant_rc_fields = self.collect_variant_rc_fields(resolved_ty, pool_tag);

        if variant_rc_fields.iter().all(Vec::is_empty) {
            return;
        }

        // §07.2: Niche-encoded enum — conditional RC.
        if let Some(encoding) = self.get_niche_encoding(resolved_ty) {
            self.emit_niche_enum_rc(
                val,
                resolved_ty,
                pool_tag,
                &variant_rc_fields,
                &encoding,
                is_inc,
                count,
            );
            return;
        }

        let dir = if is_inc { "rc_inc" } else { "rc_dec" };

        let enum_llvm_ty = self.resolve_type(resolved_ty);
        let alloca = self.builder.alloca(enum_llvm_ty, &format!("{dir}.enum"));
        self.builder.store(val, alloca);

        // Load tag (narrowed type at field 0 — §07.1)
        let tag_ty = self
            .builder
            .struct_field_type(enum_llvm_ty, 0)
            .unwrap_or_else(|| self.builder.i64_type());
        let tag_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, alloca, 0, &format!("{dir}.tag.ptr"));
        let tag_val = self.builder.load(tag_ty, tag_ptr, &format!("{dir}.tag"));

        let done_block = self
            .builder
            .append_block(self.current_function, &format!("{dir}.done"));

        // Get full variant field lists for byte-offset computation (Enum only)
        let all_variant_fields: Vec<Vec<Idx>> = if pool_tag == Tag::Enum {
            self.pool
                .enum_variants(resolved_ty)
                .into_iter()
                .map(|(_, fields)| fields)
                .collect()
        } else {
            Vec::new()
        };

        let mut cases = Vec::new();
        for (i, fields) in variant_rc_fields.iter().enumerate() {
            if fields.is_empty() {
                continue;
            }
            let block = self
                .builder
                .append_block(self.current_function, &format!("{dir}.v{i}"));
            let tag_const = self.builder.const_int_matching(tag_val, i as u64);
            cases.push((tag_const, block, fields.as_slice(), i));
        }

        let switch_cases: Vec<_> = cases
            .iter()
            .map(|(tag, block, _, _)| (*tag, *block))
            .collect();
        self.builder.switch(tag_val, done_block, &switch_cases);

        for &(_, block, fields, variant_idx) in &cases {
            self.builder.position_at_end(block);

            let offsets = all_variant_fields
                .get(variant_idx)
                .map(|vf| compute_variant_field_offsets(vf, resolved_ty, self))
                .unwrap_or_default();

            for &(field_index, field_type) in fields {
                if matches!(pool_tag, Tag::Result | Tag::Option) {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let struct_idx = 1 + field_index;
                    let field_ptr = self.builder.struct_gep(
                        enum_llvm_ty,
                        alloca,
                        struct_idx,
                        &format!("{dir}.payload.ptr"),
                    );
                    let field_val =
                        self.builder
                            .load(field_llvm_ty, field_ptr, &format!("{dir}.payload"));
                    if is_inc {
                        self.inc_value_rc(field_val, field_type, count);
                    } else {
                        self.dec_value_rc(field_val, field_type);
                    }
                } else if is_boxed_enum_field(self.pool, resolved_ty, field_type) {
                    let payload_ptr =
                        self.builder
                            .struct_gep(enum_llvm_ty, alloca, 1, &format!("{dir}.payload"));
                    let i8_ty = self.builder.i8_type();
                    let byte_off = offsets.get(field_index as usize).copied().unwrap_or(0);
                    let idx = self.builder.const_i64(byte_off as i64);
                    let field_ptr =
                        self.builder
                            .gep(i8_ty, payload_ptr, &[idx], &format!("{dir}.field.ptr"));
                    let ptr_ty = self.builder.ptr_type();
                    let rc_ptr = self
                        .builder
                        .load(ptr_ty, field_ptr, &format!("{dir}.field.rc"));
                    if is_inc {
                        self.call_rc_inc_all(&[rc_ptr], count);
                    } else {
                        let drop_fn = self.get_or_generate_drop_fn(field_type);
                        self.call_rc_dec_all(&[rc_ptr], drop_fn);
                    }
                } else {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let payload_ptr =
                        self.builder
                            .struct_gep(enum_llvm_ty, alloca, 1, &format!("{dir}.payload"));
                    let i8_ty = self.builder.i8_type();
                    let byte_off = offsets.get(field_index as usize).copied().unwrap_or(0);
                    let idx = self.builder.const_i64(byte_off as i64);
                    let field_ptr =
                        self.builder
                            .gep(i8_ty, payload_ptr, &[idx], &format!("{dir}.field.ptr"));
                    let field_val =
                        self.builder
                            .load(field_llvm_ty, field_ptr, &format!("{dir}.field"));
                    if is_inc {
                        self.inc_value_rc(field_val, field_type, count);
                    } else {
                        self.dec_value_rc(field_val, field_type);
                    }
                }
            }

            self.builder.br(done_block);
        }

        self.builder.position_at_end(done_block);
    }

    /// §07.2: Shared niche-aware RC inc/dec for enum values.
    ///
    /// For niche-encoded 2-variant enums: load the niche field, compare
    /// against `niche_value`, skip RC for the niche variant, emit RC for
    /// the data variant's fields.
    fn emit_niche_enum_rc(
        &mut self,
        val: ValueId,
        resolved_ty: Idx,
        pool_tag: Tag,
        variant_rc_fields: &[Vec<(u32, Idx)>],
        encoding: &super::tag_access::TagEncoding,
        is_inc: bool,
        count: u32,
    ) {
        let enum_llvm_ty = self.resolve_type(resolved_ty);
        let prefix = if is_inc { "rc_inc" } else { "rc_dec" };
        let alloca = self
            .builder
            .alloca(enum_llvm_ty, &format!("{prefix}.niche"));
        self.builder.store(val, alloca);

        let niche_idx = encoding.niche_field_index().unwrap();
        let niche_value = encoding.niche_value().unwrap();
        let niche_variant_idx = encoding.niche_variant_idx().unwrap() as usize;

        // Load niche field
        let field_ty = self
            .builder
            .struct_field_type(enum_llvm_ty, niche_idx)
            .unwrap_or_else(|| self.builder.i64_type());
        let field_ptr = self.builder.struct_gep(
            enum_llvm_ty,
            alloca,
            niche_idx,
            &format!("{prefix}.niche.ptr"),
        );
        let field_val = self
            .builder
            .load(field_ty, field_ptr, &format!("{prefix}.niche.val"));

        let is_niche =
            self.niche_is_sentinel(field_val, niche_value, &format!("{prefix}.is_niche"));

        let data_block = self
            .builder
            .append_block(self.current_function, &format!("{prefix}.data"));
        let done_block = self
            .builder
            .append_block(self.current_function, &format!("{prefix}.done"));
        self.builder.cond_br(is_niche, done_block, data_block);

        // Emit RC ops for data variant fields.
        self.builder.position_at_end(data_block);
        let data_variant_idx = usize::from(niche_variant_idx == 0);
        if let Some(data_fields) = variant_rc_fields.get(data_variant_idx) {
            // Option/Result niche layout: payload fields at struct index 0+.
            let is_option_result = matches!(pool_tag, Tag::Option | Tag::Result);
            for &(field_index, field_type) in data_fields {
                let struct_idx = if is_option_result {
                    // Niche layout: no tag field, payload at struct index 0.
                    field_index
                } else {
                    field_index
                };
                let field_llvm_ty = self.resolve_type(field_type);
                let gep = self.builder.struct_gep(
                    enum_llvm_ty,
                    alloca,
                    struct_idx,
                    &format!("{prefix}.f{field_index}.ptr"),
                );
                let fval =
                    self.builder
                        .load(field_llvm_ty, gep, &format!("{prefix}.f{field_index}"));
                if is_inc {
                    self.inc_value_rc(fval, field_type, count);
                } else {
                    self.dec_value_rc(fval, field_type);
                }
            }
        }
        self.builder.br(done_block);

        self.builder.position_at_end(done_block);
    }
}
