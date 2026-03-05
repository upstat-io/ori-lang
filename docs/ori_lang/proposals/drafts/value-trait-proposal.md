# Proposal: Value Trait — ARC-Free Value Types

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-05
**Affects:** Compiler (type checker, ARC pipeline, LLVM codegen), type system, memory model, spec
**Related:** `capability-unification-generics-proposal.md` (approved), `memory-model-edge-cases-proposal.md` (approved), `sendable-interior-mutability-proposal.md` (approved), `clone-trait-proposal.md` (approved)

---

## Summary

Introduce a `Value` marker trait that guarantees a type is always stored inline, copied by value, and completely bypasses ARC (automatic reference counting). Types declared with `Value` are never heap-allocated, never reference-counted, and always bitwise-copyable — like primitives, but for user-defined types.

```ori
type Point: Value, Eq = { x: float, y: float }
type Color: Value, Eq = { r: byte, g: byte, b: byte, a: byte }
type Vec3: Value = { x: float, y: float, z: float }
```

---

## Problem Statement

Ori's ARC system already optimizes small all-scalar structs to skip reference counting (the `ArcClassifier` in `ori_arc/src/classify/mod.rs` classifies them as `Scalar`). However, this optimization is **implicit and fragile**:

### 1. Silent Performance Regression

Adding a single reference-counted field silently changes a type from `Scalar` to `DefiniteRef`, introducing ARC overhead with no compiler feedback:

```ori
type GameEntity = { x: float, y: float, health: int }  // Scalar today

// Later, someone adds a name field:
type GameEntity = { x: float, y: float, health: int, name: str }
// Now DefiniteRef — ARC overhead on every copy, but no warning
```

### 2. No Semantic Contract

Library authors cannot express "this type must always be a value type" in their API. Users can unknowingly break performance invariants that the library depends on.

### 3. Implicit Clone vs Copy Ambiguity

Ori currently treats all value-passing as conceptual "copy" — but there's no way to distinguish types that are trivially copyable (bitwise copy, no side effects) from types that require deep cloning. The `Clone` trait exists for explicit deep copies, but there's no marker for "this type is always trivially copyable."

### 4. Const Generic Eligibility

The capability-unification proposal ties const generic eligibility to `Eq + Hashable`. Value types are natural candidates for const generics — but there's no way to express "this type is always a compile-time value."

---

## Design

### The `Value` Trait

`Value` is a marker trait in the prelude with no methods:

```ori
trait Value: Clone, Eq {
    // Marker trait — no methods.
    // Guarantees: inline storage, bitwise copy, no ARC, no Drop.
}
```

**Supertrait rationale**: Value types are always copyable (trivially, via bitwise copy — satisfying `Clone`) and must support equality comparison (required for identity-free semantics). If a type can be bitwise-copied, `Clone` is automatically satisfied with zero cost.

### Declaration Syntax

Following the capability-unification proposal (`:` for structural capabilities):

```ori
// Value type with derived Eq
type Point: Value, Eq = { x: float, y: float }

// Value type with derived Eq and Hashable
type Color: Value, Eq, Hashable = { x: byte, y: byte, z: byte, a: byte }

// Value type — Eq is implied by Value's supertrait, but derived impl still needed
type Vec3: Value, Eq = { x: float, y: float, z: float }

// Sum types can also be Value
type Direction: Value, Eq = North | South | East | West

// Newtypes wrapping value types
type Meters: Value, Eq = float
type UserId: Value, Eq = int
```

### Primitive Types Are Implicitly `Value`

All primitive types satisfy `Value` without declaration:

| Type | Value? | Reason |
|------|--------|--------|
| `int` | Yes | 8-byte scalar |
| `float` | Yes | 8-byte scalar |
| `bool` | Yes | 1-byte scalar |
| `char` | Yes | 4-byte scalar |
| `byte` | Yes | 1-byte scalar |
| `void` | Yes | Zero-size |
| `Duration` | Yes | 8-byte scalar |
| `Size` | Yes | 8-byte scalar |
| `Ordering` | Yes | Enum of 3 unit variants |
| `Never` | Yes | Uninhabited (vacuously true) |
| `str` | **No** | Heap-allocated, reference-counted |

