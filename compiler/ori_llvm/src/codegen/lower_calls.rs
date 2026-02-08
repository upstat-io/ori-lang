//! Function call, method call, and lambda lowering for V2 codegen.
//!
//! Handles direct calls, named-argument calls, method dispatch (builtin +
//! user-defined), and lambda/closure compilation with capture analysis.

use ori_ir::{CallArgRange, ExprId, ExprKind, ExprRange, Name, ParamRange};
use ori_types::Idx;

use crate::aot::mangle::Mangler;

use super::abi::ReturnPassing;
use super::expr_lowerer::ExprLowerer;
use super::scope::ScopeBinding;
use super::type_info::TypeInfo;
use super::value_id::{FunctionId, ValueId};

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Direct call (positional args)
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Call { func, args }`.
    ///
    /// Handles:
    /// 1. Built-in type conversions (`str()`, `int()`, `float()`, `byte()`)
    /// 2. Closure calls (if callee is a local binding)
    /// 3. Direct function calls via module lookup
    pub(crate) fn lower_call(&mut self, func: ExprId, args: ExprRange) -> Option<ValueId> {
        let func_expr = self.arena.get_expr(func);

        // Check if callee is a named function
        if let ExprKind::Ident(func_name) = &func_expr.kind {
            let name_str = self.resolve_name(*func_name);

            // Built-in type conversion functions
            match name_str {
                "str" => return self.lower_builtin_str(args),
                "int" => return self.lower_builtin_int(args),
                "float" => return self.lower_builtin_float(args),
                "byte" => return self.lower_builtin_byte(args),
                _ => {}
            }

            // Check if callee is a local binding (closure)
            if let Some(binding) = self.scope.lookup(*func_name) {
                let callee_type = self.expr_type(func);
                return self.lower_closure_call(binding, args, callee_type);
            }

            // Look up in declared function map (has ABI info for sret)
            if let Some((func_id, abi)) = self.functions.get(func_name) {
                return self.lower_abi_call(*func_id, abi, args);
            }

            // Look up in LLVM module (runtime functions, etc.)
            if let Some(llvm_func) = self.builder.scx().llmod.get_function(name_str) {
                let func_id = self.builder.intern_function(llvm_func);
                return self.lower_direct_call(func_id, args);
            }

            tracing::warn!(name = name_str, "unresolved function in call");
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
        let arg_ids = self.arena.get_expr_list(args);
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
    fn lower_direct_call(&mut self, func_id: FunctionId, args: ExprRange) -> Option<ValueId> {
        let arg_ids = self.arena.get_expr_list(args);
        let mut arg_vals = Vec::with_capacity(arg_ids.len());
        for &arg_id in arg_ids {
            arg_vals.push(self.lower(arg_id)?);
        }

        self.builder.call(func_id, &arg_vals, "call")
    }

    /// Lower a call to a function with known ABI (sret-aware).
    ///
    /// Uses `call_with_sret` for functions that return large types via
    /// hidden pointer parameter, and regular `call` for direct returns.
    fn lower_abi_call(
        &mut self,
        func_id: FunctionId,
        abi: &super::abi::FunctionAbi,
        args: ExprRange,
    ) -> Option<ValueId> {
        let arg_ids = self.arena.get_expr_list(args);
        let mut arg_vals = Vec::with_capacity(arg_ids.len());
        for &arg_id in arg_ids {
            arg_vals.push(self.lower(arg_id)?);
        }

        match &abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(abi.return_abi.ty);
                self.builder
                    .call_with_sret(func_id, &arg_vals, ret_ty, "call")
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.builder.call(func_id, &arg_vals, "call")
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
        args: ExprRange,
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
        let arg_ids = self.arena.get_expr_list(args);
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
    // Named-argument call
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::CallNamed { func, args }`.
    ///
    /// Named arguments are compiled in source order. Proper reordering
    /// to match parameter declaration order requires function signature
    /// lookup, which is deferred to function ABI integration.
    pub(crate) fn lower_call_named(&mut self, func: ExprId, args: CallArgRange) -> Option<ValueId> {
        let func_expr = self.arena.get_expr(func);

        if let ExprKind::Ident(func_name) = &func_expr.kind {
            let name_str = self.resolve_name(*func_name);

            // Check if callee is a local binding (closure)
            if let Some(binding) = self.scope.lookup(*func_name) {
                let callee_type = self.expr_type(func);
                return self.lower_closure_call_named(binding, args, callee_type);
            }

            // Look up in declared function map (has ABI info for sret)
            if let Some((func_id, abi)) = self.functions.get(func_name) {
                return self.lower_abi_call_named(*func_id, abi, args);
            }

            // Look up in LLVM module (runtime functions, etc.)
            if let Some(llvm_func) = self.builder.scx().llmod.get_function(name_str) {
                let func_id = self.builder.intern_function(llvm_func);
                return self.lower_direct_call_named(func_id, args);
            }

            tracing::warn!(name = name_str, "unresolved function in named call");
            return None;
        }

        // Non-identifier callee — fat-pointer closure dispatch
        let callee_val = self.lower(func)?;

        let fn_ptr = self.builder.extract_value(callee_val, 0, "callee.fn_ptr")?;
        let env_ptr = self
            .builder
            .extract_value(callee_val, 1, "callee.env_ptr")?;

        let call_args = self.arena.get_call_args(args);
        let mut arg_vals = Vec::with_capacity(call_args.len() + 1);
        arg_vals.push(env_ptr);
        for arg in call_args {
            arg_vals.push(self.lower(arg.value)?);
        }

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
                .call_indirect(ret_ty, &call_param_types, fn_ptr, &arg_vals, "call_named")
        } else {
            let i64_ty = self.builder.i64_type();
            let ptr_ty = self.builder.ptr_type();
            let mut param_tys = vec![ptr_ty];
            param_tys.extend(call_args.iter().map(|_| i64_ty));
            self.builder
                .call_indirect(i64_ty, &param_tys, fn_ptr, &arg_vals, "call_named")
        }
    }

    /// Lower a direct named-argument call.
    fn lower_direct_call_named(
        &mut self,
        func_id: FunctionId,
        args: CallArgRange,
    ) -> Option<ValueId> {
        let call_args = self.arena.get_call_args(args);
        let mut arg_vals = Vec::with_capacity(call_args.len());
        for arg in call_args {
            arg_vals.push(self.lower(arg.value)?);
        }

        self.builder.call(func_id, &arg_vals, "call_named")
    }

    /// Lower a named-argument call with known ABI (sret-aware).
    fn lower_abi_call_named(
        &mut self,
        func_id: FunctionId,
        abi: &super::abi::FunctionAbi,
        args: CallArgRange,
    ) -> Option<ValueId> {
        let call_args = self.arena.get_call_args(args);
        let mut arg_vals = Vec::with_capacity(call_args.len());
        for arg in call_args {
            arg_vals.push(self.lower(arg.value)?);
        }

        match &abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(abi.return_abi.ty);
                self.builder
                    .call_with_sret(func_id, &arg_vals, ret_ty, "call_named")
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.builder.call(func_id, &arg_vals, "call_named")
            }
        }
    }

    /// Lower a closure call with named arguments via fat-pointer dispatch.
    fn lower_closure_call_named(
        &mut self,
        binding: ScopeBinding,
        args: CallArgRange,
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
        let call_args = self.arena.get_call_args(args);
        let mut arg_vals = Vec::with_capacity(call_args.len() + 1);
        arg_vals.push(env_ptr); // Hidden env_ptr as first arg
        for arg in call_args {
            arg_vals.push(self.lower(arg.value)?);
        }

        // Get actual param/return types
        let type_info = self.type_info.get(callee_type);
        let (param_idxs, ret_idx) = if let TypeInfo::Function { params, ret } = &type_info {
            (params.clone(), *ret)
        } else {
            let param_types = vec![Idx::INT; call_args.len()];
            (param_types, Idx::INT)
        };

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

    /// Lower `ExprKind::MethodCall { receiver, method, args }`.
    ///
    /// Dispatch order:
    /// 1. Built-in methods (type-specific, inline codegen)
    /// 2. Type-qualified method lookup via `method_functions[(type_name, method)]`
    /// 3. Bare-name function map fallback (`functions[method]`)
    /// 4. LLVM module lookup (runtime functions)
    pub(crate) fn lower_method_call(
        &mut self,
        receiver: ExprId,
        method: Name,
        args: ExprRange,
    ) -> Option<ValueId> {
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

            let arg_ids = self.arena.get_expr_list(args);
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

    /// Lower `ExprKind::MethodCallNamed { receiver, method, args }`.
    ///
    /// Dispatch order mirrors `lower_method_call`:
    /// 1. Type-qualified method lookup via `method_functions[(type_name, method)]`
    /// 2. Bare-name function map fallback
    /// 3. LLVM module lookup (runtime functions)
    pub(crate) fn lower_method_call_named(
        &mut self,
        receiver: ExprId,
        method: Name,
        args: CallArgRange,
    ) -> Option<ValueId> {
        let recv_type = self.expr_type(receiver);
        let recv_val = self.lower(receiver)?;

        // 1. Type-qualified method lookup
        if let Some(&type_name) = self.type_idx_to_name.get(&recv_type) {
            if let Some((func_id, abi)) = self.method_functions.get(&(type_name, method)) {
                let func_id = *func_id;
                let abi = abi.clone();
                return self.emit_method_call_named(
                    func_id,
                    &abi,
                    recv_val,
                    args,
                    "method_call_named",
                );
            }
        }

        // 2. Bare-name fallback
        if let Some((func_id, abi)) = self.functions.get(&method) {
            let func_id = *func_id;
            let abi = abi.clone();
            return self.emit_method_call_named(func_id, &abi, recv_val, args, "method_call_named");
        }

        let method_name = self.resolve_name(method);

        // 3. LLVM module lookup (runtime functions, etc.)
        if let Some(llvm_func) = self.builder.scx().llmod.get_function(method_name) {
            let func_id = self.builder.intern_function(llvm_func);

            let call_args = self.arena.get_call_args(args);
            let mut all_args = Vec::with_capacity(call_args.len() + 1);
            all_args.push(recv_val);
            for arg in call_args {
                all_args.push(self.lower(arg.value)?);
            }

            return self.builder.call(func_id, &all_args, "method_call_named");
        }

        tracing::warn!(
            method = method_name,
            ?recv_type,
            "unresolved named method call"
        );
        self.builder.record_codegen_error();
        None
    }

    // -----------------------------------------------------------------------
    // Method call emission helpers
    // -----------------------------------------------------------------------

    /// Emit a method call with positional args, handling sret returns.
    ///
    /// Used by both type-qualified and bare-name method dispatch to avoid
    /// duplicating the receiver-prepend + sret logic.
    fn emit_method_call(
        &mut self,
        func_id: FunctionId,
        abi: &super::abi::FunctionAbi,
        recv_val: ValueId,
        args: ExprRange,
        name: &str,
    ) -> Option<ValueId> {
        let arg_ids = self.arena.get_expr_list(args);
        let mut all_args = Vec::with_capacity(arg_ids.len() + 1);
        all_args.push(recv_val);
        for &arg_id in arg_ids {
            all_args.push(self.lower(arg_id)?);
        }

        match &abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(abi.return_abi.ty);
                self.builder
                    .call_with_sret(func_id, &all_args, ret_ty, name)
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.builder.call(func_id, &all_args, name)
            }
        }
    }

    /// Emit a method call with named args, handling sret returns.
    fn emit_method_call_named(
        &mut self,
        func_id: FunctionId,
        abi: &super::abi::FunctionAbi,
        recv_val: ValueId,
        args: CallArgRange,
        name: &str,
    ) -> Option<ValueId> {
        let call_args = self.arena.get_call_args(args);
        let mut all_args = Vec::with_capacity(call_args.len() + 1);
        all_args.push(recv_val);
        for arg in call_args {
            all_args.push(self.lower(arg.value)?);
        }

        match &abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(abi.return_abi.ty);
                self.builder
                    .call_with_sret(func_id, &all_args, ret_ty, name)
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.builder.call(func_id, &all_args, name)
            }
        }
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
        args: ExprRange,
    ) -> Option<ValueId> {
        match recv_type {
            Idx::INT | Idx::DURATION | Idx::SIZE => self.lower_int_method(recv_val, method, args),
            Idx::FLOAT => self.lower_float_method(recv_val, method, args),
            Idx::BOOL => self.lower_bool_method(recv_val, method, args),
            Idx::ORDERING => self.lower_ordering_method(recv_val, method),
            Idx::STR => self.lower_str_method(recv_val, method, args),
            _ => {
                // Check for option/result methods
                let type_info = self.type_info.get(recv_type);
                match &type_info {
                    TypeInfo::Option { .. } => self.lower_option_method(recv_val, method, args),
                    TypeInfo::Result { .. } => self.lower_result_method(recv_val, method, args),
                    TypeInfo::List { .. } => {
                        self.lower_list_method(recv_val, recv_type, method, args)
                    }
                    _ => None,
                }
            }
        }
    }

    /// Built-in int methods.
    fn lower_int_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: ExprRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.arena.get_expr_list(args);
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
            _ => None,
        }
    }

    /// Built-in float methods.
    fn lower_float_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: ExprRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.arena.get_expr_list(args);
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
            _ => None,
        }
    }

    /// Built-in bool methods.
    fn lower_bool_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: ExprRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.arena.get_expr_list(args);
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
        _args: ExprRange,
    ) -> Option<ValueId> {
        match method {
            "len" | "length" => self.builder.extract_value(recv, 0, "str.len"),
            "is_empty" => {
                let len = self.builder.extract_value(recv, 0, "str.len")?;
                let zero = self.builder.const_i64(0);
                Some(self.builder.icmp_eq(len, zero, "str.is_empty"))
            }
            _ => None,
        }
    }

    /// Built-in Option methods.
    fn lower_option_method(
        &mut self,
        recv: ValueId,
        method: &str,
        _args: ExprRange,
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
            _ => None,
        }
    }

    /// Built-in Result methods.
    fn lower_result_method(
        &mut self,
        recv: ValueId,
        method: &str,
        _args: ExprRange,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "res.tag")?;
        let zero = self.builder.const_i8(0);

        match method {
            "is_ok" => Some(self.builder.icmp_eq(tag, zero, "res.is_ok")),
            "is_err" => Some(self.builder.icmp_ne(tag, zero, "res.is_err")),
            "unwrap" => self.builder.extract_value(recv, 1, "res.unwrap"),
            _ => None,
        }
    }

    /// Built-in List methods.
    fn lower_list_method(
        &mut self,
        recv: ValueId,
        _recv_type: Idx,
        method: &str,
        _args: ExprRange,
    ) -> Option<ValueId> {
        match method {
            "len" | "length" => self.builder.extract_value(recv, 0, "list.len"),
            "is_empty" => {
                let len = self.builder.extract_value(recv, 0, "list.len")?;
                let zero = self.builder.const_i64(0);
                Some(self.builder.icmp_eq(len, zero, "list.is_empty"))
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Built-in type conversion functions
    // -----------------------------------------------------------------------

    /// Lower `str(expr)` — convert value to string.
    fn lower_builtin_str(&mut self, args: ExprRange) -> Option<ValueId> {
        let arg_ids = self.arena.get_expr_list(args);
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
                None
            }
        }
    }

    /// Lower `int(expr)` — convert value to int.
    fn lower_builtin_int(&mut self, args: ExprRange) -> Option<ValueId> {
        let arg_ids = self.arena.get_expr_list(args);
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
    fn lower_builtin_float(&mut self, args: ExprRange) -> Option<ValueId> {
        let arg_ids = self.arena.get_expr_list(args);
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
    fn lower_builtin_byte(&mut self, args: ExprRange) -> Option<ValueId> {
        let arg_ids = self.arena.get_expr_list(args);
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
    // Lambda
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Lambda { params, body }`.
    ///
    /// Produces a fat-pointer closure `{ fn_ptr: ptr, env_ptr: ptr }`:
    /// 1. Capture analysis: find free variables with their types
    /// 2. Declare lambda function with hidden `ptr %env` first param + actual-typed params
    /// 3. In lambda body: unpack captures from env struct via `struct_gep`
    /// 4. Compile body, emit return at native type (no i64 coercion)
    /// 5. Build fat pointer: `{ fn_ptr, env_ptr }` (`env_ptr` = null if no captures)
    pub(crate) fn lower_lambda(
        &mut self,
        params: ParamRange,
        body: ExprId,
        lambda_id: ExprId,
    ) -> Option<ValueId> {
        let param_list = self.arena.get_params(params);

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
    fn find_captures(&mut self, body: ExprId, params: ParamRange) -> Vec<(Name, ValueId, Idx)> {
        let param_list = self.arena.get_params(params);
        let param_names: Vec<Name> = param_list.iter().map(|p| p.name).collect();

        let mut captures = Vec::new();
        let mut seen = std::collections::HashSet::new();
        self.collect_free_vars(body, &param_names, &mut captures, &mut seen);
        captures
    }

    /// Recursively collect free variables from an expression.
    #[allow(clippy::too_many_lines)] // Exhaustive traversal over all ExprKind variants
    fn collect_free_vars(
        &mut self,
        expr_id: ExprId,
        params: &[Name],
        captures: &mut Vec<(Name, ValueId, Idx)>,
        seen: &mut std::collections::HashSet<Name>,
    ) {
        if !expr_id.is_valid() {
            return;
        }

        let expr = self.arena.get_expr(expr_id);
        match &expr.kind {
            ExprKind::Ident(name) => {
                // Capture if: in outer scope, not a parameter, not already captured
                if !params.contains(name) && !seen.contains(name) {
                    if let Some(binding) = self.scope.lookup(*name) {
                        seen.insert(*name);
                        let val = match binding {
                            ScopeBinding::Immutable(v) => v,
                            ScopeBinding::Mutable { ptr, ty } => {
                                // Capture current value (by-value semantics)
                                self.builder.load(ty, ptr, "capture")
                            }
                        };
                        let capture_type = self.expr_type(expr_id);
                        captures.push((*name, val, capture_type));
                    }
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.collect_free_vars(*left, params, captures, seen);
                self.collect_free_vars(*right, params, captures, seen);
            }
            ExprKind::Unary { operand, .. } => {
                self.collect_free_vars(*operand, params, captures, seen);
            }
            ExprKind::Call { func, args } => {
                self.collect_free_vars(*func, params, captures, seen);
                for &arg in self.arena.get_expr_list(*args) {
                    self.collect_free_vars(arg, params, captures, seen);
                }
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_free_vars(*cond, params, captures, seen);
                self.collect_free_vars(*then_branch, params, captures, seen);
                self.collect_free_vars(*else_branch, params, captures, seen);
            }
            ExprKind::Block { stmts, result } => {
                for stmt in self.arena.get_stmt_range(*stmts) {
                    match &stmt.kind {
                        ori_ir::StmtKind::Expr(e) => {
                            self.collect_free_vars(*e, params, captures, seen);
                        }
                        ori_ir::StmtKind::Let { init, .. } => {
                            self.collect_free_vars(*init, params, captures, seen);
                        }
                    }
                }
                self.collect_free_vars(*result, params, captures, seen);
            }
            ExprKind::Lambda { body, .. } | ExprKind::Loop { body } => {
                self.collect_free_vars(*body, params, captures, seen);
            }
            ExprKind::Field { receiver, .. } => {
                self.collect_free_vars(*receiver, params, captures, seen);
            }
            ExprKind::Index { receiver, index } => {
                self.collect_free_vars(*receiver, params, captures, seen);
                self.collect_free_vars(*index, params, captures, seen);
            }
            ExprKind::For {
                iter, body, guard, ..
            } => {
                self.collect_free_vars(*iter, params, captures, seen);
                self.collect_free_vars(*guard, params, captures, seen);
                self.collect_free_vars(*body, params, captures, seen);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.collect_free_vars(*scrutinee, params, captures, seen);
                for arm in self.arena.get_arms(*arms) {
                    self.collect_free_vars(arm.body, params, captures, seen);
                }
            }
            ExprKind::Ok(e)
            | ExprKind::Err(e)
            | ExprKind::Some(e)
            | ExprKind::Try(e)
            | ExprKind::Await(e)
            | ExprKind::Break(e)
            | ExprKind::Continue(e) => {
                self.collect_free_vars(*e, params, captures, seen);
            }
            ExprKind::Assign { target, value } => {
                self.collect_free_vars(*target, params, captures, seen);
                self.collect_free_vars(*value, params, captures, seen);
            }
            ExprKind::Cast { expr, .. } => {
                self.collect_free_vars(*expr, params, captures, seen);
            }
            ExprKind::Tuple(range) | ExprKind::List(range) => {
                for &e in self.arena.get_expr_list(*range) {
                    self.collect_free_vars(e, params, captures, seen);
                }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.collect_free_vars(*receiver, params, captures, seen);
                for &arg in self.arena.get_expr_list(*args) {
                    self.collect_free_vars(arg, params, captures, seen);
                }
            }
            ExprKind::WithCapability { body, provider, .. } => {
                self.collect_free_vars(*provider, params, captures, seen);
                self.collect_free_vars(*body, params, captures, seen);
            }
            // Leaf expressions — no free variables
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Char(_)
            | ExprKind::String(_)
            | ExprKind::Unit
            | ExprKind::None
            | ExprKind::Error
            | ExprKind::SelfRef
            | ExprKind::FunctionRef(_)
            | ExprKind::Const(_)
            | ExprKind::HashLength
            | ExprKind::TemplateFull(_)
            | ExprKind::Duration { .. }
            | ExprKind::Size { .. } => {}
            // Complex expressions that may have sub-expressions
            ExprKind::Let { init, .. } => {
                self.collect_free_vars(*init, params, captures, seen);
            }
            ExprKind::CallNamed { func, args } => {
                self.collect_free_vars(*func, params, captures, seen);
                for arg in self.arena.get_call_args(*args) {
                    self.collect_free_vars(arg.value, params, captures, seen);
                }
            }
            ExprKind::MethodCallNamed { receiver, args, .. } => {
                self.collect_free_vars(*receiver, params, captures, seen);
                for arg in self.arena.get_call_args(*args) {
                    self.collect_free_vars(arg.value, params, captures, seen);
                }
            }
            ExprKind::TemplateLiteral { parts, .. } => {
                for part in self.arena.get_template_parts(*parts) {
                    self.collect_free_vars(part.expr, params, captures, seen);
                }
            }
            ExprKind::Range {
                start, end, step, ..
            } => {
                self.collect_free_vars(*start, params, captures, seen);
                self.collect_free_vars(*end, params, captures, seen);
                self.collect_free_vars(*step, params, captures, seen);
            }
            ExprKind::Struct { fields, .. } => {
                for fi in self.arena.get_field_inits(*fields) {
                    if let Some(val) = fi.value {
                        self.collect_free_vars(val, params, captures, seen);
                    }
                }
            }
            ExprKind::StructWithSpread { fields, .. } => {
                for field in self.arena.get_struct_lit_fields(*fields) {
                    match field {
                        ori_ir::StructLitField::Field(fi) => {
                            if let Some(val) = fi.value {
                                self.collect_free_vars(val, params, captures, seen);
                            }
                        }
                        ori_ir::StructLitField::Spread { expr, .. } => {
                            self.collect_free_vars(*expr, params, captures, seen);
                        }
                    }
                }
            }
            ExprKind::Map(entries) => {
                for entry in self.arena.get_map_entries(*entries) {
                    self.collect_free_vars(entry.key, params, captures, seen);
                    self.collect_free_vars(entry.value, params, captures, seen);
                }
            }
            ExprKind::ListWithSpread(elems) => {
                for elem in self.arena.get_list_elements(*elems) {
                    match elem {
                        ori_ir::ListElement::Expr { expr, .. }
                        | ori_ir::ListElement::Spread { expr, .. } => {
                            self.collect_free_vars(*expr, params, captures, seen);
                        }
                    }
                }
            }
            ExprKind::MapWithSpread(elems) => {
                for elem in self.arena.get_map_elements(*elems) {
                    match elem {
                        ori_ir::MapElement::Entry(entry) => {
                            self.collect_free_vars(entry.key, params, captures, seen);
                            self.collect_free_vars(entry.value, params, captures, seen);
                        }
                        ori_ir::MapElement::Spread { expr, .. } => {
                            self.collect_free_vars(*expr, params, captures, seen);
                        }
                    }
                }
            }
            ExprKind::FunctionSeq(seq_id) => {
                let seq = self.arena.get_function_seq(*seq_id);
                match seq {
                    ori_ir::FunctionSeq::Run {
                        bindings, result, ..
                    }
                    | ori_ir::FunctionSeq::Try {
                        bindings, result, ..
                    } => {
                        for binding in self.arena.get_seq_bindings(*bindings) {
                            match binding {
                                ori_ir::SeqBinding::Let { value, .. } => {
                                    self.collect_free_vars(*value, params, captures, seen);
                                }
                                ori_ir::SeqBinding::Stmt { expr, .. } => {
                                    self.collect_free_vars(*expr, params, captures, seen);
                                }
                            }
                        }
                        self.collect_free_vars(*result, params, captures, seen);
                    }
                    ori_ir::FunctionSeq::Match {
                        scrutinee, arms, ..
                    } => {
                        self.collect_free_vars(*scrutinee, params, captures, seen);
                        for arm in self.arena.get_arms(*arms) {
                            self.collect_free_vars(arm.body, params, captures, seen);
                        }
                    }
                    ori_ir::FunctionSeq::ForPattern {
                        over,
                        map,
                        arm,
                        default,
                        ..
                    } => {
                        self.collect_free_vars(*over, params, captures, seen);
                        if let Some(m) = map {
                            self.collect_free_vars(*m, params, captures, seen);
                        }
                        self.collect_free_vars(arm.body, params, captures, seen);
                        self.collect_free_vars(*default, params, captures, seen);
                    }
                }
            }
            ExprKind::FunctionExp(fexp_id) => {
                let exp = self.arena.get_function_exp(*fexp_id);
                for ne in self.arena.get_named_exprs(exp.props) {
                    self.collect_free_vars(ne.value, params, captures, seen);
                }
            }
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

        // Compute environment size (conservative: 8 bytes per capture minimum)
        let env_size: usize = captures.len() * 8;
        let size_val = self.builder.const_i64(env_size as i64);

        // Allocate via ori_closure_box (heap allocation)
        let i64_ty = self.builder.i64_type();
        let ptr_ty = self.builder.ptr_type();
        let box_func = self
            .builder
            .get_or_declare_function("ori_closure_box", &[i64_ty], ptr_ty);
        let heap_ptr = self.builder.call(box_func, &[size_val], "env.ptr")?;

        // Store each capture into the environment struct
        for (i, (_, val, _)) in captures.iter().enumerate() {
            let field_ptr = self.builder.struct_gep(
                env_struct_ty_id,
                heap_ptr,
                i as u32,
                &format!("env.field.{i}"),
            );
            self.builder.store(*val, field_ptr);
        }

        Some(heap_ptr)
    }
}
