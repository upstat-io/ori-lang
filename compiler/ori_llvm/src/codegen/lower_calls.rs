//! Function call, method call, and lambda lowering for V2 codegen.
//!
//! Handles direct calls, method dispatch (builtin + user-defined), and
//! lambda/closure compilation with capture analysis.

use ori_ir::canon::{CanExpr, CanId, CanParamRange, CanRange};
use ori_ir::Name;
use ori_types::Idx;

use crate::aot::mangle::Mangler;

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
    // Method call emission helper
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

    // -----------------------------------------------------------------------
    // Built-in method dispatch
    // -----------------------------------------------------------------------

    /// Dispatch built-in methods based on receiver type.
    ///
    /// Returns `None` if the method is not a built-in, allowing fallthrough
    /// to user-defined method lookup.
    fn lower_builtin_method(
        &mut self,
        recv_val: ValueId,
        recv_type: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match recv_type {
            Idx::INT | Idx::DURATION | Idx::SIZE => self.lower_int_method(recv_val, method, args),
            Idx::FLOAT => self.lower_float_method(recv_val, method, args),
            Idx::BOOL => self.lower_bool_method(recv_val, method, args),
            Idx::ORDERING => self.lower_ordering_method(recv_val, method),
            Idx::STR => self.lower_str_method(recv_val, method, args),
            // Scalar value types: clone is identity (bitwise copy)
            Idx::BYTE | Idx::CHAR if method == "clone" => Some(recv_val),
            _ => {
                // Check for option/result methods
                let type_info = self.type_info.get(recv_type);
                match &type_info {
                    TypeInfo::Option { .. } => self.lower_option_method(recv_val, method, args),
                    TypeInfo::Result { ok, .. } => {
                        self.lower_result_method(recv_val, *ok, method, args)
                    }
                    TypeInfo::List { .. } => {
                        self.lower_list_method(recv_val, recv_type, method, args)
                    }
                    // Map/Set are ARC-managed {len, cap, ptr} — clone is identity
                    TypeInfo::Map { .. } | TypeInfo::Set { .. } if method == "clone" => {
                        Some(recv_val)
                    }
                    // Tuple is a value type {A, B, ...} — clone is identity
                    TypeInfo::Tuple { .. } if method == "clone" => Some(recv_val),
                    // Tuple.len() is a compile-time constant
                    TypeInfo::Tuple { elements } if method == "len" => {
                        Some(self.builder.const_i64(elements.len() as i64))
                    }
                    _ => None,
                }
            }
        }
    }

    /// Built-in int methods.
    fn lower_int_method(&mut self, recv: ValueId, method: &str, args: CanRange) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                // Three-way comparison: returns Ordering (i8)
                // Less=0 if self < other, Equal=1 if self == other, Greater=2 if self > other
                let lt = self.builder.icmp_slt(recv, other, "cmp.lt");
                let gt = self.builder.icmp_sgt(recv, other, "cmp.gt");
                let zero = self.builder.const_i8(0); // Less
                let one = self.builder.const_i8(1); // Equal
                let two = self.builder.const_i8(2); // Greater
                let gt_or_eq = self.builder.select(gt, two, one, "cmp.gt_or_eq");
                Some(self.builder.select(lt, zero, gt_or_eq, "cmp.result"))
            }
            "abs" => {
                // abs(x) = x < 0 ? -x : x
                let zero = self.builder.const_i64(0);
                let is_neg = self.builder.icmp_slt(recv, zero, "abs.neg");
                let negated = self.builder.neg(recv, "abs.negated");
                Some(self.builder.select(is_neg, negated, recv, "abs"))
            }
            // Value types: clone/hash are identity operations
            "clone" | "hash" => Some(recv),
            _ => None,
        }
    }

    /// Built-in float methods.
    fn lower_float_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                let lt = self.builder.fcmp_olt(recv, other, "fcmp.lt");
                let gt = self.builder.fcmp_ogt(recv, other, "fcmp.gt");
                let zero = self.builder.const_i8(0);
                let one = self.builder.const_i8(1);
                let two = self.builder.const_i8(2);
                let gt_or_eq = self.builder.select(gt, two, one, "fcmp.gt_or_eq");
                Some(self.builder.select(lt, zero, gt_or_eq, "fcmp.result"))
            }
            "abs" => {
                let zero = self.builder.const_f64(0.0);
                let is_neg = self.builder.fcmp_olt(recv, zero, "fabs.neg");
                let negated = self.builder.fneg(recv, "fabs.negated");
                Some(self.builder.select(is_neg, negated, recv, "fabs"))
            }
            // Value type: clone is identity
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// Built-in bool methods.
    fn lower_bool_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                // false < true
                let i8_ty = self.builder.i8_type();
                let i8_self = self.builder.zext(recv, i8_ty, "b2i8.self");
                let i8_ty2 = self.builder.i8_type();
                let i8_other = self.builder.zext(other, i8_ty2, "b2i8.other");
                let lt = self.builder.icmp_ult(i8_self, i8_other, "bcmp.lt");
                let gt = self.builder.icmp_ugt(i8_self, i8_other, "bcmp.gt");
                let zero = self.builder.const_i8(0);
                let one = self.builder.const_i8(1);
                let two = self.builder.const_i8(2);
                let gt_or_eq = self.builder.select(gt, two, one, "bcmp.gt_or_eq");
                Some(self.builder.select(lt, zero, gt_or_eq, "bcmp.result"))
            }
            // Value type: clone is identity
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// Built-in Ordering methods.
    fn lower_ordering_method(&mut self, recv: ValueId, method: &str) -> Option<ValueId> {
        let less = self.builder.const_i8(0);
        let equal = self.builder.const_i8(1);
        let greater = self.builder.const_i8(2);

        match method {
            "is_less" => Some(self.builder.icmp_eq(recv, less, "ord.is_less")),
            "is_equal" => Some(self.builder.icmp_eq(recv, equal, "ord.is_equal")),
            "is_greater" => Some(self.builder.icmp_eq(recv, greater, "ord.is_greater")),
            "is_less_or_equal" => Some(self.builder.icmp_ne(recv, greater, "ord.is_le")),
            "is_greater_or_equal" => Some(self.builder.icmp_ne(recv, less, "ord.is_ge")),
            "reverse" => {
                // 2 - tag: Less(0)↔Greater(2), Equal(1) unchanged
                Some(self.builder.sub(greater, recv, "ord.reverse"))
            }
            "equals" | "clone" | "hash" => {
                // Identity operations
                Some(recv)
            }
            _ => None,
        }
    }

    /// Built-in string methods.
    fn lower_str_method(
        &mut self,
        recv: ValueId,
        method: &str,
        _args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "len" | "length" => self.builder.extract_value(recv, 0, "str.len"),
            "is_empty" => {
                let len = self.builder.extract_value(recv, 0, "str.len")?;
                let zero = self.builder.const_i64(0);
                Some(self.builder.icmp_eq(len, zero, "str.is_empty"))
            }
            // Strings are immutable {len, ptr} — clone is identity (ARC shares data)
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// Built-in Option methods.
    fn lower_option_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "opt.tag")?;
        let zero = self.builder.const_i8(0);

        match method {
            "is_some" => Some(self.builder.icmp_ne(tag, zero, "opt.is_some")),
            "is_none" => Some(self.builder.icmp_eq(tag, zero, "opt.is_none")),
            "unwrap" => {
                // Extract payload (no runtime check in this simplified version)
                self.builder.extract_value(recv, 1, "opt.unwrap")
            }
            "unwrap_or" => {
                // Some(v) → v, None → default
                let is_some = self.builder.icmp_ne(tag, zero, "opt.is_some");
                let payload = self.builder.extract_value(recv, 1, "opt.payload")?;
                let arg_ids = self.canon.arena.get_expr_list(args);
                let default_val = self.lower(*arg_ids.first()?)?;
                Some(
                    self.builder
                        .select(is_some, payload, default_val, "opt.unwrap_or"),
                )
            }
            // Option is a value type {i8, T} — clone is identity (bitwise copy)
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// Built-in Result methods.
    ///
    /// For `unwrap()`, the extracted payload is the `max(ok, err)` slot.
    /// When ok and err have different sizes, the payload must be coerced
    /// to the actual ok type via alloca reinterpretation.
    ///
    /// `ok_type` is passed from the dispatch site (`lower_builtin_method`)
    /// which already destructured `TypeInfo::Result { ok, .. }`, avoiding
    /// a redundant `TypeInfoStore::get` call.
    fn lower_result_method(
        &mut self,
        recv: ValueId,
        ok_type: Idx,
        method: &str,
        _args: CanRange,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "res.tag")?;
        let zero = self.builder.const_i8(0);

        match method {
            "is_ok" => Some(self.builder.icmp_eq(tag, zero, "res.is_ok")),
            "is_err" => Some(self.builder.icmp_ne(tag, zero, "res.is_err")),
            "unwrap" => {
                let payload = self.builder.extract_value(recv, 1, "res.unwrap")?;
                // Coerce payload to ok type (payload slot may be larger than ok type
                // due to Result's max(ok, err) layout)
                Some(self.coerce_payload(payload, ok_type))
            }
            // Result is a value type {i8, max(T, E)} — clone is identity (bitwise copy)
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// Built-in List methods.
    fn lower_list_method(
        &mut self,
        recv: ValueId,
        _recv_type: Idx,
        method: &str,
        _args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "len" | "length" => self.builder.extract_value(recv, 0, "list.len"),
            "is_empty" => {
                let len = self.builder.extract_value(recv, 0, "list.len")?;
                let zero = self.builder.const_i64(0);
                Some(self.builder.icmp_eq(len, zero, "list.is_empty"))
            }
            // List is {len, cap, ptr} — clone is identity (ARC shares data)
            "clone" => Some(recv),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Built-in type conversion functions
    // -----------------------------------------------------------------------

    /// Lower `str(expr)` — convert value to string.
    fn lower_builtin_str(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let arg_id = *arg_ids.first()?;
        let val = self.lower(arg_id)?;
        let arg_type = self.expr_type(arg_id);

        let str_ty = self.resolve_type(Idx::STR);
        let i64_ty = self.builder.i64_type();
        let f64_ty = self.builder.f64_type();
        let bool_ty = self.builder.bool_type();

        match arg_type {
            Idx::INT => {
                let func =
                    self.builder
                        .get_or_declare_function("ori_str_from_int", &[i64_ty], str_ty);
                self.builder.call(func, &[val], "str_from_int")
            }
            Idx::FLOAT => {
                let func =
                    self.builder
                        .get_or_declare_function("ori_str_from_float", &[f64_ty], str_ty);
                self.builder.call(func, &[val], "str_from_float")
            }
            Idx::BOOL => {
                let func =
                    self.builder
                        .get_or_declare_function("ori_str_from_bool", &[bool_ty], str_ty);
                self.builder.call(func, &[val], "str_from_bool")
            }
            _ => {
                tracing::warn!(?arg_type, "str() conversion for unsupported type");
                self.builder.record_codegen_error();
                None
            }
        }
    }

    /// Lower `int(expr)` — convert value to int.
    fn lower_builtin_int(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let arg_id = *arg_ids.first()?;
        let val = self.lower(arg_id)?;
        let arg_type = self.expr_type(arg_id);

        match arg_type {
            Idx::FLOAT => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.fp_to_si(val, i64_ty, "float2int"))
            }
            Idx::BOOL => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.zext(val, i64_ty, "bool2int"))
            }
            Idx::CHAR => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.sext(val, i64_ty, "char2int"))
            }
            Idx::BYTE => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.sext(val, i64_ty, "byte2int"))
            }
            Idx::INT => Some(val),
            _ => {
                tracing::warn!(?arg_type, "int() conversion for unsupported type");
                Some(val)
            }
        }
    }

    /// Lower `float(expr)` — convert value to float.
    fn lower_builtin_float(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let arg_id = *arg_ids.first()?;
        let val = self.lower(arg_id)?;
        let arg_type = self.expr_type(arg_id);

        match arg_type {
            Idx::INT => {
                let f64_ty = self.builder.f64_type();
                Some(self.builder.si_to_fp(val, f64_ty, "int2float"))
            }
            Idx::FLOAT => Some(val),
            _ => {
                tracing::warn!(?arg_type, "float() conversion for unsupported type");
                Some(val)
            }
        }
    }

    /// Lower `byte(expr)` — convert value to byte.
    fn lower_builtin_byte(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let arg_id = *arg_ids.first()?;
        let val = self.lower(arg_id)?;
        let arg_type = self.expr_type(arg_id);

        match arg_type {
            Idx::INT => {
                let i8_ty = self.builder.i8_type();
                Some(self.builder.trunc(val, i8_ty, "int2byte"))
            }
            Idx::BYTE => Some(val),
            _ => {
                tracing::warn!(?arg_type, "byte() conversion for unsupported type");
                Some(val)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Testing builtins
    // -----------------------------------------------------------------------

    /// Lower `assert_eq(actual, expected)` → typed runtime call.
    ///
    /// Generic `assert_eq<T: Eq>` can't be compiled by the LLVM backend (no
    /// monomorphization). Instead, we dispatch to a concrete runtime function
    /// based on the argument type: `ori_assert_eq_int`, `ori_assert_eq_bool`,
    /// `ori_assert_eq_float`, or `ori_assert_eq_str`.
    fn lower_builtin_assert_eq(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let actual_id = *arg_ids.first()?;
        let expected_id = *arg_ids.get(1)?;

        let actual_type = self.expr_type(actual_id);
        let (func_name, pass_by_ptr) = match actual_type {
            Idx::INT => ("ori_assert_eq_int", false),
            Idx::BOOL => ("ori_assert_eq_bool", false),
            Idx::FLOAT => ("ori_assert_eq_float", false),
            Idx::STR => ("ori_assert_eq_str", true),
            _ => {
                tracing::warn!(?actual_type, "assert_eq: unsupported argument type");
                self.builder.record_codegen_error();
                return None;
            }
        };

        let actual = self.lower(actual_id)?;
        let expected = self.lower(expected_id)?;

        let llvm_func = self.builder.scx().llmod.get_function(func_name)?;
        let func_id = self.builder.intern_function(llvm_func);

        if pass_by_ptr {
            // Strings are {i64, ptr} structs — runtime expects pointers
            let actual_ptr = self.alloca_and_store(actual, "assert_eq.actual");
            let expected_ptr = self.alloca_and_store(expected, "assert_eq.expected");
            self.builder.call(func_id, &[actual_ptr, expected_ptr], "")
        } else {
            self.builder.call(func_id, &[actual, expected], "")
        }
    }

    // -----------------------------------------------------------------------
    // Lambda
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::Lambda { params, body }`.
    ///
    /// Produces a fat-pointer closure `{ fn_ptr: ptr, env_ptr: ptr }`:
    /// 1. Capture analysis: find free variables with their types
    /// 2. Declare lambda function with hidden `ptr %env` first param + actual-typed params
    /// 3. In lambda body: unpack captures from env struct via `struct_gep`
    /// 4. Compile body, emit return at native type (no i64 coercion)
    /// 5. Build fat pointer: `{ fn_ptr, env_ptr }` (`env_ptr` = null if no captures)
    pub(crate) fn lower_lambda(
        &mut self,
        params: CanParamRange,
        body: CanId,
        lambda_id: CanId,
    ) -> Option<ValueId> {
        let param_list = self.canon.arena.get_params(params);

        // Step 1: Capture analysis
        let captures = self.find_captures(body, params);

        // Step 2: Get lambda type info for actual param/return types
        let lambda_type_idx = self.expr_type(lambda_id);
        let type_info = self.type_info.get(lambda_type_idx);
        let (fn_param_types, fn_ret_type) = if let TypeInfo::Function { params, ret } = &type_info {
            (params.clone(), *ret)
        } else {
            tracing::warn!(?type_info, "lambda has non-Function type info");
            // Fallback: use i64 for everything
            let param_types = vec![Idx::INT; param_list.len()];
            (param_types, Idx::INT)
        };

        // Step 3: Generate unique mangled lambda name via module-wide counter
        let counter = self.lambda_counter.get();
        self.lambda_counter.set(counter + 1);
        let mangler = Mangler::new();
        let lambda_name = mangler.mangle_function(self.module_path, &format!("__lambda_{counter}"));

        // Step 4: Build LLVM function signature
        // First param: hidden ptr %env (for captures)
        // Remaining params: actual types from type info
        let ptr_ty = self.builder.ptr_type();
        let mut llvm_param_types = Vec::with_capacity(1 + fn_param_types.len());
        llvm_param_types.push(ptr_ty); // hidden env_ptr

        for &param_idx in &fn_param_types {
            let llvm_ty = self.type_resolver.resolve(param_idx);
            llvm_param_types.push(self.builder.register_type(llvm_ty));
        }

        // Return type: actual type (unit maps to i64)
        let ret_llvm_ty = self.type_resolver.resolve(fn_ret_type);
        let ret_ty_id = self.builder.register_type(ret_llvm_ty);

        // Declare the lambda function
        let lambda_func = self
            .builder
            .declare_function(&lambda_name, &llvm_param_types, ret_ty_id);
        self.builder.set_fastcc(lambda_func);
        let entry_bb = self.builder.append_block(lambda_func, "entry");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.current_function;

        // Set up lambda context
        self.builder.set_current_function(lambda_func);
        self.builder.position_at_end(entry_bb);

        // Create lambda scope (swap out parent)
        let parent_scope = std::mem::take(&mut self.scope);

        // Bind user parameters (LLVM params start at index 1, after hidden env_ptr)
        for (i, param) in param_list.iter().enumerate() {
            let param_val = self.builder.get_param(lambda_func, (i + 1) as u32);
            self.scope.bind_immutable(param.name, param_val);
        }

        // Bind captures by unpacking from environment struct
        if !captures.is_empty() {
            let env_ptr = self.builder.get_param(lambda_func, 0);

            // Build environment struct type from capture types
            let capture_llvm_types: Vec<_> = captures
                .iter()
                .map(|&(_, _, ty)| self.type_resolver.resolve(ty))
                .collect();
            let env_struct_ty = self.builder.scx().type_struct(&capture_llvm_types, false);
            let env_struct_ty_id = self.builder.register_type(env_struct_ty.into());

            for (i, (name, _, ty)) in captures.iter().enumerate() {
                let field_ptr = self.builder.struct_gep(
                    env_struct_ty_id,
                    env_ptr,
                    i as u32,
                    &format!("cap.{i}"),
                );
                let field_ty = self.type_resolver.resolve(*ty);
                let field_ty_id = self.builder.register_type(field_ty);
                let cap_val = self
                    .builder
                    .load(field_ty_id, field_ptr, &format!("cap.{i}.val"));
                self.scope.bind_immutable(*name, cap_val);
            }
        }

        // Step 5: Compile body
        let body_val = self.lower(body);

        // Emit return (native type, no coercion)
        if !self.builder.current_block_terminated() {
            if let Some(val) = body_val {
                self.builder.ret(val);
            } else {
                // Unit return
                let zero = self.builder.const_i64(0);
                self.builder.ret(zero);
            }
        }

        // Restore context
        self.scope = parent_scope;
        self.current_function = saved_func;
        self.builder.set_current_function(saved_func);
        self.builder.restore_position(saved_pos);

        // Step 6: Build fat-pointer closure { fn_ptr, env_ptr }
        let fn_val = self.builder.get_function_value(lambda_func);
        let fn_ptr = fn_val.as_global_value().as_pointer_value();
        let fn_ptr_id = self.builder.intern_value(fn_ptr.into());

        let env_ptr = self.build_environment(&captures)?;

        let closure_ty = self.builder.closure_type();
        let fat_ptr = self
            .builder
            .build_struct(closure_ty, &[fn_ptr_id, env_ptr], "closure");
        Some(fat_ptr)
    }

    /// Find free variables (captures) used in a lambda body.
    ///
    /// Returns `(Name, ValueId, Idx)` triples — name, current value, and
    /// type index — for each captured variable. The type is needed to build
    /// the environment struct with native-typed fields.
    fn find_captures(&mut self, body: CanId, params: CanParamRange) -> Vec<(Name, ValueId, Idx)> {
        let param_list = self.canon.arena.get_params(params);
        let param_names: Vec<Name> = param_list.iter().map(|p| p.name).collect();

        let mut captures = Vec::new();
        let mut seen = std::collections::HashSet::new();
        self.collect_free_vars(body, &param_names, &mut captures, &mut seen);
        captures
    }

    /// Recursively collect free variables from a canonical expression.
    #[expect(
        clippy::too_many_lines,
        reason = "dispatch table — each arm is 1-3 lines"
    )]
    fn collect_free_vars(
        &mut self,
        expr_id: CanId,
        params: &[Name],
        captures: &mut Vec<(Name, ValueId, Idx)>,
        seen: &mut std::collections::HashSet<Name>,
    ) {
        if !expr_id.is_valid() {
            return;
        }

        let kind = *self.canon.arena.kind(expr_id);
        match kind {
            CanExpr::Ident(name) => {
                // Capture if: in outer scope, not a parameter, not already captured
                if !params.contains(&name) && !seen.contains(&name) {
                    if let Some(binding) = self.scope.lookup(name) {
                        seen.insert(name);
                        let val = match binding {
                            ScopeBinding::Immutable(v) => v,
                            ScopeBinding::Mutable { ptr, ty } => {
                                // Capture current value (by-value semantics)
                                self.builder.load(ty, ptr, "capture")
                            }
                        };
                        let capture_type = self.expr_type(expr_id);
                        captures.push((name, val, capture_type));
                    }
                }
            }
            CanExpr::Binary { left, right, .. } => {
                self.collect_free_vars(left, params, captures, seen);
                self.collect_free_vars(right, params, captures, seen);
            }
            CanExpr::Unary { operand, .. } => {
                self.collect_free_vars(operand, params, captures, seen);
            }
            CanExpr::Call { func, args } => {
                self.collect_free_vars(func, params, captures, seen);
                for &arg in self.canon.arena.get_expr_list(args) {
                    self.collect_free_vars(arg, params, captures, seen);
                }
            }
            CanExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_free_vars(cond, params, captures, seen);
                self.collect_free_vars(then_branch, params, captures, seen);
                self.collect_free_vars(else_branch, params, captures, seen);
            }
            CanExpr::Block { stmts, result } => {
                for &stmt in self.canon.arena.get_expr_list(stmts) {
                    self.collect_free_vars(stmt, params, captures, seen);
                }
                self.collect_free_vars(result, params, captures, seen);
            }
            CanExpr::Lambda { body, .. } | CanExpr::Loop { body, .. } => {
                self.collect_free_vars(body, params, captures, seen);
            }
            CanExpr::Field { receiver, .. } => {
                self.collect_free_vars(receiver, params, captures, seen);
            }
            CanExpr::Index { receiver, index } => {
                self.collect_free_vars(receiver, params, captures, seen);
                self.collect_free_vars(index, params, captures, seen);
            }
            CanExpr::For {
                iter, body, guard, ..
            } => {
                self.collect_free_vars(iter, params, captures, seen);
                self.collect_free_vars(guard, params, captures, seen);
                self.collect_free_vars(body, params, captures, seen);
            }
            CanExpr::Match {
                scrutinee, arms, ..
            } => {
                self.collect_free_vars(scrutinee, params, captures, seen);
                for &arm_body in self.canon.arena.get_expr_list(arms) {
                    self.collect_free_vars(arm_body, params, captures, seen);
                }
            }
            CanExpr::Ok(e)
            | CanExpr::Err(e)
            | CanExpr::Some(e)
            | CanExpr::Try(e)
            | CanExpr::Await(e)
            | CanExpr::Break { value: e, .. }
            | CanExpr::Continue { value: e, .. } => {
                self.collect_free_vars(e, params, captures, seen);
            }
            CanExpr::Assign { target, value } => {
                self.collect_free_vars(target, params, captures, seen);
                self.collect_free_vars(value, params, captures, seen);
            }
            CanExpr::Cast { expr, .. } => {
                self.collect_free_vars(expr, params, captures, seen);
            }
            CanExpr::Tuple(range) | CanExpr::List(range) => {
                for &e in self.canon.arena.get_expr_list(range) {
                    self.collect_free_vars(e, params, captures, seen);
                }
            }
            CanExpr::MethodCall { receiver, args, .. } => {
                self.collect_free_vars(receiver, params, captures, seen);
                for &arg in self.canon.arena.get_expr_list(args) {
                    self.collect_free_vars(arg, params, captures, seen);
                }
            }
            CanExpr::WithCapability { body, provider, .. } => {
                self.collect_free_vars(provider, params, captures, seen);
                self.collect_free_vars(body, params, captures, seen);
            }
            CanExpr::Let { init, .. } => {
                self.collect_free_vars(init, params, captures, seen);
            }
            CanExpr::Range {
                start, end, step, ..
            } => {
                self.collect_free_vars(start, params, captures, seen);
                self.collect_free_vars(end, params, captures, seen);
                self.collect_free_vars(step, params, captures, seen);
            }
            CanExpr::Struct { fields, .. } => {
                for fi in self.canon.arena.get_fields(fields) {
                    self.collect_free_vars(fi.value, params, captures, seen);
                }
            }
            CanExpr::Map(entries) => {
                for entry in self.canon.arena.get_map_entries(entries) {
                    self.collect_free_vars(entry.key, params, captures, seen);
                    self.collect_free_vars(entry.value, params, captures, seen);
                }
            }
            CanExpr::FunctionExp { props, .. } => {
                for ne in self.canon.arena.get_named_exprs(props) {
                    self.collect_free_vars(ne.value, params, captures, seen);
                }
            }
            // Leaf expressions — no free variables
            CanExpr::Constant(_)
            | CanExpr::Int(_)
            | CanExpr::Float(_)
            | CanExpr::Bool(_)
            | CanExpr::Char(_)
            | CanExpr::Str(_)
            | CanExpr::Unit
            | CanExpr::None
            | CanExpr::Error
            | CanExpr::SelfRef
            | CanExpr::FunctionRef(_)
            | CanExpr::TypeRef(_)
            | CanExpr::Const(_)
            | CanExpr::HashLength
            | CanExpr::Duration { .. }
            | CanExpr::Size { .. } => {}
        }
    }

    /// Build a heap-allocated environment struct from captured values.
    ///
    /// Returns a pointer to the environment, or null if no captures.
    /// The environment is a struct with one field per capture, stored at
    /// its native LLVM type (no i64 coercion).
    fn build_environment(&mut self, captures: &[(Name, ValueId, Idx)]) -> Option<ValueId> {
        if captures.is_empty() {
            return Some(self.builder.const_null_ptr());
        }

        // Build environment struct type from capture types
        let capture_llvm_types: Vec<_> = captures
            .iter()
            .map(|&(_, _, ty)| self.type_resolver.resolve(ty))
            .collect();
        let env_struct_ty = self.builder.scx().type_struct(&capture_llvm_types, false);
        let env_struct_ty_id = self.builder.register_type(env_struct_ty.into());

        // Compute environment size using LLVM's target-aware size_of
        let size_val = self
            .builder
            .intern_value(env_struct_ty.size_of().unwrap().into());

        // Allocate via ori_rc_alloc (V2: data-pointer style, 8-byte header)
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();
        // All closure env structs have 8-byte alignment (fields are i64 or ptr).
        // ori_rc_alloc enforces min 8-byte alignment internally as well.
        let align_val = self
            .builder
            .intern_value(self.builder.scx().type_i64().const_int(8, false).into());
        let rc_alloc_func =
            self.builder
                .get_or_declare_function("ori_rc_alloc", &[i64_ty, i64_ty], ptr_ty);
        // ori_rc_alloc returns data_ptr directly (no separate ori_rc_data call)
        let data_ptr = self
            .builder
            .call(rc_alloc_func, &[size_val, align_val], "env.data")?;

        // Store each capture into the environment struct
        for (i, (_, val, _)) in captures.iter().enumerate() {
            let field_ptr = self.builder.struct_gep(
                env_struct_ty_id,
                data_ptr,
                i as u32,
                &format!("env.field.{i}"),
            );
            self.builder.store(*val, field_ptr);
        }

        Some(data_ptr)
    }
}
