//! Function call and method call lowering for V2 codegen.
//!
//! Handles direct calls, method dispatch (builtin + user-defined), and
//! exception-aware invoke helpers with cleanup landingpads.
//!
//! Related modules:
//! - `lower_lambdas` — lambda/closure compilation with capture analysis
//! - `lower_conversion_builtins` — `str()`, `int()`, `float()`, `byte()`, `assert_eq()`
//! - `lower_builtin_methods/` — built-in method dispatch

use ori_ir::canon::{CanExpr, CanId, CanRange};
use ori_ir::Name;
use ori_types::Idx;

use super::abi::{ParamPassing, ReturnPassing};
use super::expr_lowerer::ExprLowerer;
use super::scope::ScopeBinding;
use super::type_info::TypeInfo;
use super::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Direct call (positional args)
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::Call { func, args }`.
    ///
    /// Handles:
    /// 1. Built-in type conversions (`str()`, `int()`, `float()`, `byte()`)
    /// 2. Closure calls (if callee is a local binding)
    /// 3. Direct function calls via module lookup
    pub(crate) fn lower_call(&mut self, func: CanId, args: CanRange) -> Option<ValueId> {
        let func_kind = *self.canon.arena.kind(func);

        // Check if callee is a named function
        if let CanExpr::Ident(func_name) = func_kind {
            let name_str = self.resolve_name(func_name);

            // Built-in type conversion and testing functions
            match name_str {
                "str" => return self.lower_builtin_str(args),
                "int" => return self.lower_builtin_int(args),
                "float" => return self.lower_builtin_float(args),
                "byte" => return self.lower_builtin_byte(args),
                "assert_eq" => return self.lower_builtin_assert_eq(args),
                "hash_combine" => return self.lower_builtin_hash_combine(args),
                _ => {}
            }

            // Check if callee is a local binding (closure)
            if let Some(binding) = self.scope.lookup(func_name) {
                let callee_type = self.expr_type(func);
                return self.lower_closure_call(binding, args, callee_type);
            }

            // Look up in declared function map (has ABI info for sret)
            if let Some((func_id, abi)) = self.functions.get(&func_name) {
                return self.lower_abi_call(*func_id, abi, args);
            }

            // Look up in LLVM module (runtime functions, etc.)
            if let Some(llvm_func) = self.builder.scx().llmod.get_function(name_str) {
                let func_id = self.builder.intern_function(llvm_func);
                return self.lower_direct_call(func_id, args);
            }

            tracing::warn!(name = name_str, "unresolved function in call");
            self.builder.record_codegen_error();
            return None;
        }

        // Non-identifier callee (e.g., IIFE `(x -> x*2)(5)` or chained `f(1)(2)`)
        // The callee is a fat-pointer closure { fn_ptr, env_ptr }
        let callee_val = self.lower(func)?;

        // Extract fn_ptr and env_ptr from fat pointer
        let fn_ptr = self.builder.extract_value(callee_val, 0, "callee.fn_ptr")?;
        let env_ptr = self
            .builder
            .extract_value(callee_val, 1, "callee.env_ptr")?;

        // Compile args with env_ptr prepended
        let arg_ids = self.canon.arena.get_expr_list(args);
        let mut arg_vals = Vec::with_capacity(arg_ids.len() + 1);
        arg_vals.push(env_ptr);
        for &arg_id in arg_ids {
            arg_vals.push(self.lower(arg_id)?);
        }

        // Get actual types from TypeInfo
        let callee_type = self.expr_type(func);
        let type_info = self.type_info.get(callee_type);
        if let TypeInfo::Function { params, ret } = &type_info {
            let ptr_ty = self.builder.ptr_type();
            let mut call_param_types = Vec::with_capacity(1 + params.len());
            call_param_types.push(ptr_ty);
            for &idx in params {
                call_param_types.push(self.resolve_type(idx));
            }
            let ret_ty = self.resolve_type(*ret);
            self.builder
                .call_indirect(ret_ty, &call_param_types, fn_ptr, &arg_vals, "call")
        } else {
            // Fallback: treat as ptr + i64 args
            let i64_ty = self.builder.i64_type();
            let ptr_ty = self.builder.ptr_type();
            let mut param_tys = vec![ptr_ty];
            param_tys.extend(arg_ids.iter().map(|_| i64_ty));
            self.builder
                .call_indirect(i64_ty, &param_tys, fn_ptr, &arg_vals, "call")
        }
    }

    /// Lower a direct function call with positional arguments.
    fn lower_direct_call(&mut self, func_id: FunctionId, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let mut arg_vals = Vec::with_capacity(arg_ids.len());
        for &arg_id in arg_ids {
            arg_vals.push(self.lower(arg_id)?);
        }

        self.builder.call(func_id, &arg_vals, "call")
    }

    /// Lower a call to a function with known ABI (sret + borrow-aware).
    ///
    /// Handles three parameter passing modes:
    /// - `Direct`: pass value as-is
    /// - `Indirect`: pass value as-is (already a pointer from caller)
    /// - `Reference`: create stack alloca, store value, pass pointer (no RC)
    ///
    /// Uses `call_with_sret` for functions that return large types via
    /// hidden pointer parameter, and regular `call` for direct returns.
    fn lower_abi_call(
        &mut self,
        func_id: FunctionId,
        abi: &super::abi::FunctionAbi,
        args: CanRange,
    ) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let mut raw_arg_vals = Vec::with_capacity(arg_ids.len());
        for &arg_id in arg_ids {
            raw_arg_vals.push(self.lower(arg_id)?);
        }

        // Build final argument list, respecting passing modes
        let arg_vals = self.apply_param_passing(&raw_arg_vals, &abi.params);

        match &abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(abi.return_abi.ty);
                self.invoke_user_function_sret(func_id, &arg_vals, ret_ty, "call")
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.invoke_user_function(func_id, &arg_vals, "call")
            }
        }
    }

    /// Lower a closure call via fat-pointer dispatch.
    ///
    /// Closures are `{ fn_ptr: ptr, env_ptr: ptr }`. Calling convention:
    /// 1. Extract `fn_ptr` and `env_ptr` from the fat pointer
    /// 2. Prepend `env_ptr` as the first argument
    /// 3. Call indirectly through `fn_ptr` with actual types
    fn lower_closure_call(
        &mut self,
        binding: ScopeBinding,
        args: CanRange,
        callee_type: Idx,
    ) -> Option<ValueId> {
        let closure_val = match binding {
            ScopeBinding::Immutable(val) => val,
            ScopeBinding::Mutable { ptr, ty } => self.builder.load(ty, ptr, "closure"),
        };

        // Extract fn_ptr and env_ptr from fat pointer
        let fn_ptr = self
            .builder
            .extract_value(closure_val, 0, "closure.fn_ptr")?;
        let env_ptr = self
            .builder
            .extract_value(closure_val, 1, "closure.env_ptr")?;

        // Compile arguments
        let arg_ids = self.canon.arena.get_expr_list(args);
        let mut arg_vals = Vec::with_capacity(arg_ids.len() + 1);
        arg_vals.push(env_ptr); // Hidden env_ptr as first arg
        for &arg_id in arg_ids {
            arg_vals.push(self.lower(arg_id)?);
        }

        // Get actual param types and return type from TypeInfo
        let type_info = self.type_info.get(callee_type);
        let (param_idxs, ret_idx) = if let TypeInfo::Function { params, ret } = &type_info {
            (params.clone(), *ret)
        } else {
            tracing::warn!(?type_info, "closure call on non-Function type");
            let param_types = vec![Idx::INT; arg_ids.len()];
            (param_types, Idx::INT)
        };

        // Build LLVM call param types: [ptr (env), ...actual_params]
        let ptr_ty = self.builder.ptr_type();
        let mut call_param_types = Vec::with_capacity(1 + param_idxs.len());
        call_param_types.push(ptr_ty);
        for &idx in &param_idxs {
            let llvm_ty = self.type_resolver.resolve(idx);
            call_param_types.push(self.builder.register_type(llvm_ty));
        }

        let ret_llvm_ty = self.type_resolver.resolve(ret_idx);
        let ret_ty_id = self.builder.register_type(ret_llvm_ty);

        self.builder.call_indirect(
            ret_ty_id,
            &call_param_types,
            fn_ptr,
            &arg_vals,
            "closure_call",
        )
    }

    // -----------------------------------------------------------------------
    // Method call
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::MethodCall { receiver, method, args }`.
    ///
    /// Dispatch order:
    /// 0. Static method calls (`Type.method()`) — receiver is a type, not a value
    /// 1. Built-in methods (type-specific, inline codegen)
    /// 2. Type-qualified method lookup via `method_functions[(type_name, method)]`
    /// 3. Bare-name function map fallback (`functions[method]`)
    /// 4. LLVM module lookup (runtime functions)
    pub(crate) fn lower_method_call(
        &mut self,
        receiver: CanId,
        method: Name,
        args: CanRange,
    ) -> Option<ValueId> {
        // 0. Static method calls: receiver is a TypeRef (e.g., `Point.default()`)
        //    These have no `self` parameter — call without prepending receiver.
        let recv_kind = *self.canon.arena.kind(receiver);
        if let CanExpr::TypeRef(_) = recv_kind {
            return self.lower_static_method_call(receiver, method, args);
        }

        let recv_type = self.expr_type(receiver);
        let recv_val = self.lower(receiver)?;

        // 1. Try built-in method dispatch
        let method_str = self.resolve_name(method).to_owned();
        if let Some(result) = self.lower_builtin_method(recv_val, recv_type, &method_str, args) {
            return Some(result);
        }

        // 2. Type-qualified method lookup: resolve receiver Idx → type Name,
        //    then look up (type_name, method_name) in method_functions
        if let Some(&type_name) = self.type_idx_to_name.get(&recv_type) {
            if let Some((func_id, abi)) = self.method_functions.get(&(type_name, method)) {
                let func_id = *func_id;
                let abi = abi.clone();
                return self.emit_method_call(func_id, &abi, recv_val, args, "method_call");
            }
        }

        // 3. Bare-name fallback: check function map (sret-aware)
        if let Some((func_id, abi)) = self.functions.get(&method) {
            let func_id = *func_id;
            let abi = abi.clone();
            return self.emit_method_call(func_id, &abi, recv_val, args, "method_call");
        }

        // 4. LLVM module lookup (runtime functions, etc.)
        if let Some(llvm_func) = self.builder.scx().llmod.get_function(&method_str) {
            let func_id = self.builder.intern_function(llvm_func);

            let arg_ids = self.canon.arena.get_expr_list(args);
            let mut all_args = Vec::with_capacity(arg_ids.len() + 1);
            all_args.push(recv_val);
            for &arg_id in arg_ids {
                all_args.push(self.lower(arg_id)?);
            }

            return self.builder.call(func_id, &all_args, "method_call");
        }

        tracing::warn!(
            method = %method_str,
            ?recv_type,
            "unresolved method call"
        );
        self.builder.record_codegen_error();
        None
    }

    /// Lower a static method call (`Type.method(args)`).
    ///
    /// Static methods have no `self` parameter — the receiver is a type
    /// reference, not a value. Used for factory methods like `Type.default()`.
    fn lower_static_method_call(
        &mut self,
        receiver: CanId,
        method: Name,
        args: CanRange,
    ) -> Option<ValueId> {
        let recv_type = self.expr_type(receiver);

        // Look up method in method_functions using the type name
        if let Some(&type_name) = self.type_idx_to_name.get(&recv_type) {
            if let Some((func_id, abi)) = self.method_functions.get(&(type_name, method)) {
                let func_id = *func_id;
                let abi = abi.clone();
                return self.emit_static_call(func_id, &abi, args, "static_method");
            }
        }

        let method_str = self.resolve_name(method);
        tracing::warn!(
            method = %method_str,
            ?recv_type,
            "unresolved static method call"
        );
        self.builder.record_codegen_error();
        None
    }

    // -----------------------------------------------------------------------
    // Method call emission helpers
    // -----------------------------------------------------------------------

    /// Emit a static method call (no receiver/self), handling sret + borrow passing.
    ///
    /// Used for factory methods like `Type.default()` where the receiver is a
    /// type reference, not a value to pass as the first argument.
    fn emit_static_call(
        &mut self,
        func_id: FunctionId,
        abi: &super::abi::FunctionAbi,
        args: CanRange,
        name: &str,
    ) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let mut raw_args = Vec::with_capacity(arg_ids.len());
        for &arg_id in arg_ids {
            raw_args.push(self.lower(arg_id)?);
        }

        // Apply param passing modes (Reference → alloca + store + pass ptr)
        let all_args = self.apply_param_passing(&raw_args, &abi.params);

        match &abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(abi.return_abi.ty);
                self.invoke_user_function_sret(func_id, &all_args, ret_ty, name)
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.invoke_user_function(func_id, &all_args, name)
            }
        }
    }

    /// Emit a method call with positional args, handling sret + borrow passing.
    ///
    /// Used by both type-qualified and bare-name method dispatch to avoid
    /// duplicating the receiver-prepend + sret logic.
    fn emit_method_call(
        &mut self,
        func_id: FunctionId,
        abi: &super::abi::FunctionAbi,
        recv_val: ValueId,
        args: CanRange,
        name: &str,
    ) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let mut raw_args = Vec::with_capacity(arg_ids.len() + 1);
        raw_args.push(recv_val);
        for &arg_id in arg_ids {
            raw_args.push(self.lower(arg_id)?);
        }

        // Apply param passing modes (Reference → alloca + store + pass ptr)
        let all_args = self.apply_param_passing(&raw_args, &abi.params);

        match &abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(abi.return_abi.ty);
                self.invoke_user_function_sret(func_id, &all_args, ret_ty, name)
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.invoke_user_function(func_id, &all_args, name)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Exception-aware call helpers
    // -----------------------------------------------------------------------

    /// Ensure the personality function is set on the current LLVM function.
    ///
    /// Looks up `rust_eh_personality` from the LLVM module, interns it,
    /// and sets it as the personality function on the current function.
    /// Idempotent — calling multiple times on the same function is safe.
    ///
    /// If `rust_eh_personality` is not found (meaning `declare_runtime()` was
    /// not called), declares it inline as a fallback so codegen can proceed
    /// without crashing.
    fn ensure_personality(&mut self) -> FunctionId {
        let scx = self.builder.scx();
        let personality_fn = if let Some(f) = scx.llmod.get_function("rust_eh_personality") {
            f
        } else {
            tracing::error!(
                "rust_eh_personality not declared — declare_runtime() should be called first"
            );
            // Declare inline as fallback so codegen can proceed.
            let i32_ty = scx.type_i32();
            scx.llmod.add_function(
                "rust_eh_personality",
                i32_ty.fn_type(&[i32_ty.into()], false),
                Some(inkwell::module::Linkage::External),
            )
        };
        let personality_id = self.builder.intern_function(personality_fn);
        self.builder
            .set_personality(self.current_function, personality_id);
        personality_id
    }

    /// Emit an `invoke` to a user-defined function with a cleanup landingpad.
    ///
    /// User-defined functions may panic (unwind via Rust's panic infrastructure).
    /// Using `invoke` instead of `call` gives LLVM correct unwind edges so
    /// cleanup code (RC decrements) can run during stack unwinding.
    ///
    /// The cleanup landingpad currently re-raises immediately. RC cleanup
    /// will be inserted here once cross-block liveness analysis is wired.
    pub(crate) fn invoke_user_function(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        name: &str,
    ) -> Option<ValueId> {
        let personality = self.ensure_personality();

        let normal_bb = self
            .builder
            .append_block(self.current_function, &format!("{name}.cont"));
        let unwind_bb = self
            .builder
            .append_block(self.current_function, &format!("{name}.unwind"));

        let result = self
            .builder
            .invoke(func_id, args, normal_bb, unwind_bb, name);

        // Build cleanup landingpad: catch-all cleanup, re-raise
        self.builder.position_at_end(unwind_bb);
        let lp = self.builder.landingpad(personality, true, "lp");
        self.builder.resume(lp);

        // Continue in normal block
        self.builder.position_at_end(normal_bb);

        result
    }

    /// Emit an `invoke` with sret return convention and a cleanup landingpad.
    ///
    /// Like [`invoke_user_function`] but for functions returning via hidden
    /// sret pointer. The sret alloca is in the entry block, the invoke
    /// branches to normal/unwind, and the load happens in the normal block.
    pub(crate) fn invoke_user_function_sret(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        sret_type: LLVMTypeId,
        name: &str,
    ) -> Option<ValueId> {
        let personality = self.ensure_personality();

        let sret_ptr = self.builder.create_entry_alloca(
            self.current_function,
            &format!("{name}.sret"),
            sret_type,
        );

        let mut full_args = Vec::with_capacity(args.len() + 1);
        full_args.push(sret_ptr);
        full_args.extend_from_slice(args);

        let normal_bb = self
            .builder
            .append_block(self.current_function, &format!("{name}.cont"));
        let unwind_bb = self
            .builder
            .append_block(self.current_function, &format!("{name}.unwind"));

        // Invoke the sret function (void return — result is stored through sret pointer)
        self.builder
            .invoke(func_id, &full_args, normal_bb, unwind_bb, "");

        // Build cleanup landingpad
        self.builder.position_at_end(unwind_bb);
        let lp = self.builder.landingpad(personality, true, "lp");
        self.builder.resume(lp);

        // Continue in normal block and load result
        self.builder.position_at_end(normal_bb);
        let result = self.builder.load(sret_type, sret_ptr, name);
        Some(result)
    }

    // -----------------------------------------------------------------------
    // Parameter passing mode application
    // -----------------------------------------------------------------------

    /// Apply parameter passing modes to argument values.
    ///
    /// For `Reference` parameters: creates a stack alloca in the *caller*'s
    /// entry block, stores the value, and returns the pointer. The callee
    /// receives a borrowed pointer with no RC operations.
    ///
    /// For `Direct`/`Indirect`: passes through as-is.
    /// For `Void`: skips the parameter.
    // SYNC: also update ArcIrEmitter::apply_param_passing in arc_emitter.rs
    pub(crate) fn apply_param_passing(
        &mut self,
        raw_args: &[ValueId],
        param_abis: &[super::abi::ParamAbi],
    ) -> Vec<ValueId> {
        let caller_func = self.current_function;
        let mut result = Vec::with_capacity(raw_args.len());
        let mut arg_idx = 0;

        for param_abi in param_abis {
            if arg_idx >= raw_args.len() {
                break;
            }

            match &param_abi.passing {
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    // Indirect or borrowed: create alloca in caller's entry, store value, pass pointer
                    let param_ty = self.type_resolver.resolve(param_abi.ty);
                    let param_ty_id = self.builder.register_type(param_ty);
                    let alloca =
                        self.builder
                            .create_entry_alloca(caller_func, "ref_arg", param_ty_id);
                    self.builder.store(raw_args[arg_idx], alloca);
                    result.push(alloca);
                    arg_idx += 1;
                }
                ParamPassing::Direct => {
                    result.push(raw_args[arg_idx]);
                    arg_idx += 1;
                }
                ParamPassing::Void => {
                    // Void params are not physically passed — skip
                }
            }
        }

        // If there are more args than ABI params (shouldn't happen in
        // well-typed code), pass remaining args directly
        while arg_idx < raw_args.len() {
            result.push(raw_args[arg_idx]);
            arg_idx += 1;
        }

        result
    }
}
