---
section: "01"
title: "Merkle Hash Foundation"
status: not_started
goal: "Replace pool-local hashes with content-addressed Merkle hashes that are stable across independent Pool instances"
inspired_by:
  - "Git object model — content-addressed storage where SHA = identity (https://git-scm.com/book/en/v2/Git-Internals-Git-Objects)"
  - "Zig InternPool — global interning with stable indices (~/projects/reference_repos/lang_repos/zig/src/InternPool.zig)"
  - "IPFS — content-addressed file system, hash = universal address"
  - "Nix store — content-addressed package management, reproducible hashes"
sections:
  - id: "01.1"
    title: "Tag Layout Classification"
    status: not_started
  - id: "01.2"
    title: "Child Reference Map"
    status: not_started
  - id: "01.3"
    title: "Merkle Hash Function Design"
    status: not_started
  - id: "01.4"
    title: "Implementation"
    status: not_started
  - id: "01.5"
    title: "Primitive & Fixed-Index Stability"
    status: not_started
---

# Section 01: Merkle Hash Foundation

**Status:** Not Started
**Goal:** Replace pool-local hashes with content-addressed Merkle hashes that are stable across
independent Pool instances, enabling O(1) cross-module type identity.

**Why this matters:** The current `compute_hash` function hashes raw `Idx` values — sequential
integers assigned by insertion order. `List<MyStruct>` gets hash `h(LIST, 50)` in Pool A but
`h(LIST, 30)` in Pool B if `MyStruct` was interned at different positions. This makes cross-module
type comparison impossible without structural traversal, and forces full AST re-walking at every
import boundary. With generics (depth 3-5, width 4-6), capabilities, and effects, the re-interning
cost grows combinatorially with type complexity. Merkle hashing eliminates this: each type's hash
depends only on its structure, not its pool position. Same type → same hash → O(1) identity.

**This section** is the foundation for all downstream sections. Every subsequent optimization
(hash-forwarded signatures, hash-first imports, backend integration) depends on hash stability.

---

## 01.1 Tag Layout Classification — NOT STARTED

**Goal:** Document the complete data layout for all 44 Tag variants, classifying which positions
in `data` and `extra` are child Idx references (must be Merkle-hashed) vs structural data
(hashed directly).

**File:** `compiler/ori_types/src/tag/mod.rs` (Tag enum, lines 18-143)

**Complete Tag Inventory (44 variants, 6 categories):**

### Primitives (Tags 0-11) — No Children

| Tag | Value | `data` | `extra` | Children |
|-----|-------|--------|---------|----------|
| `Int` | 0 | unused (0) | none | none |
| `Float` | 1 | unused (0) | none | none |
| `Bool` | 2 | unused (0) | none | none |
| `Str` | 3 | unused (0) | none | none |
| `Char` | 4 | unused (0) | none | none |
| `Byte` | 5 | unused (0) | none | none |
| `Unit` | 6 | unused (0) | none | none |
| `Never` | 7 | unused (0) | none | none |
| `Error` | 8 | unused (0) | none | none |
| `Duration` | 9 | unused (0) | none | none |
| `Size` | 10 | unused (0) | none | none |
| `Ordering` | 11 | unused (0) | none | none |

**Merkle hash:** `hash(tag)` — no children to recurse into.

### Simple Containers (Tags 16-22) — One Child in `data`

| Tag | Value | `data` | `extra` | Children |
|-----|-------|--------|---------|----------|
| `List` | 16 | child `Idx.raw()` | none | `data` = element type |
| `Option` | 17 | child `Idx.raw()` | none | `data` = inner type |
| `Set` | 18 | child `Idx.raw()` | none | `data` = element type |
| `Channel` | 19 | child `Idx.raw()` | none | `data` = element type |
| `Range` | 20 | child `Idx.raw()` | none | `data` = element type |
| `Iterator` | 21 | child `Idx.raw()` | none | `data` = element type |
| `DoubleEndedIterator` | 22 | child `Idx.raw()` | none | `data` = element type |

**Construction:** `pool.intern(Tag::List, elem.raw())` — `construct/mod.rs:27`
**Merkle hash:** `hash(tag, self.hashes[data as usize])` — replace raw Idx with child's Merkle hash.

