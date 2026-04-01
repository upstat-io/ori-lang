//! Bitfield-based trait satisfaction tables.
//!
//! [`TraitSet`] is a 64-bit bitfield where each bit represents a well-known trait.
//! Satisfaction checks become a single bitwise AND instead of N-way `||` chains.
//!
//! Modeled after TypeScript's `TypeFlags` and Zig's `InternPool` packed queries.

use crate::Idx;

/// Bit positions for well-known traits.
///
/// Internal to the `well_known` module. Adding a new well-known trait requires
/// adding a constant here and updating the satisfaction tables below.
pub(super) mod trait_bits {
    pub const EQ: u32 = 0;
    pub const COMPARABLE: u32 = 1;
    pub const CLONE: u32 = 2;
    pub const HASHABLE: u32 = 3;
    pub const DEFAULT: u32 = 4;
    pub const PRINTABLE: u32 = 5;
    pub const DEBUG: u32 = 6;
    pub const SENDABLE: u32 = 7;
    pub const ADD: u32 = 8;
    pub const SUB: u32 = 9;
    pub const MUL: u32 = 10;
    pub const DIV: u32 = 11;
    pub const FLOOR_DIV: u32 = 12;
    pub const REM: u32 = 13;
    pub const NEG: u32 = 14;
    pub const BIT_AND: u32 = 15;
    pub const BIT_OR: u32 = 16;
    pub const BIT_XOR: u32 = 17;
    pub const BIT_NOT: u32 = 18;
    pub const SHL: u32 = 19;
    pub const SHR: u32 = 20;
    pub const NOT: u32 = 21;
    pub const LEN: u32 = 22;
    pub const IS_EMPTY: u32 = 23;
    pub const ITERABLE: u32 = 24;
    pub const ITERATOR: u32 = 25;
    pub const DOUBLE_ENDED_ITERATOR: u32 = 26;
    pub const FORMATTABLE: u32 = 27;

    /// Total number of trait bits currently assigned.
    #[cfg(test)]
    pub const COUNT: u32 = 28;
}

/// Map a trait name string to its bit position in [`TraitSet`].
///
/// Returns `None` for trait names that are not tracked in the bitfield system.
/// This is the join point between registry string-based trait names and
/// bitfield positions.
pub(super) fn trait_name_to_bit(name: &str) -> Option<u32> {
    use trait_bits as tb;
    match name {
        "Eq" => Some(tb::EQ),
        "Comparable" => Some(tb::COMPARABLE),
        "Clone" => Some(tb::CLONE),
        "Hashable" => Some(tb::HASHABLE),
        "Default" => Some(tb::DEFAULT),
        "Printable" => Some(tb::PRINTABLE),
        "Debug" => Some(tb::DEBUG),
        "Sendable" => Some(tb::SENDABLE),
        "Add" => Some(tb::ADD),
        "Sub" => Some(tb::SUB),
        "Mul" => Some(tb::MUL),
        "Div" => Some(tb::DIV),
        "FloorDiv" => Some(tb::FLOOR_DIV),
        "Rem" => Some(tb::REM),
        "Neg" => Some(tb::NEG),
        "BitAnd" => Some(tb::BIT_AND),
        "BitOr" => Some(tb::BIT_OR),
        "BitXor" => Some(tb::BIT_XOR),
        "BitNot" => Some(tb::BIT_NOT),
        "Shl" => Some(tb::SHL),
        "Shr" => Some(tb::SHR),
        "Not" => Some(tb::NOT),
        "Len" => Some(tb::LEN),
        "IsEmpty" => Some(tb::IS_EMPTY),
        "Iterable" => Some(tb::ITERABLE),
        "Iterator" => Some(tb::ITERATOR),
        "DoubleEndedIterator" => Some(tb::DOUBLE_ENDED_ITERATOR),
        "Formattable" => Some(tb::FORMATTABLE),
        _ => None,
    }
}

/// A bitfield representing a set of well-known traits.
///
/// Each bit position corresponds to a specific trait (see [`trait_bits`]).
/// Satisfaction checks become O(1) bitwise AND operations instead of N-way
/// `Name` comparison chains.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TraitSet(u64);

impl TraitSet {
    /// The empty trait set — no traits satisfied.
    pub const EMPTY: Self = Self(0);

    /// Create a `TraitSet` from a slice of bit positions.
    ///
    /// Used in const context to build pre-computed trait sets for each type.
    pub const fn from_bits(bits: &[u32]) -> Self {
        let mut val = 0u64;
        let mut i = 0;
        while i < bits.len() {
            debug_assert!(bits[i] < 64, "bit position must be < 64");
            val |= 1 << bits[i];
            i += 1;
        }
        Self(val)
    }

    /// Check if this set contains the trait at the given bit position.
    #[inline]
    pub const fn contains(self, bit: u32) -> bool {
        self.0 & (1 << bit) != 0
    }

    /// Union two trait sets.
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Number of traits in this set.
    #[cfg(test)]
    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }
}

// Satisfaction table builders — derived from ori_registry

