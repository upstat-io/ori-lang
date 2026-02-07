---
title: "Notation"
description: "Ori Language Specification — Notation"
order: 1
section: "Foundations"
---

# Notation

The syntax is specified using Extended Backus-Naur Form (EBNF).

> **Grammar:** The complete formal grammar is in [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf).

## Productions

Productions are expressions terminated by `.`:

```
production_name = expression .
```

## Operators

| Notation | Meaning |
|----------|---------|
| `a b` | Sequence |
| `a \| b` | Alternation |
| `[ a ]` | Optional (0 or 1) |
| `{ a }` | Repetition (0 or more) |
| `( a )` | Grouping |
| `"x"` | Literal keyword |
| `'c'` | Literal character |
| `'a' … 'z'` | Character range (inclusive) |

## Naming

| Style | Usage |
|-------|-------|
| `lower_case` | Production names |
| `PascalCase` | Type names |
| `snake_case` | Functions, variables |

## Terminology

| Term | Meaning |
|------|---------|
| must | Absolute requirement |
| must not | Absolute prohibition |
| may | Optional |
| error | Compile-time failure |
| panic | Run-time failure |

## Examples

Valid:

```ori
@add (a: int, b: int) -> int = a + b
```

Invalid:

```ori
@add (a: int, b: int) = a + b  // error: missing return type
```
