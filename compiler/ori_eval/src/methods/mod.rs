//! Method dispatch implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for built-in method calls. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.
//!
//! # Module Structure
//!
//! - [`helpers`]: Argument validation and shared utilities
//! - [`numeric`]: Methods on `int` and `float` types
//! - [`collections`]: Methods on `list`, `str`, `map`, and `range` types
//! - [`variants`]: Methods on `Option`, `Result`, `bool`, `char`, `byte`, and `newtype`
//! - [`units`]: Methods on `Duration` and `Size` types
//! - [`ordering`]: Methods on `Ordering` type
//! - [`compare`]: Value comparison utilities

mod collections;
mod compare;
mod helpers;
mod iterator;
mod numeric;
mod ordering;
mod units;
mod variants;

use ori_ir::{Name, StringInterner};
use ori_patterns::{no_such_method, EvalResult, Value};

pub use helpers::EVAL_BUILTIN_METHODS;

/// Pre-interned method names for builtin method dispatch.
///
/// Constructed once per Interpreter (like `TypeNames`, `PrintNames`, `PropNames`).
/// Each field holds the interned `Name` for a builtin method, enabling
/// `u32 == u32` comparison instead of string matching during dispatch.
///
/// This eliminates the de-interning that previously happened in
/// `dispatch_builtin_method` on every builtin method call.
#[derive(Clone, Copy)]
pub(crate) struct BuiltinMethodNames {
    // Common trait methods (used across many types)
    pub(crate) add: Name,
    pub(crate) sub: Name,
    pub(crate) mul: Name,
    pub(crate) div: Name,
    pub(crate) rem: Name,
    pub(crate) neg: Name,
    pub(crate) compare: Name,
    pub(crate) equals: Name,
    pub(crate) clone_: Name,
    pub(crate) to_str: Name,
    pub(crate) debug: Name,
    pub(crate) hash: Name,
    pub(crate) contains: Name,
    pub(crate) len: Name,
    pub(crate) is_empty: Name,
    pub(crate) not: Name,
    pub(crate) unwrap: Name,
    pub(crate) concat: Name,

    // Numeric (int-specific)
    pub(crate) floor_div: Name,
    pub(crate) bit_and: Name,
    pub(crate) bit_or: Name,
    pub(crate) bit_xor: Name,
    pub(crate) bit_not: Name,
    pub(crate) shl: Name,
    pub(crate) shr: Name,

    // Collection (string-specific)
    pub(crate) to_uppercase: Name,
    pub(crate) to_lowercase: Name,
    pub(crate) trim: Name,
    pub(crate) starts_with: Name,
    pub(crate) ends_with: Name,

    // Collection (list-specific)
    pub(crate) first: Name,
    pub(crate) last: Name,

    // Collection (map-specific)
    pub(crate) contains_key: Name,
    pub(crate) keys: Name,
    pub(crate) values: Name,

    // Variant (Option-specific)
    pub(crate) unwrap_or: Name,
    pub(crate) is_some: Name,
    pub(crate) is_none: Name,
    pub(crate) ok_or: Name,

    // Variant (Result-specific)
    pub(crate) is_ok: Name,
    pub(crate) is_err: Name,

    // Ordering
    pub(crate) is_less: Name,
    pub(crate) is_equal: Name,
    pub(crate) is_greater: Name,
    pub(crate) is_less_or_equal: Name,
    pub(crate) is_greater_or_equal: Name,
    pub(crate) reverse: Name,
    pub(crate) then: Name,

    // Type names for associated function dispatch
    pub(crate) duration: Name,
    pub(crate) size: Name,

    // Duration/Size operator aliases
    pub(crate) subtract: Name,
    pub(crate) multiply: Name,
    pub(crate) divide: Name,
    pub(crate) remainder: Name,
    pub(crate) negate: Name,

    // Duration accessors
    pub(crate) nanoseconds: Name,
    pub(crate) microseconds: Name,
    pub(crate) milliseconds: Name,
    pub(crate) seconds: Name,
    pub(crate) minutes: Name,
    pub(crate) hours: Name,

