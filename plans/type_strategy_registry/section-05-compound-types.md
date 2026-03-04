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

**Context:** Compound types occupy a middle ground between simple primitives (int, float, bool) and generic containers (List, Map, Option). They have pre-interned `TypeId`/`Idx` values (Duration=9, Size=10, Ordering=11), rich method sets, operator overloading (Duration, Size), and enum variant structure (Ordering). They also exhibit the most complex sync gaps in the current codebase: Duration has 35+ methods in typeck but only 23 in eval's `EVAL_BUILTIN_METHODS`, and Channel has 9 methods in typeck but zero eval dispatch handlers.

**Scope boundary:** This section covers Duration, Size, Ordering, Error, and Channel as `TypeDef` declarations. Format spec types (Alignment, Sign, FormatType, FormatSpec) are addressed in subsection 05.6 as a scoping decision — they are type variants rather than method-bearing types, so they may not need full `TypeDef` entries.

---

## 05.1 Duration TypeDef

**File:** `compiler/ori_registry/src/defs/duration.rs`

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

### Associated Functions (static constructors)

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

**Operator aliases in eval:** The evaluator additionally accepts `subtract`, `multiply`, `divide`, `negate`, `remainder` as method names. These are eval-only aliases (not in typeck or IR) tracked in `EVAL_METHODS_NOT_IN_TYPECK`. The registry should declare canonical names only (`sub`, `mul`, `div`, `neg`, `rem`); eval can resolve aliases locally.

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
| `is_less` | — | — | — | Y (traits.rs) | Planned |
| `is_greater` | — | — | — | Y (traits.rs) | Planned |
| `is_less_or_equal` | — | — | — | Y (traits.rs) | Planned |
| `is_greater_or_equal` | — | — | — | Y (traits.rs) | Planned |

**Observations:**
- ori_ir BUILTIN_METHODS has 18 Duration entries (lines 556-658); many typeck-only methods are in `TYPECK_METHODS_NOT_IN_IR` allowlist (24 entries)
- Duration operators are in `EVAL_METHODS_NOT_IN_TYPECK` because typeck uses operator inference, not method dispatch
- LLVM backend handles Duration via `IntInstr` (same as int) for operators, plus specific trait method handlers
- Several accessor/predicate methods (is_zero, is_positive, is_negative, abs) exist in typeck but not eval or IR

### Tasks

- [ ] Define `DURATION_METHODS: &[MethodDef]` with all 35+ methods
- [ ] Define `DURATION_OPS: OpDefs` with IntInstr for all arithmetic operators
- [ ] Mark associated functions (from_*, zero) with `MethodKind::Associated` or equivalent
- [ ] Mark conversion aliases (as_*, to_*) returning `float` with appropriate `ReturnTag`
- [ ] Document heterogeneous operators: mul/div take `int`, not `Self`
- [ ] Unit test: method count matches expected
- [ ] Unit test: all trait methods have correct trait_name

---

## 05.2 Size TypeDef

**File:** `compiler/ori_registry/src/defs/size.rs`

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

### Associated Functions (static constructors)

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
| `is_less` | — | — | — | Y (traits.rs) | Planned |
| `is_greater` | — | — | — | Y (traits.rs) | Planned |
| `is_less_or_equal` | — | — | — | Y (traits.rs) | Planned |
| `is_greater_or_equal` | — | — | — | Y (traits.rs) | Planned |

**Observations:**
- Size accessors (`bytes`, `kilobytes`, etc.) are in eval's `EVAL_BUILTIN_METHODS` and ori_ir but surprisingly NOT resolved by typeck's `resolve_size_method()` — typeck has the `to_*`/`as_*` aliases instead. This is a naming drift between eval and typeck.
- Consistency allowlists: `EVAL_METHODS_NOT_IN_TYPECK` has 13 Size entries, `TYPECK_METHODS_NOT_IN_IR` has 17 Size entries, `TYPECK_METHODS_NOT_IN_EVAL` has 17 Size entries.
- The registry must establish canonical names and declare aliases explicitly.

### Tasks

- [ ] Define `SIZE_METHODS: &[MethodDef]` with all 30+ methods
- [ ] Define `SIZE_OPS: OpDefs` with IntInstr for add/sub/mul/div/rem (no neg)
- [ ] Resolve naming drift: decide canonical names for accessors (e.g., `bytes` vs `to_bytes` vs `as_bytes`)
- [ ] Mark associated functions with `MethodKind::Associated`
- [ ] Document heterogeneous operators: mul/div take `int`, not `Self`
- [ ] Document non-negative invariant (sub can fail, no neg operator)
- [ ] Unit test: method count matches expected
- [ ] Unit test: no `neg` operator defined

---

## 05.3 Ordering TypeDef

**File:** `compiler/ori_registry/src/defs/ordering.rs`

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

