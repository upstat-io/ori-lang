//! Type checker using bidirectional type inference.
//!
//! The type checker has two modes:
//! - **Infer**: Determine the type of an expression
//! - **Check**: Verify an expression has an expected type
//!
//! This bidirectional approach enables better error messages and
//! handles cases where type information flows in both directions.

mod unify;
mod context;
mod expr;
mod pattern;

pub use unify::{Unifier, UnifyError};
pub use context::TypeContext;

use crate::intern::{Name, TypeId, TypeInterner, TypeKind, StringInterner};
use crate::syntax::{Span, ExprId, ExprArena};
use crate::errors::{Diagnostic, DiagnosticBag};
use crate::hir::{Scopes, DefinitionRegistry, Resolver, ResolvedName};

/// Result of type checking an expression.
#[derive(Clone, Debug)]
pub struct TypedExpr {
    /// The expression ID.
    pub expr: ExprId,
    /// The inferred type.
    pub ty: TypeId,
}

/// Type error with context.
#[derive(Clone, Debug)]
pub struct TypeError {
    pub kind: TypeErrorKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypeErrorKind {
    /// Type mismatch: expected X, found Y.
    Mismatch { expected: TypeId, found: TypeId },
    /// Cannot unify two types.
    CannotUnify { left: TypeId, right: TypeId },
    /// Unknown identifier.
    UnknownIdent(Name),
    /// Unknown function.
    UnknownFunction(Name),
    /// Unknown type.
    UnknownType(Name),
    /// Wrong number of arguments.
    WrongArgCount { expected: usize, found: usize },
    /// Missing required argument.
    MissingArg(Name),
    /// Unexpected argument.
    UnexpectedArg(Name),
    /// Not callable.
    NotCallable(TypeId),
    /// Not indexable.
    NotIndexable(TypeId),
    /// No such field.
    NoSuchField { ty: TypeId, field: Name },
    /// No such method.
    NoSuchMethod { ty: TypeId, method: Name },
    /// Missing capability.
    MissingCapability(Name),
    /// Cannot assign to immutable.
    CannotAssign,
    /// Break outside loop.
    BreakOutsideLoop,
    /// Continue outside loop.
    ContinueOutsideLoop,
    /// Return outside function.
    ReturnOutsideFunction,
    /// Pattern type mismatch.
    PatternMismatch { pattern: &'static str, expected: TypeId },
    /// Invalid operator for types.
    InvalidOperator { op: &'static str, left: TypeId, right: TypeId },
    /// Division by zero (if detected statically).
    DivisionByZero,
    /// Infinite type (occurs check failure).
    InfiniteType,
}

impl TypeError {
    pub fn mismatch(expected: TypeId, found: TypeId, span: Span) -> Self {
        TypeError {
            kind: TypeErrorKind::Mismatch { expected, found },
            span,
        }
    }

    pub fn unknown_ident(name: Name, span: Span) -> Self {
        TypeError {
            kind: TypeErrorKind::UnknownIdent(name),
            span,
        }
    }

    pub fn wrong_arg_count(expected: usize, found: usize, span: Span) -> Self {
        TypeError {
            kind: TypeErrorKind::WrongArgCount { expected, found },
            span,
        }
    }

    pub fn not_callable(ty: TypeId, span: Span) -> Self {
        TypeError {
            kind: TypeErrorKind::NotCallable(ty),
            span,
        }
    }

    pub fn no_such_field(ty: TypeId, field: Name, span: Span) -> Self {
        TypeError {
            kind: TypeErrorKind::NoSuchField { ty, field },
            span,
        }
    }

    pub fn no_such_method(ty: TypeId, method: Name, span: Span) -> Self {
        TypeError {
            kind: TypeErrorKind::NoSuchMethod { ty, method },
            span,
        }
    }

    pub fn to_diagnostic(&self, interner: &StringInterner, types: &TypeInterner) -> Diagnostic {
        let msg = match &self.kind {
            TypeErrorKind::Mismatch { expected, found } => {
                format!(
                    "type mismatch: expected `{}`, found `{}`",
                    format_type(*expected, types, interner),
                    format_type(*found, types, interner)
                )
            }
            TypeErrorKind::CannotUnify { left, right } => {
                format!(
                    "cannot unify `{}` with `{}`",
                    format_type(*left, types, interner),
                    format_type(*right, types, interner)
                )
            }
            TypeErrorKind::UnknownIdent(name) => {
                format!("cannot find `{}` in this scope", interner.lookup(*name))
            }
            TypeErrorKind::UnknownFunction(name) => {
                format!("cannot find function `@{}`", interner.lookup(*name))
            }
            TypeErrorKind::UnknownType(name) => {
                format!("cannot find type `{}`", interner.lookup(*name))
            }
            TypeErrorKind::WrongArgCount { expected, found } => {
                format!("expected {} arguments, found {}", expected, found)
            }
            TypeErrorKind::MissingArg(name) => {
                format!("missing required argument `{}`", interner.lookup(*name))
            }
            TypeErrorKind::UnexpectedArg(name) => {
                format!("unexpected argument `{}`", interner.lookup(*name))
            }
            TypeErrorKind::NotCallable(ty) => {
                format!("`{}` is not callable", format_type(*ty, types, interner))
            }
            TypeErrorKind::NotIndexable(ty) => {
                format!("`{}` cannot be indexed", format_type(*ty, types, interner))
            }
            TypeErrorKind::NoSuchField { ty, field } => {
                format!(
                    "no field `{}` on type `{}`",
                    interner.lookup(*field),
                    format_type(*ty, types, interner)
                )
            }
            TypeErrorKind::NoSuchMethod { ty, method } => {
                format!(
                    "no method `{}` on type `{}`",
                    interner.lookup(*method),
                    format_type(*ty, types, interner)
                )
            }
            TypeErrorKind::MissingCapability(cap) => {
                format!("missing capability `{}`", interner.lookup(*cap))
            }
            TypeErrorKind::CannotAssign => "cannot assign to immutable binding".to_string(),
            TypeErrorKind::BreakOutsideLoop => "`break` outside of loop".to_string(),
            TypeErrorKind::ContinueOutsideLoop => "`continue` outside of loop".to_string(),
            TypeErrorKind::ReturnOutsideFunction => "`return` outside of function".to_string(),
            TypeErrorKind::PatternMismatch { pattern, expected } => {
                format!(
                    "`{}` pattern expects `{}`, found different type",
                    pattern,
                    format_type(*expected, types, interner)
                )
            }
            TypeErrorKind::InvalidOperator { op, left, right } => {
                format!(
                    "cannot apply `{}` to `{}` and `{}`",
                    op,
                    format_type(*left, types, interner),
                    format_type(*right, types, interner)
                )
            }
            TypeErrorKind::DivisionByZero => "division by zero".to_string(),
            TypeErrorKind::InfiniteType => "infinite type detected".to_string(),
        };

        Diagnostic::error(msg, self.span).with_code("E3001")
    }
}

/// Format a type for display.
fn format_type(ty: TypeId, types: &TypeInterner, interner: &StringInterner) -> String {
    match ty {
        TypeId::INT => "int".to_string(),
        TypeId::FLOAT => "float".to_string(),
        TypeId::BOOL => "bool".to_string(),
        TypeId::STR => "str".to_string(),
        TypeId::CHAR => "char".to_string(),
        TypeId::BYTE => "byte".to_string(),
        TypeId::VOID => "void".to_string(),
        TypeId::NEVER => "Never".to_string(),
        TypeId::INFER => "_".to_string(),
        _ => {
            if let Some(kind) = types.lookup(ty) {
                match kind {
                    TypeKind::List(elem) => format!("[{}]", format_type(elem, types, interner)),
                    TypeKind::Option(inner) => {
                        format!("Option<{}>", format_type(inner, types, interner))
                    }
                    TypeKind::Result { ok, err } => {
                        format!(
                            "Result<{}, {}>",
                            format_type(ok, types, interner),
                            format_type(err, types, interner)
                        )
                    }
                    TypeKind::Named { name, .. } => interner.lookup(name).to_string(),
                    TypeKind::Function { params, ret } => {
                        let param_types = types.get_list(params);
                        let params_str: Vec<_> = param_types
                            .iter()
                            .map(|t| format_type(*t, types, interner))
                            .collect();
                        format!(
                            "({}) -> {}",
                            params_str.join(", "),
                            format_type(ret, types, interner)
                        )
                    }
                    TypeKind::Tuple(elems) => {
                        let elem_types = types.get_list(elems);
                        let elems_str: Vec<_> = elem_types
                            .iter()
                            .map(|t| format_type(*t, types, interner))
                            .collect();
                        format!("({})", elems_str.join(", "))
                    }
                    TypeKind::Map { key, value } => {
                        format!(
                            "{{{}: {}}}",
                            format_type(key, types, interner),
                            format_type(value, types, interner)
                        )
                    }
                    TypeKind::Error => "<error>".to_string(),
                    _ => format!("?{}", ty.raw()),
                }
            } else {
                format!("?{}", ty.raw())
            }
        }
    }
}
