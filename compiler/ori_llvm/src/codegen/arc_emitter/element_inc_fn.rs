//! Element increment callback generation for collection storage.

use ori_types::Idx;

use crate::codegen::value_id::{FunctionId, ValueId};

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Get or generate an element-inc function for a collection's element type.
    ///
    /// Element-inc functions receive a pointer to an element **within a data
    /// buffer** and increment that element's RC children. Used by COW slow
    /// paths to account for byte-copied elements that now live in a new buffer.
    ///
    /// Returns null for scalar types or types whose elements have no RC children.
    pub(super) fn get_or_generate_elem_inc_fn(&mut self, element_type: Idx) -> ValueId {
        if self.classifier.is_scalar(element_type) {
            return self.builder.const_null_ptr();
        }

        if let Some(&func_id) = self.elem_inc_fn_cache.get(&element_type) {
            return self.builder.get_function_ptr(func_id);
        }

        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        let saved_emitter_func = self.current_function;
        let saved_cleanup_pad = self.current_cleanup_pad.take();

        let func_id = self.generate_elem_inc_fn_body(element_type);

        if self.verify_arc {
            let fn_val = self.builder.get_function_value(func_id);
            if !fn_val.verify(true) {
                tracing::error!("LLVM IR verification failed (generate_elem_inc_fn)");
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

    /// Generate the body of an element-inc function for a given element type.
    ///
    /// The function signature is `void (ptr %elem)`. It loads the element
    /// value from `%elem` and increments all RC-managed children.
    fn generate_elem_inc_fn_body(&mut self, element_type: Idx) -> FunctionId {
        let ptr_ty = self.builder.ptr_type();

        let name = format!("_ori_elem_inc${}", element_type.raw());
        let func_id = self.builder.get_or_declare_void_function(&name, &[ptr_ty]);

        if self.builder.function_has_body(func_id) {
            self.elem_inc_fn_cache.insert(element_type, func_id);
            return func_id;
        }

        self.builder.set_module_local(func_id);
        self.builder.add_nounwind_attribute(func_id);
        self.builder.add_cold_attribute(func_id);
        self.builder.add_uwtable_attribute(func_id);
        self.builder.add_noundef_param_attribute(func_id, 0);

        // Why: caching before body emission terminates recursive-type callback generation.
        self.elem_inc_fn_cache.insert(element_type, func_id);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);
        self.current_function = func_id;

        let elem_ptr = self.builder.get_param(func_id, 0);

        let elem_llvm_ty = self.resolve_type(element_type);
        let elem_val = self.builder.load(elem_llvm_ty, elem_ptr, "elem");

        self.inc_value_rc(elem_val, element_type, 1);

        self.builder.ret_void();
        func_id
    }
}
