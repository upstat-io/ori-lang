//! Identifier and function reference type inference.

use crate::checker::TypeChecker;
use crate::suggest;
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

    // Check for newtype constructors (e.g., `UserId` for `type UserId = str`)
    if let Some(info) = checker.registries.types.lookup_newtype_constructor(name) {
        return newtype_constructor_type(&info);
    }

    // Try to suggest a similar name
    let name_str = checker.context.interner.lookup(name);
    let message = if let Some(suggestion) = suggest::suggest_identifier(checker, name) {
        format!("unknown identifier `{name_str}`, did you mean `{suggestion}`?")
    } else {
        format!("unknown identifier `{name_str}`")
    };

    checker.push_error(message, span, ori_diagnostic::ErrorCode::E2003);
    Type::Error
}

/// Get the type for a variant constructor.
///
/// - Unit variants (no fields) return the enum type directly
/// - Variants with fields return a function type: `(field_types) -> EnumType`
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

/// Get the type for a newtype constructor.
///
/// Newtypes always take one argument (the underlying value) and return the newtype.
/// Returns a function type: `(underlying_type) -> NewtypeName`
fn newtype_constructor_type(info: &crate::registry::NewtypeConstructorInfo) -> Type {
    let newtype = Type::Named(info.newtype_name);
    Type::Function {
        params: vec![info.underlying_type.clone()],
        ret: Box::new(newtype),
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
        // Try to suggest a similar function name
        let name_str = checker.context.interner.lookup(name);
        let message = if let Some(suggestion) = suggest::suggest_function(checker, name) {
            format!("unknown function `@{name_str}`, did you mean `@{suggestion}`?")
        } else {
            format!("unknown function `@{name_str}`")
        };

        checker.push_error(message, span, ori_diagnostic::ErrorCode::E2003);
        Type::Error
    }
}
