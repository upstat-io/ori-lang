//! Tests for well-known name resolution and trait satisfaction bitfields.

use ori_ir::StringInterner;

use super::trait_bits;
use super::{TraitSet, WellKnownNames};
use crate::Idx;

// ── TraitSet construction ───────────────────────────────────────────

#[test]
fn empty_set_contains_nothing() {
    for bit in 0..trait_bits::COUNT {
        assert!(
            !TraitSet::EMPTY.contains(bit),
            "EMPTY should not contain bit {bit}"
        );
    }
}

#[test]
fn default_is_empty() {
    assert_eq!(TraitSet::default(), TraitSet::EMPTY);
}

#[test]
fn from_bits_empty_slice() {
    let set = TraitSet::from_bits(&[]);
    assert_eq!(set, TraitSet::EMPTY);
}

#[test]
fn from_bits_single() {
    let set = TraitSet::from_bits(&[trait_bits::EQ]);
    assert!(set.contains(trait_bits::EQ));
    assert!(!set.contains(trait_bits::CLONE));
    assert!(!set.contains(trait_bits::ADD));
}

#[test]
fn from_bits_multiple() {
    let set = TraitSet::from_bits(&[trait_bits::EQ, trait_bits::CLONE, trait_bits::HASHABLE]);
    assert!(set.contains(trait_bits::EQ));
    assert!(set.contains(trait_bits::CLONE));
    assert!(set.contains(trait_bits::HASHABLE));
    assert!(!set.contains(trait_bits::COMPARABLE));
    assert!(!set.contains(trait_bits::ADD));
}

#[test]
fn from_bits_all_traits() {
    let all: Vec<u32> = (0..trait_bits::COUNT).collect();
    let set = TraitSet::from_bits(&all);
    for bit in 0..trait_bits::COUNT {
        assert!(set.contains(bit), "should contain bit {bit}");
    }
}

#[test]
fn from_bits_duplicate_bits_are_idempotent() {
    let set = TraitSet::from_bits(&[trait_bits::EQ, trait_bits::EQ, trait_bits::EQ]);
    assert!(set.contains(trait_bits::EQ));
    assert_eq!(set.count(), 1);
}

// ── TraitSet operations ─────────────────────────────────────────────

#[test]
fn union_combines_bits() {
    let a = TraitSet::from_bits(&[trait_bits::EQ, trait_bits::CLONE]);
    let b = TraitSet::from_bits(&[trait_bits::HASHABLE, trait_bits::ADD]);
    let merged = a.union(b);
    assert!(merged.contains(trait_bits::EQ));
    assert!(merged.contains(trait_bits::CLONE));
    assert!(merged.contains(trait_bits::HASHABLE));
    assert!(merged.contains(trait_bits::ADD));
    assert!(!merged.contains(trait_bits::SUB));
}

#[test]
fn union_with_empty_is_identity() {
    let set = TraitSet::from_bits(&[trait_bits::EQ, trait_bits::CLONE]);
    assert_eq!(set.union(TraitSet::EMPTY), set);
    assert_eq!(TraitSet::EMPTY.union(set), set);
}

#[test]
fn union_overlapping_is_idempotent() {
    let a = TraitSet::from_bits(&[trait_bits::EQ, trait_bits::CLONE]);
    let b = TraitSet::from_bits(&[trait_bits::CLONE, trait_bits::HASHABLE]);
    let merged = a.union(b);
    assert_eq!(merged.count(), 3);
    assert!(merged.contains(trait_bits::EQ));
    assert!(merged.contains(trait_bits::CLONE));
    assert!(merged.contains(trait_bits::HASHABLE));
}

#[test]
fn union_is_commutative() {
    let a = TraitSet::from_bits(&[trait_bits::EQ, trait_bits::ADD]);
    let b = TraitSet::from_bits(&[trait_bits::CLONE, trait_bits::SUB]);
    assert_eq!(a.union(b), b.union(a));
}

#[test]
fn union_is_associative() {
    let a = TraitSet::from_bits(&[trait_bits::EQ]);
    let b = TraitSet::from_bits(&[trait_bits::CLONE]);
    let c = TraitSet::from_bits(&[trait_bits::ADD]);
    assert_eq!(a.union(b).union(c), a.union(b.union(c)));
}