### Two-Child Containers (Tags 32-34) — Children in `extra`

| Tag | Value | `data` | `extra` layout | Children |
|-----|-------|--------|----------------|----------|
| `Map` | 32 | extra offset | `[key_idx, value_idx]` | `extra[0]` = key, `extra[1]` = value |
| `Result` | 33 | extra offset | `[ok_idx, err_idx]` | `extra[0]` = ok, `extra[1]` = err |
| `Borrowed` | 34 | extra offset | `[inner_idx, lifetime_id]` | `extra[0]` = inner; `extra[1]` = lifetime (structural) |

**Construction:** `pool.intern_complex(Tag::Map, &[key.raw(), value.raw()])` — `construct/mod.rs:67`
**Merkle hash:** `hash(tag, self.hashes[extra[0]], self.hashes[extra[1]])` for Map/Result.
For Borrowed: `hash(tag, self.hashes[extra[0]], extra[1])` — lifetime ID is structural, not a child type.

### Complex Types (Tags 48-51) — Variable-Length Children in `extra`

| Tag | Value | `data` | `extra` layout | Children |
|-----|-------|--------|----------------|----------|
| `Function` | 48 | extra offset | `[param_count, p0, p1, ..., ret]` | `extra[1..1+count]` = params, `extra[1+count]` = return |
| `Tuple` | 49 | extra offset | `[elem_count, e0, e1, ...]` | `extra[1..1+count]` = elements |
| `Struct` | 50 | extra offset | `[name_lo, name_hi, field_count, f0_name, f0_type, ...]` | `extra[3 + i*2 + 1]` = field types (alternating) |
| `Enum` | 51 | extra offset | `[name_lo, name_hi, var_count, v0_name, v0_fc, v0_f0, ...]` | variant field types (walk required) |

**Function Merkle hash:**
```
hash(FUNCTION, count,
     self.hashes[extra[1]], self.hashes[extra[2]], ...,  // param hashes
     self.hashes[extra[1+count]])                        // return hash
```

**Tuple Merkle hash:**
```
hash(TUPLE, count,
     self.hashes[extra[1]], self.hashes[extra[2]], ...)   // element hashes
```

**Struct Merkle hash:**
```
hash(STRUCT, name_lo, name_hi, field_count,
     extra[3+0],                       // f0 name (structural)
     self.hashes[extra[3+1]],          // f0 type (child hash)
     extra[3+2],                       // f1 name (structural)
     self.hashes[extra[3+3]],          // f1 type (child hash)
     ...)
```

**Enum Merkle hash:**
```
hash(ENUM, name_lo, name_hi, variant_count,
     v0_name, v0_field_count,
     self.hashes[v0_f0], self.hashes[v0_f1], ...,    // v0 field type hashes
     v1_name, v1_field_count,
     self.hashes[v1_f0], ...,                         // v1 field type hashes
     ...)
```

### Named Types (Tags 80-82) — Name + Optional Children in `extra`

| Tag | Value | `data` | `extra` layout | Children |
|-----|-------|--------|----------------|----------|
| `Named` | 80 | extra offset | `[name_lo, name_hi]` | none (unresolved name reference) |
| `Applied` | 81 | extra offset | `[name_lo, name_hi, arg_count, a0, a1, ...]` | `extra[3+i]` = type arguments |
| `Alias` | 82 | extra offset | (layout TBD — not yet fully implemented) | TBD |

**Applied Merkle hash:**
```
hash(APPLIED, name_lo, name_hi, arg_count,
     self.hashes[extra[3]], self.hashes[extra[4]], ...)  // arg hashes
```

**Named Merkle hash:** `hash(NAMED, name_lo, name_hi)` — no children, name is structural.

### Type Variables (Tags 96-98) — Identity in `data`, No Children

| Tag | Value | `data` | `extra` | Children |
|-----|-------|--------|---------|----------|
| `Var` | 96 | var_states index | none | none |
| `BoundVar` | 97 | var_states index | none | none |
| `RigidVar` | 98 | var_states index | none | none |

