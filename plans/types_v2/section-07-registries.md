---
section: "07"
title: Registries
status: complete
goal: Type, trait, and method registries for user-defined types
sections:
  - id: "07.1"
    title: TypeRegistry
    status: complete
  - id: "07.2"
    title: TraitRegistry
    status: complete
  - id: "07.3"
    title: MethodRegistry
    status: complete
  - id: "07.4"
    title: Built-in Methods
    status: complete
  - id: "07.5"
    title: Method Lookup Algorithm
    status: complete
---

# Section 07: Registries

**Status:** In Progress (~20%)
**Goal:** Unified registries for types, traits, and methods
**Source:** Current Ori implementation, improved design

---

## 07.1 TypeRegistry ✅

**Goal:** Registry for user-defined types (structs, enums)
**Status:** Complete (2026-02-04)

### Implementation

Created `ori_types/src/registry/types.rs` with:

- `TypeRegistry` — Dual-indexed (BTreeMap + FxHashMap) registry
- `TypeEntry` — Type definition with name, idx, kind, span, type_params, visibility
- `TypeKind` — Struct, Enum, Newtype, Alias variants
- `StructDef`, `FieldDef`, `VariantDef`, `VariantFields`
- `Visibility` — Public/Private

### Key Features

- O(1) lookup by name or pool Idx
- O(1) variant constructor lookup via secondary index
- O(1) struct field lookup by name
- Deterministic iteration via BTreeMap
- Full test coverage (8 tests)

### Tasks

- [x] Create `ori_types/src/registry/types.rs` ✅ (2026-02-04)
- [x] Define `TypeRegistry` with lookup methods ✅
- [x] Define `TypeEntry`, `TypeKind`, etc. ✅
- [x] Implement type registration (struct, enum, newtype, alias) ✅
- [x] Implement variant lookup ✅
- [x] Add tests for type registry (8 tests) ✅

---

## 07.2 TraitRegistry ✅

**Goal:** Registry for traits and implementations
**Status:** Complete (2026-02-04)

### Implementation

Created `ori_types/src/registry/traits.rs` with:

- `TraitRegistry` — Dual-indexed registry for traits and implementations
- `TraitEntry` — Trait definition with methods and associated types
- `TraitMethodDef` — Trait method signature with default body support
- `TraitAssocTypeDef` — Associated type definition with bounds
- `ImplEntry` — Implementation with methods and associated type impls
- `ImplMethodDef` — Method implementation with body reference
- `WhereConstraint` — Where clause constraint representation
- `MethodLookup` — Result enum for method resolution

### Key Features

- O(1) trait lookup by name or Idx
- O(1) impl lookup by self type or trait
- Inherent impl support (impl without trait)
- Method lookup with inherent-first priority
- Coherence checking (`has_impl()`)
- Full test coverage (7 tests)

### Tasks

- [x] Create `ori_types/src/registry/traits.rs` ✅ (2026-02-04)
- [x] Define `TraitRegistry` with lookup methods ✅
- [x] Define `TraitEntry`, `ImplEntry`, etc. ✅
- [x] Implement trait registration ✅
- [x] Implement impl registration with indexing ✅
- [x] Add impl lookup by type and trait ✅
- [x] Add method lookup with priority ✅
- [x] Add coherence checking ✅
- [x] Add tests for trait registry (7 tests) ✅

---

## 07.3 MethodRegistry ✅

**Goal:** Unified method lookup across all sources
**Status:** Complete (2026-02-04)

### Implementation

Created `ori_types/src/registry/methods.rs` with:

- `MethodRegistry` — Unified registry combining builtins with user methods
- `BuiltinMethod` — Built-in method definition with return type computation
- `BuiltinMethodKind` — Fixed, Element, or Transform return types
- `MethodTransform` — How to transform receiver type to get return type
- `HigherOrderMethod` — Signature patterns for map/filter/fold/etc.
- `MethodResolution` — Result enum distinguishing builtin vs impl methods

### Key Features

- Builtin methods registered at construction
- Return type computation for containers (element extraction, Option wrapping)
- Unified lookup: builtins → inherent → trait methods
- Documentation strings for IDE integration
- Full test coverage (5 tests)

### Tasks

