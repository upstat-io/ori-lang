# Lexer V2 Index

> **This is a FULL REPLACEMENT of the current Logos-based lexer.**
> No backwards compatibility. Old lexer is deleted entirely.

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Two-Layer Architecture
**File:** `section-01-architecture.md` | **Status:** Not Started

```
two-layer, architecture, crate, separation, rustc_lexer
low-level tokenizer, high-level processor, compiler integration
reusable, LSP, formatter, syntax highlighter
stable API, pure function, no dependencies
ori_lexer_core, ori_lexer
```

---

### Section 02: Compact Token Representation
**File:** `section-02-tokens.md` | **Status:** Not Started

```
token, compact, 8-byte, memory, size
RawToken, Tag, TokenStorage, TokenList
SoA, Structure-of-Arrays, MultiArrayList
no end offset, lazy computation, derive end
TokenFlags, bitfield, space before, newline before
lazy line/column, LineIndex, binary search
span, position, byte offset
```

---

### Section 03: State Machine Design
**File:** `section-03-state-machine.md` | **Status:** Not Started

```
state machine, hand-written, labeled switch
Logos migration, DFA, regex, patterns
sentinel, null terminator, bounds check
State enum, transitions, continue
control flow, jump table, optimization
single pass, streaming, iterator
```

---

### Section 04: Keyword & Operator Handling
**File:** `section-04-keywords.md` | **Status:** Not Started

```
keyword, perfect hash, O(1), lookup
collision, compile-time, validation
operator, precedence, associativity
OPERATOR_INFO, table, metadata
context-sensitive, timeout, parallel, cache
token gluing, breaking, compound operators
>> shift right, >= greater equal, generics
```

---

### Section 05: Unicode & Escape Handling
**File:** `section-05-unicode.md` | **Status:** Not Started

```
unicode, UTF-8, identifier, XID
XID_Start, XID_Continue, unicode-ident
escape sequence, \n, \r, \t, \\
hex escape, \xHH, \u{XXXX}
string interpolation, ${}, template
raw string, r"...", no escapes
char literal, single quote
```

---

### Section 06: Error Handling
**File:** `section-06-errors.md` | **Status:** Not Started

```
error, diagnostic, message, lexical
LexError, structured, rich, detailed
empathetic, Elm, Gleam, friendly
unterminated string, invalid escape
common mistake, semicolon, single quote
error recovery, continue, resilient
```

---

### Section 07: Performance Optimizations
**File:** `section-07-performance.md` | **Status:** Not Started

```
performance, optimization, SIMD, fast
whitespace, skip, 8 bytes, chunk
memchr, comment, delimiter, search
branchless, character check, ASCII
buffer, sentinel, bounds check
cache, memory, allocation
```

---

### Section 08: Parser Integration
**File:** `section-08-integration.md` | **Status:** Not Started

```
parser, integration, Parser V2, cursor
trivia, comment, whitespace, preserve
ModuleExtra, doc comment, classification
incremental, relex, range, edit
whitespace-sensitive, space before
adjacent, compound, context
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Two-Layer Architecture | `section-01-architecture.md` |
| 02 | Compact Token Representation | `section-02-tokens.md` |
| 03 | State Machine Design | `section-03-state-machine.md` |
| 04 | Keyword & Operator Handling | `section-04-keywords.md` |
| 05 | Unicode & Escape Handling | `section-05-unicode.md` |
| 06 | Error Handling | `section-06-errors.md` |
| 07 | Performance Optimizations | `section-07-performance.md` |
| 08 | Parser Integration | `section-08-integration.md` |

---

## Cross-References

| Topic | Lexer V2 Section | Parser V2 Section |
|-------|------------------|-------------------|
| Keywords | 04.1-04.2 | 02.1-02.2 |
| Precedence | 04.3 | 02.3 |
| Adjacent tokens | 04.5 | 02.4 |
| Error messages | 06.x | 04.x |
| Incremental | 08.3 | 05.x |
| Trivia/Metadata | 08.1-08.2 | 06.x |
