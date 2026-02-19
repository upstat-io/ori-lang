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
//! # Performance
//!
//! Two layers of optimization:
//!
//! 1. **[`WellKnownNames`]**: Pre-interns all type and trait names at checker
//!    startup, enabling O(1) `Name` (u32) comparison instead of acquiring a
//!    `RwLock` read guard + string matching on every annotation.
//!
//! 2. **[`TraitSet`]**: Bitfield-based trait satisfaction. Each primitive and
//!    compound type has a pre-computed `TraitSet` where each bit represents a
//!    well-known trait. Satisfaction checks become a single bitwise AND instead
//!    of N-way `||` chains.

mod trait_set;

use ori_ir::{Name, StringInterner};

use crate::{Idx, Pool, Tag};
use trait_set::trait_bits;
use trait_set::TraitSet;

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

    // Well-known trait names (frequently accessed by external code)
    pub hashable: Name,
    pub printable: Name,
    pub into_method: Name,

    // Well-known keyword names
    pub self_kw: Name,

    // Trait satisfaction bitfields (pre-computed at construction)
    trait_bit_map: [(Name, u32); 27],
    prim_trait_sets: [TraitSet; Idx::PRIMITIVE_COUNT as usize],

    // Compound type trait sets
    list_traits: TraitSet,
    map_set_traits: TraitSet,
    option_traits: TraitSet,
    result_traits: TraitSet,
    tuple_traits: TraitSet,
    range_traits: TraitSet,
    str_compound_traits: TraitSet, // str as Iterable (compound-level check)
    dei_traits: TraitSet,          // DoubleEndedIterator
    iterator_compound_traits: TraitSet,
}

/// Intern all 27 trait names and build the Name → bit position map.
fn build_trait_bit_map(interner: &StringInterner) -> ([(Name, u32); 27], Name, Name) {
    use trait_bits as tb;

    let hashable = interner.intern("Hashable");
    let printable = interner.intern("Printable");

    let map = [
        (interner.intern("Eq"), tb::EQ),
        (interner.intern("Comparable"), tb::COMPARABLE),
        (interner.intern("Clone"), tb::CLONE),
        (hashable, tb::HASHABLE),
        (interner.intern("Default"), tb::DEFAULT),
        (printable, tb::PRINTABLE),
        (interner.intern("Debug"), tb::DEBUG),
        (interner.intern("Sendable"), tb::SENDABLE),
        (interner.intern("Add"), tb::ADD),
        (interner.intern("Sub"), tb::SUB),
        (interner.intern("Mul"), tb::MUL),
        (interner.intern("Div"), tb::DIV),
        (interner.intern("FloorDiv"), tb::FLOOR_DIV),
        (interner.intern("Rem"), tb::REM),
        (interner.intern("Neg"), tb::NEG),
        (interner.intern("BitAnd"), tb::BIT_AND),
        (interner.intern("BitOr"), tb::BIT_OR),
        (interner.intern("BitXor"), tb::BIT_XOR),
        (interner.intern("BitNot"), tb::BIT_NOT),
        (interner.intern("Shl"), tb::SHL),
        (interner.intern("Shr"), tb::SHR),
        (interner.intern("Not"), tb::NOT),
        (interner.intern("Len"), tb::LEN),
        (interner.intern("IsEmpty"), tb::IS_EMPTY),
        (interner.intern("Iterable"), tb::ITERABLE),
        (interner.intern("Iterator"), tb::ITERATOR),
        (
            interner.intern("DoubleEndedIterator"),
            tb::DOUBLE_ENDED_ITERATOR,
        ),
    ];

    (map, hashable, printable)
}

impl WellKnownNames {
    /// Intern all well-known names and build trait satisfaction tables.
    pub fn new(interner: &StringInterner) -> Self {
        let (trait_bit_map, hashable, printable) = build_trait_bit_map(interner);

        // Iterator/DEI names are needed for both trait_bit_map and struct fields
        let iterator = interner.intern("Iterator");
        let double_ended_iterator = interner.intern("DoubleEndedIterator");

        let prim_trait_sets = trait_set::build_prim_trait_sets();
        let (
            list_traits,
            map_set_traits,
            option_traits,
            result_traits,
            tuple_traits,
            range_traits,
            str_compound_traits,
            dei_traits,
            iterator_compound_traits,
        ) = trait_set::build_compound_trait_sets();

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
            iterator,
            double_ended_iterator,
            hashable,
            printable,
            into_method: interner.intern("into"),
            self_kw: interner.intern("self"),
            trait_bit_map,
            prim_trait_sets,
            list_traits,
            map_set_traits,
            option_traits,
            result_traits,
            tuple_traits,
            range_traits,
            str_compound_traits,
            dei_traits,
            iterator_compound_traits,
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

    // ── Trait satisfaction (bitfield-based O(1) lookup) ────────────────

    /// Check if a primitive type inherently satisfies a trait.
    ///
    /// O(1) via pre-computed bitfield lookup. Each primitive type has a
    /// pre-built `TraitSet`; the trait `Name` is mapped to a bit position
    /// via `trait_bit_map`, then checked with a single bitwise AND.
    #[inline]
    pub fn primitive_satisfies_trait(&self, ty: Idx, t: Name) -> bool {
        let idx = ty.raw() as usize;
        if idx >= self.prim_trait_sets.len() {
            return false;
        }
        match self.trait_bit(t) {
            Some(bit) => self.prim_trait_sets[idx].contains(bit),
            None => false,
        }
    }

    /// Check if a type satisfies a trait, including compound types via Pool tags.
    ///
    /// Checks primitives first (most common case), then compound types by tag.
    /// O(1) via pre-computed bitfield lookup for both.
    pub fn type_satisfies_trait(&self, ty: Idx, t: Name, pool: &Pool) -> bool {
        if self.primitive_satisfies_trait(ty, t) {
            return true;
        }

        let set = match pool.tag(ty) {
            Tag::List => self.list_traits,
            Tag::Map | Tag::Set => self.map_set_traits,
            Tag::Option => self.option_traits,
            Tag::Result => self.result_traits,
            Tag::Tuple => self.tuple_traits,
            Tag::Range => self.range_traits,
            Tag::Str => self.str_compound_traits,
            Tag::DoubleEndedIterator => self.dei_traits,
            Tag::Iterator => self.iterator_compound_traits,
            _ => return false,
        };

        match self.trait_bit(t) {
            Some(bit) => set.contains(bit),
            None => false,
        }
    }

    /// Convert a trait `Name` to its bit position in [`TraitSet`].
    ///
    /// Linear scan over 27 entries — fast enough for `u32` comparisons.
    #[inline]
    fn trait_bit(&self, name: Name) -> Option<u32> {
        for &(n, bit) in &self.trait_bit_map {
            if n == name {
                return Some(bit);
            }
        }
        None
    }

    /// Check if a Name is registered as a well-known trait in the satisfaction system.
    ///
    /// Test-only: used by sync enforcement tests to verify `DerivedTrait` names
    /// are properly mapped to trait satisfaction bits.
    #[cfg(test)]
    pub fn has_trait_bit(&self, name: Name) -> bool {
        self.trait_bit(name).is_some()
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

#[cfg(test)]
mod tests;