### Compound Type Rules

| Type | Value When |
|------|-----------|
| `(T, U, ...)` | All elements are `Value` |
| `Option<T>` | `T: Value` |
| `Result<T, E>` | `T: Value` and `E: Value` |
| `Range<T>` | `T: Value` |
| `[T]` | **Never** — heap-allocated |
| `{K: V}` | **Never** — heap-allocated |
| `Set<T>` | **Never** — heap-allocated |
| `(T) -> U` | **Never** — may capture heap-allocated values |

### Fixed-Capacity Lists

Fixed-capacity lists (`[T, max N]`) are a special case. They are inline-allocated with a compile-time maximum size, so they could satisfy `Value` when `T: Value`:

```ori
type SmallBuffer: Value, Eq = { data: [byte, max 64], len: int }
```

Whether `[T, max N]` satisfies `Value` depends on implementation strategy. **This proposal defers this decision** — initially, `[T, max N]` does NOT satisfy `Value`. A future proposal can add it once the fixed-capacity list representation is finalized.

---

## Semantic Rules

### Rule 1: All Fields Must Be `Value`

Every field of a `Value` type must itself satisfy `Value`:

```ori
type Valid: Value, Eq = { x: int, y: float }       // OK: int and float are Value

type Invalid: Value, Eq = { x: int, name: str }    // ERROR: str is not Value
```

### Rule 2: No `Drop` Implementation

`Value` types cannot implement `Drop`. Bitwise-copyable types with custom destructors are contradictory — if a type needs cleanup, it has identity semantics and is not a pure value:

```ori
type Handle: Value, Eq = { fd: int }

impl Handle: Drop {                   // ERROR: Value types cannot implement Drop
    @drop (self) -> void = close(self.fd)
}
```

### Rule 3: No Recursive Types

Recursive types require heap indirection and cannot be stored inline:

```ori
type Tree: Value, Eq = {     // ERROR: recursive type cannot be Value
    value: int,
    children: [Tree],        // [Tree] is heap-allocated
}
```

### Rule 4: Size Limit

Value types must fit within a reasonable inline size threshold. Types exceeding **128 bytes** produce a warning; types exceeding **256 bytes** produce an error:

```ori
type TooLarge: Value, Eq = {
    matrix: (float, float, float, float,
             float, float, float, float,
             float, float, float, float,
             float, float, float, float,
             float, float, float, float,
             float, float, float, float,
             float, float, float, float,
             float, float, float, float,
             float, float, float, float)
}
// ERROR: Value type exceeds 256 bytes (288 bytes)
// help: consider using a heap-allocated type instead
```

**Rationale**: Excessively large value types cause stack overflow risk and poor cache performance from frequent large copies. The 128/256-byte thresholds are conservative — most value types (points, colors, quaternions, small matrices) are well under 64 bytes.

### Rule 5: Automatic `Clone` Satisfaction

A `Value` type automatically satisfies `Clone` via bitwise copy. No derived or manual `Clone` implementation is needed:

```ori
type Point: Value, Eq = { x: float, y: float }

// Point automatically satisfies Clone — .clone() returns a bitwise copy
let p1 = Point { x: 1.0, y: 2.0 }
let p2 = p1.clone()  // Works, but is a no-op (same as let p2 = p1)
```

### Rule 6: Automatic `Sendable` Satisfaction

All `Value` types are automatically `Sendable`. Since they have no heap references and no identity, they are trivially safe to send across task boundaries:

```ori
type Point: Value, Eq = { x: float, y: float }

// Point is automatically Sendable — no verification needed
parallel(tasks: [
    () -> compute(Point { x: 1.0, y: 2.0 }),  // OK
])
```

### Rule 7: No Manual Implementation

Like `Sendable`, `Value` cannot be manually implemented:

```ori
impl MyType: Value { }  // ERROR: Value cannot be implemented manually
```

`Value` is declared on the type definition (via `:`) and verified by the compiler. This prevents users from lying about value semantics.

---

## Interaction with ARC Pipeline

### Current Behavior (Implicit)

The `ArcClassifier` already classifies all-scalar structs as `Scalar`:

```
type Point = { x: float, y: float }
→ ArcClassifier: Scalar (no RC ops emitted)
```

