// Expression lowering for AST to TIR
// Converts typed AST expressions to TIR expressions

use crate::ast::{BinaryOp, Expr, SpannedExpr, Span};
use crate::ir::{FuncRef, TExpr, TExprKind, TStmt, Type};
use super::captures::{CaptureAnalyzer, resolve_captures};
use super::types::is_builtin;
use super::Lowerer;

impl Lowerer {
    /// Lower a spanned expression to TIR, preserving the span
    /// This is the preferred entry point for top-level expressions (function bodies, etc.)
    /// Uses the fast path that computes types inline, avoiding redundant type checking.
    pub fn lower_spanned_expr(&mut self, spanned: &SpannedExpr) -> Result<TExpr, String> {
        self.lower_expr_fast(&spanned.expr, spanned.span.clone())
    }

    /// Lower an expression to TIR (uses placeholder span for nested expressions)
    /// Uses the fast path that computes types inline, avoiding redundant type checking.
    pub fn lower_expr(&mut self, expr: &Expr) -> Result<TExpr, String> {
        self.lower_expr_fast(expr, 0..0)
    }

    /// Lower an expression with a known type (uses placeholder span)
    pub(super) fn lower_expr_with_type(&mut self, expr: &Expr, ty: Type) -> Result<TExpr, String> {
        // For nested expressions without spans, use a placeholder
        self.lower_expr_with_span(expr, ty, 0..0)
    }

    /// Lower an expression with a known type and span
    fn lower_expr_with_span(&mut self, expr: &Expr, ty: Type, span: Span) -> Result<TExpr, String> {

        let kind = match expr {
            // Literals
            Expr::Int(n) => TExprKind::Int(*n),
            Expr::Float(f) => TExprKind::Float(*f),
            Expr::String(s) => TExprKind::String(s.clone()),
            Expr::Bool(b) => TExprKind::Bool(*b),
            Expr::Nil => TExprKind::Nil,

            // Identifiers
            Expr::Ident(name) => self.lower_ident(name)?,

            // Config
            Expr::Config(name) => TExprKind::Config(name.clone()),

            // Length placeholder
            Expr::LengthPlaceholder => {
                TExprKind::Call {
                    func: FuncRef::Builtin("__length_placeholder".to_string()),
                    args: vec![],
                }
            }

            // Collections
            Expr::List(exprs) => {
                let elems = exprs
                    .iter()
                    .map(|e| self.lower_expr(e))
                    .collect::<Result<Vec<_>, _>>()?;
                TExprKind::List(elems)
            }

            Expr::MapLiteral(entries) => {
                let tentries = entries
                    .iter()
                    .map(|(k, v)| Ok((self.lower_expr(k)?, self.lower_expr(v)?)))
                    .collect::<Result<Vec<_>, String>>()?;
                TExprKind::MapLiteral(tentries)
            }

            Expr::Tuple(exprs) => {
                let elems = exprs
                    .iter()
                    .map(|e| self.lower_expr(e))
                    .collect::<Result<Vec<_>, _>>()?;
                TExprKind::Tuple(elems)
            }

            Expr::Struct { name, fields } => {
                let tfields = fields
                    .iter()
                    .map(|(n, e)| Ok((n.clone(), self.lower_expr(e)?)))
                    .collect::<Result<Vec<_>, String>>()?;
                TExprKind::Struct {
                    name: name.clone(),
                    fields: tfields,
                }
            }

            // Operations
            Expr::Binary { op, left, right } => {
                let left = self.lower_expr(left)?;
                let right = self.lower_expr(right)?;
                TExprKind::Binary {
                    op: *op,
                    left: Box::new(left),
                    right: Box::new(right),
                }
            }

            Expr::Unary { op, operand } => {
                let operand = self.lower_expr(operand)?;
                TExprKind::Unary {
                    op: *op,
                    operand: Box::new(operand),
                }
            }

            // Access
            Expr::Field(obj, field) => {
                let obj = self.lower_expr(obj)?;
                TExprKind::Field(Box::new(obj), field.clone())
            }

            Expr::Index(obj, idx) => {
                let obj = self.lower_expr(obj)?;
                let idx = self.lower_expr(idx)?;
                TExprKind::Index(Box::new(obj), Box::new(idx))
            }

            // Calls
            Expr::Call { func, args } => self.lower_call(func, args)?,

            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
                let receiver = self.lower_expr(receiver)?;
                let targs = args
                    .iter()
                    .map(|a| self.lower_expr(a))
                    .collect::<Result<Vec<_>, _>>()?;

                TExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    method: method.clone(),
                    args: targs,
                }
            }

