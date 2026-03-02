---
section: "02"
title: "Lexical Elements (§7)"
status: not-started
goal: "Expand §7 from 282 lines to ~750 lines with precise token, literal, and semicolon rules"
inspired_by:
  - "Go spec 'Lexical elements' — precise EBNF for every literal, escape table, semicolon rules"
  - "Rust reference 'Tokens' — comprehensive escape sequence table"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Identifier Rules"
    status: not-started
  - id: "02.2"
    title: "Integer Literal Precision"
    status: not-started
  - id: "02.3"
    title: "Float Literal Precision"
    status: not-started
  - id: "02.4"
    title: "String and Character Escape Sequences"
    status: not-started
  - id: "02.5"
    title: "Duration and Size Literal Semantics"
    status: not-started
  - id: "02.6"
    title: "Semicolon Rules"
    status: not-started
  - id: "02.7"
    title: "Whitespace and Token Boundaries"
    status: not-started
  - id: "02.8"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Lexical Elements (§7)

**Status:** Not Started
**Goal:** §7 should precisely define every token type with enough detail to implement a lexer. Currently 282 lines — target ~750. The Go spec's lexical elements section is the model: every literal form gets its own EBNF production, every escape gets a table row, and semicolon insertion has 4 crisp rules.

**Context:** The current §7 shows examples of each literal type but rarely defines the precise syntax. For instance, §7.7.1 shows `42`, `1_000_000`, `0xFF`, `0b1010` as examples but doesn't state: What happens with leading zeros (`007`)? Can hex have separators (`0xFF_FF`)? What's the maximum integer literal value? Can `0x` appear without digits? These are all questions a lexer implementor must answer.

**Reference implementations:**
- **Go** `ref/spec#Integer_literals` through `#String_literals`: EBNF for each literal, exhaustive escape table
- **Rust** `reference/src/tokens.md`: Comprehensive token definitions with edge cases
- **Zig** `doc/langref.md#Tokens`: Token definitions with disambiguations

---

## 02.1 Identifier Rules

**File:** `docs/ori_lang/v2026/spec/07-lexical-elements.md` — expand §7.2

Current §7.2 is 2 lines: "Identifiers are case-sensitive. Must not start with digit or be a reserved keyword."

- [ ] Add EBNF production: `identifier = letter { letter | digit }`
  - Reference `letter` and `digit` from §6
- [ ] State whether Unicode identifiers are allowed or ASCII-only
  - Check compiler: does `ori_lexer` accept Unicode letters?
  - Recommendation: ASCII-only for v2026 (like Go pre-1.0), with NOTE that future editions may expand
- [ ] State maximum identifier length (implementation-defined, at least 1000 characters)
- [ ] State that identifiers are compared byte-for-byte (no Unicode normalization like NFC/NFD)
- [ ] Clarify: the `$` prefix is NOT part of the identifier — `$x` and `x` are the same name (cross-ref §12.4)
- [ ] Clarify: the `@` prefix is NOT part of the identifier — it's a sigil

---

## 02.2 Integer Literal Precision

**File:** `docs/ori_lang/v2026/spec/07-lexical-elements.md` — expand §7.7.1

- [ ] Add EBNF productions:
  ```
  int_lit     = decimal_lit | hex_lit | octal_lit | binary_lit
  decimal_lit = "0" | ( non_zero_digit { [ "_" ] digit } )
  hex_lit     = "0" ( "x" | "X" ) hex_digit { [ "_" ] hex_digit }
  binary_lit  = "0" ( "b" | "B" ) bin_digit { [ "_" ] bin_digit }
  ```
- [ ] State: no octal prefix (`0o` / `0O`) — decision needed: add it or document absence?
  - Check compiler: does lexer accept `0o777`?
- [ ] State: leading zeros in decimal literals (`007`) — are they valid decimal or an error?
  - Check compiler behavior
- [ ] Digit separator rules:
  - Cannot be leading: `_42` is identifier, not literal
  - Cannot be trailing: `42_` is an error
  - Cannot be adjacent: `4__2` is an error
  - Can appear between any digits: `1_000_000`, `0xFF_FF`, `0b1010_0101`
- [ ] Integer literal range: literal value shall be representable as `int` (i64). Literal values outside -2^63 to 2^63-1 are compile-time errors
  - Exception: `int.min` is an associated constant, not a literal (because `-9223372036854775808` would overflow positive literal + negate)
- [ ] Hex digits: both `a-f` and `A-F` accepted; no mixing restriction

---

## 02.3 Float Literal Precision

**File:** `docs/ori_lang/v2026/spec/07-lexical-elements.md` — expand §7.7.2

- [ ] Add EBNF production:
  ```
  float_lit = decimal_digits "." [ decimal_digits ] [ exponent ]
            | decimal_digits exponent
  exponent  = ( "e" | "E" ) [ "+" | "-" ] decimal_digits
  ```
- [ ] State: `.5` (no leading digit) — valid or error?
  - Check compiler
- [ ] State: `5.` (no trailing digit) — valid or error?
  - Check compiler
- [ ] State: digit separators in float literals allowed: `1_000.000_001`
- [ ] State: float literal precision — IEEE 754 double (64-bit). Values outside representable range produce ±Inf
- [ ] State: `Inf`, `-Inf`, `NaN` — are these literals or named constants?
  - Check compiler: how are these accessed? Likely `float.inf`, `float.nan` constants
