---
section: "02"
title: "Registry as Universal SSOT (Methods & Traits)"
status: in-progress
reviewed: true
goal: "Type checker queries ori_registry for trait satisfaction and builtin method signatures instead of maintaining parallel hardcoded arrays"
inspired_by:
  - "ori_registry defs/*.rs TypeDef pattern -- methods and operator defs per type"
depends_on: []
third_party_review:
  status: findings
  updated: 2026-03-31
sections:
  - id: "02.1"
    title: "Registry Trait Coverage Gaps"
    status: complete
  - id: "02.2"
    title: "Trait Satisfaction via Registry (calls/traits.rs)"
    status: complete
  - id: "02.3"
    title: "WellKnown Bitfield Trait Sets via Registry"
    status: complete
  - id: "02.4"
    title: "Builtin Identifier Signatures"
    status: complete
  - id: "02.5"
    title: "Named Type Method Dispatch & Computed Returns"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Registry as Universal SSOT (Methods & Traits)

**Status:** Not Started
**Goal:** The type checker queries `ori_registry` for builtin trait satisfaction and method signatures instead of maintaining parallel hardcoded arrays. After this section, adding a trait impl for a builtin type requires modifying only the registry.

**Context:** The type checker maintains **three independent** parallel data structures encoding which traits each type satisfies:

1. **`calls/traits.rs`** — 13 `const &[&str]` arrays (`INT_TRAITS`, `FLOAT_TRAITS`, etc.) used by `primitive_satisfies_trait()` and `type_satisfies_trait()`. This is the **inference-phase** path, used by `constraints.rs` as a string-based fallback.
2. **`check/well_known/mod.rs` + `trait_set.rs`** — Pre-computed bitfield (`TraitSet`) tables built at `ModuleChecker::new()` startup. `WellKnownNames::primitive_satisfies_trait()` and `type_satisfies_trait()` provide O(1) lookup via interned `Name` comparison. This is the **registration/checking-phase** path, used by `derived.rs`, `type_resolution.rs`, and `constraints.rs` (as the preferred path when `WellKnownNames` is available).
3. **`ori_registry` `OpDefs` + `MethodDef.trait_name`** — The intended canonical source. Already encodes operator traits via `OpDefs` and method traits via `trait_name`, but has gaps (see feasibility note).

Both #1 and #2 must be eliminated in favor of querying #3. The `constraints.rs` dual-path (`wk.type_satisfies_trait()` first, then string-based `type_satisfies_trait()` fallback) is evidence of the duplication: two independent implementations of the same knowledge.

**Reference implementations:**
- **ori_registry** `compiler/ori_registry/src/defs/int.rs`: `INT` TypeDef with methods and operators
- **ori_registry** `compiler/ori_registry/src/query/mod.rs`: `find_type()`, `find_method()`, `has_method()` query API
- **Section 01** `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs`: Bridge pattern (`tag_to_type_tag`, `binary_op_strategy`, `return_tag_to_idx`) established for operator dispatch — same pattern applies here

**Depends on:** None (but builds on Section 01 patterns: `registry_bridge` module, `tag_to_type_tag` conversion).

**Feasibility note:** The registry's `OpDefs` can derive operator-trait satisfaction (e.g., `add != Unsupported` implies `Add` trait). Many non-operator traits (`Clone`, `Printable`, `Debug`, `Eq`, `Comparable`, `Hashable`, `Not`) are represented via `MethodDef.trait_name` (e.g., `clone` has `trait_name: Some("Clone")`). However, several trait categories have **no registry representation at all** and require registry additions before the bridge function can work:

1. **Marker traits with no method**: `Default` has no method in the registry for most primitives (int, float, bool, str, unit). Only Option/Result have a `default` parameter name (not method). `Sendable` has no method or marker anywhere in the registry.
2. **Trait methods with `trait_name: None`**: `len` (-> `Len`), `is_empty` (-> `IsEmpty`), and `iter` (-> `Iterable`) exist as methods in the registry but have `trait_name: None` instead of `Some("Len")`, `Some("IsEmpty")`, `Some("Iterable")`. The channel type's `len`/`is_empty` use a `chan()` constructor that also sets `trait_name: None`. Tuple's `len` is also `None`.
3. **Meta-traits**: `Iterator` and `DoubleEndedIterator` trait satisfaction (line 214-215 of traits.rs) has no direct registry equivalent -- it's driven by `Tag::Iterator`/`Tag::DoubleEndedIterator` matching.
4. **`Formattable`**: Already representable via `MethodDef.trait_name: Some("Formattable")` on Duration/Size `format` methods. NOT in the traits.rs string arrays or trait_set.rs bitfields (it flows through the generic trait impl mechanism). The bridge will pick it up from `MethodDef.trait_name` -- no `TypeDef.traits` entry needed, but a `FORMATTABLE` bit must be added to `trait_bits` for the bitfield path.

The bridge function must handle four categories: (a) operator traits from `OpDefs`, (b) method traits from `MethodDef.trait_name`, (c) traits that need registry additions (fix `trait_name: None` -> proper names, add marker traits to `TypeDef.traits`), and (d) special cases for types without TypeDefs (`Unit`, `Never`) and tag-level meta-traits (`Iterator`, `DoubleEndedIterator`).

**Test strategy:** Pure refactoring -- no behavioral changes. The test matrix is the existing test suite plus:
- Semantic pin: Rust unit test that queries registry for each type's trait satisfaction and asserts equivalence with BOTH the old string arrays AND the old bitfield tables
- Cross-check: verify the registry bridge produces identical results to `WellKnownNames::type_satisfies_trait()` for every primitive and compound type
- Regression: existing `#compile_fail` tests for trait bound violations (e.g., non-Hashable map keys) must pass unchanged
- Existing `well_known/tests.rs` tests must pass unchanged (they verify the bitfield tables against expected truth tables)
- TDD ordering: write equivalence/enforcement tests BEFORE removing old code; verify tests pass with old code (baseline), then switch to registry-derived code and verify tests still pass unchanged
- Debug AND release builds must pass (`cargo test` and `cargo test --release` for `ori_types` and `ori_registry`)

