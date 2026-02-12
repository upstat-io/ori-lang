---
title: "Constant Folding"
description: "Ori Compiler Design — Compile-Time Constant Evaluation"
order: 653
section: "Canonicalization"
---

# Constant Folding

The constant folding step evaluates compile-time-known expressions during canonicalization and replaces them with `CanExpr::Constant(ConstantId)` references into the `ConstantPool`.

## ConstantPool

The `ConstantPool` is a content-addressed store of compile-time values:

```rust
pub struct ConstantPool {
    values: Vec<ConstValue>,
    // Content-hash dedup: identical values share one ConstantId
}
```

### Pre-Interned Sentinels

Common constants are pre-interned for O(1) access:

| Sentinel | Value | ConstantId |
|----------|-------|------------|
| `unit` | `()` | 0 |
| `true` | `true` | 1 |
| `false` | `false` | 2 |
| `0` | `0` (int) | 3 |
| `1` | `1` (int) | 4 |
| `""` | empty string | 5 |

### ConstValue

```rust
pub enum ConstValue {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Byte(u8),
    Char(char),
}
```

## What Gets Folded

Expressions that are fully known at compile time:

| Expression | Folded To |
|-----------|-----------|
| `1 + 2` | `Constant(id)` → `ConstValue::Int(3)` |
| `true && false` | `Constant(id)` → `ConstValue::Bool(false)` |
| `"hello" ++ " world"` | `Constant(id)` → `ConstValue::Str("hello world")` |
| `-42` | `Constant(id)` → `ConstValue::Int(-42)` |

## Design Rationale

- **Evaluator benefit**: Skip runtime computation for known values
- **Codegen benefit**: LLVM can emit constants directly (no runtime evaluation)
- **Content dedup**: Identical constant values share storage, reducing memory
- **Phase correctness**: Folding runs after type checking, so all types are resolved — no risk of folding type-incorrect expressions
