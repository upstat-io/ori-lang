---
section: "02"
title: "Hash Stability Testing"
status: complete
goal: "Comprehensive test suite proving Merkle hashes are stable across independent pools and free of practical collisions"
inspired_by:
  - "Git test suite — content-addressed object identity verification"
  - "Zig InternPool tests — index stability across compilation units"
sections:
  - id: "02.1"
    title: "Cross-Pool Stability Tests"
    status: complete
  - id: "02.2"
    title: "Collision Detection & Distribution"
    status: complete
  - id: "02.3"
    title: "Structural Equality Verification"
    status: complete
  - id: "02.4"
    title: "Debug Tooling"
    status: complete
---

# Section 02: Hash Stability Testing

**Status:** Not Started
**Goal:** Build a comprehensive test suite that proves the core invariant: structurally identical
types produce identical Merkle hashes across independent Pool instances, regardless of interning
order, pool size, or Idx assignment.

**Why this matters:** Merkle hashing is the foundation for all downstream optimizations. If
hash stability has even one edge case failure, every consumer (hash-forwarded signatures,
hash-first imports, backend integration) silently produces wrong results. These bugs are
catastrophic and near-impossible to diagnose without explicit stability tests. This section
must be complete and passing before any downstream section begins.

**This section** depends on Section 01 (Merkle hash implementation). It blocks Sections 03-07.

---

## 02.1 Cross-Pool Stability Tests — COMPLETE

**Goal:** For every tag category, verify that the same type structure produces the same Merkle
hash across two independently constructed pools with different interning histories.

**Test Strategy:** For each test, create two fresh pools. Intern different "noise" types in
Pool B first (to shift Idx assignments), then intern the target type in both pools and verify
hash equality.

**File:** `compiler/ori_types/src/pool/tests.rs`

**Test Matrix:**

| Test Name | Type | Depth | Why |
|-----------|------|-------|-----|
| `merkle_primitives` | `int`, `str`, `bool`, all 12 | 0 | Baseline — fixed indices |
| `merkle_simple_container` | `List<int>`, `Option<str>`, `Set<bool>` | 1 | Child-in-data path |
| `merkle_simple_container_shifted` | `List<int>` with noise types | 1 | Different Idx, same hash |
| `merkle_two_child` | `Map<str, int>`, `Result<bool, str>` | 1 | Two-child extra path |
| `merkle_function_nullary` | `() -> int` | 1 | Function with 0 params |
| `merkle_function_unary` | `(int) -> str` | 1 | Function with 1 param |
| `merkle_function_multi` | `(int, str, bool) -> float` | 1 | Function with N params |
| `merkle_tuple_pair` | `(int, str)` | 1 | Tuple with 2 elements |
| `merkle_tuple_triple` | `(int, str, bool)` | 1 | Tuple with 3 elements |
| `merkle_nested_list` | `List<List<int>>` | 2 | Depth 2, same path |
| `merkle_nested_map` | `Map<str, List<Option<int>>>` | 3 | Depth 3, mixed containers |
| `merkle_function_complex` | `(Map<str, List<int>>) -> Option<bool>` | 3 | Complex function sig |
| `merkle_struct` | `struct Foo { x: int, y: str }` | 1 | Struct with fields |
| `merkle_struct_nested` | `struct Bar { inner: List<int> }` | 2 | Struct referencing container |
| `merkle_enum_unit` | `enum Dir { N, S, E, W }` | 0 | Enum with unit variants |
| `merkle_enum_fields` | `enum Result { Ok(int), Err(str) }` | 1 | Enum with field variants |
| `merkle_applied` | `Foo<int, str>` | 1 | Applied generic type |
| `merkle_scheme` | `forall T. (T) -> T` | 1 | Quantified type scheme |
| `merkle_depth_4` | `List<Map<str, Option<List<int>>>>` | 4 | Realistic generic depth |
| `merkle_depth_5` | Function returning depth-4 | 5 | Max expected depth |

**Test template:**

```rust
#[test]
fn merkle_simple_container_shifted() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2's indices by interning unrelated types
    let _ = p2.list(Idx::FLOAT);
    let _ = p2.map(Idx::STR, Idx::BOOL);
    let _ = p2.option(Idx::CHAR);
    let _ = p2.function(&[Idx::INT], Idx::STR);

    // Target type: List<int>
    let idx1 = p1.list(Idx::INT);
    let idx2 = p2.list(Idx::INT);

    assert_ne!(idx1, idx2, "Indices must differ (p2 shifted)");
    assert_eq!(p1.hash(idx1), p2.hash(idx2),
        "List<int> Merkle hash must be identical across pools");
}
```

**Non-primitive child test (the critical case):**