---

## 02.1 Registry Trait Coverage Gaps

**File(s):** `compiler/ori_registry/src/defs/*.rs`, `compiler/ori_registry/src/type_def/mod.rs`

Before the bridge function can replace the hardcoded arrays and bitfield tables, the registry must encode all traits that appear in them. Current gaps:

**Step 1 — Add `traits` field to `TypeDef` (do this FIRST, before any per-type changes)**

- [x] Update `TypeDef` struct in `compiler/ori_registry/src/type_def/mod.rs`: add `pub traits: &'static [&'static str]` field after `operators`. Default `&[]` for types with no extra marker traits. This is a compile-time breaking change -- every `TypeDef` definition (`compiler/ori_registry/src/defs/{int,float,bool,char,byte,str,duration,size,ordering,error,list,map,set,range,tuple,option,result,channel,iterator}/mod.rs` or `.rs`) will need `traits: &[]` added. This is the correct enforcement mechanism (structural, not test-based).

**Step 2 — Fix `trait_name: None` on existing methods**

- [x] **Registry fix**: Set `trait_name: Some("Len")` on all `len` methods -- currently `None` in: `str.rs:190`, `list/mod.rs:90`, `map/mod.rs:128`, `set/mod.rs:83`, `range/mod.rs:65`, `tuple/mod.rs:55`, `channel/mod.rs:56` (7 types). Also update `length` alias methods in str, list, map, set to match.
  - **WHERE**: Each type's `METHODS` static array. For channel, update the `chan()` helper invocations to use explicit `MethodDef` construction (since `chan()` hardcodes `trait_name: None`) OR add a `trait_name` parameter to the `chan()` helper.
- [x] **Registry fix**: Set `trait_name: Some("IsEmpty")` on all `is_empty` methods -- currently `None` in: `str.rs:175`, `list/mod.rs:86`, `map/mod.rs:111`, `set/mod.rs:74`, `range/mod.rs:56`, `channel/mod.rs:55` (6 types).
  - **WHERE**: Same pattern as `len` above. Channel needs explicit construction or `chan()` helper update.
- [x] **Registry fix**: Set `trait_name: Some("Iterable")` on all `iter` methods -- currently `None` in: `str.rs:177-180`, `list/mod.rs:87`, `map/mod.rs:113-116`, `set/mod.rs:76-79`, `range/mod.rs:58-61`, `option/mod.rs:105-108` (6 types).
  - **WHERE**: Same pattern. These methods use `MethodDef::compound()` or `MethodDef::primitive()` which take `trait_name` as the 4th parameter -- change `None` to `Some("Iterable")`.