**Observations:**
- Ordering has the best coverage of the compound types — nearly all methods are implemented across typeck, eval, and LLVM.
- `then_with` takes a closure parameter. The IR `BUILTIN_METHODS` currently uses `ParamSpec` which has no `Closure` variant that captures return type. This is noted in `TYPECK_METHODS_NOT_IN_IR`: "Ordering — then_with takes closure, not expressible in IR ParamSpec." The registry needs a richer `ParamDef` to express this (e.g., `ParamDef { ty: ReturnTag::Fresh, ownership: Ownership::Copy }` for the closure parameter, with the actual closure signature resolved by the type checker).
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

### Tasks

- [ ] Define `ORDERING_METHODS: &[MethodDef]` with all 14 methods
- [ ] Define `ORDERING_OPS: OpDefs` with IntInstr for eq only
- [ ] Define `ORDERING_VARIANTS` with tag values
- [ ] Handle `then_with` closure parameter: use `ParamDef { ty: ReturnTag::Fresh, ownership: Ownership::Copy }` per Section 01 decision 4 (no `Closure` variant). **Note:** `then_with`'s return type is static (`Ordering`), so the registry return is correct. However, closure parameter validation (constraining the arg to `() -> Ordering`) requires adding a `then_with` arm to `unify_higher_order_constraints` in `method_call.rs` — the current function only handles iterator methods. This is a pre-existing typeck gap, not blocking for registry migration, but should be tracked as a follow-up task.
- [ ] **Builtin parameter validation (Section 09):** Add `then_with` arm to `unify_higher_order_constraints` (method_call.rs:165) constraining the closure param to `() -> Ordering`. Current function only handles iterator methods — `then_with` hits the `_ => {}` no-op.
- [ ] Document the no-comparison-operators constraint (no `<`/`>` on Ordering itself)
- [ ] Unit test: variant count == 3
- [ ] Unit test: all predicate methods return bool

---

## 05.4 Error TypeDef

**File:** `compiler/ori_registry/src/defs/error.rs`

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

### Tasks

- [ ] Define `ERROR_METHODS: &[MethodDef]` with all 8 methods
- [ ] Use `ParamDef { ty: ReturnTag::Fresh, ownership: Ownership::Owned }` for TraceEntry parameter (`with_trace`)
- [ ] Use `ReturnTag::Fresh` for `trace_entries` return
- [ ] **Migration bridge (Section 09):** Add explicit type resolution for `trace_entries` in the type checker — construct `[TraceEntry]` via `intern_name` → `named` → `list` instead of unconstrained `fresh_var()` (current gap at `resolve_by_type.rs:310`). **Prerequisite:** Section 09 test infrastructure task must land first (all `InferEngine` test helpers must call `set_interner()`).
- [ ] **Builtin parameter validation (Section 09):** Add `with_trace` parameter enforcement — constrain the argument to `TraceEntry`. Current builtin dispatch only returns the method's return type (`Idx::ERROR` at resolve_by_type.rs:309); the parameter type is unchecked. Requires extending `unify_higher_order_constraints` or adding a separate builtin-param validation pass.
- [ ] Document Arc memory strategy implications
- [ ] Note: no operators, no LLVM coverage (planned for AOT Error support phase)
- [ ] Unit test: all methods borrow receiver

---

## 05.5 Channel TypeDef

**File:** `compiler/ori_registry/src/defs/channel.rs`

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

### Tasks

- [ ] Define `CHANNEL_METHODS: &[MethodDef]` with all 9 methods (plus aliases)
- [ ] Use `type_params: TypeParamArity::Fixed(1)` for Channel<T>
- [ ] Use `ReturnTag::OptionOf(TypeProjection::Element)` for `recv`/`try_recv`
- [ ] Set `backend_required: false` on all methods (eval, IR, LLVM coverage is zero)
- [ ] Unit test: all 9 methods present with correct return types

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
| `ParsedFormatSpec` | `format_spec.rs` | — | `format.rs` (consumes ori_ir) | — |

### Analysis

Format spec types are fundamentally different from the other types in this section:

1. **They are variant types, not method-bearing types.** Alignment, Sign, and FormatType are enums consumed by the format pipeline. They have no methods, no operators, no trait implementations. They exist purely as type variants for type checking.

2. **They already have a single source of truth.** `ori_ir::format_spec` defines `Align`, `Sign`, and `FormatType`. `ori_types` re-registers them as enum types for type checking. `ori_eval` and `ori_rt` consume the `ori_ir` definitions directly.

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

| Type | TypeTag | Memory | Methods (typeck) | Methods (eval) | Methods (IR) | Methods (LLVM) | Status |
|------|---------|--------|-----------------|----------------|--------------|----------------|--------|
| Duration | `Duration` (9) | Copy | 35 | 23 | 18 | 10 | Partial coverage |
| Size | `Size` (10) | Copy | 30 | 20 | 16 | 10 | Partial coverage |
| Ordering | `Ordering` (11) | Copy | 14 | 13 | 13 | 12 | Good coverage |
| Error | `Error` (8) | Arc | 8 | 8 | 0 | 0 | Typeck+eval only |
| Channel | `Channel` | Arc | 9 | 0 | 0 | 0 | Typeck only |
| Alignment | — | Copy | (enum, no methods) | — | — | — | Not a TypeDef |
| Sign | — | Copy | (enum, no methods) | — | — | — | Not a TypeDef |
| FormatType | — | Copy | (enum, no methods) | — | — | — | Not a TypeDef |
| FormatSpec | — | — | (struct, no methods) | — | — | — | Not a TypeDef |

