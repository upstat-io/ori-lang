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
//! - [`list`]: Methods on `list` type
//! - [`collections`]: Methods on `str`, `map`, `range`, and `set` types
//! - [`variants`]: Methods on `Option`, `Result`, `bool`, `char`, `byte`, and `newtype`
//! - [`units`]: Methods on `Duration` and `Size` types
//! - [`ordering`]: Methods on `Ordering` type
//! - [`compare`]: Value comparison utilities

mod collections;
pub(crate) mod compare;
mod dispatch_check;
mod error;
pub(crate) mod helpers;
mod list;
mod names;
mod numeric;
mod ordering;
mod units;
mod variants;

#[cfg(test)]
mod tests;

pub use dispatch_check::can_dispatch_builtin;
pub(crate) use names::{BuiltinMethodNames, DispatchCtx};

use ori_ir::{Name, StringInterner};
use ori_patterns::{no_such_method, EvalResult, Value};

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
    // Comparable trait - lexicographic element comparison
    } else if method == n.compare {
        helpers::require_args("compare", 1, args.len())?;
        let Value::Tuple(a_elems) = &receiver else {
            unreachable!("dispatch_tuple_method called with non-tuple receiver")
        };
        let Value::Tuple(b_elems) = &args[0] else {
            return Err(ori_patterns::wrong_arg_type("compare", "tuple").into());
        };
        // Lexicographic comparison: compare element by element
        for (a, b) in a_elems.iter().zip(b_elems.iter()) {
            let ord = compare::compare_values(a, b, ctx.interner)?;
            if ord != std::cmp::Ordering::Equal {
                return Ok(compare::ordering_to_value(ord));
            }
        }
        // All compared elements equal, compare by length (shorter < longer)
        Ok(compare::ordering_to_value(
            a_elems.len().cmp(&b_elems.len()),
        ))
    // Eq trait - element-wise equality
    } else if method == n.equals {
        helpers::require_args("equals", 1, args.len())?;
        let eq = compare::equals_values(&receiver, &args[0], ctx.interner)?;
        Ok(Value::Bool(eq))
    // Hashable trait - recursive element hash
    } else if method == n.hash {
        helpers::require_args("hash", 0, args.len())?;
        Ok(Value::int(compare::hash_value(&receiver, ctx.interner)?))
    // Debug trait - structural representation
    } else if method == n.debug {
        helpers::require_args("debug", 0, args.len())?;
        Ok(Value::string(helpers::debug_value(&receiver)))
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
        Value::List(_) => list::dispatch_list_method(receiver, method, args, ctx),
        Value::Str(_) => collections::dispatch_string_method(receiver, method, args, ctx),
        Value::Map(_) => collections::dispatch_map_method(receiver, method, args, ctx),
        Value::Range(_) => collections::dispatch_range_method(receiver, method, args, ctx),
        Value::Set(_) => collections::dispatch_set_method(receiver, method, args, ctx),
        // Iterator methods are dispatched by CollectionMethodResolver (priority 1),
        // not the builtin resolver. See interpreter/method_dispatch/iterator.rs.
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
        Value::Error(_) => error::dispatch_error_method(receiver, method, args, ctx),
        _ => {
            let method_str = ctx.interner.lookup(method);
            Err(no_such_method(method_str, receiver.type_name()).into())
        }
    }
}

/// Dispatch a built-in method call by string name (test-only convenience).
///
/// **Warning:** Interns all 97+ builtin method names on every call. Do not
/// use in production paths. This exists solely for tests that construct
/// method names as strings.
///
/// Internally delegates to [`dispatch_builtin_method`] with a freshly-built
/// `DispatchCtx`, so dispatch behavior is identical to the production path.
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

/// NEVER CALLED. Exists solely so that Rust's exhaustive match checker
/// forces updates to this crate when a new `TypeTag` variant is added.
/// If you see a compile error pointing here, a new `TypeTag` was added
/// to `ori_registry` without updating the evaluator's method dispatch.
// Compile-time exhaustiveness guard (Roc pattern). See plans/type_strategy_registry/section-14.
#[allow(
    dead_code,
    unreachable_code,
    reason = "compile-time exhaustiveness guard — never called"
)]
fn _enforce_type_tag_exhaustiveness(tag: ori_registry::TypeTag) {
    match tag {
        // All 23 TypeTag variants — dispatched via dispatch_builtin_method() + resolvers
        ori_registry::TypeTag::Int
        | ori_registry::TypeTag::Float
        | ori_registry::TypeTag::Bool
        | ori_registry::TypeTag::Char
        | ori_registry::TypeTag::Byte
        | ori_registry::TypeTag::Duration
        | ori_registry::TypeTag::Size
        | ori_registry::TypeTag::Ordering
        | ori_registry::TypeTag::Str
        | ori_registry::TypeTag::Error
        | ori_registry::TypeTag::List
        | ori_registry::TypeTag::Map
        | ori_registry::TypeTag::Set
        | ori_registry::TypeTag::Range
        | ori_registry::TypeTag::Tuple
        | ori_registry::TypeTag::Option
        | ori_registry::TypeTag::Result
        | ori_registry::TypeTag::Iterator
        | ori_registry::TypeTag::DoubleEndedIterator
        | ori_registry::TypeTag::Channel
        | ori_registry::TypeTag::Function
        | ori_registry::TypeTag::Unit
        | ori_registry::TypeTag::Never => {}
    }
}