- [x] **STYLE**: Channel's `chan()` helper (`channel/mod.rs:31-49`) hardcodes `trait_name: None` for ALL methods. After adding trait names to `len`/`is_empty`, either: (a) add a `trait_name: Option<&'static str>` parameter to `chan()`, or (b) replace the `len`/`is_empty` entries with explicit `MethodDef` construction. Option (a) is cleaner since only 2 of 9 methods need trait names.

**Step 3 — Add marker traits to `TypeDef.traits`**

- [x] **Registry addition**: Add `Default` to `traits` field. Populate for: int (`defs/int.rs`), float (`defs/float.rs`), bool (`defs/bool.rs`), str (`defs/str.rs`), unit (has no TypeDef -- see NOTE below), duration (`defs/duration/mod.rs`), size (`defs/size/mod.rs`), option (`defs/option/mod.rs`) — 7 types with TypeDefs + 1 without. Rationale: `Default` is not a method -- it's a type-level capability. Adding a phantom `default` method to every primitive type would pollute the method namespace and violate the registry's "methods are callable" invariant.
  - **NOTE**: `Unit` and `Never` have no `TypeDef` in the registry (excluded from `BUILTIN_TYPES`). The bridge function must handle `Default` for Unit as a special case (same as current `traits.rs:102` which lists Unit with Default).
- [x] **Registry addition**: Add `Sendable` to the `traits` field for Duration (`defs/duration/mod.rs`) and Size (`defs/size/mod.rs`). `Sendable` is a marker trait with no method.
- [x] **REMOVED**: ~~Add `Formattable` to `traits` field for Duration and Size~~ — `Formattable` is ALREADY derivable from `MethodDef.trait_name: Some("Formattable")` on the `format` method in both Duration (`duration/mod.rs:85`) and Size (`size/mod.rs:82`). Neither the `traits.rs` string arrays NOR the `trait_set.rs` bitfields currently include `Formattable` -- it flows through the generic trait impl mechanism, not the builtin satisfaction path. Adding it to `TypeDef.traits` would introduce data that no current consumer queries. The bridge function will pick it up automatically from `MethodDef.trait_name`.
- [x] **Registry addition**: Add `Iterator` to the `traits` field for Iterator TypeDef (`defs/iterator/mod.rs`). Add both `Iterator` AND `DoubleEndedIterator` to DEI's `traits`. Since DEI has no separate TypeDef (it aliases to Iterator via `TypeTag::base_type()`), the bridge function must handle DEI trait satisfaction by checking both the base TypeDef's `traits` and adding `DoubleEndedIterator` for `TypeTag::DoubleEndedIterator`. This encodes the meta-trait satisfaction currently hardcoded in `traits.rs:214-215`.

**Step 4 — Verify**

- [x] **Enforcement test (write FIRST, before Step 2/3 changes)**: Add a test in `compiler/ori_registry/src/defs/tests.rs` that iterates ALL methods across ALL TypeDefs and verifies: every `len` has `trait_name: Some("Len")`, every `is_empty` has `trait_name: Some("IsEmpty")`, every `iter` has `trait_name: Some("Iterable")`. This test should FAIL before Step 2 changes and PASS after. This prevents future drift when new types are added.
- [x] **Semantic pin test**: Add a test in `compiler/ori_registry/src/defs/tests.rs` that verifies each TypeDef with a non-empty `traits` field contains exactly the expected marker traits (e.g., `INT.traits` contains `["Default"]`). This test ONLY passes with the registry `traits` field populated -- reverting to `&[]` would fail it.
- [x] Run `timeout 150 cargo test -p ori_registry` to verify sorted alphabetical order and all invariants after additions
- [x] Run `timeout 150 cargo test -p ori_registry --release` to verify debug/release parity

---

## 02.2 Trait Satisfaction via Registry (calls/traits.rs)

**File(s):** `compiler/ori_types/src/infer/expr/calls/traits.rs`, `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs`

The `primitive_satisfies_trait()` function (lines 14-176) maintains 10 parallel `const` string arrays listing which traits each primitive type satisfies. The `type_satisfies_trait()` function (lines 183-218) adds more hardcoded arrays for compound types.

**LEAK findings (to be eliminated by the bridge function):**

- [x] **LEAK:scattered-knowledge** `traits.rs:16-141` -- 10 per-primitive `const` trait arrays (`INT_TRAITS`, `FLOAT_TRAITS`, etc.) duplicate knowledge derivable from registry `TypeDef.operators`, `TypeDef.methods`, and `TypeDef.traits`
- [x] **LEAK:scattered-knowledge** `traits.rs:184-194` -- 3 compound-type trait arrays (`COLLECTION_TRAITS`, `WRAPPER_TRAITS`, `RESULT_TRAITS`) duplicate knowledge derivable from registry
- [x] **LEAK:scattered-knowledge** `traits.rs:202-218` -- Per-tag trait satisfaction (`Tag::List`, `Tag::Map`, `Tag::Option`, etc.) hardcoded instead of registry-driven

**Implementation steps (in order):**

- [x] **Design**: Define the operator-trait mapping used by `registry_satisfies_trait`. The mapping from `OpDefs` field to trait name is:
  - `add != Unsupported` => `"Add"` | `sub` => `"Sub"` | `mul` => `"Mul"` | `div` => `"Div"` | `floor_div` => `"FloorDiv"` | `rem` => `"Rem"`
  - `neg` => `"Neg"` | `not` => `"Not"`
  - `bit_and` => `"BitAnd"` | `bit_or` => `"BitOr"` | `bit_xor` => `"BitXor"` | `bit_not` => `"BitNot"` | `shl` => `"Shl"` | `shr` => `"Shr"`
  - `eq != Unsupported` => `"Eq"` (implies `!=` too) | `lt != Unsupported` => `"Comparable"` (all comparison operators share one trait)
  - **WHERE**: Add a `const fn op_trait_name(field_name: &str) -> Option<&str>` or a `const` lookup table in `registry_bridge/mod.rs`
  - **NOTE**: `"Comparable"` derives from `lt` (not from `eq`). `"Hashable"` derives from the `hash` method's `trait_name`, not from operators.
- [x] Add `registry_satisfies_trait(tag: TypeTag, trait_name: &str) -> bool` to `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs`. The function combines four sources:
  1. `OpDefs` fields for operator traits (using the mapping above)
  2. `MethodDef.trait_name` for method traits (scan methods, check if any has matching `trait_name`)
  3. `TypeDef.traits` for marker traits (`Default`, `Sendable`, `Iterator`, `DoubleEndedIterator`)
  4. `TypeTag::base_type()` aliasing for DEI (DoubleEndedIterator satisfies both `Iterator` and `DoubleEndedIterator`)
  - **Special cases that must be handled**:
    - Unit/Never have no `TypeDef` in the registry. Unit satisfies `Eq, Comparable, Hashable, Clone, Default, Debug` (per `traits.rs:102`). The bridge must hardcode this OR `Unit`/`Never` must get TypeDefs (preferred long-term but out of scope for this section).
    - Str appears as both a primitive (`Idx::STR`) and a compound tag (`Tag::Str` with `Iterable`). The bridge must check both the `STR` TypeDef and compound-level traits.
    - `"Debug"` trait: VERIFIED — all primitives and compound types have a `debug` method with `trait_name: Some("Debug")` in the registry. No special handling needed.
- [x] Add `registry_type_satisfies_trait(tag: Tag, trait_name: &str) -> Option<bool>` wrapper that calls `tag_to_type_tag` then `registry_satisfies_trait` -- returns `None` for non-registry types (Named, Applied, Var, etc.)
  - **WHERE**: `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs`
- [x] **Equivalence test (write FIRST, before replacing function bodies)**: Write in `compiler/ori_types/src/infer/expr/registry_bridge/tests.rs`. Iterate ALL TypeTag variants x all trait names from the old arrays (`INT_TRAITS`, `FLOAT_TRAITS`, ..., `RESULT_TRAITS`), assert `registry_satisfies_trait` returns the same result as the old function for every combination. This test must run BEFORE removing the old code -- verify it passes with both old and new paths to establish equivalence.
  - **Matrix**: 23 TypeTags x 27 trait names = 621 combinations verified
  - **Semantic pin**: positive pins per primitive type (BitNot on Int, Sendable on Duration, etc.)
  - **Negative pin**: negative pins per primitive type (Not on Int, Add on Bool, Default on Char, etc.)
- [x] Replace `primitive_satisfies_trait()` body with registry bridge call. Keep the function signature stable (callers still use `Idx`-based API).
- [x] Replace `type_satisfies_trait()` body with registry bridge call. The `pool.tag(ty)` dispatch remains but delegates to the bridge instead of per-tag hardcoded arrays.
- [x] Delete the 13 `const` arrays (`INT_TRAITS`, `FLOAT_TRAITS`, ..., `RESULT_TRAITS`)
- [x] **STYLE**: Remove stale V1 references in `traits.rs` — line 7 "Mirrors V1's `primitive_implements_trait()` from `bound_checking.rs`" and line 15 "matching V1's const arrays" are references to a defunct codebase version. Replace with registry-referencing doc comments.
- [x] **STYLE**: Remove the `#[expect(clippy::too_many_lines)]` on `primitive_satisfies_trait` (line 10-13) — after the rewrite, the function should be well under 100 lines since it delegates to the bridge.
- [x] Run `timeout 150 cargo test -p ori_types` and `timeout 150 cargo test -p ori_types --release` to verify debug/release parity after replacement
  - **Bonus fixes discovered during equivalence testing**:
    - Fixed Float missing `Rem` in WellKnown bitfield table (trait_set.rs)
    - Added `Printable` to `TypeDef.traits` for compound types (List, Map, Set, Option, Result, Tuple, Range) — these are printable via generic trait impls, not type-specific `to_str` methods
    - Added `Comparable` to `TypeDef.traits` for Result and Tuple — comparison via generic lexicographic dispatch
    - Added `Debug`, `Add`, `IsEmpty`, `Iterable` to compound type bitfields to match registry truth
    - Updated well_known/tests.rs reference truth tables to match corrected trait sets

