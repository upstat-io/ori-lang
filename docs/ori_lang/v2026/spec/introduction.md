---
title: "Introduction"
description: "Ori Language Specification — Introduction"
order: -1
section: "Front Matter"
---

# Introduction

## Design Principle

**Lean Core, Rich Libraries.** The language core defines only constructs requiring special syntax or static analysis. Data transformation and utilities are standard library methods.

| Core (compiler) | Library (stdlib) |
|-----------------|------------------|
| `run`, `try`, `match`, `recurse` | `map`, `filter`, `fold`, `find` |
| `parallel`, `spawn`, `timeout` | `retry`, `validate` |
| `cache`, `with` | Collection methods |

See [Clause 15](15-patterns.md) for core constructs. See [Annex C](annex-c-built-in-functions.md) for library methods.
