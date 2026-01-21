// AST to TIR lowering for Sigil
// Combines type checking with IR building
//
// This module converts a type-checked AST to the Typed Intermediate Representation (TIR).
// Every expression in the TIR carries its resolved type.

use super::check::check_expr;
use super::compat::infer_type;
use super::context::TypeContext;
use crate::ast::{self, BinaryOp, Module, TypeExpr};
use crate::ir::{
    FuncRef, IterDirection, LocalId, LocalTable, OnError, TConfig, TExpr, TExprKind, TField,
    TFunction, TImport, TImportItem, TMatch, TMatchArm, TMatchPattern, TModule, TParam, TPattern,
    TStmt, TTest, TTypeDef, TTypeDefKind, TVariant, Type,
};
use std::collections::HashMap;

/// Lowerer converts AST to TIR
pub struct Lowerer {
    ctx: TypeContext,
    locals: LocalTable,
    local_scope: HashMap<String, LocalId>,
    param_indices: HashMap<String, usize>,
}

impl Lowerer {
    pub fn new(ctx: &TypeContext) -> Self {
        Lowerer {
            ctx: ctx.child(),
            locals: LocalTable::new(),
            local_scope: HashMap::new(),
            param_indices: HashMap::new(),
        }
    }

    /// Lower a complete module to TIR
    pub fn lower_module(module: &Module, ctx: &TypeContext) -> Result<TModule, String> {
        let mut tmodule = TModule::new(module.name.clone());

        // First pass: lower type definitions
        for item in &module.items {
            if let ast::Item::TypeDef(td) = item {
                tmodule.types.push(Self::lower_typedef(td, ctx)?);
            }
        }

        // Second pass: lower imports
        for item in &module.items {
            if let ast::Item::Use(u) = item {
                tmodule.imports.push(TImport {
                    path: u.path.clone(),
                    items: u
                        .items
                        .iter()
                        .map(|i| TImportItem {
                            name: i.name.clone(),
                            alias: i.alias.clone(),
                        })
                        .collect(),
                    span: u.span.clone(),
                });
            }
        }

        // Third pass: lower configs
        for item in &module.items {
            if let ast::Item::Config(cd) = item {
                let mut lowerer = Lowerer::new(ctx);
                tmodule.configs.push(lowerer.lower_config(cd)?);
            }
        }

        // Fourth pass: lower functions
        for item in &module.items {
            if let ast::Item::Function(fd) = item {
                let mut lowerer = Lowerer::new(ctx);
                tmodule.functions.push(lowerer.lower_function(fd)?);
            }
        }

        // Fifth pass: lower tests
        for item in &module.items {
            if let ast::Item::Test(td) = item {
                let mut lowerer = Lowerer::new(ctx);
                tmodule.tests.push(lowerer.lower_test(td)?);
            }
        }

        Ok(tmodule)
    }

