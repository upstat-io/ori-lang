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
        _interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        _args: &[Type],
        _span: Span,
    ) -> MethodTypeResult {
        match receiver_ty {
            Type::Duration => check_duration_method(method),
            Type::Size => check_size_method(method),
            _ => unreachable!("handles() verified type is Duration or Size"),
        }
    }
}

fn check_duration_method(method: &str) -> MethodTypeResult {
    match method {
        // Extraction methods - return int (truncated unit value)
        "nanoseconds" | "microseconds" | "milliseconds" | "seconds" | "minutes" | "hours" => {
            MethodTypeResult::Ok(Type::Int)
        }
        _ => MethodTypeResult::Err(MethodTypeError::new(
            format!("unknown method `{method}` for type `Duration`"),
            ErrorCode::E2002,
        )),
    }
}

fn check_size_method(method: &str) -> MethodTypeResult {
    match method {
        // Extraction methods - return int (truncated unit value)
        "bytes" | "kilobytes" | "megabytes" | "gigabytes" | "terabytes" => {
            MethodTypeResult::Ok(Type::Int)
        }
        _ => MethodTypeResult::Err(MethodTypeError::new(
            format!("unknown method `{method}` for type `Size`"),
            ErrorCode::E2002,
        )),
    }
}

/// Check Duration associated functions (factory methods).
fn check_duration_associated(
    ctx: &mut InferenceContext,
    method: &str,
    args: &[Type],
) -> Option<MethodTypeResult> {
    match method {
        "from_nanoseconds" | "from_microseconds" | "from_milliseconds" | "from_seconds"
        | "from_minutes" | "from_hours" => {
            if args.len() != 1 {
                return Some(MethodTypeResult::Err(MethodTypeError::new(
                    format!("Duration.{method} expects 1 argument, found {}", args.len()),
                    ErrorCode::E2004,
                )));
            }
            if ctx.unify(&args[0], &Type::Int).is_err() {
                return Some(MethodTypeResult::Err(MethodTypeError::new(
                    format!("Duration.{method} expects int argument"),
                    ErrorCode::E2001,
                )));
            }
            Some(MethodTypeResult::Ok(Type::Duration))
        }
        _ => None,
    }
}

/// Check Size associated functions (factory methods).
fn check_size_associated(
    ctx: &mut InferenceContext,
    method: &str,
    args: &[Type],
) -> Option<MethodTypeResult> {
    match method {
        "from_bytes" | "from_kilobytes" | "from_megabytes" | "from_gigabytes"
        | "from_terabytes" => {
            if args.len() != 1 {
                return Some(MethodTypeResult::Err(MethodTypeError::new(
                    format!("Size.{method} expects 1 argument, found {}", args.len()),
                    ErrorCode::E2004,
                )));
            }
            if ctx.unify(&args[0], &Type::Int).is_err() {
                return Some(MethodTypeResult::Err(MethodTypeError::new(
                    format!("Size.{method} expects int argument"),
                    ErrorCode::E2001,
                )));
            }
            Some(MethodTypeResult::Ok(Type::Size))
        }
        _ => None,
    }
}