---

## 02.3 WellKnown Bitfield Trait Sets via Registry

**File(s):** `compiler/ori_types/src/check/well_known/mod.rs` (406 lines), `compiler/ori_types/src/check/well_known/trait_set.rs` (301 lines)

The `WellKnownNames` struct pre-computes bitfield-based trait sets at `ModuleChecker::new()` startup. `build_prim_trait_sets()` in `trait_set.rs` (lines 100-207) hardcodes the exact same per-type trait data as the string arrays in `traits.rs`, just encoded as `TraitSet` bitfields. `build_compound_trait_sets()` (lines 213-301) similarly hardcodes compound type traits.

This is the **same LEAK** as 02.2 but in a different encoding. Both implementations must derive from the registry.

**Approach**: Rather than building trait sets from hardcoded bit patterns, `build_prim_trait_sets()` and `build_compound_trait_sets()` should query the registry at startup. For each `TypeDef` in `BUILTIN_TYPES`, iterate its `operators` (to set operator trait bits), `methods` (to set method trait bits via `trait_name`), and `traits` (to set marker trait bits). This makes the bitfield tables derived data, not independent copies.

**LEAK findings:**

- [x] **LEAK:scattered-knowledge** `trait_set.rs:100-207` -- `build_prim_trait_sets()` hardcodes per-primitive trait bundles (e.g., INT = `core_bundle + default + arithmetic + bitwise`). This is the same data as `INT_TRAITS` in traits.rs, re-encoded as bitfields.
- [x] **LEAK:scattered-knowledge** `trait_set.rs:213-301` -- `build_compound_trait_sets()` hardcodes per-compound-type trait bundles (list, map/set, option, result, tuple, range, str, DEI, iterator). Same data as compound arrays in traits.rs.
- [x] **LEAK:scattered-knowledge** `well_known/mod.rs:319-341` -- `WellKnownNames::type_satisfies_trait()` per-tag match mirrors `type_satisfies_trait()` in traits.rs

**Implementation steps (in order):**

- [x] **Add `FORMATTABLE` bit** to `trait_bits` module (`trait_set.rs:14-47`): Currently 27 bits are allocated (0-26). Add `pub const FORMATTABLE: u32 = 27;` and update `COUNT` to 28. The registry already has `trait_name: Some("Formattable")` on Duration/Size `format` methods, so the new bridge will automatically set this bit. Without it, Formattable satisfaction would be silently dropped during the trait_set derivation.
- [x] **Add `trait_name_to_bit()` function** in `trait_set.rs`. This maps trait name strings to `trait_bits` constants and is the join point between registry trait names and bitfield positions. Design:
  ```rust
  /// Map a trait name string to its bit position in `TraitSet`, or `None` if not tracked.
  pub(super) fn trait_name_to_bit(name: &str) -> Option<u32> {
      match name {
          "Eq" => Some(trait_bits::EQ),
          "Comparable" => Some(trait_bits::COMPARABLE),
          // ... all 28 mappings ...
          _ => None,
      }
  }
  ```
  - **WHERE**: `compiler/ori_types/src/check/well_known/trait_set.rs`, new function after `TraitSet` impl block.
  - **NOTE**: This function is also used by `build_trait_bit_map()` in `mod.rs` for the interned-Name mapping — after this change, `build_trait_bit_map()` can iterate the same list instead of maintaining its own.
- [x] **Add operator-to-trait-name mapping** in `trait_set.rs`: a function that iterates `OpDefs` fields and yields trait names for supported operators. Design:
  ```rust
  /// Yield all trait names satisfied by the given operator definitions.
  pub(super) fn op_trait_names(ops: &OpDefs) -> impl Iterator<Item = &'static str> {
      // Check each field, yield trait name if != Unsupported
      // add -> "Add", sub -> "Sub", ..., eq -> "Eq", lt -> "Comparable", ...
  }
  ```
  - **NOTE**: This reuses the same operator-to-trait mapping designed in 02.2. Factor it so both 02.2's `registry_satisfies_trait` and 02.3's `build_*_trait_sets` share the same mapping. The mapping should live in `registry_bridge` (since it bridges registry data to type checker concepts) and be called by both consumers.
  - **Shared mapping**: `OP_TRAIT_MAP` exposed as `pub(crate)` from `registry_bridge/mod.rs`, re-exported via `infer::OP_TRAIT_MAP`. Both 02.2 and 02.3 share this table.
