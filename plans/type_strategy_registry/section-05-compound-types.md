---
section: "05"
title: "Compound Type Definitions"
status: not-started
goal: "Define complete TypeDef specifications for Duration, Size, Ordering, Error, Channel, and format spec types in ori_registry"
depends_on:
  - "01"
  - "02"
sections:
  - id: "05.1"
    title: "Duration TypeDef"
    status: not-started
  - id: "05.2"
    title: "Size TypeDef"
    status: not-started
  - id: "05.3"
    title: "Ordering TypeDef"
    status: not-started
  - id: "05.4"
    title: "Error TypeDef"
    status: not-started
  - id: "05.5"
    title: "Channel TypeDef"
    status: not-started
  - id: "05.6"
    title: "Format Spec Types"
    status: not-started
  - id: "05.7"
    title: "Cross-Reference & Validation"
    status: not-started
---

# Section 05: Compound Type Definitions

**Context:** Compound types occupy a middle ground between simple primitives (int, float, bool) and generic containers (List, Map, Option). They have pre-interned `TypeId`/`Idx` values (Duration=9, Size=10, Ordering=11), rich method sets, operator overloading (Duration, Size), and enum variant structure (Ordering). They also exhibit the most complex sync gaps in the current codebase: Duration has 35 methods in typeck but only 23 in eval's `EVAL_BUILTIN_METHODS`, and Channel has 9 methods in typeck but zero eval dispatch handlers.

**Scope boundary:** This section covers Duration, Size, Ordering, Error, and Channel as `TypeDef` declarations. Format spec types (Alignment, Sign, FormatType, FormatSpec) are addressed in subsection 05.6 as a scoping decision — they are type variants rather than method-bearing types, so they may not need full `TypeDef` entries.

### Prerequisite: `const fn` Helpers for Compound Types

Section 03 defines `MethodDef::primitive()` — a `const fn` helper that fills in 5 constant frozen fields, keeping each method at ~1 line. Compound types **cannot reuse `MethodDef::primitive()`** because their frozen fields vary:
- `pure`: `true` for most, but `false` for all Channel methods
- `backend_required`: `true` for some, `false` for typeck-only and unimplemented methods
- `kind`: `MethodKind::Instance` for most, `MethodKind::Associated` for associated functions

**Required helpers** (define in `method.rs` alongside `MethodDef::primitive()`):

```rust
/// Instance method with configurable pure and backend_required.
const fn compound(name, params, returns, trait_name, receiver, pure, backend_required) -> MethodDef
/// Associated function (factory). Always Instance-like but kind=Associated.
const fn associated(name, params, returns, pure) -> MethodDef
```

Without these helpers, Duration alone (35 methods x ~12 lines = 420 lines + OpDefs + header) would exceed the 500-line file size limit. With helpers at ~1 line/method:
- `duration/mod.rs`: ~35 methods + OpDefs + header = ~80-100 lines
- `size/mod.rs`: ~30 methods + OpDefs + header = ~70-90 lines
- `ordering/mod.rs`: ~15 methods + OpDefs + VariantSpec + header = ~60-80 lines
- `error/mod.rs`: ~8 methods + header = ~30-40 lines
- `channel/mod.rs`: ~11 methods + header = ~40-50 lines
- **Total: ~280-360 lines** across 5 `mod.rs` files (well within limits; tests in separate `tests.rs` siblings)

> **WARNING (BLOAT risk):** If the `const fn` helpers are not implemented, `duration/mod.rs` WILL exceed 500 lines. Define the helpers in `method.rs` (Section 01/02) BEFORE implementing Section 05. This is the same prerequisite as Section 03.

### Test File Convention

Per hygiene rules (`impl-hygiene.md`), tests go in **sibling `tests.rs` files**, not inline. Each compound type file uses `#[cfg(test)] mod tests;` at the bottom, with test bodies in:
- `compiler/ori_registry/src/defs/duration/tests.rs`
- `compiler/ori_registry/src/defs/size/tests.rs`
- `compiler/ori_registry/src/defs/ordering/tests.rs`
- `compiler/ori_registry/src/defs/error/tests.rs`
- `compiler/ori_registry/src/defs/channel/tests.rs`

This means each type definition must be a **directory module** (`duration/mod.rs` + `duration/tests.rs`), not a flat file (`duration.rs`). Update file paths accordingly:
- `compiler/ori_registry/src/defs/duration/mod.rs` (was `duration.rs`)
- `compiler/ori_registry/src/defs/size/mod.rs` (was `size.rs`)
- `compiler/ori_registry/src/defs/ordering/mod.rs` (was `ordering.rs`)
- `compiler/ori_registry/src/defs/error/mod.rs` (was `error.rs`)
- `compiler/ori_registry/src/defs/channel/mod.rs` (was `channel.rs`)

### Operator Alias Scoping

The evaluator accepts long-form operator aliases (`subtract`, `multiply`, `divide`, `remainder`, `negate`) that are NOT in typeck or IR. These aliases are **eval-only dispatch sugar** — they exist in `EVAL_METHODS_NOT_IN_TYPECK` and `EVAL_METHODS_NOT_IN_IR`. The registry declares **only the canonical short forms** (`sub`, `mul`, `div`, `rem`, `neg`) matching IR and LLVM. The long-form aliases remain in the evaluator's dispatch layer (Section 10 wiring), not in the registry. This is a phase boundary concern: the registry is pure data consumed by all phases; eval-only aliases are eval-phase logic.

---

## 05.1 Duration TypeDef

**File:** `compiler/ori_registry/src/defs/duration/mod.rs` (tests: `duration/tests.rs`)

### Representation

```rust
pub const DURATION: TypeDef = TypeDef {
    tag: TypeTag::Duration,
    name: "Duration",
    type_params: TypeParamArity::Fixed(0),
    memory: MemoryStrategy::Copy,
    methods: &DURATION_METHODS,
    operators: DURATION_OPS,
};
```

Duration is stored as `i64` nanoseconds. It is a Copy type (bitwise copy, no ARC). Conversion constants live in `ori_ir::builtin_constants::duration` and are consumed by both eval and LLVM backends.

### Instance Methods (accessors)

These extract a component from the internal nanosecond representation via integer division:

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `nanoseconds` | — | `int` | Borrow | — | `ns` directly |
| `microseconds` | — | `int` | Borrow | — | `ns / NS_PER_US` |
| `milliseconds` | — | `int` | Borrow | — | `ns / NS_PER_MS` |
| `seconds` | — | `int` | Borrow | — | `ns / NS_PER_S` |
| `minutes` | — | `int` | Borrow | — | `ns / NS_PER_M` |
| `hours` | — | `int` | Borrow | — | `ns / NS_PER_H` |

### Instance Methods (predicates)

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `is_zero` | — | `bool` | Borrow | — | `ns == 0` |
| `is_positive` | — | `bool` | Borrow | — | `ns > 0` |
| `is_negative` | — | `bool` | Borrow | — | `ns < 0` |
| `abs` | — | `Duration` | Borrow | — | `ns.abs()` as Duration |

### Instance Methods (conversion aliases)

These return `float` representations:

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `as_seconds` / `to_seconds` | — | `float` | Borrow | — | `ns as f64 / NS_PER_S` |
| `as_millis` / `to_millis` | — | `float` | Borrow | — | `ns as f64 / NS_PER_MS` |
| `as_micros` / `to_micros` | — | `float` | Borrow | — | `ns as f64 / NS_PER_US` |
| `as_nanos` / `to_nanos` | — | `float` | Borrow | — | `ns as f64` |

### Associated Functions

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `from_nanoseconds` / `from_nanos` | `(int)` | `Duration` | Static | — | `val * 1` |
| `from_microseconds` / `from_micros` | `(int)` | `Duration` | Static | — | `val * NS_PER_US` |
| `from_milliseconds` / `from_millis` | `(int)` | `Duration` | Static | — | `val * NS_PER_MS` |
| `from_seconds` | `(int)` | `Duration` | Static | — | `val * NS_PER_S` |
| `from_minutes` | `(int)` | `Duration` | Static | — | `val * NS_PER_M` |
| `from_hours` | `(int)` | `Duration` | Static | — | `val * NS_PER_H` |
| `zero` | — | `Duration` | Static | — | `Duration(0)` |