            // Lambdas
            Expr::Lambda { params, body } => self.lower_lambda(params, body)?,

            // Control flow
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(condition)?;
                let then_br = self.lower_expr(then_branch)?;
                let else_br = if let Some(eb) = else_branch {
                    self.lower_expr(eb)?
                } else {
                    TExpr::nil(span.clone())
                };

                TExprKind::If {
                    cond: Box::new(cond),
                    then_branch: Box::new(then_br),
                    else_branch: Box::new(else_br),
                }
            }

            Expr::Match(m) => {
                let scrutinee = self.lower_expr(&m.scrutinee)?;
                let scrutinee_ty = scrutinee.ty.clone();

                let arms = m
                    .arms
                    .iter()
                    .map(|arm| {
                        let pattern = self.lower_match_pattern(&arm.pattern, &scrutinee_ty)?;
                        let body = self.lower_expr(&arm.body)?;
                        Ok(crate::ir::TMatchArm { pattern, body })
                    })
                    .collect::<Result<Vec<_>, String>>()?;

                TExprKind::Match(Box::new(crate::ir::TMatch {
                    scrutinee,
                    scrutinee_ty,
                    arms,
                }))
            }

            Expr::Block(exprs) => self.lower_block(exprs, &span)?,

            Expr::For {
                binding,
                iterator,
                body,
            } => {
                let iter = self.lower_expr(iterator)?;

                // Get element type from iterator
                let elem_ty = match &iter.ty {
                    Type::List(inner) => *inner.clone(),
                    Type::Range => Type::Int,
                    _ => Type::Any,
                };

                // Add loop binding to scope (loop bindings are immutable)
                let old_scope = self.local_scope.clone();
                let binding_id = self.locals.add(binding.clone(), elem_ty, false, false);
                self.local_scope.insert(binding.clone(), binding_id);

                let body = self.lower_expr(body)?;

                self.local_scope = old_scope;

                TExprKind::For {
                    binding: binding_id,
                    iter: Box::new(iter),
                    body: Box::new(body),
                }
            }

            // Let binding (outside block)
            Expr::Let { name, mutable, value } => {
                let val = self.lower_expr(value)?;
                let val_ty = val.ty.clone();
                let local_id = self.locals.add(name.clone(), val_ty, false, *mutable);
                self.local_scope.insert(name.clone(), local_id);
                TExprKind::Assign {
                    target: local_id,
                    value: Box::new(val),
                }
            }

            // Reassignment (outside block)
            Expr::Reassign { target, value } => {
                let val = self.lower_expr(value)?;
                if let Some(&local_id) = self.local_scope.get(target) {
                    TExprKind::Assign {
                        target: local_id,
                        value: Box::new(val),
                    }
                } else {
                    return Err(format!("Cannot assign to undeclared variable '{}'", target));
                }
            }

            // Range
            Expr::Range { start, end } => {
                let start = self.lower_expr(start)?;
                let end = self.lower_expr(end)?;
                TExprKind::Range {
                    start: Box::new(start),
                    end: Box::new(end),
                }
            }

            // Patterns
            Expr::Pattern(p) => {
                let pattern = self.lower_pattern(p)?;
                TExprKind::Pattern(Box::new(pattern))
            }

            // Result/Option
            Expr::Ok(inner) => {
                let inner = self.lower_expr(inner)?;
                TExprKind::Ok(Box::new(inner))
            }

            Expr::Err(inner) => {
                let inner = self.lower_expr(inner)?;
                TExprKind::Err(Box::new(inner))
            }

            Expr::Some(inner) => {
                let inner = self.lower_expr(inner)?;
                TExprKind::Some(Box::new(inner))
            }

            Expr::None_ => TExprKind::None_,

            Expr::Coalesce { value, default } => {
                let value = self.lower_expr(value)?;
                let default = self.lower_expr(default)?;
                TExprKind::Coalesce {
                    value: Box::new(value),
                    default: Box::new(default),
                }
            }

            Expr::Unwrap(inner) => {
                let inner = self.lower_expr(inner)?;
                TExprKind::Unwrap(Box::new(inner))
            }
        };

        Ok(TExpr::new(kind, ty, span))
    }

    /// Lower an identifier expression
    fn lower_ident(&self, name: &str) -> Result<TExprKind, String> {
        // Check if it's a parameter
        if let Some(&idx) = self.param_indices.get(name) {
            return Ok(TExprKind::Param(idx));
        }
        // Check if it's a local
        if let Some(&local_id) = self.local_scope.get(name) {
            return Ok(TExprKind::Local(local_id));
        }
        // Check if it's a function (for first-class function references)
        if self.ctx.lookup_function(name).is_some() {
            // For operator functions like +, -, etc.
            return Ok(match name {
                "+" => TExprKind::Call {
                    func: FuncRef::Operator(BinaryOp::Add),
                    args: vec![],
                },
                "-" => TExprKind::Call {
                    func: FuncRef::Operator(BinaryOp::Sub),
                    args: vec![],
                },
                "*" => TExprKind::Call {
                    func: FuncRef::Operator(BinaryOp::Mul),
                    args: vec![],
                },
                "/" => TExprKind::Call {
                    func: FuncRef::Operator(BinaryOp::Div),
                    args: vec![],
                },
                "%" => TExprKind::Call {
                    func: FuncRef::Operator(BinaryOp::Mod),
                    args: vec![],
                },
                _ => TExprKind::Call {
                    func: FuncRef::User(name.to_string()),
                    args: vec![],
                },
            });
        }
        Err(format!("Unknown identifier: {}", name))
    }

    /// Lower a function call expression
    pub(super) fn lower_call(&mut self, func: &Expr, args: &[Expr]) -> Result<TExprKind, String> {
        let targs = args
            .iter()
            .map(|a| self.lower_expr(a))
            .collect::<Result<Vec<_>, _>>()?;

        // Determine the function reference
        let func_ref = match func {
            Expr::Ident(name) => {
                // Check if it's a builtin
                if is_builtin(name) {
                    FuncRef::Builtin(name.clone())
                } else {
                    FuncRef::User(name.clone())
                }
            }
            _ => {
                // Complex function expression - lower it and handle at runtime
                FuncRef::Builtin("__call".to_string())
            }
        };

        Ok(TExprKind::Call {
            func: func_ref,
            args: targs,
        })
    }

    /// Lower a lambda expression
    pub(super) fn lower_lambda(&mut self, params: &[String], body: &Expr) -> Result<TExprKind, String> {
        // Analyze captures BEFORE modifying scope
        // This determines which outer scope variables are used in the lambda body
        let mut analyzer = CaptureAnalyzer::new();
        let free_vars = analyzer.analyze(params, body);
        let captures = resolve_captures(&free_vars, &self.local_scope);

        // Save current scope
        let old_scope = self.local_scope.clone();
        let old_params = self.param_indices.clone();

        // Add lambda parameters to scope
        let mut typed_params = Vec::new();
        for (i, param) in params.iter().enumerate() {
            // For now, infer type from context or default to Any
            let param_ty = Type::Any;
            typed_params.push((param.clone(), param_ty.clone()));
            self.param_indices.insert(param.clone(), i);
        }

        // Lower body
        let body = self.lower_expr(body)?;

        // Restore scope
        self.local_scope = old_scope;
        self.param_indices = old_params;

        Ok(TExprKind::Lambda {
            params: typed_params,
            captures,
            body: Box::new(body),
        })
    }

    /// Lower a block expression
    fn lower_block(
        &mut self,
        exprs: &[Expr],
        span: &std::ops::Range<usize>,
    ) -> Result<TExprKind, String> {
        // Save the current scope
        let old_scope = self.local_scope.clone();

        let mut stmts = Vec::new();
        let mut last_expr = None;

        for (i, e) in exprs.iter().enumerate() {
            if i == exprs.len() - 1 {
                // Last expression is the result
                last_expr = Some(self.lower_expr(e)?);
            } else {
                // Check for let bindings
                if let Expr::Let { name, mutable, value } = e {
                    let val = self.lower_expr(value)?;
                    let val_ty = val.ty.clone();
                    let local_id = self.locals.add(name.clone(), val_ty, false, *mutable);
                    self.local_scope.insert(name.clone(), local_id);
                    stmts.push(TStmt::Let { local: local_id, value: val });
                }
                // Check for reassignment (mutable only)
                else if let Expr::Reassign { target, value } = e {
                    let val = self.lower_expr(value)?;
                    if let Some(&local_id) = self.local_scope.get(target) {
                        stmts.push(TStmt::Expr(TExpr::new(
                            TExprKind::Assign {
                                target: local_id,
                                value: Box::new(val),
                            },
                            Type::Void,
                            span.clone(),
                        )));
                    } else {
                        return Err(format!("Cannot assign to undeclared variable '{}'", target));
                    }
                } else {
                    stmts.push(TStmt::Expr(self.lower_expr(e)?));
                }
            }
        }

        // Restore scope after block
        self.local_scope = old_scope;

        let result = last_expr.unwrap_or_else(|| TExpr::nil(span.clone()));

        Ok(TExprKind::Block(stmts, Box::new(result)))
    }
}