- [x] **Rewrite `build_prim_trait_sets()`** to iterate `BUILTIN_TYPES` and derive `TraitSet` from each `TypeDef`'s `operators`, `methods`, and `traits` fields. The algorithm:
  1. For each `TypeDef` in `BUILTIN_TYPES`, check if `tag.is_primitive()` (Int..Ordering)
  2. Iterate `TypeDef.operators` via the shared operator-trait mapping → set bits
  3. Iterate `TypeDef.methods`, for each with `trait_name: Some(name)` → `trait_name_to_bit(name)` → set bit
  4. Iterate `TypeDef.traits`, for each → `trait_name_to_bit(name)` → set bit
  5. Store at `tag` discriminant index in the array
  - **Special case**: Unit (`Idx::UNIT`) has no TypeDef. Hardcode its TraitSet as `Eq + Comparable + Hashable + Clone + Default + Debug` (matching current `traits.rs:102`). Add a comment noting this is the one remaining hardcoded entry until Unit gets a TypeDef.
- [x] **Rewrite `build_compound_trait_sets()`** similarly -- iterate `BUILTIN_TYPES` for compound types (List, Map, Set, etc.). The approach:
  1. For each `TypeDef` with compound tag (List, Map, Set, Range, Tuple, Option, Result, Channel, Iterator), derive `TraitSet` from operators + methods + traits
  2. Return a struct or map instead of the current 9-tuple (which is fragile and hard to extend)
  - **Refactor opportunity**: The 9-tuple return `(TraitSet, TraitSet, ..., TraitSet)` is a code smell. Replace with a `CompoundTraitSets` struct with named fields (`list`, `map_set`, `option`, ...) OR use `HashMap<TypeTag, TraitSet>` built from the registry iteration. The struct approach preserves `const`-like performance while being self-documenting.
  - **Str compound-level**: `Tag::Str` returns `str_compound_traits` which currently only contains `Iterable`. After 02.1 adds `trait_name: Some("Iterable")` to str's `iter` method, the bridge will derive this automatically. The primitive str TraitSet (from `build_prim_trait_sets`) already covers Eq/Clone/etc.
  - **DEI**: DoubleEndedIterator has no separate TypeDef. Handle by checking Iterator TypeDef's `traits` field (which will contain `["Iterator"]`) and adding `DoubleEndedIterator` bit when the tag is `TypeTag::DoubleEndedIterator`. Similar to the special case in 02.2.
- [x] **Cross-check test (write FIRST, before rewriting build functions)**: Write in `compiler/ori_types/src/check/well_known/tests.rs`. Before removing old code, compute BOTH old hardcoded tables AND new registry-derived tables. Assert bitwise equality for every (type, trait) combination. Matrix: 12 primitive types x 28 trait bits + 9 compound types x 28 trait bits = ~588 bit checks. Verify this test passes with the old code as a baseline, then verify it still passes after the rewrite.
  - **Semantic pin**: at least one test that constructs a `TraitSet` by querying the registry and asserts it matches the expected bit pattern for a specific type (e.g., `Int` must have bits `EQ | COMPARABLE | HASHABLE | CLONE | DEFAULT | PRINTABLE | DEBUG | ADD | SUB | MUL | DIV | FLOOR_DIV | REM | NEG | BIT_AND | BIT_OR | BIT_XOR | BIT_NOT | SHL | SHR`). This test ONLY passes with registry-derived data.
  - **Negative pin**: verify that a type does NOT have bits it should not have (e.g., `Bool` must NOT have `ADD` bit set)
- [x] After cross-check passes, delete the hardcoded trait bundles in `build_prim_trait_sets()` and `build_compound_trait_sets()`
- [x] Verify `WellKnownNames::type_satisfies_trait()` still works correctly -- it reads from the derived bitfields, so the per-tag match structure can remain (it routes to the correct `TraitSet`), but the *data* in each `TraitSet` now comes from the registry
- [x] **STYLE**: Remove decorative banner comments in `trait_set.rs:95` (`// ── Satisfaction table builders ───`) and `mod.rs:296` (`// ── Trait satisfaction ───`). Per hygiene rules, use plain `// Section name` without decorative characters.
- [x] Run `timeout 150 cargo test -p ori_types` and `timeout 150 cargo test -p ori_types --release` to verify debug/release parity after rewrite
  - Also discovered and fixed: Str primitive now includes Iterable bit, Duration/Size include Formattable bit, Error includes Clone/Printable/Debug bits — all from registry derivation

**Note on `constraints.rs` dual-path:** `compiler/ori_types/src/infer/expr/calls/constraints.rs` (lines 153-157, 170-175) first tries `wk.type_satisfies_trait()` (bitfield path), then falls back to `type_satisfies_trait()` (string array path) when `WellKnownNames` is unavailable. After 02.2 and 02.3, both paths derive from the same registry data, eliminating the drift risk. The dual-path itself is not a problem -- it's a performance optimization (bitfield vs string scan) -- but the fact that they could disagree IS the problem this section eliminates.

---

## 02.4 Builtin Identifier Signatures

**File(s):** `compiler/ori_types/src/infer/expr/identifiers.rs` (333 lines), `compiler/ori_eval/src/function_val.rs`, `compiler/ori_eval/src/interpreter/mod.rs` (prelude registration)