But this is implicit — the compiler could silently change classification if fields change.

### New Behavior (Explicit + Enforced)

With `Value`, the classifier has an explicit signal:

```
type Point: Value, Eq = { x: float, y: float }
→ Type checker: verify all fields are Value ✓
→ ArcClassifier: Scalar (guaranteed by Value contract)
```

If a field violates the contract, the type checker catches it **before** the ARC pipeline runs:

```
type Bad: Value, Eq = { x: float, name: str }
→ Type checker: ERROR — str is not Value
```

### Implementation Strategy

The `Value` trait acts as a **compile-time assertion** that feeds into the existing ARC pipeline:

1. **Type checker** validates all `Value` rules (fields, no Drop, no recursion, size)
2. **ARC classifier** can fast-path `Value` types to `Scalar` without field traversal
3. **LLVM codegen** can use `memcpy` for `Value` types without RC inc/dec
4. **Borrow inference** skips `Value` parameters (always `Owned`, trivially cheap)

---

## Interaction with Other Features

### Generics and Bounds

`Value` can be used as a generic bound:

```ori
@fast_swap<T: Value> (a: T, b: T) -> (T, T) = (b, a)

@zero_copy_buffer<T: Value, $N: int> (initial: T) -> [T, max N] =
    // T is guaranteed inline — no RC overhead in buffer operations
    ...
```

### Const Generic Eligibility

The capability-unification proposal requires `Eq + Hashable` for const generic types. Since `Value` implies `Eq` (via supertrait), `Value + Hashable` types are natural const generic candidates:

```ori
type Color: Value, Eq, Hashable = { r: byte, g: byte, b: byte }

@create_palette<$theme: Color> () -> [Color] = [theme, ...]
```

### Struct Update Syntax

Value types work naturally with struct update syntax — no COW overhead:

```ori
type Point: Value, Eq = { x: float, y: float }

let p1 = Point { x: 1.0, y: 2.0 }
let p2 = { ...p1, x: 3.0 }  // Bitwise copy + field override, no RC
```

### Pattern Matching

Value types in pattern matching need no RC operations for binding:

```ori
type Color: Value, Eq = { r: byte, g: byte, b: byte, a: byte }

match pixel {
    Color { r, g, b, a: 255 } -> rgb_blend(r, g, b),  // No RC inc for r, g, b
    Color { a: 0, .. } -> transparent(),
    c -> default_blend(c),  // Bitwise copy, no RC
}
```

### `as` Conversions

Value types may implement `As` for conversions between value types:

```ori
type Celsius: Value, Eq = float
type Fahrenheit: Value, Eq = float

impl Celsius: As<Fahrenheit> {
    @as (self) -> Fahrenheit = Fahrenheit(self.inner * 9.0 / 5.0 + 32.0)
}
```

---

## Error Messages

### Non-Value Field

```
error[E2040]: field `name` of type `str` does not satisfy `Value`
  --> src/types.ori:2:5
   |
1  | type Entity: Value, Eq = {
   |              ----- `Value` declared here
2  |     name: str,
   |     ^^^^^^^^^ `str` is heap-allocated and reference-counted
   |
   = note: all fields of a `Value` type must themselves be `Value`
   = help: remove `Value` from the type declaration, or change `name` to a value type
```

### Drop Conflict

```
error[E2041]: `Value` type `Handle` cannot implement `Drop`
  --> src/types.ori:3:1
   |
1  | type Handle: Value, Eq = { fd: int }
   |              ----- `Value` declared here
   ...
3  | impl Handle: Drop {
   | ^^^^^^^^^^^^^^^^^^^^^ `Drop` requires identity semantics
   |
   = note: `Value` types are bitwise-copyable — custom destructors would
           run on every copy, not just the "last" one
   = help: remove `Value` to use reference-counted semantics with `Drop`
```

### Size Exceeded

```
error[E2042]: `Value` type `HugeMatrix` exceeds maximum size (256 bytes)
  --> src/types.ori:1:1
   |
1  | type HugeMatrix: Value, Eq = { ... }
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ size is 512 bytes
   |
   = note: Value types are copied on every assignment — large values
           cause stack pressure and poor cache performance
   = help: remove `Value` to use heap allocation, or reduce the type's size
```