**Important:** Type variables are NOT structurally hashable in the normal sense.
A `Var` at `data=5` in Pool A and `data=3` in Pool B might represent the same
logical variable (both named `T`, same binding position). Variable identity is
determined by the VarState (Unbound { id, name }, Rigid { name }, etc.), not by
the raw `data` value.

**Merkle hash for type variables:** For cross-module signatures, type variables
appear inside Scheme types. The variable's identity should be hashed by its
*position within the scheme* (de Bruijn-style), not by its pool-local var_states
index. See Section 01.3 for the design.

### Scheme (Tag 112) — Variable IDs + Body Child

| Tag | Value | `data` | `extra` layout | Children |
|-----|-------|--------|----------------|----------|
| `Scheme` | 112 | extra offset | `[var_count, v0_id, v1_id, ..., body_idx]` | `extra[1+var_count]` = body type |

**Merkle hash:**
```
hash(SCHEME, var_count,
     extra[1], extra[2], ...,               // var IDs (structural — positional)
     self.hashes[extra[1+var_count]])        // body hash (child)
```

### Special Types (Tags 240-255) — Implementation-Specific

| Tag | Value | `data` | `extra` | Children |
|-----|-------|--------|---------|----------|
| `Projection` | 240 | TBD | TBD | TBD |
| `ModuleNs` | 241 | TBD | TBD | TBD |
| `Infer` | 254 | TBD | TBD | TBD |
| `SelfType` | 255 | TBD | TBD | TBD |

These are transient types used during type checking. They should not appear in
cross-module signatures. Merkle hash: `hash(tag, data)` — treat as opaque.
Add `debug_assert!` that these never appear in exported FunctionSig hashes.

**Exit Criteria:**
- [ ] Every Tag variant classified with data/extra layout
- [ ] Child reference positions documented for each tag
- [ ] Structural vs child-reference distinction clear for every field
- [ ] Special types verified to never appear in cross-module signatures

---

## 01.2 Child Reference Map — NOT STARTED

**Goal:** Create a compile-time-queryable classification of which data/extra positions are
child Idx references for each tag. This is the lookup table the Merkle hash function uses.

**Approach:** Add methods to `Tag` in `tag/mod.rs`:

```rust
impl Tag {
    /// Whether `data` field contains a child type Idx (vs structural data).
    ///
    /// True for simple containers (List, Option, Set, Channel, Range,
    /// Iterator, DoubleEndedIterator) where data = child Idx.
    /// False for primitives (data unused), complex types (data = extra offset),
    /// and type variables (data = var_states index).
    #[inline]
    pub const fn has_child_in_data(self) -> bool {
        matches!(
            self,
            Self::List | Self::Option | Self::Set | Self::Channel
            | Self::Range | Self::Iterator | Self::DoubleEndedIterator
        )
    }

    /// Whether this tag uses the `extra` array for child type references.
    ///
    /// True for Map, Result, Borrowed, Function, Tuple, Struct, Enum,
    /// Applied, Scheme. The layout of children within extra varies by tag —
    /// see `merkle_hash_extra()` for tag-specific traversal.
    #[inline]
    pub const fn has_children_in_extra(self) -> bool {
        matches!(
            self,
            Self::Map | Self::Result | Self::Borrowed
            | Self::Function | Self::Tuple | Self::Struct | Self::Enum
            | Self::Applied | Self::Scheme
        )
    }

    /// Whether this tag is a leaf type (no child type references).
    #[inline]
    pub const fn is_merkle_leaf(self) -> bool {
        !self.has_child_in_data() && !self.has_children_in_extra()
    }
}
```

**Files:**
- `compiler/ori_types/src/tag/mod.rs` — add methods
- `compiler/ori_types/src/tag/tests.rs` — exhaustiveness tests

**Tests:**
- Exhaustiveness test: every Tag variant appears in exactly one of the three
  categories (child-in-data, children-in-extra, leaf)
- `matches!` coverage test using `Tag::all_variants()` or similar

**Exit Criteria:**
- [ ] `has_child_in_data()` covers all simple containers
- [ ] `has_children_in_extra()` covers all complex/two-child/applied/scheme types
- [ ] `is_merkle_leaf()` covers all primitives, named, variables, special types
- [ ] Exhaustiveness test prevents silent drift when new tags are added

