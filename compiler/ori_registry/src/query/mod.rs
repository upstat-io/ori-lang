//! Query API for the Ori type registry.
//!
//! Provides lookup functions for finding types and methods by tag or name.
//! All functions are pure — they scan the static [`BUILTIN_TYPES`] array
//! assembled in [`crate::defs`].
//!
//! # DEI-Aware Method Lookup
//!
//! For `TypeTag::Iterator`, method queries exclude DEI-only methods
//! (`next_back`, `rev`, `last`, `rfind`, `rfold`). For
//! `TypeTag::DoubleEndedIterator`, all methods are visible. Both tags
//! resolve to the same `TypeDef` via [`TypeTag::base_type()`].

use std::sync::LazyLock;

use crate::defs::BUILTIN_TYPES;
use crate::{MethodDef, TypeDef, TypeTag};

/// Finds the [`TypeDef`] for a given [`TypeTag`].
///
/// Returns `None` for tags not in the registry (e.g., `Unit`, `Never`,
/// `Function`, user-defined types).
///
/// Note: `TypeTag::DoubleEndedIterator` returns `None` because no
/// separate `TypeDef` exists — use [`find_method()`] or [`methods_for()`]
/// which handle DEI aliasing via `base_type()`.
///
/// # Examples
///
/// ```
/// use ori_registry::{find_type, TypeTag};
///
/// let int_def = find_type(TypeTag::Int).unwrap();
/// assert_eq!(int_def.name, "int");
///
/// assert!(find_type(TypeTag::Unit).is_none());
/// ```
#[must_use]
pub const fn find_type(tag: TypeTag) -> Option<&'static TypeDef> {
    let mut i = 0;
    while i < BUILTIN_TYPES.len() {
        if BUILTIN_TYPES[i].tag as u8 == tag as u8 {
            return Some(BUILTIN_TYPES[i]);
        }
        i += 1;
    }
    None
}

/// Finds a method by name on a given type.
///
/// Applies [`TypeTag::base_type()`] aliasing so both `Iterator` and
/// `DoubleEndedIterator` resolve to the same method list. For plain
/// `Iterator`, DEI-only methods are excluded.
///
/// Returns `None` if the type is not in the registry or the method
/// does not exist (or is DEI-only on a plain Iterator receiver).
///
/// # Examples
///
/// ```
/// use ori_registry::{find_method, TypeTag, ReturnTag};
///
/// let abs = find_method(TypeTag::Int, "abs").unwrap();
/// assert_eq!(abs.name, "abs");
///
/// assert!(find_method(TypeTag::Int, "nonexistent").is_none());
/// ```
#[must_use]
pub fn find_method(tag: TypeTag, name: &str) -> Option<&'static MethodDef> {
    let base = tag.base_type();
    let td = find_type(base)?;
    td.methods
        .iter()
        .find(|m| m.name == name && (tag != TypeTag::Iterator || !m.dei_only))
}

/// Returns `true` if the given type has a method with the specified name.
///
/// Convenience wrapper around `find_method(...).is_some()`.
///
/// # Examples
///
/// ```
/// use ori_registry::{has_method, TypeTag};
///
/// assert!(has_method(TypeTag::Int, "abs"));
/// assert!(!has_method(TypeTag::Iterator, "next_back")); // DEI-only
/// ```
#[inline]
#[must_use]
pub fn has_method(tag: TypeTag, name: &str) -> bool {
    find_method(tag, name).is_some()
}

/// Returns all method definitions for a type, as a raw slice.
///
/// This returns the **unfiltered** method slice from the `TypeDef` —
/// DEI-only methods are included even for `TypeTag::Iterator`.
/// For DEI-filtered iteration, use [`method_names_for()`] instead.
///
/// Applies [`TypeTag::base_type()`] aliasing. Returns an empty
/// slice for types not in the registry.
///
/// # Examples
///
/// ```
/// use ori_registry::{methods_for, TypeTag};
///
/// let int_methods = methods_for(TypeTag::Int);
/// assert!(int_methods.iter().any(|m| m.name == "abs"));
///
/// assert!(methods_for(TypeTag::Unit).is_empty());
/// ```
#[must_use]
pub fn methods_for(tag: TypeTag) -> &'static [MethodDef] {
    let base = tag.base_type();
    match find_type(base) {
        Some(td) => td.methods,
        None => &[],
    }
}

/// Returns an iterator over method names for a given type.
///
/// DEI-aware: for `TypeTag::Iterator`, excludes DEI-only method names.
/// For `TypeTag::DoubleEndedIterator`, includes all method names.
/// Returns an empty iterator for types not in the registry.
///
/// # Examples
///
/// ```
/// use ori_registry::{method_names_for, TypeTag};
///
/// let names: Vec<_> = method_names_for(TypeTag::Int).collect();
/// assert!(names.contains(&"abs"));
/// assert!(names.contains(&"to_str"));
/// ```
pub fn method_names_for(tag: TypeTag) -> impl Iterator<Item = &'static str> {
    let base = tag.base_type();
    let methods = match find_type(base) {
        Some(td) => td.methods,
        None => &[],
    };
    methods
        .iter()
        .filter(move |m| tag != TypeTag::Iterator || !m.dei_only)
        .map(|m| m.name)
}