### Size Warning

```
warning[W2040]: `Value` type `Matrix4x4` is large (128 bytes)
  --> src/types.ori:1:1
   |
1  | type Matrix4x4: Value, Eq = { ... }
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ consider whether heap allocation is more appropriate
   |
   = note: Value types are copied on every assignment
   = help: types over 128 bytes often perform better with reference counting
```

### Generic Bound Violation

```
error[E2043]: type `[int]` does not satisfy bound `Value`
  --> src/types.ori:5:20
   |
5  |     fast_swap(a: list1, b: list2)
   |                  ^^^^^ `[int]` is heap-allocated
   |
   = note: `fast_swap` requires `T: Value`
   = help: `[int]` uses reference counting and cannot be a `Value`
```

---

## Prior Art

### Rust — `Copy` Trait

Rust's `Copy` trait is the closest analogue. Types implementing `Copy` are bitwise-copied on assignment (no `move` semantics), and must not implement `Drop`. `Copy` implies `Clone`.

**Similarities**: marker trait, field recursion, no Drop, implies Clone.
**Differences**: Rust's `Copy` is auto-derivable with `#[derive(Copy)]`; Ori's `Value` is declared on the type. Rust separates `Copy` from `Clone`; Ori's `Value` implies both.

### C# — `struct` vs `class`

C# uses `struct` for value types and `class` for reference types. Value types are stack-allocated, copied by value, and cannot be null.

**Similarities**: explicit opt-in, inline storage, copy semantics.
**Differences**: C# makes this a fundamental type-system split. Ori keeps a single `type` keyword and uses the `Value` trait for opt-in value semantics.

### Swift — Value Types (Structs)

Swift structs are value types with COW (copy-on-write) optimization. All Swift structs are value types; classes are reference types.

**Similarities**: value semantics, copy-by-default.
**Differences**: Swift makes ALL structs value types. Ori defaults to ARC for all types and uses `Value` for opt-in ARC bypass. Swift uses COW for collections inside structs; Ori's `Value` types cannot contain collections.

### Lean 4 — Scalar Classification

Lean 4's LCNF IR classifies types as scalar or object for RC insertion, similar to Ori's `ArcClassifier`. However, this is an implementation detail, not a user-facing trait.

**Similarities**: scalar classification bypasses RC.
**Differences**: Lean's classification is implicit; Ori's `Value` makes it explicit and enforced.

### Koka — Value Types

Koka has value types that are unboxed and stored inline. Like Lean, this is mostly automatic rather than user-declared.

**Similarities**: inline storage, no boxing overhead.
**Differences**: Koka infers value representation; Ori lets users declare it.

---

## Examples

### Game Development

```ori
type Vec2: Value, Eq = { x: float, y: float }
type Vec3: Value, Eq = { x: float, y: float, z: float }
type Quaternion: Value, Eq = { x: float, y: float, z: float, w: float }
type AABB: Value, Eq = { min: Vec3, max: Vec3 }  // 48 bytes — fine

@lerp (a: Vec3, b: Vec3, t: float) -> Vec3 =
    Vec3 {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        z: a.z + (b.z - a.z) * t,
    }

// No ARC overhead anywhere in this hot path
@update_positions (positions: [Vec3], velocities: [Vec3], dt: float) -> [Vec3] =
    positions.zip(other: velocities)
        |> .map(transform: (p, v) -> Vec3 {
            x: p.x + v.x * dt,
            y: p.y + v.y * dt,
            z: p.z + v.z * dt,
        })
        |> .collect()
```

### Domain Modeling with Newtypes

```ori
type Meters: Value, Eq, Comparable = float
type Seconds: Value, Eq, Comparable = float
type MetersPerSecond: Value, Eq, Comparable = float

@velocity (distance: Meters, time: Seconds) -> MetersPerSecond =
    MetersPerSecond(distance.inner / time.inner)

// All operations are zero-overhead — no heap, no RC
```

### Color Processing

