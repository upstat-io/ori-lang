---
plan: "type_strategy_registry"
section: "03"
title: "Primitive Type Definitions (int, float, bool, byte, char)"
status: not-started
depends_on:
  - "01"
  - "02"
subsections:
  - id: "03.1"
    title: "INT TypeDef"
    status: not-started
  - id: "03.2"
    title: "FLOAT TypeDef"
    status: not-started
  - id: "03.3"
    title: "BOOL TypeDef"
    status: not-started
  - id: "03.4"
    title: "BYTE TypeDef"
    status: not-started
  - id: "03.5"
    title: "CHAR TypeDef"
    status: not-started
  - id: "03.6"
    title: "Validation Against Current Codebase"
    status: not-started
  - id: "03.6a"
    title: "Registry-Internal Tests"
    status: not-started
  - id: "03.6b"
    title: "Cross-Crate Validation Tests (DEFERRED to Section 09/14)"
    status: not-started
  - id: "03.6.1"
    title: "Operator Strategy Correctness Tests"
    status: not-started
  - id: "03.6.2"
    title: "OpDefs Field Coverage Tests"
    status: not-started
  - id: "03.6.3"
    title: "Return Type Accuracy Tests (DEFERRED to Section 09)"
    status: not-started
---

# Section 03: Primitive Type Definitions

## Overview

This section defines the `const TypeDef` entries for the five primitive types: `int`, `float`, `bool`, `byte`, and `char`. All five use `MemoryStrategy::Copy` (bitwise copyable, no ARC overhead, no heap allocation). These are the simplest type definitions in the registry and serve as the template for all subsequent type definitions.

## Design Decisions

### Trait methods are included as regular MethodDefs

Every trait method (`compare`, `equals`, `clone`, `hash`, `debug`, `to_str`) appears as an explicit `MethodDef` entry in the type's method list. There is no separate "trait methods" concept at the registry level. The `trait_name` field on `MethodDef` records the association (e.g., `Some("Comparable")` for `compare`) but the method is listed exactly once, alongside direct methods like `abs` or `is_alpha`.