### Trait Methods

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `compare` | `(Self)` | `Ordering` | Borrow | Comparable | `ns.cmp(&other.ns)` |
| `equals` | `(Self)` | `bool` | Borrow | Eq | `ns == other.ns` |
| `clone` | — | `Self` | Borrow | Clone | Bitwise copy |
| `hash` | — | `int` | Borrow | Hashable | Hash of "Duration" + ns |
| `to_str` | — | `str` | Borrow | Printable | Human-readable ("5s", "100ms") |
| `debug` | — | `str` | Borrow | Debug | Same as `to_str` for Duration |
| `format` | — | `str` | Borrow | Formattable | Template string formatting |

### Operator Methods

| Method | Params | Returns | Receiver | Trait | OpStrategy | Notes |
|--------|--------|---------|----------|-------|------------|-------|
| `add` | `(Self)` | `Self` | Borrow | Add | IntInstr | `ns + other.ns` |
| `sub` | `(Self)` | `Self` | Borrow | Sub | IntInstr | `ns - other.ns` |
| `mul` | `(int)` | `Self` | Borrow | Mul | IntInstr | `ns * scalar` (heterogeneous) |
| `div` | `(int)` | `Self` | Borrow | Div | IntInstr | `ns / scalar` (heterogeneous) |
| `rem` | `(Self)` | `Self` | Borrow | Rem | IntInstr | `ns % other.ns` |
| `neg` | — | `Self` | Borrow | Neg | IntInstr | `-ns` |

**Operator method name discrepancy:** The spec (`operator-rules.md`) defines the canonical trait method names as `subtract`, `multiply`, `divide`, `remainder`, `negate`. The IR registry (`ori_ir/src/builtin_methods/mod.rs`) uses shortened forms `sub`, `mul`, `div`, `rem`, `neg` — these are the names consumed by `ori_arc` and `ori_llvm`. The evaluator accepts both forms (short and long). The long-form aliases are tracked in `EVAL_METHODS_NOT_IN_TYPECK` (because typeck uses operator inference, not method dispatch). The registry should declare the IR short forms (`sub`, `mul`, `div`, `neg`, `rem`) as primary (matching current IR and LLVM consumers) and document the spec-canonical long forms as aliases.

### Current Coverage Matrix

| Method | ori_types | ori_eval (dispatch) | ori_ir | ori_llvm | Registry |
|--------|-----------|---------------------|--------|----------|----------|
| `nanoseconds` | Y | Y (Name-based) | Y | — | Planned |
| `microseconds` | Y | Y (Name-based) | Y | — | Planned |
| `milliseconds` | Y | Y (Name-based) | Y | — | Planned |
| `seconds` | Y | Y (Name-based) | Y | — | Planned |
| `minutes` | Y | Y (Name-based) | Y | — | Planned |
| `hours` | Y | Y (Name-based) | Y | — | Planned |
| `is_zero` | Y | — (gap) | — | — | Planned |
| `is_positive` | Y | — (gap) | — | — | Planned |
| `is_negative` | Y | — (gap) | — | — | Planned |
| `abs` | Y | — (gap) | — | — | Planned |
| `as_seconds` | Y | — (gap) | — | — | Planned |
| `as_millis` | Y | — (gap) | — | — | Planned |
| `as_micros` | Y | — (gap) | — | — | Planned |
| `as_nanos` | Y | — (gap) | — | — | Planned |
| `from_nanoseconds` | Y | Y (string-based) | — | — | Planned |
| `from_microseconds` | Y | Y (string-based) | — | — | Planned |
| `from_milliseconds` | Y | Y (string-based) | — | — | Planned |
| `from_seconds` | Y | Y (string-based) | — | — | Planned |
| `from_minutes` | Y | Y (string-based) | — | — | Planned |
| `from_hours` | Y | Y (string-based) | — | — | Planned |
| `zero` | Y | — (gap) | — | — | Planned |
| `compare` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `equals` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `clone` | Y | Y (Name-based) | Y | Y (primitives.rs) | Planned |
| `hash` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `to_str` | Y | Y (Name-based) | Y | Y (primitives.rs) | Planned |
| `debug` | Y | Y (Name-based) | Y | — | Planned |
| `format` | Y | — (gap) | — | — | Planned |
| `add` | Y | Y (Name-based) | Y | — (operator) | Planned |
| `sub` | Y | Y (Name-based) | Y | — (operator) | Planned |
| `mul` | Y | Y (Name-based) | Y | — (operator) | Planned |
| `div` | Y | Y (Name-based) | Y | — (operator) | Planned |
| `rem` | Y | Y (Name-based) | Y | — (operator) | Planned |
| `neg` | Y | Y (Name-based) | Y | — (operator) | Planned |
| `is_equal` | — | — | — | Y (traits.rs) | Planned |
| `is_less` | — | — | — | Y (traits.rs) | Planned |
| `is_greater` | — | — | — | Y (traits.rs) | Planned |
| `is_less_or_equal` | — | — | — | Y (traits.rs) | Planned |
| `is_greater_or_equal` | — | — | — | Y (traits.rs) | Planned |

**Observations:**
- ori_ir BUILTIN_METHODS has 18 Duration entries (lines 571-674); many typeck-only methods are in `TYPECK_METHODS_NOT_IN_IR` allowlist (23 entries)
- Duration operators are in `EVAL_METHODS_NOT_IN_TYPECK` because typeck uses operator inference, not method dispatch
- LLVM backend handles Duration via `IntInstr` (same as int) for operators, plus specific trait method handlers
- Several accessor/predicate methods (is_zero, is_positive, is_negative, abs) exist in typeck but not eval or IR
- LLVM has `is_equal` (alias for `equals`) which is not in typeck or eval for Duration — it's an LLVM-only method

### Traits Not Covered by the Registry (Duration-specific)

The following traits apply to Duration but are NOT represented as `MethodDef` entries in the method list:

1. **`Default`** — Duration implements Default with value `0ns` (zero nanoseconds). The `default()` method is an associated function (`Duration.default()`), not an instance method. Default trait satisfaction is handled by `well_known::type_satisfies_trait()`, not the method registry. Per Section 03 precedent (primitives), `default()` does not appear in the method list.

2. **`Formattable`** — Duration has an **explicit** `format` entry in the type checker (unlike primitives which get Formattable via blanket impl from Printable). The `format` method IS included in the method table above. This is a deliberate difference from the primitive pattern in Section 03.

3. **`Sendable`** — Duration is implicitly `Sendable` (Copy type, no heap allocation). Handled by the compiler's auto-derivation logic, not the registry.

4. **`Value`** — Duration is implicitly `Value` (marker trait, Copy type). Auto-derived by the compiler, not part of the registry.

### MethodDef Frozen Field Defaults (Duration)

Per frozen decision 13, every `MethodDef` has 10 fields. For Duration **instance methods**, the following fields have constant values:
- `pure`: `true` (all Duration methods are side-effect free; may panic on overflow)
- `backend_required`: `true` for methods with eval+IR+LLVM coverage; `false` for typeck-only methods (is_zero, is_positive, is_negative, abs, as_*, to_*, format)
- `kind`: `MethodKind::Instance`
- `dei_only`: `false` (not iterator methods)
- `dei_propagation`: `DeiPropagation::NotApplicable`