```ori
type RGBA: Value, Eq, Hashable = { r: byte, g: byte, b: byte, a: byte }

@blend (src: RGBA, dst: RGBA) -> RGBA = {
    let sa = src.a as float / 255.0
    let da = dst.a as float / 255.0
    let out_a = sa + da * (1.0 - sa)

    RGBA {
        r: ((src.r as float * sa + dst.r as float * da * (1.0 - sa)) / out_a) as byte,
        g: ((src.g as float * sa + dst.g as float * da * (1.0 - sa)) / out_a) as byte,
        b: ((src.b as float * sa + dst.b as float * da * (1.0 - sa)) / out_a) as byte,
        a: (out_a * 255.0) as byte,
    }
}
```

### Value Sum Types

```ori
type Axis: Value, Eq = X | Y | Z

type CardinalDirection: Value, Eq = North | South | East | West

type Tile: Value, Eq = Wall | Floor | Door(locked: bool) | Stairs(direction: int)

@opposite (dir: CardinalDirection) -> CardinalDirection =
    match dir {
        North -> South,
        South -> North,
        East -> West,
        West -> East,
    }
```

---

## Implementation

### Phase 1: Type Checker Validation

1. Add `Value` to the prelude as a marker trait (no methods)
2. In `ori_types/check/registration/`, register `Value` with supertrait `Clone + Eq`
3. In `ori_types/check/`, add validation pass for `Value` types:
   - All fields satisfy `Value` (recursive check)
   - No `Drop` impl registered for the type
   - No recursive type structure
   - Size within threshold
4. Add error codes E2040-E2043, warning W2040
5. `Value` types automatically satisfy `Sendable`

### Phase 2: ARC Pipeline Fast Path

1. In `ori_arc/src/classify/mod.rs`, add fast path: types with `Value` trait → `ArcClass::Scalar` (skip field traversal)
2. In `ori_arc/src/borrow/`, skip borrow inference for `Value` parameters (always cheap to copy)
3. In `ori_arc/src/rc_insert/`, skip RC insertion for `Value`-typed variables

### Phase 3: LLVM Codegen Optimization

1. In `ori_llvm`, use `memcpy` for `Value` types instead of field-by-field copy with RC
2. `Value` types can be passed in registers or by value (no indirection through pointers)
3. LLVM can fully optimize `Value` type operations (SROA, constant folding)

### Phase 4: Const Generic Integration

1. `Value + Hashable` types are eligible as const generic parameters
2. `Value` types can appear in const expressions

### Test Cases

1. Basic `Value` type declaration and usage
2. `Value` type with all-scalar fields — verify no RC operations emitted
3. `Value` type with non-Value field — verify compile error E2040
4. `Value` type with `Drop` impl — verify compile error E2041
5. `Value` type exceeding size limit — verify error E2042 / warning W2040
6. `Value` as generic bound — verify enforcement at call sites
7. `Value` sum types (all-unit and with Value payloads)
8. `Value` newtype wrapping Value type
9. `Value` newtype wrapping non-Value type — verify compile error
10. Nested `Value` types (`AABB` containing `Vec3`)
11. `Value` type in pattern matching — verify no RC operations
12. `Value` type satisfies `Clone` without explicit derive
13. `Value` type satisfies `Sendable` automatically
14. `Value` type in `parallel()` tasks — no Sendable verification needed
15. Implicit `Value` for primitive types in generic contexts

---

## Spec Changes Required

### Update `06-types.md`

Add "Value Types" section:
- `Value` trait definition (marker, no methods)
- Supertrait relationship (`Clone + Eq`)
- Field recursion rules
- Size thresholds
- Interaction with primitives

### Update `15-memory-model.md`

Add "Value Type Memory Semantics" section:
- Inline storage guarantee
- Bitwise copy semantics
- No ARC participation
- No Drop allowed

### Update `08-traits.md`

Add `Value` to prelude traits table:
- Marker trait, no methods
- Cannot be manually implemented
- Automatic `Clone` and `Sendable` satisfaction

### Update `grammar.ebnf`

No grammar changes needed — `Value` uses the existing `:` syntax from the capability-unification proposal.

### Add Error Codes

| Code | Description |
|------|-------------|
| E2040 | Field of `Value` type does not satisfy `Value` |
| E2041 | `Value` type cannot implement `Drop` |
| E2042 | `Value` type exceeds maximum size (256 bytes) |
| E2043 | Type does not satisfy `Value` bound |
| W2040 | `Value` type is large (>128 bytes) |

