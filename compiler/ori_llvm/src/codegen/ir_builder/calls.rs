//! Function call and declaration operations for `IrBuilder`.
//!
//! Contains direct call emission (call, tail call, indirect call, sret),
//! function declaration, and calling convention methods.
//!
//! For invoke/exception handling, see [`super::invoke`].
//! For function/parameter attributes, see [`super::attributes`].

use inkwell::module::Linkage;
use inkwell::types::{BasicMetadataTypeEnum, BasicType};

use super::IrBuilder;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'ctx> IrBuilder<'_, 'ctx> {
    // -- Direct calls --

    /// Build a direct function call.
    ///
    /// Returns `None` for void-returning functions.
    pub fn call(&mut self, callee: FunctionId, args: &[ValueId], name: &str) -> Option<ValueId> {
        let func = self.arena.get_function(callee);
        let arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .map(|&id| self.arena.get_value(id).into())
            .collect();
        let call_val = self
            .builder
            .build_call(func, &arg_vals, name)
            .expect("call");
        // Explicitly copy the callee's calling convention to the call instruction.
        // inkwell's build_call does NOT reliably propagate fastcc from the callee
        // function — without this, nounwind-downgraded invokes lose their fastcc,
        // causing calling convention mismatches and wrong results.
        call_val.set_call_convention(func.get_call_conventions());
        self.last_call_site = Some(call_val);
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build an indirect call through a function pointer.
    ///
    /// `return_type` is the function's return type; `param_types` are the
    /// parameter types. These are used to construct the LLVM function type
    /// needed for the indirect call.
    ///
    /// Returns `None` for void-returning functions.
    pub fn call_indirect(
        &mut self,
        return_type: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        args: &[ValueId],
        name: &str,
    ) -> Option<ValueId> {
        let raw = self.arena.get_value(fn_ptr);
        if !raw.is_pointer_value() {
            tracing::error!(val_type = ?raw.get_type(), "call_indirect on non-pointer");
            self.record_codegen_error();
            return None;
        }
        let ptr = raw.into_pointer_value();
        let arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .map(|&id| self.arena.get_value(id).into())
            .collect();

        let ret_ty = self.arena.get_type(return_type);
        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let func_ty = ret_ty.fn_type(&param_tys, false);

        let call_val = self
            .builder
            .build_indirect_call(func_ty, ptr, &arg_vals, name)
            .expect("call_indirect");
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Indirect call to a void-returning function through a function pointer.
    ///
    /// Used by trampolines calling closures that use sret ABI (the closure
    /// writes its result through a pointer parameter and returns void).
    pub fn call_indirect_void(
        &mut self,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        args: &[ValueId],
    ) {
        let raw = self.arena.get_value(fn_ptr);
        if !raw.is_pointer_value() {
            tracing::error!(val_type = ?raw.get_type(), "call_indirect_void on non-pointer");
            self.record_codegen_error();
            return;
        }
        let ptr = raw.into_pointer_value();
        let arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .map(|&id| self.arena.get_value(id).into())
            .collect();

        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let fn_type = self.scx.type_void_func(&param_tys);

        self.builder
            .build_indirect_call(fn_type, ptr, &arg_vals, "")
            .expect("call_indirect_void");
    }

    // -- sret call helper --

    /// Build a call to an sret function, hiding the ABI complexity.
    ///
    /// For functions using the sret convention:
    /// 1. Allocates stack space for the return value
    /// 2. Prepends the sret pointer as the first argument
    /// 3. Calls the void function
    /// 4. Loads the result from the sret pointer
    ///
    /// Returns the loaded result value, making sret transparent to callers.
    pub fn call_with_sret(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        sret_type: LLVMTypeId,
        name: &str,
    ) -> Option<ValueId> {
        let func = self
            .current_function
            .expect("call_with_sret requires active function");

        // Allocate stack space at entry block for the return value
        let sret_ptr = self.create_entry_alloca(func, &format!("{name}.sret"), sret_type);

        // Prepend sret pointer to args
        let mut full_args = Vec::with_capacity(args.len() + 1);
        full_args.push(sret_ptr);
        full_args.extend_from_slice(args);

        // Call the void function (sret functions always return void)
        self.call(callee, &full_args, "");

        // Load the result from the sret pointer
        let result = self.load(sret_type, sret_ptr, name);
        Some(result)
    }

    // -- Function declaration --

    /// Declare a function in the LLVM module.
    pub fn declare_function(
        &mut self,
        name: &str,
        param_types: &[LLVMTypeId],
        return_type: LLVMTypeId,
    ) -> FunctionId {
        let ret_ty = self.arena.get_type(return_type);
        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let fn_type = ret_ty.fn_type(&param_tys, false);
        let func = self.scx.llmod.add_function(name, fn_type, None);
        self.arena.push_function(func)
    }

    /// Declare a void-returning function in the LLVM module.
    pub fn declare_void_function(&mut self, name: &str, param_types: &[LLVMTypeId]) -> FunctionId {
        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let fn_type = self.scx.type_void_func(&param_tys);
        let func = self.scx.llmod.add_function(name, fn_type, None);
        self.arena.push_function(func)
    }

    /// Declare an external function with `External` linkage.
    ///
    /// Used for runtime library functions (`ori_print`, `ori_panic`, etc.)
    /// and imported functions from other modules. Supports void return
    /// (pass `None` for `return_type`).
    pub fn declare_extern_function(
        &mut self,
        name: &str,
        param_types: &[LLVMTypeId],
        return_type: Option<LLVMTypeId>,
    ) -> FunctionId {
        // Reuse existing declaration if present
        if let Some(func) = self.scx.llmod.get_function(name) {
            return self.arena.push_function(func);
        }

        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();

        let fn_type = match return_type {
            Some(ret_id) => {
                let ret_ty = self.arena.get_type(ret_id);
                ret_ty.fn_type(&param_tys, false)
            }
            None => self.scx.type_void_func(&param_tys),
        };

        let func = self
            .scx
            .llmod
            .add_function(name, fn_type, Some(Linkage::External));
        self.arena.push_function(func)
    }

    /// Get or declare a function by name.
    ///
    /// If the function already exists in the module, registers it in the
    /// arena and returns its ID. Otherwise declares a new function.
    pub fn get_or_declare_function(
        &mut self,
        name: &str,
        param_types: &[LLVMTypeId],
        return_type: LLVMTypeId,
    ) -> FunctionId {
        if let Some(func) = self.scx.llmod.get_function(name) {
            self.arena.push_function(func)
        } else {
            self.declare_function(name, param_types, return_type)
        }
    }

    /// Get or declare a void-returning function by name.
    ///
    /// If the function already exists in the module, registers it in the
    /// arena and returns its ID. Otherwise declares a new void function.
    pub fn get_or_declare_void_function(
        &mut self,
        name: &str,
        param_types: &[LLVMTypeId],
    ) -> FunctionId {
        if let Some(func) = self.scx.llmod.get_function(name) {
            self.arena.push_function(func)
        } else {
            self.declare_void_function(name, param_types)
        }
    }

    /// Get a function's address as a pointer `ValueId`.
    ///
    /// Used for passing function pointers to runtime calls (e.g., registering
    /// the panic handler trampoline).
    pub fn get_function_ptr(&mut self, func: FunctionId) -> ValueId {
        let func_val = self.arena.get_function(func);
        let ptr_val = func_val.as_global_value().as_pointer_value();
        self.arena.push_value(ptr_val.into())
    }

    /// Check whether a function already has a body (basic blocks).
    ///
    /// Used to avoid regenerating auxiliary functions (compare thunks, drop
    /// functions) that may already exist in the LLVM module from a previous
    /// emitter instance.
    pub fn function_has_body(&self, func: FunctionId) -> bool {
        let func_val = self.arena.get_function(func);
        func_val.count_basic_blocks() > 0
    }

    // -- Calling conventions --

    /// Set the calling convention on a function.
    ///
    /// Convention IDs: 0 = C, 8 = fastcc. See LLVM CallingConv.h.
    pub fn set_calling_convention(&mut self, func: FunctionId, conv: u32) {
        let f = self.arena.get_function(func);
        f.set_call_conventions(conv);
    }

    /// Set `fastcc` calling convention on a function.
    ///
    /// Internal Ori functions use `fastcc` for better optimization (tail calls,
    /// non-standard register allocation).
    pub fn set_fastcc(&mut self, func: FunctionId) {
        self.set_calling_convention(func, 8); // LLVM FastCC = 8
    }

    /// Set C calling convention on a function.
    ///
    /// Used for `@main`, extern functions, and runtime library calls.
    pub fn set_ccc(&mut self, func: FunctionId) {
        self.set_calling_convention(func, 0); // LLVM CCC = 0
    }
}