---

## 01.3 Merkle Hash Function Design — NOT STARTED

**Goal:** Design the Merkle hash algorithm that produces identical hashes for structurally
identical types regardless of which Pool they're interned in.

**Core Invariant:**
```
For any type T, for any two Pools P1 and P2:
  If T is interned as idx1 in P1 and idx2 in P2,
  then P1.merkle_hash(idx1) == P2.merkle_hash(idx2)
```

**Algorithm:**

```rust
/// Compute a content-addressed Merkle hash for a type.
///
/// The hash depends ONLY on the type's structural content — never on pool-local
/// Idx values. Children are referenced by their Merkle hashes (looked up in
/// `self.hashes[]`), not by their raw Idx values.
///
/// This function requires `&self` because it reads child hashes from the pool.
/// Since types are interned bottom-up (children before parents), child hashes
/// are always available when the parent is being interned.
fn merkle_hash(&self, tag: Tag, data: u32, extra_start: usize, extra_len: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();

    (tag as u8).hash(&mut h);

    if tag.has_child_in_data() {
        // data = child Idx → hash child's Merkle hash
        self.hashes[data as usize].hash(&mut h);
    } else if tag.has_children_in_extra() {
        // Tag-specific extra layout — see merkle_hash_extra()
        self.merkle_hash_extra(tag, extra_start, extra_len, &mut h);
    } else {
        // Leaf type (primitive, named, variable, special) — hash data directly
        data.hash(&mut h);
    }

    h.finish()
}
```

**Tag-Specific Extra Hashing:**

```rust
fn merkle_hash_extra(&self, tag: Tag, start: usize, len: usize, h: &mut impl Hasher) {
    let extra = &self.extra[start..start + len];
    match tag {
        // Two-child: both positions are child Idx
        Tag::Map | Tag::Result => {
            self.hashes[extra[0] as usize].hash(h);  // key/ok
            self.hashes[extra[1] as usize].hash(h);  // value/err
        }

        // Borrowed: inner is child, lifetime is structural
        Tag::Borrowed => {
            self.hashes[extra[0] as usize].hash(h);  // inner type
            extra[1].hash(h);                          // lifetime ID (structural)
        }

        // Function: [count, p0, p1, ..., ret]
        Tag::Function => {
            let count = extra[0] as usize;
            count.hash(h);
            for i in 0..count {
                self.hashes[extra[1 + i] as usize].hash(h);  // param hashes
            }
            self.hashes[extra[1 + count] as usize].hash(h);  // return hash
        }

        // Tuple: [count, e0, e1, ...]
        Tag::Tuple => {
            let count = extra[0] as usize;
            count.hash(h);
            for i in 0..count {
                self.hashes[extra[1 + i] as usize].hash(h);  // element hashes
            }
        }

        // Struct: [name_lo, name_hi, field_count, f0_name, f0_type, ...]
        Tag::Struct => {
            extra[0].hash(h);  // name_lo (structural)
            extra[1].hash(h);  // name_hi (structural)
            let field_count = extra[2] as usize;
            field_count.hash(h);
            for i in 0..field_count {
                extra[3 + i * 2].hash(h);                          // field name (structural)
                self.hashes[extra[3 + i * 2 + 1] as usize].hash(h); // field type (child hash)
            }
        }

        // Enum: [name_lo, name_hi, variant_count, v0_name, v0_fc, v0_f0, ...]
        Tag::Enum => {
            extra[0].hash(h);  // name_lo
            extra[1].hash(h);  // name_hi
            let variant_count = extra[2] as usize;
            variant_count.hash(h);
            let mut offset = 3;
            for _ in 0..variant_count {
                extra[offset].hash(h);      // variant name (structural)
                let fc = extra[offset + 1] as usize;
                fc.hash(h);                 // field count (structural)
                for j in 0..fc {
                    self.hashes[extra[offset + 2 + j] as usize].hash(h); // field type (child hash)
                }
                offset += 2 + fc;
            }
        }

        // Applied: [name_lo, name_hi, arg_count, a0, a1, ...]
        Tag::Applied => {
            extra[0].hash(h);  // name_lo (structural)
            extra[1].hash(h);  // name_hi (structural)
            let arg_count = extra[2] as usize;
            arg_count.hash(h);
            for i in 0..arg_count {
                self.hashes[extra[3 + i] as usize].hash(h);  // arg hashes
            }
        }

        // Scheme: [var_count, v0_id, v1_id, ..., body_idx]
        Tag::Scheme => {
            let var_count = extra[0] as usize;
            var_count.hash(h);
            // Var IDs are positional (de Bruijn-like) — hash as structural
            for i in 0..var_count {
                extra[1 + i].hash(h);
            }
            // Body is a child type
            self.hashes[extra[1 + var_count] as usize].hash(h);
        }

        _ => unreachable!("has_children_in_extra() returned true for {:?}", tag),
    }
}
```

