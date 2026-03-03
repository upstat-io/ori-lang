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

use super::ArcIrEmitter;
use crate::codegen::value_id::{FunctionId, ValueId};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
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
        let func_id = self.builder.declare_void_function(&name, &[ptr_ty]);
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        self.builder.add_cold_attribute(func_id);

        // Cache before body generation to handle recursive types
        self.elem_dec_fn_cache.insert(element_type, func_id);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);
        self.current_function = func_id;

        let elem_ptr = self.builder.get_param(func_id, 0);

        // Load the element value from the pointer
        let elem_llvm_ty = self.resolve_type(element_type);
        let elem_val = self.builder.load(elem_llvm_ty, elem_ptr, "elem");

        // Dec all RC children of the element value
        self.dec_value_rc(elem_val, element_type);

        self.builder.ret_void();
        func_id
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
        let func_id = self.builder.declare_void_function(&name, &[ptr_ty]);
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        self.builder.add_cold_attribute(func_id);

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
