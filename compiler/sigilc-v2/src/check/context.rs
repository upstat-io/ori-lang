//! Type checking context that combines all type checking state.

use crate::intern::{Name, TypeId, TypeInterner, TypeKind, StringInterner};
use crate::syntax::{Span, ExprId, ExprArena, ExprKind, BinaryOp, UnaryOp, BindingPattern, StmtKind};
use crate::errors::{Diagnostic, DiagnosticBag};
use crate::hir::{Scopes, DefinitionRegistry, Resolver, ResolvedName, BuiltinKind};
use super::unify::{Unifier, UnifyError};
use super::{TypeError, TypeErrorKind};

/// Type checking context.
pub struct TypeContext<'a> {
    /// String interner.
    pub interner: &'a StringInterner,
    /// Type interner.
    pub types: &'a TypeInterner,
    /// Expression arena.
    pub arena: &'a ExprArena,
    /// Scope manager.
    pub scopes: Scopes,
    /// Local definitions.
    pub local_registry: &'a DefinitionRegistry,
    /// Imported definitions.
    pub imported_registry: &'a DefinitionRegistry,
    /// Type unifier.
    pub unifier: Unifier<'a>,
    /// Collected errors.
    pub errors: DiagnosticBag,
    /// Current capabilities.
    pub capabilities: Vec<Name>,
}

impl<'a> TypeContext<'a> {
    /// Create a new type checking context.
    pub fn new(
        interner: &'a StringInterner,
        types: &'a TypeInterner,
        arena: &'a ExprArena,
        local_registry: &'a DefinitionRegistry,
        imported_registry: &'a DefinitionRegistry,
    ) -> Self {
        TypeContext {
            interner,
            types,
            arena,
            scopes: Scopes::new(),
            local_registry,
            imported_registry,
            unifier: Unifier::new(types),
            errors: DiagnosticBag::new(),
            capabilities: Vec::new(),
        }
    }

