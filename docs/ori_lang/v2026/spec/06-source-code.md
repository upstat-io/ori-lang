---
title: "Source Code Representation"
description: "Clause 6: Ori Language Specification — Source Code Representation"
order: 6
section: "Language"
---

# 6 Source Code Representation

Source code is Unicode text encoded in UTF-8. The text is not canonicalized, so a single accented code point is distinct from the same character constructed from combining an accent and a letter; those are treated as two code points. This document uses the unqualified term _character_ to refer to a Unicode code point in the source text.

Each code point is distinct; for instance, uppercase and lowercase letters are different characters.

> **Grammar:** See [grammar.ebnf](grammar.md) § LEXICAL GRAMMAR, Characters

## 6.1 Characters

The following terms denote specific character categories:

```ebnf
newline      = /* the Unicode code point U+000A */ .
unicode_char = /* any Unicode code point except NUL (U+0000) */ .
```

A _unicode character_ is any Unicode code point except U+0000 (NUL). The NUL byte shall not appear in source text; its presence is an error.

_Control characters_ U+0001 through U+001F, except for the following, shall not appear in source text outside of string and character literals:

- U+0009 (horizontal tab)
- U+000A (newline)
- U+000D (carriage return)

The presence of any other control character outside a string or character literal is an error.

NOTE  Control characters may be represented in string and character literals using escape sequences (see [7.7.3.1](07-lexical-elements.md#7731-escape-sequences)).

_Comments_ (see [7.1](07-lexical-elements.md#71-comments)) may contain any _unicode character_.

### 6.1.1 Line endings

A carriage return U+000D immediately followed by a newline U+000A is normalized to a single newline. A lone carriage return not followed by a newline is also treated as a newline. After normalization, all line endings in source text are represented as U+000A.

### 6.1.2 Letters and digits

The following terms define the character categories used by identifier rules in [Clause 7](07-lexical-elements.md):

```ebnf
letter = 'A' … 'Z' | 'a' … 'z' .
digit  = '0' … '9' .
```

Identifiers are restricted to ASCII letters, ASCII digits, and underscore (see [7.2](07-lexical-elements.md#72-identifiers)). The underscore character `_` (U+005F) is considered a letter for the purpose of identifier formation.

NOTE  Future editions may expand identifier characters to include Unicode letter and digit categories. The current restriction simplifies tooling and ensures all identifiers are representable in ASCII.

## 6.2 Source Files

Source files use the `.ori` extension. Test files use the `.test.ori` extension.

File names shall:
- begin with an ASCII letter (`A`–`Z`, `a`–`z`) or underscore (`_`)
- contain only ASCII letters, ASCII digits (`0`–`9`), and underscores
- end with `.ori` or `.test.ori`

EXAMPLE  Valid file names: `main.ori`, `http_client.ori`, `math_test.test.ori`.

EXAMPLE  The following are not valid file names: `2fast.ori` (starts with digit), `my-module.ori` (contains hyphen), `module.ORI` (wrong extension case).

## 6.3 Encoding

Source files shall be valid UTF-8 without byte order mark (BOM, U+FEFF). The presence of a BOM at any position is an error. Invalid UTF-8 byte sequences are an error.

NOTE  Some editors insert a UTF-8 BOM. Ori rejects it to guarantee that byte offsets are unambiguous from the first byte.

## 6.4 Source Positions

A _source position_ identifies a location in source text for use in diagnostics and trace entries.

A _line_ is a sequence of characters terminated by a newline (after normalization per 6.1.1) or by the end of the source file. Lines are numbered sequentially starting at 1.

A _column_ is the byte offset from the start of a line, numbered starting at 1. The first byte of a line is column 1.

NOTE  Column numbers count UTF-8 bytes, not Unicode code points or grapheme clusters. A multi-byte character occupies multiple column positions. This matches the convention used by the Language Server Protocol and common editors.

Source positions are represented at runtime by the `TraceEntry` type (see [9.9.1](09-types.md#991-traceentry)), which has `line: int` and `column: int` fields following these conventions.

EXAMPLE  In the line `let $π = 3`, the identifier `π` (U+03C0, encoded as 2 bytes CE B0) occupies columns 6–7. The `=` is at column 9.

## 6.5 Line Continuation

Ori uses _implicit line continuation_: a newline does not terminate a logical line when the current token context indicates that the expression is incomplete.

A newline is not a statement terminator when the last token before the newline is:

- a binary operator: `+`, `-`, `*`, `/`, `%`, `**`, `div`, `@`, `&&`, `||`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `..`, `..=`, `??`, `|>`, `&`, `|`, `^`, `<<`, `>>`
- an opening delimiter: `(`, `[`, `{`
- a comma (`,`), assignment operator (`=`, `+=`, `-=`, etc.), arrow (`->`), or colon (`:`)

NOTE  This list is exhaustive. Closing delimiters, identifiers, literals, and keywords at end of line do terminate the logical line unless they match one of the continuation tokens above.

## 6.6 Module Mapping

Each source file defines one _module_. The module name derives from the file path relative to the source root:

| File Path | Module Name |
|-----------|-------------|
| `src/main.ori` | `main` |
| `src/http/client.ori` | `http.client` |
| `src/utils/string_helpers.ori` | `utils.string_helpers` |

Directory separators in the file path map to dots (`.`) in the module name. The `.ori` extension is stripped.

See [Clause 18](18-modules.md) for the complete module system specification.

## 6.7 Source File Constraints

The maximum source file size is implementation-defined. An implementation shall accept source files of at least 2 147 483 647 bytes (2^31 − 1).

A source file with no declarations produces no module and has no effect on the program.

NOTE  There is no requirement for a trailing newline at end of file, though the formatter inserts one.

NOTE  Line length is not limited by the language. The formatter enforces a 100-character line width as a style guideline; the specification imposes no such constraint.
