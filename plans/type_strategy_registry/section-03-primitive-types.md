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

---

## 03.1 INT TypeDef

**Source of truth locations:**
- Type checker: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` `resolve_int_method()` (lines 613-625)
- IR registry: `compiler/ori_ir/src/builtin_methods/mod.rs` int section (lines 192-328)
- LLVM primitives: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` (lines 7-14)
- LLVM traits: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` int entries
- Eval dispatch: `compiler/ori_eval/src/methods/helpers/mod.rs` `EVAL_BUILTIN_METHODS` int entries

### Const Definition

```rust
pub const INT: TypeDef = TypeDef {
    tag: TypeTag::Int,
    name: "int",
    memory: MemoryStrategy::Copy,
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
        bit_and:   OpStrategy::IntInstr,
        bit_or:    OpStrategy::IntInstr,
        bit_xor:   OpStrategy::IntInstr,
        bit_not:   OpStrategy::IntInstr,
        shl:       OpStrategy::IntInstr,
        shr:       OpStrategy::IntInstr,
    },
    traits: &["Eq", "Clone", "Hashable", "Printable", "Debug", "Comparable"],
};
```

### Notes

- `f()` and `to_float()` are aliases (both return `Float`, both map to `sitofp`). `into()` is also an alias for `sitofp`.
- `byte()` and `to_byte()` are aliases (both return `Byte`, both map to `trunc i64 to i8`).
- All int operators use signed arithmetic/comparison (`sdiv`, `srem`, `icmp slt`, etc.).
- `hash()` is identity (receiver is already `i64`).
- All methods borrow receiver (primitives are `Copy`, so borrow is semantically trivial but signals no ownership transfer).

---

## 03.2 FLOAT TypeDef

**Source of truth locations:**
- Type checker: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` `resolve_float_method()` (lines 627-639)
- IR registry: `compiler/ori_ir/src/builtin_methods/mod.rs` float section (lines 330-431)
- LLVM primitives: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` (lines 16-19)
- LLVM traits: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` float entries
- Eval dispatch: `compiler/ori_eval/src/methods/helpers/mod.rs` `EVAL_BUILTIN_METHODS` float entries

### Const Definition

```rust
pub const FLOAT: TypeDef = TypeDef {
    tag: TypeTag::Float,
    name: "float",
    memory: MemoryStrategy::Copy,
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
        MethodDef::new("sub",       &[Param::SelfType], ReturnTag::SelfType, Some("Sub"),      Ownership::Borrow),
    ],
    operators: OpDefs {
        add:       OpStrategy::FloatInstr,
        sub:       OpStrategy::FloatInstr,
        mul:       OpStrategy::FloatInstr,
        div:       OpStrategy::FloatInstr,
        rem:       OpStrategy::FloatInstr,      // frem
        floor_div: OpStrategy::Unsupported,     // float has no floor_div operator
        eq:        OpStrategy::FloatInstr,      // fcmp oeq
        neq:       OpStrategy::FloatInstr,      // fcmp one
        lt:        OpStrategy::FloatInstr,      // fcmp olt (ordered)
        gt:        OpStrategy::FloatInstr,      // fcmp ogt (ordered)
        lt_eq:     OpStrategy::FloatInstr,      // fcmp ole (ordered)
        gt_eq:     OpStrategy::FloatInstr,      // fcmp oge (ordered)
        neg:       OpStrategy::FloatInstr,      // fneg
        bit_and:   OpStrategy::Unsupported,
        bit_or:    OpStrategy::Unsupported,
        bit_xor:   OpStrategy::Unsupported,
        bit_not:   OpStrategy::Unsupported,
        shl:       OpStrategy::Unsupported,
        shr:       OpStrategy::Unsupported,
    },
    traits: &["Eq", "Clone", "Hashable", "Printable", "Debug", "Comparable"],
};
```

### Notes