**Design Decisions:**

1. **FxHasher, not SipHash/xxHash:** Matches current implementation. FxHash is ~3x faster
   than SipHash for small inputs. Collision resistance is irrelevant (adversarial input
   impossible — only compiler-generated types). Birthday-paradox collision probability
   for 10K types with 64-bit FxHash: ~5.4 × 10⁻¹², effectively zero.

2. **Type variables hash by var_states index:** Within a single pool, type variables
   are identified by their var_states index. For cross-module transport, variables appear
   inside Scheme types where their position serves as identity (first var, second var, etc.).
   The Scheme's var IDs are hashed as structural data (positional), and the body hash
   uses the var's Merkle hash (which includes its var_states index). This is correct
   because: (a) within a pool, same var_states index = same variable; (b) across pools,
   variables only appear in Schemes where positional identity matches.

3. **Names are structural:** `ori_ir::Name` is a globally interned string. The raw `u32`
   value of a Name is the same across all pools (Names are interned in a global interner,
   not per-pool). So `name_lo/name_hi` can be hashed directly.

4. **`&self` requirement:** Unlike the current static `compute_hash`, Merkle hashing
   requires `&self` to look up child hashes. This means `intern()` and `intern_complex()`
   must call a method on `&self` before inserting. Since Rust's borrow checker prevents
   `&self` while `&mut self` is held, the hash must be computed from the raw data before
   mutating the pool. The current code already does this — `compute_hash` is called before
   `self.items.push()`.

   **Key insight:** At the point of interning, all children are ALREADY interned (types
   are built bottom-up). So `self.hashes[child_idx]` is always valid. The hash is computed
   from immutable data (`self.hashes` is only appended to, never modified), then the new
   item is pushed.

**Exit Criteria:**
- [ ] Merkle hash algorithm handles all 44 tag variants
- [ ] Child Idx positions correctly identified for every tag
- [ ] Structural data positions correctly identified for every tag
- [ ] Type variable hashing design documented and justified
- [ ] `&self` borrow safety verified (no aliasing during interning)
- [ ] FxHash collision bound acceptable for target type counts

---

## 01.4 Implementation — NOT STARTED

**Goal:** Replace `compute_hash` with `merkle_hash` in `pool/mod.rs`, updating `intern()`
and `intern_complex()` to use the new function.

**Files to modify:**
- `compiler/ori_types/src/pool/mod.rs` — replace hash computation
- `compiler/ori_types/src/tag/mod.rs` — add classification methods

**Current code** (`pool/mod.rs:317-326`):
```rust
fn compute_hash(tag: Tag, data: u32, extra: &[u32]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    (tag as u8).hash(&mut hasher);
    data.hash(&mut hasher);        // ← raw Idx value for containers!
    extra.hash(&mut hasher);       // ← raw Idx values for complex types!
    hasher.finish()
}
```

