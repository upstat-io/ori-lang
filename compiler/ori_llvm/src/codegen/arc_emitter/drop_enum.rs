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
//!   `min_tag_width` (i8 for ≤256 variants,); fields at byte offsets
//!   within the payload array

use ori_types::{Idx, Pool, Tag};

use super::context::is_boxed_enum_field;
use super::field_walk::FieldWalkOps;
use super::ArcIrEmitter;
use crate::codegen::type_info::TypeLayoutResolver;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

/// [`FieldWalkOps`] for a HEAP enum variant payload reached via the drop-fn
/// path (`emit_drop_enum` → `_ori_drop$<Enum>`, e.g. a boxed-recursive enum
/// node).
///
/// `data_ptr` points at the heap-allocated enum. Option/Result payloads are
/// typed fields at struct index `1 + field_index`; general-enum payloads live
/// at byte offsets within the `[M x i64]` payload area. `dec_children` is the
/// drop-fn dispatch (`emit_drop_rc_dec`), which dec's the inline field VALUE's
/// own RC children after the field's user `@drop` has run via the canonical
/// walk.
struct HeapEnumPayloadOps {
    data_ptr: ValueId,
    enum_llvm_ty: LLVMTypeId,
    owner_ty: Idx,
    /// Option/Result: typed payload field at struct index `1 + field_index`.
    /// `false` for general enum: byte-offset GEP into the payload array.
    is_option_result: bool,
    /// Byte offsets (general-enum payload only); empty for Option/Result.
    offsets: Vec<u64>,
}

/// [`FieldWalkOps`] for a NICHE-encoded enum's data-variant payload. The niche
/// layout has NO explicit tag field — the data variant's payload fields live at
/// struct index `field_index` directly (the niche discriminant is encoded in an
/// invalid bit pattern of one payload field, not a separate slot).
///
/// Shared by the heap drop-fn niche path (`emit_drop_enum_niche` →
/// `_ori_drop$<Enum>`, `value` = `data_ptr`, `dec_children` = `emit_drop_rc_dec`)
/// and the inline value-traversal niche path (`emit_niche_enum_rc` dec branch,
/// `value` = `alloca`, `dec_children` = `dec_value_rc`). Routing both through
/// [`ArcIrEmitter::dec_fields_may_unwind`] threads the per-field cleanup pad so a
/// panicking payload field's `@drop` still frees the later-walked siblings —
/// matching every other enum payload path.
pub(super) struct NicheEnumPayloadOps {
    /// Pointer to the niche-encoded enum value (heap `data_ptr` or inline alloca).
    pub(super) value: ValueId,
    pub(super) enum_llvm_ty: LLVMTypeId,
    pub(super) owner_ty: Idx,
    /// `true` for the value-traversal dec path (`dec_value_rc`); `false` for the
    /// heap drop-fn path (`emit_drop_rc_dec`).
    pub(super) value_traversal: bool,
}

impl FieldWalkOps for NicheEnumPayloadOps {
    fn load<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        walk: &[(u32, Idx)],
        idx: usize,
    ) -> Option<(ValueId, bool)> {
        let (field_index, field_type) = walk[idx];
        let boxed = is_boxed_enum_field(emitter.pool, self.owner_ty, field_type);
        // Niche layout: no tag field — payload starts at struct index 0, so the
        // field lives at struct index `field_index`.
        let field_ptr = emitter.builder.struct_gep(
            self.enum_llvm_ty,
            self.value,
            field_index,
            &format!("niche.f{field_index}.ptr"),
        );
        if boxed {
            let ptr_ty = emitter.builder.ptr_type();
            let rc_ptr =
                emitter
                    .builder
                    .load(ptr_ty, field_ptr, &format!("niche.f{field_index}.rc"));
            Some((rc_ptr, true))
        } else {
            let field_llvm_ty = emitter.resolve_type(field_type);
            let fv =
                emitter
                    .builder
                    .load(field_llvm_ty, field_ptr, &format!("niche.f{field_index}"));
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
        emitter.emit_drop_rc_dec_with_fn(rc_ptr, drop_fn);
    }

    fn dec_children<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        field_value: ValueId,
        field_type: Idx,
    ) {
        if self.value_traversal {
            emitter.dec_value_rc(field_value, field_type);
        } else {
            emitter.emit_drop_rc_dec(field_value, field_type);
        }
    }
}

