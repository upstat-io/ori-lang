---
title: "Notation"
description: "Clause 5: Ori Language Specification — Notation"
order: 5
section: "Language"
---

# 5 Notation

The syntax is specified using Extended Backus-Naur Form (EBNF).

> **Grammar:** The complete formal grammar is in [grammar.ebnf](grammar.ebnf).

## 5.1 Productions

Productions are expressions terminated by `.`:

```
production_name = expression .
```

## 5.2 Operators

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

## 5.3 Naming

| Style | Usage |
|-------|-------|
| `lower_case` | Production names |
| `PascalCase` | Type names |
| `snake_case` | Functions, variables |

## 5.4 Terminology

| Term        | Meaning                                           |
|-------------|---------------------------------------------------|
| shall       | Requirement (ISO/IEC Directives, Part 2)          |
| shall not   | Prohibition                                       |
| should      | Recommendation                                    |
| may         | Permission                                        |
| can         | Possibility or capability                         |
| error       | Compile-time failure                              |
| panic       | Run-time failure                                  |

## 5.5 Examples

EXAMPLE 1

```ori
@add (a: int, b: int) -> int = a + b;
```

EXAMPLE 2  The following is not valid because the return type is omitted:

```ori
@add (a: int, b: int) = a + b;  // error: missing return type
```
