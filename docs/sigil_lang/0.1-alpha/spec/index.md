# The Sigil Language Specification

**Version 0.1-alpha**

## Status

This specification is in **alpha** status. The language syntax and semantics are subject to change without notice. Breaking changes should be expected between alpha releases.

## Introduction

This document is the authoritative specification for the Sigil programming language. It defines the syntax, semantics, and constraints that constitute valid Sigil programs.

Sigil is a general-purpose programming language built on declarative patterns and mandatory testing. The language emphasizes explicit syntax, strict static typing, and first-class treatment of common computational patterns.

## Notation

Grammar in this specification uses Extended Backus-Naur Form (EBNF). See [Notation](01-notation.md) for the complete notation reference.

## Table of Contents

1. [Notation](01-notation.md) — EBNF notation and conventions
2. [Source Code Representation](02-source-code.md) — Character set, encoding, line structure
3. [Lexical Elements](03-lexical-elements.md) — Tokens, comments, identifiers, keywords, literals
4. [Constants](04-constants.md) — Constant expressions and compile-time values
5. [Variables](05-variables.md) — Variable bindings and config (`$`) declarations
6. [Types](06-types.md) — Primitive, compound, and user-defined types
7. [Properties of Types](07-properties-of-types.md) — Type identity, assignability, compatibility
8. [Declarations](08-declarations.md) — Functions, types, traits, impl blocks
9. [Expressions](09-expressions.md) — Operators, conditionals, lambdas, calls
10. [Patterns](10-patterns.md) — Built-in patterns: recurse, map, filter, fold, etc.
11. [Built-in Functions](11-built-in-functions.md) — Core functions provided by the language
12. [Modules](12-modules.md) — Module system, imports, visibility
13. [Testing](13-testing.md) — Mandatory testing requirements
14. [Capabilities](14-capabilities.md) — Effect system and capability traits
15. [Memory Model](15-memory-model.md) — ARC, deterministic destruction, value semantics

## Conformance

A conforming implementation must:

1. Accept all programs that satisfy this specification
2. Reject all programs that violate this specification with a diagnostic message
3. Produce behavior consistent with the semantics defined herein

Implementation-specific extensions must not alter the behavior of conforming programs.

## References

- [Design Documentation](../design/00-index.md) — Rationale and philosophy behind language decisions
- [Standard Library](../modules/std/) — Sigil standard library documentation
- [User Guide](../guide/) — Tutorials and getting started