- [ ] State: no hex float literals (unlike C's `0x1.0p10`)

---

## 02.4 String and Character Escape Sequences

**File:** `docs/ori_lang/v2026/spec/07-lexical-elements.md` — expand §7.7.3, §7.7.4, §7.7.5

Currently §7.7.3 shows `\n` and `\` but doesn't list all escapes. Need a complete table.

- [ ] Add escape sequence table for regular strings:

  | Escape | Value | Name |
  |--------|-------|------|
  | `\\` | U+005C | Backslash |
  | `\"` | U+0022 | Double quote |
  | `\n` | U+000A | Newline |
  | `\r` | U+000D | Carriage return |
  | `\t` | U+0009 | Tab |
  | `\0` | U+0000 | Null |

  - Decision needed: `\a` (alert/bell)? `\b` (backspace)? `\f` (form feed)? `\v` (vertical tab)?
  - Check compiler: which escapes does `ori_lexer` accept?

- [ ] Unicode escapes:
  - Decision needed: `\u{HHHH}` (Rust-style) or `\uHHHH` (Go/JSON-style)?
  - Check compiler for current support
  - If not yet implemented, document as "reserved for future editions"

- [ ] Template string escapes (§7.7.4):
  - `{{` → literal `{`
  - `}}` → literal `}`
  - `` \` `` → literal backtick
  - Standard escapes (`\\`, `\n`, `\t`, `\r`, `\0`) also valid
  - Invalid escape → compile-time error

- [ ] Character literal escapes (§7.7.5):
  - Same escape set as strings
  - `'\''` → single quote literal
  - Exactly one character/escape per char literal
  - Empty char literal `''` is an error
  - Multi-character `'ab'` is an error
  - Surrogate code points (U+D800–U+DFFF) in char literals are an error

- [ ] Multi-line string handling:
  - Regular strings: `\n` required (no raw multi-line)
  - Template strings: preserve whitespace exactly as written (already stated, verify)

---

## 02.5 Duration and Size Literal Semantics

**File:** `docs/ori_lang/v2026/spec/07-lexical-elements.md` — expand §7.7.7, §7.7.8

Currently just examples. Need precise rules.

- [ ] Duration literal EBNF:
  ```
  duration_lit = decimal_number duration_suffix
  decimal_number = int_lit | int_lit "." digit { digit }
  duration_suffix = "ns" | "us" | "ms" | "s" | "m" | "h"
  ```

- [ ] Duration decimal semantics:
  - `0.5s` = 500ms = 500_000_000ns — compile-time integer arithmetic
  - Result shall be a whole number of nanoseconds, else compile-time error
  - `1.5ns` = error (0.5ns not representable)
  - `0.001s` = 1ms = 1_000_000ns — OK
  - Maximum: Duration uses i64 nanoseconds, so max ~292 years

- [ ] Size literal EBNF:
  ```
  size_lit = decimal_number size_suffix
  size_suffix = "b" | "kb" | "mb" | "gb" | "tb"
  ```

- [ ] Size decimal semantics:
  - SI units (1000-based, NOT 1024): `1kb` = 1000 bytes
  - `1.5kb` = 1500 bytes — OK
  - `0.5b` = error (0.5 bytes not representable)
  - Non-negative: negative Size literals are compile-time errors

- [ ] Digit separators in duration/size numeric part: allowed? `1_000ms`?

---

## 02.6 Semicolon Rules

**File:** `docs/ori_lang/v2026/spec/07-lexical-elements.md` — expand §7.8

Current §7.8 is 3 lines. This is one of the most important sections for anyone writing Ori.

- [ ] Formalize the two contexts:
  1. **Inside blocks (`{ ... }`)**: Semicolons terminate statements. Last expression without `;` is the block value. All expressions with `;` = void block.
  2. **Top-level declarations**: Newlines terminate declarations. No semicolons between top-level items.

- [ ] Formalize the "ends with `}`" rule:
  - Declarations whose body ends with `}` do NOT need `;`
  - Declarations with expression bodies DO need `;`
  - Examples:
    ```ori
    @add (a: int, b: int) -> int = a + b;         // needs ;
    @process (x: int) -> int = { let y = x * 2; y } // no ;
    type Point = { x: int, y: int }                 // no ;
    ```

- [ ] Formalize what's a "statement" vs "result expression":
  - `let` bindings = statement (needs `;`)
  - `use` imports = statement (needs `;`)
  - Assignments = statement (needs `;`)
  - Everything else: `;` makes it a statement, no `;` makes it the result

- [ ] Error cases:
  - Missing `;` after statement that isn't the last expression
  - `;` after the last expression (valid but changes type to void)

---

## 02.7 Whitespace and Token Boundaries

**File:** `docs/ori_lang/v2026/spec/07-lexical-elements.md`

- [ ] Define whitespace characters: space (U+0020), tab (U+0009), newline (U+000A), carriage return (U+000D after normalization)
- [ ] State: whitespace separates tokens but is otherwise insignificant (not indentation-sensitive)
- [ ] State: at least one whitespace or delimiter is required between adjacent tokens that would otherwise merge
  - `intx` = identifier `intx`, not keyword `int` + identifier `x`
  - `42x` = error (not a valid token)
- [ ] Expand §7.10 Lexer-Parser Contract with:
  - Token classification: the lexer produces a flat stream; the parser handles precedence/grouping
  - Keyword recognition: reserved keywords take precedence over identifiers

---

## 02.8 Completion Checklist

- [ ] Every literal type has EBNF in grammar.ebnf (verify sync)
- [ ] Escape sequence table is complete and matches compiler
- [ ] Digit separator rules fully specified with edge cases
- [ ] Identifier syntax precisely defined with Unicode decision
- [ ] Semicolon rules formalized with examples covering all cases
- [ ] Duration/Size decimal syntax fully specified
- [ ] Float edge cases (leading dot, trailing dot, Inf/NaN) resolved
- [ ] All additions use ISO normative style
- [ ] Cross-references to grammar.ebnf updated

**Exit Criteria:** A lexer implementor can tokenize any valid Ori source from §6+§7 alone, and can report precise errors for all invalid input.
