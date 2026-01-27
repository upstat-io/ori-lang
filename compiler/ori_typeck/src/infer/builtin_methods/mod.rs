//! Built-in method type inference handlers.
//!
//! This module extracts built-in method type checking logic,
//! following the Open/Closed Principle. Each type has its own handler
//! that implements the `BuiltinMethodHandler` trait.

mod string;
mod list;
mod map;
mod option;
mod result;
mod numeric;

use ori_diagnostic::ErrorCode;
use ori_ir::{Span, StringInterner};
use ori_types::{InferenceContext, Type};

pub use string::StringMethodHandler;
pub use list::ListMethodHandler;
pub use map::MapMethodHandler;
pub use option::OptionMethodHandler;
pub use result::ResultMethodHandler;
pub use numeric::NumericMethodHandler;

/// Result of type checking a method call.
pub enum MethodTypeResult {
    /// Successfully type checked, returning the result type.
    Ok(Type),
    /// Type error occurred.
    Err(MethodTypeError),
}

/// Error from type checking a method call.
#[derive(Debug)]
pub struct MethodTypeError {
    /// Error message.
    pub message: String,
    /// Error code for diagnostics.
    pub code: ErrorCode,
}

impl MethodTypeError {
    pub fn new(message: impl Into<String>, code: ErrorCode) -> Self {
        MethodTypeError {
            message: message.into(),
            code,
        }
    }
}

/// Trait for type checking method calls on built-in types.
///
/// Implementations handle specific receiver types.
pub trait BuiltinMethodHandler: Send + Sync {
    /// Check if this handler handles the given receiver type.
    fn handles(&self, receiver_ty: &Type) -> bool;

    /// Type check the method call.
    ///
    /// The inference context is provided for unification and fresh variables.
    /// The interner is provided for method name lookup and type display.
    fn check(
        &self,
        ctx: &mut InferenceContext,
        interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        args: &[Type],
        span: Span,
    ) -> MethodTypeResult;
}

/// Registry of built-in method handlers.
///
/// Provides a way to type check method calls by delegating to registered handlers.
pub struct BuiltinMethodRegistry {
    handlers: Vec<Box<dyn BuiltinMethodHandler>>,
}

impl BuiltinMethodRegistry {
    /// Create a new built-in method registry with all handlers.
    pub fn new() -> Self {
        BuiltinMethodRegistry {
            handlers: vec![
                Box::new(StringMethodHandler),
                Box::new(ListMethodHandler),
                Box::new(MapMethodHandler),
                Box::new(OptionMethodHandler),
                Box::new(ResultMethodHandler),
                Box::new(NumericMethodHandler),
            ],
        }
    }

    /// Type check a method call.
    ///
    /// Tries each registered handler in order until one handles the receiver type.
    pub fn check(
        &self,
        ctx: &mut InferenceContext,
        interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        args: &[Type],
        span: Span,
    ) -> Option<MethodTypeResult> {
        for handler in &self.handlers {
            if handler.handles(receiver_ty) {
                return Some(handler.check(ctx, interner, receiver_ty, method, args, span));
            }
        }

        None
    }
}

impl Default for BuiltinMethodRegistry {
    fn default() -> Self {
        Self::new()
    }
}
