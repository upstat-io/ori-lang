// Expression folder for Sigil TIR
//
// Provides a trait with default implementations that recurse through the tree.
// Passes only override the methods for nodes they want to transform.
// Adding a new expression type only requires adding a default here.

use super::expr::{FuncRef, LocalId, TExpr, TExprKind, TMatch, TMatchArm, TMatchPattern, TStmt};
use super::patterns::TPattern;
use super::types::Type;
use crate::ast::{BinaryOp, Span, UnaryOp};

/// Trait for transforming TIR expressions.
/// Override only the methods you need - defaults handle recursion.
pub trait Folder {
    /// Main entry point - usually don't override this
    fn fold_expr(&mut self, expr: TExpr) -> TExpr {
        let span = expr.span.clone();
        let ty = expr.ty.clone();
        self.fold_expr_kind(expr.kind, ty, span)
    }

    /// Dispatch based on expression kind - usually don't override this
    fn fold_expr_kind(&mut self, kind: TExprKind, ty: Type, span: Span) -> TExpr {
        match kind {
            TExprKind::Int(n) => self.fold_int(n, ty, span),
            TExprKind::Float(f) => self.fold_float(f, ty, span),
            TExprKind::String(s) => self.fold_string(s, ty, span),
            TExprKind::Bool(b) => self.fold_bool(b, ty, span),
            TExprKind::Nil => self.fold_nil(ty, span),

            TExprKind::Local(id) => self.fold_local(id, ty, span),
            TExprKind::Param(idx) => self.fold_param(idx, ty, span),
            TExprKind::Config(name) => self.fold_config(name, ty, span),

            TExprKind::List(elems) => self.fold_list(elems, ty, span),
            TExprKind::MapLiteral(entries) => self.fold_map_literal(entries, ty, span),
            TExprKind::Tuple(elems) => self.fold_tuple(elems, ty, span),
            TExprKind::Struct { name, fields } => self.fold_struct(name, fields, ty, span),

            TExprKind::Binary { op, left, right } => self.fold_binary(op, *left, *right, ty, span),
            TExprKind::Unary { op, operand } => self.fold_unary(op, *operand, ty, span),

            TExprKind::Field(obj, field) => self.fold_field(*obj, field, ty, span),
            TExprKind::Index(obj, idx) => self.fold_index(*obj, *idx, ty, span),
            TExprKind::LengthOf(obj) => self.fold_length_of(*obj, ty, span),

            TExprKind::Call { func, args } => self.fold_call(func, args, ty, span),
            TExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.fold_method_call(*receiver, method, args, ty, span),

            TExprKind::Lambda {
                params,
                captures,
                body,
            } => self.fold_lambda(params, captures, *body, ty, span),

            TExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.fold_if(*cond, *then_branch, *else_branch, ty, span),
            TExprKind::Match(m) => self.fold_match(*m, ty, span),
            TExprKind::Block(stmts, result) => self.fold_block(stmts, *result, ty, span),
            TExprKind::For { binding, iter, body } => self.fold_for(binding, *iter, *body, ty, span),

            TExprKind::Assign { target, value } => self.fold_assign(target, *value, ty, span),
            TExprKind::Range { start, end } => self.fold_range(*start, *end, ty, span),

            TExprKind::Pattern(p) => self.fold_pattern(*p, ty, span),

            TExprKind::Ok(inner) => self.fold_ok(*inner, ty, span),
            TExprKind::Err(inner) => self.fold_err(*inner, ty, span),
            TExprKind::Some(inner) => self.fold_some(*inner, ty, span),
            TExprKind::None_ => self.fold_none(ty, span),
            TExprKind::Coalesce { value, default } => self.fold_coalesce(*value, *default, ty, span),
            TExprKind::Unwrap(inner) => self.fold_unwrap(*inner, ty, span),
        }
    }

    // === Literals (leaf nodes, no recursion needed) ===

    fn fold_int(&mut self, n: i64, ty: Type, span: Span) -> TExpr {
        TExpr::new(TExprKind::Int(n), ty, span)
    }