// ── TraitSet::count ─────────────────────────────────────────────────

#[test]
fn count_empty() {
    assert_eq!(TraitSet::EMPTY.count(), 0);
}

#[test]
fn count_single() {
    assert_eq!(TraitSet::from_bits(&[trait_bits::EQ]).count(), 1);
}

#[test]
fn count_multiple() {
    let set = TraitSet::from_bits(&[
        trait_bits::EQ,
        trait_bits::CLONE,
        trait_bits::HASHABLE,
        trait_bits::ADD,
        trait_bits::SUB,
    ]);
    assert_eq!(set.count(), 5);
}

// ── TraitSet is Copy ────────────────────────────────────────────────

#[test]
fn trait_set_is_copy() {
    let a = TraitSet::from_bits(&[trait_bits::EQ]);
    let b = a; // Copy, not move
    assert_eq!(a, b); // both still usable
}

// ── trait_bits completeness ─────────────────────────────────────────

#[test]
fn bit_positions_are_unique() {
    let all_bits = [
        trait_bits::EQ,
        trait_bits::COMPARABLE,
        trait_bits::CLONE,
        trait_bits::HASHABLE,
        trait_bits::DEFAULT,
        trait_bits::PRINTABLE,
        trait_bits::DEBUG,
        trait_bits::SENDABLE,
        trait_bits::ADD,
        trait_bits::SUB,
        trait_bits::MUL,
        trait_bits::DIV,
        trait_bits::FLOOR_DIV,
        trait_bits::REM,
        trait_bits::NEG,
        trait_bits::BIT_AND,
        trait_bits::BIT_OR,
        trait_bits::BIT_XOR,
        trait_bits::BIT_NOT,
        trait_bits::SHL,
        trait_bits::SHR,
        trait_bits::NOT,
        trait_bits::LEN,
        trait_bits::IS_EMPTY,
        trait_bits::ITERABLE,
        trait_bits::ITERATOR,
        trait_bits::DOUBLE_ENDED_ITERATOR,
    ];

    assert_eq!(all_bits.len(), trait_bits::COUNT as usize);

    let mut seen = std::collections::HashSet::new();
    for &bit in &all_bits {
        assert!(seen.insert(bit), "duplicate bit position: {bit}");
    }

    for &bit in &all_bits {
        assert!(bit < 64, "bit position {bit} exceeds u64 capacity");
    }
}

#[test]
fn bit_positions_are_contiguous() {
    for i in 0..trait_bits::COUNT {
        let set = TraitSet::from_bits(&[i]);
        assert!(
            set.contains(i),
            "bit {i} should be settable within COUNT range"
        );
    }
}

// ── TraitSet edge cases ─────────────────────────────────────────────

#[test]
fn contains_bit_beyond_count_is_false_for_empty() {
    assert!(!TraitSet::EMPTY.contains(63));
    assert!(!TraitSet::EMPTY.contains(32));
}

#[test]
fn high_bit_positions_work() {
    let set = TraitSet::from_bits(&[0, 31, 63]);
    assert!(set.contains(0));
    assert!(set.contains(31));
    assert!(set.contains(63));
    assert!(!set.contains(1));
    assert!(!set.contains(32));
    assert!(!set.contains(62));
}

// ── Primitive satisfaction (bitfield vs string-based equivalence) ───

/// All well-known trait names in the satisfaction system.
const ALL_TRAIT_NAMES: &[&str] = &[
    "Eq",
    "Comparable",
    "Clone",
    "Hashable",
    "Default",
    "Printable",
    "Debug",
    "Sendable",
    "Add",
    "Sub",
    "Mul",
    "Div",
    "FloorDiv",
    "Rem",
    "Neg",
    "BitAnd",
    "BitOr",
    "BitXor",
    "BitNot",
    "Shl",
    "Shr",
    "Not",
    "Len",
    "IsEmpty",
    "Iterable",
    "Iterator",
    "DoubleEndedIterator",
];

/// All primitive type Idx values with display names.
const ALL_PRIMITIVES: &[(Idx, &str)] = &[
    (Idx::INT, "int"),
    (Idx::FLOAT, "float"),
    (Idx::BOOL, "bool"),
    (Idx::STR, "str"),
    (Idx::CHAR, "char"),
    (Idx::BYTE, "byte"),
    (Idx::UNIT, "unit"),
    (Idx::NEVER, "never"),
    (Idx::ERROR, "error"),
    (Idx::DURATION, "duration"),
    (Idx::SIZE, "size"),
    (Idx::ORDERING, "ordering"),
];