**New code:**
```rust
/// Compute a content-addressed Merkle hash for interning.
///
/// Unlike the previous `compute_hash`, this hashes child types by their
/// Merkle hashes (from `self.hashes[]`), not by their raw Idx values.
/// This makes the hash stable across independent Pool instances:
/// the same type structure always produces the same hash.
fn merkle_hash(&self, tag: Tag, data: u32, extra: &[u32]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();

    (tag as u8).hash(&mut h);

    if tag.has_child_in_data() {
        self.hashes[data as usize].hash(&mut h);
    } else if !tag.has_children_in_extra() {
        // Leaf: hash data directly (primitive tag, var ID, etc.)
        data.hash(&mut h);
    }
    // Note: for has_children_in_extra(), data is the extra array offset —
    // it's pool-local and must NOT be hashed. The extra contents are hashed below.

    if tag.has_children_in_extra() {
        self.merkle_hash_extra(tag, extra, &mut h);
    }

    h.finish()
}
```

**Changes to `intern()` (`pool/mod.rs:262-281`):**

```rust
pub fn intern(&mut self, tag: Tag, data: u32) -> Idx {
    // Compute Merkle hash (reads self.hashes for child lookup)
    let hash = self.merkle_hash(tag, data, &[]);

    // Check for existing (unchanged)
    if let Some(&idx) = self.intern_map.get(&hash) {
        return idx;
    }

    // Create new (unchanged)
    let idx = Idx::from_raw(self.items.len() as u32);
    let item = Item::new(tag, data);
    let flags = self.compute_flags(tag, data, &[]);

    self.items.push(item);
    self.flags.push(flags);
    self.hashes.push(hash);
    self.intern_map.insert(hash, idx);

    idx
}
```

**Changes to `intern_complex()` (`pool/mod.rs:291-314`):**

```rust
pub fn intern_complex(&mut self, tag: Tag, extra_data: &[u32]) -> Idx {
    // Compute Merkle hash BEFORE mutating extra array
    let hash = self.merkle_hash(tag, 0, extra_data);

    // Check for existing (unchanged)
    if let Some(&idx) = self.intern_map.get(&hash) {
        return idx;
    }

    // Allocate in extra array (unchanged)
    let extra_idx = self.extra.len() as u32;
    self.extra.extend_from_slice(extra_data);

    // Create new item (unchanged)
    let idx = Idx::from_raw(self.items.len() as u32);
    let item = Item::with_extra(tag, extra_idx);
    let flags = self.compute_flags(tag, extra_idx, extra_data);

    self.items.push(item);
    self.flags.push(flags);
    self.hashes.push(hash);
    self.intern_map.insert(hash, idx);

    idx
}
```

**Critical subtlety for `intern_complex`:** The `extra_data` parameter contains the CALLER's
raw values (including Idx raw values). `merkle_hash()` receives this slice and must interpret
the Idx positions as child references (looking up `self.hashes[extra[i]]`). But for `intern()`,
the `data` parameter is the raw value — `merkle_hash` looks up `self.hashes[data]`.

The callers (`construct/mod.rs`) pass `elem.raw()`, `key.raw()`, etc. — so the positions
in the extra array DO contain `Idx.raw()` values. `merkle_hash_extra()` correctly interprets
these as indices into `self.hashes[]`.

**Also update `compute_primitive_hash()`** (`pool/mod.rs:190-195`):

```rust
fn compute_primitive_hash(tag: Tag) -> u64 {
    // Primitives have no children — hash tag only.
    // This is already Merkle-correct (no Idx values involved).
    // Keep as-is for clarity.
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    (tag as u8).hash(&mut hasher);
    hasher.finish()
}
```

No change needed — primitive hashes are already stable (tag-only, no children).

**Also update `find_tuple()`** (`pool/mod.rs:569-583`):

```rust
pub fn find_tuple(&self, elems: &[Idx]) -> Option<Idx> {
    if elems.is_empty() {
        return Some(Idx::UNIT);
    }
    // Must compute Merkle hash to look up in intern_map
    let mut extra = Vec::with_capacity(elems.len() + 1);
    extra.push(elems.len() as u32);
    for &e in elems {
        extra.push(e.raw());
    }
    let hash = self.merkle_hash(Tag::Tuple, 0, &extra);
    self.intern_map.get(&hash).copied()
}
```

