//! Lambda and closure compilation.
//!
//! Closures in Ori capture variables from their enclosing scope.
//! We compile them as:
//! 1. A lambda function with captured variables as extra parameters
//! 2. A closure struct: { i8 tag, i64 `fn_ptr`, capture0, capture1, ... }
//!
//! When calling a closure, we extract the function pointer and captured
//! values, then call the function with both regular args and captures.

use rustc_hash::FxHashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::ast::ExprKind;
use ori_ir::{ExprArena, ExprId, Name, TypeId};

use crate::builder::Builder;

/// Counter for generating unique lambda function names.
static LAMBDA_COUNTER: AtomicU64 = AtomicU64::new(0);

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile a lambda expression.
    ///
    /// Lambdas are compiled as closures with captured variables passed as
    /// extra parameters. The closure struct contains the function pointer
    /// and the captured values.
    pub(crate) fn compile_lambda(
        &self,
        params: ori_ir::ast::ParamRange,
        body: ExprId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &FxHashMap<Name, BasicValueEnum<'ll>>,
        _parent_function: FunctionValue<'ll>,
    ) -> Option<BasicValueEnum<'ll>> {
        let parameters = arena.get_params(params);

        // Collect parameter names
        let param_names: HashSet<Name> = parameters.iter().map(|p| p.name).collect();

        // Find captured variables (variables used in body that are in locals but not params)
        let captures = self.find_captures(body, arena, &param_names, locals);

        // Create a unique name for this lambda
        let lambda_id = LAMBDA_COUNTER.fetch_add(1, Ordering::SeqCst);
        let lambda_name = format!("__lambda_{lambda_id}");

        // Build parameter types: regular params + captured values (all as i64)
        let total_params = parameters.len() + captures.len();
        let param_types: Vec<BasicMetadataTypeEnum> = (0..total_params)
            .map(|_| self.cx().scx.type_i64().into())
            .collect();

        // Create function type
        let fn_type = self.cx().scx.type_i64().fn_type(&param_types, false);
        let lambda_fn = self.cx().llmod().add_function(&lambda_name, fn_type, None);

        // Create entry block for lambda
        let entry = self.cx().llcx().append_basic_block(lambda_fn, "entry");

        // Save current builder position with RAII guard - restores on drop
        let guard = self.save_position();

        // Position at lambda entry
        self.position_at_end(entry);

        // Build parameter map for lambda body
        let mut lambda_locals: FxHashMap<Name, BasicValueEnum<'ll>> = FxHashMap::default();

        // Add regular parameters
        for (i, param) in parameters.iter().enumerate() {
            if let Some(param_val) = lambda_fn.get_nth_param(i as u32) {
                param_val.set_name(self.cx().interner.lookup(param.name));
                lambda_locals.insert(param.name, param_val);
            }
        }

        // Add captured variables as parameters
        for (i, (name, _)) in captures.iter().enumerate() {
            let param_idx = (parameters.len() + i) as u32;
            if let Some(param_val) = lambda_fn.get_nth_param(param_idx) {
                let name_str = self.cx().interner.lookup(*name);
                param_val.set_name(&format!("capture_{name_str}"));
                lambda_locals.insert(*name, param_val);
            }
        }

        // Compile lambda body
        let result =
            self.compile_expr(body, arena, expr_types, &mut lambda_locals, lambda_fn, None);

        // Return result
        if let Some(val) = result {
            // Coerce to i64 if needed
            let ret_val: BasicValueEnum<'ll> = if let Some(int_val) = self.coerce_to_i64(val) {
                int_val.into()
            } else {
                val
            };
            self.ret(ret_val);
        } else {
            let zero = self.cx().scx.type_i64().const_int(0, false);
            self.ret(zero.into());
        }

        // Restore builder position to parent function BEFORE building closure struct.
        // Without this, the following instructions would be emitted after the ret.
        drop(guard);

        // Get function pointer as i64
        let fn_ptr = lambda_fn.as_global_value().as_pointer_value();
        let ptr_as_int = self.ptr_to_int(fn_ptr, self.cx().scx.type_i64(), "fn_ptr_to_int");

        // For closures without captures, return just the function pointer as i64
        if captures.is_empty() {
            return Some(ptr_as_int.into());
        }

        // For closures with captures, build and box the closure struct
        // Build closure struct: { i8 capture_count, i64 fn_ptr, capture0, capture1, ... }
        let mut field_types = vec![
            self.cx().scx.type_i8().into(),  // capture_count
            self.cx().scx.type_i64().into(), // fn_ptr as i64
        ];
        let mut field_values: Vec<BasicValueEnum<'ll>> = vec![
            self.cx()
                .scx
                .type_i8()
                .const_int(captures.len() as u64, false)
                .into(), // capture_count
            ptr_as_int.into(),
        ];

        // Add captured values
        for (_, capture_val) in &captures {
            field_types.push(self.cx().scx.type_i64().into());
            // Coerce captured value to i64
            let coerced: BasicValueEnum<'ll> =
                if let Some(int_val) = self.coerce_to_i64(*capture_val) {
                    int_val.into()
                } else {
                    *capture_val
                };
            field_values.push(coerced);
        }

        let closure_type = self.cx().scx.type_struct(&field_types, false);
        let closure_val = self.build_struct(closure_type, &field_values, "closure");

        // Box the closure: allocate heap memory and store the struct
        // Calculate size: 1 byte (count) + 8 bytes (fn_ptr) + 8 bytes * capture_count
        let size = 1 + 8 + 8 * captures.len();
        let size_val = self.cx().scx.type_i64().const_int(size as u64, false);

        // Call ori_closure_box to allocate memory
        let box_fn = self.cx().llmod().get_function("ori_closure_box").expect(
            "ori_closure_box not declared - call declare_runtime_functions() before compiling lambdas",
        );
        let box_ptr = self
            .call(box_fn, &[size_val.into()], "closure_box")?
            .into_pointer_value();

        // Store the closure struct to the allocated memory
        self.store(closure_val.into(), box_ptr);

        // Return the pointer as i64, with lowest bit set as a tag to indicate "boxed closure"
        // This allows compile_closure_call to distinguish between plain fn_ptr and boxed closure
        let ptr_int = self.ptr_to_int(box_ptr, self.cx().scx.type_i64(), "closure_ptr_int");
        let one = self.cx().scx.type_i64().const_int(1, false);
        let tagged = self.or(ptr_int, one, "closure_tagged");
        Some(tagged.into())
    }

    /// Find variables captured by a lambda expression.
    ///
    /// Returns a list of (Name, Value) pairs for variables that:
    /// - Are used in the lambda body
    /// - Are in the outer locals
    /// - Are not lambda parameters
    fn find_captures(
        &self,
        body: ExprId,
        arena: &ExprArena,
        param_names: &HashSet<Name>,
        locals: &FxHashMap<Name, BasicValueEnum<'ll>>,
    ) -> Vec<(Name, BasicValueEnum<'ll>)> {
        let mut captures = Vec::new();
        let mut seen = HashSet::new();

        self.collect_free_vars(body, arena, param_names, locals, &mut captures, &mut seen);

        captures
    }

    /// Recursively collect free variables in an expression.
    #[expect(
        clippy::self_only_used_in_recursion,
        reason = "self provides access to cx().interner for future Name resolution"
    )]
    fn collect_free_vars(
        &self,
        expr_id: ExprId,
        arena: &ExprArena,
        bound: &HashSet<Name>,
        locals: &FxHashMap<Name, BasicValueEnum<'ll>>,
        captures: &mut Vec<(Name, BasicValueEnum<'ll>)>,
        seen: &mut HashSet<Name>,
    ) {
        let expr = arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::Ident(name) => {
                // If this name is not bound locally and exists in outer locals, capture it
                if !bound.contains(name) && !seen.contains(name) {
                    if let Some(val) = locals.get(name) {
                        captures.push((*name, *val));
                        seen.insert(*name);
                    }
                }
            }

            ExprKind::Binary { left, right, .. } => {
                self.collect_free_vars(*left, arena, bound, locals, captures, seen);
                self.collect_free_vars(*right, arena, bound, locals, captures, seen);
            }

            ExprKind::Unary { operand, .. } => {
                self.collect_free_vars(*operand, arena, bound, locals, captures, seen);
            }

            ExprKind::Call { func, args } => {
                self.collect_free_vars(*func, arena, bound, locals, captures, seen);
                for arg_id in arena.iter_expr_list(*args) {
                    self.collect_free_vars(arg_id, arena, bound, locals, captures, seen);
                }
            }

            ExprKind::CallNamed { func, args } => {
                self.collect_free_vars(*func, arena, bound, locals, captures, seen);
                for arg in arena.get_call_args(*args) {
                    self.collect_free_vars(arg.value, arena, bound, locals, captures, seen);
                }
            }

            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_free_vars(*cond, arena, bound, locals, captures, seen);
                self.collect_free_vars(*then_branch, arena, bound, locals, captures, seen);
                if let Some(else_id) = else_branch {
                    self.collect_free_vars(*else_id, arena, bound, locals, captures, seen);
                }
            }

            ExprKind::Block { stmts, result } => {
                // Note: proper handling would track let bindings as new bound vars
                let stmt_list = arena.get_stmt_range(*stmts);
                for stmt in stmt_list {
                    match &stmt.kind {
                        ori_ir::ast::StmtKind::Expr(e) => {
                            self.collect_free_vars(*e, arena, bound, locals, captures, seen);
                        }
                        ori_ir::ast::StmtKind::Let { init, .. } => {
                            self.collect_free_vars(*init, arena, bound, locals, captures, seen);
                        }
                    }
                }
                if let Some(result_id) = result {
                    self.collect_free_vars(*result_id, arena, bound, locals, captures, seen);
                }
            }

            ExprKind::Lambda { body, .. } => {
                // Nested lambdas - would need to add their params to bound
                self.collect_free_vars(*body, arena, bound, locals, captures, seen);
            }

            ExprKind::Field { receiver, .. } => {
                self.collect_free_vars(*receiver, arena, bound, locals, captures, seen);
            }

            ExprKind::Index { receiver, index } => {
                self.collect_free_vars(*receiver, arena, bound, locals, captures, seen);
                self.collect_free_vars(*index, arena, bound, locals, captures, seen);
            }

            // Literals and other non-recursive expressions don't capture anything
            _ => {}
        }
    }
}
