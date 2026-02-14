//! Special construct lowering for V2 codegen.
//!
//! Handles Ori's unique expression patterns:
//! - `FunctionExp`: `print(...)`, `panic(...)`, `todo`, `recurse`, etc.
//! - `SelfRef`: recursive self-reference
//! - `Await`: async (stub)
//! - `WithCapability`: capability provision

use ori_ir::canon::{CanId, CanNamedExprRange};
use ori_ir::{FunctionExpKind, Name};
use ori_types::Idx;

use super::expr_lowerer::ExprLowerer;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // FunctionExp: print, panic, todo, recurse, etc.
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::FunctionExp { kind, props }`.
    ///
    /// `FunctionExp` is inlined in the canonical IR — the kind and props range
    /// are stored directly in the `CanExpr` variant, not behind an indirection.
    pub(crate) fn lower_function_exp(
        &mut self,
        kind: FunctionExpKind,
        props: CanNamedExprRange,
        expr_id: CanId,
    ) -> Option<ValueId> {
        match kind {
            FunctionExpKind::Print => self.lower_exp_print(props),
            FunctionExpKind::Panic => self.lower_exp_panic(props),
            FunctionExpKind::Todo => self.lower_exp_todo(),
            FunctionExpKind::Unreachable => self.lower_exp_unreachable(),
            FunctionExpKind::Recurse => self.lower_exp_recurse(props, expr_id),
            FunctionExpKind::Cache => self.lower_exp_cache(props, expr_id),
            FunctionExpKind::Catch => self.lower_exp_catch(props, expr_id),
            FunctionExpKind::Parallel => {
                tracing::warn!("parallel expression not yet implemented");
                self.builder.record_codegen_error();
                None
            }
            FunctionExpKind::Spawn => {
                tracing::warn!("spawn expression not yet implemented");
                self.builder.record_codegen_error();
                None
            }
            FunctionExpKind::Timeout => {
                tracing::warn!("timeout expression not yet implemented");
                self.builder.record_codegen_error();
                None
            }
            FunctionExpKind::With => {
                tracing::warn!("with expression not yet implemented");
                self.builder.record_codegen_error();
                None
            }
            FunctionExpKind::Channel
            | FunctionExpKind::ChannelIn
            | FunctionExpKind::ChannelOut
            | FunctionExpKind::ChannelAll => {
                tracing::warn!("{} expression not yet implemented", kind.name());
                self.builder.record_codegen_error();
                None
            }
        }
    }

    /// Lower `print(msg: expr)`.
    ///
    /// Dispatches to the appropriate `ori_print_*` runtime function
    /// based on the value type.
    fn lower_exp_print(&mut self, props: CanNamedExprRange) -> Option<ValueId> {
        let named_exprs = self.canon.arena.get_named_exprs(props);
        let msg_expr = named_exprs.iter().find(|ne| {
            let name = self.resolve_name(ne.name);
            name == "msg"
        })?;

        let val = self.lower(msg_expr.value)?;
        let val_type = self.expr_type(msg_expr.value);

        match val_type {
            Idx::INT | Idx::DURATION | Idx::SIZE => {
                let i64_ty = self.builder.i64_type();
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
    fn lower_exp_panic(&mut self, props: CanNamedExprRange) -> Option<ValueId> {
        let named_exprs = self.canon.arena.get_named_exprs(props);
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
    fn lower_exp_recurse(&mut self, props: CanNamedExprRange, _expr_id: CanId) -> Option<ValueId> {
        let named_exprs = self.canon.arena.get_named_exprs(props);
        let mut arg_vals = Vec::with_capacity(named_exprs.len());
        for ne in named_exprs {
            let val = self.lower(ne.value)?;
            arg_vals.push(val);
        }

        self.builder
            .call_tail(self.current_function, &arg_vals, "recurse")
    }

    /// Lower `cache(key: ..., value: ...)` — memoization.
    fn lower_exp_cache(&mut self, props: CanNamedExprRange, _expr_id: CanId) -> Option<ValueId> {
        // Simplified: just evaluate the value expression
        let named_exprs = self.canon.arena.get_named_exprs(props);
        for ne in named_exprs {
            let name = self.resolve_name(ne.name);
            if name == "value" || name == "expr" {
                return self.lower(ne.value);
            }
        }
        tracing::warn!("cache expression missing value property");
        self.builder.record_codegen_error();
        None
    }

    /// Lower `catch(expr: ..., handler: ...)` — error catching.
    fn lower_exp_catch(&mut self, props: CanNamedExprRange, _expr_id: CanId) -> Option<ValueId> {
        // Simplified: just evaluate the expr property
        let named_exprs = self.canon.arena.get_named_exprs(props);
        for ne in named_exprs {
            let name = self.resolve_name(ne.name);
            if name == "expr" || name == "value" {
                return self.lower(ne.value);
            }
        }
        tracing::warn!("catch expression missing expr property");
        self.builder.record_codegen_error();
        None
    }

    // -----------------------------------------------------------------------
    // SelfRef, Await, WithCapability
    // -----------------------------------------------------------------------

    /// Lower `CanExpr::SelfRef` — recursive reference to current function.
    ///
    /// Returns the current function as a pointer value.
    pub(crate) fn lower_self_ref(&mut self) -> Option<ValueId> {
        let func_val = self.builder.get_function_value(self.current_function);
        let ptr = func_val.as_global_value().as_pointer_value();
        Some(self.builder.intern_value(ptr.into()))
    }

    /// Lower `CanExpr::Await(inner)` — async (stub).
    ///
    /// For the sync runtime, await is a no-op: just evaluate the inner
    /// expression.
    pub(crate) fn lower_await(&mut self, inner: CanId) -> Option<ValueId> {
        self.lower(inner)
    }

    /// Lower `CanExpr::WithCapability { capability, provider, body }`.
    ///
    /// Capability system not yet implemented. For now, just evaluates
    /// the body expression.
    pub(crate) fn lower_with_capability(
        &mut self,
        _capability: Name,
        _provider: CanId,
        body: CanId,
    ) -> Option<ValueId> {
        self.lower(body)
    }
}
