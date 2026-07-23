//! Closure, inline-enum, iterator, and runtime RC calls.

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::CLOSURE_FIELD_ENV;

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Inc a closure (`{fn_ptr, env_ptr}`).
    ///
    /// Extract `env_ptr` (field 1), null-check (zero-capture closures have
    /// null env), then call `ori_rc_inc` on the non-null env.
    pub(super) fn emit_rc_inc_closure(&mut self, val: super::ValueId, count: u32) {
        let func_id = self.builder.runtime_fn("ori_rc_inc");

        let Some(env_ptr) = self
            .builder
            .extract_value(val, CLOSURE_FIELD_ENV, "rc_inc.env")
        else {
            return;
        };

        if self.builder.is_const_null_ptr(env_ptr) {
            return;
        }

        let is_null = self.builder.is_null_ptr(env_ptr, "rc_inc.null");
        let do_inc = self
            .builder
            .append_block(self.current_function, "rc_inc.do");
        let skip = self
            .builder
            .append_block(self.current_function, "rc_inc.skip");
        self.builder.cond_br(is_null, skip, do_inc);

        self.builder.position_at_end(do_inc);
        for _ in 0..count {
            self.emit_rt_call(func_id, &[env_ptr], "");
        }
        self.builder.br(skip);

        self.builder.position_at_end(skip);
    }

    /// Dec a closure (`{fn_ptr, env_ptr}`).
    ///
    /// Extract `env_ptr`, null-check, then load the drop function pointer
    /// from the env header and call `ori_rc_dec(env_ptr, drop_fn)`.
    pub(super) fn emit_rc_dec_closure(&mut self, val: super::ValueId) {
        let Some(env_ptr) = self
            .builder
            .extract_value(val, CLOSURE_FIELD_ENV, "rc_dec.env")
        else {
            return;
        };

        if self.builder.is_const_null_ptr(env_ptr) {
            return;
        }

        let is_null = self.builder.is_null_ptr(env_ptr, "rc_dec.null");
        let do_dec = self
            .builder
            .append_block(self.current_function, "rc_dec.do");
        let skip = self
            .builder
            .append_block(self.current_function, "rc_dec.skip");
        self.builder.cond_br(is_null, skip, do_dec);

        self.builder.position_at_end(do_dec);
        let ptr_ty = self.builder.ptr_type();
        let drop_fn = self.builder.load(ptr_ty, env_ptr, "rc_dec.drop_fn");
        let func_id = self.builder.runtime_fn("ori_rc_dec");
        self.emit_rt_call(func_id, &[env_ptr, drop_fn], "");
        self.builder.br(skip);

        self.builder.position_at_end(skip);
    }

    // Inline enums

    /// Retains the active managed fields of an inline enum.
    ///
    /// Inline enums have no container refcount; each managed payload field
    /// carries its own ownership credit.
    pub(super) fn emit_rc_inc_inline_enum(
        &mut self,
        var: ArcVarId,
        count: u32,
        func: &ArcFunction,
    ) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let resolved = self.pool.resolve_fully(ty);
        let pool_tag = self.pool.tag(resolved);
        self.emit_inline_enum_inc(val, resolved, pool_tag, count);
    }

    /// Release the active managed payload fields of an inline enum.
    pub(super) fn emit_rc_dec_inline_enum(&mut self, var: ArcVarId, func: &ArcFunction) {
        let val = self.var(var);
        let ty = func.var_type(var);
        let resolved = self.pool.resolve_fully(ty);
        let pool_tag = self.pool.tag(resolved);
        self.emit_inline_enum_dec(val, resolved, pool_tag);
    }

    // Iterators

    /// Diagnose, but do not execute, a retain on a uniquely owned iterator.
    ///
    /// Iterator boxes have no RC header, so a runtime retain would corrupt memory.
    #[expect(
        clippy::unused_self,
        reason = "part of the strategy-dispatch API on ArcIrEmitter — called by emit_rc_inc() alongside every other emit_rc_inc_<strategy> method"
    )]
    pub(super) fn emit_rc_inc_iterator(&mut self, var: ArcVarId, count: u32) {
        tracing::trace!(
            var = var.raw(),
            count,
            "RcInc on iterator handle — no-op (iterators are move-only)"
        );
    }

    /// Release an iterator through its box-aware runtime drop operation.
    pub(super) fn emit_rc_dec_iterator(&mut self, var: ArcVarId) {
        let val = self.var(var);
        self.call_iter_drop(val);
    }

    /// Call `ori_iter_drop(ptr)` — frees Box-allocated iterator state.
    pub(super) fn call_iter_drop(&mut self, ptr: super::ValueId) {
        let func_id = self.builder.runtime_fn("ori_iter_drop");
        self.emit_rt_call(func_id, &[ptr], "");
    }

    // Runtime calls

    /// Call `ori_rc_inc(ptr)` for each pointer, `count` times.
    pub(super) fn call_rc_inc_all(&mut self, ptrs: &[super::ValueId], count: u32) {
        if ptrs.is_empty() {
            return;
        }
        let func_id = self.builder.runtime_fn("ori_rc_inc");
        for &ptr in ptrs {
            for _ in 0..count {
                self.emit_rt_call(func_id, &[ptr], "");
            }
        }
    }

    /// Call `ori_list_rc_inc(data, cap)` — slice-aware RC inc for list/set.
    ///
    /// Unlike `call_rc_inc_all` which calls `ori_rc_inc(data)` directly,
    /// this passes `cap` so the runtime can check `is_slice_cap(cap)` and
    /// find the original allocation's RC header for slices.
    pub(super) fn call_list_rc_inc(
        &mut self,
        data: super::ValueId,
        cap: super::ValueId,
        count: u32,
    ) {
        let func_id = self.builder.runtime_fn("ori_list_rc_inc");
        for _ in 0..count {
            self.emit_rt_call(func_id, &[data, cap], "");
        }
    }

    /// Call `ori_rc_dec(ptr, drop_fn)` for each pointer.
    pub(super) fn call_rc_dec_all(&mut self, ptrs: &[super::ValueId], drop_fn: super::ValueId) {
        if ptrs.is_empty() {
            return;
        }
        let func_id = self.builder.runtime_fn("ori_rc_dec");
        for &ptr in ptrs {
            self.emit_rt_call(func_id, &[ptr, drop_fn], "");
        }
    }

    /// Emit a may-unwind RC dec of a boxed-recursive child as an `invoke` of
    /// `ori_rc_dec_unwind` → `normal_bb` / `cleanup_bb` (Itanium recoverable
    /// path). `ori_rc_dec_unwind` calls `drop_fn` directly (no `catch_unwind`),
    /// so a panicking user `@drop` inside the child's drop tree unwinds to
    /// `cleanup_bb` instead of aborting at the plain `ori_rc_dec` boundary.
    /// Caller owns both blocks + the post-invoke continuation.
    pub(super) fn invoke_rc_dec_unwind(
        &mut self,
        ptr: super::ValueId,
        drop_fn: super::ValueId,
        normal_bb: crate::codegen::value_id::BlockId,
        cleanup_bb: crate::codegen::value_id::BlockId,
    ) {
        let func_id = self.builder.runtime_fn("ori_rc_dec_unwind");
        self.builder
            .invoke(func_id, &[ptr, drop_fn], normal_bb, cleanup_bb, "");
    }

    /// Call `ori_str_rc_inc(data_ptr, cap)` — handles SSO, heap, and slices.
    pub(super) fn call_str_rc_inc(
        &mut self,
        data_ptr: super::ValueId,
        cap: super::ValueId,
        count: u32,
    ) {
        let func_id = self.builder.runtime_fn("ori_str_rc_inc");
        for _ in 0..count {
            self.emit_rt_call(func_id, &[data_ptr, cap], "");
        }
    }

    /// Call `ori_str_rc_dec(data_ptr, cap, drop_fn)` — handles SSO, heap, and slices.
    pub(super) fn call_str_rc_dec(
        &mut self,
        data_ptr: super::ValueId,
        cap: super::ValueId,
        drop_fn: super::ValueId,
    ) {
        let func_id = self.builder.runtime_fn("ori_str_rc_dec");
        self.emit_rt_call(func_id, &[data_ptr, cap, drop_fn], "");
    }
}