### Consistency Allowlist Entries to Eliminate

When compound types are in the registry, the following allowlist entries become obsolete:

**TYPECK_METHODS_NOT_IN_IR** (to be eliminated):
- Duration: 24 entries (abs, as_micros, as_millis, as_nanos, as_seconds, format, from_hours, from_micros, from_microseconds, from_millis, from_milliseconds, from_minutes, from_nanos, from_nanoseconds, from_seconds, is_negative, is_positive, is_zero, to_micros, to_millis, to_nanos, to_seconds, zero)
- Size: 17 entries (as_bytes, format, from_bytes, from_gb, from_gigabytes, from_kb, from_kilobytes, from_mb, from_megabytes, from_tb, from_terabytes, is_zero, to_bytes, to_gb, to_kb, to_mb, to_str, to_tb, zero)
- Ordering: 1 entry (then_with)
- Error: 8 entries (clone, debug, has_trace, message, to_str, trace, trace_entries, with_trace)

**EVAL_METHODS_NOT_IN_TYPECK** (to be eliminated):
- Duration: 11 entries (operator methods + aliases dispatched via operator inference)
- Size: 13 entries (operator methods + accessor names)

**TYPECK_METHODS_NOT_IN_EVAL** (to be eliminated):
- Duration: 24 entries (factory methods, conversions, predicates)
- Ordering: 2 entries (then_with, to_str)
- Size: 17 entries (factory methods, conversions, predicates)
- Channel: 9 entries (all methods)
- Error: 8 entries (all methods on separate list)

**Total allowlist entries eliminated by this section: ~134**

### Key Architectural Decisions Needed

1. **Associated functions vs instance methods:** ~~Resolved~~ — Frozen decision 9 (overview) defines `MethodKind` as `Instance | Associated`. Duration and Size factory methods (`from_seconds`, `from_bytes`, etc.) use `kind: MethodKind::Associated` with `receiver: Ownership::Copy` as a placeholder (no receiver).

2. **Method aliases:** Both Duration and Size have multiple names for the same operation (e.g., `to_bytes`/`as_bytes`/`bytes`). Options:
   - A: Registry declares all names as separate methods (simplest, most explicit)
   - B: Registry declares canonical name + aliases list (more structured, enables "did you mean?" suggestions)
   - C: Registry declares canonical name only; phases resolve aliases locally (current eval behavior)

3. **Heterogeneous operators:** Duration.mul takes `int`, not `Duration`. Size.mul takes `int`, not `Size`. The `OpDefs` schema from Section 01 must support heterogeneous operand types, not just `Self`.

4. **Closure parameters:** ~~Partially resolved~~ — Per Section 01 decision 4, there is no `Closure` variant in `ParamDef`. Closure parameters use `ParamDef { ty: ReturnTag::Fresh, ownership: Ownership::Copy }`. The registry's role is to declare the parameter exists; the type checker handles inference. **Important limitation:** `unify_higher_order_constraints` (method_call.rs:165) currently only handles iterator methods (`map`, `flat_map`, `filter`, `any`, `all`, `find`, `for_each`, `fold`, `rfold`). Non-iterator closure methods like `Ordering.then_with` are NOT covered — the closure parameter type is unconstrained. Since `then_with`'s return type is static (`Ordering`), this gap only affects parameter validation, not return type inference. **Follow-up task:** extend `unify_higher_order_constraints` or add a separate builtin-parameter validation pass to constrain `then_with` closure to `() -> Ordering`. This is not blocking for registry migration.

### Implementation Order

Within this section, types should be implemented in this order:

1. **Ordering** — smallest method set, best existing coverage, simplest representation (enum, no operators beyond eq)
2. **Duration** — Copy type with operators, well-tested in eval, representative of the "unit type" pattern
3. **Size** — very similar to Duration, validates that the pattern generalizes
4. **Error** — Arc type, no operators, limited coverage, tests registry with non-Copy memory strategy
5. **Channel** — generic type, zero implementation, tests registry's ability to declare unimplemented APIs

### Exit Criteria

- [ ] All 5 compound types have `TypeDef` entries in `ori_registry`
- [ ] `cargo check -p ori_registry` passes
- [ ] Each TypeDef declares the complete method set (matching typeck's TYPECK_BUILTIN_METHODS entries for that type)
- [ ] Operator methods specify correct OpStrategy (IntInstr for Duration/Size arithmetic)
- [ ] Memory strategy is correct (Copy for Duration/Size/Ordering, Arc for Error/Channel)
- [ ] Associated functions are distinguishable from instance methods
- [ ] Heterogeneous operator parameter types (int for Duration.mul/div, Size.mul/div) are expressible
- [ ] Format spec types have a documented exclusion rationale or stub entries
- [ ] Unit tests verify method counts, trait associations, receiver ownership, and return types
- [ ] All existing tests pass: `cargo test -p ori_registry`
