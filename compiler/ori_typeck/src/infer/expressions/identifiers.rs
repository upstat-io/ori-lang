//! Identifier and function reference type inference.

use crate::checker::TypeChecker;
use ori_ir::{Name, Span};
use ori_types::Type;

/// Infer type for an identifier.
pub fn infer_ident(checker: &mut TypeChecker<'_>, name: Name, span: Span) -> Type {
    if let Some(scheme) = checker.inference.env.lookup_scheme(name) {
        checker.inference.ctx.instantiate(&scheme)
    } else {
        let name_str = checker.context.interner.lookup(name);
        if let Some(ty) = builtin_function_type(checker, name_str) {
            return ty;
        }

        checker.push_error(
            format!(
                "unknown identifier `{}`",
                checker.context.interner.lookup(name)
            ),
            span,
            ori_diagnostic::ErrorCode::E2003,
        );
        Type::Error
    }
}

/// Create a conversion function type: (T) -> `ReturnType`
///
/// Used for built-in conversion functions like `int(x)`, `float(x)`, etc.
#[inline]
fn make_conversion_type(checker: &mut TypeChecker<'_>, ret: Type) -> Type {
    let param = checker.inference.ctx.fresh_var();
    Type::Function {
        params: vec![param],
        ret: Box::new(ret),
    }
}

/// Get the type for a built-in function (`function_val`).
fn builtin_function_type(checker: &mut TypeChecker<'_>, name: &str) -> Option<Type> {
    match name {
        "str" => Some(make_conversion_type(checker, Type::Str)),
        "int" => Some(make_conversion_type(checker, Type::Int)),
        "float" => Some(make_conversion_type(checker, Type::Float)),
        "byte" => Some(make_conversion_type(checker, Type::Byte)),
        _ => None,
    }
}

/// Infer type for a function reference.
pub fn infer_function_ref(checker: &mut TypeChecker<'_>, name: Name, span: Span) -> Type {
    if let Some(scheme) = checker.inference.env.lookup_scheme(name) {
        checker.inference.ctx.instantiate(&scheme)
    } else {
        checker.push_error(
            format!(
                "unknown function `@{}`",
                checker.context.interner.lookup(name)
            ),
            span,
            ori_diagnostic::ErrorCode::E2003,
        );
        Type::Error
    }
}