```rust
#[test]
fn merkle_container_of_struct_shifted() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2 heavily
    for i in 0..20 {
        let _ = p2.list(Idx::from_raw(i % 12));  // 20 noise entries
    }

    // Create struct in both pools — will get DIFFERENT Idx values
    let name = Name::from("Point");
    let fields_1 = vec![
        (Name::from("x"), Idx::INT),
        (Name::from("y"), Idx::INT),
    ];
    let fields_2 = fields_1.clone();

    let struct_1 = p1.struct_type(name, &fields_1);
    let struct_2 = p2.struct_type(name, &fields_2);

    // Struct Idx values differ
    assert_ne!(struct_1, struct_2);
    // Struct hashes must be identical (same name, same field structure)
    assert_eq!(p1.hash(struct_1), p2.hash(struct_2));

    // NOW: List<Point> — the critical test
    let list_1 = p1.list(struct_1);
    let list_2 = p2.list(struct_2);

    // List Idx values differ
    assert_ne!(list_1, list_2);
    // List hashes must be identical (List + child hash of Point)
    assert_eq!(p1.hash(list_1), p2.hash(list_2),
        "List<Point> hash must be stable: child struct at different Idx values");
}
```

**This is the test that the CURRENT hash function fails.** It's the litmus test for
Merkle correctness: a container of a non-primitive type at different Idx positions.

**Exit Criteria:**
- [x] All 20+ test cases in the matrix passing — 22 Merkle tests (2026-02-26)
- [x] At least one test per tag category (primitive, simple, two-child, complex, named, variable, scheme) (2026-02-26)
- [x] At least one "shifted" test per category (noise types to move Idx values) (2026-02-26)
- [x] Non-primitive child test (`List<UserStruct>`) passing — `merkle_container_of_struct_shifted` (2026-02-26)
- [x] Depth 4-5 test passing — `merkle_depth_4_stable` + `merkle_depth_5_function_stable` (2026-02-26)

---

## 02.2 Collision Detection & Distribution — COMPLETE

**Goal:** Verify that Merkle hashes have no practical collisions for realistic type sets,
and that the hash distribution is uniform enough for `FxHashMap` performance.

**Approach:** Generate a large set of distinct types (1,000-10,000) in a single pool,
collect all hashes, and verify:
1. No duplicate hashes for distinct types
2. Hash distribution is roughly uniform (no clustering)

**Test:**

```rust
#[test]
fn merkle_no_collisions_realistic_types() {
    let mut pool = Pool::new();
    let mut hashes = FxHashSet::default();
    let mut count = 0;

    // Generate ~2000 distinct types
    let primitives = [Idx::INT, Idx::FLOAT, Idx::BOOL, Idx::STR,
                      Idx::CHAR, Idx::BYTE, Idx::UNIT];

    // Level 1: containers of primitives (7 × 7 = 49)
    let mut level1 = Vec::new();
    for &p in &primitives {
        level1.push(pool.list(p));
        level1.push(pool.option(p));
        level1.push(pool.set(p));
        level1.push(pool.iterator(p));
        level1.push(pool.range(p));
        // etc.
    }

    // Level 2: containers of level1 types
    // Level 3: functions, tuples, maps of level2

    // ... (generate ~2000 types)

    // Collect all hashes
    for idx in 0..pool.len() {
        let h = pool.hash(Idx::from_raw(idx as u32));
        let is_new = hashes.insert(h);
        assert!(is_new, "Hash collision at index {idx}: hash 0x{h:016x}");
        count += 1;
    }

    // Verify we generated enough types
    assert!(count >= 500, "Need at least 500 distinct types for meaningful collision test");
}
```

**Distribution test (optional, for performance confidence):**

```rust
#[test]
fn merkle_hash_distribution_uniform() {
    let mut pool = Pool::new();
    // ... generate types ...

    // Check that top/bottom bits have reasonable entropy
    let mut top_byte_counts = [0u32; 256];
    for idx in 0..pool.len() {
        let h = pool.hash(Idx::from_raw(idx as u32));
        top_byte_counts[(h >> 56) as usize] += 1;
    }

    // Chi-squared test or simpler: no bucket has >5x the average
    let avg = pool.len() as f64 / 256.0;
    for &count in &top_byte_counts {
        assert!((count as f64) < avg * 5.0,
            "Hash distribution skewed: bucket has {count} entries (avg {avg:.1})");
    }
}
```

**Exit Criteria:**
- [x] No collisions in 500+ distinct types — `merkle_no_collisions_500_plus_types` (578 types) (2026-02-26)
- [x] Hash distribution test passes (no extreme clustering) — `merkle_hash_distribution_uniform` (2026-02-26)
- [x] Test runs in <100ms (not a bottleneck) — runs in <10ms (2026-02-26)

---

## 02.3 Structural Equality Verification — COMPLETE

**Goal:** Verify that hash equality implies structural equality (no false positives)
and that structural equality implies hash equality (no false negatives), by cross-checking
Merkle hashes against a reference structural comparison.

