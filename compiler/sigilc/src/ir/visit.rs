// Expression visitor for Sigil TIR
//
// Provides a trait for visiting TIR expressions and producing results.
// Unlike Folder (which transforms TExpr → TExpr), Visitor produces an
// arbitrary result type R. Useful for:
// - Code generation (R = Result<String, String>)
// - Display/formatting (R = ())
// - Analysis (R = HashSet<String>)
//
// Implementors override only the methods for nodes they care about.
// Adding a new expression type only requires adding a default here.

use super::expr::{FuncRef, LocalId, TExpr, TExprKind, TMatch, TMatchPattern, TStmt};
use super::patterns::TPattern;
use super::types::Type;
use crate::ast::{BinaryOp, Span, UnaryOp};

/// Trait for visiting TIR expressions and producing results.
/// Override only the methods you need - defaults handle recursion.
///
/// The trait is parameterized by:
/// - `R`: The result type (e.g., String for codegen, () for side-effect visitors)
///
/// Implementors must provide:
/// - `default_result()`: The base case result
/// - `combine_results()`: How to merge results from subexpressions
pub trait Visitor {
    /// The result type produced by visiting
    type Result;

    /// The default result when no transformation occurs
    fn default_result(&self) -> Self::Result;

    /// Combine two results (for binary ops, etc.)
    fn combine_results(&self, a: Self::Result, b: Self::Result) -> Self::Result;

    /// Combine multiple results
    fn combine_many(&self, results: Vec<Self::Result>) -> Self::Result {
        results
            .into_iter()
            .fold(self.default_result(), |acc, r| self.combine_results(acc, r))
    }

    /// Main entry point - usually don't override this
    fn visit_expr(&mut self, expr: &TExpr) -> Self::Result {
        self.visit_expr_kind(&expr.kind, &expr.ty, &expr.span)
    }

    /// Dispatch based on expression kind - usually don't override this
    fn visit_expr_kind(&mut self, kind: &TExprKind, ty: &Type, span: &Span) -> Self::Result {
        match kind {
            TExprKind::Int(n) => self.visit_int(*n, ty, span),
            TExprKind::Float(f) => self.visit_float(*f, ty, span),
            TExprKind::String(s) => self.visit_string(s, ty, span),
            TExprKind::Bool(b) => self.visit_bool(*b, ty, span),
            TExprKind::Nil => self.visit_nil(ty, span),

            TExprKind::Local(id) => self.visit_local(*id, ty, span),
            TExprKind::Param(idx) => self.visit_param(*idx, ty, span),
            TExprKind::Config(name) => self.visit_config(name, ty, span),

            TExprKind::List(elems) => self.visit_list(elems, ty, span),
            TExprKind::MapLiteral(entries) => self.visit_map_literal(entries, ty, span),
            TExprKind::Tuple(elems) => self.visit_tuple(elems, ty, span),
            TExprKind::Struct { name, fields } => self.visit_struct(name, fields, ty, span),

            TExprKind::Binary { op, left, right } => self.visit_binary(*op, left, right, ty, span),
            TExprKind::Unary { op, operand } => self.visit_unary(*op, operand, ty, span),

            TExprKind::Field(obj, field) => self.visit_field(obj, field, ty, span),
            TExprKind::Index(obj, idx) => self.visit_index(obj, idx, ty, span),
            TExprKind::LengthOf(obj) => self.visit_length_of(obj, ty, span),

            TExprKind::Call { func, args } => self.visit_call(func, args, ty, span),
            TExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.visit_method_call(receiver, method, args, ty, span),

            TExprKind::Lambda {
                params,
                captures,
                body,
            } => self.visit_lambda(params, captures, body, ty, span),

            TExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.visit_if(cond, then_branch, else_branch, ty, span),
            TExprKind::Match(m) => self.visit_match(m, ty, span),
            TExprKind::Block(stmts, result) => self.visit_block(stmts, result, ty, span),
            TExprKind::For { binding, iter, body } => self.visit_for(*binding, iter, body, ty, span),

            TExprKind::Assign { target, value } => self.visit_assign(*target, value, ty, span),
            TExprKind::Range { start, end } => self.visit_range(start, end, ty, span),

            TExprKind::Pattern(p) => self.visit_pattern(p, ty, span),

            TExprKind::Ok(inner) => self.visit_ok(inner, ty, span),
            TExprKind::Err(inner) => self.visit_err(inner, ty, span),
            TExprKind::Some(inner) => self.visit_some(inner, ty, span),
            TExprKind::None_ => self.visit_none(ty, span),
            TExprKind::Coalesce { value, default } => self.visit_coalesce(value, default, ty, span),
            TExprKind::Unwrap(inner) => self.visit_unwrap(inner, ty, span),
        }
    }

