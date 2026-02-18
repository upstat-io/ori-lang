//! Well-known generic type resolution — single source of truth.
//!
//! Three separate type resolution functions exist across the type checker:
//! - `resolve_parsed_type_simple()` — registration phase
//! - `resolve_type_with_vars()` — signature collection phase
//! - `resolve_parsed_type()` — inference phase
//!
//! Each must construct well-known generic types (Option, Result, Set, etc.)
//! using their dedicated Pool constructors to ensure unification works correctly.
//! This module centralizes that table so adding a new well-known generic
//! (e.g., `SortedMap`) requires updating exactly one location.

use crate::{Idx, Pool};

/// Attempt to resolve a well-known generic type by name and arity.
///
/// Well-known generic types have dedicated Pool constructors that produce
/// specific Tags (e.g., `Tag::Option`, `Tag::Result`). Using these constructors
/// ensures that `Option<int>` from a type annotation produces the same `Idx`
/// as `pool.option(int)` from inference — without this, unification fails.
///
/// Returns `Some(idx)` if the name+arity matches a well-known generic,
/// `None` otherwise (caller should fall through to `pool.applied()`).
pub(crate) fn resolve_well_known_generic(
    pool: &mut Pool,
    name: &str,
    resolved_args: &[Idx],
) -> Option<Idx> {
    match (name, resolved_args.len()) {
        ("Option", 1) => Some(pool.option(resolved_args[0])),
        ("Result", 2) => Some(pool.result(resolved_args[0], resolved_args[1])),
        ("Set", 1) => Some(pool.set(resolved_args[0])),
        ("Channel" | "Chan", 1) => Some(pool.channel(resolved_args[0])),
        ("Range", 1) => Some(pool.range(resolved_args[0])),
        ("Iterator", 1) => Some(pool.iterator(resolved_args[0])),
        ("DoubleEndedIterator", 1) => Some(pool.double_ended_iterator(resolved_args[0])),
        _ => None,
    }
}

/// Check if a named type with the given arity resolves to a concrete Pool type
/// rather than a trait object.
///
/// Derived from the same set as [`resolve_well_known_generic`]. These types
/// have dedicated Pool constructors and are NOT trait objects even if a
/// same-named trait exists in the registry. Used by object safety checks
/// to avoid false positives.
pub(crate) fn is_concrete_named_type(name: &str, num_args: usize) -> bool {
    matches!(
        (name, num_args),
        (
            "Option" | "Set" | "Channel" | "Chan" | "Range" | "Iterator" | "DoubleEndedIterator",
            1
        ) | ("Result", 2)
    )
}