    /// Lower a type definition
    fn lower_typedef(td: &ast::TypeDef, ctx: &TypeContext) -> Result<TTypeDef, String> {
        let kind = match &td.kind {
            ast::TypeDefKind::Alias(ty) => TTypeDefKind::Alias(type_expr_to_type(ty, ctx)?),
            ast::TypeDefKind::Struct(fields) => {
                let tfields = fields
                    .iter()
                    .map(|f| {
                        Ok(TField {
                            name: f.name.clone(),
                            ty: type_expr_to_type(&f.ty, ctx)?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                TTypeDefKind::Struct(tfields)
            }
            ast::TypeDefKind::Enum(variants) => {
                let tvariants = variants
                    .iter()
                    .map(|v| {
                        let fields = v
                            .fields
                            .iter()
                            .map(|f| {
                                Ok(TField {
                                    name: f.name.clone(),
                                    ty: type_expr_to_type(&f.ty, ctx)?,
                                })
                            })
                            .collect::<Result<Vec<_>, String>>()?;
                        Ok(TVariant {
                            name: v.name.clone(),
                            fields,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                TTypeDefKind::Enum(tvariants)
            }
        };

        Ok(TTypeDef {
            name: td.name.clone(),
            public: td.public,
            params: td.params.clone(),
            kind,
            span: td.span.clone(),
        })
    }

    /// Lower a config definition
    fn lower_config(&mut self, cd: &ast::ConfigDef) -> Result<TConfig, String> {
        let ty = if let Some(ref t) = cd.ty {
            type_expr_to_type(t, &self.ctx)?
        } else {
            let inferred = infer_type(&cd.value)?;
            type_expr_to_type(&inferred, &self.ctx)?
        };

        let value = self.lower_expr(&cd.value)?;

        Ok(TConfig {
            name: cd.name.clone(),
            ty,
            value,
            span: cd.span.clone(),
        })
    }

    /// Lower a function definition
    fn lower_function(&mut self, fd: &ast::FunctionDef) -> Result<TFunction, String> {
        // Setup parameter indices and add parameters to type context
        for (i, param) in fd.params.iter().enumerate() {
            self.param_indices.insert(param.name.clone(), i);
            // Add parameter to context so check_expr can find it
            self.ctx.define_local(param.name.clone(), param.ty.clone());
        }

        // Set return type for self() calls in recurse patterns
        self.ctx.set_current_return_type(fd.return_type.clone());

        // Convert parameters
        let params: Vec<TParam> = fd
            .params
            .iter()
            .map(|p| {
                Ok(TParam {
                    name: p.name.clone(),
                    ty: type_expr_to_type(&p.ty, &self.ctx)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let return_type = type_expr_to_type(&fd.return_type, &self.ctx)?;

        // Lower the body expression
        let body = self.lower_expr(&fd.body)?;

        Ok(TFunction {
            name: fd.name.clone(),
            public: fd.public,
            params,
            return_type,
            locals: std::mem::take(&mut self.locals),
            body,
            span: fd.span.clone(),
        })
    }

    /// Lower a test definition
    fn lower_test(&mut self, td: &ast::TestDef) -> Result<TTest, String> {
        let body = self.lower_expr(&td.body)?;

        Ok(TTest {
            name: td.name.clone(),
            target: td.target.clone(),
            locals: std::mem::take(&mut self.locals),
            body,
            span: td.span.clone(),
        })
    }

    /// Lower an expression to TIR
    pub fn lower_expr(&mut self, expr: &ast::Expr) -> Result<TExpr, String> {
        // First, type-check to get the type
        let ty_expr = check_expr(expr, &self.ctx)?;
        let ty = type_expr_to_type(&ty_expr, &self.ctx)?;

        self.lower_expr_with_type(expr, ty)
    }

    /// Lower an expression with a known type
    fn lower_expr_with_type(&mut self, expr: &ast::Expr, ty: Type) -> Result<TExpr, String> {
        // Get a default span - we'd need actual spans from the AST
        let span = 0..0;

        let kind = match expr {
            // Literals
            ast::Expr::Int(n) => TExprKind::Int(*n),
            ast::Expr::Float(f) => TExprKind::Float(*f),
            ast::Expr::String(s) => TExprKind::String(s.clone()),
            ast::Expr::Bool(b) => TExprKind::Bool(*b),
            ast::Expr::Nil => TExprKind::Nil,

            // Identifiers
            ast::Expr::Ident(name) => {
                // Check if it's a parameter
                if let Some(&idx) = self.param_indices.get(name) {
                    TExprKind::Param(idx)
                }
                // Check if it's a local
                else if let Some(&local_id) = self.local_scope.get(name) {
                    TExprKind::Local(local_id)
                }
                // Check if it's a function (for first-class function references)
                else if self.ctx.lookup_function(name).is_some() {
                    // For operator functions like +, -, etc.
                    match name.as_str() {
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
                            func: FuncRef::User(name.clone()),
                            args: vec![],
                        },
                    }
                } else {
                    return Err(format!("Unknown identifier: {}", name));
                }
            }

            // Config
            ast::Expr::Config(name) => TExprKind::Config(name.clone()),

            // Length placeholder
            ast::Expr::LengthPlaceholder => {
                // This should be resolved in context during lowering
                // For now, we'll represent it as a special call
                TExprKind::Call {
                    func: FuncRef::Builtin("__length_placeholder".to_string()),
                    args: vec![],
                }
            }

            // Collections
            ast::Expr::List(exprs) => {
                let elems = exprs
                    .iter()
                    .map(|e| self.lower_expr(e))
                    .collect::<Result<Vec<_>, _>>()?;
                TExprKind::List(elems)
            }

            ast::Expr::MapLiteral(entries) => {
                let tentries = entries
                    .iter()
                    .map(|(k, v)| Ok((self.lower_expr(k)?, self.lower_expr(v)?)))
                    .collect::<Result<Vec<_>, String>>()?;
                TExprKind::MapLiteral(tentries)
            }

            ast::Expr::Tuple(exprs) => {
                let elems = exprs
                    .iter()
                    .map(|e| self.lower_expr(e))
                    .collect::<Result<Vec<_>, _>>()?;
                TExprKind::Tuple(elems)
            }

            ast::Expr::Struct { name, fields } => {
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
            ast::Expr::Binary { op, left, right } => {
                let left = self.lower_expr(left)?;
                let right = self.lower_expr(right)?;
                TExprKind::Binary {
                    op: *op,
                    left: Box::new(left),
                    right: Box::new(right),
                }
            }

            ast::Expr::Unary { op, operand } => {
                let operand = self.lower_expr(operand)?;
                TExprKind::Unary {
                    op: *op,
                    operand: Box::new(operand),
                }
            }

            // Access
            ast::Expr::Field(obj, field) => {
                let obj = self.lower_expr(obj)?;
                TExprKind::Field(Box::new(obj), field.clone())
            }

            ast::Expr::Index(obj, idx) => {
                let obj = self.lower_expr(obj)?;
                let idx = self.lower_expr(idx)?;
                TExprKind::Index(Box::new(obj), Box::new(idx))
            }

            // Calls
            ast::Expr::Call { func, args } => {
                let targs = args
                    .iter()
                    .map(|a| self.lower_expr(a))
                    .collect::<Result<Vec<_>, _>>()?;

                // Determine the function reference
                let func_ref = match func.as_ref() {
                    ast::Expr::Ident(name) => {
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

                TExprKind::Call {
                    func: func_ref,
                    args: targs,
                }
            }

            ast::Expr::MethodCall {
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
            ast::Expr::Lambda { params, body } => {
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

                // Collect captures (locals from outer scope used in lambda)
                let captures = vec![]; // TODO: Implement capture analysis

                TExprKind::Lambda {
                    params: typed_params,
                    captures,
                    body: Box::new(body),
                }
            }

            // Control flow
            ast::Expr::If {
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

            ast::Expr::Match(m) => {
                let scrutinee = self.lower_expr(&m.scrutinee)?;
                let scrutinee_ty = scrutinee.ty.clone();

                let arms = m
                    .arms
                    .iter()
                    .map(|arm| {
                        let pattern = self.lower_match_pattern(&arm.pattern, &scrutinee_ty)?;
                        let body = self.lower_expr(&arm.body)?;
                        Ok(TMatchArm { pattern, body })
                    })
                    .collect::<Result<Vec<_>, String>>()?;

                TExprKind::Match(Box::new(TMatch {
                    scrutinee,
                    scrutinee_ty,
                    arms,
                }))
            }

            ast::Expr::Block(exprs) => {
                // Save the current scope
                let old_scope = self.local_scope.clone();

                let mut stmts = Vec::new();
                let mut last_expr = None;

                for (i, e) in exprs.iter().enumerate() {
                    if i == exprs.len() - 1 {
                        // Last expression is the result
                        last_expr = Some(self.lower_expr(e)?);
                    } else {
                        // Check if it's an assignment
                        if let ast::Expr::Assign { target, value } = e {
                            let val = self.lower_expr(value)?;
                            let val_ty = val.ty.clone();

                            // Check if variable already exists
                            if let Some(&local_id) = self.local_scope.get(target) {
                                // Reassignment
                                stmts.push(TStmt::Expr(TExpr::new(
                                    TExprKind::Assign {
                                        target: local_id,
                                        value: Box::new(val),
                                    },
                                    Type::Void,
                                    span.clone(),
                                )));
                            } else {
                                // New variable
                                let local_id = self.locals.add(target.clone(), val_ty, false);
                                self.local_scope.insert(target.clone(), local_id);
                                stmts.push(TStmt::Let { local: local_id, value: val });
                            }
                        } else {
                            stmts.push(TStmt::Expr(self.lower_expr(e)?));
                        }
                    }
                }

                // Restore scope after block
                self.local_scope = old_scope;

                let result = last_expr.unwrap_or_else(|| TExpr::nil(span.clone()));

                TExprKind::Block(stmts, Box::new(result))
            }

            ast::Expr::For {
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

                // Add loop binding to scope
                let old_scope = self.local_scope.clone();
                let binding_id = self.locals.add(binding.clone(), elem_ty, false);
                self.local_scope.insert(binding.clone(), binding_id);

                let body = self.lower_expr(body)?;

                self.local_scope = old_scope;

                TExprKind::For {
                    binding: binding_id,
                    iter: Box::new(iter),
                    body: Box::new(body),
                }
            }

            // Assignment (outside block)
            ast::Expr::Assign { target, value } => {
                let val = self.lower_expr(value)?;
                let val_ty = val.ty.clone();

                if let Some(&local_id) = self.local_scope.get(target) {
                    TExprKind::Assign {
                        target: local_id,
                        value: Box::new(val),
                    }
                } else {
                    // New variable
                    let local_id = self.locals.add(target.clone(), val_ty, false);
                    self.local_scope.insert(target.clone(), local_id);
                    TExprKind::Assign {
                        target: local_id,
                        value: Box::new(val),
                    }
                }
            }

            // Range
            ast::Expr::Range { start, end } => {
                let start = self.lower_expr(start)?;
                let end = self.lower_expr(end)?;
                TExprKind::Range {
                    start: Box::new(start),
                    end: Box::new(end),
                }
            }

            // Patterns
            ast::Expr::Pattern(p) => {
                let pattern = self.lower_pattern(p)?;
                TExprKind::Pattern(Box::new(pattern))
            }

            // Result/Option
            ast::Expr::Ok(inner) => {
                let inner = self.lower_expr(inner)?;
                TExprKind::Ok(Box::new(inner))
            }

            ast::Expr::Err(inner) => {
                let inner = self.lower_expr(inner)?;
                TExprKind::Err(Box::new(inner))
            }

            ast::Expr::Some(inner) => {
                let inner = self.lower_expr(inner)?;
                TExprKind::Some(Box::new(inner))
            }

            ast::Expr::None_ => TExprKind::None_,

            ast::Expr::Coalesce { value, default } => {
                let value = self.lower_expr(value)?;
                let default = self.lower_expr(default)?;
                TExprKind::Coalesce {
                    value: Box::new(value),
                    default: Box::new(default),
                }
            }

            ast::Expr::Unwrap(inner) => {
                let inner = self.lower_expr(inner)?;
                TExprKind::Unwrap(Box::new(inner))
            }
        };

        Ok(TExpr::new(kind, ty, span))
    }

    /// Lower a match pattern
    fn lower_match_pattern(
        &mut self,
        pattern: &ast::Pattern,
        scrutinee_ty: &Type,
    ) -> Result<TMatchPattern, String> {
        match pattern {
            ast::Pattern::Wildcard => Ok(TMatchPattern::Wildcard),

            ast::Pattern::Literal(expr) => {
                let texpr = self.lower_expr(expr)?;
                Ok(TMatchPattern::Literal(texpr))
            }

            ast::Pattern::Binding(name) => {
                let local_id = self.locals.add(name.clone(), scrutinee_ty.clone(), false);
                self.local_scope.insert(name.clone(), local_id);
                Ok(TMatchPattern::Binding(local_id, scrutinee_ty.clone()))
            }

            ast::Pattern::Variant { name, fields } => {
                let bindings = fields
                    .iter()
                    .map(|(field_name, sub_pattern)| {
                        // For variant fields, we need to determine the field type
                        // For now, use Any
                        let field_ty = Type::Any;
                        if let ast::Pattern::Binding(binding_name) = sub_pattern {
                            let local_id = self.locals.add(binding_name.clone(), field_ty.clone(), false);
                            self.local_scope.insert(binding_name.clone(), local_id);
                            Ok((field_name.clone(), local_id, field_ty))
                        } else {
                            Err("Nested patterns not yet supported".to_string())
                        }
                    })
                    .collect::<Result<Vec<_>, String>>()?;

                Ok(TMatchPattern::Variant {
                    name: name.clone(),
                    bindings,
                })
            }

            ast::Pattern::Condition(expr) => {
                let texpr = self.lower_expr(expr)?;
                Ok(TMatchPattern::Condition(texpr))
            }
        }
    }

    /// Lower a pattern expression
    fn lower_pattern(&mut self, p: &ast::PatternExpr) -> Result<TPattern, String> {
        match p {
            ast::PatternExpr::Fold {
                collection,
                init,
                op,
            } => {
                let coll = self.lower_expr(collection)?;
                let elem_ty = match &coll.ty {
                    Type::List(inner) => *inner.clone(),
                    _ => Type::Any,
                };
                let init_expr = self.lower_expr(init)?;
                let result_ty = init_expr.ty.clone();
                let op_expr = self.lower_expr(op)?;

                Ok(TPattern::Fold {
                    collection: coll,
                    elem_ty,
                    init: init_expr,
                    op: op_expr,
                    result_ty,
                })
            }

            ast::PatternExpr::Map {
                collection,
                transform,
            } => {
                let coll = self.lower_expr(collection)?;
                let elem_ty = match &coll.ty {
                    Type::List(inner) => *inner.clone(),
                    Type::Range => Type::Int,
                    _ => Type::Any,
                };
                let transform_expr = self.lower_expr(transform)?;

                // Infer result element type from transform
                let result_elem_ty = match &transform_expr.ty {
                    Type::Function { ret, .. } => *ret.clone(),
                    _ => Type::Any,
                };

                Ok(TPattern::Map {
                    collection: coll,
                    elem_ty,
                    transform: transform_expr,
                    result_elem_ty,
                })
            }

            ast::PatternExpr::Filter {
                collection,
                predicate,
            } => {
                let coll = self.lower_expr(collection)?;
                let elem_ty = match &coll.ty {
                    Type::List(inner) => *inner.clone(),
                    _ => Type::Any,
                };
                let pred_expr = self.lower_expr(predicate)?;

                Ok(TPattern::Filter {
                    collection: coll,
                    elem_ty,
                    predicate: pred_expr,
                })
            }

            ast::PatternExpr::Collect { range, transform } => {
                let range_expr = self.lower_expr(range)?;
                let transform_expr = self.lower_expr(transform)?;

                let result_elem_ty = match &transform_expr.ty {
                    Type::Function { ret, .. } => *ret.clone(),
                    _ => Type::Any,
                };

                Ok(TPattern::Collect {
                    range: range_expr,
                    transform: transform_expr,
                    result_elem_ty,
                })
            }

            ast::PatternExpr::Recurse {
                condition,
                base_value,
                step,
                memo,
                parallel_threshold,
            } => {
                let cond = self.lower_expr(condition)?;
                let base = self.lower_expr(base_value)?;
                let result_ty = base.ty.clone();
                let step_expr = self.lower_expr(step)?;

                Ok(TPattern::Recurse {
                    cond,
                    base,
                    step: step_expr,
                    result_ty,
                    memo: *memo,
                    parallel_threshold: *parallel_threshold,
                })
            }

            ast::PatternExpr::Iterate {
                over,
                direction,
                into,
                with,
            } => {
                let over_expr = self.lower_expr(over)?;
                let elem_ty = match &over_expr.ty {
                    Type::List(inner) => *inner.clone(),
                    Type::Range => Type::Int,
                    _ => Type::Any,
                };
                let into_expr = self.lower_expr(into)?;
                let result_ty = into_expr.ty.clone();
                let with_expr = self.lower_expr(with)?;

                let dir = match direction {
                    ast::IterDirection::Forward => IterDirection::Forward,
                    ast::IterDirection::Backward => IterDirection::Backward,
                };

                Ok(TPattern::Iterate {
                    over: over_expr,
                    elem_ty,
                    direction: dir,
                    into: into_expr,
                    with: with_expr,
                    result_ty,
                })
            }

            ast::PatternExpr::Transform { input, steps } => {
                let input_expr = self.lower_expr(input)?;
                let steps_expr = steps
                    .iter()
                    .map(|s| self.lower_expr(s))
                    .collect::<Result<Vec<_>, _>>()?;

                // Compute result type by chaining through steps
                let mut current_ty = input_expr.ty.clone();
                for step in &steps_expr {
                    if let Type::Function { ret, .. } = &step.ty {
                        current_ty = *ret.clone();
                    }
                }

                Ok(TPattern::Transform {
                    input: input_expr,
                    steps: steps_expr,
                    result_ty: current_ty,
                })
            }

            ast::PatternExpr::Count {
                collection,
                predicate,
            } => {
                let coll = self.lower_expr(collection)?;
                let elem_ty = match &coll.ty {
                    Type::List(inner) => *inner.clone(),
                    _ => Type::Any,
                };
                let pred_expr = self.lower_expr(predicate)?;

                Ok(TPattern::Count {
                    collection: coll,
                    elem_ty,
                    predicate: pred_expr,
                })
            }

            ast::PatternExpr::Parallel {
                branches,
                timeout,
                on_error,
            } => {
                let mut tbranches = Vec::new();
                let mut field_types = Vec::new();

                for (name, expr) in branches {
                    let texpr = self.lower_expr(expr)?;
                    let ty = texpr.ty.clone();
                    field_types.push((name.clone(), ty.clone()));
                    tbranches.push((name.clone(), texpr, ty));
                }

                let timeout_expr = timeout
                    .as_ref()
                    .map(|t| self.lower_expr(t))
                    .transpose()?;

                let err = match on_error {
                    ast::OnError::FailFast => OnError::FailFast,
                    ast::OnError::CollectAll => OnError::CollectAll,
                };

                let result_ty = Type::Record(field_types);

                Ok(TPattern::Parallel {
                    branches: tbranches,
                    timeout: timeout_expr,
                    on_error: err,
                    result_ty,
                })
            }
        }
    }
}

/// Convert a TypeExpr to a resolved Type
pub fn type_expr_to_type(ty: &TypeExpr, ctx: &TypeContext) -> Result<Type, String> {
    match ty {
        TypeExpr::Named(name) => match name.as_str() {
            "int" => Ok(Type::Int),
            "float" => Ok(Type::Float),
            "bool" => Ok(Type::Bool),
            "str" => Ok(Type::Str),
            "void" => Ok(Type::Void),
            "any" => Ok(Type::Any),
            "Range" => Ok(Type::Range),
            // Single uppercase letters are type parameters - keep as named
            _ if is_type_param(name) => Ok(Type::Named(name.clone())),
            // User-defined types (or forward references)
            _ => Ok(Type::Named(name.clone())),
        },

        TypeExpr::Generic(name, args) => {
            let targs: Vec<Type> = args
                .iter()
                .map(|a| type_expr_to_type(a, ctx))
                .collect::<Result<Vec<_>, _>>()?;

            match name.as_str() {
                "Result" if targs.len() == 2 => {
                    Ok(Type::Result(Box::new(targs[0].clone()), Box::new(targs[1].clone())))
                }
                "Option" if targs.len() == 1 => Ok(Type::Option(Box::new(targs[0].clone()))),
                "List" if targs.len() == 1 => Ok(Type::List(Box::new(targs[0].clone()))),
                "Map" if targs.len() == 2 => {
                    Ok(Type::Map(Box::new(targs[0].clone()), Box::new(targs[1].clone())))
                }
                _ => Ok(Type::Named(name.clone())), // User-defined generic type
            }
        }

        TypeExpr::Optional(inner) => {
            let inner_ty = type_expr_to_type(inner, ctx)?;
            Ok(Type::Option(Box::new(inner_ty)))
        }

        TypeExpr::List(inner) => {
            let inner_ty = type_expr_to_type(inner, ctx)?;
            Ok(Type::List(Box::new(inner_ty)))
        }

        TypeExpr::Map(key, value) => {
            let key_ty = type_expr_to_type(key, ctx)?;
            let value_ty = type_expr_to_type(value, ctx)?;
            Ok(Type::Map(Box::new(key_ty), Box::new(value_ty)))
        }

        TypeExpr::Tuple(elems) => {
            let elem_types: Vec<Type> = elems
                .iter()
                .map(|e| type_expr_to_type(e, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Type::Tuple(elem_types))
        }

        TypeExpr::Function(param, ret) => {
            let param_ty = type_expr_to_type(param, ctx)?;
            let ret_ty = type_expr_to_type(ret, ctx)?;

            // If param is a tuple, expand it to multiple params
            let params = match param_ty {
                Type::Tuple(types) => types,
                _ => vec![param_ty],
            };

            Ok(Type::Function {
                params,
                ret: Box::new(ret_ty),
            })
        }

        TypeExpr::Record(fields) => {
            let field_types: Vec<(String, Type)> = fields
                .iter()
                .map(|(name, ty)| Ok((name.clone(), type_expr_to_type(ty, ctx)?)))
                .collect::<Result<Vec<_>, String>>()?;
            Ok(Type::Record(field_types))
        }
    }
}

/// Check if a name is a type parameter (single uppercase letter)
fn is_type_param(name: &str) -> bool {
    name.len() == 1
        && name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
}

/// Check if a function name is a builtin
fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "str"
            | "int"
            | "float"
            | "len"
            | "assert"
            | "assert_eq"
            | "assert_err"
            | "+"
            | "-"
            | "*"
            | "/"
            | "%"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_expr_to_type_primitives() {
        let ctx = TypeContext::new();
        assert_eq!(
            type_expr_to_type(&TypeExpr::Named("int".to_string()), &ctx).unwrap(),
            Type::Int
        );
        assert_eq!(
            type_expr_to_type(&TypeExpr::Named("str".to_string()), &ctx).unwrap(),
            Type::Str
        );
        assert_eq!(
            type_expr_to_type(&TypeExpr::Named("bool".to_string()), &ctx).unwrap(),
            Type::Bool
        );
    }

    #[test]
    fn test_type_expr_to_type_list() {
        let ctx = TypeContext::new();
        let list_ty = TypeExpr::List(Box::new(TypeExpr::Named("int".to_string())));
        assert_eq!(
            type_expr_to_type(&list_ty, &ctx).unwrap(),
            Type::List(Box::new(Type::Int))
        );
    }

    #[test]
    fn test_is_builtin() {
        assert!(is_builtin("print"));
        assert!(is_builtin("+"));
        assert!(!is_builtin("foo"));
    }
}