/// Derive a `TraitSet` from a registry `TypeDef`.
///
/// Combines three knowledge sources:
/// 1. `OpDefs` fields → operator trait bits (Eq, Comparable, Add, etc.)
/// 2. `MethodDef.trait_name` → method trait bits (Clone, Hashable, Len, etc.)
/// 3. `TypeDef.traits` → marker trait bits (Default, Sendable, etc.)
fn type_def_to_trait_set(type_def: &ori_registry::TypeDef) -> TraitSet {
    use crate::infer::OP_TRAIT_MAP;
    use ori_registry::OpStrategy;

    let mut bits = Vec::new();
    let ops = &type_def.operators;

    // Operator traits: Eq from ops.eq, Comparable from ops.lt
    if ops.eq != OpStrategy::Unsupported {
        if let Some(b) = trait_name_to_bit("Eq") {
            bits.push(b);
        }
    }
    if ops.lt != OpStrategy::Unsupported {
        if let Some(b) = trait_name_to_bit("Comparable") {
            bits.push(b);
        }
    }

    // Other operator traits via the shared mapping
    for &(trait_name, accessor) in OP_TRAIT_MAP {
        if accessor(ops) != OpStrategy::Unsupported {
            if let Some(b) = trait_name_to_bit(trait_name) {
                bits.push(b);
            }
        }
    }

    // Method traits
    for method in type_def.methods {
        if let Some(name) = method.trait_name {
            if let Some(b) = trait_name_to_bit(name) {
                bits.push(b);
            }
        }
    }

    // Marker traits from TypeDef.traits
    for &name in type_def.traits {
        if let Some(b) = trait_name_to_bit(name) {
            bits.push(b);
        }
    }

    TraitSet::from_bits(&bits)
}

/// Build pre-computed trait sets for all 12 primitive types.
///
/// Derived from `ori_registry` `TypeDef` data. Each primitive's trait set
/// is computed by scanning its operators, methods, and marker traits.
/// Indexed by `Idx::raw()` (INT=0, FLOAT=1, ..., ORDERING=11).
pub(super) fn build_prim_trait_sets() -> [TraitSet; Idx::PRIMITIVE_COUNT as usize] {
    use ori_registry::TypeTag;

    // Map Idx positions to TypeTags for registry lookup
    const PRIM_MAP: &[(usize, ori_registry::TypeTag)] = &[
        (0, TypeTag::Int),
        (1, TypeTag::Float),
        (2, TypeTag::Bool),
        (3, TypeTag::Str),
        (4, TypeTag::Char),
        (5, TypeTag::Byte),
        // 6 = Unit (no TypeDef)
        // 7 = Never (no TypeDef)
        // 8 = Error — has TypeDef but skipped by old code (no standard traits)
        (9, TypeTag::Duration),
        (10, TypeTag::Size),
        (11, TypeTag::Ordering),
    ];

    let mut sets = [TraitSet::EMPTY; Idx::PRIMITIVE_COUNT as usize];

    for &(idx, type_tag) in PRIM_MAP {
        if let Some(type_def) = ori_registry::find_type(type_tag) {
            sets[idx] = type_def_to_trait_set(type_def);
        }
    }

    // Error: has methods (clone, debug, to_str) but not standard primitive traits.
    // Derive from registry like any other type.
    if let Some(error_def) = ori_registry::find_type(TypeTag::Error) {
        sets[Idx::ERROR.raw() as usize] = type_def_to_trait_set(error_def);
    }

    // Unit: no TypeDef in the registry. Hardcode until Unit gets a TypeDef.
    sets[Idx::UNIT.raw() as usize] = TraitSet::from_bits(&[
        trait_bits::EQ,
        trait_bits::COMPARABLE,
        trait_bits::HASHABLE,
        trait_bits::CLONE,
        trait_bits::DEFAULT,
        trait_bits::DEBUG,
    ]);

    // Never: no traits — stays EMPTY

    sets
}

/// Build pre-computed trait sets for compound types.
///
/// Derived from `ori_registry` `TypeDef` data. Returns one `TraitSet`
/// per compound type category.
pub(super) fn build_compound_trait_sets() -> (
    TraitSet, // list
    TraitSet, // map_set
    TraitSet, // option
    TraitSet, // result
    TraitSet, // tuple
    TraitSet, // range
    TraitSet, // str (compound-level: Iterable only)
    TraitSet, // DoubleEndedIterator
    TraitSet, // Iterator
) {
    use ori_registry::TypeTag;

    let derive = |tag: TypeTag| -> TraitSet {
        ori_registry::find_type(tag).map_or(TraitSet::EMPTY, type_def_to_trait_set)
    };

    let list = derive(TypeTag::List);
    let map_set = derive(TypeTag::Map); // Map and Set share traits
    let option = derive(TypeTag::Option);
    let result = derive(TypeTag::Result);
    let tuple = derive(TypeTag::Tuple);
    let range = derive(TypeTag::Range);

    // Str compound-level: only Iterable (primitive str handles the rest).
    // The full STR TypeDef traits are in prim_trait_sets; here we add
    // compound-level traits that the Tag::Str dispatch path checks.
    let str_compound = TraitSet::from_bits(&[trait_bits::ITERABLE]);

    // DoubleEndedIterator: inherits Iterator's traits + DoubleEndedIterator
    let iterator = derive(TypeTag::Iterator);
    let dei = iterator.union(TraitSet::from_bits(&[trait_bits::DOUBLE_ENDED_ITERATOR]));

    (
        list,
        map_set,
        option,
        result,
        tuple,
        range,
        str_compound,
        dei,
        iterator,
    )
}
