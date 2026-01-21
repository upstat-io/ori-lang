// TIR-specific traversal helpers
//
// This module provides traversal utilities for the Typed Intermediate Representation (TIR).
// Unlike AST traversal, TIR traversal has access to:
// - Type information on every expression
// - Span information for error reporting
// - Resolved identifiers (LocalId, Param indices)
//
// The TIR uses different expression types (TExpr, TExprKind) so it needs its own
// traversal infrastructure.

use crate::ir::{TExpr, TExprKind, TPattern, Type};
use crate::ast::{BinaryOp, Span, UnaryOp};

/// Trait for traversing TIR expressions.
///
/// Similar to ExprTraversal but for typed expressions.
/// TIR traversal always has access to type and span information.
pub trait TExprTraversal: Sized {
    /// Whether to automatically recurse into subexpressions
    const AUTO_RECURSE: bool;

    /// Output type
    type Output;

    /// Error type
    type Error;

    /// Default result
    fn default_result(&mut self) -> Result<Self::Output, Self::Error>;

    /// Combine results
    fn combine_results(&mut self, a: Self::Output, b: Self::Output) -> Self::Output;

    /// Combine many results
    fn combine_many(&mut self, results: Vec<Self::Output>) -> Self::Output {
        let default = match self.default_result() {
            Ok(d) => d,
            Err(_) => return results.into_iter().next().unwrap_or_else(|| {
                panic!("combine_many requires at least one result or working default_result")
            }),
        };
        results.into_iter().fold(default, |acc, r| self.combine_results(acc, r))
    }

    /// Main entry point - traverse a typed expression
    fn traverse(&mut self, expr: &TExpr) -> Result<Self::Output, Self::Error> {
        self.traverse_kind(&expr.kind, &expr.ty, &expr.span)
    }

    /// Traverse by kind, type, and span
    fn traverse_kind(
        &mut self,
        kind: &TExprKind,
        ty: &Type,
        span: &Span,
    ) -> Result<Self::Output, Self::Error> {
        match kind {
            // Literals
            TExprKind::Int(n) => self.on_int(*n, ty, span),
            TExprKind::Float(f) => self.on_float(*f, ty, span),
            TExprKind::String(s) => self.on_string(s, ty, span),
            TExprKind::Bool(b) => self.on_bool(*b, ty, span),
            TExprKind::Nil => self.on_nil(ty, span),

            // References
            TExprKind::Local(id) => self.on_local(*id, ty, span),
            TExprKind::Param(idx) => self.on_param(*idx, ty, span),
            TExprKind::Config(name) => self.on_config(name, ty, span),
            TExprKind::LengthOf(inner) => self.on_length_of(inner, ty, span),

            // Collections
            TExprKind::List(elems) => self.on_list(elems, ty, span),
            TExprKind::MapLiteral(entries) => self.on_map_literal(entries, ty, span),
            TExprKind::Tuple(elems) => self.on_tuple(elems, ty, span),
            TExprKind::Struct { name, fields } => self.on_struct(name, fields, ty, span),

            // Access
            TExprKind::Field(obj, field) => self.on_field(obj, field, ty, span),
            TExprKind::Index(obj, idx) => self.on_index(obj, idx, ty, span),

            // Calls
            TExprKind::Call { func, args } => self.on_call(func, args, ty, span),
            TExprKind::MethodCall { receiver, method, args } => {
                self.on_method_call(receiver, method, args, ty, span)
            }

            // Operations
            TExprKind::Binary { op, left, right } => self.on_binary(*op, left, right, ty, span),
            TExprKind::Unary { op, operand } => self.on_unary(*op, operand, ty, span),

            // Lambda
            TExprKind::Lambda { params, captures, body } => {
                self.on_lambda(params, captures, body, ty, span)
            }

            // Control Flow
            TExprKind::Match(m) => self.on_match(m, ty, span),
            TExprKind::If { cond, then_branch, else_branch } => {
                self.on_if(cond, then_branch, else_branch, ty, span)
            }
            TExprKind::For { binding, iter, body } => {
                self.on_for(*binding, iter, body, ty, span)
            }
            TExprKind::Block(stmts, result) => self.on_block(stmts, result, ty, span),
            TExprKind::Range { start, end } => self.on_range(start, end, ty, span),

            // Patterns
            TExprKind::Pattern(p) => self.on_pattern(p, ty, span),

            // Result/Option
            TExprKind::Ok(inner) => self.on_ok(inner, ty, span),
            TExprKind::Err(inner) => self.on_err(inner, ty, span),
            TExprKind::Some(inner) => self.on_some(inner, ty, span),
            TExprKind::None_ => self.on_none(ty, span),
            TExprKind::Coalesce { value, default } => self.on_coalesce(value, default, ty, span),
            TExprKind::Unwrap(inner) => self.on_unwrap(inner, ty, span),

            // Assignment
            TExprKind::Assign { target, value } => self.on_assign(*target, value, ty, span),

            // Capability injection
            TExprKind::With { capability, implementation, body } => {
                self.on_with(capability, implementation, body, ty, span)
            }

            // Async
            TExprKind::Await(inner) => self.on_await(inner, ty, span),
        }
    }