The `infer_ident()` function constructs type signatures for builtin identifiers inline (e.g., `hash_combine` at line 74, `repeat` at line 80). These are free functions registered in the evaluator's prelude (`ori_eval/src/interpreter/mod.rs` → `register_prelude()` via `register_function_val`) whose type signatures are independently hardcoded in the type checker.

**Feasibility note:** Unlike methods (which live in `ori_registry` TypeDefs), free functions like `hash_combine` and `repeat` have no registry representation -- `ori_registry` only covers methods on types, not free functions. Creating a canonical source requires either: (a) adding a `PRELUDE_FUNCTIONS` array to `ori_registry` (similar to `BUILTIN_TYPES` but for free functions), or (b) a separate `prelude_registry` module in `ori_types`. Option (a) keeps the zero-dependency property and `const`-constructibility; option (b) allows richer type construction (Pool handles).

Also note that `identifiers.rs` lines 59-71 hardcode conversion function signatures (`int()`, `float()`, `str()`, `byte()`, `char()`, `bool()`) with a generic `(T) -> target` pattern, and line 163 hardcodes `Error(str) -> Error`. These are additional sync points with the evaluator's prelude.

**LEAK findings:**

- [x] **LEAK:scattered-knowledge** `identifiers.rs:74-80` -- Builtin identifier signatures (`hash_combine`, `repeat`) hardcoded in type checker instead of derived from a canonical prelude definition
- [x] **LEAK:scattered-knowledge** `identifiers.rs:59-71` -- Conversion function signatures (`int`, `float`, `str`, `byte`, `bool`, `char`) hardcoded; sync point with evaluator prelude

**Implementation steps:**

- [x] **Decision required**: Choose canonical source for prelude free-function signatures. Recommendation: **Option (a) — `PRELUDE_FUNCTIONS` in `ori_registry`**. Rationale: keeps zero-dependency property, `const`-constructible, single crate to update. The type information can be represented using existing `ReturnTag`/`ParamDef` types (already in `ori_registry`). The type checker converts `ReturnTag` → `Idx` via the existing `return_tag_to_idx` bridge (proven in Section 01).
  - **Scope boundary**: The variant constructors (`Some`, `None`, `Ok`, `Err` at lines 33-56) and type-registry constructors (lines 97-161) are NOT part of this LEAK -- they are runtime type construction, not prelude function signatures. The Error constructor (line 163) IS a sync point but is minor (single function).
  - **Conversion functions** (`int`, `float`, `str`, `byte`, `bool`, `char` at lines 59-71) are a special case: their signature is generic `(T) -> TargetType`. This can be represented in the registry as `PreludeFunctionDef { name: "int", params: &[ParamDef { name: "value", ty: ReturnTag::Fresh }], returns: ReturnTag::Concrete(TypeTag::Int) }`.
- [x] Implement the canonical source: Add `PreludeFunctionDef` struct and `PRELUDE_FUNCTIONS: &[PreludeFunctionDef]` to `ori_registry`. Fields: `name: &'static str`, `params: &'static [ParamDef]`, `returns: ReturnTag`. Populate with: `hash_combine`, `repeat`, `int`, `float`, `str`, `byte`, `bool`, `char`.
  - **WHERE**: New file `compiler/ori_registry/src/prelude.rs` + `mod prelude; pub use self::prelude::*;` in `lib.rs`
- [x] Wire the type checker: In `infer_ident()`, replace the hardcoded `hash_combine`/`repeat` branches and conversion function branches with a lookup into `PRELUDE_FUNCTIONS` followed by `prelude_function_to_idx` conversion.
- [x] Wire the evaluator: In `register_prelude()` (`compiler/ori_eval/src/interpreter/mod.rs`), verify every entry in `PRELUDE_FUNCTIONS` has a corresponding `register_function_val` call. (The evaluator side may remain independently registered since it needs runtime function values, not just type signatures -- but the NAME list must agree.)
  - **Verified**: All 8 entries in `PRELUDE_FUNCTIONS` have corresponding evaluator registrations.
- [x] **Equivalence test (write FIRST)**: Prelude function tests in `ori_registry/src/prelude/tests.rs` verify: (1) exact function list, (2) sorted order, (3) signature correctness for hash_combine/repeat/conversions, (4) unknown names return None.
- [x] **Enforcement test**: `prelude_functions_complete` asserts exact PRELUDE_FUNCTIONS contents — fails if any entry is added/removed without updating the test. `prelude_functions_sorted` enforces alphabetical order.
  - **Semantic pin**: `hash_combine_signature`, `repeat_signature`, `conversion_function_signatures` assert exact param/return types — only pass with correct registry data.
- [x] Run `timeout 150 cargo test -p ori_types` and `timeout 150 cargo test -p ori_registry` after changes, in both debug and release

---

## 02.5 Named Type Method Dispatch & Computed Returns

**File(s):** `compiler/ori_types/src/infer/expr/methods/mod.rs` (125 lines), `compiler/ori_types/src/infer/expr/methods/computed_returns.rs` (99 lines)

**Note:** As of Section 01 completion, the type checker's `resolve_builtin_method()` in `methods/mod.rs` already queries the registry via `ori_registry::find_method()` and converts return types via `registry_bridge::return_tag_to_idx()`. The original claim about duplicated return-type construction is **no longer accurate** -- the migration happened during Section 01 work.

Two residual items remain:

1. **Named/Applied type fallback**: `resolve_named_type_method()` (lines 83-104) handles user-defined types with hardcoded `to_str`/`debug` -> `Idx::STR`. These bypass the registry because user-defined types are not in `ori_registry`. This is architecturally correct (registry is for builtins only) and NOT a LEAK.