For Duration **associated functions** (from_*, zero):
- `kind`: `MethodKind::Associated`
- `receiver`: `Ownership::Borrow` (irrelevant for associated functions — no receiver; use `Borrow` for consistency with Section 04's str associated functions)
- `backend_required`: `false` (associated functions are not yet in IR or LLVM)

### Conversion Alias Distinction

Duration has two naming patterns for conversion methods:
- **`as_*`** (e.g., `as_seconds`) — returns `float`, conceptual "view as" the unit
- **`to_*`** (e.g., `to_seconds`) — returns `float`, identical semantics to `as_*`

Both are represented as **separate `MethodDef` entries** with identical signatures (same pattern as str aliases in Section 04). The registry does NOT have an `alias_of` field. The canonical name for each pair is the `as_*` form; `to_*` is the alias. Both must be independently resolvable.

### Tasks

- [ ] Define `DURATION_METHODS: &[MethodDef]` with all 35 methods
- [ ] Define `DURATION_OPS: OpDefs` with IntInstr for all arithmetic operators
- [ ] Mark associated functions (from_*, zero) with `MethodKind::Associated` or equivalent
- [ ] Mark conversion aliases (as_*, to_*) returning `float` with appropriate `ReturnTag`
- [ ] Document heterogeneous operators: mul/div take `int`, not `Self`
- [ ] Set `pure: true` on all Duration methods
- [ ] Set `backend_required: false` on typeck-only methods (is_zero, is_positive, is_negative, abs, as_*, to_*, format, zero)
- [ ] Set `backend_required: false` on all associated functions (from_*, zero)
- [ ] Set `dei_only: false` and `dei_propagation: NotApplicable` on all methods
- [ ] Verify against spec (`docs/ori_lang/v2026/spec/`) that Duration method list matches spec §8.1.4
- [ ] Create `duration/tests.rs` with `#[cfg(test)] mod tests;` in `duration/mod.rs`
- [ ] Unit test: method count matches expected (35)
- [ ] Unit test: all trait methods have correct trait_name
- [ ] Unit test: all associated functions have `kind == MethodKind::Associated`
- [ ] Unit test: no method has `dei_only: true`
- [ ] Unit test: all 10 frozen fields present on every MethodDef (name, receiver, params, returns, trait_name, pure, backend_required, kind, dei_only, dei_propagation)

---

## 05.2 Size TypeDef

**File:** `compiler/ori_registry/src/defs/size/mod.rs` (tests: `size/tests.rs`)

### Representation

```rust
pub const SIZE: TypeDef = TypeDef {
    tag: TypeTag::Size,
    name: "Size",
    type_params: TypeParamArity::Fixed(0),
    memory: MemoryStrategy::Copy,
    methods: &SIZE_METHODS,
    operators: SIZE_OPS,
};
```

Size is stored as `u64` bytes. It is a Copy type. Conversion constants live in `ori_ir::builtin_constants::size` (SI units, 1000-based). Size is semantically non-negative; subtraction that would produce negative is a runtime error.

### Instance Methods (accessors)

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `bytes` | — | `int` | Borrow | — | `bytes as i64` |
| `kilobytes` | — | `int` | Borrow | — | `bytes / BYTES_PER_KB` |
| `megabytes` | — | `int` | Borrow | — | `bytes / BYTES_PER_MB` |
| `gigabytes` | — | `int` | Borrow | — | `bytes / BYTES_PER_GB` |
| `terabytes` | — | `int` | Borrow | — | `bytes / BYTES_PER_TB` |

### Instance Methods (conversion aliases)

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `to_bytes` / `as_bytes` | — | `int` | Borrow | — | Same as `bytes` |
| `to_kb` | — | `int` | Borrow | — | Same as `kilobytes` |
| `to_mb` | — | `int` | Borrow | — | Same as `megabytes` |
| `to_gb` | — | `int` | Borrow | — | Same as `gigabytes` |
| `to_tb` | — | `int` | Borrow | — | Same as `terabytes` |

### Instance Methods (predicates)

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `is_zero` | — | `bool` | Borrow | — | `bytes == 0` |

### Associated Functions

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `from_bytes` | `(int)` | `Size` | Static | — | Direct (negative check) |
| `from_kilobytes` / `from_kb` | `(int)` | `Size` | Static | — | `val * BYTES_PER_KB` |
| `from_megabytes` / `from_mb` | `(int)` | `Size` | Static | — | `val * BYTES_PER_MB` |
| `from_gigabytes` / `from_gb` | `(int)` | `Size` | Static | — | `val * BYTES_PER_GB` |
| `from_terabytes` / `from_tb` | `(int)` | `Size` | Static | — | `val * BYTES_PER_TB` |
| `zero` | — | `Size` | Static | — | `Size(0)` |

### Trait Methods

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `compare` | `(Self)` | `Ordering` | Borrow | Comparable | `bytes.cmp(&other.bytes)` |
| `equals` | `(Self)` | `bool` | Borrow | Eq | `bytes == other.bytes` |
| `clone` | — | `Self` | Borrow | Clone | Bitwise copy |
| `hash` | — | `int` | Borrow | Hashable | Hash of "Size" + bytes |
| `to_str` | — | `str` | Borrow | Printable | Human-readable ("5mb", "100kb") |
| `debug` | — | `str` | Borrow | Debug | Same as `to_str` for Size |
| `format` | — | `str` | Borrow | Formattable | Template string formatting |

### Operator Methods

| Method | Params | Returns | Receiver | Trait | OpStrategy | Notes |
|--------|--------|---------|----------|-------|------------|-------|
| `add` | `(Self)` | `Self` | Borrow | Add | IntInstr | `bytes + other.bytes` (unsigned) |
| `sub` | `(Self)` | `Self` | Borrow | Sub | IntInstr | `bytes - other.bytes` (underflow check) |
| `mul` | `(int)` | `Self` | Borrow | Mul | IntInstr | `bytes * scalar` (negative check) |
| `div` | `(int)` | `Self` | Borrow | Div | IntInstr | `bytes / scalar` (negative check) |
| `rem` | `(Self)` | `Self` | Borrow | Rem | IntInstr | `bytes % other.bytes` |

**Note:** Size does NOT have `neg` — it is semantically non-negative.

### Current Coverage Matrix

| Method | ori_types | ori_eval (dispatch) | ori_ir | ori_llvm | Registry |
|--------|-----------|---------------------|--------|----------|----------|
| `bytes` | — (gap) | Y (Name-based) | Y | — | Planned |
| `kilobytes` | — (gap) | Y (Name-based) | Y | — | Planned |
| `megabytes` | — (gap) | Y (Name-based) | Y | — | Planned |
| `gigabytes` | — (gap) | Y (Name-based) | Y | — | Planned |
| `terabytes` | — (gap) | Y (Name-based) | Y | — | Planned |
| `to_bytes` / `as_bytes` | Y | — (gap) | — | — | Planned |
| `to_kb` | Y | — (gap) | — | — | Planned |
| `to_mb` | Y | — (gap) | — | — | Planned |
| `to_gb` | Y | — (gap) | — | — | Planned |
| `to_tb` | Y | — (gap) | — | — | Planned |
| `is_zero` | Y | — (gap) | — | — | Planned |
| `from_bytes` | Y | Y (string-based) | — | — | Planned |
| `from_kilobytes` / `from_kb` | Y | Y (string-based) | — | — | Planned |
| `from_megabytes` / `from_mb` | Y | Y (string-based) | — | — | Planned |
| `from_gigabytes` / `from_gb` | Y | Y (string-based) | — | — | Planned |
| `from_terabytes` / `from_tb` | Y | Y (string-based) | — | — | Planned |
| `zero` | Y | — (gap) | — | — | Planned |
| `compare` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `equals` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `clone` | Y | Y (Name-based) | Y | Y (primitives.rs) | Planned |
| `hash` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `to_str` | Y | Y (Name-based) | Y | Y (primitives.rs) | Planned |
| `debug` | Y | Y (Name-based) | Y | — | Planned |
| `format` | Y | — (gap) | — | — | Planned |
| `add` | — (operator) | Y (Name-based) | Y | — (operator) | Planned |
| `sub` | — (operator) | Y (Name-based) | Y | — (operator) | Planned |
| `mul` | — (operator) | Y (Name-based) | Y | — (operator) | Planned |
| `div` | — (operator) | Y (Name-based) | Y | — (operator) | Planned |
| `rem` | — (operator) | Y (Name-based) | Y | — (operator) | Planned |
| `is_equal` | — | — | — | Y (traits.rs) | Planned |
| `is_less` | — | — | — | Y (traits.rs) | Planned |
| `is_greater` | — | — | — | Y (traits.rs) | Planned |
| `is_less_or_equal` | — | — | — | Y (traits.rs) | Planned |
| `is_greater_or_equal` | — | — | — | Y (traits.rs) | Planned |

**Observations:**
- Size accessors (`bytes`, `kilobytes`, etc.) are in eval's `EVAL_BUILTIN_METHODS` and ori_ir but surprisingly NOT resolved by typeck's `resolve_size_method()` — typeck has the `to_*`/`as_*` aliases instead. This is a naming drift between eval and typeck.
- LLVM has `is_equal` (alias for `equals`) which is not in typeck or eval for Size — it's an LLVM-only method.
- Consistency allowlists: `EVAL_METHODS_NOT_IN_TYPECK` has 14 Size entries, `TYPECK_METHODS_NOT_IN_IR` has 19 Size entries, `TYPECK_METHODS_NOT_IN_EVAL` has 19 Size entries.
- The registry must establish canonical names and declare aliases explicitly.

### Traits Not Covered by the Registry (Size-specific)

The following traits apply to Size but are NOT represented as `MethodDef` entries:

1. **`Default`** — Size implements Default with value `0b` (zero bytes). The `default()` method is an associated function (`Size.default()`). Default trait satisfaction is handled by `well_known::type_satisfies_trait()`, not the method registry.

2. **`Formattable`** — Size has an **explicit** `format` entry in the type checker (same as Duration, unlike primitives). The `format` method IS included in the method table above.

3. **`Sendable`** — Size is implicitly `Sendable` (Copy type). Auto-derived by the compiler.

4. **`Value`** — Size is implicitly `Value` (marker trait, Copy type). Auto-derived by the compiler.

### MethodDef Frozen Field Defaults (Size)

Per frozen decision 13, every `MethodDef` has 10 fields. For Size methods:
- `pure`: `true` (all Size methods are side-effect free; sub may panic on underflow)
- `backend_required`: `true` for methods with eval+IR+LLVM coverage; `false` for typeck-only methods (to_*, as_*, is_zero, format, zero)
- `kind`: `MethodKind::Instance` for instance methods; `MethodKind::Associated` for from_*, zero
- `dei_only`: `false` (not iterator methods)
- `dei_propagation`: `DeiPropagation::NotApplicable`

### Conversion Alias Distinction

Size has three naming patterns for accessor methods:
- **Canonical accessors** (e.g., `bytes`, `kilobytes`) — return `int`, the value in that unit
- **`to_*`** (e.g., `to_bytes`, `to_kb`) — return `int`, aliases for canonical accessors
- **`as_*`** (e.g., `as_bytes`) — return `int`, alias for `to_bytes`

The naming drift between eval (canonical) and typeck (`to_*`/`as_*`) must be resolved. All forms should be declared as separate `MethodDef` entries. The canonical names are the short forms (`bytes`, `kilobytes`, etc.) since they appear in eval and IR.

### Tasks

- [ ] Define `SIZE_METHODS: &[MethodDef]` with all 24 methods
- [ ] Define `SIZE_OPS: OpDefs` with IntInstr for add/sub/mul/div/rem (no neg)
- [ ] Resolve naming drift: decide canonical names for accessors (e.g., `bytes` vs `to_bytes` vs `as_bytes`)
- [ ] Mark associated functions with `MethodKind::Associated`
- [ ] Document heterogeneous operators: mul/div take `int`, not `Self`
- [ ] Document non-negative invariant (sub can fail, no neg operator)
- [ ] Set `pure: true` on all Size methods
- [ ] Set `backend_required: false` on typeck-only methods (to_*, as_*, is_zero, format, zero)
- [ ] Set `backend_required: false` on all associated functions (from_*, zero)
- [ ] Set `dei_only: false` and `dei_propagation: NotApplicable` on all methods
- [ ] Verify against spec (`docs/ori_lang/v2026/spec/`) that Size method list matches spec §8.1.5
- [ ] Create `size/tests.rs` with `#[cfg(test)] mod tests;` in `size/mod.rs`
- [ ] Unit test: method count matches expected (24 + canonical accessor aliases)
- [ ] Unit test: no `neg` operator defined
- [ ] Unit test: all associated functions have `kind == MethodKind::Associated`
- [ ] Unit test: all 10 frozen fields present on every MethodDef

---

## 05.3 Ordering TypeDef

**File:** `compiler/ori_registry/src/defs/ordering/mod.rs` (tests: `ordering/tests.rs`)

### Representation

```rust
pub const ORDERING: TypeDef = TypeDef {
    tag: TypeTag::Ordering,
    name: "Ordering",
    type_params: TypeParamArity::Fixed(0),
    memory: MemoryStrategy::Copy,
    methods: &ORDERING_METHODS,
    operators: ORDERING_OPS, // eq only
};
```

Ordering is a Copy enum stored as `i8` in LLVM (tag: Less=0, Equal=1, Greater=2). It is the return type of the `compare` method on all Comparable types. Registered in `ori_types/check/registration/builtin_types.rs` with three unit variants.

### Instance Methods (predicates)

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `is_less` | — | `bool` | Borrow | — | `tag == 0` |
| `is_equal` | — | `bool` | Borrow | — | `tag == 1` |
| `is_greater` | — | `bool` | Borrow | — | `tag == 2` |
| `is_less_or_equal` | — | `bool` | Borrow | — | `tag <= 1` |
| `is_greater_or_equal` | — | `bool` | Borrow | — | `tag >= 1` |

### Instance Methods (combinators)

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `reverse` | — | `Ordering` | Borrow | — | Less<->Greater, Equal unchanged |
| `then` | `(Self)` | `Ordering` | Borrow | — | If self==Equal, use other; else keep self |
| `then_with` | `(() -> Ordering)` | `Ordering` | Borrow | — | Lazy version of `then` |

### Trait Methods

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `compare` | `(Self)` | `Ordering` | Borrow | Comparable | Tag comparison (Less<Equal<Greater) |
| `equals` | `(Self)` | `bool` | Borrow | Eq | Tag equality |
| `clone` | — | `Self` | Borrow | Clone | Bitwise copy |
| `hash` | — | `int` | Borrow | Hashable | Tag as hash: Less=-1, Equal=0, Greater=1 |
| `to_str` | — | `str` | Borrow | Printable | "Less" / "Equal" / "Greater" |
| `debug` | — | `str` | Borrow | Debug | Same as `to_str` for Ordering |

### Operator Methods

| Method | Params | Returns | Receiver | Trait | OpStrategy | Notes |
|--------|--------|---------|----------|-------|------------|-------|
| `equals` | `(Self)` | `bool` | Borrow | Eq | IntInstr | Tag comparison only |

Ordering supports `==` and `!=` only (no `<`, `>` — that would be circular since `<`/`>` desugar to `compare()` which returns Ordering). However, `compare` IS defined on Ordering itself for use in derived Comparable impls (tag-based ordering: Less < Equal < Greater).

### Current Coverage Matrix

| Method | ori_types | ori_eval (dispatch) | ori_ir | ori_llvm | Registry |
|--------|-----------|---------------------|--------|----------|----------|
| `is_less` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `is_equal` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `is_greater` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `is_less_or_equal` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `is_greater_or_equal` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `reverse` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `then` | Y | Y (Name-based) | Y | — | Planned |
| `then_with` | Y | — (gap) | — | — | Planned |
| `compare` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `equals` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `clone` | Y | Y (Name-based) | Y | Y (primitives.rs) | Planned |
| `hash` | Y | Y (Name-based) | Y | Y (traits.rs) | Planned |
| `to_str` | Y | Y (Name-based) | Y | — | Planned |
| `debug` | Y | Y (Name-based) | Y | — | Planned |
| `to_int` | — | — | — | Y (primitives.rs) | Planned |

**Observations:**
- Ordering has the best coverage of the compound types — nearly all methods are implemented across typeck, eval, and LLVM.
- LLVM has one extra method not in typeck/eval/IR: `to_int` (tag value conversion in primitives.rs).
- `then_with` takes a closure parameter. The IR `BUILTIN_METHODS` has a `ParamSpec::Closure` variant, but it is a unit variant that carries no closure signature information (e.g., argument types or return type). This is noted in `TYPECK_METHODS_NOT_IN_IR`: "Ordering — then_with takes closure, not expressible in IR ParamSpec." The registry needs a richer `ParamDef` to express this (e.g., `ParamDef { ty: ReturnTag::Fresh, ownership: Ownership::Copy }` for the closure parameter, with the actual closure signature resolved by the type checker).
- `then` is in eval and typeck but not LLVM.
- The LLVM backend handles Ordering methods via `emit_ordering_method()` in `traits.rs`.

### Variant Registration

Ordering is registered as an enum in `ori_types/check/registration/builtin_types.rs::register_ordering_type()`. Variant structure is declared as a **standalone constant** (not a field on `TypeDef`, which is scoped to methods + operators). Wiring phases can validate against it:

```rust
/// Ordering variant descriptors. Not part of TypeDef — variant structure
/// is consumed by type checker registration, not by method/operator dispatch.
pub struct VariantSpec {
    pub name: &'static str,
    pub tag: u8,
    pub fields: &'static [(&'static str, ReturnTag)],
}

pub const ORDERING_VARIANTS: &[VariantSpec] = &[
    VariantSpec { name: "Less", tag: 0, fields: &[] },
    VariantSpec { name: "Equal", tag: 1, fields: &[] },
    VariantSpec { name: "Greater", tag: 2, fields: &[] },
];
```

### Traits Not Covered by the Registry (Ordering-specific)

The following traits apply to Ordering but are NOT represented as `MethodDef` entries:

1. **`Default`** — Ordering implements Default with value `Equal`. The `default()` method is an associated function (`Ordering.default()`). Default trait satisfaction is handled by `well_known::type_satisfies_trait()`, not the method registry.

2. **`Formattable`** — Ordering does NOT have an explicit `format` entry in the type checker (unlike Duration/Size). Ordering satisfies `Formattable` via the blanket impl from `Printable`, same as primitives. No `format` MethodDef needed.

3. **`Sendable`** — Ordering is implicitly `Sendable` (Copy type). Auto-derived by the compiler.

4. **`Value`** — Ordering is implicitly `Value` (marker trait, Copy type). Auto-derived by the compiler.

### MethodDef Frozen Field Defaults (Ordering)

Per frozen decision 13, every `MethodDef` has 10 fields. For Ordering methods:
- `pure`: `true` (all Ordering methods are side-effect free and cannot panic)
- `backend_required`: `true` for methods with eval+LLVM coverage; `false` for `then_with` (typeck only) and `to_int` (LLVM only)
- `kind`: `MethodKind::Instance` (Ordering has no associated functions in the current codebase)
- `dei_only`: `false` (not iterator methods)
- `dei_propagation`: `DeiPropagation::NotApplicable`

### OpDefs Clarification

Ordering's `ORDERING_OPS` declares `IntInstr` for `eq` and `neq` only. All comparison operators (`lt`, `gt`, `lt_eq`, `gt_eq`) are `Unsupported` — Ordering itself does not support `<`/`>` (that would be circular since `<`/`>` desugar to `compare()` which returns Ordering). However, the `compare` **method** IS defined on Ordering (for derived Comparable impls) — this is a method-level declaration, not an operator-level one. The Ordering `compare` method uses tag-based ordering (`Less < Equal < Greater`), which the evaluator and LLVM backend implement directly in their method handlers.

### Tasks

- [ ] Define `ORDERING_METHODS: &[MethodDef]` with all 14 methods
- [ ] Define `ORDERING_OPS: OpDefs` with IntInstr for eq/neq only; all other ops `Unsupported`
- [ ] Define `ORDERING_VARIANTS` with tag values
- [ ] Define `then_with` closure parameter as `ParamDef { ty: ReturnTag::Fresh, ownership: Ownership::Copy }` per Section 01 decision 4 (no `Closure` variant). **Note:** `then_with`'s return type is static (`Ordering`), so the registry return is correct. However, closure parameter validation (constraining the arg to `() -> Ordering`) requires adding a `then_with` arm to `unify_higher_order_constraints` in `method_call.rs` — the current function only handles iterator methods. This is a pre-existing typeck gap, not blocking for registry migration, but should be tracked as a follow-up task.
- [ ] **Builtin parameter validation (Section 09):** Add `then_with` arm to `unify_higher_order_constraints` (method_call.rs:165) constraining the closure param to `() -> Ordering`. Current function only handles iterator methods — `then_with` hits the `_ => {}` no-op. **WARNING (complexity):** This is a cross-phase change in `ori_types` (not `ori_registry`). It requires understanding the unification engine's constraint propagation. Scope it as a Section 09 follow-up, not a Section 05 blocker.
- [ ] Document the no-comparison-operators constraint (no `<`/`>` on Ordering itself)
- [ ] Set `pure: true` on all Ordering methods
- [ ] Set `backend_required: false` on `then_with` (typeck-only)
- [ ] Set `dei_only: false` and `dei_propagation: NotApplicable` on all methods
- [ ] Verify against spec (`docs/ori_lang/v2026/spec/`) that Ordering method list matches spec §8.7
- [ ] Create `ordering/tests.rs` with `#[cfg(test)] mod tests;` in `ordering/mod.rs`
- [ ] Unit test: variant count == 3
- [ ] Unit test: all predicate methods return bool
- [ ] Unit test: all methods have `kind == MethodKind::Instance`
- [ ] Unit test: OpDefs has `Unsupported` for lt, gt, lt_eq, gt_eq
- [ ] Unit test: all 10 frozen fields present on every MethodDef

---

## 05.4 Error TypeDef

**File:** `compiler/ori_registry/src/defs/error/mod.rs` (tests: `error/tests.rs`)

### Representation

```rust
pub const ERROR: TypeDef = TypeDef {
    tag: TypeTag::Error,
    name: "error",
    type_params: TypeParamArity::Fixed(0),
    memory: MemoryStrategy::Arc, // contains str message + trace Vec
    methods: &ERROR_METHODS,
    operators: OpDefs::UNSUPPORTED,
};
```

Error is an Arc type (heap-allocated, reference-counted). It contains a message string and an optional trace (Vec of TraceEntryData). Error has no operators. It is the `E` type in `Result<T, E>` and can be created by `str.into()` (wrapping a string as an error message).

**Note:** In TYPECK_BUILTIN_METHODS and EVAL_BUILTIN_METHODS, Error is lowercased as `"error"`, not `"Error"`. This is because Error is referenced via Tag::Error, not as a user-visible type constructor. The registry should use the canonical display name.

### Instance Methods

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `message` | — | `str` | Borrow | — | Extract error message |
| `trace` | — | `str` | Borrow | Traceable | Formatted trace string |
| `trace_entries` | — | `[TraceEntry]` | Borrow | Traceable | List of TraceEntry structs |
| `has_trace` | — | `bool` | Borrow | Traceable | Whether trace is populated |
| `with_trace` | `(TraceEntry)` | `error` | Borrow | Traceable | New error with appended entry |

### Trait Methods

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `to_str` | — | `str` | Borrow | Printable | String representation |
| `debug` | — | `str` | Borrow | Debug | Debug representation |
| `clone` | — | `Self` | Borrow | Clone | Deep clone (new Arc) |

### Current Coverage Matrix

| Method | ori_types | ori_eval (dispatch) | ori_ir | ori_llvm | Registry |
|--------|-----------|---------------------|--------|----------|----------|
| `message` | Y | Y (Name-based) | — | — | Planned |
| `trace` | Y | Y (Name-based) | — | — | Planned |
| `trace_entries` | Y | Y (Name-based) | — | — | Planned |
| `has_trace` | Y | Y (Name-based) | — | — | Planned |
| `with_trace` | Y | Y (Name-based) | — | — | Planned |
| `to_str` | Y | Y (Name-based) | — | — | Planned |
| `debug` | Y | Y (Name-based) | — | — | Planned |
| `clone` | Y | Y (Name-based) | — | — | Planned |

**Observations:**
- Error methods are completely absent from `ori_ir/builtin_methods/mod.rs` BUILTIN_METHODS. All 8 methods are in `TYPECK_METHODS_NOT_IN_IR` and `EVAL_METHODS_NOT_IN_IR` allowlists.
- Error methods ARE fully implemented in both typeck (`resolve_error_method`) and eval (`dispatch_error_method`).
- LLVM backend has zero Error method handlers — Error is not yet supported in AOT compilation.
- `trace_entries` returns a list of `TraceEntry` structs. Typeck currently returns `fresh_var()` (`resolve_by_type.rs:310`) with **no downstream unification constraint** — the fresh var stays unconstrained, so any call-site type is accepted. Registry uses `ReturnTag::Fresh`, but the migration task MUST add an explicit type resolution bridge in the type checker (construct `[TraceEntry]` from the registered `TraceEntry` struct type) rather than leaving it as a bare `fresh_var()`. This is a pre-existing typeck gap that the registry migration should fix.
- `with_trace` takes a `TraceEntry` struct parameter. Registry uses `ParamDef { ty: ReturnTag::Fresh, ownership: Ownership::Owned }`. **Note:** current typeck does NOT validate the parameter type — builtin dispatch returns `Idx::ERROR` (resolve_by_type.rs:309) and `unify_higher_order_constraints` only handles iterator methods. The `TraceEntry` param is unchecked at call sites. Same gap as `then_with` (see key decision 4 below): return type is correct, but parameter validation is missing. **Follow-up task:** add builtin parameter type enforcement for `with_trace`.

### Resolved Design Questions

1. ~~**Should Error be in the registry at all?**~~ **Resolved:** Yes — frozen decision 6 (overview) explicitly includes Error in `BUILTIN_TYPES` with 8 methods. LLVM coverage is tracked as `backend_required: false` on Error methods until AOT Error support is added.
2. ~~**TraceEntry dependency:**~~ **Resolved:** Error methods use `ReturnTag::Fresh` for TraceEntry parameters and returns. `TraceEntry` is a stdlib struct, not a primitive — it does NOT get a `TypeTag` variant. **Migration risk:** Unlike closure parameters (where argument types drive unification), `trace_entries` has no arguments to constrain the fresh var. The Section 09 wiring task must add an explicit bridge rule that resolves `Fresh` to the concrete `[TraceEntry]` type when the method name is `trace_entries`. See task below.
3. **Naming:** Use `"error"` (matching current eval/typeck convention) or `"Error"` (matching BuiltinType::Error display)? Decision affects backward compatibility.

### Traits Not Covered by the Registry (Error-specific)

The following traits apply to Error but are NOT represented as `MethodDef` entries:

1. **`Default`** — Error does NOT implement Default. There is no sensible default error.

2. **`Formattable`** — Error satisfies `Formattable` via the blanket impl from `Printable`. No explicit `format` MethodDef needed (same pattern as primitives in Section 03).

3. **`Sendable`** — Error is NOT `Sendable` (it is Arc/heap-allocated with reference counting).

4. **`Eq`/`Comparable`/`Hashable`** — Error does NOT implement Eq, Comparable, or Hashable. Errors are compared by identity (reference), not by value.

### MethodDef Frozen Field Defaults (Error)

Per frozen decision 13, every `MethodDef` has 10 fields. For Error methods:
- `pure`: `true` on ALL Error methods. Per frozen decision 17, `pure` means "no observable side effects (no IO, no mutation, no global state) but MAY panic." Heap allocation is invisible to the caller and is NOT an observable side effect. `clone` returns a deterministic copy; `with_trace` returns a deterministic new error. Both are pure.
- `backend_required`: `false` on ALL Error methods (LLVM has zero Error support; AOT Error support is a future phase)
- `kind`: `MethodKind::Instance` (Error has no associated functions)
- `dei_only`: `false` (not iterator methods)
- `dei_propagation`: `DeiPropagation::NotApplicable`

### Tasks

- [ ] Define `ERROR_METHODS: &[MethodDef]` with all 8 methods
- [ ] Use `ParamDef { ty: ReturnTag::Fresh, ownership: Ownership::Owned }` for TraceEntry parameter (`with_trace`)
- [ ] Use `ReturnTag::Fresh` for `trace_entries` return
- [ ] **Migration bridge (Section 09):** Add explicit type resolution for `trace_entries` in the type checker — construct `[TraceEntry]` via `intern_name` → `named` → `list` instead of unconstrained `fresh_var()` (current gap at `resolve_by_type.rs:310`). **Prerequisite:** Section 09 test infrastructure task must land first (all `InferEngine` test helpers must call `set_interner()`). **WARNING (complexity):** This requires constructing a concrete `[TraceEntry]` type from the interned struct registration. The type checker must look up `TraceEntry` by name, which introduces a dependency on type registration order. Test with both `ori check` and `ori run` to verify the constructed type unifies correctly. Scope as Section 09, not Section 05.
- [ ] **Builtin parameter validation (Section 09):** Add `with_trace` parameter enforcement — constrain the argument to `TraceEntry`. Current builtin dispatch only returns the method's return type (`Idx::ERROR` at resolve_by_type.rs:309); the parameter type is unchecked. Requires extending `unify_higher_order_constraints` or adding a separate builtin-param validation pass.
- [ ] Document Arc memory strategy implications
- [ ] Note: no operators, no LLVM coverage (planned for AOT Error support phase)
- [ ] Set `pure: true` on ALL Error methods (per frozen decision 17, allocation is not an observable side effect)
- [ ] Set `backend_required: false` on ALL Error methods (no LLVM coverage)
- [ ] Set `dei_only: false` and `dei_propagation: NotApplicable` on all methods
- [ ] Verify against spec (`docs/ori_lang/v2026/spec/`) that Error method list matches spec §8.6
- [ ] Create `error/tests.rs` with `#[cfg(test)] mod tests;` in `error/mod.rs`
- [ ] Unit test: all methods borrow receiver
- [ ] Unit test: no method has `backend_required: true`
- [ ] Unit test: all methods have `kind == MethodKind::Instance`
- [ ] Unit test: all methods have `pure: true` (allocation is not observable)
- [ ] Unit test: all 10 frozen fields present on every MethodDef

---

## 05.5 Channel TypeDef

**File:** `compiler/ori_registry/src/defs/channel/mod.rs` (tests: `channel/tests.rs`)

### Representation

```rust
pub const CHANNEL: TypeDef = TypeDef {
    tag: TypeTag::Channel,
    name: "Channel",
    memory: MemoryStrategy::Arc, // shared channel handle
    methods: &CHANNEL_METHODS,
    operators: OpDefs::UNSUPPORTED,
    type_params: TypeParamArity::Fixed(1), // Channel<T>
};
```

Channel is a generic Arc type (`Channel<T>`). It is used for concurrency communication (send/receive). Channel has no operators. Unlike the other compound types in this section, Channel is a **generic container** (requires type argument), making it structurally more similar to List/Option than to Duration/Size.

> **WARNING (risk):** Channel is the highest-risk type in this section. It is the only generic compound type, has zero eval/IR/LLVM coverage, and its return types use `ReturnTag::OptionOf(TypeProjection::Element)` — a projection combinator that depends on the query API (Section 08) resolving type parameters correctly. Validate that `ReturnTag::OptionOf(TypeProjection::Element)` round-trips through `find_method()` before wiring any consuming phase. If projection resolution proves difficult, Channel can be deferred to Section 06 (collection/wrapper types) where List/Option establish the generic pattern first.

### Instance Methods

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `send` | `(T)` | `()` | Borrow | — | Send value into channel |
| `recv` / `receive` | — | `Option<T>` | Borrow | — | Blocking receive |
| `try_recv` / `try_receive` | — | `Option<T>` | Borrow | — | Non-blocking receive |
| `close` | — | `()` | Borrow | — | Close the channel |
| `is_closed` | — | `bool` | Borrow | — | Whether channel is closed |
| `is_empty` | — | `bool` | Borrow | — | Whether channel has no pending items |
| `len` | — | `int` | Borrow | — | Number of pending items |

### Current Coverage Matrix

| Method | ori_types | ori_eval (dispatch) | ori_ir | ori_llvm | Registry |
|--------|-----------|---------------------|--------|----------|----------|
| `send` | Y | — | — | — | Planned |
| `recv` | Y | — | — | — | Planned |
| `receive` | Y | — | — | — | Planned |
| `try_recv` | Y | — | — | — | Planned |
| `try_receive` | Y | — | — | — | Planned |
| `close` | Y | — | — | — | Planned |
| `is_closed` | Y | — | — | — | Planned |
| `is_empty` | Y | — | — | — | Planned |
| `len` | Y | — | — | — | Planned |

**Observations:**
- Channel is the least-implemented compound type. It exists ONLY in typeck (`resolve_channel_method`).
- Zero eval dispatch handlers — all 9 methods are in `TYPECK_METHODS_NOT_IN_EVAL` allowlist.
- Zero ori_ir BUILTIN_METHODS entries.
- Zero LLVM handlers.
- No `Value::Channel` variant exists in `ori_patterns` (noted in consistency.rs comment: "Channel — not in eval at all yet (no Channel value type)").
- Channel is listed in `WELL_KNOWN_GENERIC_TYPES` alongside Iterator, Option, etc.

### Resolved Design Questions

1. ~~**Should Channel be in the registry now?**~~ **Resolved:** Yes — include with `backend_required: false` on all methods. Same precedent as Error (frozen decision 6): the registry documents the intended API surface even when backends are incomplete. Enforcement tests (Section 14) track the coverage gap.
2. ~~**Generic type parameter:**~~ **Resolved:** `type_params: TypeParamArity::Fixed(1)` — frozen decision 8 defines `TypeParamArity` for exactly this case.
3. ~~**Return type complexity:**~~ **Resolved:** `recv`/`try_recv` use `ReturnTag::OptionOf(TypeProjection::Element)` — this variant exists in the frozen `ReturnTag` enum (Section 01).
4. ~~**Placement decision:**~~ **Resolved:** Keep in Section 05. Channel has its own `TypeTag::Channel` and is a special-purpose concurrency primitive, not a general collection. Its generic nature doesn't mandate Section 06 placement.

### Sendable Constraint on T

Per the Ori spec, Channel types require `T: Sendable` — values sent through channels must be safe for concurrent access. This constraint is enforced by the type checker during channel construction (`channel<T>(buffer:)` requires `T: Sendable`), NOT by the registry. The registry declares `Channel<T>` with `TypeParamArity::Fixed(1)` — the `Sendable` bound on `T` is a type-checker concern (checked in `ori_types` during type inference), not a registry-level declaration.

**Why not in the registry:** The registry's `TypeParamArity` describes arity (how many type params), not bounds on those params. Adding bounds would require a richer `TypeParamDef` struct with trait constraints, which is scope creep for the current plan. The `Sendable` constraint is already enforced by the type checker and does not need registry-level duplication.

### Traits Not Covered by the Registry (Channel-specific)

1. **`Default`** — Channel does NOT implement Default. Channels must be explicitly constructed with `channel<T>(buffer:)`.

2. **`Eq`/`Comparable`/`Hashable`/`Clone`** — Channel does NOT implement these traits. Channels are mutable shared-state primitives, not value types.

3. **`Sendable`** — Channel itself is NOT `Sendable` (it contains shared mutable state).

4. **`Formattable`/`Printable`/`Debug`** — Channel does NOT implement these traits. There is no meaningful string representation of a channel.

### MethodDef Frozen Field Defaults (Channel)

Per frozen decision 13, every `MethodDef` has 10 fields. For Channel methods:
- `pure`: `false` on ALL Channel methods (send/recv/close have side effects — they mutate shared state)
- `backend_required`: `false` on ALL methods (eval, IR, LLVM coverage is all zero)
- `kind`: `MethodKind::Instance` (Channel has no associated functions in the registry; the `channel<T>(buffer:)` constructor is a free function, not an associated function)
- `dei_only`: `false` (not iterator methods)
- `dei_propagation`: `DeiPropagation::NotApplicable`

### Parameter Ownership for `send`

The `send` method takes ownership of the value being sent (`Ownership::Owned`). The value is moved into the channel — the caller cannot use it after sending. This is the correct ownership for the `T` parameter: `ParamDef { name: "value", ty: ReturnTag::ElementType, ownership: Ownership::Owned }`.

### Tasks

- [ ] Define `CHANNEL_METHODS: &[MethodDef]` with all 9 methods (plus aliases)
- [ ] Use `type_params: TypeParamArity::Fixed(1)` for Channel<T>
- [ ] Use `ReturnTag::OptionOf(TypeProjection::Element)` for `recv`/`try_recv`
- [ ] Set `backend_required: false` on all methods (eval, IR, LLVM coverage is zero)
- [ ] Set `pure: false` on ALL Channel methods (side effects from shared-state mutation)
- [ ] Set `send` parameter ownership to `Ownership::Owned` (value moved into channel)
- [ ] Use `ReturnTag::Unit` for `send`, `close` return types
- [ ] Set `dei_only: false` and `dei_propagation: NotApplicable` on all methods
- [ ] Document `T: Sendable` constraint (enforced by type checker, not registry)
- [ ] Verify against spec (`docs/ori_lang/v2026/spec/`) that Channel method list matches spec §8.15
- [ ] Create `channel/tests.rs` with `#[cfg(test)] mod tests;` in `channel/mod.rs`
- [ ] Unit test: all 9 methods present with correct return types
- [ ] Unit test: no method has `backend_required: true`
- [ ] Unit test: all methods have `pure: false`
- [ ] Unit test: `send` parameter has `Ownership::Owned`
- [ ] Unit test: all 10 frozen fields present on every MethodDef

---

## 05.6 Format Spec Types

**Files:** Evaluation only; no separate registry files

### Current Definitions

Format spec types are defined in multiple locations as part of a 4-way sync:

| Type | ori_ir | ori_types | ori_eval | ori_rt |
|------|--------|-----------|----------|--------|
| `Align` (Left, Center, Right) | `format_spec.rs` | `builtin_types.rs` (as "Alignment") | `format.rs` (consumes ori_ir) | `format/mod.rs` |
| `Sign` (Plus, Minus, Space) | `format_spec.rs` | `builtin_types.rs` | `format.rs` (consumes ori_ir) | `format/mod.rs` |
| `FormatType` (8 variants) | `format_spec.rs` | `builtin_types.rs` | `format.rs` (consumes ori_ir) | `format/mod.rs` |
| `FormatSpec` (struct) | `format_spec.rs` | `builtin_types.rs` | `format.rs` (consumes ori_ir) | `format/mod.rs` |
| `ParsedFormatSpec` | `format_spec.rs` | — | `format.rs` (consumes ori_ir) | `format/mod.rs` (own definition) |

### Analysis

Format spec types are fundamentally different from the other types in this section:

1. **They are variant types, not method-bearing types.** Alignment, Sign, and FormatType are enums consumed by the format pipeline. They have no methods, no operators, no trait implementations. They exist purely as type variants for type checking.

2. **They already have a single source of truth.** `ori_ir::format_spec` defines `Align`, `Sign`, and `FormatType`. `ori_types` re-registers them as enum types for type checking. `ori_eval` consumes the `ori_ir` definitions directly. `ori_rt` defines its own parallel `Align`/`Sign`/`FormatType`/`ParsedFormatSpec` types (`pub(crate)` in `format/mod.rs`) — it does NOT import from `ori_ir` in production code (only in tests for validation).

3. **The 4-way sync is structural, not behavioral.** The sync concern is that `ori_types::register_alignment_type()` must create variants matching `ori_ir::Align`, and `ori_rt` must handle all `FormatType` variants. This is variant-set consistency, not method-set consistency.

### Recommendation

**Do NOT create full `TypeDef` entries for format spec types.** They lack methods, operators, and memory strategy concerns — all the things `TypeDef` is designed to centralize. Instead:

- **Document them** as a known sync point in Section 13 (Migrate ori_ir & Legacy Consolidation).
- **Add an enforcement test** (Section 14) that verifies variant counts match between `ori_ir::format_spec` and `ori_types::register_*_type()`.
- **If format types ever gain methods** (e.g., `FormatType.name() -> str`), create `TypeDef` entries at that point.

### Tasks

- [ ] Decide: format spec types stay outside registry (recommended) or get minimal entries
- [ ] If outside: document exclusion rationale in registry docs
- [ ] If outside: add variant-count enforcement test to Section 14
- [ ] If inside: create stub TypeDefs with empty method lists

---

## 05.7 Cross-Reference & Validation

### Full Type Coverage Summary

| Type | TypeTag | Memory | Methods (typeck) | Methods (eval) | Methods (IR) | Methods (LLVM) | Default | Sendable | Formattable | Status |
|------|---------|--------|-----------------|----------------|--------------|----------------|---------|----------|-------------|--------|
| Duration | `Duration` (9) | Copy | 35 | 23 | 18 | 10 | `0ns` | Yes | Explicit | Partial coverage |
| Size | `Size` (10) | Copy | 24 | 20 | 16 | 10 | `0b` | Yes | Explicit | Partial coverage |
| Ordering | `Ordering` (11) | Copy | 14 | 13 | 13 | 11 | `Equal` | Yes | Blanket | Good coverage |
| Error | `Error` (8) | Arc | 8 | 8 | 0 | 0 | No | No | Blanket | Typeck+eval only |
| Channel | `Channel` | Arc | 9 | 0 | 0 | 0 | No | No | No | Typeck only |
| Alignment | — | Copy | (enum, no methods) | — | — | — | — | — | — | Not a TypeDef |
| Sign | — | Copy | (enum, no methods) | — | — | — | — | — | — | Not a TypeDef |
| FormatType | — | Copy | (enum, no methods) | — | — | — | — | — | — | Not a TypeDef |
| FormatSpec | — | — | (struct, no methods) | — | — | — | — | — | — | Not a TypeDef |

**Column notes:**
- **Default**: Default trait implementation value. "No" = type does not implement Default. Default is handled by `well_known::type_satisfies_trait()`, not the registry.
- **Sendable**: Whether the type is Sendable (safe for channel transmission). Copy types are automatically Sendable. Arc/heap types are not.
- **Formattable**: "Explicit" = type has an explicit `format` MethodDef entry in the type checker. "Blanket" = type gets Formattable via the Printable blanket impl (no explicit MethodDef needed). "No" = type does not implement Formattable.

### Consistency Allowlist Entries to Eliminate

When compound types are in the registry, the following allowlist entries become obsolete:

**TYPECK_METHODS_NOT_IN_IR** (to be eliminated):
- Duration: 23 entries (abs, as_micros, as_millis, as_nanos, as_seconds, format, from_hours, from_micros, from_microseconds, from_millis, from_milliseconds, from_minutes, from_nanos, from_nanoseconds, from_seconds, is_negative, is_positive, is_zero, to_micros, to_millis, to_nanos, to_seconds, zero)
- Size: 19 entries (as_bytes, format, from_bytes, from_gb, from_gigabytes, from_kb, from_kilobytes, from_mb, from_megabytes, from_tb, from_terabytes, is_zero, to_bytes, to_gb, to_kb, to_mb, to_str, to_tb, zero)
- Ordering: 1 entry (then_with)
- Error: 8 entries (clone, debug, has_trace, message, to_str, trace, trace_entries, with_trace)

**EVAL_METHODS_NOT_IN_TYPECK** (to be eliminated):
- Duration: 11 entries (operator methods + aliases dispatched via operator inference)
- Size: 14 entries (operator methods + accessor names)

**TYPECK_METHODS_NOT_IN_EVAL** (to be eliminated):
- Duration: 23 entries (associated functions, conversions, predicates)
- Ordering: 2 entries (then_with, to_str — note: to_str IS in eval's EVAL_BUILTIN_METHODS, so this allowlist entry is stale)
- Size: 19 entries (associated functions, conversions, predicates)
- Channel: 9 entries (all methods)

**EVAL_METHODS_NOT_IN_IR** (to be eliminated):
- Duration: 5 entries (divide, multiply, negate, remainder, subtract — long-form operator aliases in eval but not IR)
- Size: 4 entries (divide, multiply, remainder, subtract — long-form operator aliases in eval but not IR)
- Error: 8 entries (clone, debug, has_trace, message, to_str, trace, trace_entries, with_trace)

**Total allowlist entries eliminated by this section: ~147** (51 TYPECK_NOT_IN_IR + 25 EVAL_NOT_IN_TYPECK + 53 TYPECK_NOT_IN_EVAL + 17 EVAL_NOT_IN_IR)

### Key Architectural Decisions Needed

1. **Associated functions vs instance methods:** ~~Resolved~~ — Frozen decision 9 (overview) defines `MethodKind` as `Instance | Associated`. Duration and Size associated functions (`from_seconds`, `from_bytes`, etc.) use `kind: MethodKind::Associated` with `receiver: Ownership::Borrow` (irrelevant for associated functions — no receiver; `Borrow` chosen for consistency with Section 04's str associated functions).

2. **Method aliases:** Both Duration and Size have multiple names for the same operation (e.g., `to_bytes`/`as_bytes`/`bytes`). Options:
   - A: Registry declares all names as separate methods (simplest, most explicit)
   - B: Registry declares canonical name + aliases list (more structured, enables "did you mean?" suggestions)
   - C: Registry declares canonical name only; phases resolve aliases locally (current eval behavior)

3. **Heterogeneous operators:** Duration.mul takes `int`, not `Duration`. Size.mul takes `int`, not `Size`. The `OpDefs` schema from Section 01 must support heterogeneous operand types, not just `Self`.

4. **Closure parameters:** ~~Partially resolved~~ — The existing `ori_ir::builtin_methods::ParamSpec` has a `Closure` variant, but it is a unit variant carrying no closure signature information. Per Section 01 decision 4, the planned registry `ParamDef` does not have a `Closure` variant either. Closure parameters use `ParamDef { ty: ReturnTag::Fresh, ownership: Ownership::Copy }`. The registry's role is to declare the parameter exists; the type checker handles inference. **Important limitation:** `unify_higher_order_constraints` (method_call.rs:165) currently only handles iterator methods (`map`, `flat_map`, `filter`, `any`, `all`, `find`, `for_each`, `fold`, `rfold`). Non-iterator closure methods like `Ordering.then_with` are NOT covered — the closure parameter type is unconstrained. Since `then_with`'s return type is static (`Ordering`), this gap only affects parameter validation, not return type inference. **Follow-up task:** extend `unify_higher_order_constraints` or add a separate builtin-parameter validation pass to constrain `then_with` closure to `() -> Ordering`. This is not blocking for registry migration.

### Implementation Order

Within this section, types should be implemented in this order. The order follows **progressive complexity** (simple before complex) and **upstream before downstream** (types that appear as return types of other types' methods should be defined first). Note: subsection numbering (05.1-05.5) follows document layout, not implementation order — implement in the sequence below:

1. **Ordering (05.3)** — smallest method set (14), best existing coverage, simplest representation (enum, Eq + Comparable traits). **Must be first** because Duration and Size methods return `Ordering` (via `compare`). Establishing `ReturnTag::Concrete(TypeTag::Ordering)` early validates the cross-type reference pattern.
2. **Duration (05.1)** — Copy type with operators, well-tested in eval, representative of the "unit type" pattern. Introduces `MethodKind::Associated` (associated functions) and heterogeneous operators (mul/div take `int`).
3. **Size (05.2)** — very similar to Duration, validates that the pattern generalizes. Confirms the `const fn` helper approach works for a second type with the same shape.
4. **Error (05.4)** — Arc type, no operators, limited coverage, tests registry with non-Copy memory strategy. Introduces `MemoryStrategy::Arc` for compound types and `ReturnTag::Fresh` for TraceEntry references.
5. **Channel (05.5)** — generic type, zero implementation, tests registry's ability to declare unimplemented APIs. **Must be last** because it uses `ReturnTag::OptionOf(TypeProjection::Element)` which depends on the projection combinator pattern — a pattern better validated by Section 06 (collections) first. If Section 06 is implemented before 05.5, Channel can reference that precedent.

### Exit Criteria

- [ ] All 5 compound types have `TypeDef` entries in `ori_registry`
- [ ] `cargo check -p ori_registry` passes
- [ ] Each TypeDef declares the complete method set (matching typeck's TYPECK_BUILTIN_METHODS entries for that type)
- [ ] Operator methods specify correct OpStrategy (IntInstr for Duration/Size arithmetic)
- [ ] Memory strategy is correct (Copy for Duration/Size/Ordering, Arc for Error/Channel)
- [ ] Associated functions are distinguishable from instance methods (`kind: MethodKind::Associated`)
- [ ] Heterogeneous operator parameter types (int for Duration.mul/div, Size.mul/div) are expressible
- [ ] Format spec types have a documented exclusion rationale or stub entries
- [ ] Unit tests verify method counts, trait associations, receiver ownership, and return types
- [ ] All existing tests pass: `cargo test -p ori_registry`
- [ ] Every `MethodDef` has all 10 frozen fields populated (name, receiver, params, returns, trait_name, pure, backend_required, kind, dei_only, dei_propagation)
- [ ] `pure` is correctly set: `true` for all methods except Channel methods (which have observable side effects from shared-state mutation)
- [ ] `backend_required` is correctly set: `false` for Error methods, Channel methods, and typeck-only methods
- [ ] `dei_only` is `false` on all compound type methods (none are iterator methods)
- [ ] `dei_propagation` is `NotApplicable` on all compound type methods
- [ ] Conversion aliases (as_*/to_*) are declared as separate `MethodDef` entries with identical signatures
- [ ] Each subsection's method list verified against the authoritative spec (`docs/ori_lang/v2026/spec/`)
- [ ] Channel's `send` parameter uses `Ownership::Owned` for the value parameter
- [ ] All 5 type definitions are directory modules (`type/mod.rs` + `type/tests.rs`)
- [ ] Each `mod.rs` has `#[cfg(test)] mod tests;` at the bottom (not inline tests)
- [ ] `const fn` helpers (`MethodDef::compound()`, `MethodDef::associated()`) are defined in `method.rs` before Section 05 implementation begins
- [ ] Associated function receivers use `Ownership::Borrow` (consistent with Section 04's str associated functions)