    // === Literals (leaf nodes) ===

    fn visit_int(&mut self, _n: i64, _ty: &Type, _span: &Span) -> Self::Result {
        self.default_result()
    }

    fn visit_float(&mut self, _f: f64, _ty: &Type, _span: &Span) -> Self::Result {
        self.default_result()
    }

    fn visit_string(&mut self, _s: &str, _ty: &Type, _span: &Span) -> Self::Result {
        self.default_result()
    }

    fn visit_bool(&mut self, _b: bool, _ty: &Type, _span: &Span) -> Self::Result {
        self.default_result()
    }

    fn visit_nil(&mut self, _ty: &Type, _span: &Span) -> Self::Result {
        self.default_result()
    }

    // === Variables (leaf nodes) ===

    fn visit_local(&mut self, _id: LocalId, _ty: &Type, _span: &Span) -> Self::Result {
        self.default_result()
    }

    fn visit_param(&mut self, _idx: usize, _ty: &Type, _span: &Span) -> Self::Result {
        self.default_result()
    }

    fn visit_config(&mut self, _name: &str, _ty: &Type, _span: &Span) -> Self::Result {
        self.default_result()
    }

    // === Collections ===

    fn visit_list(&mut self, elems: &[TExpr], _ty: &Type, _span: &Span) -> Self::Result {
        let results: Vec<_> = elems.iter().map(|e| self.visit_expr(e)).collect();
        self.combine_many(results)
    }

    fn visit_map_literal(
        &mut self,
        entries: &[(TExpr, TExpr)],
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        let results: Vec<_> = entries
            .iter()
            .flat_map(|(k, v)| vec![self.visit_expr(k), self.visit_expr(v)])
            .collect();
        self.combine_many(results)
    }

    fn visit_tuple(&mut self, elems: &[TExpr], _ty: &Type, _span: &Span) -> Self::Result {
        let results: Vec<_> = elems.iter().map(|e| self.visit_expr(e)).collect();
        self.combine_many(results)
    }

    fn visit_struct(
        &mut self,
        _name: &str,
        fields: &[(String, TExpr)],
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        let results: Vec<_> = fields.iter().map(|(_, e)| self.visit_expr(e)).collect();
        self.combine_many(results)
    }

    // === Operations ===

    fn visit_binary(
        &mut self,
        _op: BinaryOp,
        left: &TExpr,
        right: &TExpr,
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        let l = self.visit_expr(left);
        let r = self.visit_expr(right);
        self.combine_results(l, r)
    }

    fn visit_unary(
        &mut self,
        _op: UnaryOp,
        operand: &TExpr,
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        self.visit_expr(operand)
    }

    // === Access ===

    fn visit_field(&mut self, obj: &TExpr, _field: &str, _ty: &Type, _span: &Span) -> Self::Result {
        self.visit_expr(obj)
    }

    fn visit_index(&mut self, obj: &TExpr, idx: &TExpr, _ty: &Type, _span: &Span) -> Self::Result {
        let o = self.visit_expr(obj);
        let i = self.visit_expr(idx);
        self.combine_results(o, i)
    }

    fn visit_length_of(&mut self, obj: &TExpr, _ty: &Type, _span: &Span) -> Self::Result {
        self.visit_expr(obj)
    }

    // === Calls ===