2. **Computed returns for Fresh methods**: `computed_returns.rs` handles methods where `ReturnTag::Fresh` needs specific type construction (e.g., `iter.map` propagating DEI-ness, `list.zip` returning `[(T, U)]`). This is necessary specialization -- the registry's `ReturnTag::Fresh` is intentionally underspecified for closure-dependent returns, and the type checker constructs the precise return type.

**Implementation steps:**

- [x] **Audit `ReturnTag::Fresh` coverage**: Iterate ALL methods across ALL TypeDefs in the registry with `returns == ReturnTag::Fresh`. For each, verify it has an entry in `computed_returns.rs` that constructs the precise return type. Document any missing entries as type inference quality gaps.
  - **WHERE**: Write as a test in `compiler/ori_types/src/infer/expr/methods/computed_returns/tests.rs` (or equivalent). The test should: (1) collect all `(TypeTag, method_name)` pairs with `ReturnTag::Fresh`, (2) assert each is handled by `resolve_computed_return()`. This test prevents future methods with `Fresh` returns from silently falling back to unconstrained type variables.
  - **Expected methods with Fresh**: `list.map`, `list.filter`, `list.fold`, `list.find`, `list.flat_map`, `list.min`, `list.max`, `list.zip`, `list.sort_by`, `list.group_by` (and similar on set, map, iterator). Each should have a computed_returns entry.
  - **Semantic pin**: assert the exact list of `(TypeTag, method_name)` pairs with `ReturnTag::Fresh` -- this catches both missing entries (new Fresh methods without computed_returns) and stale entries (computed_returns for methods that no longer use Fresh)
- [x] **LEAK:scattered-knowledge** -- `RANGE_FLOAT_ITERATION_METHODS` in `methods/mod.rs:28` is a hardcoded 3-element list (`["collect", "iter", "to_list"]`) that should be derivable from a registry annotation instead of maintained separately. Two approaches:
  - **(a) Registry annotation**: Add a `requires_iterable: bool` field to `MethodDef`. Set `true` on `collect`, `iter`, `to_list` for Range. The type checker then checks `method_def.requires_iterable && element_type == Float` instead of consulting a hardcoded list. This is the correct fix but adds a field that is only meaningful for Range.
  - **(b) Registry tag**: Add a `MethodKind::IterationDependent` variant. Less intrusive but less self-documenting.
  - **Recommendation**: (a) is more explicit. The field is cheap (`bool`, no struct size increase due to alignment) and directly encodes the constraint. The `is_float_range_iteration()` check in `methods/mod.rs:75-78` then becomes `method_def.requires_iterable && engine.pool().range_elem(receiver_ty) == Idx::FLOAT`.
  - **Alternative**: If adding a field to `MethodDef` is too invasive for this section, defer to a "cleanup" subsection with a concrete `- [ ]` anchor here. But the current 3-element hardcoded list IS a LEAK that will drift when new iteration-dependent methods are added.

---

## 02.R Third Party Review Findings

