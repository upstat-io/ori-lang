//! Special construct lowering for ARC IR.
//!
//! Handles Ori's unique expression patterns:
//! - `FunctionExp`: `print(...)`, `panic(...)`, `todo`, `recurse`, etc.
//! - `FormatWith`: template string format specs (`{value:>10.2f}`)

use ori_ir::canon::CanNamedExprRange;
use ori_ir::{FunctionExpKind, Name, Span};
use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue};

use super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    // FunctionExp dispatch

    /// Lower `CanExpr::FunctionExp { kind, props }` to ARC IR.
    ///
    /// Routes to type-specific lowering based on `FunctionExpKind`.
    /// Post-0.1 concurrency variants (Parallel, Spawn, Timeout, With, Channel*)
    /// are rejected at the type checker (E2040) and never reach here.
    pub(crate) fn lower_function_exp(
        &mut self,
        kind: FunctionExpKind,
        props: CanNamedExprRange,
        span: Span,
    ) -> ArcVarId {
        match kind {
            FunctionExpKind::Print => self.lower_exp_print(props, span),
            FunctionExpKind::Panic => self.lower_exp_panic(props, span),
            FunctionExpKind::Todo => self.lower_exp_todo(span),
            FunctionExpKind::Unreachable => self.lower_exp_unreachable(span),
            FunctionExpKind::Recurse => self.lower_exp_recurse(props, span),
            FunctionExpKind::Cache => self.lower_exp_cache(props, span),
            FunctionExpKind::Catch => self.lower_exp_catch(props, span),
            // Post-0.1-alpha — rejected by type checker (E2040), never reaches lowerer
            FunctionExpKind::Parallel
            | FunctionExpKind::Spawn
            | FunctionExpKind::Timeout
            | FunctionExpKind::With
            | FunctionExpKind::Channel
            | FunctionExpKind::ChannelIn
            | FunctionExpKind::ChannelOut
            | FunctionExpKind::ChannelAll => {
                unreachable!(
                    "post-0.1 concurrency feature `{}` should be rejected by type checker (E2040)",
                    kind.name()
                )
            }
        }
    }

    // Print

    /// Lower `print(msg: expr)` — dispatches to `ori_print_*` based on type.
    fn lower_exp_print(&mut self, props: CanNamedExprRange, span: Span) -> ArcVarId {
        let named_exprs = self.arena.get_named_exprs(props);
        let msg_name = self.interner.intern("msg");
        let message_name = self.interner.intern("message");
        let value_name = self.interner.intern("value");

        let msg_expr = named_exprs
            .iter()
            .find(|ne| ne.name == msg_name || ne.name == message_name || ne.name == value_name);

        let Some(ne) = msg_expr else {
            return self.emit_unit();
        };

        let val = self.lower_expr(ne.value);
        let val_type = self.expr_type(ne.value);
        let runtime_fn = match val_type {
            Idx::FLOAT => "ori_print_float",
            Idx::BOOL => "ori_print_bool",
            Idx::STR => "ori_print",
            _ => "ori_print_int",
        };

        let fn_name = self.interner.intern(runtime_fn);
        self.builder
            .emit_apply(Idx::UNIT, fn_name, vec![val], Some(span));
        self.emit_unit()
    }

    // Panic, Todo, Unreachable

    /// Lower `panic(message: expr)` — calls `ori_panic`/`ori_panic_cstr`, emits unreachable.
    fn lower_exp_panic(&mut self, props: CanNamedExprRange, span: Span) -> ArcVarId {
        let named_exprs = self.arena.get_named_exprs(props);
        let message_name = self.interner.intern("message");
        let value_name = self.interner.intern("value");
        let msg_name = self.interner.intern("msg");

        let msg_expr = named_exprs
            .iter()
            .find(|ne| ne.name == message_name || ne.name == value_name || ne.name == msg_name);

        if let Some(ne) = msg_expr {
            let val = self.lower_expr(ne.value);
            let val_type = self.expr_type(ne.value);

            let fn_name = if val_type == Idx::STR {
                self.interner.intern("ori_panic")
            } else {
                self.interner.intern("ori_panic_cstr")
            };

            self.builder
                .emit_apply(Idx::UNIT, fn_name, vec![val], Some(span));
        } else {
            let msg = self.interner.intern("explicit panic");
            let msg_var = self.builder.emit_let(
                Idx::STR,
                ArcValue::Literal(LitValue::String(msg)),
                Some(span),
            );
            let fn_name = self.interner.intern("ori_panic_cstr");
            self.builder
                .emit_apply(Idx::UNIT, fn_name, vec![msg_var], Some(span));
        }

        self.builder.terminate_unreachable();
        self.emit_unit_in_new_block()
    }

    /// Lower `todo` — panics with "not yet implemented".
    fn lower_exp_todo(&mut self, span: Span) -> ArcVarId {
        self.lower_panic_with_message("not yet implemented", span)
    }

    /// Lower `unreachable` — panics with "reached unreachable code".
    fn lower_exp_unreachable(&mut self, span: Span) -> ArcVarId {
        self.lower_panic_with_message("reached unreachable code", span)
    }

    /// Shared helper: emit `ori_panic_cstr(msg)` + unreachable.
    fn lower_panic_with_message(&mut self, message: &str, span: Span) -> ArcVarId {
        let msg = self.interner.intern(message);
        let msg_var = self.builder.emit_let(
            Idx::STR,
            ArcValue::Literal(LitValue::String(msg)),
            Some(span),
        );
        let fn_name = self.interner.intern("ori_panic_cstr");
        self.builder
            .emit_apply(Idx::UNIT, fn_name, vec![msg_var], Some(span));
        self.builder.terminate_unreachable();
        self.emit_unit_in_new_block()
    }

    // Recurse

    /// Lower `recurse(args...)` — tail call to current function.
    ///
    /// In ARC IR, this is a regular `Apply` to the enclosing function.
    /// The ARC pipeline and LLVM backend handle tail call optimization.
    fn lower_exp_recurse(&mut self, props: CanNamedExprRange, span: Span) -> ArcVarId {
        let named_exprs = self.arena.get_named_exprs(props);
        let mut arg_vars = Vec::with_capacity(named_exprs.len());
        for ne in named_exprs {
            arg_vars.push(self.lower_expr(ne.value));
        }

        // The function name is available through the ARC IR builder's
        // function context. For recurse, we use the enclosing function name.
        // Since ARC IR doesn't track the current function name in the lowerer,
        // emit as an Apply to a special `__recurse` sentinel that the emitter
        // will resolve to the current function.
        let recurse_name = self.interner.intern("__recurse");
        self.builder
            .emit_apply(Idx::UNIT, recurse_name, arg_vars, Some(span))
    }

    // Cache, Catch

    /// Lower `cache(value: expr)` — simplified, just evaluate the value expression.
    fn lower_exp_cache(&mut self, props: CanNamedExprRange, span: Span) -> ArcVarId {
        self.lower_named_prop(props, &["value", "expr"], span)
    }

    /// Lower `catch(expr: expr)` — simplified, just evaluate the expr.
    fn lower_exp_catch(&mut self, props: CanNamedExprRange, span: Span) -> ArcVarId {
        self.lower_named_prop(props, &["expr", "value"], span)
    }

    /// Helper: find and lower the first matching named prop.
    fn lower_named_prop(
        &mut self,
        props: CanNamedExprRange,
        names: &[&str],
        _span: Span,
    ) -> ArcVarId {
        let named_exprs = self.arena.get_named_exprs(props);
        let interned: Vec<Name> = names.iter().map(|n| self.interner.intern(n)).collect();
        for ne in named_exprs {
            if interned.contains(&ne.name) {
                return self.lower_expr(ne.value);
            }
        }
        self.emit_unit()
    }

    // FormatWith

    /// Lower `CanExpr::FormatWith { expr, spec }` to ARC IR.
    ///
    /// Dispatches to type-specific `ori_format_*` runtime functions.
    pub(crate) fn lower_format_with(
        &mut self,
        expr: ori_ir::canon::CanId,
        spec: Name,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let inner_ty = self.expr_type(expr);
        let val = self.lower_expr(expr);

        // Empty spec on a string: return it directly (no formatting needed)
        let spec_str = self.interner.lookup(spec);
        if spec_str.is_empty() && inner_ty == Idx::STR {
            return val;
        }

        // Create spec string literal
        let spec_var = self.builder.emit_let(
            Idx::STR,
            ArcValue::Literal(LitValue::String(spec)),
            Some(span),
        );

        let runtime_fn = match inner_ty {
            Idx::FLOAT => "ori_format_float",
            Idx::BOOL => "ori_format_bool",
            Idx::CHAR => "ori_format_char",
            Idx::STR => "ori_format_str",
            _ => "ori_format_int",
        };

        let fn_name = self.interner.intern(runtime_fn);
        self.builder
            .emit_apply(ty, fn_name, vec![val, spec_var], Some(span))
    }

    // Helpers

    /// Emit a unit literal in a fresh block after a terminator.
    ///
    /// Used after `terminate_unreachable()` to provide a valid `ArcVarId`
    /// for subsequent code (which will be dead but must be well-formed).
    fn emit_unit_in_new_block(&mut self) -> ArcVarId {
        let dead_block = self.builder.new_block();
        self.builder.position_at(dead_block);
        self.emit_unit()
    }
}
