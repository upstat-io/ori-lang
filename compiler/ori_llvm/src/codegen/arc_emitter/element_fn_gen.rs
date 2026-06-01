//! Element-level function generation for collection RC operations.
//!
//! Generates and caches element-dec, element-inc, and drop functions used by
//! collection operations (buffer RC dec, COW slow paths). Each generated function
//! has signature `void (ptr %elem)` and operates on a single element within a
//! data buffer.
//!
//! Caching is critical: element functions are requested per-collection-operation,
//! and recursive types require the cache entry to exist before body generation
//! to break cycles.

use ori_ir::{FIELD_CAP, FIELD_DATA};
use ori_types::Idx;

use super::ArcIrEmitter;
use crate::codegen::value_id::{FunctionId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Look up the user `@drop` method for a type when it implements `Drop`.
    ///
    /// Consults the canonical method map: `type_idx_to_name` resolves the
    /// type's `Name`, then `method_functions[(type_name, "drop")]` resolves
    /// the compiled `_ori_<Type>$drop` method. Returns `None` when the type
    /// does not implement `Drop`. This is the codegen-side SSOT for "does
    /// this type have a user `@drop`" — independent of the upstream burden
    /// registry, which is not threaded onto the codegen path.
    pub(super) fn user_drop_method(&self, ty: Idx) -> Option<FunctionId> {
        let type_name = self.drop_type_name(ty)?;
        let drop_name = self.interner.intern("drop");
        let (func_id, _abi) = self.ctx.method_functions.get(&(type_name, drop_name))?;
        Some(*func_id)
    }

    /// Does refcount-zero teardown of `ty` transitively run a user `@drop`
    /// (which may raise a foreign Ori exception)?
    ///
    /// Codegen-side consumer of `ori_arc::type_drop_may_unwind`: supplies the
    /// `method_functions`-based local `@drop` check ([`Self::user_drop_method`]
    /// — the codegen SSOT, since the burden-registry `user_drop` is hardcoded
    /// `None` on this path) + the per-type memo on `CodegenContext`. Gates the
    /// may-unwind drop-fn shape (skip `nounwind` + set personality + `invoke`
    /// the user `@drop`) and the may-unwind `RcDec` routing (`ori_rc_dec_unwind`
    /// via `invoke` + cleanup pad).
    pub(super) fn drop_may_unwind(&self, ty: Idx) -> bool {
        let has_user_drop = |t: Idx| self.user_drop_method(t).is_some();
        ori_arc::type_drop_may_unwind(
            ty,
            self.classifier,
            self.pool,
            &has_user_drop,
            &mut self.ctx.drop_unwind_memo.borrow_mut(),
        )
    }

    /// Resolve a type `Idx` to its registered type `Name` for method lookup.
    ///
    /// `type_idx_to_name` is keyed by the `@drop` self-param `Idx` recorded at
    /// `compile_impls` time, which may be the unresolved `Named` form while a
    /// drop-fn / elem-dec generation site holds the resolved `Struct`/`Enum`
    /// form (or vice versa). Try both keys so the lookup is robust to the
    /// resolve-state mismatch.
    pub(super) fn drop_type_name(&self, ty: Idx) -> Option<ori_ir::Name> {
        if let Some(&n) = self.ctx.type_idx_to_name.get(&ty) {
            return Some(n);
        }
        if ty.raw() as usize >= self.pool.len() {
            return None;
        }
        let resolved = self.pool.resolve_fully(ty);
        self.ctx.type_idx_to_name.get(&resolved).copied()
    }

    /// Emit the user `@drop` invocation for an inline struct/enum VALUE (not a
    /// pointer), when the type implements `Drop`.
    ///
    /// The value-traversal dec path (`dec_value_rc`) holds a loaded LLVM
    /// aggregate value, but `@drop` receives `self` by pointer. Materialize a
    /// pointer via an entry alloca + store, then forward to
    /// [`Self::emit_user_drop_via_pointer`]. No-op when the type has no user
    /// `@drop`. Plain (non-invoking) call — the value-traversal dec path is
    /// reached from runtime-driven buffer-dec loops where a nested panic
    /// aborts via the runtime drop guard.
    pub(super) fn emit_user_drop_for_inline_value(&mut self, ty: Idx, val: ValueId) {
        if self.user_drop_method(ty).is_none() {
            return;
        }
        let resolved = self.pool.resolve_fully(ty);
        let llvm_ty = self.resolve_type(resolved);
        let slot = self
            .builder
            .create_entry_alloca(self.current_function, "udrop.slot", llvm_ty);
        self.builder.store(val, slot);
        self.emit_user_drop_via_pointer(ty, slot);
    }

    /// Emit the user `@drop` invocation for a Drop type, given a pointer to
    /// the value (`data_ptr`).
    ///
    /// The `@drop` method receives `self` per its impl-method ABI. For a
    /// pass-by-pointer (`Reference` / `Indirect`) receiver (heap or
    /// over-16-byte Drop types) `data_ptr` is forwarded directly. For a
    /// pass-by-value (`Direct`) receiver (a small non-`Value` Drop type) the
    /// value is loaded from `data_ptr` first. Borrows `self`; the drop
    /// function still owns the field walk that follows.
    pub(super) fn emit_user_drop_via_pointer(&mut self, ty: Idx, data_ptr: ValueId) {
        let Some(type_name) = self.drop_type_name(ty) else {
            return;
        };
        let resolved = self.pool.resolve_fully(ty);
        let drop_name = self.interner.intern("drop");
        let Some((func_id, passing)) = self
            .ctx
            .method_functions
            .get(&(type_name, drop_name))
            .and_then(|(fid, abi)| abi.params.first().map(|p| (*fid, p.passing)))
        else {
            return;
        };
        let arg = match passing {
            crate::codegen::abi::ParamPassing::Direct => {
                let self_ty = self.resolve_type(resolved);
                self.builder.load(self_ty, data_ptr, "udrop.self")
            }
            crate::codegen::abi::ParamPassing::Indirect { .. }
            | crate::codegen::abi::ParamPassing::Reference => data_ptr,
            crate::codegen::abi::ParamPassing::Void => return,
        };
        self.emit_rt_call(func_id, &[arg], "");
    }

    /// Emit the user `@drop` invocation for a Drop type as an `invoke` (the
    /// recoverable-panic path), routing the foreign Ori exception to a cleanup
    /// landing pad instead of aborting.
    ///
    /// Mirrors [`Self::emit_user_drop_via_pointer`]'s ABI resolution, but emits
    /// `invoke @drop → normal_bb / cleanup_bb` (Itanium). On normal return the
    /// caller continues the field walk in `normal_bb`; on a `@drop` panic the
    /// caller's cleanup pad in `cleanup_bb` re-runs the field walk + free, then
    /// `resume`s. Returns `true` when an `invoke` was emitted (the current block
    /// is now terminated), `false` when there is no user `@drop` (caller emits
    /// the plain field walk).
    pub(super) fn invoke_user_drop_via_pointer(
        &mut self,
        ty: Idx,
        data_ptr: ValueId,
        normal_bb: crate::codegen::value_id::BlockId,
        cleanup_bb: crate::codegen::value_id::BlockId,
    ) -> bool {
        let Some(type_name) = self.drop_type_name(ty) else {
            return false;
        };
        let resolved = self.pool.resolve_fully(ty);
        let drop_name = self.interner.intern("drop");
        let Some((func_id, passing)) = self
            .ctx
            .method_functions
            .get(&(type_name, drop_name))
            .and_then(|(fid, abi)| abi.params.first().map(|p| (*fid, p.passing)))
        else {
            return false;
        };
        let arg = match passing {
            crate::codegen::abi::ParamPassing::Direct => {
                let self_ty = self.resolve_type(resolved);
                self.builder.load(self_ty, data_ptr, "udrop.self")
            }
            crate::codegen::abi::ParamPassing::Indirect { .. }
            | crate::codegen::abi::ParamPassing::Reference => data_ptr,
            crate::codegen::abi::ParamPassing::Void => return false,
        };
        self.builder
            .invoke(func_id, &[arg], normal_bb, cleanup_bb, "");
        true
    }

    /// Get or generate the drop function for a type.
    ///
    /// Returns a function pointer `ValueId` suitable for passing to
    /// `ori_rc_dec`. Returns null for scalar types or when no classifier
    /// is available (no drop needed).
    ///
    /// Drop functions are cached per type. For recursive types, the
    /// `FunctionId` is cached **before** body generation to break cycles.
    pub(super) fn get_or_generate_drop_fn(&mut self, ty: Idx) -> ValueId {
        // Fast path: already generated
        if let Some(&func_id) = self.drop_fn_cache.get(&ty) {
            return self.builder.get_function_ptr(func_id);
        }

        // Compute what drop operations this type needs
        let Some(drop_info) = ori_arc::compute_drop_info(ty, self.classifier, self.pool) else {
            return self.builder.const_null_ptr();
        };

        // Save current builder position (we're about to create a new function)
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let saved_emitter_func = self.current_function;
        let saved_funclet_pad = self.current_funclet_pad.take();

        // Generate the drop function (handles declaration, caching, and body).
        // Stack guard: drop generation recurses through nested type fields.
        let func_id = ori_stack::ensure_sufficient_stack(|| {
            super::drop_gen::generate_drop_fn(self, ty, &drop_info)
        });

        // Restore builder position, emitter's current function, and funclet pad
        self.current_funclet_pad = saved_funclet_pad;
        self.current_function = saved_emitter_func;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.builder.get_function_ptr(func_id)
    }

    /// Get or generate an element-dec function for a collection's element type.
    ///
    /// Element-dec functions receive a pointer to an element **within a data
    /// buffer** and decrement that element's RC children. They do NOT free
    /// the element itself (the buffer owns the storage).
    ///
    /// Returns null for scalar types or types whose elements have no RC children.
    pub(super) fn get_or_generate_elem_dec_fn(&mut self, element_type: Idx) -> ValueId {
        // Scalar elements — no RC children to dec
        if self.classifier.is_scalar(element_type) {
            return self.builder.const_null_ptr();
        }

        // Fast path: already generated
        if let Some(&func_id) = self.elem_dec_fn_cache.get(&element_type) {
            return self.builder.get_function_ptr(func_id);
        }

        // Save builder state, emitter's current function, and funclet pad
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let saved_emitter_func = self.current_function;
        let saved_funclet_pad = self.current_funclet_pad.take();

        let func_id = self.generate_elem_dec_fn_body(element_type);

        // Function-level LLVM IR verification.
        if self.verify_arc {
            let fn_val = self.builder.get_function_value(func_id);
            if !fn_val.verify(true) {
                tracing::error!("LLVM IR verification failed (generate_elem_dec_fn)");
                self.builder.record_codegen_error();
            }
        }

        // Restore builder state, emitter's current function, and funclet pad
        self.current_funclet_pad = saved_funclet_pad;
        self.current_function = saved_emitter_func;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.builder.get_function_ptr(func_id)
    }

    /// Generate the body of an element-dec function for a given element type.
    ///
    /// The function signature is `void (ptr %elem)`. It loads the element
    /// value from `%elem` and decrements all RC-managed children.
    fn generate_elem_dec_fn_body(&mut self, element_type: Idx) -> FunctionId {
        let ptr_ty = self.builder.ptr_type();

        let name = format!("_ori_elem_dec${}", element_type.raw());
        let func_id = self.builder.get_or_declare_void_function(&name, &[ptr_ty]);

        // If already generated by a previous emitter instance, reuse it.
        if self.builder.function_has_body(func_id) {
            self.elem_dec_fn_cache.insert(element_type, func_id);
            return func_id;
        }

        self.builder.set_ccc(func_id);
        // May-unwind element teardown: when the element type's drop tree reaches
        // a user `@drop` (foreign Ori exception), the element-dec thunk is NOT
        // `nounwind` — it threads the exception out so the codegen-emitted buffer
        // teardown loop's per-element cleanup pad can free the remaining elements
        // + buffer, then `resume`. Itanium only; SEH keeps `nounwind` + abort
        // (the SEH funclet-EH re-enablement anchor). A scalar / plain element
        // keeps `nounwind` (the runtime buffer-dec fast path).
        let elem_unwinds = self.drop_may_unwind(element_type)
            && self.builder.eh_model() == crate::codegen::eh_model::EhModel::Itanium;
        if elem_unwinds {
            let personality = self.builder.runtime_fn("ori_eh_personality");
            self.builder.set_personality(func_id, personality);
        } else {
            self.builder.add_nounwind_attribute(func_id);
        }
        self.builder.add_cold_attribute(func_id);
        self.builder.add_uwtable_attribute(func_id);
        self.builder.add_noundef_param_attribute(func_id, 0);

        // Cache before body generation to handle recursive types
        self.elem_dec_fn_cache.insert(element_type, func_id);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);
        self.current_function = func_id;

        let elem_ptr = self.builder.get_param(func_id, 0);

        // User `@drop` AUGMENT for the buffer element: when the element type
        // implements `Drop`, run its `@drop` FIRST (before the compiler walks
        // owned fields), passing the in-buffer element pointer. This is the
        // top-level element drop; NESTED struct/enum fields get their `@drop`
        // from `dec_aggregate_fields` below (so there is no double call on the
        // top-level value). The buffer-dec loop that calls this thunk is
        // runtime-driven (`ori_buffer_rc_dec` / `ori_buffer_drop_unique` via
        // `call_drop_fn`), so this is a plain (non-invoking) call — a panic in
        // a collection element's `@drop` aborts via the runtime drop guard.
        // Top-level element `@drop` (AUGMENT), then dec the element's RC
        // children. When the element's own `@drop` may unwind (Itanium), the
        // `@drop` is an `invoke`: on a panic the cleanup pad still decs the
        // element's owned children (no leak), then `resume`s so the codegen
        // buffer-teardown loop's per-element cleanup pad can drain the rest.
        if elem_unwinds && self.user_drop_method(element_type).is_some() {
            let cont = self
                .builder
                .append_block(self.current_function, "elem_dec.cont");
            let cleanup = self
                .builder
                .append_block(self.current_function, "elem_dec.cleanup");
            if self.invoke_user_drop_via_pointer(element_type, elem_ptr, cont, cleanup) {
                self.builder.position_at_end(cont);
                self.emit_elem_value_field_dec(element_type, elem_ptr);
                self.builder.ret_void();

                self.builder.position_at_end(cleanup);
                let personality = self.builder.runtime_fn("ori_eh_personality");
                let lp = self.builder.landingpad(personality, true, "elem_dec.lp");
                let enter = self.builder.runtime_fn("ori_drop_cleanup_enter");
                self.builder.call(enter, &[], "");
                self.emit_elem_value_field_dec(element_type, elem_ptr);
                let exit = self.builder.runtime_fn("ori_drop_cleanup_exit");
                self.builder.call(exit, &[], "");
                self.builder.resume(lp);
                return func_id;
            }
        }

        // Plain path: top-level `@drop` (if any) as a plain call, then field dec.
        self.emit_user_drop_via_pointer(element_type, elem_ptr);
        self.emit_elem_value_field_dec(element_type, elem_ptr);
        self.builder.ret_void();
        func_id
    }

    /// Dec the RC children of a buffer element VALUE — no user `@drop`, no
    /// terminator. The caller owns the `@drop` call (plain or `invoke`) and the
    /// block terminator (`ret_void` / `resume`).
    ///
    /// `str` elements route through `ori_str_rc_dec` (slice-aware: SSO +
    /// `SLICE_FLAG`); all other elements through `dec_value_rc`.
    fn emit_elem_value_field_dec(&mut self, element_type: Idx, elem_ptr: ValueId) {
        let elem_llvm_ty = self.resolve_type(element_type);
        let elem_val = self.builder.load(elem_llvm_ty, elem_ptr, "elem");

        // For str elements, use ori_str_rc_dec which handles seamless slices
        // from str.split(). Normal dec_value_rc calls ori_rc_dec on the data
        // pointer, but for slices the data pointer is an interior pointer into
        // the original string's allocation — not the start of an RC block.
        // Spec: ori_str_rc_dec checks SSO, SLICE_FLAG in cap, then delegates.
        let resolved = self.pool.resolve_fully(element_type);
        let tag = self.pool.tag(resolved);
        if tag == ori_types::Tag::Str {
            if let Some(dp) = self
                .builder
                .extract_value(elem_val, FIELD_DATA, "elem.data")
            {
                let do_dec = self
                    .builder
                    .append_block(self.current_function, "elem_dec.str_heap");
                let skip = self
                    .builder
                    .append_block(self.current_function, "elem_dec.str_skip");
                let is_sso = self.emit_sso_check(dp, "elem_dec.str");
                self.builder.cond_br(is_sso, skip, do_dec);

                self.builder.position_at_end(do_dec);
                let drop_fn = self.get_or_generate_drop_fn(element_type);
                let cap = self
                    .builder
                    .extract_value(elem_val, FIELD_CAP, "elem.cap")
                    .expect("str must have cap field");
                self.call_str_rc_dec(dp, cap, drop_fn);
                self.builder.br(skip);

                self.builder.position_at_end(skip);
            }
        } else {
            // Dec all RC children of the element value
            self.dec_value_rc(elem_val, element_type);
        }
    }

    /// Get or generate an element-inc function for a collection's element type.
    ///
    /// Element-inc functions receive a pointer to an element **within a data
    /// buffer** and increment that element's RC children. Used by COW slow
    /// paths to account for byte-copied elements that now live in a new buffer.
    ///
    /// Returns null for scalar types or types whose elements have no RC children.
    pub(super) fn get_or_generate_elem_inc_fn(&mut self, element_type: Idx) -> ValueId {
        // Scalar elements — no RC children to inc
        if self.classifier.is_scalar(element_type) {
            return self.builder.const_null_ptr();
        }

        // Fast path: already generated
        if let Some(&func_id) = self.elem_inc_fn_cache.get(&element_type) {
            return self.builder.get_function_ptr(func_id);
        }

        // Save builder state, emitter's current function, and funclet pad
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let saved_emitter_func = self.current_function;
        let saved_funclet_pad = self.current_funclet_pad.take();

        let func_id = self.generate_elem_inc_fn_body(element_type);

        // Function-level LLVM IR verification.
        if self.verify_arc {
            let fn_val = self.builder.get_function_value(func_id);
            if !fn_val.verify(true) {
                tracing::error!("LLVM IR verification failed (generate_elem_inc_fn)");
                self.builder.record_codegen_error();
            }
        }

        // Restore builder state, emitter's current function, and funclet pad
        self.current_funclet_pad = saved_funclet_pad;
        self.current_function = saved_emitter_func;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.builder.get_function_ptr(func_id)
    }

    /// Generate the body of an element-inc function for a given element type.
    ///
    /// The function signature is `void (ptr %elem)`. It loads the element
    /// value from `%elem` and increments all RC-managed children.
    fn generate_elem_inc_fn_body(&mut self, element_type: Idx) -> FunctionId {
        let ptr_ty = self.builder.ptr_type();

        let name = format!("_ori_elem_inc${}", element_type.raw());
        let func_id = self.builder.get_or_declare_void_function(&name, &[ptr_ty]);

        // If already generated by a previous emitter instance, reuse it.
        if self.builder.function_has_body(func_id) {
            self.elem_inc_fn_cache.insert(element_type, func_id);
            return func_id;
        }

        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        self.builder.add_cold_attribute(func_id);
        self.builder.add_uwtable_attribute(func_id);
        self.builder.add_noundef_param_attribute(func_id, 0);

        // Cache before body generation to handle recursive types
        self.elem_inc_fn_cache.insert(element_type, func_id);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);
        self.current_function = func_id;

        let elem_ptr = self.builder.get_param(func_id, 0);

        // Load the element value from the pointer
        let elem_llvm_ty = self.resolve_type(element_type);
        let elem_val = self.builder.load(elem_llvm_ty, elem_ptr, "elem");

        // Inc all RC children of the element value
        self.inc_value_rc(elem_val, element_type, 1);

        self.builder.ret_void();
        func_id
    }
}