**Rationale:** The consuming phases (type checker, evaluator, LLVM backend) all need to resolve these methods by name on specific types. A separate trait-method table would force two lookups. The `trait_name` field provides the association for phases that need it (e.g., LLVM codegen's trait dispatch path) without imposing structural complexity.

### Operator methods are included as MethodDefs with OpStrategy

Operator-desugared methods (`add`, `sub`, `mul`, `div`, `rem`, `neg`, `bit_and`, etc.) appear in the method list with their `trait_name` set (e.g., `Some("Add")`). The `OpDefs` on the `TypeDef` separately declares the operator strategy for the *binary operator codegen path* (`emit_binary_op`), which is a separate dispatch from method calls.

**Rationale:** In the current codebase, operators flow through two paths:
1. **Operator inference** (type checker) and **`emit_binary_op`** (LLVM) — dispatches on `BinaryOp` enum directly, uses `is_float`/`is_str` guards.
2. **Trait method calls** (evaluator, LLVM trait dispatch) — resolves `"add"`, `"sub"` etc. as method calls on the receiver type.

The registry unifies both: `OpDefs` feeds path 1, method entries feed path 2.

### Comparison semantics split: signed vs unsigned

The `OpStrategy` for comparison operators differs by type:
- `int` uses `IntInstr` (signed comparison: `icmp slt`, `icmp sgt`, etc.)
- `bool`, `byte`, `char` use `UnsignedCmp` (unsigned: `icmp ult`, `icmp ugt`, etc.)
- `float` uses `FloatInstr` (ordered floating-point: `fcmp olt`, `fcmp ogt`, etc.)

This matches the current LLVM codegen in `traits.rs` where `emit_comparison_predicate` dispatches to `emit_int_predicate` (signed), `emit_unsigned_predicate`, or `emit_float_predicate`.

### MethodDef::new() Signature Convention

The `MethodDef::new()` calls in sections 03.1–03.5 use an **abbreviated 5-parameter form**: `(name, params, returns, trait_name, receiver)`. This is shorthand. The full `MethodDef` struct defined in Section 01 has 10 fields: `name`, `receiver` (Ownership), `params`, `returns`, `trait_name`, `pure`, `backend_required`, `kind`, `dei_only`, `dei_propagation`. For all primitive type methods in this section, the 5 omitted fields have constant values:
- `pure`: `true` (no observable side effects; may panic — see frozen decision below)
- `backend_required`: `true` (primitives must work in both eval and LLVM)
- `kind`: `MethodKind::Instance` (no associated functions on primitives)
- `dei_only`: `false` (not iterator methods)
- `dei_propagation`: `DeiPropagation::NotApplicable` (not iterator adapters)

The abbreviated form is used here for readability. **Implementation MUST define a `const fn` helper** (e.g., `MethodDef::primitive(name, params, returns, trait_name, receiver)`) that fills in the 5 constant fields above. Without this helper, each method literal requires ~12 lines (all 10 fields), and `float.rs` (42 methods x 12 = 504 lines + OpDefs + header) would exceed the 500-line file size limit. The `const fn` helper keeps each method at ~1 line, matching the abbreviated notation shown in this plan.

**Type coercion note:** The code blocks below pass both `TypeTag::Int` and `ReturnTag::SelfType` in the `returns` position. In actual Rust `const fn` code, `TypeTag::Int` must be wrapped as `ReturnTag::Concrete(TypeTag::Int)` because `From` trait conversions are not available in const contexts. The plan omits the wrapper for brevity. The `const fn` helper should accept `ReturnTag` directly.

> **WARNING (BLOAT risk):** If the `const fn` helper is not implemented, `float.rs` WILL exceed 500 lines and `int.rs` (35 methods) will be borderline at ~475 lines. Define the helper in `method.rs` (Section 01/02) BEFORE implementing Section 03.

> **FROZEN DECISION (purity):** `pure: true` means "no observable side effects (no IO, no mutation, no global state) but MAY panic on invalid input." This matches Swift's `readnone` and Lean's purity model. The optimizer MAY reorder, CSE, and hoist pure calls, but MUST NOT eliminate them if reachable (because the panic must fire). All primitive methods are `pure: true`, including `div`, `rem`, `pow`, `floor_div`, `shl`, `shr`. If a future `may_panic` flag is needed for finer-grained optimization, it can be added as a separate field without changing `pure` semantics.

### Param Shorthand in Method Definitions

The `params` argument in `MethodDef::new()` uses `Param::SelfType` as shorthand. Section 01 defines the parameter type as `ParamDef` (a struct with `name`, `ty`, and `ownership` fields). `Param::SelfType` is a convenience constant that should be defined as:

```rust
const SELF_TYPE: ParamDef = ParamDef {
    name: "other",
    ty: ReturnTag::SelfType,
    ownership: Ownership::Copy,  // primitives are value types
};
```

Implementation should define `Param` as a module or namespace providing these constants (e.g., `Param::SELF_TYPE` or `ParamDef::SELF_TYPE`). The plan uses the shorthand `Param::SelfType` for readability.

**Parameter name:** `"other"` is a placeholder. The actual name may vary per method (e.g., `"other"` for `equals`/`compare`, `"exponent"` for `pow`). For registry-level declaration, the exact parameter name is informational (error messages), not semantic.

**Parameter ownership:** The constant above uses `Ownership::Copy` because all primitive `SelfType` parameters are value types. Non-primitive sections (04-07) that use `Param::SelfType` should define their own constant with `Ownership::Borrow` for reference types.

### FROZEN DECISION: Receiver Ownership — Borrow for All Primitives

Section 01 defines `Ownership::Copy` for "value-type receivers" and `Ownership::Borrow` for "reference-type receivers that read without consuming." For primitives (all `MemoryStrategy::Copy`), the runtime behavior is identical: no `rc_inc` or `rc_dec` is emitted in either case. **All primitive method receivers use `Ownership::Borrow`.** This is a frozen decision.

**Rationale:** Using `Borrow` avoids a migration hazard. The ARC pass currently checks `receiver == Borrow` to skip RC operations. If primitives used `Copy` instead, every ARC check would need `receiver == Borrow || receiver == Copy`. Using `Borrow` uniformly means existing `== Borrow` checks work without modification during the wiring phase (Sections 09-13). The `Ownership::Copy` variant remains available for future use (e.g., distinguishing pass-by-value semantics in the optimizer) but is not used by any primitive type definition in this section.

### TypeDef Field Completeness

The `TypeDef` struct (Section 01) has 6 fields: `tag`, `name`, `memory`, `type_params`, `methods`, `operators`. The `const TypeDef` blocks in 03.1–03.5 must include all 6 fields. For all primitive types in this section:
- `type_params`: `TypeParamArity::Fixed(0)` (primitives have no type parameters)

**Note:** The `// Traits: [...]` comments in the `TypeDef` code blocks of 03.1–03.5 are **informational only** — they document which traits each type implements for cross-reference purposes, but traits are NOT a field on the `TypeDef` struct (Section 01). Trait satisfaction is handled by the well_known bitfield system in `ori_types/src/check/well_known/mod.rs`, not by the registry. See "Traits Not Covered by the Registry" below.

### OpDefs Field Completeness

The `OpDefs` struct (Section 01) has **20 fields**. The `not` field (logical NOT, `!x`) is distinct from `bit_not` (bitwise NOT, `~x`). Every OpDefs literal must include all 20 fields. For primitive types:
- `not`: `BoolLogic` for `bool` (only type with logical NOT); `Unsupported` for all others

### Traits Not Covered by the Registry

The following traits are mentioned in the Ori syntax reference for primitives but are **not part of the registry's scope**:

1. **`Default`** — int (default 0), float (default 0.0), bool (default false) implement Default per the well_known bitfield (`REFERENCE_TRUTH` in `tests.rs`). char and byte do NOT implement Default. The `default()` method is an associated function (`Type.default()`), not an instance method, so it does not appear in the method list. Default trait satisfaction is handled by `well_known::type_satisfies_trait()`.

2. **`Formattable`** — All primitives satisfy `Formattable` via a blanket impl (any type implementing `Printable` automatically gets `Formattable`). The `format(spec:)` method is resolved through trait dispatch, not through `resolve_*_method()`. It does NOT appear in `TYPECK_BUILTIN_METHODS` for any primitive. Only `Duration` and `Size` have explicit `format` entries in the type checker.

3. **`Value`** — Primitives are implicitly `Value` (marker trait, no methods). This is auto-derived by the compiler and not part of the well_known bitfield or the method registry.

4. **`Sendable`** — Primitives are implicitly `Sendable` (marker trait, no methods, auto-derived). Not in the REFERENCE_TRUTH for int/float/bool/byte/char (only Duration, Size have it explicitly listed). Handled by the compiler's auto-derivation logic.

5. **`Into`** — int has `into()` returning float, and str has `into()` returning Error. These are registered as direct methods with `trait_name: None` because the builtin `into()` is hardcoded in the type checker, not resolved through trait dispatch. The `Into` trait exists in the stdlib for user-defined types but builtin `into()` bypasses it. See int notes (03.1).

6. **`Pow`** (`**` operator) — `pow` is a direct instance method on int and float (no trait_name). The `**` operator is desugared to `Pow.power()` before reaching the IR (frozen decision 16), so there is no `pow` field in `OpDefs`. The `pow` method listed in the method table is the method-call form, not the operator form.

---

## 03.1 INT TypeDef

**Source of truth locations:**
- Type checker: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` `resolve_int_method()` (lines 167-179)
- TYPECK registry: `compiler/ori_types/src/infer/expr/methods/mod.rs` `TYPECK_BUILTIN_METHODS` int entries (lines 291-313)
- IR registry: `compiler/ori_ir/src/builtin_methods/mod.rs` int section (lines 192-328)
- LLVM primitives: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` (lines 7-14)
- LLVM traits: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` int entries (lines 24-25, 39, 47, 55-58)
- Eval dispatch: `compiler/ori_eval/src/methods/helpers/mod.rs` `EVAL_BUILTIN_METHODS` int entries (lines 162-182)

### Const Definition

```rust
pub const INT: TypeDef = TypeDef {
    tag: TypeTag::Int,
    name: "int",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: &[
        // === Direct methods ===
        MethodDef::new("abs",      &[],                TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("byte",     &[],                TypeTag::Byte,  None,                Ownership::Borrow),
        MethodDef::new("clamp",    &[Param::SelfType, Param::SelfType], TypeTag::Int, None, Ownership::Borrow),
        MethodDef::new("f",        &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("into",     &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("is_even",  &[],                TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("is_negative", &[],             TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("is_odd",   &[],                TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("is_positive", &[],             TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("is_zero",  &[],                TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("max",      &[Param::SelfType], TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("min",      &[Param::SelfType], TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("pow",      &[Param::SelfType], TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("signum",   &[],                TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("to_byte",  &[],                TypeTag::Byte,  None,                Ownership::Borrow),
        MethodDef::new("to_float", &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("to_str",   &[],                TypeTag::Str,   Some("Printable"),   Ownership::Borrow),
        // === Trait methods ===
        MethodDef::new("clone",    &[],                ReturnTag::SelfType, Some("Clone"),     Ownership::Borrow),
        MethodDef::new("compare",  &[Param::SelfType], TypeTag::Ordering, Some("Comparable"), Ownership::Borrow),
        MethodDef::new("debug",    &[],                TypeTag::Str,   Some("Debug"),       Ownership::Borrow),
        MethodDef::new("equals",   &[Param::SelfType], TypeTag::Bool,  Some("Eq"),          Ownership::Borrow),
        MethodDef::new("hash",     &[],                TypeTag::Int,   Some("Hashable"),    Ownership::Borrow),
        // === Operator trait methods ===
        MethodDef::new("add",      &[Param::SelfType], ReturnTag::SelfType, Some("Add"),      Ownership::Borrow),
        MethodDef::new("bit_and",  &[Param::SelfType], ReturnTag::SelfType, Some("BitAnd"),   Ownership::Borrow),
        MethodDef::new("bit_not",  &[],                ReturnTag::SelfType, Some("BitNot"),    Ownership::Borrow),
        MethodDef::new("bit_or",   &[Param::SelfType], ReturnTag::SelfType, Some("BitOr"),    Ownership::Borrow),
        MethodDef::new("bit_xor",  &[Param::SelfType], ReturnTag::SelfType, Some("BitXor"),   Ownership::Borrow),
        MethodDef::new("div",      &[Param::SelfType], ReturnTag::SelfType, Some("Div"),      Ownership::Borrow),
        MethodDef::new("floor_div",&[Param::SelfType], ReturnTag::SelfType, Some("FloorDiv"), Ownership::Borrow),
        MethodDef::new("mul",      &[Param::SelfType], ReturnTag::SelfType, Some("Mul"),      Ownership::Borrow),
        MethodDef::new("neg",      &[],                ReturnTag::SelfType, Some("Neg"),       Ownership::Borrow),
        MethodDef::new("rem",      &[Param::SelfType], ReturnTag::SelfType, Some("Rem"),      Ownership::Borrow),
        MethodDef::new("shl",      &[Param::SelfType], ReturnTag::SelfType, Some("Shl"),      Ownership::Borrow),
        MethodDef::new("shr",      &[Param::SelfType], ReturnTag::SelfType, Some("Shr"),      Ownership::Borrow),
        MethodDef::new("sub",      &[Param::SelfType], ReturnTag::SelfType, Some("Sub"),      Ownership::Borrow),
    ],
    operators: OpDefs {
        add:       OpStrategy::IntInstr,
        sub:       OpStrategy::IntInstr,
        mul:       OpStrategy::IntInstr,
        div:       OpStrategy::IntInstr,
        rem:       OpStrategy::IntInstr,
        floor_div: OpStrategy::IntInstr,
        eq:        OpStrategy::IntInstr,      // icmp eq
        neq:       OpStrategy::IntInstr,      // icmp ne
        lt:        OpStrategy::IntInstr,      // icmp slt (SIGNED)
        gt:        OpStrategy::IntInstr,      // icmp sgt (SIGNED)
        lt_eq:     OpStrategy::IntInstr,      // icmp sle (SIGNED)
        gt_eq:     OpStrategy::IntInstr,      // icmp sge (SIGNED)
        neg:       OpStrategy::IntInstr,
        not:       OpStrategy::Unsupported,
        bit_and:   OpStrategy::IntInstr,
        bit_or:    OpStrategy::IntInstr,
        bit_xor:   OpStrategy::IntInstr,
        bit_not:   OpStrategy::IntInstr,
        shl:       OpStrategy::IntInstr,
        shr:       OpStrategy::IntInstr,
    },
    // Traits: Eq, Comparable, Clone, Hashable, Default, Printable, Debug,
    //         Add, Sub, Mul, Div, FloorDiv, Rem, Neg, BitAnd, BitOr, BitXor, BitNot, Shl, Shr
    // (per REFERENCE_TRUTH in well_known/tests.rs — informational, not a TypeDef field)
};
```

### Notes

**Registry declarations (what the definition above captures):**
- `f()` and `to_float()` are aliases (both return `Float`). `into()` also returns `Float`.
- `byte()` and `to_byte()` are aliases (both return `Byte`).
- All int operators use `IntInstr` (signed arithmetic/comparison).
- `hash()` returns `Int` (identity -- receiver is already `i64`).
- `into()` has `trait_name: None` despite implementing the `Into` trait semantically. The type checker hardcodes `int.into()` -> `Float` in `resolve_int_method()` rather than resolving through trait dispatch.
- `pow()` is a direct instance method (`trait_name: None`), not a trait method. The `**` operator desugars to `Pow.power()` before reaching the IR (frozen decision 16), which is a separate dispatch path from `int.pow(n)` method calls.

**Not declared by the registry:**
- LLVM instruction selection (`sitofp`, `trunc i64 to i8`, `sdiv`, etc.) is a backend concern. The registry declares `IntInstr`; the LLVM backend maps that to specific instructions.
- Trait satisfaction (which traits int implements) is handled by the well_known bitfield, not the registry. See "Traits Not Covered by the Registry" above.

---

## 03.2 FLOAT TypeDef

**Source of truth locations:**
- Type checker: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` `resolve_float_method()` (lines 181-193)
- TYPECK registry: `compiler/ori_types/src/infer/expr/methods/mod.rs` `TYPECK_BUILTIN_METHODS` float entries (lines 253-290)
- IR registry: `compiler/ori_ir/src/builtin_methods/mod.rs` float section (lines 330-431)
- LLVM primitives: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` (lines 16-19)
- LLVM traits: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` float entries (lines 26-27, 40, 48, 59-62)
- Eval dispatch: `compiler/ori_eval/src/methods/helpers/mod.rs` `EVAL_BUILTIN_METHODS` float entries (lines 151-161)

### Const Definition

```rust
pub const FLOAT: TypeDef = TypeDef {
    tag: TypeTag::Float,
    name: "float",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: &[
        // === Direct methods ===
        MethodDef::new("abs",       &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("acos",      &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("asin",      &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("atan",      &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("atan2",     &[Param::SelfType], TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("cbrt",      &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("ceil",      &[],                TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("clamp",     &[Param::SelfType, Param::SelfType], TypeTag::Float, None, Ownership::Borrow),
        MethodDef::new("cos",       &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("exp",       &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("floor",     &[],                TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("is_finite", &[],                TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("is_infinite", &[],              TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("is_nan",    &[],                TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("is_negative", &[],              TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("is_normal", &[],                TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("is_positive", &[],              TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("is_zero",   &[],                TypeTag::Bool,  None,                Ownership::Borrow),
        MethodDef::new("ln",        &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("log10",     &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("log2",      &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("max",       &[Param::SelfType], TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("min",       &[Param::SelfType], TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("pow",       &[Param::SelfType], TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("round",     &[],                TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("signum",    &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("sin",       &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("sqrt",      &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("tan",       &[],                TypeTag::Float, None,                Ownership::Borrow),
        MethodDef::new("to_int",    &[],                TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("to_str",    &[],                TypeTag::Str,   Some("Printable"),   Ownership::Borrow),
        MethodDef::new("trunc",     &[],                TypeTag::Int,   None,                Ownership::Borrow),
        // === Trait methods ===
        MethodDef::new("clone",     &[],                ReturnTag::SelfType, Some("Clone"),     Ownership::Borrow),
        MethodDef::new("compare",   &[Param::SelfType], TypeTag::Ordering, Some("Comparable"), Ownership::Borrow),
        MethodDef::new("debug",     &[],                TypeTag::Str,   Some("Debug"),       Ownership::Borrow),
        MethodDef::new("equals",    &[Param::SelfType], TypeTag::Bool,  Some("Eq"),          Ownership::Borrow),
        MethodDef::new("hash",      &[],                TypeTag::Int,   Some("Hashable"),    Ownership::Borrow),
        // === Operator trait methods ===
        MethodDef::new("add",       &[Param::SelfType], ReturnTag::SelfType, Some("Add"),      Ownership::Borrow),
        MethodDef::new("div",       &[Param::SelfType], ReturnTag::SelfType, Some("Div"),      Ownership::Borrow),
        MethodDef::new("mul",       &[Param::SelfType], ReturnTag::SelfType, Some("Mul"),      Ownership::Borrow),
        MethodDef::new("neg",       &[],                ReturnTag::SelfType, Some("Neg"),       Ownership::Borrow),
        MethodDef::new("rem",       &[Param::SelfType], ReturnTag::SelfType, Some("Rem"),      Ownership::Borrow),
        MethodDef::new("sub",       &[Param::SelfType], ReturnTag::SelfType, Some("Sub"),      Ownership::Borrow),
    ],
    operators: OpDefs {
        add:       OpStrategy::FloatInstr,
        sub:       OpStrategy::FloatInstr,
        mul:       OpStrategy::FloatInstr,
        div:       OpStrategy::FloatInstr,
        rem:       OpStrategy::FloatInstr,      // frem — LLVM handles it; registry proactively enables it (trait_set.rs to be updated in Section 09)
        floor_div: OpStrategy::Unsupported,     // float has no floor_div operator
        eq:        OpStrategy::FloatInstr,      // fcmp oeq
        neq:       OpStrategy::FloatInstr,      // fcmp one
        lt:        OpStrategy::FloatInstr,      // fcmp olt (ordered)
        gt:        OpStrategy::FloatInstr,      // fcmp ogt (ordered)
        lt_eq:     OpStrategy::FloatInstr,      // fcmp ole (ordered)
        gt_eq:     OpStrategy::FloatInstr,      // fcmp oge (ordered)
        neg:       OpStrategy::FloatInstr,      // fneg
        not:       OpStrategy::Unsupported,
        bit_and:   OpStrategy::Unsupported,
        bit_or:    OpStrategy::Unsupported,
        bit_xor:   OpStrategy::Unsupported,
        bit_not:   OpStrategy::Unsupported,
        shl:       OpStrategy::Unsupported,
        shr:       OpStrategy::Unsupported,
    },
    // Traits: Eq, Comparable, Clone, Hashable, Default, Printable, Debug,
    //         Add, Sub, Mul, Div, Rem, Neg
    // (per REFERENCE_TRUTH + rem proactive addition — informational, not a TypeDef field)
};
```

### Notes

**Registry declarations:**
- `floor()`, `ceil()`, `round()`, `trunc()`, `to_int()` all return `TypeTag::Int` (not `Float`). This matches the type checker (`resolve_float_method` returns `Idx::INT` for these).
- `hash()` returns `TypeTag::Int`. Implementation note: hash uses `+/-0` normalization (IEEE 754 requires `hash(+0.0) == hash(-0.0)` because `+0.0 == -0.0`).
- `rem` is `OpStrategy::FloatInstr` — the LLVM backend already handles `BinaryOp::Mod` for float via `frem`. The registry proactively enables it; `trait_set.rs` will be updated to include REM for float during Section 09 wiring.
- `floor_div` is `OpStrategy::Unsupported` — floor division is integer-only in Ori.
- No bitwise operators (float bits are not directly manipulable in Ori).
- `Hashable` trait methods (`hash`) are included in the registry even though `ori_ir` `BUILTIN_METHODS` does not yet list them. The type checker and LLVM backend both support `float.hash()`.

**Known discrepancy to fix during migration (Section 13):**
- `ori_ir` `BUILTIN_METHODS` declares `floor`, `ceil`, `round` with `ReturnSpec::SelfType` (meaning Float), but the type checker returns `Idx::INT`. The registry uses `TypeTag::Int` (matching the type checker). This ori_ir discrepancy will be resolved in Section 13.

---

## 03.3 BOOL TypeDef

**Source of truth locations:**
- Type checker: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` `resolve_bool_method()` (lines 315-323)
- TYPECK registry: `compiler/ori_types/src/infer/expr/methods/mod.rs` `TYPECK_BUILTIN_METHODS` bool entries (lines 207-213)
- IR registry: `compiler/ori_ir/src/builtin_methods/mod.rs` bool section (lines 432-446)
- LLVM primitives: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` (lines 21-23)
- LLVM traits: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` bool entries (lines 28-29, 41, 49, 63-66)
- Eval dispatch: `compiler/ori_eval/src/methods/helpers/mod.rs` `EVAL_BUILTIN_METHODS` bool entries (lines 120-126)

### Const Definition

```rust
pub const BOOL: TypeDef = TypeDef {
    tag: TypeTag::Bool,
    name: "bool",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: &[
        // === Direct methods ===
        MethodDef::new("to_int",   &[],                TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("to_str",   &[],                TypeTag::Str,   Some("Printable"),   Ownership::Borrow),
        // === Trait methods ===
        MethodDef::new("clone",    &[],                ReturnTag::SelfType, Some("Clone"),     Ownership::Borrow),
        MethodDef::new("compare",  &[Param::SelfType], TypeTag::Ordering, Some("Comparable"), Ownership::Borrow),
        MethodDef::new("debug",    &[],                TypeTag::Str,   Some("Debug"),       Ownership::Borrow),
        MethodDef::new("equals",   &[Param::SelfType], TypeTag::Bool,  Some("Eq"),          Ownership::Borrow),
        MethodDef::new("hash",     &[],                TypeTag::Int,   Some("Hashable"),    Ownership::Borrow),
        // === Operator trait method ===
        MethodDef::new("not",      &[],                TypeTag::Bool,  Some("Not"),         Ownership::Borrow),
    ],
    operators: OpDefs {
        add:       OpStrategy::Unsupported,
        sub:       OpStrategy::Unsupported,
        mul:       OpStrategy::Unsupported,
        div:       OpStrategy::Unsupported,
        rem:       OpStrategy::Unsupported,
        floor_div: OpStrategy::Unsupported,
        eq:        OpStrategy::BoolLogic,     // icmp eq (i1)
        neq:       OpStrategy::BoolLogic,     // icmp ne (i1)
        lt:        OpStrategy::UnsignedCmp,   // icmp ult (false < true)
        gt:        OpStrategy::UnsignedCmp,   // icmp ugt
        lt_eq:     OpStrategy::UnsignedCmp,   // icmp ule
        gt_eq:     OpStrategy::UnsignedCmp,   // icmp uge
        neg:       OpStrategy::Unsupported,
        not:       OpStrategy::BoolLogic,  
        bit_and:   OpStrategy::Unsupported,   // logical && is short-circuit, not a method
        bit_or:    OpStrategy::Unsupported,
        bit_xor:   OpStrategy::Unsupported,
        bit_not:   OpStrategy::Unsupported,
        shl:       OpStrategy::Unsupported,
        shr:       OpStrategy::Unsupported,
    },
    // Traits: Eq, Comparable, Clone, Hashable, Default, Printable, Debug, Not
    // (per REFERENCE_TRUTH — informational, not a TypeDef field)
};
```

### Notes

**Registry declarations:**
- Bool has minimal methods: `to_int`, `to_str`, trait methods, and `not` (operator trait).
- No arithmetic operators.
- Equality uses `BoolLogic`; ordering comparisons use `UnsignedCmp` (`false < true` maps to `0 < 1`).
- `not` appears both as a `MethodDef` (for trait dispatch `bool_value.not()`) and as `OpDefs.not = BoolLogic` (for `!x` operator syntax). These are the same operation accessed through two dispatch paths.
- `to_int()` is included in the registry even though it is absent from `ori_ir` `BUILTIN_METHODS` (listed in `TYPECK_METHODS_NOT_IN_IR`).

---

## 03.4 BYTE TypeDef

**Source of truth locations:**
- Type checker: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` `resolve_byte_method()` (lines 325-337)
- TYPECK registry: `compiler/ori_types/src/infer/expr/methods/mod.rs` `TYPECK_BUILTIN_METHODS` byte entries (lines 214-226)
- IR registry: `compiler/ori_ir/src/builtin_methods/mod.rs` byte section (lines 454-460)
- LLVM primitives: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` (lines 28-29)
- LLVM traits: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` byte entries (lines 32-33, 43, 51, 71-74)
- Eval dispatch: `compiler/ori_eval/src/methods/helpers/mod.rs` `EVAL_BUILTIN_METHODS` byte entries (lines 128-133)

### Const Definition

```rust
pub const BYTE: TypeDef = TypeDef {
    tag: TypeTag::Byte,
    name: "byte",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: &[
        // === Direct methods ===
        MethodDef::new("is_ascii",            &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_ascii_alpha",      &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_ascii_digit",      &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_ascii_whitespace", &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("to_char",  &[],                TypeTag::Char,  None,                Ownership::Borrow),
        MethodDef::new("to_int",   &[],                TypeTag::Int,   None,                Ownership::Borrow),
        MethodDef::new("to_str",   &[],                TypeTag::Str,   Some("Printable"),   Ownership::Borrow),
        // === Trait methods ===
        MethodDef::new("clone",    &[],                ReturnTag::SelfType, Some("Clone"),     Ownership::Borrow),
        MethodDef::new("compare",  &[Param::SelfType], TypeTag::Ordering, Some("Comparable"), Ownership::Borrow),
        MethodDef::new("debug",    &[],                TypeTag::Str,   Some("Debug"),       Ownership::Borrow),
        MethodDef::new("equals",   &[Param::SelfType], TypeTag::Bool,  Some("Eq"),          Ownership::Borrow),
        MethodDef::new("hash",     &[],                TypeTag::Int,   Some("Hashable"),    Ownership::Borrow),
        // === Operator trait methods ===
        MethodDef::new("add",      &[Param::SelfType], ReturnTag::SelfType, Some("Add"),      Ownership::Borrow),
        MethodDef::new("bit_and",  &[Param::SelfType], ReturnTag::SelfType, Some("BitAnd"),   Ownership::Borrow),
        MethodDef::new("bit_not",  &[],                ReturnTag::SelfType, Some("BitNot"),    Ownership::Borrow),
        MethodDef::new("bit_or",   &[Param::SelfType], ReturnTag::SelfType, Some("BitOr"),    Ownership::Borrow),
        MethodDef::new("bit_xor",  &[Param::SelfType], ReturnTag::SelfType, Some("BitXor"),   Ownership::Borrow),
        MethodDef::new("div",      &[Param::SelfType], ReturnTag::SelfType, Some("Div"),      Ownership::Borrow),
        MethodDef::new("mul",      &[Param::SelfType], ReturnTag::SelfType, Some("Mul"),      Ownership::Borrow),
        MethodDef::new("rem",      &[Param::SelfType], ReturnTag::SelfType, Some("Rem"),      Ownership::Borrow),
        MethodDef::new("shl",      &[Param::SelfType], ReturnTag::SelfType, Some("Shl"),      Ownership::Borrow),
        MethodDef::new("shr",      &[Param::SelfType], ReturnTag::SelfType, Some("Shr"),      Ownership::Borrow),
        MethodDef::new("sub",      &[Param::SelfType], ReturnTag::SelfType, Some("Sub"),      Ownership::Borrow),
    ],
    operators: OpDefs {
        add:       OpStrategy::IntInstr,      // byte arithmetic uses i8 add
        sub:       OpStrategy::IntInstr,
        mul:       OpStrategy::IntInstr,
        div:       OpStrategy::IntInstr,
        rem:       OpStrategy::IntInstr,
        floor_div: OpStrategy::Unsupported,
        eq:        OpStrategy::IntInstr,      // icmp eq (i8)
        neq:       OpStrategy::IntInstr,      // icmp ne (i8)
        lt:        OpStrategy::UnsignedCmp,   // icmp ult (UNSIGNED — byte is 0-255)
        gt:        OpStrategy::UnsignedCmp,   // icmp ugt
        lt_eq:     OpStrategy::UnsignedCmp,   // icmp ule
        gt_eq:     OpStrategy::UnsignedCmp,   // icmp uge
        neg:       OpStrategy::Unsupported,   // byte is unsigned, no negation
        not:       OpStrategy::Unsupported,
        bit_and:   OpStrategy::IntInstr,      // i8 bitwise AND
        bit_or:    OpStrategy::IntInstr,      // i8 bitwise OR
        bit_xor:   OpStrategy::IntInstr,      // i8 bitwise XOR
        bit_not:   OpStrategy::IntInstr,      // i8 bitwise NOT
        shl:       OpStrategy::IntInstr,      // i8 shift left
        shr:       OpStrategy::IntInstr,      // i8 shift right
    },
    // Traits: Eq, Comparable, Clone, Hashable, Printable, Debug,
    //         Add, Sub, Mul, Div, Rem, BitAnd, BitOr, BitXor, BitNot, Shl, Shr
    // (per REFERENCE_TRUTH — informational, not a TypeDef field; note: NO Default on byte)
};
```

### Notes

**Registry declarations:**
- Byte is unsigned (range 0-255). Arithmetic (`+`, `-`, `*`, `/`, `%`) and bitwise (`&`, `|`, `^`, `~`, `<<`, `>>`) operators are supported. Comparison uses `UnsignedCmp`. No `floor_div` or `neg` (byte is unsigned).
- `to_char()` converts byte value to Unicode codepoint (valid for ASCII range 0-127).
- `is_ascii`, `is_ascii_alpha`, `is_ascii_digit`, `is_ascii_whitespace` are predicate methods returning `Bool`.
- The `ori_ir` `BUILTIN_METHODS` currently only has trait methods for byte. The registry adds the direct methods (`to_int`, `to_char`, `is_ascii_*`) and all operator trait methods.

---

## 03.5 CHAR TypeDef

**Source of truth locations:**
- Type checker: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` `resolve_char_method()` (lines 339-349)
- TYPECK registry: `compiler/ori_types/src/infer/expr/methods/mod.rs` `TYPECK_BUILTIN_METHODS` char entries (lines 227-243)
- IR registry: `compiler/ori_ir/src/builtin_methods/mod.rs` char section (lines 447-453)
- LLVM primitives: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` (lines 25-26)
- LLVM traits: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` char entries (lines 30-31, 42, 50, 67-70)
- Eval dispatch: `compiler/ori_eval/src/methods/helpers/mod.rs` `EVAL_BUILTIN_METHODS` char entries (lines 135-140)

### Const Definition

```rust
pub const CHAR: TypeDef = TypeDef {
    tag: TypeTag::Char,
    name: "char",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: &[
        // === Direct methods ===
        MethodDef::new("is_alpha",     &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_ascii",     &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_digit",     &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_lowercase", &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_uppercase", &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_whitespace",&[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("to_byte",      &[], TypeTag::Byte, None, Ownership::Borrow),
        MethodDef::new("to_int",       &[], TypeTag::Int,  None, Ownership::Borrow),
        MethodDef::new("to_lowercase", &[], TypeTag::Char, None, Ownership::Borrow),
        MethodDef::new("to_str",       &[], TypeTag::Str,  Some("Printable"), Ownership::Borrow),
        MethodDef::new("to_uppercase", &[], TypeTag::Char, None, Ownership::Borrow),
        // === Trait methods ===
        MethodDef::new("clone",    &[],                ReturnTag::SelfType, Some("Clone"),     Ownership::Borrow),
        MethodDef::new("compare",  &[Param::SelfType], TypeTag::Ordering, Some("Comparable"), Ownership::Borrow),
        MethodDef::new("debug",    &[],                TypeTag::Str,   Some("Debug"),       Ownership::Borrow),
        MethodDef::new("equals",   &[Param::SelfType], TypeTag::Bool,  Some("Eq"),          Ownership::Borrow),
        MethodDef::new("hash",     &[],                TypeTag::Int,   Some("Hashable"),    Ownership::Borrow),
    ],
    operators: OpDefs {
        add:       OpStrategy::Unsupported,   // no char arithmetic
        sub:       OpStrategy::Unsupported,
        mul:       OpStrategy::Unsupported,
        div:       OpStrategy::Unsupported,
        rem:       OpStrategy::Unsupported,
        floor_div: OpStrategy::Unsupported,
        eq:        OpStrategy::IntInstr,      // icmp eq (i32 — Unicode scalar)
        neq:       OpStrategy::IntInstr,      // icmp ne (i32)
        lt:        OpStrategy::UnsignedCmp,   // icmp ult (Unicode codepoint ordering)
        gt:        OpStrategy::UnsignedCmp,   // icmp ugt
        lt_eq:     OpStrategy::UnsignedCmp,   // icmp ule
        gt_eq:     OpStrategy::UnsignedCmp,   // icmp uge
        neg:       OpStrategy::Unsupported,
        not:       OpStrategy::Unsupported,
        bit_and:   OpStrategy::Unsupported,
        bit_or:    OpStrategy::Unsupported,
        bit_xor:   OpStrategy::Unsupported,
        bit_not:   OpStrategy::Unsupported,
        shl:       OpStrategy::Unsupported,
        shr:       OpStrategy::Unsupported,
    },
    // Traits: Eq, Comparable, Clone, Hashable, Printable, Debug
    // (per REFERENCE_TRUTH — informational, not a TypeDef field; note: NO Default on char)
};
```

### Notes

**Registry declarations:**
- Char represents a Unicode scalar value (U+0000 to U+10FFFF). No arithmetic operators.
- Rich predicate methods: `is_alpha`, `is_digit`, `is_whitespace`, `is_uppercase`, `is_lowercase`, `is_ascii`.
- Case conversion methods: `to_uppercase`, `to_lowercase` (return `Char`).
- `to_byte()` returns `TypeTag::Byte`. The current type checker returns `Idx::INT` (widened for arithmetic), but the registry fixes this semantic inconsistency — `to_byte()` should return `Byte`. The type checker's `resolve_char_method` must be updated to return `Idx::BYTE` during Section 09 wiring.
- `compare` uses `UnsignedCmp` (Unicode codepoint ordering is unsigned).
- The `ori_ir` `BUILTIN_METHODS` currently only has trait methods for char. The registry adds all direct methods.

---

## 03.6 Validation Against Current Codebase

### Cross-Reference Table

This table verifies every method on the five primitive types across all four compiler phases plus the proposed registry.

**Column definitions:**
- **TC**: Type checker -- `TYPECK_BUILTIN_METHODS` + `resolve_*_method()` in ori_types
- **EV**: Evaluator -- `EVAL_BUILTIN_METHODS` in ori_eval
- **IR**: IR registry -- `BUILTIN_METHODS` in ori_ir
- **LL**: LLVM backend -- `declare_builtins!` registrations in ori_llvm
- **REG**: Proposed registry definition (this section)

**Legend:**
- **Y** = present in the phase's direct method list
- **-** = absent from the phase entirely
- **R** = present but dispatched via a method resolver function (not in the flat dispatch list)
- **O** = present but dispatched via operator inference (not in the method dispatch list; the type checker resolves operators like `+` through `BinaryOp` dispatch, not method lookup)

#### INT

| Method | TC | EV | IR | LL | REG | Notes |
|--------|----|----|----|----|-----|-------|
| `abs` | Y | R | Y | Y | Y | Eval uses method resolver |
| `add` | O | Y | Y | - | Y | TC uses operator inference; LL uses emit_binary_op |
| `bit_and` | O | Y | Y | - | Y | |
| `bit_not` | O | Y | Y | - | Y | |
| `bit_or` | O | Y | Y | - | Y | |
| `bit_xor` | O | Y | Y | - | Y | |
| `byte` | Y | - | - | Y | Y | Alias for to_byte |
| `clamp` | Y | - | - | - | Y | Not yet in IR/Eval/LLVM |
| `clone` | Y | Y | Y | Y | Y | All phases |
| `compare` | Y | Y | Y | Y | Y | All phases |
| `debug` | Y | Y | Y | - | Y | Not in LLVM builtins (falls through to runtime) |
| `div` | O | Y | Y | - | Y | |
| `equals` | Y | Y | Y | Y | Y | All phases |
| `f` | Y | - | - | Y | Y | Alias for to_float; LLVM has it |
| `floor_div` | O | Y | Y | - | Y | |
| `hash` | Y | Y | Y | Y | Y | All phases |
| `into` | Y | Y | - | Y | Y | Not in IR; LLVM maps to sitofp |
| `is_equal` | - | - | - | Y | - | LLVM trait method (alias for equals) |
| `is_even` | Y | - | - | - | Y | Typeck only |
| `is_greater` | - | - | - | Y | - | LLVM comparison predicate |
| `is_greater_or_equal` | - | - | - | Y | - | LLVM comparison predicate |
| `is_less` | - | - | - | Y | - | LLVM comparison predicate |
| `is_less_or_equal` | - | - | - | Y | - | LLVM comparison predicate |
| `is_negative` | Y | - | - | - | Y | Typeck only |
| `is_odd` | Y | - | - | - | Y | Typeck only |
| `is_positive` | Y | - | - | - | Y | Typeck only |
| `is_zero` | Y | - | - | - | Y | Typeck only |
| `max` | Y | R | Y | - | Y | Eval uses method resolver |
| `min` | Y | R | Y | - | Y | Eval uses method resolver |
| `mul` | O | Y | Y | - | Y | |
| `neg` | O | Y | Y | - | Y | |
| `pow` | Y | - | - | - | Y | Typeck only |
| `rem` | O | Y | Y | - | Y | |
| `shl` | O | Y | Y | - | Y | |
| `shr` | O | Y | Y | - | Y | |
| `signum` | Y | - | - | - | Y | Typeck only |
| `sub` | O | Y | Y | - | Y | |
| `to_byte` | Y | - | - | - | Y | Not in eval/IR/LLVM |
| `to_float` | Y | - | - | Y | Y | LLVM has it |
| `to_int` | - | - | - | Y | - | LLVM primitives (identity for int) |
| `to_str` | Y | Y | Y | Y | Y | All phases |

#### FLOAT

| Method | TC | EV | IR | LL | REG | Notes |
|--------|----|----|----|----|-----|-------|
| `abs` | Y | R | Y | Y | Y | |
| `acos` | Y | - | - | - | Y | Typeck only |
| `add` | O | Y | Y | - | Y | |
| `asin` | Y | - | - | - | Y | Typeck only |
| `atan` | Y | - | - | - | Y | Typeck only |
| `atan2` | Y | - | - | - | Y | Typeck only |
| `cbrt` | Y | - | - | - | Y | Typeck only |
| `ceil` | Y | R | Y | - | Y | IR says SelfType (bug) |
| `clamp` | Y | - | - | - | Y | Typeck only |
| `clone` | Y | Y | Y | Y | Y | |
| `compare` | Y | Y | Y | Y | Y | |
| `cos` | Y | - | - | - | Y | Typeck only |
| `debug` | Y | Y | Y | - | Y | |
| `div` | O | Y | Y | - | Y | |
| `equals` | Y | Y | Y | Y | Y | |
| `exp` | Y | - | - | - | Y | Typeck only |
| `floor` | Y | R | Y | - | Y | IR says SelfType (bug) |
| `hash` | Y | Y | - | Y | Y | Not in IR yet |
| `is_equal` | - | - | - | Y | - | LLVM trait method (alias for equals) |
| `is_finite` | Y | - | - | - | Y | Typeck only |
| `is_greater` | - | - | - | Y | - | LLVM comparison predicate |
| `is_greater_or_equal` | - | - | - | Y | - | LLVM comparison predicate |
| `is_infinite` | Y | - | - | - | Y | Typeck only |
| `is_less` | - | - | - | Y | - | LLVM comparison predicate |
| `is_less_or_equal` | - | - | - | Y | - | LLVM comparison predicate |
| `is_nan` | Y | - | - | - | Y | Typeck only |
| `is_negative` | Y | - | - | - | Y | Typeck only |
| `is_normal` | Y | - | - | - | Y | Typeck only |
| `is_positive` | Y | - | - | - | Y | Typeck only |
| `is_zero` | Y | - | - | - | Y | Typeck only |
| `ln` | Y | - | - | - | Y | Typeck only |
| `log10` | Y | - | - | - | Y | Typeck only |
| `log2` | Y | - | - | - | Y | Typeck only |
| `max` | Y | R | Y | - | Y | |
| `min` | Y | R | Y | - | Y | |
| `mul` | O | Y | Y | - | Y | |
| `neg` | O | Y | Y | - | Y | |
| `pow` | Y | - | - | - | Y | Typeck only |
| `rem` | - | - | - | - | Y | Proactive addition; LLVM handles frem; trait_set.rs to be updated in Section 09 |
| `round` | Y | R | Y | - | Y | IR says SelfType (bug) |
| `signum` | Y | - | - | - | Y | Typeck only |
| `sin` | Y | - | - | - | Y | Typeck only |
| `sqrt` | Y | R | Y | - | Y | |
| `sub` | O | Y | Y | - | Y | |
| `tan` | Y | - | - | - | Y | Typeck only |
| `to_int` | Y | - | - | Y | Y | |
| `to_str` | Y | Y | Y | Y | Y | |
| `trunc` | Y | - | - | - | Y | Typeck only |

#### BOOL

| Method | TC | EV | IR | LL | REG | Notes |
|--------|----|----|----|----|-----|-------|
| `clone` | Y | Y | Y | Y | Y | |
| `compare` | Y | Y | Y | Y | Y | |
| `debug` | Y | Y | Y | - | Y | |
| `equals` | Y | Y | Y | Y | Y | |
| `hash` | Y | Y | Y | Y | Y | |
| `is_equal` | - | - | - | Y | - | LLVM trait method (alias for equals) |
| `is_greater` | - | - | - | Y | - | LLVM comparison predicate |
| `is_greater_or_equal` | - | - | - | Y | - | LLVM comparison predicate |
| `is_less` | - | - | - | Y | - | LLVM comparison predicate |
| `is_less_or_equal` | - | - | - | Y | - | LLVM comparison predicate |
| `not` | O | Y | Y | - | Y | Operator trait |
| `to_int` | Y | - | - | Y | Y | LLVM has it |
| `to_str` | Y | Y | Y | Y | Y | |

#### BYTE

| Method | TC | EV | IR | LL | REG | Notes |
|--------|----|----|----|----|-----|-------|
| `add` | O | - | - | - | Y | Operator trait (byte arithmetic) |
| `bit_and` | O | - | - | - | Y | Operator trait (byte bitwise) |
| `bit_not` | O | - | - | - | Y | Operator trait (byte bitwise) |
| `bit_or` | O | - | - | - | Y | Operator trait (byte bitwise) |
| `bit_xor` | O | - | - | - | Y | Operator trait (byte bitwise) |
| `clone` | Y | Y | Y | Y | Y | |
| `compare` | Y | Y | Y | Y | Y | |
| `debug` | Y | Y | Y | - | Y | |
| `div` | O | - | - | - | Y | Operator trait (byte arithmetic) |
| `equals` | Y | Y | Y | Y | Y | |
| `hash` | Y | Y | Y | Y | Y | |
| `is_ascii` | Y | - | - | - | Y | Typeck only |
| `is_ascii_alpha` | Y | - | - | - | Y | Typeck only |
| `is_ascii_digit` | Y | - | - | - | Y | Typeck only |
| `is_ascii_whitespace` | Y | - | - | - | Y | Typeck only |
| `is_equal` | - | - | - | Y | - | LLVM trait method (alias for equals) |
| `is_greater` | - | - | - | Y | - | LLVM comparison predicate |
| `is_greater_or_equal` | - | - | - | Y | - | LLVM comparison predicate |
| `is_less` | - | - | - | Y | - | LLVM comparison predicate |
| `is_less_or_equal` | - | - | - | Y | - | LLVM comparison predicate |
| `mul` | O | - | - | - | Y | Operator trait (byte arithmetic) |
| `rem` | O | - | - | - | Y | Operator trait (byte arithmetic) |
| `shl` | O | - | - | - | Y | Operator trait (byte bitwise) |
| `shr` | O | - | - | - | Y | Operator trait (byte bitwise) |
| `sub` | O | - | - | - | Y | Operator trait (byte arithmetic) |
| `to_char` | Y | - | - | - | Y | Typeck only |
| `to_int` | Y | - | - | Y | Y | LLVM has it |
| `to_str` | Y | Y | Y | - | Y | |

#### CHAR

| Method | TC | EV | IR | LL | REG | Notes |
|--------|----|----|----|----|-----|-------|
| `clone` | Y | Y | Y | Y | Y | |
| `compare` | Y | Y | Y | Y | Y | |
| `debug` | Y | Y | Y | - | Y | |
| `equals` | Y | Y | Y | Y | Y | |
| `hash` | Y | Y | Y | Y | Y | |
| `is_alpha` | Y | - | - | - | Y | Typeck only |
| `is_ascii` | Y | - | - | - | Y | Typeck only |
| `is_digit` | Y | - | - | - | Y | Typeck only |
| `is_equal` | - | - | - | Y | - | LLVM trait method (alias for equals) |
| `is_greater` | - | - | - | Y | - | LLVM comparison predicate |
| `is_greater_or_equal` | - | - | - | Y | - | LLVM comparison predicate |
| `is_less` | - | - | - | Y | - | LLVM comparison predicate |
| `is_less_or_equal` | - | - | - | Y | - | LLVM comparison predicate |
| `is_lowercase` | Y | - | - | - | Y | Typeck only |
| `is_uppercase` | Y | - | - | - | Y | Typeck only |
| `is_whitespace` | Y | - | - | - | Y | Typeck only |
| `to_byte` | Y | - | - | - | Y | Typeck returns Int (bug); registry returns Byte (fix in Section 09) |
| `to_int` | Y | - | - | Y | Y | LLVM has it |
| `to_lowercase` | Y | - | - | - | Y | Typeck only |
| `to_str` | Y | Y | Y | - | Y | |
| `to_uppercase` | Y | - | - | - | Y | Typeck only |

### Method Count Summary

LL counts include both `primitives.rs` entries and `traits.rs` entries (equals, compare, hash, comparison predicates like `is_less`/`is_greater`/etc., and `is_equal`). These LLVM-only comparison predicate methods are NOT included in the REG count because they are not part of the proposed registry schema (they are LLVM-specific trait dispatch aliases, not user-facing methods).

| Type | TC Methods | EV Methods | IR Methods | LL Methods | REG Methods |
|------|-----------|-----------|-----------|-----------|-------------|
| int | 22 | 20 | 22 | 16 | 35 |
| float | 37 | 11 | 17 | 12 | 43 |
| bool | 7 | 7 | 7 | 11 | 8 |
| byte | 12 | 6 | 6 | 10 | 23 |
| char | 16 | 6 | 6 | 10 | 16 |
| **Total** | **94** | **50** | **58** | **59** | **125** |

The registry (125 entries) is the superset of TC + operator methods. It captures every method from every phase. Methods that exist only in the type checker today will gain eval/LLVM implementations over time, but the registry declares them from day one. The LLVM backend additionally has comparison predicate methods (`is_less`, `is_greater`, etc.) and `is_equal` that are trait dispatch aliases — these are not separate user-facing methods and thus not counted in REG.

### Known Discrepancies to Resolve

1. **float.floor/ceil/round return type** (ori_ir): IR says `SelfType` (Float), typeck says `Int`. Registry uses `Int`. Fix IR when migrating (Section 13).
2. **float.hash** (ori_ir): Not in IR `BUILTIN_METHODS`. Registry includes it. Add to IR in Section 13.
3. **char.to_byte return type** (BREAKING FIX): Typeck currently returns `Idx::INT`, but the registry uses `TypeTag::Byte` — the correct return type for a method named `to_byte()`. During Section 09 wiring, `resolve_char_method("to_byte")` must change from `Idx::INT` to `Idx::BYTE`. This is a minor semantic fix: code like `ch.to_byte() + 1` would need `ch.to_byte().to_int() + 1`. Update eval and LLVM codegen paths in the same commit.
4. **Operator methods not in LLVM builtins**: Operators (`add`, `sub`, etc.) are not in LLVM's `declare_builtins!` because they flow through `emit_binary_op`, not method dispatch. The registry captures both paths.
5. **LLVM comparison predicate methods**: The LLVM backend registers `is_less`, `is_greater`, `is_less_or_equal`, `is_greater_or_equal`, and `is_equal` for all scalar types in `traits.rs`. These are not in the type checker's `TYPECK_BUILTIN_METHODS`, the evaluator's `EVAL_BUILTIN_METHODS`, or the IR's `BUILTIN_METHODS`. They are LLVM-specific trait dispatch aliases (the type checker desugars comparison operators to `compare()` + `Ordering` predicate calls). The registry does NOT include these as separate `MethodDef` entries — they are codegen implementation details, not user-facing methods.
6. **int.to_int in LLVM**: The LLVM `primitives.rs` registers `("int", "to_int")` (identity operation). This is not in the type checker or evaluator for int (it's redundant). The registry does not include it.
7. **bool `not` operator alignment**: The LLVM backend handles `!x` for bool via `builder.not()` (`xor x, true`). The `not` method on bool (trait_name `"Not"`) appears as a `MethodDef` in the method list AND as `OpDefs.not = BoolLogic` in the operator table. These are the same operation accessed through two dispatch paths: the method table serves trait dispatch (`bool_value.not()`), while `OpDefs.not` serves `emit_unary_op()` for the `!` operator syntax.
8. **Default trait not uniform**: int, float, and bool implement Default (with values 0, 0.0, false respectively). char and byte do NOT implement Default per the well_known REFERENCE_TRUTH table. This is intentional — there is no obvious "default" char or byte value.
9. **float.rem proactive addition** (BEHAVIOR CHANGE): The registry declares `float.rem = FloatInstr` and includes a `rem` operator trait method, but `trait_set.rs` does not currently register REM for float. The LLVM backend already handles `BinaryOp::Mod` for float via `frem`. During Section 09 wiring, `trait_set.rs` must add REM to float's operator traits so that `5.0 % 2.0` type-checks. Add tests for edge cases: `NaN % x`, `x % 0.0` (IEEE 754: returns NaN), `Inf % x`.

---

## Implementation Tasks

### Prerequisites (must be complete before starting 03.1-03.5)

- [ ] Section 01: `MethodDef` struct with all 10 fields defined
- [ ] Section 01: `ParamDef` struct defined, plus `ParamDef::SELF_TYPE` convenience constant (used as `Param::SelfType` shorthand in this section's method definitions)
- [ ] Section 01/02: `const fn MethodDef::primitive(name, params, returns, trait_name, receiver) -> MethodDef` helper defined — fills `pure: true`, `backend_required: true`, `kind: Instance`, `dei_only: false`, `dei_propagation: NotApplicable`. **Without this helper, `float.rs` will exceed the 500-line file size limit.**
- [ ] Section 02: `ori_registry` crate scaffolding complete (`Cargo.toml`, `lib.rs`, `defs/mod.rs`)

### 03.1 INT TypeDef
- [ ] Create `ori_registry/src/defs/int.rs`
- [ ] Define `pub const INT: TypeDef` with all 35 methods and `type_params: TypeParamArity::Fixed(0)`
- [ ] Define `OpDefs` with all 20 fields: `IntInstr` for arithmetic/bitwise, `Unsupported` for `not`
- [ ] Verify all methods from `resolve_int_method()` are present (cross-crate, deferred to Section 09)
- [ ] Verify all int entries from `TYPECK_BUILTIN_METHODS` are present (cross-crate, deferred to Section 09)

### 03.2 FLOAT TypeDef

> **WARNING (BLOAT):** `float.rs` has 42 methods — the most of any primitive. Without the `const fn` helper from Section 01/02, this file will be ~559 lines and violate the 500-line limit. If the helper cannot be implemented, split into `defs/float/mod.rs` + `defs/float/methods.rs`.

- [ ] Create `ori_registry/src/defs/float.rs`
- [ ] Define `pub const FLOAT: TypeDef` with all 43 methods and `type_params: TypeParamArity::Fixed(0)`
- [ ] Define `OpDefs` with all 20 fields: `FloatInstr` for arithmetic/comparison/rem, `Unsupported` for bitwise/not/floor_div
- [ ] Verify all methods from `resolve_float_method()` are present (cross-crate, deferred to Section 09)
- [ ] Verify all float entries from `TYPECK_BUILTIN_METHODS` are present (cross-crate, deferred to Section 09)
- [ ] Document floor/ceil/round return type discrepancy with ori_ir

### 03.3 BOOL TypeDef
- [ ] Create `ori_registry/src/defs/bool.rs`
- [ ] Define `pub const BOOL: TypeDef` with all 8 methods and `type_params: TypeParamArity::Fixed(0)`
- [ ] Define `OpDefs` with all 20 fields: `BoolLogic` for eq/neq/not, `UnsignedCmp` for ordering, `Unsupported` for arithmetic/bitwise
- [ ] Verify all methods from `resolve_bool_method()` are present (cross-crate, deferred to Section 09)
- [ ] Verify all bool entries from `TYPECK_BUILTIN_METHODS` are present (cross-crate, deferred to Section 09)

### 03.4 BYTE TypeDef
- [ ] Create `ori_registry/src/defs/byte.rs`
- [ ] Define `pub const BYTE: TypeDef` with all 23 methods (12 direct/trait + 11 operator) and `type_params: TypeParamArity::Fixed(0)`
- [ ] Define `OpDefs` with all 20 fields: `IntInstr` for arithmetic/bitwise, `UnsignedCmp` for ordering, `Unsupported` for floor_div/neg/not
- [ ] Verify all methods from `resolve_byte_method()` are present (cross-crate, deferred to Section 09)
- [ ] Verify all byte entries from `TYPECK_BUILTIN_METHODS` are present (cross-crate, deferred to Section 09)

### 03.5 CHAR TypeDef
- [ ] Create `ori_registry/src/defs/char.rs`
- [ ] Define `pub const CHAR: TypeDef` with all 16 methods and `type_params: TypeParamArity::Fixed(0)`
- [ ] Define `OpDefs` with all 20 fields: `IntInstr` for eq/neq, `UnsignedCmp` for ordering, `Unsupported` for arithmetic/bitwise/not
- [ ] Verify all methods from `resolve_char_method()` are present (cross-crate, deferred to Section 09)
- [ ] Verify all char entries from `TYPECK_BUILTIN_METHODS` are present (cross-crate, deferred to Section 09)

### 03.6 Validation

**Test file location:** `ori_registry/src/defs/tests.rs` (sibling tests.rs convention). Declare `#[cfg(test)] mod tests;` at the bottom of `ori_registry/src/defs/mod.rs`.

#### 03.6a Registry-Internal Tests (in `ori_registry/src/defs/tests.rs`)

These tests verify internal consistency of the registry data and require NO cross-crate dependencies:

- [ ] Write `#[test] fn no_duplicate_methods()` — verify no `TypeDef` has duplicate method names
- [ ] Write `#[test] fn all_primitives_are_copy()` — verify `MemoryStrategy::Copy` for all five types
- [ ] Write `#[test] fn all_primitives_have_zero_type_params()` — verify `TypeParamArity::Fixed(0)` for all five types
- [ ] Write `#[test] fn all_methods_have_names()` — verify no method has an empty name string
- [ ] Write `#[test] fn methods_alphabetically_sorted()` — verify methods within each TypeDef are sorted by name (matches `TYPECK_BUILTIN_METHODS` convention)
- [ ] Verify `cargo c -p ori_registry` passes with all definitions
- [ ] Verify `cargo test -p ori_registry` passes

#### 03.6b Cross-Crate Validation Tests (DEFERRED to Section 09/14)

> **IMPORTANT:** `ori_registry` has zero dependencies. Tests that reference `TYPECK_BUILTIN_METHODS`, `resolve_*_method()`, `EVAL_BUILTIN_METHODS`, or LLVM backend data **cannot live in `ori_registry`**. These cross-crate validation tests must live in a crate that depends on both `ori_registry` and the target crate — either `oric/tests/` (integration tests) or in the wiring sections (09-13) where the dependency exists.

The following tests are deferred to Section 09 (Wire Type Checker) or Section 14 (Enforcement):

- [ ] Write `int_methods_match_typeck()` — iterate `INT.methods`, verify each name appears in `TYPECK_BUILTIN_METHODS` int entries **(Section 09 or 14, in `oric/tests/`)**
- [ ] Write `float_methods_match_typeck()` — same for float **(Section 09 or 14)**
- [ ] Write `bool_methods_match_typeck()` — same for bool **(Section 09 or 14)**
- [ ] Write `byte_methods_match_typeck()` — same for byte **(Section 09 or 14)**
- [ ] Write `char_methods_match_typeck()` — same for char **(Section 09 or 14)**
- [ ] Write `all_typeck_primitives_in_registry()` — iterate `TYPECK_BUILTIN_METHODS` for int/float/bool/byte/char, verify each appears in the corresponding `TypeDef` **(Section 09 or 14)**

### 03.6.1 Operator Strategy Correctness Tests

**Test file location:** Same `ori_registry/src/defs/tests.rs` as 03.6a — these are registry-internal (no cross-crate deps).

These tests verify that the `OpDefs` entries produce correct LLVM semantics (signed vs unsigned comparison, correct instruction choice). They are critical because an incorrect strategy would silently generate wrong code.

- [ ] Write `#[test] fn int_comparison_is_signed()` — verify `INT.operators.lt == IntInstr` (signed `icmp slt`, not `UnsignedCmp`)
- [ ] Write `#[test] fn byte_comparison_is_unsigned()` — verify `BYTE.operators.lt == UnsignedCmp` (unsigned `icmp ult`, not `IntInstr`)
- [ ] Write `#[test] fn char_comparison_is_unsigned()` — verify `CHAR.operators.lt == UnsignedCmp`
- [ ] Write `#[test] fn bool_comparison_is_unsigned()` — verify `BOOL.operators.lt == UnsignedCmp` (false < true)
- [ ] Write `#[test] fn bool_equality_is_bool_logic()` — verify `BOOL.operators.eq == BoolLogic`
- [ ] Write `#[test] fn float_comparison_is_float_instr()` — verify `FLOAT.operators.lt == FloatInstr` (ordered `fcmp olt`)
- [ ] Write `#[test] fn bool_not_is_bool_logic()` — verify `BOOL.operators.not == BoolLogic`
- [ ] Write `#[test] fn non_bool_not_is_unsupported()` — verify `INT/FLOAT/BYTE/CHAR.operators.not == Unsupported`
- [ ] Write `#[test] fn float_has_no_bitwise_ops()` — verify all bitwise OpDefs fields are `Unsupported` for float
- [ ] Write `#[test] fn float_has_no_rem_or_floor_div()` — verify `FLOAT.operators.rem == Unsupported` and `FLOAT.operators.floor_div == Unsupported`
- [ ] Write `#[test] fn char_has_no_arithmetic()` — verify add/sub/mul/div/rem/floor_div are all `Unsupported` for char
- [ ] Write `#[test] fn byte_has_no_neg()` — verify `BYTE.operators.neg == Unsupported` (unsigned type)

### 03.6.2 OpDefs Field Coverage Tests

These tests ensure the registry's `OpDefs` correctly captures every operator that the LLVM backend handles per type, and that no `Unsupported` entry contradicts what `emit_binary_op` actually supports.

#### Registry-internal (in `ori_registry/src/defs/tests.rs`):

- [ ] Write `#[test] fn opdefs_has_all_20_fields()` — compile-time or runtime verification that every `OpDefs` block has exactly 20 fields (add, sub, mul, div, rem, floor_div, eq, neq, lt, gt, lt_eq, gt_eq, neg, not, bit_and, bit_or, bit_xor, bit_not, shl, shr)

#### Cross-crate (DEFERRED to Section 12 or 14, in `oric/tests/` or `ori_llvm/tests/`):

> **IMPORTANT:** These tests reference LLVM backend internals (`emit_binary_op`, `emit_unary_op`) and cannot live in `ori_registry` (zero dependencies).

- [ ] Write `non_unsupported_ops_match_llvm_emit_binary_op()` — for each type, collect all OpDefs fields that are NOT `Unsupported`, verify each has a corresponding handler in `emit_binary_op` (or `emit_unary_op`) in the LLVM backend. **(Section 12 or 14, in `oric/tests/` or `ori_llvm/tests/`)**
- [ ] Write `llvm_handled_ops_not_unsupported_in_registry()` — inverse: for each type the LLVM backend handles in `emit_binary_op`, verify the registry does NOT mark that operator as `Unsupported`. **(Section 12 or 14)**

### 03.6.3 Return Type Accuracy Tests

These tests verify that the registry's `returns` field matches the actual type checker's `resolve_*_method()` return values. This catches discrepancies like the float floor/ceil/round return type bug.

> **IMPORTANT:** These tests reference `resolve_*_method()` functions from `ori_types` and cannot live in `ori_registry` (zero dependencies). They must be implemented in Section 09 (Wire Type Checker) where the `ori_types` -> `ori_registry` dependency exists, or in `oric/tests/` integration tests.

- [ ] Write `int_method_return_types_match_resolver()` — for each method in `INT.methods` that has a concrete `TypeTag` return, verify it matches `resolve_int_method(name)` mapping **(Section 09, in `ori_types/` or `oric/tests/`)**
- [ ] Write `float_method_return_types_match_resolver()` — same for float (critical: verifies floor/ceil/round return `Int`) **(Section 09)**
- [ ] Write `bool_method_return_types_match_resolver()` — same for bool **(Section 09)**
- [ ] Write `byte_method_return_types_match_resolver()` — same for byte **(Section 09)**
- [ ] Write `char_method_return_types_match_resolver()` — same for char **(Section 09)**

---

## Exit Criteria

### Section 03 Completion (verifiable at implementation time)

- [ ] `cargo c -p ori_registry` compiles successfully
- [ ] `cargo test -p ori_registry` passes (registry-internal tests only)
- [ ] `./test-all.sh` passes (no regressions in existing crates)
- [ ] All five `TypeDef` constants (`INT`, `FLOAT`, `BOOL`, `BYTE`, `CHAR`) are defined and exported
- [ ] Every `TypeDef` has all 6 struct fields: `tag`, `name`, `memory`, `type_params`, `methods`, `operators`
- [ ] Every `OpDefs` has all 20 fields (including `not`)
- [ ] `OpDefs` correctly distinguishes `IntInstr` vs `FloatInstr` vs `UnsignedCmp` vs `BoolLogic` vs `Unsupported` for each operator on each type
- [ ] `BOOL.operators.not == BoolLogic`; all other types have `not == Unsupported`
- [ ] No duplicate method names within any `TypeDef`
- [ ] All five types have `MemoryStrategy::Copy`
- [ ] All five types have `TypeParamArity::Fixed(0)`
- [ ] Operator strategy correctness tests pass (signed/unsigned/float semantics)
- [ ] No source file exceeds 500 lines (excluding test files)
- [ ] Test file uses sibling `tests.rs` convention (`ori_registry/src/defs/tests.rs`)
- [ ] Cross-reference table in this document matches the implemented definitions

### Deferred Exit Criteria (verified in Sections 09-14)

The following require cross-crate dependencies and CANNOT be verified until wiring sections:

- [ ] Every method from `resolve_int_method()` (22 methods) appears in `INT.methods` **(Section 09)**
- [ ] Every method from `resolve_float_method()` (37 methods) appears in `FLOAT.methods`, plus 5 operator trait methods (add, div, mul, neg, sub) = 42 total **(Section 09)**
- [ ] Every method from `resolve_bool_method()` (7 methods) appears in `BOOL.methods` **(Section 09)**
- [ ] Every method from `resolve_byte_method()` (12 methods) appears in `BYTE.methods`, plus 11 operator trait methods (add, sub, mul, div, rem, bit_and, bit_or, bit_xor, bit_not, shl, shr) = 23 total **(Section 09)**
- [ ] Every method from `resolve_char_method()` (16 methods) appears in `CHAR.methods` **(Section 09)**
- [ ] All `TYPECK_BUILTIN_METHODS` entries for these five types are accounted for in the registry **(Section 09)**
- [ ] Return type accuracy tests pass (registry matches `resolve_*_method()`) **(Section 09)**
