//! Special construct lowering for V2 codegen.
//!
//! Handles Ori's unique expression patterns:
//! - `FunctionSeq`: `run { ... }`, `try { ... }`, `match`, `for` patterns
//! - `FunctionExp`: `print(...)`, `panic(...)`, `todo`, `recurse`, etc.
//! - `SelfRef`: recursive self-reference
//! - `Await`: async (stub)
//! - `WithCapability`: capability provision

use ori_ir::{
    ExprId, FunctionExpId, FunctionExpKind, FunctionSeq, FunctionSeqId, Name, SeqBinding,
};
use ori_types::Idx;

use super::expr_lowerer::ExprLowerer;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // FunctionSeq: run, try, match, for_pattern
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::FunctionSeq(id)`.
    pub(crate) fn lower_function_seq(
        &mut self,
        seq_id: FunctionSeqId,
        expr_id: ExprId,
    ) -> Option<ValueId> {
        let seq = self.arena.get_function_seq(seq_id).clone();
        match &seq {
            FunctionSeq::Run {
                bindings, result, ..
            } => self.lower_seq_run(*bindings, *result),
            FunctionSeq::Try {
                bindings, result, ..
            } => self.lower_seq_try(*bindings, *result, expr_id),
            FunctionSeq::Match {
                scrutinee, arms, ..
            } => self.lower_match(*scrutinee, *arms, expr_id),
            FunctionSeq::ForPattern {
                over,
                map,
                arm,
                default,
                ..
            } => self.lower_seq_for_pattern(*over, *map, arm, *default, expr_id),
        }
    }

    /// Lower `run { binding1; binding2; ...; result }`.
    ///
    /// Sequential execution: each binding is evaluated in order, with
    /// let bindings adding to the scope.
    fn lower_seq_run(
        &mut self,
        bindings: ori_ir::SeqBindingRange,
        result: ExprId,
    ) -> Option<ValueId> {
        let binding_slice = self.arena.get_seq_bindings(bindings);

        // Create a child scope for the run block
        let child = self.scope.child();
        let parent = std::mem::replace(&mut self.scope, child);

        for binding in binding_slice {
            match binding {
                SeqBinding::Let {
                    pattern,
                    value,
                    mutable,
                    ..
                } => {
                    self.lower_let(*pattern, *value, *mutable);
                }
                SeqBinding::Stmt { expr, .. } => {
                    self.lower(*expr);
                }
            }
            if self.builder.current_block_terminated() {
                break;
            }
        }

        let result_val = if result.is_valid() && !self.builder.current_block_terminated() {
            self.lower(result)
        } else {
            None
        };

        self.scope = parent;
        result_val
    }

    /// Lower `try { binding1; binding2; ...; result }`.
    ///
    /// Like `run`, but each binding that returns `Result` is automatically
    /// unwrapped with `?` semantics. If any step fails, the whole block
    /// returns the error.
    fn lower_seq_try(
        &mut self,
        bindings: ori_ir::SeqBindingRange,
        result: ExprId,
        _expr_id: ExprId,
    ) -> Option<ValueId> {
        // For now, lower exactly like `run` — full try semantics require
        // checking each binding's result type and inserting automatic `?`
        // propagation, which is done by the type checker rewriting to
        // explicit Try nodes.
        self.lower_seq_run(bindings, result)
    }

    /// Lower `for pattern` — iterate and pattern-match.
    fn lower_seq_for_pattern(
        &mut self,
        over: ExprId,
        map: Option<ExprId>,
        arm: &ori_ir::MatchArm,
        default: ExprId,
        _expr_id: ExprId,
    ) -> Option<ValueId> {
        // Simplified: evaluate `over`, try to match the pattern on each element,
        // apply `map` if present, fall through to `default`.
        let over_val = self.lower(over)?;
        let _ = (map, arm, default, over_val);
        tracing::debug!("for_pattern lowering — simplified stub");

        // Return unit for now
        Some(self.builder.const_i64(0))
    }

    // -----------------------------------------------------------------------
    // FunctionExp: print, panic, todo, recurse, etc.
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::FunctionExp(id)`.
    pub(crate) fn lower_function_exp(
        &mut self,
        fexp_id: FunctionExpId,
        expr_id: ExprId,
    ) -> Option<ValueId> {
        let exp = self.arena.get_function_exp(fexp_id).clone();

        match exp.kind {
            FunctionExpKind::Print => self.lower_exp_print(&exp),
            FunctionExpKind::Panic => self.lower_exp_panic(&exp),
            FunctionExpKind::Todo => self.lower_exp_todo(),
            FunctionExpKind::Unreachable => self.lower_exp_unreachable(),
            FunctionExpKind::Recurse => self.lower_exp_recurse(&exp, expr_id),
            FunctionExpKind::Cache => self.lower_exp_cache(&exp, expr_id),
            FunctionExpKind::Catch => self.lower_exp_catch(&exp, expr_id),
            FunctionExpKind::Parallel => {
                tracing::warn!("parallel expression not yet implemented");
                None
            }
            FunctionExpKind::Spawn => {
                tracing::warn!("spawn expression not yet implemented");
                None
            }
            FunctionExpKind::Timeout => {
                tracing::warn!("timeout expression not yet implemented");
                None
            }
            FunctionExpKind::With => {
                tracing::warn!("with expression not yet implemented");
                None
            }
        }
    }

    /// Lower `print(msg: expr)`.
    ///
    /// Dispatches to the appropriate `ori_print_*` runtime function
    /// based on the value type.
    fn lower_exp_print(&mut self, exp: &ori_ir::FunctionExp) -> Option<ValueId> {
        let named_exprs = self.arena.get_named_exprs(exp.props);
        let msg_expr = named_exprs.iter().find(|ne| {
            let name = self.resolve_name(ne.name);
            name == "msg"
        })?;

        let val = self.lower(msg_expr.value)?;
        let val_type = self.expr_type(msg_expr.value);

        match val_type {
            Idx::INT | Idx::DURATION | Idx::SIZE => {
                let i64_ty = self.builder.i64_type();
                // Use void return type — declare with a dummy return then discard
                let func = self.builder.get_or_declare_function(
                    "ori_print_int",
                    &[i64_ty],
                    i64_ty, // placeholder; call returns void
                );
                self.builder.call(func, &[val], "");
            }
            Idx::FLOAT => {
                let f64_ty = self.builder.f64_type();
                let func =
                    self.builder
                        .get_or_declare_function("ori_print_float", &[f64_ty], f64_ty);
                self.builder.call(func, &[val], "");
            }
            Idx::BOOL => {
                let bool_ty = self.builder.bool_type();
                let func =
                    self.builder
                        .get_or_declare_function("ori_print_bool", &[bool_ty], bool_ty);
                self.builder.call(func, &[val], "");
            }
            Idx::STR => {
                // String: pass pointer to {len, data} struct
                let ptr = self.alloca_and_store(val, "print.str");
                let ptr_ty = self.builder.ptr_type();
                let func = self.builder.get_or_declare_function(
                    "ori_print",
                    &[ptr_ty],
                    ptr_ty, // placeholder
                );
                self.builder.call(func, &[ptr], "");
            }
            _ => {
                // Fall back to printing as int
                let coerced = self.coerce_to_i64(val, val_type);
                let i64_ty = self.builder.i64_type();
                let func = self
                    .builder
                    .get_or_declare_function("ori_print_int", &[i64_ty], i64_ty);
                self.builder.call(func, &[coerced], "");
            }
        }

        // print returns unit
        Some(self.builder.const_i64(0))
    }

    /// Lower `panic(message: expr)`.
    ///
    /// Calls `ori_panic` with the message string, then emits `unreachable`.
    fn lower_exp_panic(&mut self, exp: &ori_ir::FunctionExp) -> Option<ValueId> {
        let named_exprs = self.arena.get_named_exprs(exp.props);
        let msg_expr = named_exprs.iter().find(|ne| {
            let name = self.resolve_name(ne.name);
            name == "message" || name == "value"
        });

        if let Some(ne) = msg_expr {
            let val = self.lower(ne.value)?;
            let val_type = self.expr_type(ne.value);

            if val_type == Idx::STR {
                let ptr = self.alloca_and_store(val, "panic.msg");
                if let Some(panic_fn) = self.builder.scx().llmod.get_function("ori_panic") {
                    let func_id = self.builder.intern_function(panic_fn);
                    self.builder.call(func_id, &[ptr], "");
                }
            } else {
                // Non-string panic — use a default message
                let msg = self
                    .builder
                    .build_global_string_ptr("panic: non-string message", "panic.default");
                if let Some(panic_fn) = self.builder.scx().llmod.get_function("ori_panic_cstr") {
                    let func_id = self.builder.intern_function(panic_fn);
                    self.builder.call(func_id, &[msg], "");
                }
            }
        } else {
            // No message — default panic
            let msg = self
                .builder
                .build_global_string_ptr("explicit panic", "panic.msg");
            if let Some(panic_fn) = self.builder.scx().llmod.get_function("ori_panic_cstr") {
                let func_id = self.builder.intern_function(panic_fn);
                self.builder.call(func_id, &[msg], "");
            }
        }

        self.builder.unreachable();
        None // panic never returns
    }

    /// Lower `todo` — panics with "not yet implemented".
    fn lower_exp_todo(&mut self) -> Option<ValueId> {
        let msg = self
            .builder
            .build_global_string_ptr("not yet implemented", "todo.msg");
        if let Some(panic_fn) = self.builder.scx().llmod.get_function("ori_panic_cstr") {
            let func_id = self.builder.intern_function(panic_fn);
            self.builder.call(func_id, &[msg], "");
        }
        self.builder.unreachable();
        None
    }

    /// Lower `unreachable` — emits LLVM unreachable.
    fn lower_exp_unreachable(&mut self) -> Option<ValueId> {
        let msg = self
            .builder
            .build_global_string_ptr("reached unreachable code", "unreach.msg");
        if let Some(panic_fn) = self.builder.scx().llmod.get_function("ori_panic_cstr") {
            let func_id = self.builder.intern_function(panic_fn);
            self.builder.call(func_id, &[msg], "");
        }
        self.builder.unreachable();
        None
    }

    /// Lower `recurse(args...)` — tail-recursive call to current function.
    ///
    /// Compiles the arguments, then emits a tail call to the current
    /// function. The `tail` attribute combined with `fastcc` enables LLVM
    /// to perform tail call optimization (reusing the caller's stack frame),
    /// preventing stack overflow on deep recursion.
    fn lower_exp_recurse(
        &mut self,
        exp: &ori_ir::FunctionExp,
        _expr_id: ExprId,
    ) -> Option<ValueId> {
        let named_exprs = self.arena.get_named_exprs(exp.props);
        let mut arg_vals = Vec::with_capacity(named_exprs.len());
        for ne in named_exprs {
            let val = self.lower(ne.value)?;
            arg_vals.push(val);
        }

        self.builder
            .call_tail(self.current_function, &arg_vals, "recurse")
    }

    /// Lower `cache(key: ..., value: ...)` — memoization.
    fn lower_exp_cache(&mut self, exp: &ori_ir::FunctionExp, _expr_id: ExprId) -> Option<ValueId> {
        // Simplified: just evaluate the value expression
        let named_exprs = self.arena.get_named_exprs(exp.props);
        for ne in named_exprs {
            let name = self.resolve_name(ne.name);
            if name == "value" || name == "expr" {
                return self.lower(ne.value);
            }
        }
        tracing::warn!("cache expression missing value property");
        None
    }

    /// Lower `catch(expr: ..., handler: ...)` — error catching.
    fn lower_exp_catch(&mut self, exp: &ori_ir::FunctionExp, _expr_id: ExprId) -> Option<ValueId> {
        // Simplified: just evaluate the expr property
        let named_exprs = self.arena.get_named_exprs(exp.props);
        for ne in named_exprs {
            let name = self.resolve_name(ne.name);
            if name == "expr" || name == "value" {
                return self.lower(ne.value);
            }
        }
        tracing::warn!("catch expression missing expr property");
        None
    }

    // -----------------------------------------------------------------------
    // SelfRef, Await, WithCapability
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::SelfRef` — recursive reference to current function.
    ///
    /// Returns the current function as a pointer value.
    pub(crate) fn lower_self_ref(&mut self) -> Option<ValueId> {
        let func_val = self.builder.get_function_value(self.current_function);
        let ptr = func_val.as_global_value().as_pointer_value();
        Some(self.builder.intern_value(ptr.into()))
    }

    /// Lower `ExprKind::Await(inner)` — async (stub).
    ///
    /// For the sync runtime, await is a no-op: just evaluate the inner
    /// expression.
    pub(crate) fn lower_await(&mut self, inner: ExprId) -> Option<ValueId> {
        self.lower(inner)
    }

    /// Lower `ExprKind::WithCapability { capability, provider, body }`.
    ///
    /// Capability system not yet implemented. For now, just evaluates
    /// the body expression.
    pub(crate) fn lower_with_capability(
        &mut self,
        _capability: Name,
        _provider: ExprId,
        body: ExprId,
    ) -> Option<ValueId> {
        self.lower(body)
    }
}