    fn fold_float(&mut self, f: f64, ty: Type, span: Span) -> TExpr {
        TExpr::new(TExprKind::Float(f), ty, span)
    }

    fn fold_string(&mut self, s: String, ty: Type, span: Span) -> TExpr {
        TExpr::new(TExprKind::String(s), ty, span)
    }

    fn fold_bool(&mut self, b: bool, ty: Type, span: Span) -> TExpr {
        TExpr::new(TExprKind::Bool(b), ty, span)
    }

    fn fold_nil(&mut self, ty: Type, span: Span) -> TExpr {
        TExpr::new(TExprKind::Nil, ty, span)
    }

    // === Variables (leaf nodes) ===

    fn fold_local(&mut self, id: LocalId, ty: Type, span: Span) -> TExpr {
        TExpr::new(TExprKind::Local(id), ty, span)
    }

    fn fold_param(&mut self, idx: usize, ty: Type, span: Span) -> TExpr {
        TExpr::new(TExprKind::Param(idx), ty, span)
    }

    fn fold_config(&mut self, name: String, ty: Type, span: Span) -> TExpr {
        TExpr::new(TExprKind::Config(name), ty, span)
    }

    // === Collections ===

    fn fold_list(&mut self, elems: Vec<TExpr>, ty: Type, span: Span) -> TExpr {
        let elems = elems.into_iter().map(|e| self.fold_expr(e)).collect();
        TExpr::new(TExprKind::List(elems), ty, span)
    }

    fn fold_map_literal(&mut self, entries: Vec<(TExpr, TExpr)>, ty: Type, span: Span) -> TExpr {
        let entries = entries
            .into_iter()
            .map(|(k, v)| (self.fold_expr(k), self.fold_expr(v)))
            .collect();
        TExpr::new(TExprKind::MapLiteral(entries), ty, span)
    }

    fn fold_tuple(&mut self, elems: Vec<TExpr>, ty: Type, span: Span) -> TExpr {
        let elems = elems.into_iter().map(|e| self.fold_expr(e)).collect();
        TExpr::new(TExprKind::Tuple(elems), ty, span)
    }

    fn fold_struct(
        &mut self,
        name: String,
        fields: Vec<(String, TExpr)>,
        ty: Type,
        span: Span,
    ) -> TExpr {
        let fields = fields
            .into_iter()
            .map(|(n, e)| (n, self.fold_expr(e)))
            .collect();
        TExpr::new(TExprKind::Struct { name, fields }, ty, span)
    }

    // === Operations ===

    fn fold_binary(
        &mut self,
        op: BinaryOp,
        left: TExpr,
        right: TExpr,
        ty: Type,
        span: Span,
    ) -> TExpr {
        let left = self.fold_expr(left);
        let right = self.fold_expr(right);
        TExpr::new(
            TExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            ty,
            span,
        )
    }

    fn fold_unary(&mut self, op: UnaryOp, operand: TExpr, ty: Type, span: Span) -> TExpr {
        let operand = self.fold_expr(operand);
        TExpr::new(
            TExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
            ty,
            span,
        )
    }

    // === Access ===

    fn fold_field(&mut self, obj: TExpr, field: String, ty: Type, span: Span) -> TExpr {
        let obj = self.fold_expr(obj);
        TExpr::new(TExprKind::Field(Box::new(obj), field), ty, span)
    }

    fn fold_index(&mut self, obj: TExpr, idx: TExpr, ty: Type, span: Span) -> TExpr {
        let obj = self.fold_expr(obj);
        let idx = self.fold_expr(idx);
        TExpr::new(TExprKind::Index(Box::new(obj), Box::new(idx)), ty, span)
    }

    fn fold_length_of(&mut self, obj: TExpr, ty: Type, span: Span) -> TExpr {
        let obj = self.fold_expr(obj);
        TExpr::new(TExprKind::LengthOf(Box::new(obj)), ty, span)
    }

    // === Calls ===

