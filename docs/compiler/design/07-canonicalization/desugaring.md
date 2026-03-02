---
title: "Desugaring"
description: "Ori Compiler Design — Syntactic Sugar Elimination"
order: 701
section: "Canonicalization"
---

# Desugaring

## What Is Desugaring?

Every programming language faces a tension: the surface syntax users write should be expressive and ergonomic, but the internal representation compilers operate on should be minimal and uniform. **Desugaring** is the compiler phase that resolves this tension by translating convenient surface syntax ("syntactic sugar") into a smaller set of core constructs that the rest of the compiler understands.

The term comes from Peter Landin's 1964 observation that some language constructs are "syntactic sugar" — they make the language sweeter to use but add no fundamental expressiveness. `x += 1` is sugar for `x = x + 1`. Template strings like `` `Hello, {name}` `` are sugar for string concatenation. Named arguments like `f(x: 1, y: 2)` are sugar for positional calls `f(1, 2)`. Each of these can be mechanically translated into simpler forms without losing meaning.

### Why Compilers Desugar

Without desugaring, every downstream phase — type checking, optimization, code generation — must handle both the sugared and unsugared forms of each construct. This creates a multiplicative complexity problem:

```text
N sugar variants × M compiler phases = N×M code paths to maintain
```

Desugaring collapses this to:

```text
N translations (once) + M phases × core variants only
```

A language with 7 sugar variants and 3 downstream phases goes from 21 code paths to 7 + 3×(core only). More importantly, each downstream phase can be written and tested against a single canonical form, eliminating entire classes of "forgot to handle this variant" bugs.

### The Desugaring Contract

A correct desugaring transformation must preserve three properties:

1. **Semantics** — the desugared form produces the same result as the original
2. **Types** — if the original type-checks, the desugared form type-checks identically
3. **Errors** — error messages still point to the original source location (via span preservation)

The transformation is purely mechanical — it reorders, restructures, or replaces syntax nodes, but never performs semantic analysis, type inference, or optimization. Those are the jobs of other phases.

### When To Desugar: Early vs Late

Compilers choose different points in the pipeline for desugaring:

| Strategy | Examples | Tradeoff |
|----------|----------|----------|
| **Before type checking** | Haskell (do-notation), Scala (for-comprehensions) | Simpler type checker, but sugar-aware error messages are harder |
| **After type checking** | Ori, Roc, Elm | Type checker sees sugar (better errors), backends see core (simpler codegen) |
| **Progressive** | GHC (Core → STG → Cmm), Rust (HIR → MIR) | Multiple IR levels, each desugaring further |

Ori desugars **after type checking** — the type checker operates on the full sugared AST with named arguments, template literals, and spreads, producing richer error messages that reference the syntax the user actually wrote. Only after types are resolved does canonicalization strip the sugar away.

## How Ori Desugars

Desugaring is **integrated into `lower_expr()`**, not a separate pass. When the lowerer encounters a sugar variant in the AST, it calls the corresponding `desugar_*` method inline, producing canonical expressions directly. This eliminates 7 syntactic sugar variants from the AST in a single traversal alongside all other lowering work. The transformation is mechanical and type-preserving — no semantic analysis occurs.

### The Core Idea: Two Distinct Types

The key architectural decision is that `CanExpr` (canonical) is a **separate type** from `ExprKind` (AST). Sugar variants exist in `ExprKind` but have no corresponding variant in `CanExpr`:

| `ExprKind` (AST) | `CanExpr` (Canonical) | What Happens |
|---|---|---|
| `CallNamed` | — | Reordered to positional `Call` |
| `MethodCallNamed` | — | Reordered to positional `MethodCall` |
| `TemplateLiteral` | — | Expanded to concatenation chain |
| `TemplateFull` | — | Expanded to concatenation chain |
| `ListWithSpread` | — | Lowered to method calls |
| `MapWithSpread` | — | Lowered to method calls |
| `StructWithSpread` | — | Lowered to method calls |

Because these variants don't exist in `CanExpr`, a backend that tries to match on `CallNamed` gets a **compile error** — not a runtime panic, not a silent bug. The Rust type system enforces that all sugar has been eliminated before any backend sees the IR.

This is stronger than the common approach of using a shared AST type with a "should never appear after desugaring" comment. Comments don't prevent bugs; distinct types do.

## Sugar Variants Eliminated

### 1. Named Calls → Positional Calls

Ori requires named arguments at call sites, which improves readability but means argument order in source may differ from parameter order in the function signature:

```ori
// Source: arguments in any order, with labels
fetch(timeout: 5s, url: "https://example.com")

// Canonical: arguments reordered to match signature
fetch("https://example.com", 5s)
```

The lowerer uses the function's type signature (resolved during type checking) to map each named argument to its positional index. This is a pure reordering — no values change, only their position in the argument list.

`ExprKind::CallNamed` becomes `CanExpr::Call` with arguments in signature order.

### 2. Named Method Calls → Positional Method Calls

The same named-to-positional translation applies to method calls:

```ori
// Source
list.filter(predicate: x -> x > 0)

// Canonical
list.filter(x -> x > 0)
```

`ExprKind::MethodCallNamed` becomes `CanExpr::MethodCall` with positional arguments.

### 3. Template Literals → String Concatenation

Template literals are convenient syntax for building strings with embedded expressions. The lowerer expands them into explicit concatenation chains with `to_str()` conversions:

```ori
// Source
`Hello, {name}! You are {age} years old.`

// Canonical
"Hello, " ++ to_str(name) ++ "! You are " ++ to_str(age) ++ " years old."
```

Each interpolated expression gets wrapped in a `to_str()` call (the `Printable` trait's method), then the segments are joined with the `++` concat operator. A template with N interpolations becomes a left-folded chain of N×2 concat operations.

`ExprKind::TemplateLiteral` and `TemplateFull` become chains of `CanExpr::BinaryOp(Concat)`.

### 4. List Spread → Method Calls

List spread syntax creates a new list by combining existing lists with new elements:

```ori
// Source
[...existing, new_item]

// Canonical
existing.append(new_item)
```

`ExprKind::ListWithSpread` becomes method calls on list operations.

### 5. Map Spread → Method Calls

Map spread merges maps and adds new key-value pairs:

```ori
// Source
{...base_map, key: value}

// Canonical
base_map.with(key, value)
```

`ExprKind::MapWithSpread` becomes method calls on map operations.

### 6. Struct Spread → Method Calls

Struct spread creates a copy of a struct with some fields overridden:

```ori
// Source
Point { ...base, x: 10 }

// Canonical
base.with_x(10)
```

This is a functional update — the original struct is not mutated. The desugared form uses synthesized `with_*` method calls that produce new struct values.

`ExprKind::StructWithSpread` becomes method calls that produce updated struct values.

## Earlier Desugarings (Before Canonicalization)

Not all desugaring happens in `ori_canon`. Some transformations occur earlier in the pipeline:

| Sugar | Desugared To | Where |
|-------|-------------|-------|
| `x \|> f(a:)` (pipe) | `let tmp = x; f(a: tmp)` | Type checker |
| `x += 1` (compound assign) | `x = x + 1` | Parser |
| `x < y` (comparison ops) | `x.compare(y).is_less()` | Type checker |
| `for x in items yield x * 2` | Iterator/collect expansion | Type checker |

These desugar earlier because they affect type inference or need semantic information that only the type checker has. Pipe desugaring, for example, must determine which parameter to fill — this requires knowing the function's signature, which is type-level information.

The canonicalization phase handles only the sugar that survived through type checking intact.

## Design Rationale

Desugaring at the canonical IR boundary means:

1. **Backends are simpler** — `ori_eval` and `ori_arc` handle fewer expression variants, reducing code and bug surface
2. **Type-level enforcement** — `CanExpr` physically cannot represent sugar, so backends can't accidentally miss a case
3. **Single point of truth** — desugaring logic lives in one place (`ori_canon/src/desugar/`), not duplicated across backends
4. **Testing is focused** — each backend tests core semantics, not sugar-to-core translation
5. **Better error messages** — because the type checker sees the sugared AST, errors reference named arguments, template syntax, and spreads that the user actually wrote, rather than the desugared equivalents they never see

## Prior Art

| Language | Approach |
|----------|----------|
| **Haskell/GHC** | Progressive desugaring: source → Core (eliminates do-notation, list comprehensions, where-clauses) → STG → Cmm. Each IR level is simpler than the last. |
| **Rust** | HIR desugars `for` loops, `?` operator, `async`/`await`. MIR desugars further (drops, pattern matching). Two-stage approach. |
| **Roc** | `ast::Expr` → `can::Expr` eliminates string interpolation, record updates, `when` sugar. Similar to Ori's single-phase approach. |
| **Elm** | `Source` → `Canonical` eliminates ports, effects, operators. Decision trees added in `Optimized` phase. |
| **Scala** | `for`-comprehensions desugar to `flatMap`/`map`/`filter` chains before type checking — the classic "desugar early" approach. |