**Exit Criteria:**
- [ ] `compute_hash` replaced with `merkle_hash` in all call sites
- [ ] `intern()` uses `merkle_hash` for simple types
- [ ] `intern_complex()` uses `merkle_hash` for complex types
- [ ] `find_tuple()` uses `merkle_hash` for lookup
- [ ] `compute_primitive_hash()` verified unchanged (already stable)
- [ ] All existing tests pass (hash values change but semantics preserved)
- [ ] `cargo c` succeeds for `ori_types`

---

## 01.5 Primitive & Fixed-Index Stability — NOT STARTED

**Goal:** Verify that primitives at fixed indices (0-11) produce the same Merkle hash as before,
and that types composed only of primitives (`List<int>`, `(int, str) -> bool`) produce identical
hashes across pools.

**Why:** Primitives are at fixed positions (Idx 0-11) in every pool, interned during
`intern_primitives()` (`pool/mod.rs:116-149`). Their hashes should be identical to the current
`compute_hash` output because: (a) primitives hash only the tag (no children); (b) simple
containers of primitives hash the child's Merkle hash, which equals the child's current hash
(since primitives have no children, their Merkle hash = current hash).

**Verification steps:**
1. Build two fresh Pools
2. Intern `INT`, `FLOAT`, `BOOL`, etc. in both → verify identical hashes
3. Intern `List<INT>`, `Option<BOOL>`, `Map<STR, INT>` in both → verify identical hashes
4. Intern `(INT, STR) -> BOOL` in both → verify identical hashes
5. Intern `List<List<INT>>` (depth 2) → verify identical hashes

**Test file:** `compiler/ori_types/src/pool/tests.rs`

```rust
#[test]
fn merkle_hash_primitives_stable_across_pools() {
    let p1 = Pool::new();
    let p2 = Pool::new();

    // Primitives at fixed indices should have identical hashes
    for idx in [Idx::INT, Idx::FLOAT, Idx::BOOL, Idx::STR, Idx::CHAR,
                Idx::BYTE, Idx::UNIT, Idx::NEVER, Idx::ERROR,
                Idx::DURATION, Idx::SIZE, Idx::ORDERING] {
        assert_eq!(p1.hash(idx), p2.hash(idx),
            "Primitive {:?} hash differs across pools", p1.tag(idx));
    }
}

#[test]
fn merkle_hash_containers_stable_across_pools() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Intern some unrelated types in p2 first to shift indices
    let _ = p2.list(Idx::FLOAT);   // This pushes p2's next index further
    let _ = p2.option(Idx::BOOL);

    // Now intern the same type in both pools
    let list_int_1 = p1.list(Idx::INT);
    let list_int_2 = p2.list(Idx::INT);

    // Indices will differ (p2 has more items), but hashes must match
    assert_ne!(list_int_1, list_int_2, "Indices should differ");
    assert_eq!(p1.hash(list_int_1), p2.hash(list_int_2),
        "List<int> hash must be stable across pools");
}
```

**Edge cases to test:**
- Same type interned in different order → same hash
- Nested containers (`List<List<int>>`) → same hash across pools
- Function types with multiple params → same hash across pools
- Struct types with same name and fields → same hash across pools
- Scheme types with same variable structure → same hash across pools

**Exit Criteria:**
- [ ] All primitive hashes identical across fresh pools
- [ ] Simple container hashes stable despite different interning order
- [ ] Complex type hashes stable despite different Idx assignments
- [ ] At least 10 cross-pool stability tests covering all tag categories
- [ ] Tests in `pool/tests.rs` with clear assertions

---

## Section 01 Completion Checklist

- [ ] All 44 Tag variants classified (01.1)
- [ ] `has_child_in_data()`, `has_children_in_extra()`, `is_merkle_leaf()` added to Tag (01.2)
- [ ] Exhaustiveness test for tag classification (01.2)
- [ ] `merkle_hash()` and `merkle_hash_extra()` implemented in Pool (01.3, 01.4)
- [ ] `intern()`, `intern_complex()`, `find_tuple()` updated (01.4)
- [ ] `compute_hash()` removed or deprecated (01.4)
- [ ] Cross-pool hash stability tests passing (01.5)
- [ ] All existing `cargo t -p ori_types` tests pass
- [ ] `cargo c` succeeds for all crates in workspace
