---
title: "Overview"
description: "Ori Language Specification"
order: 0
---

# Ori Language Specification

Version 0.1-alpha

## Design Principle

**Lean Core, Rich Libraries.** The language core defines only constructs requiring special syntax or static analysis. Data transformation and utilities are standard library methods.

| Core (compiler) | Library (stdlib) |
|-----------------|------------------|
| `run`, `try`, `match`, `recurse` | `map`, `filter`, `fold`, `find` |
| `parallel`, `spawn`, `timeout` | `retry`, `validate` |
| `cache`, `with` | Collection methods |

See [Patterns](10-patterns.md) for core constructs. See [Built-in Functions](11-built-in-functions.md) for library methods.

## Status

Alpha. Breaking changes expected.

## Conformance

Implementations must:
- Accept conforming programs
- Reject non-conforming programs with diagnostics
- Produce specified behavior

Extensions must not alter conforming program behavior.