- `floor()`, `ceil()`, `round()`, `trunc()`, `to_int()` all return `Int` (not `Float`). The current `ori_ir` registry incorrectly declares `floor`/`ceil`/`round` as `ReturnSpec::SelfType` (Float). The registry will fix this by using `TypeTag::Int`, matching the type checker.
- `hash()` uses `+/-0` normalization + bitcast to `i64` (IEEE 754 semantics require `hash(+0.0) == hash(-0.0)` because `+0.0 == -0.0`).
- No `rem` operator in the IR's float section — but the LLVM `emit_binary_op` does handle `BinaryOp::Mod` for float (`frem`). The registry will include it.
- No bitwise operators (float bits are not directly manipulable in Ori).
- No `floor_div` operator (floor division is integer-only in Ori).
- `float` does NOT have `Hashable` in the ori_ir `BUILTIN_METHODS` yet (listed in `TYPECK_METHODS_NOT_IN_IR`). The registry will include it since the type checker and LLVM backend both support it.

### Discrepancy: ori_ir floor/ceil/round return type

The `ori_ir` `BUILTIN_METHODS` declares `floor`, `ceil`, `round` with `ReturnSpec::SelfType` (meaning Float), but the type checker returns `Idx::INT` for these methods. The correct behavior is returning `Int` (truncation/rounding to integer). The registry definition uses `TypeTag::Int`, which matches the type checker. When ori_ir is migrated (Section 13), this discrepancy will be resolved.

---

## 03.3 BOOL TypeDef

**Source of truth locations:**
- Type checker: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` `resolve_bool_method()` (lines 761-769)
- IR registry: `compiler/ori_ir/src/builtin_methods/mod.rs` bool section (lines 432-446)
- LLVM primitives: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` (lines 21-23)
- LLVM traits: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` bool entries
- Eval dispatch: `compiler/ori_eval/src/methods/helpers/mod.rs` `EVAL_BUILTIN_METHODS` bool entries

### Const Definition

```rust
pub const BOOL: TypeDef = TypeDef {
    tag: TypeTag::Bool,
    name: "bool",
    memory: MemoryStrategy::Copy,
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
        bit_and:   OpStrategy::Unsupported,   // logical && is short-circuit, not a method
        bit_or:    OpStrategy::Unsupported,
        bit_xor:   OpStrategy::Unsupported,
        bit_not:   OpStrategy::Unsupported,
        shl:       OpStrategy::Unsupported,
        shr:       OpStrategy::Unsupported,
    },
    traits: &["Eq", "Clone", "Hashable", "Printable", "Debug", "Comparable"],
};
```

### Notes

- Bool has minimal methods: `to_int` (zero-extend `i1` to `i64`), `to_str` (runtime call), and trait methods.
- `not` operator trait method is present (logical negation of `i1`).
- No arithmetic operators.
- `compare` uses unsigned comparison (`false < true` maps to `0 < 1`), matching the LLVM codegen in `emit_compare` which dispatches Bool to unsigned.
- Equality uses `BoolLogic` (direct `icmp eq` on `i1` values).
- Ordering comparisons use `UnsignedCmp` (`icmp ult`, `icmp ugt`, etc.), matching the LLVM `emit_unsigned_predicate` path.
- `hash()` = `zext i1 to i64` (0 or 1).
- `to_int()` is in `TYPECK_BUILTIN_METHODS` and LLVM primitives but listed in `TYPECK_METHODS_NOT_IN_IR`. The registry includes it.

---

## 03.4 BYTE TypeDef

**Source of truth locations:**
- Type checker: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` `resolve_byte_method()` (lines 771-783)
- IR registry: `compiler/ori_ir/src/builtin_methods/mod.rs` byte section (lines 454-460)
- LLVM primitives: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` (lines 28-29)
- LLVM traits: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` byte entries
- Eval dispatch: `compiler/ori_eval/src/methods/helpers/mod.rs` `EVAL_BUILTIN_METHODS` byte entries

### Const Definition

```rust
pub const BYTE: TypeDef = TypeDef {
    tag: TypeTag::Byte,
    name: "byte",
    memory: MemoryStrategy::Copy,
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
        bit_and:   OpStrategy::Unsupported,
        bit_or:    OpStrategy::Unsupported,
        bit_xor:   OpStrategy::Unsupported,
        bit_not:   OpStrategy::Unsupported,
        shl:       OpStrategy::Unsupported,
        shr:       OpStrategy::Unsupported,
    },
    traits: &["Eq", "Clone", "Hashable", "Printable", "Debug", "Comparable"],
};
```

