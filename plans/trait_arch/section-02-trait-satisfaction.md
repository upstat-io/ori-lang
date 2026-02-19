---
section: "02"
title: Data-Driven Trait Satisfaction
status: not-started
goal: Replace the 200-line N×M inline boolean chain with a data-driven trait satisfaction table
sections:
  - id: "02.1"
    title: TraitSet Bitfield
    status: not-started
  - id: "02.2"
    title: Primitive Satisfaction Table
    status: not-started
  - id: "02.3"
    title: Compound Type Satisfaction Table
    status: not-started
  - id: "02.4"
    title: Migration
    status: not-started
  - id: "02.5"
    title: Completion Checklist
    status: not-started
---

# Section 02: Data-Driven Trait Satisfaction

**Status:** Not Started
**Goal:** Replace the `primitive_satisfies_trait()` and `type_satisfies_trait()` functions — currently a 200-line inline boolean matrix with N×M `||` chains — with a data-driven lookup table using bitfield trait sets. Adding a trait to a primitive type becomes one line, not 10 branches.

**Depends on:** Section 01 (uses `DerivedTrait::ALL` for initialization validation)

**Reference compilers:**
- **Zig** `src/InternPool.zig` — Type info stored as packed data; bitfield queries for capabilities
- **TypeScript** `src/compiler/types.ts` — `TypeFlags` bitflag enum; O(1) queries via `flags & TypeFlags.StringLike`
- **Rust** `compiler/rustc_middle/ty/sty.rs` — `TyKind` with pre-computed metadata; type properties queried via methods, not inline matching

**Current state:** `well_known.rs:255-379` contains `primitive_satisfies_trait()` — 10 `if-else` branches (one per primitive type: int, float, bool, str, char, byte, unit, duration, size, ordering), each with 6-19 `||` comparisons. `type_satisfies_trait()` at lines 386-440 adds 8 more branches for compound types (List, Map, Set, Option, Result, Tuple, Range, iterators). Adding a trait like `Debug` required inserting `|| t == self.debug_trait` in every branch that supports it — 10 separate edits.

---

## 02.1 TraitSet Bitfield

### Design

A bitfield where each bit represents a well-known trait. Trait satisfaction becomes a single bitwise AND.

```rust
// compiler/ori_types/src/check/well_known.rs

/// A bitfield representing a set of well-known traits.
///
/// Each bit position corresponds to a specific trait. Satisfaction
/// checks become O(1) bitwise AND operations instead of N-way
/// string/Name comparison chains.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TraitSet(u64);

impl TraitSet {
    pub const EMPTY: TraitSet = TraitSet(0);

    /// Create a TraitSet from a slice of bit positions.
    const fn from_bits(bits: &[u32]) -> Self {
        let mut val = 0u64;
        let mut i = 0;
        while i < bits.len() {
            val |= 1 << bits[i];
            i += 1;
        }
        TraitSet(val)
    }

    /// Check if this set contains the trait at the given bit position.
    #[inline]
    pub fn contains(self, bit: u32) -> bool {
        self.0 & (1 << bit) != 0
    }

    /// Union two trait sets.
    pub const fn union(self, other: TraitSet) -> TraitSet {
        TraitSet(self.0 | other.0)
    }
}
```

### Bit Assignment

Bit positions assigned to well-known traits. The assignment is internal to `well_known.rs` and not exposed to other modules.

```rust
// Bit positions for well-known traits (internal to this module)
mod trait_bits {
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
}
```

- [ ] Define `TraitSet` struct with `contains()`, `union()`, `from_bits()`
- [ ] Define `trait_bits` module with bit positions for all current well-known traits
- [ ] Unit tests: `contains()` for set/unset bits, `union()`, `from_bits()` with various combinations
- [ ] Verify `TraitSet` is `Copy` — no allocation, no indirection

---

## 02.2 Primitive Satisfaction Table

### Design

Replace the 10-branch if-else chain with a static table indexed by primitive type.

```rust
impl WellKnownNames {
    /// Pre-computed trait sets for each primitive type.
    ///
    /// Indexed by `Idx::raw()` for primitives 0..=11.
    /// Built once during `WellKnownNames::new()`.
    prim_trait_sets: [TraitSet; 12],

    /// Map from pre-interned trait Name to bit position.
    ///
    /// Used by `primitive_satisfies_trait()` to convert a Name query
    /// into a bit position for O(1) lookup.
    trait_bit_map: [(Name, u32); 27], // sorted by Name for binary search, or linear for 27 items
}
```

### Table Construction