    // Size accessors
    pub(crate) bytes: Name,
    pub(crate) kilobytes: Name,
    pub(crate) megabytes: Name,
    pub(crate) gigabytes: Name,
    pub(crate) terabytes: Name,

    // Iterator
    pub(crate) iter: Name,
    pub(crate) next: Name,
}

impl BuiltinMethodNames {
    /// Pre-intern all builtin method names.
    pub(crate) fn new(interner: &StringInterner) -> Self {
        Self {
            // Common trait methods
            add: interner.intern("add"),
            sub: interner.intern("sub"),
            mul: interner.intern("mul"),
            div: interner.intern("div"),
            rem: interner.intern("rem"),
            neg: interner.intern("neg"),
            compare: interner.intern("compare"),
            equals: interner.intern("equals"),
            clone_: interner.intern("clone"),
            to_str: interner.intern("to_str"),
            debug: interner.intern("debug"),
            hash: interner.intern("hash"),
            contains: interner.intern("contains"),
            len: interner.intern("len"),
            is_empty: interner.intern("is_empty"),
            not: interner.intern("not"),
            unwrap: interner.intern("unwrap"),
            concat: interner.intern("concat"),
            // Numeric
            floor_div: interner.intern("floor_div"),
            bit_and: interner.intern("bit_and"),
            bit_or: interner.intern("bit_or"),
            bit_xor: interner.intern("bit_xor"),
            bit_not: interner.intern("bit_not"),
            shl: interner.intern("shl"),
            shr: interner.intern("shr"),
            // String
            to_uppercase: interner.intern("to_uppercase"),
            to_lowercase: interner.intern("to_lowercase"),
            trim: interner.intern("trim"),
            starts_with: interner.intern("starts_with"),
            ends_with: interner.intern("ends_with"),
            // List
            first: interner.intern("first"),
            last: interner.intern("last"),
            // Map
            contains_key: interner.intern("contains_key"),
            keys: interner.intern("keys"),
            values: interner.intern("values"),
            // Option
            unwrap_or: interner.intern("unwrap_or"),
            is_some: interner.intern("is_some"),
            is_none: interner.intern("is_none"),
            ok_or: interner.intern("ok_or"),
            // Result
            is_ok: interner.intern("is_ok"),
            is_err: interner.intern("is_err"),
            // Ordering
            is_less: interner.intern("is_less"),
            is_equal: interner.intern("is_equal"),
            is_greater: interner.intern("is_greater"),
            is_less_or_equal: interner.intern("is_less_or_equal"),
            is_greater_or_equal: interner.intern("is_greater_or_equal"),
            reverse: interner.intern("reverse"),
            then: interner.intern("then"),
            // Type names
            duration: interner.intern("Duration"),
            size: interner.intern("Size"),
            // Duration/Size aliases
            subtract: interner.intern("subtract"),
            multiply: interner.intern("multiply"),
            divide: interner.intern("divide"),
            remainder: interner.intern("remainder"),
            negate: interner.intern("negate"),
            // Duration accessors
            nanoseconds: interner.intern("nanoseconds"),
            microseconds: interner.intern("microseconds"),
            milliseconds: interner.intern("milliseconds"),
            seconds: interner.intern("seconds"),
            minutes: interner.intern("minutes"),
            hours: interner.intern("hours"),
            // Size accessors
            bytes: interner.intern("bytes"),
            kilobytes: interner.intern("kilobytes"),
            megabytes: interner.intern("megabytes"),
            gigabytes: interner.intern("gigabytes"),
            terabytes: interner.intern("terabytes"),
            // Iterator
            iter: interner.intern("iter"),
            next: interner.intern("next"),
        }
    }
}

/// Context for builtin method dispatch.
///
/// Bundles pre-interned method names and the string interner to keep
/// sub-dispatch function signatures at 4 parameters. The interner is
/// only used on cold paths (error messages, deep comparisons).
#[derive(Clone, Copy)]
pub(crate) struct DispatchCtx<'a> {
    pub(crate) names: &'a BuiltinMethodNames,
    pub(crate) interner: &'a StringInterner,
}

