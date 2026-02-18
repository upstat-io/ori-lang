//! Well-known type name resolution — single source of truth.
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
//!
//! # Performance: `WellKnownNames`
//!
//! The [`WellKnownNames`] cache pre-interns all primitive and well-known generic
//! type names at checker startup, enabling O(1) `Name` (u32) comparison instead
//! of acquiring a `RwLock` read guard + string matching on every annotation.
//! This is the primary optimization for closing the annotated-vs-inferred gap.

use ori_ir::{Name, StringInterner};

use crate::{Idx, Pool};

/// Pre-interned names for all primitive and well-known generic types.
///
/// Initialized once during `ModuleChecker::new()`. All resolution paths
/// (`resolve_type_with_vars`, `resolve_parsed_type_simple`, object safety)
/// use this cache to compare `Name` values directly (u32 equality) instead
/// of calling `interner().lookup(name)` (`RwLock` + string match).
pub(crate) struct WellKnownNames {
    // Primitive type names
    pub int: Name,
    pub float: Name,
    pub bool: Name,
    pub str: Name,
    pub char: Name,
    pub byte: Name,
    pub void: Name,
    pub unit_parens: Name, // "()"
    pub never: Name,
    pub never_upper: Name, // "Never"
    pub duration: Name,
    pub duration_upper: Name, // "Duration"
    pub size: Name,
    pub size_upper: Name, // "Size"
    pub ordering: Name,
    pub ordering_upper: Name, // "Ordering"

    // Well-known generic type names
    pub option: Name,
    pub result: Name,
    pub set: Name,
    pub channel: Name,
    pub chan: Name,
    pub range: Name,
    pub iterator: Name,
    pub double_ended_iterator: Name,
}

impl WellKnownNames {
    /// Intern all well-known names using the given interner.
    pub fn new(interner: &StringInterner) -> Self {
        Self {
            int: interner.intern("int"),
            float: interner.intern("float"),
            bool: interner.intern("bool"),
            str: interner.intern("str"),
            char: interner.intern("char"),
            byte: interner.intern("byte"),
            void: interner.intern("void"),
            unit_parens: interner.intern("()"),
            never: interner.intern("never"),
            never_upper: interner.intern("Never"),
            duration: interner.intern("duration"),
            duration_upper: interner.intern("Duration"),
            size: interner.intern("size"),
            size_upper: interner.intern("Size"),
            ordering: interner.intern("ordering"),
            ordering_upper: interner.intern("Ordering"),
            option: interner.intern("Option"),
            result: interner.intern("Result"),
            set: interner.intern("Set"),
            channel: interner.intern("Channel"),
            chan: interner.intern("Chan"),
            range: interner.intern("Range"),
            iterator: interner.intern("Iterator"),
            double_ended_iterator: interner.intern("DoubleEndedIterator"),
        }
    }

    /// Resolve a primitive type name to its fixed `Idx`, or `None` if not primitive.
    ///
    /// Pure `Name` (u32) comparison — no interner lock, no string allocation.
    #[inline]
    pub fn resolve_primitive(&self, name: Name) -> Option<Idx> {
        if name == self.int {
            Some(Idx::INT)
        } else if name == self.float {
            Some(Idx::FLOAT)
        } else if name == self.bool {
            Some(Idx::BOOL)
        } else if name == self.str {
            Some(Idx::STR)
        } else if name == self.char {
            Some(Idx::CHAR)
        } else if name == self.byte {
            Some(Idx::BYTE)
        } else if name == self.void || name == self.unit_parens {
            Some(Idx::UNIT)
        } else if name == self.never || name == self.never_upper {
            Some(Idx::NEVER)
        } else if name == self.duration || name == self.duration_upper {
            Some(Idx::DURATION)
        } else if name == self.size || name == self.size_upper {
            Some(Idx::SIZE)
        } else if name == self.ordering || name == self.ordering_upper {
            Some(Idx::ORDERING)
        } else {
            None
        }
    }

    /// Resolve a well-known generic type by `Name` and construct it in the pool.
    ///
    /// Equivalent to [`resolve_well_known_generic`] but uses `Name` comparison
    /// instead of string comparison.
    #[inline]
    pub fn resolve_generic(&self, pool: &mut Pool, name: Name, args: &[Idx]) -> Option<Idx> {
        let arity = args.len();
        if arity == 1 {
            if name == self.option {
                Some(pool.option(args[0]))
            } else if name == self.set {
                Some(pool.set(args[0]))
            } else if name == self.channel || name == self.chan {
                Some(pool.channel(args[0]))
            } else if name == self.range {
                Some(pool.range(args[0]))
            } else if name == self.iterator {
                Some(pool.iterator(args[0]))
            } else if name == self.double_ended_iterator {
                Some(pool.double_ended_iterator(args[0]))
            } else {
                None
            }
        } else if arity == 2 && name == self.result {
            Some(pool.result(args[0], args[1]))
        } else {
            None
        }
    }

    /// Check if a name with the given arity is a well-known concrete type.
    ///
    /// Equivalent to [`is_concrete_named_type`] but uses `Name` comparison.
    #[inline]
    pub fn is_concrete(&self, name: Name, num_args: usize) -> bool {
        (num_args == 1
            && (name == self.option
                || name == self.set
                || name == self.channel
                || name == self.chan
                || name == self.range
                || name == self.iterator
                || name == self.double_ended_iterator))
            || (num_args == 2 && name == self.result)
    }

    /// Resolve a registration-phase primitive (subset: Ordering, Duration, Size).
    ///
    /// Used by `resolve_parsed_type_simple` which only needs the non-keyword
    /// primitives (parser already handles int/bool/str as `ParsedType::Primitive`).
    #[inline]
    pub fn resolve_registration_primitive(&self, name: Name) -> Option<Idx> {
        if name == self.ordering || name == self.ordering_upper {
            Some(Idx::ORDERING)
        } else if name == self.duration || name == self.duration_upper {
            Some(Idx::DURATION)
        } else if name == self.size || name == self.size_upper {
            Some(Idx::SIZE)
        } else {
            None
        }
    }
}

/// Attempt to resolve a well-known generic type by string name and arity.
///
/// This is the string-based fallback used by the inference phase, which has
/// an optional interner. Prefer [`WellKnownNames::resolve_generic`] when a
/// `WellKnownNames` cache is available.
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

/// Check if a named type with the given arity resolves to a concrete Pool type.
///
/// String-based fallback for the inference phase. Prefer
/// [`WellKnownNames::is_concrete`] when a `WellKnownNames` cache is available.
pub(crate) fn is_concrete_named_type(name: &str, num_args: usize) -> bool {
    matches!(
        (name, num_args),
        (
            "Option" | "Set" | "Channel" | "Chan" | "Range" | "Iterator" | "DoubleEndedIterator",
            1
        ) | ("Result", 2)
    )
}