- [x] Create `ori_types/src/registry/methods.rs` ✅ (2026-02-04)
- [x] Define `MethodRegistry` combining all sources ✅
- [x] Define `MethodResolution` enum ✅
- [x] Implement unified `lookup()` method ✅
- [x] Implement return type computation ✅
- [x] Add tests for method resolution (5 tests) ✅

---

## 07.4 Built-in Methods ✅

**Goal:** Define built-in methods for primitive and collection types
**Status:** Complete (2026-02-04)

### Implementation

Built-in methods are registered in `MethodRegistry::register_builtins()`:

**List methods:** `len`, `is_empty`, `first`, `last`, `reverse`, `contains`

**Option methods:** `is_some`, `is_none`, `unwrap`, `expect`

**Result methods:** `is_ok`, `is_err`, `unwrap`, `unwrap_err`, `ok`, `err`

**Map methods:** `len`, `is_empty`, `contains_key`, `get`, `keys`, `values`

**Set methods:** `len`, `is_empty`, `contains`

**String methods:** `len`, `is_empty`, `to_upper`, `to_lower`, `trim`, `trim_start`, `trim_end`, `starts_with`, `ends_with`, `contains`, `chars`, `bytes`

**Int methods:** `abs`, `to_float`, `to_str`, `min`, `max`, `clamp`

**Float methods:** `abs`, `floor`, `ceil`, `round`, `trunc`, `to_int`, `to_str`, `is_nan`, `is_infinite`, `is_finite`, `sqrt`, `sin`, `cos`, `tan`, `ln`, `log10`, `exp`, `pow`, `min`, `max`, `clamp`

### Return Type Computation

Methods use `BuiltinMethodKind` to compute return types:
- `Fixed(Idx)` — Returns a constant type
- `Element` — Returns the container's element type
- `Transform` — Applies a transformation (wrap in Option, extract key/value, etc.)

### Tasks

- [x] Implement all List methods ✅
- [x] Implement all Option methods ✅
- [x] Implement all Result methods ✅
- [x] Implement all String methods ✅
- [x] Implement all Int/Float methods ✅
- [x] Implement Map methods ✅
- [x] Implement Set methods ✅
- [x] Add return type computation ✅
- [x] Add tests for built-in method resolution ✅

---

## 07.5 Method Lookup Algorithm

**Goal:** Define the complete method resolution algorithm
**Status:** In Progress (~80%)

### Algorithm (Implemented)

```
lookup_method(receiver_ty, method_name):
    1. Get tag of receiver_ty

    2. Check BUILT-IN methods:
       - Look up (tag, method_name) in builtin registry
       - If found, return MethodResolution::Builtin

    3. Check INHERENT methods (via TraitRegistry):
       - Look up inherent impl for receiver_ty
       - Check if impl has method with method_name
       - If found, return MethodResolution::Impl(Inherent)

    4. Check TRAIT methods (via TraitRegistry):
       - For each impl where self_type matches receiver_ty:
         - Check if impl has method with method_name
         - If found, return MethodResolution::Impl(Trait)

    5. Return None (method not found)
```

### Tasks

- [x] Implement basic method lookup algorithm ✅
- [x] Handle builtin → inherent → trait priority ✅
- [ ] Handle auto-deref for Option/Result — deferred per proposal (`optional-method-forwarding-proposal.md`)
- [ ] Handle method ambiguity (multiple matches) — deferred until needed
- [ ] Add caching for frequently used lookups — deferred until profiling shows need

---

## 07.6 Completion Checklist

- [x] `TypeRegistry` complete with all operations ✅
- [x] `TraitRegistry` complete with coherence checking ✅
- [x] `MethodRegistry` unifying all method sources ✅
- [x] All built-in methods registered ✅
- [x] Method lookup algorithm working (builtin → inherent → trait) ✅
- [x] All tests passing ✅ (20 registry tests total)

**Section 07 Status:** ✅ Complete (2026-02-05)

**Deferred to future plans:**
1. Auto-deref for Option/Result — per `optional-method-forwarding-proposal.md`
2. Method ambiguity handling — implement when multiple trait impls become common
3. Caching — performance optimization, implement when profiling shows need

**Exit Criteria:** ✅ Met. Method calls resolve correctly through the unified registry, finding built-in methods, inherent methods, and trait methods in the correct priority order.