/// Reference truth table: `(Idx, display_name, expected_trait_names)`.
///
/// The exact same trait lists from the old string-based `primitive_satisfies_trait()`
/// in `calls.rs`. Any difference between the bitfield result and this table is a regression.
#[rustfmt::skip]
const REFERENCE_TRUTH: &[(Idx, &str, &[&str])] = &[
    (Idx::INT, "int", &[
        "Eq", "Comparable", "Clone", "Hashable", "Default", "Printable", "Debug",
        "Add", "Sub", "Mul", "Div", "FloorDiv", "Rem", "Neg",
        "BitAnd", "BitOr", "BitXor", "BitNot", "Shl", "Shr",
    ]),
    (Idx::FLOAT, "float", &[
        "Eq", "Comparable", "Clone", "Hashable", "Default", "Printable", "Debug",
        "Add", "Sub", "Mul", "Div", "Neg",
    ]),
    (Idx::BOOL, "bool", &[
        "Eq", "Comparable", "Clone", "Hashable", "Default", "Printable", "Debug", "Not",
    ]),
    (Idx::STR, "str", &[
        "Eq", "Comparable", "Clone", "Hashable", "Default", "Printable", "Debug",
        "Len", "IsEmpty", "Add",
    ]),
    (Idx::CHAR, "char", &[
        "Eq", "Comparable", "Clone", "Hashable", "Printable", "Debug",
    ]),
    (Idx::BYTE, "byte", &[
        "Eq", "Comparable", "Clone", "Hashable", "Printable", "Debug",
        "Add", "Sub", "Mul", "Div", "Rem",
        "BitAnd", "BitOr", "BitXor", "BitNot", "Shl", "Shr",
    ]),
    (Idx::UNIT, "unit", &["Eq", "Clone", "Default", "Debug"]),
    (Idx::NEVER, "never", &[]),
    (Idx::ERROR, "error", &[]),
    (Idx::DURATION, "duration", &[
        "Eq", "Comparable", "Clone", "Hashable", "Default", "Printable", "Debug",
        "Sendable", "Add", "Sub", "Mul", "Div", "Rem", "Neg",
    ]),
    (Idx::SIZE, "size", &[
        "Eq", "Comparable", "Clone", "Hashable", "Default", "Printable", "Debug",
        "Sendable", "Add", "Sub", "Mul", "Div", "Rem",
    ]),
    (Idx::ORDERING, "ordering", &[
        "Eq", "Comparable", "Clone", "Hashable", "Printable", "Debug",
    ]),
];

/// Validate the bitfield implementation against the canonical trait sets.
#[test]
fn bitfield_matches_reference_truth() {
    let interner = StringInterner::new();
    let wk = WellKnownNames::new(&interner);

    let mut mismatches = Vec::new();
    for &(prim, prim_name, expected_traits) in REFERENCE_TRUTH {
        check_prim_traits(
            &wk,
            &interner,
            prim,
            prim_name,
            expected_traits,
            &mut mismatches,
        );
    }

    assert!(
        mismatches.is_empty(),
        "Bitfield satisfaction mismatches vs reference truth:\n{}",
        mismatches.join("\n")
    );
}

/// Check one primitive's expected and unexpected trait satisfaction.
fn check_prim_traits(
    wk: &WellKnownNames,
    interner: &StringInterner,
    prim: Idx,
    prim_name: &str,
    expected_traits: &[&str],
    mismatches: &mut Vec<String>,
) {
    for &t in expected_traits {
        let interned = interner.intern(t);
        if !wk.primitive_satisfies_trait(prim, interned) {
            mismatches.push(format!("  {prim_name} should satisfy {t} but doesn't"));
        }
    }

    let expected_set: std::collections::HashSet<&str> = expected_traits.iter().copied().collect();
    for &t in ALL_TRAIT_NAMES {
        if !expected_set.contains(t) {
            let interned = interner.intern(t);
            if wk.primitive_satisfies_trait(prim, interned) {
                mismatches.push(format!("  {prim_name} should NOT satisfy {t} but does"));
            }
        }
    }
}

