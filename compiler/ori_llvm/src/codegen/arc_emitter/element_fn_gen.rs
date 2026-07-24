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

use ori_types::Idx;

use crate::codegen::value_id::{FunctionId, ValueId};

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Look up the user `@drop` method for a type when it implements `Drop`.
    ///
    /// A bound production emitter consumes only the executable artifact's exact
    /// user-drop table. Unbound codegen fixtures retain the general method-map
    /// lookup so low-level emitter tests can be constructed in isolation.
    pub(super) fn user_drop_method(&self, ty: Idx) -> Option<FunctionId> {
        self.user_drop_callable(ty).map(|(function, _)| function)
    }

    /// Resolve the exact physical callable selected for a user-drop operation.
    fn user_drop_callable(
        &self,
        ty: Idx,
    ) -> Option<(FunctionId, crate::codegen::abi::FunctionAbi)> {
        if self.ctx.executable_facts_bound {
            return self
                .ctx
                .user_drop_functions
                .get(&ty)
                .or_else(|| {
                    self.ctx
                        .user_drop_functions
                        .get(&self.pool.resolve_fully(ty))
                })
                .cloned();
        }

        let drop_name = self.interner.intern("drop");
        self.user_method(ty, drop_name)
    }

    /// Resolve a user trait-method impl for `ty` by interned method `Name`.
    ///
    /// Codegen SSOT for "does this type have a user `@<method>` impl + what is
    /// its ABI". Consulted by `user_drop_method` (`"drop"`) and the map/set
    /// hash/equality thunk generators. Returns the `FunctionAbi` so callers read
    /// per-param `passing` to thread self/operands by-value (`Direct`) vs
    /// by-pointer (`Indirect` / `Reference`). Manual `impl T: Trait` and
    /// `#derive(Trait)` both register in `method_functions`, so one lookup
    /// serves both. `Name`-keyed (not `&str`) so callers intern once.
    pub(super) fn user_method(
        &self,
        ty: Idx,
        method_name: ori_ir::Name,
    ) -> Option<(FunctionId, crate::codegen::abi::FunctionAbi)> {
        if self.ctx.executable_facts_bound {
            return self.lookup_exact_method_target(ty, method_name).cloned();
        }
        // A multi-instantiation generic-composite map/set key
        // dispatches the per-instantiation derived `hash`/`eq` keyed on the
        // materialized concrete Idx before the last-instantiation-wins type-name
        // map. A user `@drop` (the other consumer) is not a derived method, so it
        // never hits `mono_derive_functions` and falls through unchanged.
        let resolved = self.pool.resolve_fully(ty);
        if let Some((func_id, abi)) = self.ctx.mono_derive_functions.get(&(resolved, method_name)) {
            return Some((*func_id, abi.clone()));
        }
        let type_name = self.drop_type_name(ty)?;
        let (func_id, abi) = self.ctx.method_functions.get(&(type_name, method_name))?;
        Some((*func_id, abi.clone()))
    }

    /// Resolve the exact callable implementing `Eq` for a collection element.
    ///
    /// Source impls register the Spec method `equals`; generated derives use
    /// [`ori_ir::DerivedTrait::Eq`]'s internal `eq` identity. Both identities
    /// are canonical at their producer, and lookup remains receiver-qualified.
    pub(super) fn user_eq_callable(
        &self,
        ty: Idx,
    ) -> Option<(FunctionId, crate::codegen::abi::FunctionAbi)> {
        let surface_name = self.interner.intern("equals");
        if let Some(callable) = self.user_method(ty, surface_name) {
            return Some(callable);
        }

        let derived_name = self.interner.intern(ori_ir::DerivedTrait::Eq.method_name());
        self.user_method(ty, derived_name)
    }

    /// Does refcount-zero teardown of `ty` transitively run a user `@drop`
    /// (which may raise a foreign Ori exception)?
    ///
    /// Codegen-side consumer of `ori_arc::type_drop_may_unwind`: supplies the
    /// artifact-bound local `@drop` check ([`Self::user_drop_method`]) + the
    /// per-type memo on `CodegenContext`. Gates the
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
    /// `type_idx_to_name` may contain either the unresolved `Named` form or its
    /// resolved Struct/Enum form, so lookup accepts both identities.
    pub(super) fn drop_type_name(&self, ty: Idx) -> Option<ori_ir::Name> {
        if let Some(&n) = self.ctx.type_idx_to_name.get(&ty) {
            return Some(n);
        }
        if ty.raw() as usize >= self.pool.len() {
            return None;
        }
        let resolved = self.pool.resolve_fully(ty);
        if let Some(&n) = self.ctx.type_idx_to_name.get(&resolved) {
            return Some(n);
        }
        // `resolve_fully` returns the input unchanged for out-of-bounds indices
        // (and a Var may resolve to a synthetic out-of-pool idx), so re-check
        // before indexing via `pool.tag`.
        if resolved.raw() as usize >= self.pool.len() {
            return None;
        }
        // `type_idx_to_name` is keyed by each impl method's self-param Idx, which
        // is often the unresolved `Named` form. A receiver arriving as the
        // resolved `Struct`/`Enum` form (e.g. a map key type at a hash/eq thunk
        // site) is unreachable from that key via raw/resolve_fully lookup.
        // Resolve the nominal name straight from the pool descriptor as the
        // final tier.
        match self.pool.tag(resolved) {
            ori_types::Tag::Struct => Some(self.pool.struct_name(resolved)),
            ori_types::Tag::Enum => Some(self.pool.enum_name(resolved)),
            ori_types::Tag::Named => Some(self.pool.named_name(resolved)),
            ori_types::Tag::Applied => Some(self.pool.applied_name(resolved)),
            _ => None,
        }
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
        let resolved = self.pool.resolve_fully(ty);
        let Some((func_id, abi)) = self.user_drop_callable(ty) else {
            return;
        };
        let Some(passing) = abi.params.first().map(|parameter| parameter.passing) else {
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

    /// Emit the user `@drop` for an inline struct/enum VALUE as an `invoke`
    /// (recoverable-panic path): materialize a pointer (entry alloca + store)
    /// then `invoke` the `@drop` → `normal_bb` / `cleanup_bb`. Returns `true`
    /// when an `invoke` was emitted (current block terminated), `false` when the
    /// type has no user `@drop` (caller falls back to the plain field walk).
    pub(super) fn invoke_user_drop_for_inline_value(
        &mut self,
        ty: Idx,
        val: ValueId,
        normal_bb: crate::codegen::value_id::BlockId,
        cleanup_bb: crate::codegen::value_id::BlockId,
    ) -> bool {
        if self.user_drop_method(ty).is_none() {
            return false;
        }
        let resolved = self.pool.resolve_fully(ty);
        let llvm_ty = self.resolve_type(resolved);
        let slot = self
            .builder
            .create_entry_alloca(self.current_function, "udrop.slot", llvm_ty);
        self.builder.store(val, slot);
        self.invoke_user_drop_via_pointer(ty, slot, normal_bb, cleanup_bb)
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
        let resolved = self.pool.resolve_fully(ty);
        let Some((func_id, abi)) = self.user_drop_callable(ty) else {
            return false;
        };
        let Some(passing) = abi.params.first().map(|parameter| parameter.passing) else {
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
        if let Some(&func_id) = self.drop_fn_cache.get(&ty) {
            return self.builder.get_function_ptr(func_id);
        }

        let Some(drop_info) = ori_arc::compute_drop_info(ty, self.classifier, self.pool) else {
            return self.builder.const_null_ptr();
        };

        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let saved_emitter_func = self.current_function;
        let saved_cleanup_pad = self.current_cleanup_pad.take();

        // Why: Recursive type fields can exceed the native stack during drop generation.
        let func_id = ori_stack::ensure_sufficient_stack(|| {
            super::drop_gen::generate_drop_fn(self, ty, &drop_info)
        });

        self.current_cleanup_pad = saved_cleanup_pad;
        self.current_function = saved_emitter_func;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.builder.get_function_ptr(func_id)
    }
}
