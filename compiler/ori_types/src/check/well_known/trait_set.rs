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
    // Room for 37 more traits before needing u128

    /// Total number of trait bits currently assigned.
    #[cfg(test)]
    pub const COUNT: u32 = 27;
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

// ── Satisfaction table builders ─────────────────────────────────────

/// Build pre-computed trait sets for all 12 primitive types.
///
/// Indexed by `Idx::raw()` (INT=0, FLOAT=1, ..., ORDERING=11).
pub(super) fn build_prim_trait_sets() -> [TraitSet; Idx::PRIMITIVE_COUNT as usize] {
    use trait_bits as tb;

    let mut sets = [TraitSet::EMPTY; Idx::PRIMITIVE_COUNT as usize];

    // Reusable trait bundles
    let eq_cmp_clone_hash = TraitSet::from_bits(&[tb::EQ, tb::COMPARABLE, tb::CLONE, tb::HASHABLE]);
    let print_debug = TraitSet::from_bits(&[tb::PRINTABLE, tb::DEBUG]);
    let default = TraitSet::from_bits(&[tb::DEFAULT]);
    let core_bundle = eq_cmp_clone_hash.union(print_debug);

    // INT: core + default + arithmetic + bitwise
    sets[Idx::INT.raw() as usize] = core_bundle.union(default).union(TraitSet::from_bits(&[
        tb::ADD,
        tb::SUB,
        tb::MUL,
        tb::DIV,
        tb::FLOOR_DIV,
        tb::REM,
        tb::NEG,
        tb::BIT_AND,
        tb::BIT_OR,
        tb::BIT_XOR,
        tb::BIT_NOT,
        tb::SHL,
        tb::SHR,
    ]));

    // FLOAT: core + default + basic arithmetic
    sets[Idx::FLOAT.raw() as usize] = core_bundle.union(default).union(TraitSet::from_bits(&[
        tb::ADD,
        tb::SUB,
        tb::MUL,
        tb::DIV,
        tb::NEG,
    ]));

    // BOOL: core + default + not
    sets[Idx::BOOL.raw() as usize] = core_bundle
        .union(default)
        .union(TraitSet::from_bits(&[tb::NOT]));

    // STR: core + default + len/is_empty/add
    sets[Idx::STR.raw() as usize] =
        core_bundle
            .union(default)
            .union(TraitSet::from_bits(&[tb::LEN, tb::IS_EMPTY, tb::ADD]));

    // CHAR: eq + comparable + clone + hashable + printable + debug (no default)
    sets[Idx::CHAR.raw() as usize] = core_bundle;

    // BYTE: core (no default) + arithmetic + bitwise (no floor_div, no neg)
    sets[Idx::BYTE.raw() as usize] = core_bundle.union(TraitSet::from_bits(&[
        tb::ADD,
        tb::SUB,
        tb::MUL,
        tb::DIV,
        tb::REM,
        tb::BIT_AND,
        tb::BIT_OR,
        tb::BIT_XOR,
        tb::BIT_NOT,
        tb::SHL,
        tb::SHR,
    ]));

    // UNIT: eq + clone + default + debug (no comparable, no printable, no hashable)
    sets[Idx::UNIT.raw() as usize] =
        TraitSet::from_bits(&[tb::EQ, tb::CLONE, tb::DEFAULT, tb::DEBUG]);

    // NEVER: no traits (index 7) — stays EMPTY

    // ERROR: no traits (index 8) — stays EMPTY

    // DURATION: core + default + sendable + arithmetic (no floor_div, no bitwise)
    sets[Idx::DURATION.raw() as usize] = core_bundle.union(default).union(TraitSet::from_bits(&[
        tb::SENDABLE,
        tb::ADD,
        tb::SUB,
        tb::MUL,
        tb::DIV,
        tb::REM,
        tb::NEG,
    ]));

    // SIZE: core + default + sendable + arithmetic (no neg, no floor_div, no bitwise)
    sets[Idx::SIZE.raw() as usize] = core_bundle.union(default).union(TraitSet::from_bits(&[
        tb::SENDABLE,
        tb::ADD,
        tb::SUB,
        tb::MUL,
        tb::DIV,
        tb::REM,
    ]));

    // ORDERING: eq + comparable + clone + hashable + printable + debug (no default)
    sets[Idx::ORDERING.raw() as usize] = core_bundle;

    sets
}

/// Build pre-computed trait sets for compound types.
///
/// Returns one `TraitSet` per compound type category: List, Map/Set,
/// Option, Result, Tuple, Range, Str (compound), `DoubleEndedIterator`, Iterator.
#[expect(
    clippy::type_complexity,
    reason = "tuple return for struct initialization"
)]
pub(super) fn build_compound_trait_sets() -> (
    TraitSet, // list
    TraitSet, // map/set
    TraitSet, // option
    TraitSet, // result
    TraitSet, // tuple
    TraitSet, // range
    TraitSet, // str (compound-level: Iterable)
    TraitSet, // DoubleEndedIterator
    TraitSet, // Iterator
) {
    use trait_bits as tb;

    // List: eq, clone, hashable, printable, len, is_empty, comparable, iterable
    let list = TraitSet::from_bits(&[
        tb::EQ,
        tb::CLONE,
        tb::HASHABLE,
        tb::PRINTABLE,
        tb::LEN,
        tb::IS_EMPTY,
        tb::COMPARABLE,
        tb::ITERABLE,
    ]);

    // Map/Set: eq, clone, hashable, printable, len, is_empty, iterable
    let map_set = TraitSet::from_bits(&[
        tb::EQ,
        tb::CLONE,
        tb::HASHABLE,
        tb::PRINTABLE,
        tb::LEN,
        tb::IS_EMPTY,
        tb::ITERABLE,
    ]);

    // Option: eq, comparable, clone, hashable, printable, default
    let option = TraitSet::from_bits(&[
        tb::EQ,
        tb::COMPARABLE,
        tb::CLONE,
        tb::HASHABLE,
        tb::PRINTABLE,
        tb::DEFAULT,
    ]);

    // Result: eq, comparable, clone, hashable, printable
    let result = TraitSet::from_bits(&[
        tb::EQ,
        tb::COMPARABLE,
        tb::CLONE,
        tb::HASHABLE,
        tb::PRINTABLE,
    ]);

    // Tuple: eq, comparable, clone, hashable, printable, len
    let tuple = TraitSet::from_bits(&[
        tb::EQ,
        tb::COMPARABLE,
        tb::CLONE,
        tb::HASHABLE,
        tb::PRINTABLE,
        tb::LEN,
    ]);

    // Range: printable, len, iterable
    let range = TraitSet::from_bits(&[tb::PRINTABLE, tb::LEN, tb::ITERABLE]);

    // Str (compound-level): iterable only (primitive str already handles the rest)
    let str_compound = TraitSet::from_bits(&[tb::ITERABLE]);

    // DoubleEndedIterator: iterator + double_ended_iterator
    let dei = TraitSet::from_bits(&[tb::ITERATOR, tb::DOUBLE_ENDED_ITERATOR]);

    // Iterator: iterator
    let iterator_compound = TraitSet::from_bits(&[tb::ITERATOR]);

    (
        list,
        map_set,
        option,
        result,
        tuple,
        range,
        str_compound,
        dei,
        iterator_compound,
    )
}
