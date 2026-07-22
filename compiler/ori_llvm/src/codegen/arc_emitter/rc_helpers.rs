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
use super::drop_enum::{compute_variant_field_offsets, variant_field_offset};
use super::field_walk::FieldWalkOps;
use super::ArcIrEmitter;
use crate::codegen::value_id::{BlockId, LLVMTypeId, ValueId};

/// [`FieldWalkOps`] for an inline tagged-enum variant payload reached via the
/// value-traversal dec path (`dec_value_rc` → `emit_inline_enum_dec`).
///
/// The enum value is stored to `alloca`; the active variant's payload fields
/// are accessed either as typed struct fields at index `1 + field_index`
/// (Option/Result) or via byte-offset GEP into the `[M x i64]` payload area
/// (general enum). `dec_children` is value traversal (`dec_value_rc`).
struct TaggedEnumPayloadOps {
    alloca: ValueId,
    enum_llvm_ty: LLVMTypeId,
    owner_ty: Idx,
    /// Option/Result: typed payload field at struct index `1 + field_index`.
    /// `false` for general enum: byte-offset GEP into the payload array.
    is_option_result: bool,
    /// Byte offsets (general-enum payload only); empty for Option/Result.
    offsets: Vec<u64>,
}

impl FieldWalkOps for TaggedEnumPayloadOps {
    fn load<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        walk: &[(u32, Idx)],
        idx: usize,
    ) -> Option<(ValueId, bool)> {
        let (field_index, field_type) = walk[idx];
        let boxed = is_boxed_enum_field(emitter.pool, self.owner_ty, field_type);
        let field_ptr = if self.is_option_result {
            emitter.builder.struct_gep(
                self.enum_llvm_ty,
                self.alloca,
                1 + field_index,
                "dec.payload.ptr",
            )
        } else {
            let payload_ptr =
                emitter
                    .builder
                    .struct_gep(self.enum_llvm_ty, self.alloca, 1, "dec.payload");
            let i8_ty = emitter.builder.i8_type();
            let byte_off = variant_field_offset(&self.offsets, field_index as usize);
            let off = emitter.builder.const_i64(byte_off as i64);
            emitter
                .builder
                .gep(i8_ty, payload_ptr, &[off], "dec.field.ptr")
        };
        if boxed {
            let ptr_ty = emitter.builder.ptr_type();
            let rc_ptr = emitter.builder.load(ptr_ty, field_ptr, "dec.payload.rc");
            Some((rc_ptr, true))
        } else {
            let field_llvm_ty = emitter.resolve_type(field_type);
            let fv = emitter
                .builder
                .load(field_llvm_ty, field_ptr, "dec.payload");
            Some((fv, false))
        }
    }

    fn dec_boxed<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        rc_ptr: ValueId,
        field_type: Idx,
    ) {
        let drop_fn = emitter.get_or_generate_drop_fn(field_type);
        emitter.call_rc_dec_all(&[rc_ptr], drop_fn);
    }

    fn dec_children<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        field_value: ValueId,
        field_type: Idx,
    ) {
        emitter.dec_value_rc(field_value, field_type);
    }
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Extract the RC-managed data pointer(s) from a value based on its type.
    ///
    /// `ori_rc_inc`/`ori_rc_dec` take raw pointers to RC-allocated heap data.
    /// When a value is an inline aggregate (List, Str, Map, Set, structs with
    /// RC fields), we extract the embedded data pointer(s). For compound types
    /// (struct, tuple, option, result, enum) that contain RC fields, we
    /// recursively extract from each RC'd field.
    ///
    /// | Type | Layout | Data Ptr Field(s) |
    /// |--------|--------------------------------|--------------------|
    /// | List | `{i64, i64, ptr}` | field 2 |
    /// | Set | `{i64, i64, ptr}` | field 2 |
    /// | Str | `{i64, i64, ptr}` (SSO union) | field 2 (SSO→null) |
    /// | Map | `{i64, i64, ptr}` | field 2 |
    /// | Struct | `{field0, field1,...}` | recurse per field |
    /// | Tuple | `{elem0, elem1,...}` | recurse per elem |
    /// | Option | `{i64 tag, T payload}` | recurse into inner |
    /// | Result | `{i64 tag, payload}` | recurse into ok/err|
    /// | Enum | `{i64 tag, payload}` | recurse into fields|
    /// | Function| `{ptr fn, ptr env}` | env ptr (field 1) |
    /// | Other | already a ptr | use directly |
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
                // {i64 tag, T payload} — recurse into inner type at field 1.
                // This flat path is NOT tag-aware, so callers MUST NOT route a
                // possibly-None Option through it: `emit_drop_rc_dec`,
                // `inc_value_rc`/`dec_value_rc`, and the buffer elem-dec glue all
                // route Option through the tag-aware value-traversal
                // (`emit_inline_enum_inc`/`_dec`) before reaching here, so this
                // arm is unreached for a live None (a None's field-1 payload is
                // uninitialized).
                let inner = self.pool.option_inner(resolved);
                if self.classifier.has_managed_ownership_obligation(inner) {
                    if let Some(field) = self.builder.extract_value(val, 1, "rc.opt_inner") {
                        if is_boxed_enum_field(self.pool, resolved, inner) {
                            // Boxed recursive inner: payload is the RC box
                            // pointer — use directly.
                            return vec![field];
                        }
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
                // Why: The closure code pointer carries no heap ownership.
                if let Some(env_ptr) =
                    self.builder
                        .extract_value(val, CLOSURE_FIELD_ENV, "rc.closure_env")
                {
                    vec![env_ptr]
                } else {
                    vec![]
                }
            }
            // INVARIANT: Range stores only scalar start, end, step, and inclusivity fields.
            Tag::Range => vec![],
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
            if self.classifier.has_managed_ownership_obligation(field_ty) {
                let mem_i = self.remap_struct_field(ty, i as u32);
                if let Some(field_val) =
                    self.builder
                        .extract_value(val, mem_i, &format!("rc.field.{i}"))
                {
                    if is_boxed_enum_field(self.pool, ty, field_ty) {
                        // Boxed recursive field: the slot value is already the
                        // RC box pointer — use it directly, do not recurse into
                        // the (inline) child layout.
                        ptrs.push(field_val);
                    } else {
                        ptrs.extend(self.extract_rc_data_ptrs(field_val, field_ty));
                    }
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
            if self.classifier.has_managed_ownership_obligation(elem_ty) {
                let mem_i = self.remap_struct_field(ty, i as u32);
                if let Some(elem_val) =
                    self.builder
                        .extract_value(val, mem_i, &format!("rc.elem.{i}"))
                {
                    if is_boxed_enum_field(self.pool, ty, elem_ty) {
                        // Boxed recursive element: slot value is the RC box
                        // pointer — use directly.
                        ptrs.push(elem_val);
                    } else {
                        ptrs.extend(self.extract_rc_data_ptrs(elem_val, elem_ty));
                    }
                }
            }
        }
        ptrs
    }

    /// Collect per-variant RC field info for an inline enum.
    ///
    /// Returns a vec-of-vecs: `[variant_idx][field_idx] = (field_position, field_type)`.
    /// Empty inner vec means the variant has no RC fields.
    /// Whether a variant-payload position holding `field_ty` inside the tagged
    /// union `owner_ty` needs an RC dec/inc: either the payload's inline type
    /// is RC-bearing, OR the position is a boxed recursive back-edge (a heap
    /// RC box that must be dropped regardless of the inline classification).
    pub(super) fn payload_needs_rc(&self, owner_ty: Idx, field_ty: Idx) -> bool {
        self.classifier.has_managed_ownership_obligation(field_ty)
            || is_boxed_enum_field(self.pool, owner_ty, field_ty)
    }

    fn collect_variant_rc_fields(&self, resolved_ty: Idx, pool_tag: Tag) -> Vec<Vec<(u32, Idx)>> {
        match pool_tag {
            Tag::Result => {
                let ok_ty = self.pool.result_ok(resolved_ty);
                let err_ty = self.pool.result_err(resolved_ty);
                // A boxed recursive back-edge is a heap RC box that ALWAYS
                // needs dec, even when the payload's inline type would not be
                // classified RC-bearing (e.g. a `Value`-shaped recursive struct).
                let ok_fields = if self.payload_needs_rc(resolved_ty, ok_ty) {
                    vec![(0_u32, ok_ty)]
                } else {
                    vec![]
                };
                let err_fields = if self.payload_needs_rc(resolved_ty, err_ty) {
                    vec![(0_u32, err_ty)]
                } else {
                    vec![]
                };
                vec![ok_fields, err_fields]
            }
            Tag::Option => {
                let inner = self.pool.option_inner(resolved_ty);
                let some_fields = if self.payload_needs_rc(resolved_ty, inner) {
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
                            .filter(|(_, ty)| self.payload_needs_rc(resolved_ty, **ty))
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
        self.emit_inline_enum_rc_core(
            val,
            resolved_ty,
            pool_tag,
            super::emitter_utils::RcOperation::Retain { count },
        );
    }

    /// Delegates to shared `emit_inline_enum_rc_core` with dec direction.
    pub(super) fn emit_inline_enum_dec(&mut self, val: ValueId, resolved_ty: Idx, pool_tag: Tag) {
        self.emit_inline_enum_rc_core(
            val,
            resolved_ty,
            pool_tag,
            super::emitter_utils::RcOperation::Release,
        );
    }

    /// Shared core for inline enum RC inc/dec.
    ///
    /// Both directions (inc/dec) share identical tag-switch scaffolding:
    /// alloca → load tag → switch on variants → GEP to payload field.
    /// The only difference is which RC operation is applied to each field.
    fn emit_inline_enum_rc_core(
        &mut self,
        val: ValueId,
        resolved_ty: Idx,
        pool_tag: Tag,
        operation: super::emitter_utils::RcOperation,
    ) {
        let variant_rc_fields = self.collect_variant_rc_fields(resolved_ty, pool_tag);

        if variant_rc_fields.iter().all(Vec::is_empty) {
            return;
        }

        // Tagged-pointer enum — decode tag, dispatch per variant.
        if self.get_tagged_ptr_encoding(resolved_ty).is_some() {
            self.emit_tagged_ptr_enum_rc(val, &variant_rc_fields, operation);
            return;
        }

        // Tagless single-variant enum — struct-like field RC, no tag,
        // no niche, no switch.
        if self.is_tagless_enum(resolved_ty) {
            self.emit_inline_tagless_rc(val, resolved_ty, operation);
            return;
        }

        // Niche-encoded enum — conditional RC.
        if let Some(encoding) = self.get_niche_encoding(resolved_ty) {
            self.emit_niche_enum_rc(
                val,
                resolved_ty,
                pool_tag,
                &variant_rc_fields,
                &encoding,
                operation,
            );
            return;
        }

        let dir = operation.prefix();

        let enum_llvm_ty = self.resolve_type(resolved_ty);
        let alloca = self.builder.alloca(enum_llvm_ty, &format!("{dir}.enum"));
        self.builder.store(val, alloca);

        // Load tag (narrowed type at field 0 —)
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

        let is_option_result = matches!(pool_tag, Tag::Result | Tag::Option);
        for &(_, block, fields, variant_idx) in &cases {
            self.builder.position_at_end(block);

            let offsets = if pool_tag == Tag::Enum {
                // OOB variant = upstream typeck/canon bug; an empty fallback
                // would silently skip the payload RC walk.
                let variant_fields = all_variant_fields.get(variant_idx).unwrap_or_else(|| {
                    unreachable!("RC walk variant {variant_idx} out of bounds for enum type")
                });
                compute_variant_field_offsets(variant_fields, resolved_ty, self)
            } else {
                // Option/Result: typed field access, no byte offsets needed.
                Vec::new()
            };

            // Teardown (dec) walks payload fields in REVERSE declaration order
            // (LIFO) per `drop-trait-proposal.md §Drop and panic`; inc keeps
            // forward order (unobservable for inc). Collect into an owned Vec so
            // the iteration does not borrow `cases`.
            let ordered: Vec<(u32, Idx)> =
                super::emitter_utils::field_rc_walk_order(fields, operation.field_walk_order());

            if let Some(count) = operation.retain_count() {
                self.inc_enum_payload_fields(
                    &ordered,
                    enum_llvm_ty,
                    alloca,
                    resolved_ty,
                    is_option_result,
                    &offsets,
                    count,
                );
            } else {
                // Dec routes through the canonical may-unwind field-walk SSOT so
                // a panicking payload field's user `@drop` still frees the
                // later-walked sibling payload fields via the per-field cleanup
                // pad (matching the struct path).
                let ops = TaggedEnumPayloadOps {
                    alloca,
                    enum_llvm_ty,
                    owner_ty: resolved_ty,
                    is_option_result,
                    offsets,
                };
                self.dec_fields_may_unwind(&ops, &ordered, 0);
            }

            self.builder.br(done_block);
        }

        self.builder.position_at_end(done_block);
    }

    /// Inc the RC children of a tagged-enum variant's payload fields (forward
    /// order; inc has no user `@drop` and no unwind, so order is unobservable).
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors the inline-enum payload access surface; grouping adds indirection"
    )]
    fn inc_enum_payload_fields(
        &mut self,
        ordered: &[(u32, Idx)],
        enum_llvm_ty: LLVMTypeId,
        alloca: ValueId,
        owner_ty: Idx,
        is_option_result: bool,
        offsets: &[u64],
        count: u32,
    ) {
        for &(field_index, field_type) in ordered {
            let boxed = is_boxed_enum_field(self.pool, owner_ty, field_type);
            let field_ptr = if is_option_result {
                self.builder
                    .struct_gep(enum_llvm_ty, alloca, 1 + field_index, "inc.payload.ptr")
            } else {
                let payload_ptr = self
                    .builder
                    .struct_gep(enum_llvm_ty, alloca, 1, "inc.payload");
                let i8_ty = self.builder.i8_type();
                let byte_off = variant_field_offset(offsets, field_index as usize);
                let off = self.builder.const_i64(byte_off as i64);
                self.builder
                    .gep(i8_ty, payload_ptr, &[off], "inc.field.ptr")
            };
            if boxed {
                let ptr_ty = self.builder.ptr_type();
                let rc_ptr = self.builder.load(ptr_ty, field_ptr, "inc.payload.rc");
                self.call_rc_inc_all(&[rc_ptr], count);
            } else {
                let field_llvm_ty = self.resolve_type(field_type);
                let field_val = self.builder.load(field_llvm_ty, field_ptr, "inc.payload");
                self.inc_value_rc(field_val, field_type, count);
            }
        }
    }

    /// Shared niche-aware RC inc/dec for enum values.
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
        operation: super::emitter_utils::RcOperation,
    ) {
        let enum_llvm_ty = self.resolve_type(resolved_ty);
        let prefix = operation.prefix();
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
        let _ = pool_tag; // niche layout has no tag slot — Option/Result + general enum identical
        self.builder.position_at_end(data_block);
        let data_variant_idx = usize::from(niche_variant_idx == 0);
        if let Some(data_fields) = variant_rc_fields.get(data_variant_idx) {
            if let Some(count) = operation.retain_count() {
                // Niche layout: no tag field — payload fields at struct index
                // `field_index`. Inc keeps forward order (unobservable for inc).
                for &(field_index, field_type) in data_fields {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let gep = self.builder.struct_gep(
                        enum_llvm_ty,
                        alloca,
                        field_index,
                        &format!("{prefix}.f{field_index}.ptr"),
                    );
                    let fval =
                        self.builder
                            .load(field_llvm_ty, gep, &format!("{prefix}.f{field_index}"));
                    self.inc_value_rc(fval, field_type, count);
                }
            } else {
                // Dec routes through the canonical may-unwind field-walk SSOT so
                // a panicking payload field's user `@drop` still frees the
                // later-walked sibling payload fields via the per-field cleanup
                // pad (matching every other enum payload path). Reverse-decl
                // (LIFO) teardown order.
                let walk = super::emitter_utils::field_rc_walk_order(
                    data_fields,
                    super::emitter_utils::FieldRcWalkOrder::Teardown,
                );
                let ops = super::drop_enum::NicheEnumPayloadOps {
                    value: alloca,
                    enum_llvm_ty,
                    owner_ty: resolved_ty,
                    value_traversal: true,
                };
                self.dec_fields_may_unwind(&ops, &walk, 0);
            }
        }
        self.builder.br(done_block);

        self.builder.position_at_end(done_block);
    }

    /// RC inc/dec for tagged-pointer encoded enums.
    ///
    /// The encoded value is a single i64. For each pointer-bearing variant,
    /// emit a switch case that decodes the pointer (high 61 bits) and
    /// applies the appropriate RC operation. Unit variants are skipped —
    /// their tag bits identify the variant but carry no payload to count.
    ///
    /// Each variant's RC field list has length 0 (unit) or 1 (single-pointer
    /// payload). The latter is enforced by `can_use_tagged_pointer`.
    fn emit_tagged_ptr_enum_rc(
        &mut self,
        val: ValueId,
        variant_rc_fields: &[Vec<(u32, Idx)>],
        operation: super::emitter_utils::RcOperation,
    ) {
        let dir = operation.prefix();

        // Decode the tag bits from the encoded value.
        let tag_val = self.tagged_ptr_decode_tag(val, &format!("{dir}.tag"));

        let done_block = self
            .builder
            .append_block(self.current_function, &format!("{dir}.done"));

        // Emit a per-variant block for every pointer-bearing variant.
        // Unit variants share the default → done path.
        let mut cases: Vec<(ValueId, BlockId, Idx)> = Vec::new();
        for (i, fields) in variant_rc_fields.iter().enumerate() {
            if fields.is_empty() {
                continue;
            }
            debug_assert!(
                fields.len() == 1,
                "tagged-pointer variant must have at most one RC field"
            );
            let (_, field_type) = fields[0];
            let block = self
                .builder
                .append_block(self.current_function, &format!("{dir}.tp.v{i}"));
            // Variant index is bounded by 8 (3-bit tag), so the
            // `usize → u64` widening is exact.
            let tag_const = self.builder.const_int_matching(tag_val, i as u64);
            cases.push((tag_const, block, field_type));
        }

        // No pointer-bearing variants → nothing to do.
        if cases.is_empty() {
            self.builder.br(done_block);
            self.builder.position_at_end(done_block);
            return;
        }

        let switch_cases: Vec<(ValueId, BlockId)> =
            cases.iter().map(|(t, b, _)| (*t, *b)).collect();
        self.builder.switch(tag_val, done_block, &switch_cases);

        // Per-variant: decode pointer, apply RC op, jump to done.
        for &(_, block, field_type) in &cases {
            self.builder.position_at_end(block);
            let ptr_val = self.tagged_ptr_decode_ptr(val, &format!("{dir}.tp.ptr"));
            if let Some(count) = operation.retain_count() {
                self.inc_value_rc(ptr_val, field_type, count);
            } else {
                self.dec_value_rc(ptr_val, field_type);
            }
            self.builder.br(done_block);
        }

        self.builder.position_at_end(done_block);
    }
}