// ── Specific primitive satisfaction checks ──────────────────────────

fn make_wk() -> (StringInterner, WellKnownNames) {
    let interner = StringInterner::new();
    let wk = WellKnownNames::new(&interner);
    (interner, wk)
}

#[test]
fn int_satisfies_expected_traits() {
    let (interner, wk) = make_wk();

    let yes = [
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "FloorDiv",
        "Rem",
        "Neg",
        "BitAnd",
        "BitOr",
        "BitXor",
        "BitNot",
        "Shl",
        "Shr",
    ];
    let no = ["Sendable", "Not", "Len", "IsEmpty", "Iterable", "Iterator"];

    for name in yes {
        assert!(
            wk.primitive_satisfies_trait(Idx::INT, interner.intern(name)),
            "int should satisfy {name}"
        );
    }
    for name in no {
        assert!(
            !wk.primitive_satisfies_trait(Idx::INT, interner.intern(name)),
            "int should NOT satisfy {name}"
        );
    }
}

#[test]
fn unit_satisfies_only_eq_clone_default_debug() {
    let (interner, wk) = make_wk();

    let yes = ["Eq", "Clone", "Default", "Debug"];
    let no = ["Comparable", "Hashable", "Printable", "Add", "Sendable"];

    for name in yes {
        assert!(
            wk.primitive_satisfies_trait(Idx::UNIT, interner.intern(name)),
            "unit should satisfy {name}"
        );
    }
    for name in no {
        assert!(
            !wk.primitive_satisfies_trait(Idx::UNIT, interner.intern(name)),
            "unit should NOT satisfy {name}"
        );
    }
}

#[test]
fn never_satisfies_no_traits() {
    let (interner, wk) = make_wk();
    for &name in ALL_TRAIT_NAMES {
        assert!(
            !wk.primitive_satisfies_trait(Idx::NEVER, interner.intern(name)),
            "never should NOT satisfy {name}"
        );
    }
}

#[test]
fn error_satisfies_no_traits() {
    let (interner, wk) = make_wk();
    for &name in ALL_TRAIT_NAMES {
        assert!(
            !wk.primitive_satisfies_trait(Idx::ERROR, interner.intern(name)),
            "error should NOT satisfy {name}"
        );
    }
}

#[test]
fn unknown_trait_never_satisfied() {
    let (interner, wk) = make_wk();
    let bogus = interner.intern("BogusTraitThatDoesNotExist");
    for &(prim, prim_name) in ALL_PRIMITIVES {
        assert!(
            !wk.primitive_satisfies_trait(prim, bogus),
            "{prim_name} should NOT satisfy unknown trait"
        );
    }
}

#[test]
fn non_primitive_idx_returns_false() {
    let (interner, wk) = make_wk();
    let eq = interner.intern("Eq");
    // Dynamic index far beyond primitive range
    let dynamic = Idx::from_raw(Idx::FIRST_DYNAMIC + 100);
    assert!(!wk.primitive_satisfies_trait(dynamic, eq));
}

// ── Compound type satisfaction ──────────────────────────────────────