**Approach:** Implement a `structural_eq(p1: &Pool, idx1: Idx, p2: &Pool, idx2: Idx) -> bool`
function that recursively compares types by structure (tag + children), ignoring Idx values.
Use it to verify that `hash(idx1) == hash(idx2)` ↔ `structural_eq(idx1, idx2)` for a large
type set.

```rust
/// Reference implementation: structural equality across pools.
/// Used ONLY for testing — not for production (O(tree_depth) per comparison).
#[cfg(test)]
fn structural_eq(p1: &Pool, idx1: Idx, p2: &Pool, idx2: Idx) -> bool {
    let tag1 = p1.tag(idx1);
    let tag2 = p2.tag(idx2);
    if tag1 != tag2 {
        return false;
    }

    match tag1 {
        // Primitives: tag equality is sufficient
        tag if tag.is_merkle_leaf() && tag.is_primitive() => true,

        // Simple containers: compare child
        tag if tag.has_child_in_data() => {
            let child1 = Idx::from_raw(p1.data(idx1));
            let child2 = Idx::from_raw(p2.data(idx2));
            structural_eq(p1, child1, p2, child2)
        }

        // ... (recursive cases for all tags)
        _ => todo!()
    }
}
```

**Test:**

```rust
#[test]
fn merkle_hash_matches_structural_equality() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Create shifted pools with ~100 types each
    // For each type in p1, create the same type in p2
    // Verify: hash equality ↔ structural equality

    let types_1: Vec<Idx> = /* ... generate types in p1 ... */;
    let types_2: Vec<Idx> = /* ... same types in p2, different order ... */;

    for (&idx1, &idx2) in types_1.iter().zip(&types_2) {
        let hash_eq = p1.hash(idx1) == p2.hash(idx2);
        let struct_eq = structural_eq(&p1, idx1, &p2, idx2);
        assert_eq!(hash_eq, struct_eq,
            "Hash/structural mismatch for {:?} vs {:?}",
            p1.format(idx1), p2.format(idx2));
    }
}
```

**Exit Criteria:**
- [x] `structural_eq` reference implementation covering all tag categories — exhaustive match on all 37 Tag variants (2026-02-26)
- [x] 100+ types verified: hash equality ↔ structural equality — `merkle_hash_matches_structural_equality` (170+ pairs) (2026-02-26)
- [x] No false positives (different types with same hash) — `merkle_structural_neq_implies_hash_neq` (2026-02-26)
- [x] No false negatives (same type with different hash) — verified in `merkle_hash_matches_structural_equality` (2026-02-26)

---

## 02.4 Debug Tooling — COMPLETE

**Goal:** Add diagnostic helpers for debugging hash mismatches during development.

**Tools:**

1. **`Pool::format_hash(idx: Idx) -> String`** — Returns a human-readable hash breakdown:
   ```
   List<int> @ Idx(15): hash=0x1a2b3c4d5e6f7890
     tag=List, child_hash=0x0000000000000001 (INT)
   ```

2. **`Pool::debug_hash_tree(idx: Idx) -> String`** — Recursive hash tree:
   ```
   (int, str) -> bool @ hash=0xABCD...
     Function(2 params, 1 ret)
       param[0]: int @ hash=0x0001 (primitive)
       param[1]: str @ hash=0x0003 (primitive)
       return:   bool @ hash=0x0002 (primitive)
   ```

3. **`debug_assert!` in hash-forwarded operations** — When a hash lookup succeeds,
   optionally verify the found type structurally matches (cfg(debug_assertions) only).

**Files:**
- `compiler/ori_types/src/pool/format/mod.rs` — add hash formatting methods
- `compiler/ori_types/src/pool/mod.rs` — add debug assertions

**Exit Criteria:**
- [x] `format_hash` prints tag + hash + child hashes for any type (2026-02-26)
- [x] `debug_hash_tree` prints recursive breakdown (2026-02-26)
- [x] Debug assertions in hash lookups (debug builds only) — `debug_assert_eq!` in `intern()` and `intern_complex()` (2026-02-26)
- [x] Tooling used in at least one failing test to demonstrate diagnostic value — `merkle_container_of_struct_shifted` uses `format_hash` in assertions (2026-02-26)

---

## Section 02 Completion Checklist

- [x] 20+ cross-pool stability tests covering all tag categories (02.1) — 22 Merkle stability tests (2026-02-26)
- [x] Non-primitive child test (container of user type) passing (02.1) — `merkle_container_of_struct_shifted` (2026-02-26)
- [x] Collision detection with 500+ types, zero collisions (02.2) — 578 types, 0 collisions (2026-02-26)
- [x] Structural equality cross-check for 100+ types (02.3) — 170+ pairs verified (2026-02-26)
- [x] Debug tooling (`format_hash`, `debug_hash_tree`) implemented (02.4) (2026-02-26)
- [x] All tests in `pool/tests.rs` passing — 29 pool tests, 15 format tests, 10,614 total (2026-02-26)
- [x] Section 02 blocking gate: ALL tests must pass before Sections 03-07 (2026-02-26)
