---
title: "Desugaring"
description: "Ori Compiler Design — Syntactic Sugar Elimination"
order: 651
section: "Canonicalization"
---

# Desugaring

The desugaring step eliminates 7 syntactic sugar variants from the AST, producing canonical expressions that backends never need to handle. This is a mechanical, type-preserving transformation — no semantic analysis occurs.

## Sugar Variants Eliminated

### 1. Named Calls → Positional Calls

```ori
// Source (sugar)
fetch(url: "https://example.com", timeout: 5s)

// Canonical (desugared)
fetch("https://example.com", 5s)
```

`ExprKind::CallNamed` becomes `CanExpr::Call` with arguments reordered to match the function signature.

### 2. Named Method Calls → Positional Method Calls

```ori
// Source (sugar)
list.filter(predicate: x -> x > 0)

// Canonical (desugared)
list.filter(x -> x > 0)
```

`ExprKind::MethodCallNamed` becomes `CanExpr::MethodCall` with positional arguments.

### 3. Template Literals → String Concatenation

```ori
// Source (sugar)
"Hello, {name}! You are {age} years old."

// Canonical (desugared)
"Hello, " ++ str(name) ++ "! You are " ++ str(age) ++ " years old."
```

`ExprKind::TemplateLiteral` and `TemplateFull` become chains of `CanExpr::BinaryOp(Concat)`.

### 4. List Spread → Method Calls

```ori
// Source (sugar)
[...existing, new_item]

// Canonical (desugared)
existing.append(new_item)
```

`ExprKind::ListWithSpread` becomes method calls on list operations.

### 5. Map Spread → Method Calls

```ori
// Source (sugar)
{...base_map, key: value}

// Canonical (desugared)
base_map.with(key, value)
```

`ExprKind::MapWithSpread` becomes method calls on map operations.

### 6. Struct Spread → Method Calls

```ori
// Source (sugar)
Point { ...base, x: 10 }

// Canonical (desugared)
base.with_x(10)
```

`ExprKind::StructWithSpread` becomes method calls that produce updated struct values.

## Design Rationale

Desugaring at the canonical IR boundary means:

1. **Backends are simpler** — `ori_eval` and `ori_arc` handle ~17 fewer expression variants
2. **Type-level enforcement** — `CanExpr` physically cannot represent sugar, so backends can't accidentally miss a case
3. **Single point of truth** — desugaring logic lives in one place (`ori_canon/src/desugar.rs`), not duplicated across backends
4. **Testing is focused** — each backend tests core semantics, not sugar-to-core translation