#[test]
fn compound_type_satisfaction_via_pool() {
    use crate::Pool;

    let interner = StringInterner::new();
    let wk = WellKnownNames::new(&interner);
    let mut pool = Pool::new();

    let list_int = pool.list(Idx::INT);
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let set_int = pool.set(Idx::INT);
    let opt_int = pool.option(Idx::INT);
    let res_int_str = pool.result(Idx::INT, Idx::STR);
    let tuple = pool.tuple(&[Idx::INT, Idx::STR]);
    let range_int = pool.range(Idx::INT);
    let iter_int = pool.iterator(Idx::INT);
    let dei_int = pool.double_ended_iterator(Idx::INT);

    // List: eq, clone, hashable, printable, len, is_empty, comparable, iterable
    let list_yes = [
        "Eq",
        "Clone",
        "Hashable",
        "Printable",
        "Len",
        "IsEmpty",
        "Comparable",
        "Iterable",
    ];
    let list_no = ["Default", "Add", "Debug"];
    for name in list_yes {
        assert!(
            wk.type_satisfies_trait(list_int, interner.intern(name), &pool),
            "List<int> should satisfy {name}"
        );
    }
    for name in list_no {
        assert!(
            !wk.type_satisfies_trait(list_int, interner.intern(name), &pool),
            "List<int> should NOT satisfy {name}"
        );
    }

    // Map: eq, clone, hashable, printable, len, is_empty, iterable (no comparable)
    assert!(wk.type_satisfies_trait(map_str_int, interner.intern("Iterable"), &pool));
    assert!(!wk.type_satisfies_trait(map_str_int, interner.intern("Comparable"), &pool));

    // Set: same as Map
    assert!(wk.type_satisfies_trait(set_int, interner.intern("Len"), &pool));
    assert!(!wk.type_satisfies_trait(set_int, interner.intern("Comparable"), &pool));

    // Option: eq, comparable, clone, hashable, printable, default
    assert!(wk.type_satisfies_trait(opt_int, interner.intern("Default"), &pool));
    assert!(!wk.type_satisfies_trait(opt_int, interner.intern("Len"), &pool));

    // Result: eq, comparable, clone, hashable, printable (no default)
    assert!(wk.type_satisfies_trait(res_int_str, interner.intern("Comparable"), &pool));
    assert!(!wk.type_satisfies_trait(res_int_str, interner.intern("Default"), &pool));

    // Tuple: eq, comparable, clone, hashable, printable, len
    assert!(wk.type_satisfies_trait(tuple, interner.intern("Len"), &pool));
    assert!(!wk.type_satisfies_trait(tuple, interner.intern("Iterable"), &pool));

    // Range: printable, len, iterable
    assert!(wk.type_satisfies_trait(range_int, interner.intern("Iterable"), &pool));
    assert!(!wk.type_satisfies_trait(range_int, interner.intern("Eq"), &pool));

    // Iterator: iterator trait only
    assert!(wk.type_satisfies_trait(iter_int, interner.intern("Iterator"), &pool));
    assert!(!wk.type_satisfies_trait(iter_int, interner.intern("Eq"), &pool));

    // DoubleEndedIterator: iterator + double_ended_iterator
    assert!(wk.type_satisfies_trait(dei_int, interner.intern("Iterator"), &pool));
    assert!(wk.type_satisfies_trait(dei_int, interner.intern("DoubleEndedIterator"), &pool));
    assert!(!wk.type_satisfies_trait(dei_int, interner.intern("Eq"), &pool));
}

#[test]
fn str_compound_iterable() {
    use crate::Pool;

    let interner = StringInterner::new();
    let wk = WellKnownNames::new(&interner);
    let pool = Pool::new();

    // str primitive already satisfies many traits, but the compound check
    // adds Iterable (for iteration over characters)
    assert!(wk.type_satisfies_trait(Idx::STR, interner.intern("Iterable"), &pool));
    assert!(wk.type_satisfies_trait(Idx::STR, interner.intern("Eq"), &pool));
}

// ── trait_bit_map sync ──────────────────────────────────────────────

#[test]
fn trait_bit_map_covers_all_trait_names() {
    let (interner, wk) = make_wk();

    // Bidirectional sync: ALL_TRAIT_NAMES must list exactly one entry per trait bit.
    // Without this, a new bit added to trait_bits + build_trait_bit_map() could be
    // silently untested if ALL_TRAIT_NAMES isn't updated.
    assert_eq!(
        ALL_TRAIT_NAMES.len(),
        trait_bits::COUNT as usize,
        "ALL_TRAIT_NAMES ({}) out of sync with trait_bits::COUNT ({})",
        ALL_TRAIT_NAMES.len(),
        trait_bits::COUNT,
    );

    for &name in ALL_TRAIT_NAMES {
        let interned = interner.intern(name);
        assert!(
            wk.has_trait_bit(interned),
            "trait '{name}' should have a bit in trait_bit_map"
        );
    }
}

#[test]
fn trait_bit_map_has_no_duplicates() {
    let interner = StringInterner::new();
    let wk = WellKnownNames::new(&interner);

    let mut seen_bits = std::collections::HashSet::new();
    let mut seen_names = std::collections::HashSet::new();

    for &(name, bit) in &wk.trait_bit_map {
        assert!(
            seen_bits.insert(bit),
            "duplicate bit {bit} in trait_bit_map"
        );
        assert!(seen_names.insert(name), "duplicate Name in trait_bit_map");
    }
}