    fn visit_call(
        &mut self,
        _func: &FuncRef,
        args: &[TExpr],
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        let results: Vec<_> = args.iter().map(|a| self.visit_expr(a)).collect();
        self.combine_many(results)
    }

    fn visit_method_call(
        &mut self,
        receiver: &TExpr,
        _method: &str,
        args: &[TExpr],
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        let r = self.visit_expr(receiver);
        let arg_results: Vec<_> = args.iter().map(|a| self.visit_expr(a)).collect();
        self.combine_results(r, self.combine_many(arg_results))
    }

    // === Lambda ===

    fn visit_lambda(
        &mut self,
        _params: &[(String, Type)],
        _captures: &[LocalId],
        body: &TExpr,
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        self.visit_expr(body)
    }

    // === Control flow ===

    fn visit_if(
        &mut self,
        cond: &TExpr,
        then_branch: &TExpr,
        else_branch: &TExpr,
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        let c = self.visit_expr(cond);
        let t = self.visit_expr(then_branch);
        let e = self.visit_expr(else_branch);
        self.combine_results(c, self.combine_results(t, e))
    }

    fn visit_match(&mut self, m: &TMatch, _ty: &Type, _span: &Span) -> Self::Result {
        let s = self.visit_expr(&m.scrutinee);
        let arm_results: Vec<_> = m
            .arms
            .iter()
            .map(|arm| {
                let p = self.visit_match_pattern(&arm.pattern);
                let b = self.visit_expr(&arm.body);
                self.combine_results(p, b)
            })
            .collect();
        self.combine_results(s, self.combine_many(arm_results))
    }

    fn visit_match_pattern(&mut self, pattern: &TMatchPattern) -> Self::Result {
        match pattern {
            TMatchPattern::Literal(expr) => self.visit_expr(expr),
            TMatchPattern::Condition(expr) => self.visit_expr(expr),
            _ => self.default_result(),
        }
    }

    fn visit_block(
        &mut self,
        stmts: &[TStmt],
        result: &TExpr,
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        let stmt_results: Vec<_> = stmts.iter().map(|s| self.visit_stmt(s)).collect();
        let r = self.visit_expr(result);
        self.combine_results(self.combine_many(stmt_results), r)
    }

    fn visit_stmt(&mut self, stmt: &TStmt) -> Self::Result {
        match stmt {
            TStmt::Expr(e) => self.visit_expr(e),
            TStmt::Let { value, .. } => self.visit_expr(value),
        }
    }

    fn visit_for(
        &mut self,
        _binding: LocalId,
        iter: &TExpr,
        body: &TExpr,
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        let i = self.visit_expr(iter);
        let b = self.visit_expr(body);
        self.combine_results(i, b)
    }

    // === Assignment and Range ===

    fn visit_assign(
        &mut self,
        _target: LocalId,
        value: &TExpr,
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        self.visit_expr(value)
    }

    fn visit_range(&mut self, start: &TExpr, end: &TExpr, _ty: &Type, _span: &Span) -> Self::Result {
        let s = self.visit_expr(start);
        let e = self.visit_expr(end);
        self.combine_results(s, e)
    }

    // === Patterns ===

    fn visit_pattern(&mut self, pattern: &TPattern, _ty: &Type, _span: &Span) -> Self::Result {
        self.visit_tpattern(pattern)
    }

