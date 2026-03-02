---
section: "01"
title: "Source Code (§6)"
status: not-started
goal: "Expand §6 from 47 lines to ~130 lines with precise character, encoding, and position rules"
inspired_by:
  - "Go spec 'Source code representation' — defines characters, letters, digits precisely"
depends_on: []
sections:
  - id: "01.1"
    title: "Characters and Unicode Categories"
    status: not-started
  - id: "01.2"
    title: "Letters and Digits"
    status: not-started
  - id: "01.3"
    title: "Source Position Model"
    status: not-started
  - id: "01.4"
    title: "Source File Constraints"
    status: not-started
  - id: "01.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Source Code (§6)

**Status:** Not Started
**Goal:** §6 should define what constitutes valid source text with enough precision that a lexer implementor never has to guess. Currently 47 lines — target ~130 lines.

**Context:** The current §6 says "Source code is Unicode text encoded in UTF-8" and lists file naming rules. But it doesn't define valid source characters, how positions are computed, what happens with NUL bytes, or how lines are counted. Go's equivalent section precisely defines `unicode_char`, `unicode_letter`, `unicode_digit`, and `newline` — these feed directly into identifier and literal rules.

**Reference implementations:**
- **Go** `ref/spec#Source_code_representation`: Defines `unicode_char`, `unicode_letter`, `unicode_digit` as grammar productions
- **Rust** `reference/src/input-format.md`: BOM handling, NUL rejection, normalization

---

## 01.1 Characters and Unicode Categories

**File:** `docs/ori_lang/v2026/spec/06-source-code.md`

Add a §6.1 Characters subsection that defines which Unicode characters are valid in source code.

- [ ] Define `unicode_char` — any Unicode code point except NUL (U+0000)
- [ ] State that NUL bytes in source text are an error
- [ ] Define handling of non-printable control characters (U+0001–U+001F except \t, \n, \r)
  - Decision needed: reject or allow in string/char literals only?
- [ ] Clarify that all Unicode code points are valid in string and character literals (via escapes or direct UTF-8)
- [ ] Clarify that comments may contain any valid `unicode_char`

---

## 01.2 Letters and Digits

**File:** `docs/ori_lang/v2026/spec/06-source-code.md`

Add a §6.X subsection defining the character categories used by identifier rules in §7.

- [ ] Define `letter` — Unicode category L (Letter) plus underscore `_`
- [ ] Define `digit` — Unicode category Nd (Decimal digit), specifically ASCII `0`–`9` only
  - Decision needed: Do identifiers allow non-ASCII digits? (Go does not, Rust does not)
- [ ] Define `identifier_char` — `letter | digit`
- [ ] Cross-reference: "These definitions are used by §7.2 Identifiers"

---

## 01.3 Source Position Model

**File:** `docs/ori_lang/v2026/spec/06-source-code.md`

Formalize how positions in source code are computed (needed for diagnostics and TraceEntry).

- [ ] Define "line" — delimited by \n (after normalization)
- [ ] Define line numbering — 1-based
- [ ] Define column numbering — 1-based, in bytes (UTF-8 byte offset from line start)
  - Decision needed: byte offset vs codepoint offset vs grapheme cluster? (Go uses byte, Rust uses byte)
- [ ] Cross-reference to `TraceEntry` type (§9.9.1) which has `line: int, column: int`

---

## 01.4 Source File Constraints

**File:** `docs/ori_lang/v2026/spec/06-source-code.md`

Expand existing §6.2/§6.3 with implementation limits.

- [ ] State maximum source file size (implementation-defined, at least 2^31-1 bytes)
- [ ] State that a source file shall contain at least one declaration (empty files are valid but produce no module)
- [ ] Clarify behavior of trailing newline — recommended but not required
- [ ] State that line length is unlimited (formatter enforces 100, but spec does not require it)

---

## 01.5 Completion Checklist

- [ ] §6 defines `unicode_char`, `letter`, `digit` precisely
- [ ] NUL handling specified (error)
- [ ] BOM handling specified (error — already present, verify)
- [ ] Position model formalized (line/column, 1-based, byte offset)
- [ ] File constraints documented
- [ ] Cross-references to §7 added
- [ ] All additions use ISO normative style (`shall`, `NOTE`, `EXAMPLE`)

**Exit Criteria:** A lexer implementor can determine from §6 alone whether any given byte sequence is valid Ori source text, and can compute source positions for diagnostics.
