---
title: "Ori Language Specification"
description: "Table of contents and overview"
order: -10
sidebar_title: "Specification"
sidebar_order: 1
sidebar_path: "/docs/spec"
---

# Ori Language Specification

Version 0.1-alpha

Tools shall conform to the linked specification documents.

## Terminology

| Term        | Meaning                                           |
|-------------|---------------------------------------------------|
| shall       | Requirement (ISO/IEC Directives, Part 2)          |
| shall not   | Prohibition                                       |
| may         | Permission                                        |
| error       | Compile-time failure                              |

## Front Matter

| | Title |
|---|-------|
| — | [Foreword](foreword.md) |
| — | [Introduction](introduction.md) |

## Clauses

| Clause | Title |
|--------|-------|
| §1 | [Scope](01-scope.md) |
| §2 | [Normative references](02-normative-references.md) |
| §3 | [Terms and definitions](03-terms-and-definitions.md) |
| §4 | [Conformance](04-conformance.md) |
| §5 | [Notation](05-notation.md) |
| §6 | [Source code](06-source-code.md) |
| §7 | [Lexical elements](07-lexical-elements.md) |
| §8 | [Types](08-types.md) |
| §9 | [Properties of types](09-properties-of-types.md) |
| §10 | [Declarations](10-declarations.md) |
| §11 | [Blocks and scope](11-blocks-and-scope.md) |
| §12 | [Constants](12-constants.md) |
| §13 | [Variables](13-variables.md) |
| §14 | [Expressions](14-expressions.md) |
| §15 | [Patterns](15-patterns.md) |
| §16 | [Control flow](16-control-flow.md) |
| §17 | [Errors and panics](17-errors-and-panics.md) |
| §18 | [Modules](18-modules.md) |
| §19 | [Testing](19-testing.md) |
| §20 | [Capabilities](20-capabilities.md) |
| §21 | [Memory model](21-memory-model.md) |
| §22 | [Concurrency model](22-concurrency-model.md) |
| §23 | [Program execution](23-program-execution.md) |
| §24 | [Constant expressions](24-constant-expressions.md) |
| §25 | [Conditional compilation](25-conditional-compilation.md) |
| §26 | [Foreign function interface](26-ffi.md) |
| §27 | [Reflection](27-reflection.md) |

## Annexes

| Annex | Title | Type |
|-------|-------|------|
| A | [Formal grammar](annex-a-grammar.md) | Normative |
| B | [Operator rules](annex-b-operator-rules.md) | Normative |
| C | [Built-in functions](annex-c-built-in-functions.md) | Normative |
| D | [Formatting](annex-d-formatting.md) | Informative |
| E | [System considerations](annex-e-system-considerations.md) | Informative |

## References

| | |
|---|---|
| — | [Bibliography](bibliography.md) |

## Companion Files

| File | Description |
|------|-------------|
| [grammar.ebnf](grammar.ebnf) | Formal grammar (EBNF) |
| [operator-rules.md](operator-rules.md) | Operator typing and evaluation rules |