    fn visit_tpattern(&mut self, pattern: &TPattern) -> Self::Result {
        match pattern {
            TPattern::Fold {
                collection,
                init,
                op,
                ..
            } => {
                let c = self.visit_expr(collection);
                let i = self.visit_expr(init);
                let o = self.visit_expr(op);
                self.combine_results(c, self.combine_results(i, o))
            }
            TPattern::Map {
                collection,
                transform,
                ..
            } => {
                let c = self.visit_expr(collection);
                let t = self.visit_expr(transform);
                self.combine_results(c, t)
            }
            TPattern::Filter {
                collection,
                predicate,
                ..
            } => {
                let c = self.visit_expr(collection);
                let p = self.visit_expr(predicate);
                self.combine_results(c, p)
            }
            TPattern::Collect {
                range, transform, ..
            } => {
                let r = self.visit_expr(range);
                let t = self.visit_expr(transform);
                self.combine_results(r, t)
            }
            TPattern::Recurse {
                cond, base, step, ..
            } => {
                let c = self.visit_expr(cond);
                let b = self.visit_expr(base);
                let s = self.visit_expr(step);
                self.combine_results(c, self.combine_results(b, s))
            }
            TPattern::Iterate {
                over, into, with, ..
            } => {
                let o = self.visit_expr(over);
                let i = self.visit_expr(into);
                let w = self.visit_expr(with);
                self.combine_results(o, self.combine_results(i, w))
            }
            TPattern::Transform { input, steps, .. } => {
                let i = self.visit_expr(input);
                let step_results: Vec<_> = steps.iter().map(|s| self.visit_expr(s)).collect();
                self.combine_results(i, self.combine_many(step_results))
            }
            TPattern::Count {
                collection,
                predicate,
                ..
            } => {
                let c = self.visit_expr(collection);
                let p = self.visit_expr(predicate);
                self.combine_results(c, p)
            }
            TPattern::Parallel { branches, .. } => {
                let results: Vec<_> = branches.iter().map(|(_, e, _)| self.visit_expr(e)).collect();
                self.combine_many(results)
            }
        }
    }

    // === Result/Option ===

    fn visit_ok(&mut self, inner: &TExpr, _ty: &Type, _span: &Span) -> Self::Result {
        self.visit_expr(inner)
    }

    fn visit_err(&mut self, inner: &TExpr, _ty: &Type, _span: &Span) -> Self::Result {
        self.visit_expr(inner)
    }

    fn visit_some(&mut self, inner: &TExpr, _ty: &Type, _span: &Span) -> Self::Result {
        self.visit_expr(inner)
    }

    fn visit_none(&mut self, _ty: &Type, _span: &Span) -> Self::Result {
        self.default_result()
    }

    fn visit_coalesce(
        &mut self,
        value: &TExpr,
        default: &TExpr,
        _ty: &Type,
        _span: &Span,
    ) -> Self::Result {
        let v = self.visit_expr(value);
        let d = self.visit_expr(default);
        self.combine_results(v, d)
    }