```rust
impl WellKnownNames {
    fn build_primitive_trait_sets(&self) -> [TraitSet; 12] {
        use trait_bits::*;

        let mut sets = [TraitSet::EMPTY; 12];

        // Common trait bundles (reusable)
        let eq_clone_hash_print_debug = TraitSet::from_bits(
            &[EQ, CLONE, HASHABLE, PRINTABLE, DEBUG]
        );
        let comparable = TraitSet::from_bits(&[COMPARABLE]);
        let default = TraitSet::from_bits(&[DEFAULT]);

        // INT: eq, comparable, clone, hashable, default, printable, debug,
        //      add, sub, mul, div, floor_div, rem, neg,
        //      bit_and, bit_or, bit_xor, bit_not, shl, shr
        sets[Idx::INT.raw() as usize] = eq_clone_hash_print_debug
            .union(comparable)
            .union(default)
            .union(TraitSet::from_bits(&[
                ADD, SUB, MUL, DIV, FLOOR_DIV, REM, NEG,
                BIT_AND, BIT_OR, BIT_XOR, BIT_NOT, SHL, SHR,
            ]));

        // FLOAT: eq, comparable, clone, hashable, default, printable, debug,
        //        add, sub, mul, div, neg
        sets[Idx::FLOAT.raw() as usize] = eq_clone_hash_print_debug
            .union(comparable)
            .union(default)
            .union(TraitSet::from_bits(&[ADD, SUB, MUL, DIV, NEG]));

        // BOOL: eq, comparable, clone, hashable, default, printable, debug, not
        sets[Idx::BOOL.raw() as usize] = eq_clone_hash_print_debug
            .union(comparable)
            .union(default)
            .union(TraitSet::from_bits(&[NOT]));

        // STR: eq, comparable, clone, hashable, default, printable, debug, len, is_empty, add
        sets[Idx::STR.raw() as usize] = eq_clone_hash_print_debug
            .union(comparable)
            .union(default)
            .union(TraitSet::from_bits(&[LEN, IS_EMPTY, ADD]));

        // CHAR: eq, comparable, clone, hashable, printable, debug
        sets[Idx::CHAR.raw() as usize] = eq_clone_hash_print_debug
            .union(comparable);

        // BYTE: eq, comparable, clone, hashable, printable, debug,
        //       add, sub, mul, div, rem, bit_and, bit_or, bit_xor, bit_not, shl, shr
        sets[Idx::BYTE.raw() as usize] = eq_clone_hash_print_debug
            .union(comparable)
            .union(TraitSet::from_bits(&[
                ADD, SUB, MUL, DIV, REM,
                BIT_AND, BIT_OR, BIT_XOR, BIT_NOT, SHL, SHR,
            ]));

        // UNIT: eq, clone, default, debug
        sets[Idx::UNIT.raw() as usize] = TraitSet::from_bits(
            &[EQ, CLONE, DEFAULT, DEBUG]
        );

        // NEVER: (no traits — the never type satisfies nothing)
        // sets[Idx::NEVER.raw() as usize] = TraitSet::EMPTY;

        // DURATION: eq, comparable, clone, hashable, default, printable, debug,
        //           sendable, add, sub, mul, div, rem, neg
        sets[Idx::DURATION.raw() as usize] = eq_clone_hash_print_debug
            .union(comparable)
            .union(default)
            .union(TraitSet::from_bits(&[SENDABLE, ADD, SUB, MUL, DIV, REM, NEG]));

        // SIZE: eq, comparable, clone, hashable, default, printable, debug,
        //       sendable, add, sub, mul, div, rem
        sets[Idx::SIZE.raw() as usize] = eq_clone_hash_print_debug
            .union(comparable)
            .union(default)
            .union(TraitSet::from_bits(&[SENDABLE, ADD, SUB, MUL, DIV, REM]));

        // ORDERING: eq, comparable, clone, hashable, printable, debug
        sets[Idx::ORDERING.raw() as usize] = eq_clone_hash_print_debug
            .union(comparable);

        sets
    }
}
```

### Updated Satisfaction Check

```rust
impl WellKnownNames {
    /// Check if a primitive type inherently satisfies a trait.
    ///
    /// O(1) via pre-computed bitfield lookup. Replaces the previous
    /// N×M inline || chain.
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

    /// Convert a trait Name to its bit position.
    ///
    /// Linear scan over 27 entries — fast enough for u32 comparisons.
    /// Could be optimized to a HashMap if the trait count grows past ~50.
    #[inline]
    fn trait_bit(&self, name: Name) -> Option<u32> {
        for &(n, bit) in &self.trait_bit_map {
            if n == name {
                return Some(bit);
            }
        }
        None
    }
}
```

