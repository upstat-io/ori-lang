//! Element decrement callback generation for collection storage.

use ori_ir::{FIELD_CAP, FIELD_DATA};
use ori_types::Idx;

use crate::codegen::value_id::{FunctionId, ValueId};

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Get or generate an element-dec function for a collection's element type.
    ///
    /// Element-dec functions receive a pointer to an element **within a data
    /// buffer** and decrement that element's RC children. They do NOT free
    /// the element itself (the buffer owns the storage).
    ///
    /// Returns null for scalar types or types whose elements have no RC children.
    pub(super) fn get_or_generate_elem_dec_fn(&mut self, element_type: Idx) -> ValueId {
        // Why: a user drop still requires a teardown thunk for an otherwise scalar element.
        if self.classifier.is_scalar(element_type) && self.user_drop_method(element_type).is_none()
        {
            return self.builder.const_null_ptr();
        }

        if let Some(&func_id) = self.elem_dec_fn_cache.get(&element_type) {
            return self.builder.get_function_ptr(func_id);
        }

        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let saved_emitter_func = self.current_function;
        let saved_cleanup_pad = self.current_cleanup_pad.take();

        let func_id = self.generate_elem_dec_fn_body(element_type);

        if self.verify_arc {
            let fn_val = self.builder.get_function_value(func_id);
            if !fn_val.verify(true) {
                tracing::error!("LLVM IR verification failed (generate_elem_dec_fn)");
                self.builder.record_codegen_error();
            }
        }

        self.current_cleanup_pad = saved_cleanup_pad;
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

        if self.builder.function_has_body(func_id) {
            self.elem_dec_fn_cache.insert(element_type, func_id);
            return func_id;
        }

        self.builder.set_module_local(func_id);
        // INVARIANT: Itanium propagates user-drop unwinds to the buffer cleanup; SEH aborts instead.
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

        // Why: caching before body emission terminates recursive-type callback generation.
        self.elem_dec_fn_cache.insert(element_type, func_id);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);
        self.current_function = func_id;

        let elem_ptr = self.builder.get_param(func_id, 0);

        // INVARIANT: top-level user drop precedes compiler-owned field release.
        // INVARIANT: Itanium cleanup releases this element's children before outer buffer cleanup resumes.
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

        // Why: string slices use interior pointers; capacity flags recover the owning allocation.
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
                let Some(cap) = self.builder.extract_value(elem_val, FIELD_CAP, "elem.cap") else {
                    // Why: Tag::Str always resolves to the canonical three-field string layout.
                    unreachable!("string element requires a capacity field")
                };
                self.call_str_rc_dec(dp, cap, drop_fn);
                self.builder.br(skip);

                self.builder.position_at_end(skip);
            }
        } else {
            self.dec_value_rc(elem_val, element_type);
        }
    }
}
