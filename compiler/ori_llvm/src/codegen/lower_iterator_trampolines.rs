//! Trampoline generation for iterator adapter closures.
//!
//! Bridges typed Ori closures (`fastcc (ptr %env, T) -> U`) to the runtime's
//! generic C ABI (`ccc (ptr %wrapper, ptr %in, ptr %out) -> void` for map,
//! `ccc (ptr %wrapper, ptr %in) -> bool` for filter).
//!
//! Each trampoline unpacks the original closure's `fn_ptr` and `env_ptr` from a
//! wrapper struct, loads typed values from input pointers, calls the closure
//! with `fastcc`, and stores the result to the output pointer.

use ori_types::Idx;

use super::expr_lowerer::ExprLowerer;
use super::value_id::{FunctionId, ValueId};

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    /// Generate a map trampoline and wrapper struct for a closure.
    ///
    /// Returns `(wrapper_ptr, trampoline_fn_ptr)` suitable for passing to
    /// `ori_iter_map(iter, trampoline_fn, wrapper_ptr, in_size)`.
    ///
    /// The wrapper struct layout is `{ ptr fn_ptr, ptr env_ptr }` — allocated
    /// via `ori_alloc` and populated with the closure's components.
    pub(crate) fn generate_map_trampoline(
        &mut self,
        closure_val: ValueId,
        elem_type: Idx,
        result_type: Idx,
    ) -> Option<(ValueId, ValueId)> {
        // Extract fn_ptr and env_ptr from the closure fat pointer { ptr, ptr }
        let fn_ptr = self
            .builder
            .extract_value(closure_val, 0, "map.closure.fn")?;
        let env_ptr = self
            .builder
            .extract_value(closure_val, 1, "map.closure.env")?;

        // Allocate wrapper struct { ptr fn_ptr, ptr env_ptr } = 16 bytes
        let wrapper = self.alloc_trampoline_wrapper(fn_ptr, env_ptr)?;

        // Get or create the trampoline function for this (T -> U) signature
        let trampoline = self.get_or_create_map_trampoline(elem_type, result_type);
        let trampoline_ptr = self.builder.get_function_value(trampoline);
        let trampoline_val = self
            .builder
            .intern_value(trampoline_ptr.as_global_value().as_pointer_value().into());

        Some((wrapper, trampoline_val))
    }

    /// Generate a filter trampoline and wrapper struct for a closure.
    ///
    /// Returns `(wrapper_ptr, trampoline_fn_ptr)` suitable for passing to
    /// `ori_iter_filter(iter, trampoline_fn, wrapper_ptr, elem_size)`.
    pub(crate) fn generate_filter_trampoline(
        &mut self,
        closure_val: ValueId,
        elem_type: Idx,
    ) -> Option<(ValueId, ValueId)> {
        let fn_ptr = self
            .builder
            .extract_value(closure_val, 0, "filt.closure.fn")?;
        let env_ptr = self
            .builder
            .extract_value(closure_val, 1, "filt.closure.env")?;

        let wrapper = self.alloc_trampoline_wrapper(fn_ptr, env_ptr)?;

        let trampoline = self.get_or_create_filter_trampoline(elem_type);
        let trampoline_ptr = self.builder.get_function_value(trampoline);
        let trampoline_val = self
            .builder
            .intern_value(trampoline_ptr.as_global_value().as_pointer_value().into());

        Some((wrapper, trampoline_val))
    }

    /// Generate a `for_each` trampoline and wrapper struct for a closure.
    ///
    /// Returns `(wrapper_ptr, trampoline_fn_ptr)` suitable for passing to
    /// `ori_iter_for_each(iter, trampoline_fn, wrapper_ptr, elem_size)`.
    pub(crate) fn generate_for_each_trampoline(
        &mut self,
        closure_val: ValueId,
        elem_type: Idx,
    ) -> Option<(ValueId, ValueId)> {
        let fn_ptr = self
            .builder
            .extract_value(closure_val, 0, "foreach.closure.fn")?;
        let env_ptr = self
            .builder
            .extract_value(closure_val, 1, "foreach.closure.env")?;

        let wrapper = self.alloc_trampoline_wrapper(fn_ptr, env_ptr)?;

        let trampoline = self.get_or_create_for_each_trampoline(elem_type);
        let trampoline_ptr = self.builder.get_function_value(trampoline);
        let trampoline_val = self
            .builder
            .intern_value(trampoline_ptr.as_global_value().as_pointer_value().into());

        Some((wrapper, trampoline_val))
    }

    /// Generate a fold trampoline and wrapper struct for a closure.
    ///
    /// Returns `(wrapper_ptr, trampoline_fn_ptr)` suitable for passing to
    /// `ori_iter_fold(iter, init, trampoline_fn, wrapper_ptr, ...)`.
    pub(crate) fn generate_fold_trampoline(
        &mut self,
        closure_val: ValueId,
        acc_type: Idx,
        elem_type: Idx,
    ) -> Option<(ValueId, ValueId)> {
        let fn_ptr = self
            .builder
            .extract_value(closure_val, 0, "fold.closure.fn")?;
        let env_ptr = self
            .builder
            .extract_value(closure_val, 1, "fold.closure.env")?;

        let wrapper = self.alloc_trampoline_wrapper(fn_ptr, env_ptr)?;

        let trampoline = self.get_or_create_fold_trampoline(acc_type, elem_type);
        let trampoline_ptr = self.builder.get_function_value(trampoline);
        let trampoline_val = self
            .builder
            .intern_value(trampoline_ptr.as_global_value().as_pointer_value().into());

        Some((wrapper, trampoline_val))
    }

    /// Allocate and populate a trampoline wrapper struct `{ ptr fn_ptr, ptr env_ptr }`.
    fn alloc_trampoline_wrapper(&mut self, fn_ptr: ValueId, env_ptr: ValueId) -> Option<ValueId> {
        // Wrapper = { ptr, ptr } = 16 bytes, 8-byte aligned
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();
        let sixteen = self.builder.const_i64(16);
        let eight = self.builder.const_i64(8);
        let alloc_fn = self
            .builder
            .get_or_declare_function("ori_alloc", &[i64_ty, i64_ty], ptr_ty);
        let wrapper = self
            .builder
            .call(alloc_fn, &[sixteen, eight], "trampoline.wrapper")?;

        // Store fn_ptr at offset 0
        self.builder.store(fn_ptr, wrapper);

        // Store env_ptr at offset 8 (using raw GEP on i8)
        let i8_ty = self.builder.i8_type();
        let eight_val = self.builder.const_i64(8);
        let env_slot = self
            .builder
            .gep(i8_ty, wrapper, &[eight_val], "trampoline.env_slot");
        self.builder.store(env_ptr, env_slot);

        Some(wrapper)
    }

    /// Get or create a map trampoline function for the given `(T -> U)` types.
    ///
    /// Signature: `ccc void @_ori_tramp_map_N(ptr %wrapper, ptr %in, ptr %out)`
    fn get_or_create_map_trampoline(&mut self, elem_type: Idx, result_type: Idx) -> FunctionId {
        let counter = self.lambda_counter.get();
        self.lambda_counter.set(counter + 1);
        let name = format!("_ori_tramp_map_{counter}");

        let ptr_ty = self.builder.ptr_type();

        // declare ccc void @tramp(ptr %wrapper, ptr %in_ptr, ptr %out_ptr)
        let tramp = self
            .builder
            .declare_void_function(&name, &[ptr_ty, ptr_ty, ptr_ty]);
        // C calling convention (called by Rust runtime)
        self.builder.set_ccc(tramp);

        let entry = self.builder.append_block(tramp, "entry");
        let saved_pos = self.builder.save_position();
        let saved_func = self.current_function;

        self.builder.set_current_function(tramp);
        self.builder.position_at_end(entry);

        // Load fn_ptr and env_ptr from wrapper struct
        let wrapper_param = self.builder.get_param(tramp, 0);
        let in_param = self.builder.get_param(tramp, 1);
        let out_param = self.builder.get_param(tramp, 2);

        let fn_ptr = self.builder.load(ptr_ty, wrapper_param, "tramp.fn_ptr");
        let i8_ty = self.builder.i8_type();
        let eight = self.builder.const_i64(8);
        let env_slot = self
            .builder
            .gep(i8_ty, wrapper_param, &[eight], "tramp.env_slot");
        let env_ptr = self.builder.load(ptr_ty, env_slot, "tramp.env_ptr");

        // Load typed input value from in_ptr
        let elem_llvm_ty = self.type_resolver.resolve(elem_type);
        let elem_ty_id = self.builder.register_type(elem_llvm_ty);
        let input_val = self.builder.load(elem_ty_id, in_param, "tramp.input");

        // Call closure: fastcc U @fn_ptr(ptr %env, T %input)
        let result_llvm_ty = self.type_resolver.resolve(result_type);
        let result_ty_id = self.builder.register_type(result_llvm_ty);
        let call_result = self.builder.call_indirect(
            result_ty_id,
            &[ptr_ty, elem_ty_id],
            fn_ptr,
            &[env_ptr, input_val],
            "tramp.result",
        );

        // Store result to out_ptr
        if let Some(result) = call_result {
            self.builder.store(result, out_param);
        }

        // Return void
        self.builder.ret_void();

        // Restore
        self.current_function = saved_func;
        self.builder.set_current_function(saved_func);
        self.builder.restore_position(saved_pos);

        tramp
    }

    /// Get or create a filter trampoline function for the given `T` type.
    ///
    /// Signature: `ccc i1 @_ori_tramp_filter_N(ptr %wrapper, ptr %elem_ptr)`
    fn get_or_create_filter_trampoline(&mut self, elem_type: Idx) -> FunctionId {
        let counter = self.lambda_counter.get();
        self.lambda_counter.set(counter + 1);
        let name = format!("_ori_tramp_filter_{counter}");

        let ptr_ty = self.builder.ptr_type();
        let bool_ty = self.builder.bool_type();

        // declare ccc i1 @tramp(ptr %wrapper, ptr %elem_ptr)
        let tramp = self
            .builder
            .declare_function(&name, &[ptr_ty, ptr_ty], bool_ty);
        self.builder.set_ccc(tramp);

        let entry = self.builder.append_block(tramp, "entry");
        let saved_pos = self.builder.save_position();
        let saved_func = self.current_function;

        self.builder.set_current_function(tramp);
        self.builder.position_at_end(entry);

        // Load fn_ptr and env_ptr from wrapper
        let wrapper_param = self.builder.get_param(tramp, 0);
        let elem_param = self.builder.get_param(tramp, 1);

        let fn_ptr = self.builder.load(ptr_ty, wrapper_param, "tramp.fn_ptr");
        let i8_ty = self.builder.i8_type();
        let eight = self.builder.const_i64(8);
        let env_slot = self
            .builder
            .gep(i8_ty, wrapper_param, &[eight], "tramp.env_slot");
        let env_ptr = self.builder.load(ptr_ty, env_slot, "tramp.env_ptr");

        // Load typed element from elem_ptr
        let elem_llvm_ty = self.type_resolver.resolve(elem_type);
        let elem_ty_id = self.builder.register_type(elem_llvm_ty);
        let elem_val = self.builder.load(elem_ty_id, elem_param, "tramp.elem");

        // Call closure: fastcc bool @fn_ptr(ptr %env, T %elem)
        let call_result = self.builder.call_indirect(
            bool_ty,
            &[ptr_ty, elem_ty_id],
            fn_ptr,
            &[env_ptr, elem_val],
            "tramp.result",
        );

        // Return the bool result
        if let Some(result) = call_result {
            self.builder.ret(result);
        } else {
            let false_val = self.builder.const_bool(false);
            self.builder.ret(false_val);
        }

        // Restore
        self.current_function = saved_func;
        self.builder.set_current_function(saved_func);
        self.builder.restore_position(saved_pos);

        tramp
    }

    /// Get or create a `for_each` trampoline for the given `T` type.
    ///
    /// Signature: `ccc void @_ori_tramp_foreach_N(ptr %wrapper, ptr %elem_ptr)`
    ///
    /// Calls the closure and discards the return value (closure returns void
    /// in Ori, but LLVM may emit unit as i64).
    fn get_or_create_for_each_trampoline(&mut self, elem_type: Idx) -> FunctionId {
        let counter = self.lambda_counter.get();
        self.lambda_counter.set(counter + 1);
        let name = format!("_ori_tramp_foreach_{counter}");

        let ptr_ty = self.builder.ptr_type();

        let tramp = self.builder.declare_void_function(&name, &[ptr_ty, ptr_ty]);
        self.builder.set_ccc(tramp);

        let entry = self.builder.append_block(tramp, "entry");
        let saved_pos = self.builder.save_position();
        let saved_func = self.current_function;

        self.builder.set_current_function(tramp);
        self.builder.position_at_end(entry);

        let wrapper_param = self.builder.get_param(tramp, 0);
        let elem_param = self.builder.get_param(tramp, 1);

        let fn_ptr = self.builder.load(ptr_ty, wrapper_param, "tramp.fn_ptr");
        let i8_ty = self.builder.i8_type();
        let eight = self.builder.const_i64(8);
        let env_slot = self
            .builder
            .gep(i8_ty, wrapper_param, &[eight], "tramp.env_slot");
        let env_ptr = self.builder.load(ptr_ty, env_slot, "tramp.env_ptr");

        let elem_llvm_ty = self.type_resolver.resolve(elem_type);
        let elem_ty_id = self.builder.register_type(elem_llvm_ty);
        let elem_val = self.builder.load(elem_ty_id, elem_param, "tramp.elem");

        // Ori's void/unit maps to i64 in LLVM — call and discard result
        let i64_ty = self.builder.i64_type();
        let _call_result = self.builder.call_indirect(
            i64_ty,
            &[ptr_ty, elem_ty_id],
            fn_ptr,
            &[env_ptr, elem_val],
            "tramp.discard",
        );

        self.builder.ret_void();

        self.current_function = saved_func;
        self.builder.set_current_function(saved_func);
        self.builder.restore_position(saved_pos);

        tramp
    }

    /// Get or create a fold trampoline for the given `(Acc, T) -> Acc` types.
    ///
    /// Signature: `ccc void @_ori_tramp_fold_N(ptr %wrapper, ptr %acc_ptr, ptr %elem_ptr, ptr %out_ptr)`
    ///
    /// Loads typed acc + elem from pointers, calls the 2-arg closure, stores result to `out_ptr`.
    fn get_or_create_fold_trampoline(&mut self, acc_type: Idx, elem_type: Idx) -> FunctionId {
        let counter = self.lambda_counter.get();
        self.lambda_counter.set(counter + 1);
        let name = format!("_ori_tramp_fold_{counter}");

        let ptr_ty = self.builder.ptr_type();

        let tramp = self
            .builder
            .declare_void_function(&name, &[ptr_ty, ptr_ty, ptr_ty, ptr_ty]);
        self.builder.set_ccc(tramp);

        let entry = self.builder.append_block(tramp, "entry");
        let saved_pos = self.builder.save_position();
        let saved_func = self.current_function;

        self.builder.set_current_function(tramp);
        self.builder.position_at_end(entry);

        let wrapper_param = self.builder.get_param(tramp, 0);
        let acc_param = self.builder.get_param(tramp, 1);
        let elem_param = self.builder.get_param(tramp, 2);
        let out_param = self.builder.get_param(tramp, 3);

        // Load fn_ptr and env_ptr from wrapper
        let fn_ptr = self.builder.load(ptr_ty, wrapper_param, "tramp.fn_ptr");
        let i8_ty = self.builder.i8_type();
        let eight = self.builder.const_i64(8);
        let env_slot = self
            .builder
            .gep(i8_ty, wrapper_param, &[eight], "tramp.env_slot");
        let env_ptr = self.builder.load(ptr_ty, env_slot, "tramp.env_ptr");

        // Load typed accumulator
        let acc_llvm_ty = self.type_resolver.resolve(acc_type);
        let acc_ty_id = self.builder.register_type(acc_llvm_ty);
        let acc_val = self.builder.load(acc_ty_id, acc_param, "tramp.acc");

        // Load typed element
        let elem_llvm_ty = self.type_resolver.resolve(elem_type);
        let elem_ty_id = self.builder.register_type(elem_llvm_ty);
        let elem_val = self.builder.load(elem_ty_id, elem_param, "tramp.elem");

        // Call closure: fastcc Acc @fn_ptr(ptr %env, Acc %acc, T %elem)
        let call_result = self.builder.call_indirect(
            acc_ty_id,
            &[ptr_ty, acc_ty_id, elem_ty_id],
            fn_ptr,
            &[env_ptr, acc_val, elem_val],
            "tramp.result",
        );

        // Store result to out_ptr
        if let Some(result) = call_result {
            self.builder.store(result, out_param);
        }

        self.builder.ret_void();

        self.current_function = saved_func;
        self.builder.set_current_function(saved_func);
        self.builder.restore_position(saved_pos);

        tramp
    }
}