### Notes

- Byte is `i8` (unsigned semantics, range 0-255). Arithmetic operators exist in the type checker (operators like `+`, `-`, `*`, `/`, `%` work on byte values) but comparison uses unsigned semantics.
- `to_int()` = `zext i8 to i64` (zero-extend, since byte is unsigned).
- `to_char()` converts byte value to Unicode codepoint (only valid for ASCII range 0-127, wraps/truncates for 128-255).
- `is_ascii`, `is_ascii_alpha`, `is_ascii_digit`, `is_ascii_whitespace` are predicate methods.
- `hash()` = `zext i8 to i64`.
- `compare` uses unsigned comparison (`emit_icmp_ordering` with `signed=false`), matching the LLVM codegen.
- The IR registry (ori_ir) currently only has trait methods for byte (compare, equals, clone, hash, to_str, debug). The registry adds the direct methods (`to_int`, `to_char`, `is_ascii_*`).
- No bitwise operators on byte (unlike int). No negation.

---

## 03.5 CHAR TypeDef

**Source of truth locations:**
- Type checker: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` `resolve_char_method()` (lines 785-795)
- IR registry: `compiler/ori_ir/src/builtin_methods/mod.rs` char section (lines 447-453)
- LLVM primitives: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs` (lines 25-26)
- LLVM traits: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs` char entries
- Eval dispatch: `compiler/ori_eval/src/methods/helpers/mod.rs` `EVAL_BUILTIN_METHODS` char entries

### Const Definition

```rust
pub const CHAR: TypeDef = TypeDef {
    tag: TypeTag::Char,
    name: "char",
    memory: MemoryStrategy::Copy,
    methods: &[
        // === Direct methods ===
        MethodDef::new("is_alpha",     &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_ascii",     &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_digit",     &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_lowercase", &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_uppercase", &[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("is_whitespace",&[], TypeTag::Bool, None, Ownership::Borrow),
        MethodDef::new("to_byte",      &[], TypeTag::Int,  None, Ownership::Borrow),
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
        bit_and:   OpStrategy::Unsupported,
        bit_or:    OpStrategy::Unsupported,
        bit_xor:   OpStrategy::Unsupported,
        bit_not:   OpStrategy::Unsupported,
        shl:       OpStrategy::Unsupported,
        shr:       OpStrategy::Unsupported,
    },
    traits: &["Eq", "Clone", "Hashable", "Printable", "Debug", "Comparable"],
};
```

### Notes

- Char is `i32` internally (Unicode scalar value, U+0000 to U+10FFFF).
- No arithmetic operators (char + char is not meaningful).
- Rich predicate methods: `is_alpha`, `is_digit`, `is_whitespace`, `is_uppercase`, `is_lowercase`, `is_ascii`.
- Case conversion methods: `to_uppercase`, `to_lowercase` (return `Char`).
- `to_int()` = `sext i32 to i64` (sign-extend, since Unicode scalars fit in unsigned 21 bits but the `i32` representation is sign-extended for consistency with the `i64` int type).
- `to_byte()` returns `Int` in the type checker (`resolve_char_method` maps it to `Idx::INT`). This is a type checker design choice (byte value widened to int for arithmetic compatibility). The registry follows the type checker.
- `hash()` = `sext i32 to i64` (matches LLVM codegen in `emit_hash`).
- `compare` uses unsigned comparison (Unicode codepoint ordering is unsigned).
- The IR registry (ori_ir) currently only has trait methods for char. The registry adds all direct methods.

---

## 03.6 Validation Against Current Codebase

### Cross-Reference Table

This table verifies every method on the five primitive types across all four compiler phases. Phases:
- **TC**: `TYPECK_BUILTIN_METHODS` + `resolve_*_method()` (ori_types)
- **EV**: `EVAL_BUILTIN_METHODS` (ori_eval)
- **IR**: `BUILTIN_METHODS` (ori_ir)
- **LL**: LLVM `declare_builtins!` registrations (ori_llvm)
- **REG**: Proposed registry definition (this section)

Legend: Y = present, `-` = absent, `R` = dispatched via method resolver (not in direct dispatch list), `O` = dispatched via operator inference (not in method dispatch list)

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
| `is_even` | Y | - | - | - | Y | Typeck only |
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
| `is_finite` | Y | - | - | - | Y | Typeck only |
| `is_infinite` | Y | - | - | - | Y | Typeck only |
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
| `not` | O | Y | Y | - | Y | Operator trait |
| `to_int` | Y | - | - | Y | Y | LLVM has it |
| `to_str` | Y | Y | Y | Y | Y | |

#### BYTE

| Method | TC | EV | IR | LL | REG | Notes |
|--------|----|----|----|----|-----|-------|
| `clone` | Y | Y | Y | Y | Y | |
| `compare` | Y | Y | Y | Y | Y | |
| `debug` | Y | Y | Y | - | Y | |
| `equals` | Y | Y | Y | Y | Y | |
| `hash` | Y | Y | Y | Y | Y | |
| `is_ascii` | Y | - | - | - | Y | Typeck only |
| `is_ascii_alpha` | Y | - | - | - | Y | Typeck only |
| `is_ascii_digit` | Y | - | - | - | Y | Typeck only |
| `is_ascii_whitespace` | Y | - | - | - | Y | Typeck only |
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
| `is_lowercase` | Y | - | - | - | Y | Typeck only |
| `is_uppercase` | Y | - | - | - | Y | Typeck only |
| `is_whitespace` | Y | - | - | - | Y | Typeck only |
| `to_byte` | Y | - | - | - | Y | Returns Int per typeck |
| `to_int` | Y | - | - | Y | Y | LLVM has it |
| `to_lowercase` | Y | - | - | - | Y | Typeck only |
| `to_str` | Y | Y | Y | - | Y | |
| `to_uppercase` | Y | - | - | - | Y | Typeck only |

### Method Count Summary

| Type | TC Methods | EV Methods | IR Methods | LL Methods | REG Methods |
|------|-----------|-----------|-----------|-----------|-------------|
| int | 22 | 20 | 20 | 11 | 35 |
| float | 37 | 11 | 13 | 7 | 37 |
| bool | 7 | 7 | 7 | 6 | 8 |
| byte | 11 | 6 | 6 | 4 | 12 |
| char | 16 | 6 | 6 | 4 | 16 |
| **Total** | **93** | **50** | **52** | **32** | **108** |

The registry (108 entries) is the superset. It captures every method from every phase. Methods that exist only in the type checker today will gain eval/LLVM implementations over time, but the registry declares them from day one.

### Known Discrepancies to Resolve

1. **float.floor/ceil/round return type** (ori_ir): IR says `SelfType` (Float), typeck says `Int`. Registry uses `Int`. Fix IR when migrating (Section 13).
2. **float.hash** (ori_ir): Not in IR `BUILTIN_METHODS`. Registry includes it. Add to IR in Section 13.
3. **char.to_byte return type**: Typeck returns `Idx::INT`, not `Idx::BYTE`. This is intentional (byte value widened for arithmetic). Registry uses `TypeTag::Int` to match typeck. LLVM would use the same `sext` or `trunc` path.
4. **Operator methods not in LLVM builtins**: Operators (`add`, `sub`, etc.) are not in LLVM's `declare_builtins!` because they flow through `emit_binary_op`, not method dispatch. The registry captures both paths.

---

## Implementation Tasks

### 03.1 INT TypeDef
- [ ] Create `ori_registry/src/defs/int.rs`
- [ ] Define `pub const INT: TypeDef` with all 35 methods
- [ ] Define `OpDefs` with `IntInstr` for all arithmetic and bitwise operators
- [ ] Verify all methods from `resolve_int_method()` are present
- [ ] Verify all int entries from `TYPECK_BUILTIN_METHODS` are present

### 03.2 FLOAT TypeDef
- [ ] Create `ori_registry/src/defs/float.rs`
- [ ] Define `pub const FLOAT: TypeDef` with all 37 methods
- [ ] Define `OpDefs` with `FloatInstr` for arithmetic, `Unsupported` for bitwise
- [ ] Verify all methods from `resolve_float_method()` are present
- [ ] Verify all float entries from `TYPECK_BUILTIN_METHODS` are present
- [ ] Document floor/ceil/round return type discrepancy with ori_ir

### 03.3 BOOL TypeDef
- [ ] Create `ori_registry/src/defs/bool.rs`
- [ ] Define `pub const BOOL: TypeDef` with all 8 methods
- [ ] Define `OpDefs` with `BoolLogic` for eq/neq, `UnsignedCmp` for ordering, `Unsupported` for arithmetic
- [ ] Verify all methods from `resolve_bool_method()` are present
- [ ] Verify all bool entries from `TYPECK_BUILTIN_METHODS` are present

### 03.4 BYTE TypeDef
- [ ] Create `ori_registry/src/defs/byte.rs`
- [ ] Define `pub const BYTE: TypeDef` with all 12 methods
- [ ] Define `OpDefs` with `IntInstr` for arithmetic, `UnsignedCmp` for ordering
- [ ] Verify all methods from `resolve_byte_method()` are present
- [ ] Verify all byte entries from `TYPECK_BUILTIN_METHODS` are present

### 03.5 CHAR TypeDef
- [ ] Create `ori_registry/src/defs/char.rs`
- [ ] Define `pub const CHAR: TypeDef` with all 16 methods
- [ ] Define `OpDefs` with `IntInstr` for eq/neq, `UnsignedCmp` for ordering, `Unsupported` for arithmetic
- [ ] Verify all methods from `resolve_char_method()` are present
- [ ] Verify all char entries from `TYPECK_BUILTIN_METHODS` are present

### 03.6 Validation
- [ ] Write `#[test] fn int_methods_match_typeck()` — iterate `INT.methods`, verify each name appears in `TYPECK_BUILTIN_METHODS` int entries
- [ ] Write `#[test] fn float_methods_match_typeck()` — same for float
- [ ] Write `#[test] fn bool_methods_match_typeck()` — same for bool
- [ ] Write `#[test] fn byte_methods_match_typeck()` — same for byte
- [ ] Write `#[test] fn char_methods_match_typeck()` — same for char
- [ ] Write `#[test] fn all_typeck_primitives_in_registry()` — iterate `TYPECK_BUILTIN_METHODS` for int/float/bool/byte/char, verify each appears in the corresponding `TypeDef`
- [ ] Write `#[test] fn no_duplicate_methods()` — verify no `TypeDef` has duplicate method names
- [ ] Write `#[test] fn all_primitives_are_copy()` — verify `MemoryStrategy::Copy` for all five types
- [ ] Verify `cargo c -p ori_registry` passes with all definitions

---

## Exit Criteria

- [ ] `cargo c -p ori_registry` compiles successfully
- [ ] All five `TypeDef` constants (`INT`, `FLOAT`, `BOOL`, `BYTE`, `CHAR`) are defined and exported
- [ ] Every method from `resolve_int_method()` (22 methods) appears in `INT.methods`
- [ ] Every method from `resolve_float_method()` (37 methods) appears in `FLOAT.methods`
- [ ] Every method from `resolve_bool_method()` (7 methods) appears in `BOOL.methods`
- [ ] Every method from `resolve_byte_method()` (11 methods) appears in `BYTE.methods`
- [ ] Every method from `resolve_char_method()` (16 methods) appears in `CHAR.methods`
- [ ] All `TYPECK_BUILTIN_METHODS` entries for these five types are accounted for in the registry
- [ ] `OpDefs` correctly distinguishes `IntInstr` vs `FloatInstr` vs `UnsignedCmp` vs `BoolLogic` vs `Unsupported` for each operator on each type
- [ ] No duplicate method names within any `TypeDef`
- [ ] All five types have `MemoryStrategy::Copy`
- [ ] Validation tests pass in `cargo test -p ori_registry`
- [ ] Cross-reference table in this document matches the implemented definitions