    fn fold_call(&mut self, func: FuncRef, args: Vec<TExpr>, ty: Type, span: Span) -> TExpr {
        let args = args.into_iter().map(|a| self.fold_expr(a)).collect();
        TExpr::new(TExprKind::Call { func, args }, ty, span)
    }

    fn fold_method_call(
        &mut self,
        receiver: TExpr,
        method: String,
        args: Vec<TExpr>,
        ty: Type,
        span: Span,
    ) -> TExpr {
        let receiver = self.fold_expr(receiver);
        let args = args.into_iter().map(|a| self.fold_expr(a)).collect();
        TExpr::new(
            TExprKind::MethodCall {
                receiver: Box::new(receiver),
                method,
                args,
            },
            ty,
            span,
        )
    }

    // === Lambda ===

    fn fold_lambda(
        &mut self,
        params: Vec<(String, Type)>,
        captures: Vec<LocalId>,
        body: TExpr,
        ty: Type,
        span: Span,
    ) -> TExpr {
        let body = self.fold_expr(body);
        TExpr::new(
            TExprKind::Lambda {
                params,
                captures,
                body: Box::new(body),
            },
            ty,
            span,
        )
    }

    // === Control flow ===

    fn fold_if(
        &mut self,
        cond: TExpr,
        then_branch: TExpr,
        else_branch: TExpr,
        ty: Type,
        span: Span,
    ) -> TExpr {
        let cond = self.fold_expr(cond);
        let then_branch = self.fold_expr(then_branch);
        let else_branch = self.fold_expr(else_branch);
        TExpr::new(
            TExprKind::If {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            },
            ty,
            span,
        )
    }

    fn fold_match(&mut self, m: TMatch, ty: Type, span: Span) -> TExpr {
        let scrutinee = self.fold_expr(m.scrutinee);
        let arms = m
            .arms
            .into_iter()
            .map(|arm| TMatchArm {
                pattern: self.fold_match_pattern(arm.pattern),
                body: self.fold_expr(arm.body),
            })
            .collect();
        TExpr::new(
            TExprKind::Match(Box::new(TMatch {
                scrutinee,
                scrutinee_ty: m.scrutinee_ty,
                arms,
            })),
            ty,
            span,
        )
    }

    fn fold_match_pattern(&mut self, pattern: TMatchPattern) -> TMatchPattern {
        match pattern {
            TMatchPattern::Literal(expr) => TMatchPattern::Literal(self.fold_expr(expr)),
            TMatchPattern::Condition(expr) => TMatchPattern::Condition(self.fold_expr(expr)),
            other => other,
        }
    }

    fn fold_block(&mut self, stmts: Vec<TStmt>, result: TExpr, ty: Type, span: Span) -> TExpr {
        let stmts = stmts.into_iter().map(|s| self.fold_stmt(s)).collect();
        let result = self.fold_expr(result);
        TExpr::new(TExprKind::Block(stmts, Box::new(result)), ty, span)
    }

    fn fold_stmt(&mut self, stmt: TStmt) -> TStmt {
        match stmt {
            TStmt::Expr(e) => TStmt::Expr(self.fold_expr(e)),
            TStmt::Let { local, value } => TStmt::Let {
                local,
                value: self.fold_expr(value),
            },
        }
    }

    fn fold_for(
        &mut self,
        binding: LocalId,
        iter: TExpr,
        body: TExpr,
        ty: Type,
        span: Span,
    ) -> TExpr {
        let iter = self.fold_expr(iter);
        let body = self.fold_expr(body);
        TExpr::new(
            TExprKind::For {
                binding,
                iter: Box::new(iter),
                body: Box::new(body),
            },
            ty,
            span,
        )
    }

    // === Assignment and Range ===

    fn fold_assign(&mut self, target: LocalId, value: TExpr, ty: Type, span: Span) -> TExpr {
        let value = self.fold_expr(value);
        TExpr::new(
            TExprKind::Assign {
                target,
                value: Box::new(value),
            },
            ty,
            span,
        )
    }