---

## Design Decisions

1. **Marker trait over keyword** — Using `type Point: Value` instead of a separate keyword (`value type Point`, `struct Point`, `inline type Point`) keeps one `type` keyword and leverages the capability-unification model. Value semantics is a *capability* of the type, not a different kind of type.

2. **Opt-in over opt-out** — Defaulting to ARC and opting into value semantics is safer than the reverse. Most types benefit from ARC (shared ownership, no large copies). Only performance-critical small types need `Value`. This avoids C#'s problem where choosing `struct` vs `class` is a premature decision.

3. **Supertrait `Clone + Eq`** — Every value type is trivially cloneable (bitwise copy) and should support equality (value types have no identity, only value). This avoids orphan `Value` types that can't be compared or copied.

4. **No manual impl** — Same rationale as `Sendable`: `Value` is a safety property verified by the compiler. Manual implementation could cause misclassification in the ARC pipeline, leading to use-after-free or leaks.

5. **Size limits** — Excessively large value types defeat the purpose (stack pressure, cache misses from large copies). The 128/256-byte thresholds are conservative and match typical cache line multiples. Most real-world value types (Vec3, Color, Quaternion, Matrix4) are well under 128 bytes.

6. **`str` excluded** — Strings are reference-counted in Ori (including SSO for ≤23 bytes). Even though short strings are inline, the `str` type itself participates in ARC at the type level. Mixing SSO with `Value` semantics creates confusion about when copies are free vs expensive.

7. **Functions excluded** — Closures may capture heap-allocated values. Even function pointers (no captures) use a fat-value representation. Excluding functions keeps `Value` simple and predictable.

8. **Fixed-capacity lists deferred** — `[T, max N]` *could* be `Value` (inline-allocated, compile-time bounded), but the representation details need to settle first. A follow-up proposal can add this.

---

## Open Questions

1. **Should `Value` imply `Eq`, or should they be independent?** The current design makes `Eq` a supertrait. Alternative: `Value` is independent, and you can have `type Point: Value = { ... }` without `Eq`. This would allow `Value` types with fields that don't support equality (e.g., value wrappers around opaque data).

2. **Should the size limit be configurable per type?** For example, `type BigMatrix: Value(max_size: 512), Eq = { ... }`. This adds complexity but handles legitimate large-but-inline use cases (e.g., 4x4 matrices = 128 bytes).

3. **Should `Value` types automatically derive `Eq`?** Since `Eq` is a supertrait, every `Value` type needs an `Eq` implementation. Should the compiler auto-derive it (like Sendable), or should the user explicitly write `: Value, Eq`?

4. **Should `[T, max N]` satisfy `Value` when `T: Value`?** This is deferred in this proposal but is a natural extension. The main concern is whether fixed-capacity list internals (capacity tracking, length field) complicate the "always bitwise-copyable" guarantee.

---

## Future Extensions

### Value Generics

```ori
@stack_allocate<T: Value> (count: int) -> [T, max 1024] =
    // Guaranteed no heap allocation for T — all inline
    ...
```

### SIMD-Friendly Value Types

```ori
#repr("aligned", 16)
type Vec4f: Value, Eq = { x: float, y: float, z: float, w: float }

// LLVM can auto-vectorize operations on aligned Value types
```

### `Value` in `embed()`

```ori
let $palette: [RGBA, max 256] = embed("palette.bin")
// Compile-time embedded binary data as Value array — zero runtime allocation
```

---

## Summary

| Aspect | Rule |
|--------|------|
| What it is | Marker trait guaranteeing inline, ARC-free value semantics |
| Syntax | `type T: Value, Eq = { ... }` |
| Supertrait | `Clone + Eq` |
| Fields | All must satisfy `Value` |
| Drop | Prohibited |
| Size | Warning >128 bytes, error >256 bytes |
| Sendable | Automatically satisfied |
| Manual impl | Prohibited |
| Primitives | Implicitly `Value` |
| Collections | `[T]`, `{K: V}`, `Set<T>` — never `Value` |
| Functions | Never `Value` |
| ARC effect | Classified as `Scalar` — no RC operations |
| Keyword | None — uses existing `:` syntax |
