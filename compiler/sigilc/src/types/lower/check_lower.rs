// Combined type checking and lowering for AST to TIR
//
// This module provides a single-pass transformation from AST to TIR,
// eliminating the redundant type checking that occurred when lowering
// was a separate pass.
//
// The key insight is that type checking and lowering visit the same
// AST structure, so we can produce TIR nodes directly during type checking.

use super::types::{is_builtin, type_expr_to_type};
use super::Lowerer;
use crate::ast::{BinaryOp, Expr, MatchExpr, Span, UnaryOp};
use crate::ir::{FuncRef, TExpr, TExprKind, TMatch, TMatchArm, TStmt, Type};
use crate::types::check_expr;
use crate::types::check_pattern::check_pattern_expr;

impl Lowerer {
    /// Lower an expression to TIR without re-type-checking
    ///
    /// This method computes the type inline during lowering, avoiding
    /// the overhead of calling the full type checker for each subexpression.
    /// It assumes the AST has already been validated by a prior type-check pass.
    pub fn lower_expr_fast(&mut self, expr: &Expr, span: Span) -> Result<TExpr, String> {
        let (kind, ty) = self.lower_and_infer(expr, &span)?;
        Ok(TExpr { kind, ty, span })
    }

    /// Lower expression and compute its type in a single pass
    fn lower_and_infer(&mut self, expr: &Expr, span: &Span) -> Result<(TExprKind, Type), String> {
        match expr {
            // Literals - type is known statically
            Expr::Int(n) => Ok((TExprKind::Int(*n), Type::Int)),
            Expr::Float(f) => Ok((TExprKind::Float(*f), Type::Float)),
            Expr::String(s) => Ok((TExprKind::String(s.clone()), Type::Str)),
            Expr::Bool(b) => Ok((TExprKind::Bool(*b), Type::Bool)),
            Expr::Nil => Ok((TExprKind::Nil, Type::Void)),

            // Identifiers - look up in scope
            Expr::Ident(name) => self.lower_ident_fast(name),

            // Config - look up in context
            Expr::Config(name) => {
                let ty = self
                    .ctx
                    .lookup_config(name)
                    .ok_or_else(|| format!("Unknown config ${}", name))?;
                let ir_ty = type_expr_to_type(ty, &self.ctx)?;
                Ok((TExprKind::Config(name.clone()), ir_ty))
            }

            // Length placeholder
            Expr::LengthPlaceholder => Ok((
                TExprKind::Call {
                    func: FuncRef::Builtin("__length_placeholder".to_string()),
                    args: vec![],
                },
                Type::Int,
            )),

            // Collections
            Expr::List(exprs) => self.lower_list_fast(exprs, span),
            Expr::MapLiteral(entries) => self.lower_map_fast(entries, span),
            Expr::Tuple(exprs) => self.lower_tuple_fast(exprs, span),
            Expr::Struct { name, fields } => self.lower_struct_fast(name, fields, span),

            // Operations
            Expr::Binary { op, left, right } => self.lower_binary_fast(*op, left, right, span),
            Expr::Unary { op, operand } => self.lower_unary_fast(*op, operand, span),

            // Access
            Expr::Field(obj, field) => self.lower_field_fast(obj, field, span),
            Expr::Index(obj, idx) => self.lower_index_fast(obj, idx, span),

            // Calls - these still need type checking for overload resolution
            Expr::Call { func, args } => {
                // Fall back to check_expr for complex call resolution
                let ty_expr = check_expr(expr, &self.ctx).map_err(|d| d.message)?;
                let ty = type_expr_to_type(&ty_expr, &self.ctx)?;
                let kind = self.lower_call(func, args)?;
                Ok((kind, ty))
            }

            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
                let recv = self.lower_expr_fast(receiver, span.clone())?;
                let targs: Vec<TExpr> = args
                    .iter()
                    .map(|a| self.lower_expr_fast(a, span.clone()))
                    .collect::<Result<Vec<_>, _>>()?;

                // Infer result type from method
                let result_ty = self.infer_method_result_type(&recv.ty, method)?;

                Ok((
                    TExprKind::MethodCall {
                        receiver: Box::new(recv),
                        method: method.clone(),
                        args: targs,
                    },
                    result_ty,
                ))
            }

            // Lambdas - need type context for inference
            Expr::Lambda { params, body } => {
                // Fall back to check_expr for lambda type inference
                let ty_expr = check_expr(expr, &self.ctx).map_err(|d| d.message)?;
                let ty = type_expr_to_type(&ty_expr, &self.ctx)?;
                let kind = self.lower_lambda(params, body)?;
                Ok((kind, ty))
            }

            // Control flow
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => self.lower_if_fast(condition, then_branch, else_branch.as_deref(), span),

            Expr::Match(m) => self.lower_match_fast(m, span),
            Expr::Block(exprs) => self.lower_block_fast(exprs, span),

            Expr::For {
                binding,
                iterator,
                body,
            } => self.lower_for_fast(binding, iterator, body, span),

            Expr::Range { start, end } => {
                let s = self.lower_expr_fast(start, span.clone())?;
                let e = self.lower_expr_fast(end, span.clone())?;
                Ok((
                    TExprKind::Range {
                        start: Box::new(s),
                        end: Box::new(e),
                    },
                    Type::Range,
                ))
            }

            // Patterns - need full type checking
            Expr::Pattern(p) => {
                let ty_expr = check_pattern_expr(p, &self.ctx).map_err(|d| d.message)?;
                let ty = type_expr_to_type(&ty_expr, &self.ctx)?;
                let pattern = self.lower_pattern(p)?;
                Ok((TExprKind::Pattern(Box::new(pattern)), ty))
            }

            // Result/Option
            Expr::Ok(inner) => {
                let inner_expr = self.lower_expr_fast(inner, span.clone())?;
                let inner_ty = inner_expr.ty.clone();
                Ok((
                    TExprKind::Ok(Box::new(inner_expr)),
                    Type::Result(Box::new(inner_ty), Box::new(Type::Any)),
                ))
            }

            Expr::Err(inner) => {
                let inner_expr = self.lower_expr_fast(inner, span.clone())?;
                let inner_ty = inner_expr.ty.clone();
                Ok((
                    TExprKind::Err(Box::new(inner_expr)),
                    Type::Result(Box::new(Type::Any), Box::new(inner_ty)),
                ))
            }

            Expr::Some(inner) => {
                let inner_expr = self.lower_expr_fast(inner, span.clone())?;
                let inner_ty = inner_expr.ty.clone();
                Ok((
                    TExprKind::Some(Box::new(inner_expr)),
                    Type::Option(Box::new(inner_ty)),
                ))
            }

            Expr::None_ => Ok((TExprKind::None_, Type::Option(Box::new(Type::Any)))),

            Expr::Coalesce { value, default } => {
                let val = self.lower_expr_fast(value, span.clone())?;
                let def = self.lower_expr_fast(default, span.clone())?;
                let result_ty = def.ty.clone();
                Ok((
                    TExprKind::Coalesce {
                        value: Box::new(val),
                        default: Box::new(def),
                    },
                    result_ty,
                ))
            }

            Expr::Unwrap(inner) => {
                let inner_expr = self.lower_expr_fast(inner, span.clone())?;
                let result_ty = match &inner_expr.ty {
                    Type::Option(inner) => *inner.clone(),
                    Type::Result(ok, _) => *ok.clone(),
                    _ => Type::Any,
                };
                Ok((TExprKind::Unwrap(Box::new(inner_expr)), result_ty))
            }

            // Let/Reassign
            Expr::Let {
                name,
                mutable,
                value,
            } => {
                let val = self.lower_expr_fast(value, span.clone())?;
                let val_ty = val.ty.clone();
                let local_id = self.locals.add(name.clone(), val_ty, false, *mutable);
                self.local_scope.insert(name.clone(), local_id);
                Ok((
                    TExprKind::Assign {
                        target: local_id,
                        value: Box::new(val),
                    },
                    Type::Void,
                ))
            }

            Expr::Reassign { target, value } => {
                let val = self.lower_expr_fast(value, span.clone())?;
                if let Some(&local_id) = self.local_scope.get(target) {
                    Ok((
                        TExprKind::Assign {
                            target: local_id,
                            value: Box::new(val),
                        },
                        Type::Void,
                    ))
                } else {
                    Err(format!("Cannot assign to undeclared variable '{}'", target))
                }
            }

            Expr::With {
                capability,
                implementation,
                body,
            } => {
                // Lower the implementation and body
                let impl_expr = self.lower_expr_fast(implementation, span.clone())?;
                let body_expr = self.lower_expr_fast(body, span.clone())?;
                let body_ty = body_expr.ty.clone();
                Ok((
                    TExprKind::With {
                        capability: capability.clone(),
                        implementation: Box::new(impl_expr),
                        body: Box::new(body_expr),
                    },
                    body_ty,
                ))
            }
        }
    }

    fn lower_ident_fast(&self, name: &str) -> Result<(TExprKind, Type), String> {
        // Check local scope first
        if let Some(&local_id) = self.local_scope.get(name) {
            let ty = self
                .locals
                .get(local_id)
                .map(|info| info.ty.clone())
                .unwrap_or(Type::Any);
            return Ok((TExprKind::Local(local_id), ty));
        }

        // Check parameters using param_indices
        if let Some(&idx) = self.param_indices.get(name) {
            // Parameters are added to context, look up the type
            if let Some(ty_expr) = self.ctx.lookup_local(name) {
                let ty = type_expr_to_type(ty_expr, &self.ctx)?;
                return Ok((TExprKind::Param(idx), ty));
            }
        }

        // Check functions
        if let Some(sig) = self.ctx.lookup_function(name) {
            let ret_ty = type_expr_to_type(&sig.return_type, &self.ctx)?;
            let param_tys: Vec<Type> = sig
                .params
                .iter()
                .map(|(_, ty)| type_expr_to_type(ty, &self.ctx))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok((
                TExprKind::Call {
                    func: if is_builtin(name) {
                        FuncRef::Builtin(name.to_string())
                    } else {
                        FuncRef::User(name.to_string())
                    },
                    args: vec![],
                },
                Type::Function {
                    params: param_tys,
                    ret: Box::new(ret_ty),
                },
            ));
        }

        Err(format!("Unknown identifier '{}'", name))
    }

    fn lower_list_fast(
        &mut self,
        exprs: &[Expr],
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        if exprs.is_empty() {
            // Empty list - use Any as element type, will be resolved by context
            return Ok((TExprKind::List(vec![]), Type::List(Box::new(Type::Any))));
        }

        let elems: Vec<TExpr> = exprs
            .iter()
            .map(|e| self.lower_expr_fast(e, span.clone()))
            .collect::<Result<Vec<_>, _>>()?;

        let elem_ty = elems.first().map(|e| e.ty.clone()).unwrap_or(Type::Any);
        Ok((TExprKind::List(elems), Type::List(Box::new(elem_ty))))
    }

    fn lower_map_fast(
        &mut self,
        entries: &[(Expr, Expr)],
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        let tentries: Vec<(TExpr, TExpr)> = entries
            .iter()
            .map(|(k, v)| {
                Ok((
                    self.lower_expr_fast(k, span.clone())?,
                    self.lower_expr_fast(v, span.clone())?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let (key_ty, val_ty) = if let Some((k, v)) = tentries.first() {
            (k.ty.clone(), v.ty.clone())
        } else {
            (Type::Any, Type::Any)
        };

        Ok((
            TExprKind::MapLiteral(tentries),
            Type::Map(Box::new(key_ty), Box::new(val_ty)),
        ))
    }

    fn lower_tuple_fast(
        &mut self,
        exprs: &[Expr],
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        let elems: Vec<TExpr> = exprs
            .iter()
            .map(|e| self.lower_expr_fast(e, span.clone()))
            .collect::<Result<Vec<_>, _>>()?;

        let elem_tys: Vec<Type> = elems.iter().map(|e| e.ty.clone()).collect();
        Ok((TExprKind::Tuple(elems), Type::Tuple(elem_tys)))
    }

    fn lower_struct_fast(
        &mut self,
        name: &str,
        fields: &[(String, Expr)],
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        let tfields: Vec<(String, TExpr)> = fields
            .iter()
            .map(|(n, e)| Ok((n.clone(), self.lower_expr_fast(e, span.clone())?)))
            .collect::<Result<Vec<_>, String>>()?;

        Ok((
            TExprKind::Struct {
                name: name.to_string(),
                fields: tfields,
            },
            Type::Named(name.to_string()),
        ))
    }

    fn lower_binary_fast(
        &mut self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        let l = self.lower_expr_fast(left, span.clone())?;
        let r = self.lower_expr_fast(right, span.clone())?;

        let result_ty = match op {
            // Comparison operators always return bool
            BinaryOp::Lt
            | BinaryOp::LtEq
            | BinaryOp::Gt
            | BinaryOp::GtEq
            | BinaryOp::Eq
            | BinaryOp::NotEq => Type::Bool,
            // Logical operators return bool
            BinaryOp::And | BinaryOp::Or => Type::Bool,
            // Arithmetic operators return the type of operands
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::IntDiv
            | BinaryOp::Mod => {
                // For string concatenation, result is string
                if matches!(l.ty, Type::Str) {
                    Type::Str
                } else {
                    l.ty.clone()
                }
            }
            // Pipe returns result of right side (fallback to Any for safety)
            BinaryOp::Pipe => r.ty.clone(),
        };

        Ok((
            TExprKind::Binary {
                op,
                left: Box::new(l),
                right: Box::new(r),
            },
            result_ty,
        ))
    }

    fn lower_unary_fast(
        &mut self,
        op: UnaryOp,
        operand: &Expr,
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        let inner = self.lower_expr_fast(operand, span.clone())?;
        let result_ty = match op {
            UnaryOp::Neg => inner.ty.clone(),
            UnaryOp::Not => Type::Bool,
        };

        Ok((
            TExprKind::Unary {
                op,
                operand: Box::new(inner),
            },
            result_ty,
        ))
    }

    fn lower_field_fast(
        &mut self,
        obj: &Expr,
        field: &str,
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        use crate::ast::TypeDefKind;
        let obj_expr = self.lower_expr_fast(obj, span.clone())?;

        // Infer field type from object type
        let field_ty = match &obj_expr.ty {
            Type::Named(name) => {
                if let Some(td) = self.ctx.lookup_type(name) {
                    // Extract fields from TypeDefKind::Struct
                    if let TypeDefKind::Struct(fields) = &td.kind {
                        fields
                            .iter()
                            .find(|f| f.name == field)
                            .map(|f| type_expr_to_type(&f.ty, &self.ctx))
                            .transpose()?
                            .unwrap_or(Type::Any)
                    } else {
                        Type::Any
                    }
                } else {
                    Type::Any
                }
            }
            Type::Record(fields) => fields
                .iter()
                .find(|(n, _)| n == field)
                .map(|(_, ty)| ty.clone())
                .unwrap_or(Type::Any),
            Type::Tuple(types) => {
                // Handle tuple field access like .0, .1
                if let Ok(idx) = field.parse::<usize>() {
                    types.get(idx).cloned().unwrap_or(Type::Any)
                } else {
                    Type::Any
                }
            }
            _ => Type::Any,
        };

        Ok((
            TExprKind::Field(Box::new(obj_expr), field.to_string()),
            field_ty,
        ))
    }

    fn lower_index_fast(
        &mut self,
        obj: &Expr,
        idx: &Expr,
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        let obj_expr = self.lower_expr_fast(obj, span.clone())?;
        let idx_expr = self.lower_expr_fast(idx, span.clone())?;

        let elem_ty = match &obj_expr.ty {
            Type::List(inner) => *inner.clone(),
            Type::Str => Type::Str,
            Type::Map(_, v) => *v.clone(),
            Type::Tuple(types) => {
                // For constant index, get the specific type
                if let TExprKind::Int(i) = &idx_expr.kind {
                    types.get(*i as usize).cloned().unwrap_or(Type::Any)
                } else {
                    Type::Any
                }
            }
            _ => Type::Any,
        };

        Ok((
            TExprKind::Index(Box::new(obj_expr), Box::new(idx_expr)),
            elem_ty,
        ))
    }

    fn lower_if_fast(
        &mut self,
        cond: &Expr,
        then_br: &Expr,
        else_br: Option<&Expr>,
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        let c = self.lower_expr_fast(cond, span.clone())?;
        let t = self.lower_expr_fast(then_br, span.clone())?;
        let e = if let Some(eb) = else_br {
            self.lower_expr_fast(eb, span.clone())?
        } else {
            TExpr::nil(span.clone())
        };

        let result_ty = t.ty.clone();

        Ok((
            TExprKind::If {
                cond: Box::new(c),
                then_branch: Box::new(t),
                else_branch: Box::new(e),
            },
            result_ty,
        ))
    }

    fn lower_match_fast(
        &mut self,
        m: &MatchExpr,
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        let scrutinee = self.lower_expr_fast(&m.scrutinee, span.clone())?;
        let scrutinee_ty = scrutinee.ty.clone();

        let mut result_ty = Type::Any;
        let arms: Vec<TMatchArm> = m
            .arms
            .iter()
            .map(|arm| {
                let pattern = self.lower_match_pattern(&arm.pattern, &scrutinee_ty)?;
                let body = self.lower_expr_fast(&arm.body, span.clone())?;
                if result_ty == Type::Any {
                    result_ty = body.ty.clone();
                }
                Ok(TMatchArm { pattern, body })
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok((
            TExprKind::Match(Box::new(TMatch {
                scrutinee,
                scrutinee_ty,
                arms,
            })),
            result_ty,
        ))
    }

    fn lower_block_fast(
        &mut self,
        exprs: &[Expr],
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        if exprs.is_empty() {
            return Ok((
                TExprKind::Block(vec![], Box::new(TExpr::nil(span.clone()))),
                Type::Void,
            ));
        }

        let old_scope = self.local_scope.clone();
        let mut stmts = Vec::new();

        for (i, expr) in exprs.iter().enumerate() {
            let is_last = i == exprs.len() - 1;

            if is_last {
                let result = self.lower_expr_fast(expr, span.clone())?;
                let result_ty = result.ty.clone();
                self.local_scope = old_scope;
                return Ok((TExprKind::Block(stmts, Box::new(result)), result_ty));
            }

            match expr {
                Expr::Let {
                    name,
                    mutable,
                    value,
                } => {
                    let val = self.lower_expr_fast(value, span.clone())?;
                    let val_ty = val.ty.clone();
                    let local_id = self.locals.add(name.clone(), val_ty, false, *mutable);
                    self.local_scope.insert(name.clone(), local_id);
                    stmts.push(TStmt::Let {
                        local: local_id,
                        value: val,
                    });
                }
                Expr::Reassign { target, value } => {
                    let val = self.lower_expr_fast(value, span.clone())?;
                    if let Some(&local_id) = self.local_scope.get(target) {
                        stmts.push(TStmt::Expr(TExpr {
                            kind: TExprKind::Assign {
                                target: local_id,
                                value: Box::new(val),
                            },
                            ty: Type::Void,
                            span: span.clone(),
                        }));
                    }
                }
                _ => {
                    let e = self.lower_expr_fast(expr, span.clone())?;
                    stmts.push(TStmt::Expr(e));
                }
            }
        }

        self.local_scope = old_scope;
        Ok((
            TExprKind::Block(stmts, Box::new(TExpr::nil(span.clone()))),
            Type::Void,
        ))
    }

    fn lower_for_fast(
        &mut self,
        binding: &str,
        iterator: &Expr,
        body: &Expr,
        span: &Span,
    ) -> Result<(TExprKind, Type), String> {
        let iter = self.lower_expr_fast(iterator, span.clone())?;

        let elem_ty = match &iter.ty {
            Type::List(inner) => *inner.clone(),
            Type::Range => Type::Int,
            _ => Type::Any,
        };

        let old_scope = self.local_scope.clone();
        let binding_id = self.locals.add(binding.to_string(), elem_ty, false, false);
        self.local_scope.insert(binding.to_string(), binding_id);

        let body_expr = self.lower_expr_fast(body, span.clone())?;

        self.local_scope = old_scope;

        Ok((
            TExprKind::For {
                binding: binding_id,
                iter: Box::new(iter),
                body: Box::new(body_expr),
            },
            Type::Void,
        ))
    }

    /// Infer the result type of a method call
    fn infer_method_result_type(&self, receiver_ty: &Type, method: &str) -> Result<Type, String> {
        match (receiver_ty, method) {
            // String methods
            (Type::Str, "len") => Ok(Type::Int),
            (Type::Str, "split") => Ok(Type::List(Box::new(Type::Str))),
            (Type::Str, "trim" | "upper" | "lower") => Ok(Type::Str),
            (Type::Str, "contains" | "starts_with" | "ends_with") => Ok(Type::Bool),
            (Type::Str, "chars") => Ok(Type::List(Box::new(Type::Str))),

            // List methods
            (Type::List(_), "len") => Ok(Type::Int),
            (Type::List(inner), "first" | "last") => Ok(Type::Option(inner.clone())),
            (Type::List(inner), "push" | "reverse" | "sort") => Ok(Type::List(inner.clone())),
            (Type::List(inner), "pop") => Ok(*inner.clone()),
            (Type::List(_), "contains") => Ok(Type::Bool),
            (Type::List(_), "map") => Ok(Type::List(Box::new(Type::Any))),
            (Type::List(inner), "filter") => Ok(Type::List(inner.clone())),
            (Type::List(_), "join") => Ok(Type::Str),

            // Map methods
            (Type::Map(_, _), "len") => Ok(Type::Int),
            (Type::Map(_, v), "get") => Ok(Type::Option(v.clone())),
            (Type::Map(_, _), "contains_key") => Ok(Type::Bool),
            (Type::Map(k, _), "keys") => Ok(Type::List(k.clone())),
            (Type::Map(_, v), "values") => Ok(Type::List(v.clone())),

            // Option methods
            (Type::Option(inner), "unwrap") => Ok(*inner.clone()),
            (Type::Option(_), "is_some" | "is_none") => Ok(Type::Bool),
            (Type::Option(_), "map") => Ok(Type::Option(Box::new(Type::Any))),

            // Result methods
            (Type::Result(ok, _), "unwrap") => Ok(*ok.clone()),
            (Type::Result(_, _), "is_ok" | "is_err") => Ok(Type::Bool),

            _ => Ok(Type::Any),
        }
    }
}