- [ ] Add `prim_trait_sets: [TraitSet; 12]` field to `WellKnownNames`
- [ ] Add `trait_bit_map: [(Name, u32); N]` field to `WellKnownNames`
- [ ] Implement `build_primitive_trait_sets()` initialization
- [ ] Implement `trait_bit()` lookup
- [ ] Replace `primitive_satisfies_trait()` with bitfield version
- [ ] Unit tests: every primitive type × every supported trait returns `true`
- [ ] Unit tests: every primitive type × non-supported trait returns `false`
- [ ] Unit tests: unknown traits return `false`

---

## 02.3 Compound Type Satisfaction Table

### Design

The compound type satisfaction check (`type_satisfies_trait`, lines 386-440) follows the same N×M pattern for 8 compound types. Replace with a similar table approach.

```rust
impl WellKnownNames {
    /// Check if a type satisfies a trait, including compound types.
    pub fn type_satisfies_trait(&self, ty: Idx, t: Name, pool: &Pool) -> bool {
        // Check primitives first (most common case)
        if self.primitive_satisfies_trait(ty, t) {
            return true;
        }

        // Check compound types by tag
        let set = match pool.tag(ty) {
            Tag::List => self.list_traits,
            Tag::Map | Tag::Set => self.map_set_traits,
            Tag::Option => self.option_traits,
            Tag::Result => self.result_traits,
            Tag::Tuple => self.tuple_traits,
            Tag::Range => self.range_traits,
            Tag::Str => self.str_compound_traits,
            Tag::DoubleEndedIterator => self.dei_traits,
            Tag::Iterator => self.iterator_traits,
            _ => return false,
        };

        match self.trait_bit(t) {
            Some(bit) => set.contains(bit),
            None => false,
        }
    }
}
```

The compound trait sets are pre-computed fields on `WellKnownNames`, initialized once.

- [ ] Add compound trait set fields to `WellKnownNames` (list_traits, map_set_traits, etc.)
- [ ] Initialize compound trait sets in `build_compound_trait_sets()`
- [ ] Replace `type_satisfies_trait()` with bitfield version
- [ ] Unit tests: each compound type × each supported trait
- [ ] Verify the `Tag::Str => self.str_compound_traits` handles `Iterable` (currently only compound-level check for str)

---

## 02.4 Migration

### Step-by-Step

1. **Add `TraitSet` and `trait_bits`** — new code, no changes to existing API
2. **Add fields to `WellKnownNames`** — new fields, computed in `new()`
3. **Add `build_primitive_trait_sets()` and `build_compound_trait_sets()`** — new methods
4. **Replace `primitive_satisfies_trait()` body** — same signature, new implementation
5. **Replace `type_satisfies_trait()` body** — same signature, new implementation
6. **Run `./test-all.sh`** — full suite must pass unchanged
7. **Delete old code** — remove the 200 lines of `||` chains

### Verification Strategy

Before deleting the old code, run both implementations side-by-side as a validation test:

```rust
#[cfg(test)]
fn verify_equivalence(wk: &WellKnownNames, pool: &Pool) {
    let all_primitives = [
        Idx::INT, Idx::FLOAT, Idx::BOOL, Idx::STR, Idx::CHAR,
        Idx::BYTE, Idx::UNIT, Idx::DURATION, Idx::SIZE, Idx::ORDERING,
    ];
    for &prim in &all_primitives {
        for &(name, _bit) in &wk.trait_bit_map {
            let old = wk.primitive_satisfies_trait_old(prim, name);
            let new = wk.primitive_satisfies_trait(prim, name);
            assert_eq!(old, new, "Mismatch for {:?} × {:?}", prim, name);
        }
    }
}
```

- [ ] Write equivalence test that runs both old and new implementations
- [ ] Run equivalence test — all assertions pass
- [ ] Replace old implementations
- [ ] Delete old code
- [ ] `./test-all.sh` passes

---

## 02.5 Completion Checklist

- [ ] `TraitSet` bitfield struct defined and tested
- [ ] `trait_bits` module with bit positions for all 27 well-known traits
- [ ] `prim_trait_sets` pre-computed for all 12 primitive types
- [ ] Compound type trait sets pre-computed for all 9 compound tag types
- [ ] `primitive_satisfies_trait()` uses bitfield lookup (O(1))
- [ ] `type_satisfies_trait()` uses bitfield lookup (O(1))
- [ ] Old N×M `||` chain code deleted
- [ ] Equivalence test validates correctness during migration
- [ ] Net line reduction: ~200 lines of `||` chains → ~80 lines of table construction
- [ ] `./test-all.sh` passes with zero regressions

**Exit Criteria:** Adding a new trait to a primitive type is a single line in the table builder. The `||` chains are gone. The lookup is O(1) via bitwise AND. File stays under 500 lines.