    fn visit_unwrap(&mut self, inner: &TExpr, _ty: &Type, _span: &Span) -> Self::Result {
        self.visit_expr(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::expr::FuncRef;
    use std::collections::HashSet;

    /// Example visitor that counts nodes
    struct NodeCounter;

    impl Visitor for NodeCounter {
        type Result = usize;

        fn default_result(&self) -> usize {
            1 // Each node counts as 1
        }

        fn combine_results(&self, a: usize, b: usize) -> usize {
            a + b
        }
    }

    #[test]
    fn test_node_counter() {
        let mut counter = NodeCounter;
        let expr = TExpr::new(TExprKind::Int(42), Type::Int, 0..1);
        let count = counter.visit_expr(&expr);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_binary_combines_children() {
        let mut counter = NodeCounter;
        let expr = TExpr::new(
            TExprKind::Binary {
                op: BinaryOp::Add,
                left: Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
                right: Box::new(TExpr::new(TExprKind::Int(2), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        let count = counter.visit_expr(&expr);
        // Binary combines children: 1 + 1 = 2 (doesn't add 1 for itself)
        assert_eq!(count, 2);
    }

    #[test]
    fn test_visit_literals() {
        let mut counter = NodeCounter;

        // Float
        let expr = TExpr::new(TExprKind::Float(3.14), Type::Float, 0..1);
        assert_eq!(counter.visit_expr(&expr), 1);

        // String
        let expr = TExpr::new(TExprKind::String("hi".to_string()), Type::Str, 0..1);
        assert_eq!(counter.visit_expr(&expr), 1);

        // Bool
        let expr = TExpr::new(TExprKind::Bool(true), Type::Bool, 0..1);
        assert_eq!(counter.visit_expr(&expr), 1);

        // Nil
        let expr = TExpr::new(TExprKind::Nil, Type::Void, 0..1);
        assert_eq!(counter.visit_expr(&expr), 1);
    }

    #[test]
    fn test_visit_variables() {
        let mut counter = NodeCounter;

        // Local
        let expr = TExpr::new(TExprKind::Local(LocalId(0)), Type::Int, 0..1);
        assert_eq!(counter.visit_expr(&expr), 1);

        // Param
        let expr = TExpr::new(TExprKind::Param(0), Type::Int, 0..1);
        assert_eq!(counter.visit_expr(&expr), 1);

        // Config
        let expr = TExpr::new(TExprKind::Config("cfg".to_string()), Type::Int, 0..1);
        assert_eq!(counter.visit_expr(&expr), 1);
    }

    #[test]
    fn test_visit_collections() {
        let mut counter = NodeCounter;

        // List with 2 elements
        let expr = TExpr::new(
            TExprKind::List(vec![
                TExpr::new(TExprKind::Int(1), Type::Int, 0..1),
                TExpr::new(TExprKind::Int(2), Type::Int, 0..1),
            ]),
            Type::List(Box::new(Type::Int)),
            0..1,
        );
        // combine_many: 1 (default) + 1 + 1 = 3
        assert_eq!(counter.visit_expr(&expr), 3);

        // Tuple
        let expr = TExpr::new(
            TExprKind::Tuple(vec![
                TExpr::new(TExprKind::Int(1), Type::Int, 0..1),
            ]),
            Type::Tuple(vec![Type::Int]),
            0..1,
        );
        // combine_many: 1 (default) + 1 = 2
        assert_eq!(counter.visit_expr(&expr), 2);

        // Struct
        let expr = TExpr::new(
            TExprKind::Struct {
                name: "Point".to_string(),
                fields: vec![
                    ("x".to_string(), TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
                ],
            },
            Type::Struct {
                name: "Point".to_string(),
                fields: vec![("x".to_string(), Type::Int)],
            },
            0..1,
        );
        assert_eq!(counter.visit_expr(&expr), 2);
    }

    #[test]
    fn test_visit_operations() {
        let mut counter = NodeCounter;

        // Unary
        let expr = TExpr::new(
            TExprKind::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        assert_eq!(counter.visit_expr(&expr), 1);
    }

    #[test]
    fn test_visit_access() {
        let mut counter = NodeCounter;

        // Field
        let expr = TExpr::new(
            TExprKind::Field(
                Box::new(TExpr::new(TExprKind::Local(LocalId(0)), Type::Int, 0..1)),
                "x".to_string(),
            ),
            Type::Int,
            0..1,
        );
        assert_eq!(counter.visit_expr(&expr), 1);

        // Index
        let expr = TExpr::new(
            TExprKind::Index(
                Box::new(TExpr::new(TExprKind::Local(LocalId(0)), Type::Int, 0..1)),
                Box::new(TExpr::new(TExprKind::Int(0), Type::Int, 0..1)),
            ),
            Type::Int,
            0..1,
        );
        // obj + idx = 1 + 1 = 2
        assert_eq!(counter.visit_expr(&expr), 2);

        // LengthOf
        let expr = TExpr::new(
            TExprKind::LengthOf(Box::new(TExpr::new(
                TExprKind::List(vec![]),
                Type::List(Box::new(Type::Int)),
                0..1,
            ))),
            Type::Int,
            0..1,
        );
        // Empty list: combine_many(default=1) = 1, so LengthOf visits it = 1
        assert_eq!(counter.visit_expr(&expr), 1);
    }

    #[test]
    fn test_visit_calls() {
        let mut counter = NodeCounter;

        // Call with 1 arg
        let expr = TExpr::new(
            TExprKind::Call {
                func: FuncRef::Builtin("len".to_string()),
                args: vec![TExpr::new(TExprKind::Int(1), Type::Int, 0..1)],
            },
            Type::Int,
            0..1,
        );
        // combine_many: 1 (default) + 1 (arg) = 2
        assert_eq!(counter.visit_expr(&expr), 2);

        // MethodCall
        let expr = TExpr::new(
            TExprKind::MethodCall {
                receiver: Box::new(TExpr::new(TExprKind::String("hi".to_string()), Type::Str, 0..1)),
                method: "upper".to_string(),
                args: vec![],
            },
            Type::Str,
            0..1,
        );
        // receiver (1) + combine_many(args: default 1) = 1 + 1 = 2
        assert_eq!(counter.visit_expr(&expr), 2);
    }

    #[test]
    fn test_visit_control_flow() {
        let mut counter = NodeCounter;

        // If
        let expr = TExpr::new(
            TExprKind::If {
                cond: Box::new(TExpr::new(TExprKind::Bool(true), Type::Bool, 0..1)),
                then_branch: Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
                else_branch: Box::new(TExpr::new(TExprKind::Int(0), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        // cond(1) + (then(1) + else(1)) = 1 + 2 = 3
        assert_eq!(counter.visit_expr(&expr), 3);

        // For
        let expr = TExpr::new(
            TExprKind::For {
                binding: LocalId(0),
                iter: Box::new(TExpr::new(TExprKind::Local(LocalId(1)), Type::Int, 0..1)),
                body: Box::new(TExpr::new(TExprKind::Int(0), Type::Int, 0..1)),
            },
            Type::Void,
            0..1,
        );
        // iter(1) + body(1) = 2
        assert_eq!(counter.visit_expr(&expr), 2);

        // Block
        let expr = TExpr::new(
            TExprKind::Block(
                vec![TStmt::Expr(TExpr::new(TExprKind::Int(1), Type::Int, 0..1))],
                Box::new(TExpr::new(TExprKind::Int(2), Type::Int, 0..1)),
            ),
            Type::Int,
            0..1,
        );
        // stmts: combine_many(1 stmt = default(1) + 1) = 2
        // + result(1) = 3
        assert_eq!(counter.visit_expr(&expr), 3);
    }

    #[test]
    fn test_visit_result_option() {
        let mut counter = NodeCounter;

        // Ok
        let expr = TExpr::new(
            TExprKind::Ok(Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1))),
            Type::Result(Box::new(Type::Int), Box::new(Type::Str)),
            0..1,
        );
        assert_eq!(counter.visit_expr(&expr), 1);

        // Err
        let expr = TExpr::new(
            TExprKind::Err(Box::new(TExpr::new(TExprKind::String("e".to_string()), Type::Str, 0..1))),
            Type::Result(Box::new(Type::Int), Box::new(Type::Str)),
            0..1,
        );
        assert_eq!(counter.visit_expr(&expr), 1);

        // Some
        let expr = TExpr::new(
            TExprKind::Some(Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1))),
            Type::Option(Box::new(Type::Int)),
            0..1,
        );
        assert_eq!(counter.visit_expr(&expr), 1);

        // None
        let expr = TExpr::new(TExprKind::None_, Type::Option(Box::new(Type::Int)), 0..1);
        assert_eq!(counter.visit_expr(&expr), 1);

        // Coalesce
        let expr = TExpr::new(
            TExprKind::Coalesce {
                value: Box::new(TExpr::new(TExprKind::None_, Type::Option(Box::new(Type::Int)), 0..1)),
                default: Box::new(TExpr::new(TExprKind::Int(0), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        // value(1) + default(1) = 2
        assert_eq!(counter.visit_expr(&expr), 2);

        // Unwrap
        let expr = TExpr::new(
            TExprKind::Unwrap(Box::new(TExpr::new(
                TExprKind::Some(Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1))),
                Type::Option(Box::new(Type::Int)),
                0..1,
            ))),
            Type::Int,
            0..1,
        );
        assert_eq!(counter.visit_expr(&expr), 1);
    }

    #[test]
    fn test_visit_other() {
        let mut counter = NodeCounter;

        // Lambda
        let expr = TExpr::new(
            TExprKind::Lambda {
                params: vec![("x".to_string(), Type::Int)],
                captures: vec![],
                body: Box::new(TExpr::new(TExprKind::Param(0), Type::Int, 0..1)),
            },
            Type::Function { params: vec![Type::Int], ret: Box::new(Type::Int) },
            0..1,
        );
        assert_eq!(counter.visit_expr(&expr), 1);

        // Assign
        let expr = TExpr::new(
            TExprKind::Assign {
                target: LocalId(0),
                value: Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        assert_eq!(counter.visit_expr(&expr), 1);

        // Range
        let expr = TExpr::new(
            TExprKind::Range {
                start: Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
                end: Box::new(TExpr::new(TExprKind::Int(10), Type::Int, 0..1)),
            },
            Type::Named("Range".to_string()),
            0..1,
        );
        // start(1) + end(1) = 2
        assert_eq!(counter.visit_expr(&expr), 2);

        // MapLiteral
        let expr = TExpr::new(
            TExprKind::MapLiteral(vec![
                (
                    TExpr::new(TExprKind::String("k".to_string()), Type::Str, 0..1),
                    TExpr::new(TExprKind::Int(1), Type::Int, 0..1),
                ),
            ]),
            Type::Map(Box::new(Type::Str), Box::new(Type::Int)),
            0..1,
        );
        // combine_many: default(1) + k(1) + v(1) = 3
        assert_eq!(counter.visit_expr(&expr), 3);
    }

    /// Visitor that collects all string literals
    struct StringCollector {
        strings: Vec<String>,
    }

    impl Visitor for StringCollector {
        type Result = ();

        fn default_result(&self) {}

        fn combine_results(&self, _a: (), _b: ()) {}

        fn visit_string(&mut self, s: &str, _ty: &Type, _span: &Span) {
            self.strings.push(s.to_string());
        }
    }

    #[test]
    fn test_custom_visitor_collects_strings() {
        let mut collector = StringCollector { strings: vec![] };

        let expr = TExpr::new(
            TExprKind::Binary {
                op: BinaryOp::Add,
                left: Box::new(TExpr::new(TExprKind::String("hello".to_string()), Type::Str, 0..1)),
                right: Box::new(TExpr::new(TExprKind::String("world".to_string()), Type::Str, 0..1)),
            },
            Type::Str,
            0..1,
        );

        collector.visit_expr(&expr);
        assert_eq!(collector.strings, vec!["hello", "world"]);
    }

    /// Visitor that collects function calls
    struct CallCollector {
        calls: HashSet<String>,
    }

    impl Visitor for CallCollector {
        type Result = ();

        fn default_result(&self) {}

        fn combine_results(&self, _a: (), _b: ()) {}

        fn visit_call(&mut self, func: &FuncRef, args: &[TExpr], _ty: &Type, _span: &Span) {
            match func {
                FuncRef::User(name) | FuncRef::Builtin(name) => {
                    self.calls.insert(name.clone());
                }
                FuncRef::Operator(_) => {}
            }
            // Visit args
            for arg in args {
                self.visit_expr(arg);
            }
        }
    }

    #[test]
    fn test_custom_visitor_collects_calls() {
        let mut collector = CallCollector { calls: HashSet::new() };

        let expr = TExpr::new(
            TExprKind::Call {
                func: FuncRef::User("foo".to_string()),
                args: vec![
                    TExpr::new(
                        TExprKind::Call {
                            func: FuncRef::Builtin("len".to_string()),
                            args: vec![],
                        },
                        Type::Int,
                        0..1,
                    ),
                ],
            },
            Type::Int,
            0..1,
        );

        collector.visit_expr(&expr);
        assert!(collector.calls.contains("foo"));
        assert!(collector.calls.contains("len"));
    }

    #[test]
    fn test_combine_many() {
        let counter = NodeCounter;

        // Empty vec
        let result = counter.combine_many(vec![]);
        assert_eq!(result, 1); // Just default

        // Single element
        let result = counter.combine_many(vec![5]);
        assert_eq!(result, 6); // default(1) + 5

        // Multiple elements
        let result = counter.combine_many(vec![1, 2, 3]);
        assert_eq!(result, 7); // default(1) + 1 + 2 + 3
    }
}