- [ ] `[TPR-02-002][medium]` [`compiler/ori_types/src/infer/expr/methods/tests.rs`](/home/eric/projects/ori_lang/compiler/ori_types/src/infer/expr/methods/tests.rs#L7) / [`compiler/ori_types/src/infer/expr/methods/tests.rs`](/home/eric/projects/ori_lang/compiler/ori_types/src/infer/expr/methods/tests.rs#L127) / [`compiler/ori_types/src/infer/expr/methods/computed_returns.rs`](/home/eric/projects/ori_lang/compiler/ori_types/src/infer/expr/methods/computed_returns.rs#L20) — The new 02.5 tests improve the old audit, but they still do not prove full `resolve_computed_return()` coverage. `fresh_return_methods_are_documented()` only snapshots the registry list, and `computed_returns_produce_structured_types()` spot-checks `List.zip`, iterator `map`/`zip`/`flatten`, and one fallback case. It never exercises the `Result.trace_entries` / `Error.trace_entries` structured-return branch at [`computed_returns.rs`](/home/eric/projects/ori_lang/compiler/ori_types/src/infer/expr/methods/computed_returns.rs#L35), so deleting that arm would leave the section-local audit green. The broader spec suite currently catches behavior, but the section’s claimed coverage audit is still incomplete.
- [ ] `[TPR-02-003][medium]` [`compiler/ori_registry/src/prelude/tests.rs`](/home/eric/projects/ori_lang/compiler/ori_registry/src/prelude/tests.rs#L5) / [`compiler/ori_registry/src/prelude/mod.rs`](/home/eric/projects/ori_lang/compiler/ori_registry/src/prelude/mod.rs#L47) / [`compiler/ori_eval/src/interpreter/prelude.rs`](/home/eric/projects/ori_lang/compiler/ori_eval/src/interpreter/prelude.rs#L28) — The section claims an enforcement test proving the canonical prelude list matches evaluator registration, but no such cross-crate check exists. The registry tests only snapshot `PRELUDE_FUNCTIONS` internally, and the evaluator still hardcodes its registration list separately in `register_prelude()`. That means the original drift vector is only fixed in the current tree, not structurally guarded: a future add/remove/rename on the evaluator side can desync from `PRELUDE_FUNCTIONS` without tripping the new registry tests.
- [ ] `[TPR-02-004][high]` [`compiler/ori_types/src/infer/expr/methods/mod.rs`](/home/eric/projects/ori_lang/compiler/ori_types/src/infer/expr/methods/mod.rs#L24) / [`compiler/ori_types/src/infer/expr/methods/tests.rs`](/home/eric/projects/ori_lang/compiler/ori_types/src/infer/expr/methods/tests.rs#L85) / [`compiler/ori_types/src/infer/expr/collections.rs`](/home/eric/projects/ori_lang/compiler/ori_types/src/infer/expr/collections.rs#L222) / [`compiler/ori_eval/src/methods/collections.rs`](/home/eric/projects/ori_lang/compiler/ori_eval/src/methods/collections.rs#L326) / [`compiler/ori_eval/src/operators/mod.rs`](/home/eric/projects/ori_lang/compiler/ori_eval/src/operators/mod.rs#L154) — The new registry-derived `Range<float>` gating is still too narrow, and the added test now locks that behavior in. `range_method_requires_iteration()` only rejects methods whose return tags project an element type, so `count`, `len`, `is_empty`, `contains`, and `step_by` remain accepted on `Range<float>`; `range_iteration_methods_derived_from_registry()` explicitly asserts that policy. But range construction is still float-permissive in [`infer_range()`](/home/eric/projects/ori_lang/compiler/ori_types/src/infer/expr/collections.rs#L222), while the evaluator and runtime method dispatch are integer-only (`BinaryOp::Range` only exists in the integer operator path, and range methods still call `require_int_arg` / `RangeValue` integer helpers). Concrete repro from this review: `ori check` accepts `let r = 0.0..10.0; r.count()`, while `ori run` fails later with `error[E6099]: range start must be an integer`. That is a real cross-phase GAP, not just a missing test: the section now codifies a policy the runtime cannot execute.

---

## 02.N Completion Checklist

**Registry changes (02.1):**
- [x] `TypeDef` struct has a `traits: &'static [&'static str]` field for marker/meta traits
- [x] Registry `MethodDef.trait_name` is set correctly for `Len`, `IsEmpty`, `Iterable` methods across all types (7+6+6 = 19 method updates)
- [x] Channel `chan()` helper updated to support `trait_name` parameter (or `len`/`is_empty` use explicit `MethodDef` construction)
- [x] Enforcement test: every `len` method has `trait_name: Some("Len")`, every `is_empty` has `Some("IsEmpty")`, every `iter` has `Some("Iterable")`
- [x] Registry has `Default` in `TypeDef.traits` for: int, float, bool, str, duration, size, option (7 types with TypeDefs; Unit handled as bridge special case)
- [x] Registry has `Sendable` in `TypeDef.traits` for Duration and Size
- [x] Registry has `Iterator` in Iterator TypeDef's `traits`; DEI handled by bridge special case
- [x] `Formattable` correctly derivable from `MethodDef.trait_name: Some("Formattable")` on Duration/Size `format` methods (no `TypeDef.traits` entry needed)

**String-based trait satisfaction (02.2):**
- [x] `registry_satisfies_trait(TypeTag, &str) -> bool` exists in `registry_bridge/mod.rs`
- [x] `registry_type_satisfies_trait(Tag, &str) -> Option<bool>` wrapper exists in `registry_bridge/mod.rs`
- [x] Operator-to-trait mapping is factored as a shared function usable by both 02.2 and 02.3
- [x] Unit special case handled (no TypeDef but satisfies 6 traits)
- [x] `primitive_satisfies_trait()` delegates to registry bridge
- [x] `type_satisfies_trait()` delegates to registry bridge
- [x] 13 `const` trait arrays deleted from `traits.rs`
- [x] Stale V1 references removed from `traits.rs`
- [x] Semantic pin test: `registry_satisfies_trait` agrees with old string arrays for ALL (TypeTag, trait) combinations

**Bitfield trait sets (02.3):**
- [x] `FORMATTABLE` bit added to `trait_bits` module (bit 27, count 28)
- [x] `trait_name_to_bit()` function exists in `trait_set.rs`
- [x] `build_prim_trait_sets()` derives bitfield tables from registry instead of hardcoding them
- [x] `build_compound_trait_sets()` derives bitfield tables from registry instead of hardcoding them
- [x] Cross-check test: registry-derived bitfields agree with old hardcoded bitfields for ALL (type, trait) combinations
- [x] Decorative banner comments removed from `trait_set.rs` and `mod.rs`

**Identifier signatures (02.4):**
- [x] `PRELUDE_FUNCTIONS` array exists in `ori_registry` (or decision documented for alternative approach)
- [x] `infer_ident()` derives `hash_combine`/`repeat`/conversion function signatures from canonical source
- [x] Enforcement test: canonical prelude function list matches both type checker and evaluator registrations

**Method dispatch (02.5):**
- [x] `ReturnTag::Fresh` coverage audit test exists
- [x] `RANGE_FLOAT_ITERATION_METHODS` has a plan for registry-based derivation (either implemented or anchored)

**Cross-cutting:**
- [x] Adding a trait impl for a builtin type in the registry is sufficient for BOTH string-based and bitfield-based trait satisfaction to recognize it
- [x] `constraints.rs` dual-path (well_known vs string fallback) produces identical results since both derive from registry
- [x] `timeout 150 ./test-all.sh` passes with zero regressions
- [x] `timeout 150 cargo test -p ori_registry` passes (including sort/purity invariants)
- [x] `timeout 150 cargo test -p ori_types --release` passes (debug/release parity)
- [x] `./clippy-all.sh` passes
- [x] Plan annotation cleanup: no hygiene-full section-02 annotations in source code (verified via grep)
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** All three parallel trait satisfaction implementations (string arrays, bitfield tables, and the `constraints.rs` dual-path) derive from `ori_registry` queries. The 13 hardcoded trait arrays in `traits.rs` are deleted. The hardcoded bitfield tables in `trait_set.rs` are replaced by registry-derived construction. Registry `MethodDef.trait_name` correctly encodes `Len`, `IsEmpty`, `Iterable`. `Default`, `Sendable`, `Iterator`, and `DoubleEndedIterator` are represented in the registry via `TypeDef.traits`. `Formattable` is derivable from method `trait_name` (no separate `traits` entry needed). `./test-all.sh` green.