    fn fold_range(&mut self, start: TExpr, end: TExpr, ty: Type, span: Span) -> TExpr {
        let start = self.fold_expr(start);
        let end = self.fold_expr(end);
        TExpr::new(
            TExprKind::Range {
                start: Box::new(start),
                end: Box::new(end),
            },
            ty,
            span,
        )
    }

    // === Patterns ===

    fn fold_pattern(&mut self, pattern: TPattern, ty: Type, span: Span) -> TExpr {
        let pattern = self.fold_tpattern(pattern);
        TExpr::new(TExprKind::Pattern(Box::new(pattern)), ty, span)
    }

    fn fold_tpattern(&mut self, pattern: TPattern) -> TPattern {
        match pattern {
            TPattern::Fold {
                collection,
                elem_ty,
                init,
                op,
                result_ty,
            } => TPattern::Fold {
                collection: self.fold_expr(collection),
                elem_ty,
                init: self.fold_expr(init),
                op: self.fold_expr(op),
                result_ty,
            },
            TPattern::Map {
                collection,
                elem_ty,
                transform,
                result_elem_ty,
            } => TPattern::Map {
                collection: self.fold_expr(collection),
                elem_ty,
                transform: self.fold_expr(transform),
                result_elem_ty,
            },
            TPattern::Filter {
                collection,
                elem_ty,
                predicate,
            } => TPattern::Filter {
                collection: self.fold_expr(collection),
                elem_ty,
                predicate: self.fold_expr(predicate),
            },
            TPattern::Collect {
                range,
                transform,
                result_elem_ty,
            } => TPattern::Collect {
                range: self.fold_expr(range),
                transform: self.fold_expr(transform),
                result_elem_ty,
            },
            TPattern::Recurse {
                cond,
                base,
                step,
                result_ty,
                memo,
                parallel_threshold,
            } => TPattern::Recurse {
                cond: self.fold_expr(cond),
                base: self.fold_expr(base),
                step: self.fold_expr(step),
                result_ty,
                memo,
                parallel_threshold,
            },
            TPattern::Iterate {
                over,
                elem_ty,
                direction,
                into,
                with,
                result_ty,
            } => TPattern::Iterate {
                over: self.fold_expr(over),
                elem_ty,
                direction,
                into: self.fold_expr(into),
                with: self.fold_expr(with),
                result_ty,
            },
            TPattern::Transform {
                input,
                steps,
                result_ty,
            } => TPattern::Transform {
                input: self.fold_expr(input),
                steps: steps.into_iter().map(|s| self.fold_expr(s)).collect(),
                result_ty,
            },
            TPattern::Count {
                collection,
                elem_ty,
                predicate,
            } => TPattern::Count {
                collection: self.fold_expr(collection),
                elem_ty,
                predicate: self.fold_expr(predicate),
            },
            TPattern::Parallel {
                branches,
                timeout,
                on_error,
                result_ty,
            } => TPattern::Parallel {
                branches: branches
                    .into_iter()
                    .map(|(n, e, t)| (n, self.fold_expr(e), t))
                    .collect(),
                timeout: timeout.map(|t| self.fold_expr(t)),
                on_error,
                result_ty,
            },
        }
    }

    // === Result/Option ===

    fn fold_ok(&mut self, inner: TExpr, ty: Type, span: Span) -> TExpr {
        let inner = self.fold_expr(inner);
        TExpr::new(TExprKind::Ok(Box::new(inner)), ty, span)
    }

    fn fold_err(&mut self, inner: TExpr, ty: Type, span: Span) -> TExpr {
        let inner = self.fold_expr(inner);
        TExpr::new(TExprKind::Err(Box::new(inner)), ty, span)
    }

    fn fold_some(&mut self, inner: TExpr, ty: Type, span: Span) -> TExpr {
        let inner = self.fold_expr(inner);
        TExpr::new(TExprKind::Some(Box::new(inner)), ty, span)
    }

    fn fold_none(&mut self, ty: Type, span: Span) -> TExpr {
        TExpr::new(TExprKind::None_, ty, span)
    }

