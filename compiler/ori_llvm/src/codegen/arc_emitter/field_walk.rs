//! Canonical may-unwind RC field-walk SSOT.
//!
//! One teardown-walk skeleton shared by every per-field RC-dec site (struct /
//! tuple inline aggregate, heap drop-fn struct/enum, inline tagged-enum payload,
//! Option/Result payload, tagless-enum payload). The skeleton threads the
//! per-field may-unwind `invoke` plus a cleanup landing pad (drain remaining
//! siblings then `resume`) uniformly so a panic in one field's user `@drop`
//! still frees the later-walked siblings.
//!
//! The divergent part — how a field at a given cursor is ACCESSED (inline
//! `extract_value`, heap `struct_gep`, byte-offset `gep`) and DEC'd (value
//! traversal vs drop-fn dispatch) — is parameterized by the [`FieldWalkOps`]
//! trait. The duplicated part — the cleanup-pad control flow — lives ONLY here.
//!
//! Field order is the caller's responsibility: walks pass `walk` already in
//! reverse-declaration (LIFO) teardown order per
//! `emitter_utils::field_rc_walk_order`.

use ori_types::Idx;

use super::ArcIrEmitter;
use crate::codegen::value_id::ValueId;

/// Per-field access + dec strategy for a may-unwind field walk.
///
/// Implementors are Copy config carriers (base value/ptr, owner `Idx`, LLVM
/// type handles); all LLVM emission goes through the `&mut ArcIrEmitter`.
pub(super) trait FieldWalkOps {
    /// Load the field at `walk[idx]` into an LLVM value.
    ///
    /// Returns `(field_value, is_boxed_recursive)`, or `None` when the field
    /// slot is absent / zero-sized (the walk skips it). A `is_boxed_recursive`
    /// field holds an RC box pointer directly — it is dec'd via
    /// [`Self::dec_boxed`], never carries an inline `@drop`.
    fn load<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        walk: &[(u32, Idx)],
        idx: usize,
    ) -> Option<(ValueId, bool)>;

    /// Dec a boxed-recursive field's RC pointer through the field type's drop
    /// function (the `@drop` lives inside `_ori_drop$<field_ty>`).
    fn dec_boxed<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        rc_ptr: ValueId,
        field_type: Idx,
    );

    /// Resolve the boxed-recursive field's drop function pointer WITHOUT
    /// emitting the dec call. Used by the may-unwind walk to route the dec
    /// through `invoke ori_rc_dec_unwind` when the field type's drop tree may
    /// unwind, so the cleanup pad still drains later-walked siblings.
    fn boxed_drop_fn<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        field_type: Idx,
    ) -> ValueId {
        emitter.get_or_generate_drop_fn(field_type)
    }

    /// Dec the RC children of an inline field VALUE, AFTER its own user
    /// `@drop` has run (or been `invoke`d). For value-traversal sites this is
    /// `dec_value_rc`; for heap drop-fn sites this is `emit_drop_rc_dec`.
    fn dec_children<'scx: 'ctx, 'ctx>(
        &self,
        emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
        field_value: ValueId,
        field_type: Idx,
    );
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Walk `walk[start..]` dropping each RC field with PER-FIELD
    /// sibling-continue: when a field's own user `@drop` may unwind (Itanium),
    /// it is `invoke`d so a panic routes to a cleanup pad that still drops the
    /// panicking field's children + every remaining sibling, then `resume`s the
    /// foreign exception. A field whose `@drop` cannot unwind takes the plain
    /// path.
    ///
    /// This is the canonical SSOT consumed by every per-field RC-dec site via a
    /// [`FieldWalkOps`] access strategy.
    pub(super) fn dec_fields_may_unwind<O: FieldWalkOps>(
        &mut self,
        ops: &O,
        walk: &[(u32, Idx)],
        start: usize,
    ) {
        let itanium = self.builder.eh_model() == crate::codegen::eh_model::EhModel::Itanium;
        let mut j = start;
        while j < walk.len() {
            let (_, field_ty) = walk[j];
            let Some((fv, is_boxed)) = ops.load(self, walk, j) else {
                j += 1;
                continue;
            };
            if is_boxed {
                // A boxed-recursive child is dec'd through its drop fn via
                // `ori_rc_dec`. When that drop tree may unwind (a user `@drop`
                // inside the boxed node panics), route the dec through
                // `invoke ori_rc_dec_unwind` so the panic lands in a cleanup pad
                // that still drains the later-walked siblings, then `resume`s —
                // matching the inline-value per-field cleanup-continue below. A
                // plain `ori_rc_dec` boundary would abort the foreign exception.
                let boxed_unwinds = self.drop_may_unwind(field_ty) && itanium;
                if boxed_unwinds {
                    let drop_fn = ops.boxed_drop_fn(self, field_ty);
                    let cont = self.builder.append_block(self.current_function, "fld.cont");
                    let cleanup = self
                        .builder
                        .append_block(self.current_function, "fld.cleanup");
                    self.invoke_rc_dec_unwind(fv, drop_fn, cont, cleanup);
                    // Cleanup pad: drain every remaining sibling (plain — a
                    // nested panic aborts via the drop-cleanup-depth guard),
                    // then resume.
                    self.builder.position_at_end(cleanup);
                    let personality = self.builder.runtime_fn("ori_eh_personality");
                    let lp = self.builder.landingpad(personality, true, "fld.lp");
                    let enter = self.builder.runtime_fn("ori_drop_cleanup_enter");
                    self.builder.call(enter, &[], "");
                    self.dec_fields_plain(ops, walk, j + 1);
                    let exit = self.builder.runtime_fn("ori_drop_cleanup_exit");
                    self.builder.call(exit, &[], "");
                    self.builder.resume(lp);
                    // Normal continuation: advance.
                    self.builder.position_at_end(cont);
                    j += 1;
                    continue;
                }
                ops.dec_boxed(self, fv, field_ty);
                j += 1;
                continue;
            }
            let unwinds = self.user_drop_method(field_ty).is_some()
                && self.drop_may_unwind(field_ty)
                && itanium;
            if unwinds {
                let cont = self.builder.append_block(self.current_function, "fld.cont");
                let cleanup = self
                    .builder
                    .append_block(self.current_function, "fld.cleanup");
                if self.invoke_user_drop_for_inline_value(field_ty, fv, cont, cleanup) {
                    // Cleanup pad: drop the panicking field's own RC children +
                    // every remaining sibling field (plain — a nested panic
                    // aborts via the drop-cleanup-depth guard), then resume.
                    self.builder.position_at_end(cleanup);
                    let personality = self.builder.runtime_fn("ori_eh_personality");
                    let lp = self.builder.landingpad(personality, true, "fld.lp");
                    let enter = self.builder.runtime_fn("ori_drop_cleanup_enter");
                    self.builder.call(enter, &[], "");
                    ops.dec_children(self, fv, field_ty);
                    self.dec_fields_plain(ops, walk, j + 1);
                    let exit = self.builder.runtime_fn("ori_drop_cleanup_exit");
                    self.builder.call(exit, &[], "");
                    self.builder.resume(lp);
                    // Normal continuation: this field's children, then advance.
                    self.builder.position_at_end(cont);
                    ops.dec_children(self, fv, field_ty);
                    j += 1;
                    continue;
                }
            }
            // Plain field: run its user `@drop` (if any, non-unwinding) before
            // dropping its RC children.
            self.emit_user_drop_for_inline_value(field_ty, fv);
            ops.dec_children(self, fv, field_ty);
            j += 1;
        }
    }

    /// Plain (non-`invoke`) drain of `walk[start..]` — used inside a cleanup pad
    /// where a further (nested) panic must abort via the depth guard, not
    /// re-enter sibling-continue.
    fn dec_fields_plain<O: FieldWalkOps>(&mut self, ops: &O, walk: &[(u32, Idx)], start: usize) {
        for idx in start..walk.len() {
            let (_, field_ty) = walk[idx];
            let Some((fv, is_boxed)) = ops.load(self, walk, idx) else {
                continue;
            };
            if is_boxed {
                ops.dec_boxed(self, fv, field_ty);
            } else {
                self.emit_user_drop_for_inline_value(field_ty, fv);
                ops.dec_children(self, fv, field_ty);
            }
        }
    }
}