    /// Create a resolver for name lookup.
    pub fn resolver(&self) -> Resolver<'_> {
        Resolver::new(
            self.interner,
            &self.scopes,
            self.local_registry,
            self.imported_registry,
        )
    }

    /// Record a type error.
    pub fn error(&mut self, error: TypeError) {
        let diag = error.to_diagnostic(self.interner, self.types);
        self.errors.push(diag);
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        self.errors.has_errors()
    }

    /// Get collected diagnostics.
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.errors.into_vec()
    }

    /// Infer the type of an expression.
    pub fn infer(&mut self, expr_id: ExprId) -> TypeId {
        let expr = self.arena.get(expr_id);
        let span = expr.span;

        match &expr.kind.clone() {
            // Literals
            ExprKind::Int(_) => TypeId::INT,
            ExprKind::Float(_) => TypeId::FLOAT,
            ExprKind::Bool(_) => TypeId::BOOL,
            ExprKind::String(_) => TypeId::STR,
            ExprKind::Char(_) => TypeId::CHAR,
            ExprKind::Duration { .. } => {
                // Duration type - need to intern it
                let name = self.interner.intern("Duration");
                self.types.intern(TypeKind::Named {
                    name,
                    type_args: crate::intern::TypeRange::EMPTY,
                })
            }
            ExprKind::Size { .. } => {
                // Size type
                let name = self.interner.intern("Size");
                self.types.intern(TypeKind::Named {
                    name,
                    type_args: crate::intern::TypeRange::EMPTY,
                })
            }
            ExprKind::Unit => TypeId::VOID,
            ExprKind::None => {
                // None has type Option<_> where _ is inferred
                let var = self.unifier.fresh_var();
                self.types.intern_option(var)
            }

            // References
            ExprKind::Ident(name) => self.infer_ident(*name, span),
            ExprKind::Config(name) => self.infer_config(*name, span),
            ExprKind::FunctionRef(name) => self.infer_function_ref(*name, span),
            ExprKind::SelfRef => self.infer_self(span),
            ExprKind::HashLength => TypeId::INT,

            // Operators
            ExprKind::Binary { op, left, right } => self.infer_binary(*op, *left, *right, span),
            ExprKind::Unary { op, operand } => self.infer_unary(*op, *operand, span),

            // Calls
            ExprKind::Call { func, args } => self.infer_call(*func, *args, span),
            ExprKind::MethodCall { receiver, method, args } => {
                self.infer_method_call(*receiver, *method, *args, span)
            }

            // Access
            ExprKind::Field { receiver, field } => self.infer_field(*receiver, *field, span),
            ExprKind::Index { receiver, index } => self.infer_index(*receiver, *index, span),

            // Control flow
            ExprKind::If { cond, then_branch, else_branch } => {
                self.infer_if(*cond, *then_branch, *else_branch, span)
            }
            ExprKind::For { binding, iter, guard, body, is_yield } => {
                self.infer_for(*binding, *iter, guard.as_ref().copied(), *body, *is_yield, span)
            }
            ExprKind::Loop { body } => self.infer_loop(*body, span),

            // Bindings
            ExprKind::Let { pattern, ty, init, mutable } => {
                self.infer_let(pattern.clone(), *ty, *init, *mutable, span)
            }
            ExprKind::Lambda { params, ret_ty, body } => {
                self.infer_lambda(*params, *ret_ty, *body, span)
            }

            // Collections
            ExprKind::List(range) => self.infer_list(*range, span),
            ExprKind::Map(range) => self.infer_map(*range, span),
            ExprKind::Tuple(range) => self.infer_tuple(*range, span),
            ExprKind::Struct { name, fields } => self.infer_struct(*name, *fields, span),

            // Constructors
            ExprKind::Ok(inner) => self.infer_ok(*inner, span),
            ExprKind::Err(inner) => self.infer_err(*inner, span),
            ExprKind::Some(inner) => self.infer_some(*inner, span),

            // Control
            ExprKind::Return(value) => self.infer_return(*value, span),
            ExprKind::Break(value) => self.infer_break(*value, span),
            ExprKind::Continue => self.infer_continue(span),
            ExprKind::Try(inner) => self.infer_try(*inner, span),
            ExprKind::Await(inner) => self.infer_await(*inner, span),

            // Range
            ExprKind::Range { start, end, inclusive } => {
                self.infer_range(*start, *end, *inclusive, span)
            }

            // Pattern expressions
            ExprKind::Pattern { kind, args } => self.infer_pattern(*kind, *args, span),

            // Match
            ExprKind::Match { scrutinee, arms } => self.infer_match(*scrutinee, *arms, span),

            // Block
            ExprKind::Block { stmts, result } => self.infer_block(*stmts, *result, span),

            // Assignment
            ExprKind::Assign { target, value } => self.infer_assign(*target, *value, span),

            // Error recovery
            ExprKind::Error => self.types.intern(TypeKind::Error),
        }
    }

    /// Check that an expression has an expected type.
    pub fn check(&mut self, expr_id: ExprId, expected: TypeId) -> TypeId {
        let inferred = self.infer(expr_id);
        self.unify_or_error(inferred, expected, self.arena.get(expr_id).span)
    }

    /// Unify two types, recording an error if they don't match.
    pub fn unify_or_error(&mut self, found: TypeId, expected: TypeId, span: Span) -> TypeId {
        match self.unifier.unify(found, expected) {
            Ok(ty) => ty,
            Err(e) => {
                let error = match e {
                    UnifyError::Mismatch { left, right } => TypeError {
                        kind: TypeErrorKind::Mismatch { expected: right, found: left },
                        span,
                    },
                    UnifyError::OccursCheck { .. } => TypeError {
                        kind: TypeErrorKind::InfiniteType,
                        span,
                    },
                    UnifyError::ArityMismatch { expected: e, found: f } => TypeError {
                        kind: TypeErrorKind::WrongArgCount { expected: e, found: f },
                        span,
                    },
                };
                self.error(error);
                self.types.intern(TypeKind::Error)
            }
        }
    }

    // === Inference helpers ===

    fn infer_ident(&mut self, name: Name, span: Span) -> TypeId {
        let resolver = self.resolver();
        match resolver.resolve(name, span) {
            Ok(ResolvedName::Local(binding)) => binding.ty(),
            Ok(ResolvedName::Builtin(builtin)) => self.builtin_type(builtin),
            Ok(ResolvedName::Function(sig)) => {
                // Function used as value - return function type
                let param_types: Vec<_> = sig.params.iter().map(|p| p.ty).collect();
                self.types.intern_function(&param_types, sig.return_type)
            }
            Ok(ResolvedName::Config(config)) => config.ty,
            Ok(ResolvedName::Type(_)) => {
                self.error(TypeError {
                    kind: TypeErrorKind::UnknownIdent(name),
                    span,
                });
                self.types.intern(TypeKind::Error)
            }
            Ok(ResolvedName::Variant { enum_type, .. }) => enum_type,
            Err(_) => {
                self.error(TypeError::unknown_ident(name, span));
                self.types.intern(TypeKind::Error)
            }
        }
    }

    fn infer_config(&mut self, name: Name, span: Span) -> TypeId {
        let resolver = self.resolver();
        match resolver.resolve_config(name, span) {
            Ok(config) => config.ty,
            Err(_) => {
                self.error(TypeError {
                    kind: TypeErrorKind::UnknownIdent(name),
                    span,
                });
                self.types.intern(TypeKind::Error)
            }
        }
    }

    fn infer_function_ref(&mut self, name: Name, span: Span) -> TypeId {
        let resolver = self.resolver();
        match resolver.resolve_function(name, span) {
            Ok(sig) => {
                let param_types: Vec<_> = sig.params.iter().map(|p| p.ty).collect();
                self.types.intern_function(&param_types, sig.return_type)
            }
            Err(_) => {
                self.error(TypeError {
                    kind: TypeErrorKind::UnknownFunction(name),
                    span,
                });
                self.types.intern(TypeKind::Error)
            }
        }
    }

    fn infer_self(&mut self, span: Span) -> TypeId {
        // self in methods - would need method context
        // For now, return error
        self.error(TypeError {
            kind: TypeErrorKind::UnknownIdent(self.interner.intern("self")),
            span,
        });
        self.types.intern(TypeKind::Error)
    }

    fn builtin_type(&self, builtin: BuiltinKind) -> TypeId {
        match builtin {
            BuiltinKind::Print => {
                // print: (any) -> void
                let any = self.types.intern(TypeKind::Infer(0));
                self.types.intern_function(&[any], TypeId::VOID)
            }
            BuiltinKind::Len => {
                // len: ([T]) -> int
                let elem = self.types.intern(TypeKind::Infer(0));
                let list = self.types.intern_list(elem);
                self.types.intern_function(&[list], TypeId::INT)
            }
            BuiltinKind::Str => {
                // str: (any) -> str
                let any = self.types.intern(TypeKind::Infer(0));
                self.types.intern_function(&[any], TypeId::STR)
            }
            BuiltinKind::Int => {
                // int: (str) -> int
                self.types.intern_function(&[TypeId::STR], TypeId::INT)
            }
            BuiltinKind::Float => {
                // float: (str) -> float
                self.types.intern_function(&[TypeId::STR], TypeId::FLOAT)
            }
            BuiltinKind::Compare => {
                // compare: (T, T) -> Ordering
                let t = self.types.intern(TypeKind::Infer(0));
                let ordering = self.types.intern(TypeKind::Named {
                    name: self.interner.intern("Ordering"),
                    type_args: crate::intern::TypeRange::EMPTY,
                });
                self.types.intern_function(&[t, t], ordering)
            }
            BuiltinKind::Panic => {
                // panic: (str) -> Never
                self.types.intern_function(&[TypeId::STR], TypeId::NEVER)
            }
            BuiltinKind::Assert => {
                // assert: (bool) -> void
                self.types.intern_function(&[TypeId::BOOL], TypeId::VOID)
            }
            BuiltinKind::AssertEq => {
                // assert_eq: (T, T) -> void
                let t = self.types.intern(TypeKind::Infer(0));
                self.types.intern_function(&[t, t], TypeId::VOID)
            }
            BuiltinKind::Some => {
                // Some: (T) -> Option<T>
                let t = self.types.intern(TypeKind::Infer(0));
                let opt = self.types.intern_option(t);
                self.types.intern_function(&[t], opt)
            }
            BuiltinKind::None => {
                // None: Option<T>
                let t = self.types.intern(TypeKind::Infer(0));
                self.types.intern_option(t)
            }
            BuiltinKind::Ok => {
                // Ok: (T) -> Result<T, E>
                let t = self.types.intern(TypeKind::Infer(0));
                let e = self.types.intern(TypeKind::Infer(1));
                let result = self.types.intern_result(t, e);
                self.types.intern_function(&[t], result)
            }
            BuiltinKind::Err => {
                // Err: (E) -> Result<T, E>
                let t = self.types.intern(TypeKind::Infer(0));
                let e = self.types.intern(TypeKind::Infer(1));
                let result = self.types.intern_result(t, e);
                self.types.intern_function(&[e], result)
            }
        }
    }

    fn infer_binary(&mut self, op: BinaryOp, left: ExprId, right: ExprId, span: Span) -> TypeId {
        let left_ty = self.infer(left);
        let right_ty = self.infer(right);

        match op {
            // Arithmetic: int/float -> int/float
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div |
            BinaryOp::Mod | BinaryOp::FloorDiv => {
                // Try to unify operands
                match self.unifier.unify(left_ty, right_ty) {
                    Ok(ty) if ty == TypeId::INT || ty == TypeId::FLOAT => ty,
                    Ok(ty) if ty == TypeId::STR && op == BinaryOp::Add => TypeId::STR, // String concat
                    _ => {
                        self.error(TypeError {
                            kind: TypeErrorKind::InvalidOperator {
                                op: op.symbol(),
                                left: left_ty,
                                right: right_ty,
                            },
                            span,
                        });
                        self.types.intern(TypeKind::Error)
                    }
                }
            }

            // Comparison: T, T -> bool
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt |
            BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let _ = self.unify_or_error(left_ty, right_ty, span);
                TypeId::BOOL
            }

            // Logical: bool, bool -> bool
            BinaryOp::And | BinaryOp::Or => {
                self.check(left, TypeId::BOOL);
                self.check(right, TypeId::BOOL);
                TypeId::BOOL
            }

            // Bitwise: int, int -> int
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                self.check(left, TypeId::INT);
                self.check(right, TypeId::INT);
                TypeId::INT
            }

            // Range: T, T -> Range<T>
            BinaryOp::Range | BinaryOp::RangeInc => {
                let elem = self.unify_or_error(left_ty, right_ty, span);
                self.types.intern(TypeKind::Range(elem))
            }

            // Coalesce: Option<T>, T -> T or Result<T, E>, T -> T
            BinaryOp::Coalesce => {
                // For now, simplified version
                if let Some(TypeKind::Option(inner)) = self.types.lookup(left_ty) {
                    self.unify_or_error(inner, right_ty, span)
                } else {
                    right_ty
                }
            }

            BinaryOp::Concat => {
                self.check(left, TypeId::STR);
                self.check(right, TypeId::STR);
                TypeId::STR
            }
        }
    }

    fn infer_unary(&mut self, op: UnaryOp, operand: ExprId, span: Span) -> TypeId {
        let operand_ty = self.infer(operand);

        match op {
            UnaryOp::Not => {
                self.check(operand, TypeId::BOOL);
                TypeId::BOOL
            }
            UnaryOp::Neg => {
                if operand_ty == TypeId::INT || operand_ty == TypeId::FLOAT {
                    operand_ty
                } else {
                    self.error(TypeError {
                        kind: TypeErrorKind::InvalidOperator {
                            op: "-",
                            left: operand_ty,
                            right: operand_ty,
                        },
                        span,
                    });
                    self.types.intern(TypeKind::Error)
                }
            }
            UnaryOp::BitNot => {
                self.check(operand, TypeId::INT);
                TypeId::INT
            }
        }
    }

    fn infer_call(&mut self, func: ExprId, args: crate::syntax::ExprRange, span: Span) -> TypeId {
        let func_ty = self.infer(func);
        let arg_exprs = self.arena.get_expr_list(args);

        if let Some(TypeKind::Function { params, ret }) = self.types.lookup(func_ty) {
            let param_types = self.types.get_list(params);

            if arg_exprs.len() != param_types.len() {
                self.error(TypeError::wrong_arg_count(param_types.len(), arg_exprs.len(), span));
            } else {
                for (arg, param_ty) in arg_exprs.iter().zip(param_types.iter()) {
                    self.check(*arg, *param_ty);
                }
            }
            ret
        } else {
            self.error(TypeError::not_callable(func_ty, span));
            self.types.intern(TypeKind::Error)
        }
    }

    fn infer_method_call(
        &mut self,
        receiver: ExprId,
        method: Name,
        _args: crate::syntax::ExprRange,
        span: Span,
    ) -> TypeId {
        let receiver_ty = self.infer(receiver);

        // Look up method on type - for now simplified
        // Would need to check impls and traits
        self.error(TypeError::no_such_method(receiver_ty, method, span));
        self.types.intern(TypeKind::Error)
    }

    fn infer_field(&mut self, receiver: ExprId, field: Name, span: Span) -> TypeId {
        let receiver_ty = self.infer(receiver);

        // Check for tuple field access (.0, .1, etc.)
        // Check struct fields
        // For now simplified
        self.error(TypeError::no_such_field(receiver_ty, field, span));
        self.types.intern(TypeKind::Error)
    }

    fn infer_index(&mut self, receiver: ExprId, index: ExprId, span: Span) -> TypeId {
        let receiver_ty = self.infer(receiver);
        let index_ty = self.infer(index);

        if let Some(TypeKind::List(elem)) = self.types.lookup(receiver_ty) {
            self.check(index, TypeId::INT);
            elem
        } else if let Some(TypeKind::Map { key, value }) = self.types.lookup(receiver_ty) {
            self.unify_or_error(index_ty, key, span);
            value
        } else {
            self.error(TypeError {
                kind: TypeErrorKind::NotIndexable(receiver_ty),
                span,
            });
            self.types.intern(TypeKind::Error)
        }
    }

    fn infer_if(
        &mut self,
        cond: ExprId,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
        span: Span,
    ) -> TypeId {
        self.check(cond, TypeId::BOOL);
        let then_ty = self.infer(then_branch);

        if let Some(else_expr) = else_branch {
            let else_ty = self.infer(else_expr);
            self.unify_or_error(then_ty, else_ty, span)
        } else {
            TypeId::VOID
        }
    }

    fn infer_for(
        &mut self,
        binding: Name,
        iter: ExprId,
        guard: Option<ExprId>,
        body: ExprId,
        is_yield: bool,
        span: Span,
    ) -> TypeId {
        let iter_ty = self.infer(iter);

        // Get element type from iterable
        let elem_ty = if let Some(TypeKind::List(elem)) = self.types.lookup(iter_ty) {
            elem
        } else if let Some(TypeKind::Range(elem)) = self.types.lookup(iter_ty) {
            elem
        } else {
            self.error(TypeError {
                kind: TypeErrorKind::NotIndexable(iter_ty), // Reusing error type
                span,
            });
            self.types.intern(TypeKind::Error)
        };

        // Enter loop scope
        self.scopes.push_loop();
        self.scopes.define_loop_var(binding, elem_ty);

        // Check guard if present
        if let Some(guard) = guard {
            self.check(guard, TypeId::BOOL);
        }

        let body_ty = self.infer(body);
        self.scopes.pop();

        if is_yield {
            self.types.intern_list(body_ty)
        } else {
            TypeId::VOID
        }
    }

    fn infer_loop(&mut self, body: ExprId, _span: Span) -> TypeId {
        self.scopes.push_loop();
        let _ = self.infer(body);
        self.scopes.pop();

        // Loop returns Never unless broken out of
        TypeId::NEVER
    }

    fn infer_let(
        &mut self,
        pattern: BindingPattern,
        _ty: Option<crate::syntax::TypeExprId>,
        init: ExprId,
        mutable: bool,
        _span: Span,
    ) -> TypeId {
        let init_ty = self.infer(init);

        // If type annotation provided, check against it
        // For now, just use inferred type
        let binding_ty = init_ty;

        // Bind the pattern
        match pattern {
            BindingPattern::Name(name) => {
                self.scopes.define_local(name, binding_ty, mutable);
            }
            BindingPattern::Wildcard => {}
            // TODO: Handle other patterns
            _ => {}
        }

        TypeId::VOID
    }

    fn infer_lambda(
        &mut self,
        params: crate::syntax::ParamRange,
        ret_ty: Option<crate::syntax::TypeExprId>,
        body: ExprId,
        span: Span,
    ) -> TypeId {
        let param_defs = self.arena.get_params(params);

        // Enter new scope for lambda
        let ret = ret_ty.map(|_| self.unifier.fresh_var()).unwrap_or_else(|| self.unifier.fresh_var());
        self.scopes.push_function(ret);

        // Bind parameters
        let mut param_types = Vec::with_capacity(param_defs.len());
        for (i, param) in param_defs.iter().enumerate() {
            let param_ty = param.ty.map(|_| self.unifier.fresh_var()).unwrap_or_else(|| self.unifier.fresh_var());
            param_types.push(param_ty);
            self.scopes.define_param(param.name, param_ty, i);
        }

        // Infer body
        let body_ty = self.infer(body);
        self.unify_or_error(body_ty, ret, span);

        self.scopes.pop();

        self.types.intern_function(&param_types, self.unifier.resolve(ret))
    }

    fn infer_list(&mut self, range: crate::syntax::ExprRange, span: Span) -> TypeId {
        let elems = self.arena.get_expr_list(range);

        if elems.is_empty() {
            let elem = self.unifier.fresh_var();
            return self.types.intern_list(elem);
        }

        let first_ty = self.infer(elems[0]);
        for elem in &elems[1..] {
            let elem_ty = self.infer(*elem);
            self.unify_or_error(first_ty, elem_ty, span);
        }

        self.types.intern_list(first_ty)
    }

    fn infer_map(&mut self, range: crate::syntax::MapEntryRange, span: Span) -> TypeId {
        let entries = self.arena.get_map_entries(range);

        if entries.is_empty() {
            let key = self.unifier.fresh_var();
            let value = self.unifier.fresh_var();
            return self.types.intern_map(key, value);
        }

        let first_key_ty = self.infer(entries[0].key);
        let first_value_ty = self.infer(entries[0].value);

        for entry in &entries[1..] {
            let key_ty = self.infer(entry.key);
            let value_ty = self.infer(entry.value);
            self.unify_or_error(first_key_ty, key_ty, span);
            self.unify_or_error(first_value_ty, value_ty, span);
        }

        self.types.intern_map(first_key_ty, first_value_ty)
    }

    fn infer_tuple(&mut self, range: crate::syntax::ExprRange, _span: Span) -> TypeId {
        let elems = self.arena.get_expr_list(range);
        let elem_types: Vec<_> = elems.iter().map(|e| self.infer(*e)).collect();
        self.types.intern_tuple(&elem_types)
    }

    fn infer_struct(
        &mut self,
        name: Name,
        _fields: crate::syntax::FieldInitRange,
        span: Span,
    ) -> TypeId {
        // Look up struct type
        let resolver = self.resolver();
        match resolver.resolve_type(name, span) {
            Ok(_ty_def) => {
                // TODO: Check field types
                self.types.intern(TypeKind::Named {
                    name,
                    type_args: crate::intern::TypeRange::EMPTY,
                })
            }
            Err(_) => {
                self.error(TypeError {
                    kind: TypeErrorKind::UnknownType(name),
                    span,
                });
                self.types.intern(TypeKind::Error)
            }
        }
    }

    fn infer_ok(&mut self, inner: Option<ExprId>, _span: Span) -> TypeId {
        let ok_ty = inner.map(|e| self.infer(e)).unwrap_or(TypeId::VOID);
        let err_ty = self.unifier.fresh_var();
        self.types.intern_result(ok_ty, err_ty)
    }

    fn infer_err(&mut self, inner: Option<ExprId>, _span: Span) -> TypeId {
        let err_ty = inner.map(|e| self.infer(e)).unwrap_or(TypeId::VOID);
        let ok_ty = self.unifier.fresh_var();
        self.types.intern_result(ok_ty, err_ty)
    }

    fn infer_some(&mut self, inner: ExprId, _span: Span) -> TypeId {
        let inner_ty = self.infer(inner);
        self.types.intern_option(inner_ty)
    }

    fn infer_return(&mut self, value: Option<ExprId>, span: Span) -> TypeId {
        if let Some(expected) = self.scopes.return_type() {
            let actual = value.map(|e| self.infer(e)).unwrap_or(TypeId::VOID);
            self.unify_or_error(actual, expected, span);
        } else {
            self.error(TypeError {
                kind: TypeErrorKind::ReturnOutsideFunction,
                span,
            });
        }
        TypeId::NEVER
    }

    fn infer_break(&mut self, value: Option<ExprId>, span: Span) -> TypeId {
        if !self.scopes.in_loop() {
            self.error(TypeError {
                kind: TypeErrorKind::BreakOutsideLoop,
                span,
            });
        }
        if let Some(e) = value {
            self.infer(e);
        }
        TypeId::NEVER
    }

    fn infer_continue(&mut self, span: Span) -> TypeId {
        if !self.scopes.in_loop() {
            self.error(TypeError {
                kind: TypeErrorKind::ContinueOutsideLoop,
                span,
            });
        }
        TypeId::NEVER
    }

    fn infer_try(&mut self, inner: ExprId, span: Span) -> TypeId {
        let inner_ty = self.infer(inner);

        if let Some(TypeKind::Result { ok, .. }) = self.types.lookup(inner_ty) {
            ok
        } else if let Some(TypeKind::Option(inner)) = self.types.lookup(inner_ty) {
            inner
        } else {
            self.error(TypeError {
                kind: TypeErrorKind::PatternMismatch {
                    pattern: "?",
                    expected: inner_ty,
                },
                span,
            });
            self.types.intern(TypeKind::Error)
        }
    }

    fn infer_await(&mut self, inner: ExprId, _span: Span) -> TypeId {
        // Simplified - would need async type handling
        self.infer(inner)
    }

    fn infer_range(
        &mut self,
        start: Option<ExprId>,
        end: Option<ExprId>,
        _inclusive: bool,
        span: Span,
    ) -> TypeId {
        let elem_ty = match (start, end) {
            (Some(s), Some(e)) => {
                let start_ty = self.infer(s);
                let end_ty = self.infer(e);
                self.unify_or_error(start_ty, end_ty, span)
            }
            (Some(s), None) => self.infer(s),
            (None, Some(e)) => self.infer(e),
            (None, None) => TypeId::INT,
        };
        self.types.intern(TypeKind::Range(elem_ty))
    }

    fn infer_pattern(
        &mut self,
        _kind: crate::syntax::PatternKind,
        _args: crate::syntax::PatternArgsId,
        _span: Span,
    ) -> TypeId {
        // Delegate to pattern module
        // For now, simplified
        let var = self.unifier.fresh_var();
        var
    }

    fn infer_match(
        &mut self,
        scrutinee: ExprId,
        arms: crate::syntax::ArmRange,
        span: Span,
    ) -> TypeId {
        let _scrutinee_ty = self.infer(scrutinee);
        let arm_defs = self.arena.get_arms(arms);

        if arm_defs.is_empty() {
            return TypeId::VOID;
        }

        // All arms should return same type
        let first_ty = self.infer(arm_defs[0].body);
        for arm in &arm_defs[1..] {
            let arm_ty = self.infer(arm.body);
            self.unify_or_error(first_ty, arm_ty, span);
        }

        first_ty
    }

    fn infer_block(
        &mut self,
        stmts: crate::syntax::StmtRange,
        result: Option<ExprId>,
        _span: Span,
    ) -> TypeId {
        self.scopes.push();

        let stmt_defs = self.arena.get_stmts(stmts);
        for stmt in stmt_defs {
            match &stmt.kind {
                StmtKind::Expr(e) => {
                    self.infer(*e);
                }
                StmtKind::Let { pattern, ty, init, mutable } => {
                    self.infer_let(pattern.clone(), *ty, *init, *mutable, stmt.span);
                }
            }
        }

        let result_ty = result.map(|e| self.infer(e)).unwrap_or(TypeId::VOID);
        self.scopes.pop();
        result_ty
    }

    fn infer_assign(&mut self, target: ExprId, value: ExprId, span: Span) -> TypeId {
        let target_ty = self.infer(target);
        let value_ty = self.infer(value);
        self.unify_or_error(value_ty, target_ty, span);
        TypeId::VOID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{Lexer, Parser};

    fn check_expr(code: &str) -> (TypeId, Vec<Diagnostic>) {
        let interner = StringInterner::new();
        let types = TypeInterner::new();

        let lexer = Lexer::new(code, &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, _) = parser.parse_expression();

        let local_reg = DefinitionRegistry::new();
        let imported_reg = DefinitionRegistry::new();

        let mut ctx = TypeContext::new(&interner, &types, &arena, &local_reg, &imported_reg);
        let ty = ctx.infer(expr_id);
        let diags = ctx.into_diagnostics();

        (ty, diags)
    }

    #[test]
    fn test_infer_literals() {
        assert_eq!(check_expr("42").0, TypeId::INT);
        assert_eq!(check_expr("3.14").0, TypeId::FLOAT);
        assert_eq!(check_expr("true").0, TypeId::BOOL);
        assert_eq!(check_expr("false").0, TypeId::BOOL);
    }

    #[test]
    fn test_infer_arithmetic() {
        assert_eq!(check_expr("1 + 2").0, TypeId::INT);
        assert_eq!(check_expr("1.0 + 2.0").0, TypeId::FLOAT);
    }

    #[test]
    fn test_infer_comparison() {
        assert_eq!(check_expr("1 < 2").0, TypeId::BOOL);
        assert_eq!(check_expr("1 == 2").0, TypeId::BOOL);
    }

    #[test]
    fn test_infer_if() {
        let (ty, errs) = check_expr("if true then 1 else 2");
        assert_eq!(ty, TypeId::INT);
        assert!(errs.is_empty());
    }

    #[test]
    fn test_type_error() {
        let (_, errs) = check_expr("1 + true");
        assert!(!errs.is_empty());
    }

    #[test]
    fn test_infer_string() {
        assert_eq!(check_expr("\"hello\"").0, TypeId::STR);
    }

    #[test]
    fn test_infer_string_concat() {
        let (ty, errs) = check_expr("\"hello\" + \"world\"");
        assert_eq!(ty, TypeId::STR);
        assert!(errs.is_empty());
    }

    #[test]
    fn test_infer_unary() {
        assert_eq!(check_expr("-42").0, TypeId::INT);
        assert_eq!(check_expr("-3.14").0, TypeId::FLOAT);
        assert_eq!(check_expr("!true").0, TypeId::BOOL);
    }

    #[test]
    fn test_infer_nested_arithmetic() {
        let (ty, errs) = check_expr("(1 + 2) * (3 - 4)");
        assert_eq!(ty, TypeId::INT);
        assert!(errs.is_empty());
    }

    #[test]
    fn test_infer_logical() {
        assert_eq!(check_expr("true && false").0, TypeId::BOOL);
        assert_eq!(check_expr("true || false").0, TypeId::BOOL);
    }

    #[test]
    fn test_infer_list_literal() {
        let interner = StringInterner::new();
        let types = TypeInterner::new();

        let lexer = Lexer::new("[1, 2, 3]", &interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, &interner);
        let (expr_id, arena, _) = parser.parse_expression();

        let local_reg = DefinitionRegistry::new();
        let imported_reg = DefinitionRegistry::new();

        let mut ctx = TypeContext::new(&interner, &types, &arena, &local_reg, &imported_reg);
        let ty = ctx.infer(expr_id);

        // Should be [int]
        if let Some(TypeKind::List(elem)) = types.lookup(ty) {
            assert_eq!(elem, TypeId::INT);
        } else {
            panic!("Expected list type");
        }
    }

    #[test]
    fn test_type_mismatch_if() {
        // if branches must have same type
        let (_, errs) = check_expr("if true then 1 else \"string\"");
        assert!(!errs.is_empty());
    }

    #[test]
    fn test_comparison_ops() {
        assert_eq!(check_expr("1 <= 2").0, TypeId::BOOL);
        assert_eq!(check_expr("1 >= 2").0, TypeId::BOOL);
        assert_eq!(check_expr("1 != 2").0, TypeId::BOOL);
    }
}
