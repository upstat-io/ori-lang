//! Lambda compilation and capture analysis.
//!
//! Produces fat-pointer closures `{ fn_ptr: ptr, env_ptr: ptr }`:
//! 1. Capture analysis: find free variables with their types
//! 2. Declare lambda function with hidden `ptr %env` first param + actual-typed params
//! 3. In lambda body: unpack captures from env struct via `struct_gep`
//! 4. Compile body, emit return at native type (no i64 coercion)
//! 5. Build fat pointer: `{ fn_ptr, env_ptr }` (`env_ptr` = null if no captures)

use ori_ir::canon::{CanExpr, CanId, CanParamRange};
use ori_ir::Name;
use ori_types::Idx;

use crate::aot::mangle::Mangler;

use super::expr_lowerer::ExprLowerer;
use super::scope::ScopeBinding;
use super::type_info::TypeInfo;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
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
            | CanExpr::Unsafe(e)
            | CanExpr::Break { value: e, .. }
            | CanExpr::Continue { value: e, .. } => {
                self.collect_free_vars(e, params, captures, seen);
            }
            CanExpr::Assign { target, value } => {
                self.collect_free_vars(target, params, captures, seen);
                self.collect_free_vars(value, params, captures, seen);
            }
            CanExpr::Cast { expr, .. } | CanExpr::FormatWith { expr, .. } => {
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