    // =========================================================================
    // Handlers - default to returning default_result or recursing
    // =========================================================================

    fn on_int(&mut self, _n: i64, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_float(&mut self, _f: f64, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_string(&mut self, _s: &str, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_bool(&mut self, _b: bool, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_nil(&mut self, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_local(&mut self, _id: crate::ir::LocalId, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_param(&mut self, _idx: usize, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_config(&mut self, _name: &str, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_length_of(&mut self, inner: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(inner)
        } else {
            self.default_result()
        }
    }

    fn on_list(&mut self, elems: &[TExpr], _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let results: Result<Vec<_>, _> = elems.iter().map(|e| self.traverse(e)).collect();
            Ok(self.combine_many(results?))
        } else {
            self.default_result()
        }
    }

    fn on_map_literal(&mut self, entries: &[(TExpr, TExpr)], _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let mut results = Vec::new();
            for (k, v) in entries {
                results.push(self.traverse(k)?);
                results.push(self.traverse(v)?);
            }
            Ok(self.combine_many(results))
        } else {
            self.default_result()
        }
    }

    fn on_tuple(&mut self, elems: &[TExpr], _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let results: Result<Vec<_>, _> = elems.iter().map(|e| self.traverse(e)).collect();
            Ok(self.combine_many(results?))
        } else {
            self.default_result()
        }
    }

    fn on_struct(&mut self, _name: &str, fields: &[(String, TExpr)], _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let results: Result<Vec<_>, _> = fields.iter().map(|(_, e)| self.traverse(e)).collect();
            Ok(self.combine_many(results?))
        } else {
            self.default_result()
        }
    }

    fn on_field(&mut self, obj: &TExpr, _field: &str, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(obj)
        } else {
            self.default_result()
        }
    }

    fn on_index(&mut self, obj: &TExpr, idx: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let a = self.traverse(obj)?;
            let b = self.traverse(idx)?;
            Ok(self.combine_results(a, b))
        } else {
            self.default_result()
        }
    }

    fn on_call(&mut self, _func: &crate::ir::FuncRef, args: &[TExpr], _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let results: Result<Vec<_>, _> = args.iter().map(|e| self.traverse(e)).collect();
            Ok(self.combine_many(results?))
        } else {
            self.default_result()
        }
    }

