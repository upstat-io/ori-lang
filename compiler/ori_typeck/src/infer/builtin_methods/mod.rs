//! Built-in method type inference handlers.
//!
//! This module extracts built-in method type checking logic,
//! following the Open/Closed Principle. Each type has its own handler
//! that implements the `BuiltinMethodHandler` trait.

mod list;
mod map;
mod numeric;
mod option;
mod result;
mod string;
mod units;

use ori_diagnostic::ErrorCode;
use ori_ir::{Span, StringInterner};
use ori_types::{InferenceContext, Type};

pub use list::ListMethodHandler;
pub use map::MapMethodHandler;
pub use numeric::NumericMethodHandler;
pub use option::OptionMethodHandler;
pub use result::ResultMethodHandler;
pub use string::StringMethodHandler;
pub use units::UnitsMethodHandler;

/// All built-in methods registered in the type checker's handlers.
///
/// Used by cross-crate consistency tests to verify the evaluator and type
/// checker agree on which methods exist. Each entry is `(type_name, method_name)`.
/// Sorted by type then method for deterministic comparison.
pub const TYPECK_BUILTIN_METHODS: &[(&str, &str)] = &[
    // bool
    ("bool", "to_string"),
    // duration
    ("duration", "hours"),
    ("duration", "microseconds"),
    ("duration", "milliseconds"),
    ("duration", "minutes"),
    ("duration", "nanoseconds"),
    ("duration", "seconds"),
    // float
    ("float", "abs"),
    ("float", "ceil"),
    ("float", "compare"),
    ("float", "floor"),
    ("float", "max"),
    ("float", "min"),
    ("float", "round"),
    ("float", "sqrt"),
    ("float", "to_string"),
    // int
    ("int", "abs"),
    ("int", "compare"),
    ("int", "max"),
    ("int", "min"),
    ("int", "to_string"),
    // list
    ("list", "contains"),
    ("list", "filter"),
    ("list", "find"),
    ("list", "first"),
    ("list", "fold"),
    ("list", "is_empty"),
    ("list", "last"),
    ("list", "len"),
    ("list", "map"),
    ("list", "pop"),
    ("list", "push"),
    ("list", "reverse"),
    ("list", "sort"),
    // map
    ("map", "contains_key"),
    ("map", "get"),
    ("map", "insert"),
    ("map", "is_empty"),
    ("map", "keys"),
    ("map", "len"),
    ("map", "remove"),
    ("map", "values"),
    // option
    ("option", "and_then"),
    ("option", "filter"),
    ("option", "is_none"),
    ("option", "is_some"),
    ("option", "map"),
    ("option", "ok_or"),
    ("option", "unwrap"),
    ("option", "unwrap_or"),
    // result
    ("result", "and_then"),
    ("result", "err"),
    ("result", "is_err"),
    ("result", "is_ok"),
    ("result", "map"),
    ("result", "map_err"),
    ("result", "ok"),
    ("result", "unwrap"),
    ("result", "unwrap_err"),
    ("result", "unwrap_or"),
    // size
    ("size", "bytes"),
    ("size", "gigabytes"),
    ("size", "kilobytes"),
    ("size", "megabytes"),
    ("size", "terabytes"),
    // str
    ("str", "bytes"),
    ("str", "chars"),
    ("str", "contains"),
    ("str", "ends_with"),
    ("str", "is_empty"),
    ("str", "len"),
    ("str", "split"),
    ("str", "starts_with"),
    ("str", "to_lowercase"),
    ("str", "to_uppercase"),
    ("str", "trim"),
];

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
/// Uses direct type-based dispatch instead of linear search through handlers.
/// Each handler is a ZST (zero-sized type), so this struct has zero runtime overhead.
pub struct BuiltinMethodRegistry {
    string: StringMethodHandler,
    list: ListMethodHandler,
    map: MapMethodHandler,
    option: OptionMethodHandler,
    result: ResultMethodHandler,
    numeric: NumericMethodHandler,
    units: UnitsMethodHandler,
}

impl BuiltinMethodRegistry {
    /// Create a new built-in method registry with all handlers.
    pub fn new() -> Self {
        BuiltinMethodRegistry {
            string: StringMethodHandler,
            list: ListMethodHandler,
            map: MapMethodHandler,
            option: OptionMethodHandler,
            result: ResultMethodHandler,
            numeric: NumericMethodHandler,
            units: UnitsMethodHandler,
        }
    }

    /// Type check a method call.
    ///
    /// Dispatches directly to the appropriate handler based on receiver type.
    /// Returns `None` if no handler matches the receiver type.
    ///
    /// Uses direct dispatch (no trait object) for better inlining and no vtable overhead.
    pub fn check(
        &self,
        ctx: &mut InferenceContext,
        interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        args: &[Type],
        span: Span,
    ) -> Option<MethodTypeResult> {
        // Direct dispatch - no dyn, compiler sees concrete types for inlining
        Some(match receiver_ty {
            Type::Str => self
                .string
                .check(ctx, interner, receiver_ty, method, args, span),
            Type::List(_) => self
                .list
                .check(ctx, interner, receiver_ty, method, args, span),
            Type::Map { .. } => self
                .map
                .check(ctx, interner, receiver_ty, method, args, span),
            Type::Option(_) => self
                .option
                .check(ctx, interner, receiver_ty, method, args, span),
            Type::Result { .. } => {
                self.result
                    .check(ctx, interner, receiver_ty, method, args, span)
            }
            Type::Int | Type::Float | Type::Bool => {
                self.numeric
                    .check(ctx, interner, receiver_ty, method, args, span)
            }
            Type::Duration | Type::Size => {
                self.units
                    .check(ctx, interner, receiver_ty, method, args, span)
            }
            _ => return None,
        })
    }

    /// Type check an associated function call.
    ///
    /// Associated functions are called on type names (e.g., `Duration.from_seconds`)
    /// rather than on instances. Returns `None` if the type doesn't support
    /// associated functions.
    pub fn check_associated(
        &self,
        ctx: &mut InferenceContext,
        type_name: &str,
        method: &str,
        args: &[Type],
    ) -> Option<MethodTypeResult> {
        self.units.check_associated(ctx, type_name, method, args)
    }
}

impl Default for BuiltinMethodRegistry {
    fn default() -> Self {
        Self::new()
    }
}
