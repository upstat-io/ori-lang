//! Function call, invoke, and exception handling operations for `IrBuilder`.

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::module::Linkage;
use inkwell::types::{AnyType, BasicMetadataTypeEnum, BasicType};
use inkwell::values::BasicValueEnum;

use super::IrBuilder;
use crate::codegen::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

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
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build a direct function call marked as a tail call.
    ///
    /// Sets the `tail` attribute on the call instruction, which tells LLVM
    /// that this call is in tail position. Combined with `fastcc`, LLVM will
    /// perform tail call optimization (reusing the caller's stack frame).
    ///
    /// Returns `None` for void-returning functions.
    pub fn call_tail(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        name: &str,
    ) -> Option<ValueId> {
        let func = self.arena.get_function(callee);
        let arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .map(|&id| self.arena.get_value(id).into())
            .collect();
        let call_val = self
            .builder
            .build_call(func, &arg_vals, name)
            .expect("call_tail");
        call_val.set_tail_call(true);
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

    // -- Invoke (exception handling) --

    /// Build a direct invoke (call that may unwind).
    ///
    /// On normal return, execution continues at `then_block`.
    /// On unwind (exception), execution continues at `catch_block`.
    ///
    /// Returns `None` for void-returning functions, `Some(ValueId)` otherwise.
    /// The result value is only valid in `then_block`.
    pub fn invoke(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        then_block: BlockId,
        catch_block: BlockId,
        name: &str,
    ) -> Option<ValueId> {
        let func = self.arena.get_function(callee);
        let arg_vals: Vec<BasicValueEnum<'ctx>> =
            args.iter().map(|&id| self.arena.get_value(id)).collect();
        let then_bb = self.arena.get_block(then_block);
        let catch_bb = self.arena.get_block(catch_block);
        let call_val = self
            .builder
            .build_invoke(func, &arg_vals, then_bb, catch_bb, name)
            .expect("invoke");
        // inkwell's build_invoke does not automatically copy the calling
        // convention from the callee (unlike build_call). Without this,
        // fastcc callees get invoked with the default ccc, causing SIGSEGV.
        call_val.set_call_convention(func.get_call_conventions());
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build an indirect invoke through a function pointer.
    ///
    /// Like [`invoke`], but the callee is a function pointer with an
    /// explicit type signature.
    pub fn invoke_indirect(
        &mut self,
        return_type: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        args: &[ValueId],
        then_block: BlockId,
        catch_block: BlockId,
        name: &str,
    ) -> Option<ValueId> {
        let raw = self.arena.get_value(fn_ptr);
        if !raw.is_pointer_value() {
            tracing::error!(val_type = ?raw.get_type(), "invoke_indirect on non-pointer");
            self.record_codegen_error();
            return None;
        }
        let ptr = raw.into_pointer_value();
        let arg_vals: Vec<BasicValueEnum<'ctx>> =
            args.iter().map(|&id| self.arena.get_value(id)).collect();

        let ret_ty = self.arena.get_type(return_type);
        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let func_ty = ret_ty.fn_type(&param_tys, false);

        let then_bb = self.arena.get_block(then_block);
        let catch_bb = self.arena.get_block(catch_block);
        let call_val = self
            .builder
            .build_indirect_invoke(func_ty, ptr, &arg_vals, then_bb, catch_bb, name)
            .expect("invoke_indirect");
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build a `landingpad` instruction for exception handling cleanup.
    ///
    /// `personality` is the personality function (typically `__gxx_personality_v0`
    /// for C++/Rust Itanium EH ABI). `is_cleanup` should be `true` for cleanup
    /// pads that don't catch specific exceptions.
    ///
    /// Returns the landing pad value (an `{ i8*, i32 }` struct) as a `ValueId`.
    pub fn landingpad(&mut self, personality: FunctionId, is_cleanup: bool, name: &str) -> ValueId {
        let personality_fn = self.arena.get_function(personality);

        // Landing pad type is { ptr, i32 } (Itanium ABI convention).
        let i8_ptr_ty = self.scx.ptr_type;
        let i32_ty = self.scx.llcx.i32_type();
        let lp_ty = self
            .scx
            .llcx
            .struct_type(&[i8_ptr_ty.into(), i32_ty.into()], false);

        let lp_val = self
            .builder
            .build_landing_pad(lp_ty, personality_fn, &[], is_cleanup, name)
            .expect("landingpad");
        self.arena.push_value(lp_val)
    }

    /// Build a `resume` instruction to re-raise an exception.
    ///
    /// `value` must be the result of a `landingpad` instruction.
    /// This terminates the current basic block.
    pub fn resume(&mut self, value: ValueId) {
        let v = self.arena.get_value(value);
        self.builder.build_resume(v).expect("resume");
    }

    /// Set the personality function on an LLVM function.
    ///
    /// Required for any function containing `invoke`/`landingpad`.
    /// Typically `__gxx_personality_v0` (Itanium EH ABI on Linux/macOS).
    pub fn set_personality(&mut self, func: FunctionId, personality: FunctionId) {
        let func_val = self.arena.get_function(func);
        let personality_fn = self.arena.get_function(personality);
        func_val.set_personality_function(personality_fn);
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

    // -- Function attributes --

    /// Add the `nounwind` attribute to a function.
    ///
    /// Declares the function will not unwind (no exceptions). Enables LLVM
    /// to optimize exception handling paths around calls to this function.
    pub fn add_nounwind_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("nounwind");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `noinline` attribute to a function.
    ///
    /// Prevents LLVM from inlining this function. Used for cold paths like
    /// specialized drop functions and panic handlers.
    pub fn add_noinline_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noinline");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `cold` attribute to a function.
    ///
    /// Hints that this function is rarely called. LLVM uses this to:
    /// - Move cold code out of hot code layout
    /// - Reduce inlining priority
    /// - Optimize branch prediction away from cold paths
    pub fn add_cold_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("cold");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `noredzone` attribute to a function.
    ///
    /// Prevents the function from using the 128-byte red zone below `%rsp`
    /// (x86_64 SysV ABI). Required for JIT-compiled code where `fastcc`
    /// functions call into host runtime functions: the host functions create
    /// their own stack frames by pushing below `%rsp`, which can clobber
    /// red-zone data if the JIT code was using it.
    pub fn add_noredzone_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noredzone");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `noalias` attribute to a function's return value.
    ///
    /// Guarantees the returned pointer does not alias any other pointer
    /// visible to the caller. Used for allocation functions like `ori_rc_alloc`
    /// where the returned pointer is a fresh heap allocation.
    pub fn add_noalias_return_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noalias");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Return, attr);
    }

    /// Add the `memory(argmem: readwrite)` attribute to a function.
    ///
    /// Declares the function only reads/writes memory reachable from its pointer
    /// arguments (no global memory access, no inaccessible memory). This is
    /// critical for ARC runtime functions (`ori_rc_inc`, `ori_rc_dec`) which
    /// modify refcount at `ptr - 8` but don't access any other memory.
    ///
    /// # LLVM `MemoryEffects` Encoding
    ///
    /// The `memory` attribute uses a bitfield encoding from `ModRef.h`:
    /// - Bits \[1:0\]: `DefaultMem` access (None=0, Ref=1, Mod=2, ModRef=3)
    /// - Bits \[3:2\]: `ArgMem` access
    /// - Bits \[5:4\]: `InaccessibleMem` access
    ///
    /// `memory(argmem: readwrite)` = DefaultMem:None | ArgMem:ModRef | InaccessibleMem:None
    /// = 0 | (3 << 2) | (0 << 4) = 12
    pub fn add_memory_argmem_readwrite_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("memory");
        // MemoryEffects encoding: argmem: readwrite (ModRef=3 at ArgMem position bits [3:2])
        let attr = self.scx.llcx.create_enum_attribute(kind, 12);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `sret(T)` attribute to a function parameter.
    ///
    /// Marks the parameter as a hidden struct return pointer. LLVM uses
    /// this to optimize the return path and generate correct ABI code.
    pub fn add_sret_attribute(
        &mut self,
        func: FunctionId,
        param_index: u32,
        pointee_type: LLVMTypeId,
    ) {
        let f = self.arena.get_function(func);
        let ty = self.arena.get_type(pointee_type);
        let sret_kind = Attribute::get_named_enum_kind_id("sret");
        let sret_attr = self
            .scx
            .llcx
            .create_type_attribute(sret_kind, ty.as_any_type_enum());
        f.add_attribute(AttributeLoc::Param(param_index), sret_attr);
    }

    /// Add the `noalias` attribute to a function parameter.
    ///
    /// Guarantees the parameter pointer does not alias any other pointer
    /// visible to the callee. Required on sret parameters by the x86-64 ABI.
    pub fn add_noalias_attribute(&mut self, func: FunctionId, param_index: u32) {
        let f = self.arena.get_function(func);
        let noalias_kind = Attribute::get_named_enum_kind_id("noalias");
        let noalias_attr = self.scx.llcx.create_enum_attribute(noalias_kind, 0);
        f.add_attribute(AttributeLoc::Param(param_index), noalias_attr);
    }

    /// Add the `byval(T)` attribute to a function parameter.
    ///
    /// Indicates the parameter is passed by value on the stack. The callee
    /// receives a copy; modifications don't affect the caller's data.
    pub fn add_byval_attribute(
        &mut self,
        func: FunctionId,
        param_index: u32,
        pointee_type: LLVMTypeId,
    ) {
        let f = self.arena.get_function(func);
        let ty = self.arena.get_type(pointee_type);
        let byval_kind = Attribute::get_named_enum_kind_id("byval");
        let byval_attr = self
            .scx
            .llcx
            .create_type_attribute(byval_kind, ty.as_any_type_enum());
        f.add_attribute(AttributeLoc::Param(param_index), byval_attr);
    }
}