    fn on_method_call(&mut self, receiver: &TExpr, _method: &str, args: &[TExpr], _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let recv = self.traverse(receiver)?;
            let arg_results: Result<Vec<_>, _> = args.iter().map(|e| self.traverse(e)).collect();
            let combined = self.combine_many(arg_results?);
            Ok(self.combine_results(recv, combined))
        } else {
            self.default_result()
        }
    }

    fn on_binary(&mut self, _op: BinaryOp, left: &TExpr, right: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let a = self.traverse(left)?;
            let b = self.traverse(right)?;
            Ok(self.combine_results(a, b))
        } else {
            self.default_result()
        }
    }

    fn on_unary(&mut self, _op: UnaryOp, operand: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(operand)
        } else {
            self.default_result()
        }
    }

    fn on_lambda(&mut self, _params: &[(String, Type)], _captures: &[crate::ir::LocalId], body: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(body)
        } else {
            self.default_result()
        }
    }

    fn on_match(&mut self, m: &crate::ir::TMatch, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let scrutinee = self.traverse(&m.scrutinee)?;
            let arm_results: Result<Vec<_>, _> = m.arms.iter().map(|arm| self.traverse(&arm.body)).collect();
            let combined = self.combine_many(arm_results?);
            Ok(self.combine_results(scrutinee, combined))
        } else {
            self.default_result()
        }
    }

    fn on_if(&mut self, cond: &TExpr, then_branch: &TExpr, else_branch: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let c = self.traverse(cond)?;
            let t = self.traverse(then_branch)?;
            let e = self.traverse(else_branch)?;
            let combined = self.combine_results(c, t);
            Ok(self.combine_results(combined, e))
        } else {
            self.default_result()
        }
    }

    fn on_for(&mut self, _binding: crate::ir::LocalId, iter: &TExpr, body: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let i = self.traverse(iter)?;
            let b = self.traverse(body)?;
            Ok(self.combine_results(i, b))
        } else {
            self.default_result()
        }
    }

    fn on_block(&mut self, stmts: &[crate::ir::TStmt], result: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let mut results = Vec::new();
            for stmt in stmts {
                if let crate::ir::TStmt::Let { value, .. } = stmt {
                    results.push(self.traverse(value)?);
                } else if let crate::ir::TStmt::Expr(e) = stmt {
                    results.push(self.traverse(e)?);
                }
            }
            results.push(self.traverse(result)?);
            Ok(self.combine_many(results))
        } else {
            self.default_result()
        }
    }

    fn on_range(&mut self, start: &TExpr, end: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let s = self.traverse(start)?;
            let e = self.traverse(end)?;
            Ok(self.combine_results(s, e))
        } else {
            self.default_result()
        }
    }

    fn on_pattern(&mut self, _p: &TPattern, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_ok(&mut self, inner: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(inner)
        } else {
            self.default_result()
        }
    }

    fn on_err(&mut self, inner: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(inner)
        } else {
            self.default_result()
        }
    }

    fn on_some(&mut self, inner: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(inner)
        } else {
            self.default_result()
        }
    }

    fn on_none(&mut self, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_coalesce(&mut self, value: &TExpr, default: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let v = self.traverse(value)?;
            let d = self.traverse(default)?;
            Ok(self.combine_results(v, d))
        } else {
            self.default_result()
        }
    }

    fn on_unwrap(&mut self, inner: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(inner)
        } else {
            self.default_result()
        }
    }

    fn on_assign(&mut self, _target: crate::ir::LocalId, value: &TExpr, _ty: &Type, _span: &Span) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(value)
        } else {
            self.default_result()
        }
    }

    fn on_with(
        &mut self,
        _capability: &str,
        implementation: &TExpr,
        body: &TExpr,
        _ty: &Type,
        _span: &Span,
    ) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let i = self.traverse(implementation)?;
            let b = self.traverse(body)?;
            Ok(self.combine_results(i, b))
        } else {
            self.default_result()
        }
    }

    fn on_await(
        &mut self,
        inner: &TExpr,
        _ty: &Type,
        _span: &Span,
    ) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(inner)
        } else {
            self.default_result()
        }
    }
}
