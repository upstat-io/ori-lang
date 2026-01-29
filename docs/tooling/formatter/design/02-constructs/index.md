---
title: "Constructs Overview"
description: "Ori Formatter Design — Per-Construct Formatting Rules"
order: 1
---

# Constructs Overview

This section documents the formatting rules for each Ori language construct. All rules follow the core principle: **inline if ≤100 characters, break otherwise**.

## Construct Categories

### Declarations

Top-level items that introduce names:

| Construct | Document |
|-----------|----------|
| Functions | [Declarations](declarations.md#functions) |
| Constants | [Declarations](declarations.md#constants) |
| Type definitions | [Declarations](declarations.md#type-definitions) |
| Traits | [Declarations](declarations.md#traits) |
| Implementations | [Declarations](declarations.md#implementations) |
| Tests | [Declarations](declarations.md#tests) |

### Expressions

Computations that produce values:

| Construct | Document |
|-----------|----------|
| Function calls | [Expressions](expressions.md#function-calls) |
| Method chains | [Expressions](expressions.md#method-chains) |
| Conditionals | [Expressions](expressions.md#conditionals) |
| Lambdas | [Expressions](expressions.md#lambdas) |
| Binary expressions | [Expressions](expressions.md#binary-expressions) |
| Bindings | [Expressions](expressions.md#bindings) |

### Patterns

Compiler-recognized constructs with special syntax:

| Construct | Document |
|-----------|----------|
| `run` / `try` | [Patterns](patterns.md#run-and-try) |
| `match` | [Patterns](patterns.md#match) |
| `recurse` | [Patterns](patterns.md#recurse) |
| `parallel` / `spawn` | [Patterns](patterns.md#parallel-and-spawn) |
| `timeout` / `cache` | [Patterns](patterns.md#timeout-and-cache) |
| `with` / `in` | [Patterns](patterns.md#with-expressions) |
| `for` loops | [Patterns](patterns.md#for-loops) |
| `nursery` | [Patterns](patterns.md#nursery) |

### Collections

Aggregate data structures:

| Construct | Document |
|-----------|----------|
| Lists | [Collections](collections.md#lists) |
| Maps | [Collections](collections.md#maps) |
| Tuples | [Collections](collections.md#tuples) |
| Struct literals | [Collections](collections.md#struct-literals) |
| Ranges | [Collections](collections.md#ranges) |

## Formatting Decision Tree

For any construct, the formatter follows this decision process:

```
1. Is this an always-stacked construct (run, try, match arms)?
   YES → Use stacked format
   NO  → Continue

2. Calculate inline width of the construct

3. Does current_column + inline_width <= 100?
   YES → Use inline format
   NO  → Use broken format

4. For nested constructs, repeat from step 1
```

## Universal Rules

These rules apply across all constructs:

### Spacing

| Context | Rule | Example |
|---------|------|---------|
| Binary operators | Space around | `a + b` |
| Arrows | Space around | `x -> x + 1` |
| Colons | Space after | `x: int` |
| Commas | Space after | `f(a, b)` |
| Parentheses | No inner space | `f(x)` |
| Brackets | No inner space | `[1, 2]` |
| Braces | No inner space | `{ x: 1 }` |
| Empty delimiters | No space | `[]`, `{}`, `()` |

### Trailing Commas

| Format | Rule |
|--------|------|
| Single-line | No trailing comma |
| Multi-line | Trailing comma required |

### Blank Lines

| Context | Rule |
|---------|------|
| After imports | One blank line |
| After constants | One blank line |
| Between functions | One blank line |
| Between trait/impl methods | One blank line |
| Consecutive blank lines | Forbidden |
| Leading/trailing blank lines | Forbidden |