impl FieldWalkOps for HeapEnumPayloadOps {
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
                self.data_ptr,
                1 + field_index,
                "payload.ptr",
            )
        } else {
            let payload_ptr =
                emitter
                    .builder
                    .struct_gep(self.enum_llvm_ty, self.data_ptr, 1, "payload");
            let i8_ty = emitter.builder.i8_type();
            let byte_off = variant_field_offset(&self.offsets, field_index as usize);
            let off = emitter.builder.const_i64(byte_off as i64);
            emitter
                .builder
                .gep(i8_ty, payload_ptr, &[off], &format!("f{field_index}.ptr"))
        };
        if boxed {
            let ptr_ty = emitter.builder.ptr_type();
            let rc_ptr = emitter
                .builder
                .load(ptr_ty, field_ptr, &format!("f{field_index}.rc"));
            Some((rc_ptr, true))
        } else {
            let field_llvm_ty = emitter.resolve_type(field_type);
            let fv = emitter
                .builder
                .load(field_llvm_ty, field_ptr, &format!("f{field_index}"));
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
        emitter.emit_drop_rc_dec_with_fn(rc_ptr, drop_fn);
    }

    fn dec_children<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        field_value: ValueId,
        field_type: Idx,
    ) {
        emitter.emit_drop_rc_dec(field_value, field_type);
    }
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit drop body for an enum type (switch on tag, per-variant cleanup).
    ///
    /// `user_drop` carries the user `@drop` `FnSym` when the type implements
    /// the `Drop` trait — codegen invokes `@drop` BEFORE the
    /// discriminant-switch + per-variant field walk per
    /// `drop-trait-proposal.md` Execution Timing section. Without this AUGMENT
    /// wiring, enum-shaped Drop types (e.g.,
    /// `type EventLog = A(payload) | B(...)` + `impl EventLog: Drop`)
    /// would silently bypass the user `@drop` body during refcount-zero
    /// cleanup.
    ///
    /// Delegates per-variant cleanup to [`Self::emit_variant_burden_walk`],
    /// then frees the heap allocation and returns. The walk leaves the builder
    /// positioned at the convergence block; finalization (free + ret) happens
    /// here.
    pub(super) fn emit_drop_enum(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        ty: Idx,
        variants: &[Vec<(u32, Idx)>],
        _user_drop: Option<ori_registry::burden::FnSym>,
    ) {
        // Recoverable path: the enum's OWN `@drop` may unwind (a panicking
        // user `@drop`). Mirror the struct path (`emit_drop_fields`): `invoke`
        // the own `@drop` so a panic routes to a cleanup pad that still runs the
        // variant burden walk + free (so the variant payload is freed — no
        // leak), then `resume`s the foreign exception. Itanium only; SEH funclet
        // EH for the drop-fn cleanup pad is anchored in the recoverable-unwind
        // deliverable.
        let own_drop_unwinds = self.user_drop_method(ty).is_some()
            && self.drop_may_unwind(ty)
            && self.builder.eh_model() == crate::codegen::eh_model::EhModel::Itanium;

        if own_drop_unwinds {
            let cont_bb = self
                .builder
                .append_block(self.current_function, "edrop.cont");
            let cleanup_bb = self
                .builder
                .append_block(self.current_function, "edrop.cleanup");
            if self.invoke_user_drop_via_pointer(ty, data_ptr, cont_bb, cleanup_bb) {
                // Normal continuation: variant burden walk + free + ret.
                self.builder.position_at_end(cont_bb);
                self.emit_variant_burden_walk(func_id, data_ptr, ty, variants);
                self.emit_drop_rc_free(data_ptr, ty);
                self.builder.ret_void();

                // Cleanup pad: variant burden walk + free still run (no leak),
                // then re-raise the foreign exception to the caller's pad.
                self.builder.position_at_end(cleanup_bb);
                let personality = self.builder.runtime_fn("ori_eh_personality");
                let lp = self.builder.landingpad(personality, true, "edrop.lp");
                let enter = self.builder.runtime_fn("ori_drop_cleanup_enter");
                self.builder.call(enter, &[], "");
                self.emit_variant_burden_walk(func_id, data_ptr, ty, variants);
                self.emit_drop_rc_free(data_ptr, ty);
                let exit = self.builder.runtime_fn("ori_drop_cleanup_exit");
                self.builder.call(exit, &[], "");
                self.builder.resume(lp);
                return;
            }
            // No user `@drop` resolved after all — fall through to plain path.
        }

        // Plain path: run the user `@drop` first when the enum implements
        // `Drop` (non-unwinding, or SEH). The helper self-gates on the canonical
        // method map (the upstream burden `FnSym` is not threaded onto this
        // path), so call it unconditionally.
        self.emit_user_drop_via_pointer(ty, data_ptr);
        self.emit_variant_burden_walk(func_id, data_ptr, ty, variants);
        self.emit_drop_rc_free(data_ptr, ty);
        self.builder.ret_void();
    }

    /// Per-variant burden walk SSOT (3-encoding dispatch: tagged-pointer,
    /// niche-encoded, explicit-tag).
    ///
    /// Shared by [`Self::emit_drop_enum`] (RC=0 free path) and the
    /// `BurdenDecVariant` codegen arm at `instr_dispatch.rs` (`SetTag`
    /// pre-drop path). Per AIMS rule TF-15a + RL-10, `SetTag` invalidates
    /// ALL payload fields of the OLD variant — same per-variant walk shape
    /// the drop path uses.
    ///
    /// Exit invariant: builder positioned at the convergence (`drop_done`)
    /// block; NO free, NO ret. Callers own finalization.
    pub(super) fn emit_variant_burden_walk(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        ty: Idx,
        variants: &[Vec<(u32, Idx)>],
    ) {
        // Tagged-pointer enum drop — load the encoded i64, decode
        // the tag, dispatch to per-variant pointer dec. Tagged-pointer enums
        // are 8 bytes, so the heap-allocated case is rare (the encoding
        // typically lives inline in a parent struct), but the drop path
        // must still be correct when it does fire.
        if self.get_tagged_ptr_encoding(ty).is_some() {
            self.emit_drop_enum_tagged_ptr(func_id, data_ptr, variants);
            return;
        }

        // Tagless single-variant enum drop — struct-like field walk,
        // no tag, no niche, no switch.
        if self.is_tagless_enum(ty) {
            self.emit_drop_tagless(ty, data_ptr);
            return;
        }

        // Niche-encoded enum drop — conditional skip instead of switch.
        if let Some(encoding) = self.get_niche_encoding(ty) {
            self.emit_drop_enum_niche(func_id, data_ptr, ty, variants, &encoding);
            return;
        }

        let enum_llvm_ty = self.resolve_type(ty);

        // Load tag (narrowed type at field 0 — discriminant narrowing)
        let Some(tag_ty) = self.builder.struct_field_type(enum_llvm_ty, 0) else {
            self.builder
                .record_codegen_error_with_msg("enum drop layout is missing its tag field");
            return;
        };
        let tag_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, data_ptr, 0, "tag.ptr");
        let tag_val = self.builder.load(tag_ty, tag_ptr, "tag");

        // Convergence block: caller positions here on exit.
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

            self.emit_drop_enum_variant_fields(
                data_ptr,
                ty,
                variant_fields,
                case_variant_indices[idx],
                matches!(pool_tag, Tag::Option | Tag::Result),
            );

            self.builder.br(drop_done);
        }

        // drop.done: position only; caller owns free + ret.
        self.builder.position_at_end(drop_done);
    }

    /// Emit RC dec for the payload fields of a heap enum variant via the
    /// canonical may-unwind field-walk SSOT.
    ///
    /// Option/Result payloads are typed fields at struct index `1 +
    /// field_index`; general-enum payloads live at byte offsets within the
    /// `[M x i64]` payload area at struct field 1. `variant_idx` identifies
    /// which variant's field types drive the byte-offset computation. Routing
    /// through the SSOT runs each inline payload field's user `@drop` (in
    /// reverse-declaration teardown order) and threads the per-field cleanup
    /// pad so a panicking field's `@drop` still frees the later-walked siblings.
    pub(super) fn emit_drop_enum_variant_fields(
        &mut self,
        data_ptr: ValueId,
        ty: Idx,
        rc_fields: &[(u32, Idx)],
        variant_idx: usize,
        is_option_result: bool,
    ) {
        let enum_llvm_ty = self.resolve_type(ty);
        let (resolved_ty, resolved_tag) = resolve_type_through_aliases(ty, self.pool);

        let offsets = if is_option_result {
            // Option/Result: typed field access, no byte offsets needed.
            Vec::new()
        } else if resolved_tag == Tag::Enum {
            let all_variants = self.pool.enum_variants(resolved_ty);
            // OOB variant = upstream typeck/canon bug; an empty fallback would
            // silently skip dropping payload fields (leak).
            let Some((_, variant_field_types)) = all_variants.into_iter().nth(variant_idx) else {
                unreachable!("drop glue variant {variant_idx} out of bounds for enum type")
            };
            compute_variant_field_offsets(&variant_field_types, resolved_ty, self)
        } else {
            // Fallback: non-Enum tag for a general enum — treat field_index as
            // an i64 slot index (offset = field_index * 8). Build a dense vec
            // indexed by field_index so `HeapEnumPayloadOps::load` resolves the
            // right offset per field.
            tracing::warn!(
                ?resolved_tag,
                "drop_gen: non-Enum tag for general enum — using slot access"
            );
            let max_fi = rc_fields.iter().map(|&(fi, _)| fi).max().unwrap_or(0);
            (0..=max_fi).map(|fi| u64::from(fi) * 8).collect()
        };

        let walk = super::emitter_utils::field_rc_walk_order(
            rc_fields,
            super::emitter_utils::FieldRcWalkOrder::Teardown,
        );
        let ops = HeapEnumPayloadOps {
            data_ptr,
            enum_llvm_ty,
            owner_ty: resolved_ty,
            is_option_result,
            offsets,
        };
        self.dec_fields_may_unwind(&ops, &walk, 0);
    }
    /// Emit drop body for a niche-encoded enum.
    ///
    /// For 2-variant niche enums (Option/Result): load the niche field,
    /// compare against `niche_value`, skip drop for the niche variant (no
    /// payload to clean up), and drop fields for the data variant.
    fn emit_drop_enum_niche(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        ty: Idx,
        variants: &[Vec<(u32, Idx)>],
        encoding: &super::tag_access::TagEncoding,
    ) {
        let enum_llvm_ty = self.resolve_type(ty);
        let Some((niche_idx, niche_value, niche_variant_idx)) = encoding.niche_fields() else {
            self.builder
                .record_codegen_error_with_msg("niche drop requires a niche tag encoding");
            return;
        };
        let niche_variant_idx = niche_variant_idx as usize;

        // Load niche field
        let Some(field_ty) = self.builder.struct_field_type(enum_llvm_ty, niche_idx) else {
            self.builder
                .record_codegen_error_with_msg("niche drop layout is missing its sentinel field");
            return;
        };
        let field_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, data_ptr, niche_idx, "niche.ptr");
        let field_val = self.builder.load(field_ty, field_ptr, "niche.val");

        let is_niche = self.niche_is_sentinel(field_val, niche_value, "is.niche");

        let drop_data = self.builder.append_block(func_id, "drop.data");
        let drop_done = self.builder.append_block(func_id, "drop.done");

        // Niche variant → skip to done. Data variant → drop fields.
        self.builder.cond_br(is_niche, drop_done, drop_data);

        // Emit data variant field drops via the canonical may-unwind field-walk
        // SSOT so a panicking payload field's user `@drop` still frees the
        // later-walked siblings (matching every other enum payload path). The
        // niche layout has no tag field; `NicheEnumPayloadOps` accesses payload
        // fields at struct index `field_index`.
        self.builder.position_at_end(drop_data);
        let data_variant_idx = usize::from(niche_variant_idx == 0);
        if let Some(data_fields) = variants.get(data_variant_idx) {
            let (resolved_ty, _) = resolve_type_through_aliases(ty, self.pool);
            let walk = super::emitter_utils::field_rc_walk_order(
                data_fields,
                super::emitter_utils::FieldRcWalkOrder::Teardown,
            );
            let ops = NicheEnumPayloadOps {
                value: data_ptr,
                enum_llvm_ty,
                owner_ty: resolved_ty,
                value_traversal: false,
            };
            self.dec_fields_may_unwind(&ops, &walk, 0);
        }
        self.builder.br(drop_done);

        // done: position only; caller owns free + ret.
        self.builder.position_at_end(drop_done);
    }

    /// Emit drop body for a tagged-pointer encoded enum.
    ///
    /// Loads the 8-byte encoded value from `data_ptr`, decodes the tag bits,
    /// and dispatches to per-variant pointer dec via a switch. Pointer-bearing
    /// variants decode the high 61 bits and call the appropriate `drop_fn`;
    /// unit variants flow through the default arm to `drop.done` (no payload
    /// to clean up).
    ///
    /// `variants[i]` contains the RC-needing fields for variant `i`. For
    /// tagged-pointer enums each non-empty variant has exactly one entry —
    /// enforced by `can_use_tagged_pointer`.
    fn emit_drop_enum_tagged_ptr(
        &mut self,
        func_id: FunctionId,
        data_ptr: ValueId,
        variants: &[Vec<(u32, Idx)>],
    ) {
        // Tagged-pointer enums are stored as a single i64 slot.
        let i64_ty = self.builder.i64_type();
        let encoded = self.builder.load(i64_ty, data_ptr, "tagged.encoded");
        let tag_val = self.tagged_ptr_decode_tag(encoded, "tagged.tag");

        let drop_done = self.builder.append_block(func_id, "drop.done");

        // Build per-variant blocks for pointer-bearing variants only.
        let mut cases: Vec<(ValueId, crate::codegen::value_id::BlockId, Idx)> = Vec::new();
        for (i, variant_fields) in variants.iter().enumerate() {
            if variant_fields.is_empty() {
                continue;
            }
            debug_assert!(
                variant_fields.len() == 1,
                "tagged-pointer variant must have at most one RC field"
            );
            let (_, field_type) = variant_fields[0];
            let block = self
                .builder
                .append_block(func_id, &format!("tagged.v{i}.drop"));
            // Variant index is bounded by 8 (3-bit tag), so the
            // `usize → u64` widening is exact.
            let tag_const = self.builder.const_int_matching(tag_val, i as u64);
            cases.push((tag_const, block, field_type));
        }

        if cases.is_empty() {
            // No pointer-bearing variants — straight to free.
            self.builder.br(drop_done);
        } else {
            let switch_cases: Vec<(ValueId, crate::codegen::value_id::BlockId)> =
                cases.iter().map(|(t, b, _)| (*t, *b)).collect();
            self.builder.switch(tag_val, drop_done, &switch_cases);

            for &(_, block, field_type) in &cases {
                self.builder.position_at_end(block);
                let ptr = self.tagged_ptr_decode_ptr(encoded, "tagged.ptr");
                self.emit_drop_rc_dec(ptr, field_type);
                self.builder.br(drop_done);
            }
        }

        // done: position only; caller owns free + ret.
        self.builder.position_at_end(drop_done);
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
        // Unit/Never fields are zero-sized — they don't occupy
        // payload space. Skip them so offsets match the LLVM layout.
        let resolved_ft = emitter.pool.resolve_fully(field_ty);
        if matches!(emitter.pool.tag(resolved_ft), Tag::Unit | Tag::Never) {
            continue;
        }
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

/// Look up a variant field's byte offset from a pre-sized offset table.
///
/// The table from [`compute_variant_field_offsets`] carries one entry per
/// variant field — an out-of-range index is an upstream invariant break.
/// A silent `0` fallback would GEP the first field's address (wrong-address
/// store/load corruption), so the lookup fails loud instead.
pub(super) fn variant_field_offset(offsets: &[u64], field_index: usize) -> u64 {
    offsets.get(field_index).copied().unwrap_or_else(|| {
        unreachable!(
            "variant field offset table pre-sized for fields: index {field_index} \
             out of range for {} offsets",
            offsets.len()
        )
    })
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