/// Returns an iterator over methods that borrow their receiver.
///
/// A method borrows its receiver when `method.receiver == Ownership::Borrow`.
/// Used by the ARC pass to determine which method calls do NOT require
/// an `rc_inc` on the receiver.
///
/// DEI-aware: filters by both borrow semantics and DEI visibility.
///
/// # Examples
///
/// ```
/// use ori_registry::{borrowing_methods, TypeTag, Ownership};
///
/// for m in borrowing_methods(TypeTag::Str) {
///     assert_eq!(m.receiver, Ownership::Borrow);
/// }
/// ```
pub fn borrowing_methods(tag: TypeTag) -> impl Iterator<Item = &'static MethodDef> {
    let base = tag.base_type();
    let methods = match find_type(base) {
        Some(td) => td.methods,
        None => &[],
    };
    methods.iter().filter(move |m| {
        m.receiver == crate::Ownership::Borrow && (tag != TypeTag::Iterator || !m.dei_only)
    })
}

/// Returns a sorted, deduplicated slice of builtin method names whose
/// receiver is borrowed and whose result is independent of the receiver's
/// lifetime.
///
/// Used by ARC borrow inference to skip RC operations at call sites for
/// inline-compiled builtin methods (e.g., `len`, `is_empty`, `compare`).
///
/// **Excluded:** Iterator/DoubleEndedIterator methods and `.iter()` —
/// these create derived values with hidden dependencies on the receiver.
/// The ARC pipeline cannot model these dependencies, so they use Owned
/// semantics (the runtime handles internal RC management).
///
/// **Note:** Protocol builtins (e.g., `__index`) are NOT included — they
/// are ARC pipeline internals, not builtin type methods. The consuming
/// crate (`ori_arc`) appends them after interning.
///
/// # Example
///
/// ```ignore
/// let set: FxHashSet<Name> = ori_registry::borrowing_method_names()
///     .iter()
///     .map(|name| interner.intern(name))
///     .collect();
/// ```
pub fn borrowing_method_names() -> &'static [&'static str] {
    // Buffer holds pre-dedup entries (~412 as of 2026-03). Deduped result
    // is ~232 unique names. 480 provides ~16% headroom for new methods.
    static NAMES: LazyLock<([&'static str; 480], usize)> = LazyLock::new(|| {
        let mut buf = [""; 480];
        let mut len = 0;
        for td in BUILTIN_TYPES {
            if td.tag == TypeTag::Iterator {
                continue;
            }
            for m in td.methods {
                if m.receiver == crate::Ownership::Borrow && m.name != "iter" {
                    assert!(
                        len < 480,
                        "borrowing_method_names overflow (capacity 480): increase array size"
                    );
                    buf[len] = m.name;
                    len += 1;
                }
            }
        }
        buf[..len].sort_unstable();
        // Deduplicate in place
        let mut write = 1;
        for read in 1..len {
            if buf[read] != buf[write - 1] {
                buf[write] = buf[read];
                write += 1;
            }
        }
        (buf, if len == 0 { 0 } else { write })
    });
    &NAMES.0[..NAMES.1]
}

/// Finds a [`TypeDef`] by its string name.
///
/// Matches the `TypeDef.name` field: lowercase for primitives
/// (`"int"`, `"str"`), `PascalCase` for special types (`"Duration"`,
/// `"Iterator"`). Case-sensitive.
///
/// Returns `None` for unknown names.
///
/// # Examples
///
/// ```
/// use ori_registry::find_type_by_name;
///
/// let str_def = find_type_by_name("str").unwrap();
/// assert!(str_def.methods.len() > 5);
///
/// assert!(find_type_by_name("FooBar").is_none());
/// ```
#[must_use]
pub fn find_type_by_name(name: &str) -> Option<&'static TypeDef> {
    BUILTIN_TYPES.iter().find(|td| td.name == name).copied()
}

// Iterator-specific query helpers

/// Returns the names of methods only available on `DoubleEndedIterator`.
///
/// These are the 5 methods with `dei_only: true`: `last`, `next_back`,
/// `rev`, `rfind`, `rfold`.
pub fn dei_only_methods() -> impl Iterator<Item = &'static str> {
    methods_for(TypeTag::Iterator)
        .iter()
        .filter(|m| m.dei_only)
        .map(|m| m.name)
}

/// Returns all iterator method names (both Iterator and DEI methods).
///
/// Unlike `method_names_for(TypeTag::Iterator)` which excludes DEI-only
/// methods, this returns all 24 method names.
pub fn iterator_method_names() -> impl Iterator<Item = &'static str> {
    methods_for(TypeTag::Iterator).iter().map(|m| m.name)
}

/// Returns `true` if the given method name is DEI-only.
///
/// Looks up the method on the Iterator `TypeDef` (unfiltered) and checks
/// the `dei_only` flag.
#[must_use]
pub fn is_dei_only(method: &str) -> bool {
    methods_for(TypeTag::Iterator)
        .iter()
        .find(|m| m.name == method)
        .is_some_and(|m| m.dei_only)
}

/// Map registry `PascalCase` type names to the lowercase convention
/// used by LLVM codegen tests and eval consistency tests.
#[must_use]
pub fn legacy_type_name(registry_name: &str) -> &str {
    match registry_name {
        "Error" => "error",
        "List" => "list",
        "Map" => "map",
        "Range" => "range",
        "Tuple" => "tuple",
        other => other,
    }
}

#[cfg(test)]
mod tests;
