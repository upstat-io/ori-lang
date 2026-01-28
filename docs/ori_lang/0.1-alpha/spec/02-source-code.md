---
title: "Source Code Representation"
description: "Ori Language Specification — Source Code Representation"
order: 2
---

# Source Code Representation

Source code is Unicode text encoded in UTF-8.

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § LEXICAL GRAMMAR, SOURCE STRUCTURE

## Characters

Carriage return (U+000D) followed by newline is normalized to newline. A lone carriage return is treated as newline.

## Source Files

Source files use `.ori` extension. Test files use `.test.ori` extension.

File names must:
- Start with ASCII letter or underscore
- Contain only ASCII letters, digits, underscores
- End with `.ori` or `.test.ori`

## Encoding

Source files must be valid UTF-8 without byte order mark. Invalid UTF-8 or presence of BOM is an error.

## Line Continuation

A newline does not terminate the logical line when preceded by:
- Binary operators: `+`, `-`, `*`, `/`, `%`, `&&`, `||`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `..`, `..=`
- Opening delimiters: `(`, `[`, `{`
- Comma, assignment (`=`), arrow (`->`), colon (`:`)

## Module Mapping

Each source file defines one module. Module name derives from file path relative to source root:

| File Path | Module Name |
|-----------|-------------|
| `src/main.ori` | `main` |
| `src/http/client.ori` | `http.client` |

See [Modules](12-modules.md).