/// Dispatch an associated function call (static method without receiver instance).
///
/// Associated functions are called on type names rather than instances,
/// e.g., `Duration.from_seconds(s: 10)`.
///
/// Uses pre-interned `Name` comparison for the type name (fast `u32 == u32`).
/// Falls back to string lookup only for the method name dispatch within each
/// type's handler (cold path — associated function calls are infrequent).
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature with other dispatch functions"
)]
pub(crate) fn dispatch_associated_function(
    type_name: Name,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let method_str = ctx.interner.lookup(method);
    if type_name == ctx.names.duration {
        units::dispatch_duration_associated(method_str, &args)
    } else if type_name == ctx.names.size {
        units::dispatch_size_associated(method_str, &args)
    } else {
        let type_name_str = ctx.interner.lookup(type_name);
        Err(no_such_method(method_str, type_name_str).into())
    }
}

/// Dispatch methods on tuple values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Consistent method dispatch signature with other dispatch functions"
)]
fn dispatch_tuple_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let n = ctx.names;
    if method == n.clone_ {
        helpers::require_args("clone", 0, args.len())?;
        Ok(receiver)
    } else if method == n.len {
        helpers::require_args("len", 0, args.len())?;
        let Value::Tuple(elems) = &receiver else {
            unreachable!("dispatch_tuple_method called with non-tuple receiver")
        };
        helpers::len_to_value(elems.len(), "tuple")
    } else {
        let method_str = ctx.interner.lookup(method);
        Err(no_such_method(method_str, "tuple").into())
    }
}

/// Dispatch a built-in method call using pre-interned `Name` comparison.
///
/// This is the production entry point for built-in method calls. Uses
/// `Name`-based dispatch (`u32 == u32`) in sub-modules, avoiding the
/// de-interning that would be needed for string matching.
///
/// The `DispatchCtx` bundles pre-interned names (for hot-path comparison)
/// and the interner (for cold-path error messages).
pub(crate) fn dispatch_builtin_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    match &receiver {
        Value::Int(_) => numeric::dispatch_int_method(receiver, method, args, ctx),
        Value::Float(_) => numeric::dispatch_float_method(receiver, method, args, ctx),
        Value::Bool(_) => variants::dispatch_bool_method(receiver, method, args, ctx),
        Value::Char(_) => variants::dispatch_char_method(receiver, method, args, ctx),
        Value::Byte(_) => variants::dispatch_byte_method(receiver, method, args, ctx),
        Value::List(_) => collections::dispatch_list_method(receiver, method, args, ctx),
        Value::Str(_) => collections::dispatch_string_method(receiver, method, args, ctx),
        Value::Map(_) => collections::dispatch_map_method(receiver, method, args, ctx),
        Value::Range(_) => collections::dispatch_range_method(receiver, method, args, ctx),
        Value::Iterator(_) => iterator::dispatch_iterator_method(receiver, method, &args, ctx),
        Value::Some(_) | Value::None => {
            variants::dispatch_option_method(receiver, method, args, ctx)
        }
        Value::Ok(_) | Value::Err(_) => {
            variants::dispatch_result_method(receiver, method, args, ctx)
        }
        Value::Newtype { .. } => variants::dispatch_newtype_method(receiver, method, args, ctx),
        Value::Duration(_) => units::dispatch_duration_method(receiver, method, args, ctx),
        Value::Size(_) => units::dispatch_size_method(receiver, method, args, ctx),
        Value::Ordering(_) => ordering::dispatch_ordering_method(receiver, method, args, ctx),
        Value::Tuple(_) => dispatch_tuple_method(receiver, method, args, ctx),
        _ => {
            let method_str = ctx.interner.lookup(method);
            Err(no_such_method(method_str, receiver.type_name()).into())
        }
    }
}

/// Dispatch a built-in method call by string name.
///
/// Used by tests that construct method names directly. Production code should
/// use [`dispatch_builtin_method`] with a pre-interned `Name` via `DispatchCtx`.
pub fn dispatch_builtin_method_str(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    interner: &StringInterner,
) -> EvalResult {
    let names = BuiltinMethodNames::new(interner);
    let ctx = DispatchCtx {
        names: &names,
        interner,
    };
    let method_name = interner.intern(method);
    dispatch_builtin_method(receiver, method_name, args, &ctx)
}
