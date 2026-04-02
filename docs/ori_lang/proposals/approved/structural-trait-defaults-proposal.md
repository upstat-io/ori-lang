# Proposal: Structural Trait Defaults

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-04-02
**Approved:** 2026-04-02
**Affects:** Spec (operator-rules.md, 09-properties-of-types.md), type checker, evaluator, LLVM codegen

---

## Summary

Ori is a pure value-type language. Structural comparison, copying, and formatting are always meaningful for value types. This proposal codifies the existing compiler behavior: traits with obvious structural implementations (Eq, Clone, Debug, Printable) work automatically without explicit declaration. Declaration overrides the structural default with a custom method.

This is not a new feature. The evaluator has always provided structural equality for all types. This proposal formalizes and documents that behavior as the intended semantics.

---

## Motivation

### The Problem

The current spec (`operator-rules.md` Annex B, Comparison section) states:

```
e1 : T    e2 : T    T : Eq
──────────────────────────    EQ
     e1 ==|!= e2 : bool
```

This requires `T : Eq` for `==`/`!=`. But the compiler has never enforced this — the evaluator performs structural field-by-field comparison for all types, and the LLVM codegen now does too. Every Ori program that compares user-defined types without `#derive(Eq)` or `type T: Eq = ...` works in practice but violates the spec.

### Why Structural Defaults Are Correct for Ori

1. **Pure value semantics**: Ori has no reference types. Every value is self-contained. Structural comparison is always well-defined — there is no reference identity vs value identity ambiguity.

2. **ARC is transparent**: Reference counting is an implementation detail. Users never observe sharing. Two values with identical fields are indistinguishable regardless of their refcount.

3. **Ergonomics**: Requiring `#derive(Eq)` on every type that needs `==` adds friction without adding information. The structural comparison is the only correct default for value types.

4. **Capability unification**: The `#derive(Trait)` syntax is being replaced by `type T: Trait = { ... }` (roadmap Section 05). Under the new syntax, `Eq` on the type declaration would generate a named method for trait dispatch — but `==` should work regardless.

### Prior Art

| Language | Structural Eq Default? | Requires Declaration? | Model |
|----------|----------------------|----------------------|-------|
| **Gleam** | Yes | No | All types structurally comparable |
| **Elm** | Yes | No | All types comparable (built-in `==`) |
| **Roc** | Yes | No | Structural equality on all types |
| **Rust** | No | `#[derive(PartialEq)]` | Explicit opt-in |
| **Go** | Yes (structs) | No | Comparable if all fields are |
| **C#** | Value types: yes | `IEquatable<T>` for custom | Structural default for value types |

Ori's model aligns with Gleam/Elm/Roc (functional, value-oriented) rather than Rust (ownership-oriented).

---

## Design

### Trait Categories

Traits fall into two categories based on whether a structural default exists:

#### Traits with Structural Defaults (work without declaration)

| Trait | Structural Default | Declaration Effect |
|-------|-------------------|-------------------|
| `Eq` | Field-by-field `==`/`!=` | Overrides with custom `equals(self, other: Self) -> bool` |
| `Clone` | Deep field-by-field copy | Overrides with custom `clone(self) -> Self` |
| `Debug` | `TypeName { field: value, ... }` | Overrides with custom `debug(self) -> str` |
| `Printable` | Same as Debug default | Overrides with custom `to_str(self) -> str` |

> NOTE: The structural default for both `Debug` and `Printable` produces `TypeName { field: value, ... }` format. Types that need human-facing formatting distinct from debug output should declare `Printable` and provide a custom `to_str` implementation.

#### Traits Requiring Explicit Declaration (no structural default)

| Trait | Why No Default |
|-------|---------------|
| `Comparable` | Field ordering is a semantic choice — which field sorts first? |
| `Hashable` | Must be consistent with equality contract — structural hash may not match custom `equals` |
| `Default` | Field values are not structurally derivable |

### Type Rules (Updated)

The EQ rule in operator-rules.md changes from:

```
e1 : T    e2 : T    T : Eq
──────────────────────────    EQ
     e1 ==|!= e2 : bool
```

To:

```
e1 : T    e2 : T
─────────────────    EQ
e1 ==|!= e2 : bool
```

No trait bound required. All types support `==`/`!=`.

The ORD rule remains unchanged (requires `T : Comparable`).

### Evaluation Rules (Updated)

```
STRUCTURAL EQUALITY (default, no Eq impl)
──────────────────────────────────────────
For struct types: compare each field recursively.
For newtype types: compare inner values.
For sum types: compare variant discriminants (integer tag).
  If tags differ: not equal.
  If tags match: compare payload fields recursively
  (same as struct comparison for the variant's payload).
For compound types (List, Map, Set, Tuple, Option, Result):
  existing element-wise comparison (unchanged).
For primitives: existing direct comparison (unchanged).

STRUCTURAL INEQUALITY
─────────────────────
e1 != e2 => !(structural_eq(e1, e2))

CUSTOM EQUALITY (Eq impl present)
──────────────────────────────────
e1 == e2 => e1.equals(other: e2)
e1 != e2 => !(e1.equals(other: e2))
```

### Dispatch Priority

When evaluating `a == b`:

1. **Check for `Eq` trait impl** on the type (derived or manual)
2. **If found**: call `equals(self, other)` method
3. **If not found**: use structural comparison (field-by-field recursion)

This matches the existing evaluator behavior and the LLVM codegen's `emit_structural_eq` fallback.

### Two-Tier Model

> **NOTE:** Structural defaults enable `==` on all types without ceremony. To use a type in generic contexts with `T: Eq` bounds (e.g., map keys, sort comparisons, generic utility functions), declare `Eq` explicitly: `type T: Eq = { ... }`. This generates a named `equals` method and satisfies trait bounds. The structural behavior is identical — the declaration makes it dispatchable.
>
> For frequently-compared types, declaring `type T: Eq = { ... }` is recommended even when only structural equality is needed, as it enables method inlining in LLVM codegen and allows the type to participate in generic code.

### Method Rename: `eq` → `equals`

The `Eq` trait method is renamed from `eq` to `equals` across the compiler and prelude:

- `prelude.ori`: `@eq (self, other: Self) -> bool` → `@equals (self, other: Self) -> bool`
- All internal dispatch: ori_ir, ori_types, ori_eval, ori_llvm
- User-visible trait definition changes
- `==`/`!=` operator desugaring target changes

This aligns the method name with the readable `equals(self, other: Self)` signature used throughout this proposal and the spec.

### What `type T: Eq = { ... }` Does Under This Model

Declaring `Eq` on a type:

1. **Generates** a named `equals` method (structural by default, overridable via `impl T: Eq`)
2. **Enables** trait dispatch — other generic code with `T: Eq` bounds can call `equals()`
3. **Satisfies** supertrait requirements — `Hashable: Eq` requires the explicit declaration
4. **Enables LLVM inlining** — the named method can be compiled and inlined, vs the generic structural fallback

Without `Eq`, the structural comparison still works, but:
- The type cannot satisfy `T: Eq` bounds in generic contexts
- The type cannot implement `Hashable` (which requires `Eq`)
- The comparison uses the generic structural fallback (slightly less optimized)

### Hashable Interaction

`Hashable: Eq` remains a requirement. A type used as a map key or set element must explicitly declare `Eq` (to ensure the hash-equality contract). Structural equality alone is not sufficient for hashing because a custom `equals` override could change what "equal" means.

```ori
// This works (structural == for comparison)
type Point = { x: int, y: int }
let a = Point { x: 1, y: 2 };
let b = Point { x: 1, y: 2 };
a == b  // true (structural)

// This requires explicit Eq (for map key)
type Point: Eq, Hashable = { x: int, y: int }
let m = { Point { x: 1, y: 2 }: "origin" };
```

---

## Impact

### Spec Changes

1. **operator-rules.md** Annex B, Comparison section: Remove `T : Eq` precondition from EQ rule
2. **09-properties-of-types.md** Eq section: Redefine as "custom equality override" rather than "enables equality"
3. **09-properties-of-types.md** Clone/Debug/Printable sections: Document structural defaults
4. **08-types.md** Newtype section: Document that newtypes inherit structural comparison from inner type

### Compiler Changes

#### Already implemented
- **Evaluator**: Structural equality via `eval_struct_binary`, `eval_variant_binary`, newtype delegation — works for all types including enums
- **Type checker**: Does not gate `==`/`!=` on Eq (unchanged)
- **LLVM codegen (structs)**: `emit_structural_eq` fallback in `compound_traits.rs`

#### Implementation needed (tracked in roadmap)
- **LLVM codegen (enums)**: `emit_structural_eq` for enum types — currently only `emit_derived_eq_call` with no structural fallback (`compound_traits.rs:205`)
- **LLVM codegen (Clone)**: `emit_structural_clone` — structural deep copy without `#derive(Clone)`
- **LLVM codegen (Debug)**: `emit_structural_debug` — structural debug formatting without `#derive(Debug)`
- **LLVM codegen (Printable)**: `emit_structural_to_str` — structural string formatting without `#derive(Printable)`
- **Method rename**: `eq` → `equals` across the compiler (see "Method Rename" section above)

### Breaking Changes

None. This formalizes existing behavior. The `eq` → `equals` method rename is a compiler-internal change that does not affect user code (users call `==`/`!=`, not the method directly).

---

## Alternatives Considered

### A: Require Eq for ==

Rejected. Would break existing code. Adds friction without safety benefit in a value-type language.

### B: Auto-derive Eq for all types

Rejected. Auto-deriving is implicit magic. The structural default is simpler — it doesn't generate methods or satisfy trait bounds, it just compares fields.

---

## Resolved Questions

1. **Structural `!=` on enums**: Yes. `Color.Red != Color.Blue` works structurally — the evaluator compares variant tags, then payloads recursively. Enum variants are values; structural comparison is always well-defined.

2. **Performance guidance**: Yes. The spec includes an informative NOTE recommending `type T: Eq = { ... }` for frequently-compared types to enable method inlining and generic dispatch. This is guidance, not a requirement.

3. **Clone structural default**: Yes. `clone()` has a structural default (deep field-by-field copy). The evaluator already implements this. LLVM codegen needs `emit_structural_clone` (tracked in roadmap). The same pattern applies to Debug and Printable.
