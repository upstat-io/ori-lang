//! Identifier and function reference type inference.

use crate::checker::TypeChecker;
use ori_ir::{Name, Span};
use ori_types::Type;

/// Infer type for an identifier.
pub fn infer_ident(checker: &mut TypeChecker<'_>, name: Name, span: Span) -> Type {
    // First check local bindings
    if let Some(scheme) = checker.inference.env.lookup_scheme(name) {
        return checker.inference.ctx.instantiate(&scheme);
    }

    // Check built-in functions (int, float, str, byte)
    let name_str = checker.context.interner.lookup(name);
    if let Some(ty) = builtin_function_type(checker, name_str) {
        return ty;
    }

    // Check for variant constructors (e.g., `Running` for `type Status = Running | Done`)
    if let Some(info) = checker.registries.types.lookup_variant_constructor(name) {
        return variant_constructor_type(checker, &info);
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

/// Get the type for a variant constructor.
///
/// - Unit variants (no fields) return the enum type directly
/// - Variants with fields return a function type: (field_types) -> EnumType
fn variant_constructor_type(
    _checker: &mut TypeChecker<'_>,
    info: &crate::registry::VariantConstructorInfo,
) -> Type {
    let enum_type = Type::Named(info.enum_name);

    if info.field_types.is_empty() {
        // Unit variant: returns the enum type directly
        enum_type
    } else {
        // Variant with fields: returns a function type
        Type::Function {
            params: info.field_types.clone(),
            ret: Box::new(enum_type),
        }
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