    fn fold_coalesce(&mut self, value: TExpr, default: TExpr, ty: Type, span: Span) -> TExpr {
        let value = self.fold_expr(value);
        let default = self.fold_expr(default);
        TExpr::new(
            TExprKind::Coalesce {
                value: Box::new(value),
                default: Box::new(default),
            },
            ty,
            span,
        )
    }

    fn fold_unwrap(&mut self, inner: TExpr, ty: Type, span: Span) -> TExpr {
        let inner = self.fold_expr(inner);
        TExpr::new(TExprKind::Unwrap(Box::new(inner)), ty, span)
    }
}

/// Identity folder - returns expressions unchanged (useful as a base)
pub struct IdentityFolder;

impl Folder for IdentityFolder {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::expr::FuncRef;

    #[test]
    fn test_identity_folder() {
        let mut folder = IdentityFolder;
        let expr = TExpr::new(TExprKind::Int(42), Type::Int, 0..1);
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Int(42)));
    }

    #[test]
    fn test_fold_literals() {
        let mut folder = IdentityFolder;

        // Float
        let expr = TExpr::new(TExprKind::Float(3.14), Type::Float, 0..1);
        let result = folder.fold_expr(expr);
        if let TExprKind::Float(f) = result.kind {
            assert!((f - 3.14).abs() < 0.001);
        } else {
            panic!("Expected Float");
        }

        // String
        let expr = TExpr::new(TExprKind::String("hello".to_string()), Type::Str, 0..1);
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::String(s) if s == "hello"));

        // Bool
        let expr = TExpr::new(TExprKind::Bool(true), Type::Bool, 0..1);
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Bool(true)));

        // Nil
        let expr = TExpr::new(TExprKind::Nil, Type::Void, 0..1);
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Nil));
    }

    #[test]
    fn test_fold_variables() {
        let mut folder = IdentityFolder;

        // Local
        let expr = TExpr::new(TExprKind::Local(LocalId(0)), Type::Int, 0..1);
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Local(LocalId(0))));

        // Param
        let expr = TExpr::new(TExprKind::Param(1), Type::Int, 0..1);
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Param(1)));

        // Config
        let expr = TExpr::new(TExprKind::Config("cfg".to_string()), Type::Int, 0..1);
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Config(ref s) if s == "cfg"));
    }

    #[test]
    fn test_fold_collections() {
        let mut folder = IdentityFolder;

        // List
        let expr = TExpr::new(
            TExprKind::List(vec![
                TExpr::new(TExprKind::Int(1), Type::Int, 0..1),
                TExpr::new(TExprKind::Int(2), Type::Int, 0..1),
            ]),
            Type::List(Box::new(Type::Int)),
            0..1,
        );
        let result = folder.fold_expr(expr);
        if let TExprKind::List(elems) = result.kind {
            assert_eq!(elems.len(), 2);
        } else {
            panic!("Expected List");
        }

        // Tuple
        let expr = TExpr::new(
            TExprKind::Tuple(vec![
                TExpr::new(TExprKind::Int(1), Type::Int, 0..1),
                TExpr::new(TExprKind::String("x".to_string()), Type::Str, 0..1),
            ]),
            Type::Tuple(vec![Type::Int, Type::Str]),
            0..1,
        );
        let result = folder.fold_expr(expr);
        if let TExprKind::Tuple(elems) = result.kind {
            assert_eq!(elems.len(), 2);
        } else {
            panic!("Expected Tuple");
        }

        // Struct
        let expr = TExpr::new(
            TExprKind::Struct {
                name: "Point".to_string(),
                fields: vec![
                    ("x".to_string(), TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
                    ("y".to_string(), TExpr::new(TExprKind::Int(2), Type::Int, 0..1)),
                ],
            },
            Type::Struct {
                name: "Point".to_string(),
                fields: vec![("x".to_string(), Type::Int), ("y".to_string(), Type::Int)],
            },
            0..1,
        );
        let result = folder.fold_expr(expr);
        if let TExprKind::Struct { name, fields } = result.kind {
            assert_eq!(name, "Point");
            assert_eq!(fields.len(), 2);
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_fold_operations() {
        let mut folder = IdentityFolder;

        // Binary
        let expr = TExpr::new(
            TExprKind::Binary {
                op: BinaryOp::Add,
                left: Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
                right: Box::new(TExpr::new(TExprKind::Int(2), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Binary { op: BinaryOp::Add, .. }));

        // Unary
        let expr = TExpr::new(
            TExprKind::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(TExpr::new(TExprKind::Int(42), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Unary { op: UnaryOp::Neg, .. }));
    }

    #[test]
    fn test_fold_access() {
        let mut folder = IdentityFolder;

        // Field
        let expr = TExpr::new(
            TExprKind::Field(
                Box::new(TExpr::new(TExprKind::Local(LocalId(0)), Type::Int, 0..1)),
                "x".to_string(),
            ),
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Field(_, ref f) if f == "x"));

        // Index
        let expr = TExpr::new(
            TExprKind::Index(
                Box::new(TExpr::new(
                    TExprKind::List(vec![]),
                    Type::List(Box::new(Type::Int)),
                    0..1,
                )),
                Box::new(TExpr::new(TExprKind::Int(0), Type::Int, 0..1)),
            ),
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Index(_, _)));

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
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::LengthOf(_)));
    }

    #[test]
    fn test_fold_calls() {
        let mut folder = IdentityFolder;

        // Call
        let expr = TExpr::new(
            TExprKind::Call {
                func: FuncRef::Builtin("len".to_string()),
                args: vec![TExpr::new(
                    TExprKind::List(vec![]),
                    Type::List(Box::new(Type::Int)),
                    0..1,
                )],
            },
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Call { .. }));

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
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::MethodCall { ref method, .. } if method == "upper"));
    }

    #[test]
    fn test_fold_control_flow() {
        let mut folder = IdentityFolder;

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
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::If { .. }));

        // Block
        let expr = TExpr::new(
            TExprKind::Block(
                vec![TStmt::Expr(TExpr::new(TExprKind::Int(1), Type::Int, 0..1))],
                Box::new(TExpr::new(TExprKind::Int(2), Type::Int, 0..1)),
            ),
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Block(_, _)));

        // For
        let expr = TExpr::new(
            TExprKind::For {
                binding: LocalId(0),
                iter: Box::new(TExpr::new(
                    TExprKind::List(vec![]),
                    Type::List(Box::new(Type::Int)),
                    0..1,
                )),
                body: Box::new(TExpr::new(TExprKind::Int(0), Type::Int, 0..1)),
            },
            Type::Void,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::For { .. }));
    }

    #[test]
    fn test_fold_result_option() {
        let mut folder = IdentityFolder;

        // Ok
        let expr = TExpr::new(
            TExprKind::Ok(Box::new(TExpr::new(TExprKind::Int(42), Type::Int, 0..1))),
            Type::Result(Box::new(Type::Int), Box::new(Type::Str)),
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Ok(_)));

        // Err
        let expr = TExpr::new(
            TExprKind::Err(Box::new(TExpr::new(TExprKind::String("err".to_string()), Type::Str, 0..1))),
            Type::Result(Box::new(Type::Int), Box::new(Type::Str)),
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Err(_)));

        // Some
        let expr = TExpr::new(
            TExprKind::Some(Box::new(TExpr::new(TExprKind::Int(42), Type::Int, 0..1))),
            Type::Option(Box::new(Type::Int)),
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Some(_)));

        // None
        let expr = TExpr::new(TExprKind::None_, Type::Option(Box::new(Type::Int)), 0..1);
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::None_));

        // Coalesce
        let expr = TExpr::new(
            TExprKind::Coalesce {
                value: Box::new(TExpr::new(
                    TExprKind::Some(Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1))),
                    Type::Option(Box::new(Type::Int)),
                    0..1,
                )),
                default: Box::new(TExpr::new(TExprKind::Int(0), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Coalesce { .. }));

        // Unwrap
        let expr = TExpr::new(
            TExprKind::Unwrap(Box::new(TExpr::new(
                TExprKind::Some(Box::new(TExpr::new(TExprKind::Int(42), Type::Int, 0..1))),
                Type::Option(Box::new(Type::Int)),
                0..1,
            ))),
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Unwrap(_)));
    }

    #[test]
    fn test_fold_assign_range() {
        let mut folder = IdentityFolder;

        // Assign
        let expr = TExpr::new(
            TExprKind::Assign {
                target: LocalId(0),
                value: Box::new(TExpr::new(TExprKind::Int(42), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Assign { .. }));

        // Range
        let expr = TExpr::new(
            TExprKind::Range {
                start: Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
                end: Box::new(TExpr::new(TExprKind::Int(10), Type::Int, 0..1)),
            },
            Type::Named("Range".to_string()),
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Range { .. }));
    }

    #[test]
    fn test_fold_lambda() {
        let mut folder = IdentityFolder;

        let expr = TExpr::new(
            TExprKind::Lambda {
                params: vec![("x".to_string(), Type::Int)],
                captures: vec![],
                body: Box::new(TExpr::new(
                    TExprKind::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(TExpr::new(TExprKind::Param(0), Type::Int, 0..1)),
                        right: Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
                    },
                    Type::Int,
                    0..1,
                )),
            },
            Type::Function {
                params: vec![Type::Int],
                ret: Box::new(Type::Int),
            },
            0..1,
        );
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Lambda { .. }));
    }

    #[test]
    fn test_fold_map_literal() {
        let mut folder = IdentityFolder;

        let expr = TExpr::new(
            TExprKind::MapLiteral(vec![
                (
                    TExpr::new(TExprKind::String("key".to_string()), Type::Str, 0..1),
                    TExpr::new(TExprKind::Int(42), Type::Int, 0..1),
                ),
            ]),
            Type::Map(Box::new(Type::Str), Box::new(Type::Int)),
            0..1,
        );
        let result = folder.fold_expr(expr);
        if let TExprKind::MapLiteral(entries) = result.kind {
            assert_eq!(entries.len(), 1);
        } else {
            panic!("Expected MapLiteral");
        }
    }

    #[test]
    fn test_fold_stmt_let() {
        let mut folder = IdentityFolder;

        let stmt = TStmt::Let {
            local: LocalId(0),
            value: TExpr::new(TExprKind::Int(42), Type::Int, 0..1),
        };
        let result = folder.fold_stmt(stmt);
        assert!(matches!(result, TStmt::Let { local: LocalId(0), .. }));
    }

    /// Custom folder that doubles all integer literals
    struct IntDoubler;

    impl Folder for IntDoubler {
        fn fold_int(&mut self, n: i64, ty: Type, span: Span) -> TExpr {
            TExpr::new(TExprKind::Int(n * 2), ty, span)
        }
    }

    #[test]
    fn test_custom_folder() {
        let mut folder = IntDoubler;
        let expr = TExpr::new(TExprKind::Int(21), Type::Int, 0..1);
        let result = folder.fold_expr(expr);
        assert!(matches!(result.kind, TExprKind::Int(42)));
    }

    #[test]
    fn test_custom_folder_recursion() {
        let mut folder = IntDoubler;

        // Binary expression with int literals - both should be doubled
        let expr = TExpr::new(
            TExprKind::Binary {
                op: BinaryOp::Add,
                left: Box::new(TExpr::new(TExprKind::Int(5), Type::Int, 0..1)),
                right: Box::new(TExpr::new(TExprKind::Int(10), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        let result = folder.fold_expr(expr);
        if let TExprKind::Binary { left, right, .. } = result.kind {
            assert!(matches!(left.kind, TExprKind::Int(10)));
            assert!(matches!(right.kind, TExprKind::Int(20)));
        } else {
            panic!("Expected Binary");
        }
    }
}
