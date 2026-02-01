//! Duration and Size method handler.

use ori_diagnostic::ErrorCode;
use ori_ir::{Span, StringInterner};
use ori_types::{InferenceContext, Type};

use super::{BuiltinMethodHandler, MethodTypeError, MethodTypeResult};

/// Type checking for Duration and Size methods.
pub struct UnitsMethodHandler;

impl UnitsMethodHandler {
    /// Check associated function calls (static methods without self parameter).
    ///
    /// These are factory methods like `Duration.from_seconds(s: 10)`.
    pub fn check_associated(
        &self,
        ctx: &mut InferenceContext,
        type_name: &str,
        method: &str,
        args: &[Type],
    ) -> Option<MethodTypeResult> {
        match type_name {
            "Duration" => check_duration_associated(ctx, method, args),
            "Size" => check_size_associated(ctx, method, args),
            _ => None,
        }
    }
}

impl BuiltinMethodHandler for UnitsMethodHandler {
    fn handles(&self, receiver_ty: &Type) -> bool {
        matches!(receiver_ty, Type::Duration | Type::Size)
    }

    fn check(
        &self,
        _ctx: &mut InferenceContext,
        interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        _args: &[Type],
        _span: Span,
    ) -> MethodTypeResult {
        match receiver_ty {
            Type::Duration => check_duration_method(method, interner),
            Type::Size => check_size_method(method, interner),
            _ => unreachable!("handles() verified type is Duration or Size"),
        }
    }
}

/// Duration extraction methods (returning int).
const DURATION_INT_METHODS: &[&str] = &[
    "nanoseconds",
    "microseconds",
    "milliseconds",
    "seconds",
    "minutes",
    "hours",
    "hash", // Hashable trait
];

/// Size extraction methods (returning int).
const SIZE_INT_METHODS: &[&str] = &[
    "bytes",
    "kilobytes",
    "megabytes",
    "gigabytes",
    "terabytes",
    "hash", // Hashable trait
];

fn check_duration_method(method: &str, interner: &StringInterner) -> MethodTypeResult {
    // Check int-returning methods
    if DURATION_INT_METHODS.contains(&method) {
        return MethodTypeResult::Ok(Type::Int);
    }
    // Clone returns self type
    if method == "clone" {
        return MethodTypeResult::Ok(Type::Duration);
    }
    // to_str returns str (Printable trait)
    if method == "to_str" {
        return MethodTypeResult::Ok(Type::Str);
    }
    // equals returns bool (Eq trait)
    if method == "equals" {
        return MethodTypeResult::Ok(Type::Bool);
    }
    // compare returns Ordering (Comparable trait)
    if method == "compare" {
        let ordering = interner.intern("Ordering");
        return MethodTypeResult::Ok(Type::Named(ordering));
    }
    // Operator method aliases
    if method == "negate" || method == "neg" {
        return MethodTypeResult::Ok(Type::Duration);
    }
    MethodTypeResult::Err(MethodTypeError::new(
        format!("unknown method `{method}` for type `Duration`"),
        ErrorCode::E2002,
    ))
}

fn check_size_method(method: &str, interner: &StringInterner) -> MethodTypeResult {
    // Check int-returning methods
    if SIZE_INT_METHODS.contains(&method) {
        return MethodTypeResult::Ok(Type::Int);
    }
    // Clone returns self type
    if method == "clone" {
        return MethodTypeResult::Ok(Type::Size);
    }
    // to_str returns str (Printable trait)
    if method == "to_str" {
        return MethodTypeResult::Ok(Type::Str);
    }
    // equals returns bool (Eq trait)
    if method == "equals" {
        return MethodTypeResult::Ok(Type::Bool);
    }
    // compare returns Ordering (Comparable trait)
    if method == "compare" {
        let ordering = interner.intern("Ordering");
        return MethodTypeResult::Ok(Type::Named(ordering));
    }
    // Operator method alias
    if method == "remainder" || method == "rem" {
        return MethodTypeResult::Ok(Type::Size);
    }
    MethodTypeResult::Err(MethodTypeError::new(
        format!("unknown method `{method}` for type `Size`"),
        ErrorCode::E2002,
    ))
}

/// Check unit factory methods (associated functions).
///
/// Both Duration and Size have factory methods that create instances from
/// a specific unit (e.g., `Duration.from_seconds()`, `Size.from_bytes()`).
/// This helper factors out the common validation pattern.
fn check_unit_associated(
    ctx: &mut InferenceContext,
    method: &str,
    args: &[Type],
    type_name: &str,
    valid_methods: &[&str],
    result_type: Type,
) -> Option<MethodTypeResult> {
    if !valid_methods.contains(&method) {
        return None;
    }

    if args.len() != 1 {
        return Some(MethodTypeResult::Err(MethodTypeError::new(
            format!(
                "{type_name}.{method} expects 1 argument, found {}",
                args.len()
            ),
            ErrorCode::E2004,
        )));
    }
    if ctx.unify(&args[0], &Type::Int).is_err() {
        return Some(MethodTypeResult::Err(MethodTypeError::new(
            format!("{type_name}.{method} expects int argument"),
            ErrorCode::E2001,
        )));
    }
    Some(MethodTypeResult::Ok(result_type))
}

/// Duration factory methods.
const DURATION_FACTORIES: &[&str] = &[
    "from_nanoseconds",
    "from_microseconds",
    "from_milliseconds",
    "from_seconds",
    "from_minutes",
    "from_hours",
];

/// Size factory methods.
const SIZE_FACTORIES: &[&str] = &[
    "from_bytes",
    "from_kilobytes",
    "from_megabytes",
    "from_gigabytes",
    "from_terabytes",
];

/// Check Duration associated functions (factory methods and default).
fn check_duration_associated(
    ctx: &mut InferenceContext,
    method: &str,
    args: &[Type],
) -> Option<MethodTypeResult> {
    // Default trait associated function
    if method == "default" {
        if !args.is_empty() {
            return Some(MethodTypeResult::Err(MethodTypeError::new(
                format!("Duration.default expects 0 arguments, found {}", args.len()),
                ErrorCode::E2004,
            )));
        }
        return Some(MethodTypeResult::Ok(Type::Duration));
    }

    check_unit_associated(
        ctx,
        method,
        args,
        "Duration",
        DURATION_FACTORIES,
        Type::Duration,
    )
}

/// Check Size associated functions (factory methods and default).
fn check_size_associated(
    ctx: &mut InferenceContext,
    method: &str,
    args: &[Type],
) -> Option<MethodTypeResult> {
    // Default trait associated function
    if method == "default" {
        if !args.is_empty() {
            return Some(MethodTypeResult::Err(MethodTypeError::new(
                format!("Size.default expects 0 arguments, found {}", args.len()),
                ErrorCode::E2004,
            )));
        }
        return Some(MethodTypeResult::Ok(Type::Size));
    }

    check_unit_associated(ctx, method, args, "Size", SIZE_FACTORIES, Type::Size)
}
