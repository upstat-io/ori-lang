---
section: "06"
title: "Lexer/Parser DRY"
status: not-started
reviewed: false
goal: "Parameterize duplicated cooking functions, compound operators, and parser identifier acceptance — eliminate 15+ algorithmic duplications"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Parameterize Template Cooking Functions"
    status: not-started
  - id: "06.2"
    title: "Parameterize Numeric Cooking Functions"
    status: not-started
  - id: "06.3"
    title: "Extract Compound-Assignment Operator Helper"
    status: not-started
  - id: "06.4"
    title: "Unify Parser Identifier Acceptance"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Lexer/Parser DRY

**Status:** Not Started
**Goal:** The lexer has 4 near-identical template cooking functions, 3 integer cooking functions, 2 duration/size cooking functions, and 6 compound-assignment operator functions that share the same 2-branch skeleton. The parser has 3 `expect_ident*` functions with identical cascading structure. Parameterize all of these.

**Context:** These are all low-risk extractions — the functions are self-contained within their crates, have identical control flow, and differ only in data (TokenKind variant, radix, suffix type, error variant).

---

## 06.1 Parameterize Template Cooking Functions

**File(s):** `compiler/ori_lexer/src/cooker/escape_cooking.rs`

`cook_template_head`, `cook_template_middle`, `cook_template_tail`, `cook_template_complete` (lines 55-141) are byte-for-byte identical except for the `TokenKind` variant constructed.

- [ ] Create `cook_template_segment(offset, len, kind_ctor: fn(StringId) -> TokenKind)`:
  ```rust
  fn cook_template_segment(
      &mut self, offset: u32, len: u32,
      make_kind: fn(StringId) -> TokenKind,
  ) -> CookResult
  ```
- [ ] Rewrite all 4 template cooking functions as one-liners calling `cook_template_segment`
- [ ] Verify: `timeout 150 cargo test -p ori_lexer` passes

---

## 06.2 Parameterize Numeric Cooking Functions

**File(s):** `compiler/ori_lexer/src/cooker/numeric.rs`, `compiler/ori_lexer/src/cooker/duration_size.rs`

Three integer cookers (`cook_int`, `cook_hex_int`, `cook_bin_int`) share: slice source, strip prefix, parse, overflow error. Two unit cookers (`cook_duration`, `cook_size`) are structural twins.

- [ ] Create `cook_int_with_radix(offset, len, prefix_len, radix)` — covers all 3 integer cookers
- [ ] Create `cook_unit_literal<U>(offset, len, detect_suffix, parse_value)` — covers duration and size
- [ ] Rewrite all 5 functions as thin wrappers
- [ ] Verify: `timeout 150 cargo test -p ori_lexer` passes

---

## 06.3 Extract Compound-Assignment Operator Helper

**File(s):** `compiler/ori_lexer_core/src/raw_scanner/operators.rs`

6 functions (`plus`, `star`, `percent`, `caret`, `at`, `bang`) follow the same pattern: advance, check `b'='`, return compound variant or single variant.

- [ ] Create `try_compound_eq(start, single: RawTag, compound: RawTag) -> RawToken`:
  ```rust
  fn try_compound_eq(&mut self, start: u32, single: RawTag, compound: RawTag) -> RawToken {
      if self.try_eat(b'=') { self.tok(start, compound) } else { self.tok(start, single) }
  }
  ```
- [ ] Rewrite all 6 functions as one-liners
- [ ] Verify: `timeout 150 cargo test -p ori_lexer_core` passes

---

## 06.4 Unify Parser Identifier Acceptance

**File(s):** `compiler/ori_parse/src/cursor/mod.rs`

`expect_ident()` (line 512), `expect_member_name()` (line 536), `expect_ident_or_keyword()` (line 577) share the same cascading if-else structure.

- [ ] Create `expect_name(mode: IdentAcceptMode)` where the mode controls which additional conversions are accepted:
  ```rust
  enum IdentAcceptMode { Ident, MemberName, IdentOrKeyword }
  ```
- [ ] Rewrite all 3 functions as calls to `expect_name` with the appropriate mode
- [ ] Verify: `timeout 150 cargo test -p ori_parse` passes

---

## 06.R Third Party Review Findings

- None.

---

## 06.T Test Strategy

This section is low-risk parameterization of self-contained functions. The existing lexer/parser test suites are comprehensive and serve as the primary regression gate.

- [ ] Add unit tests for `cook_template_segment()`: verify it produces the same TokenKind for each of the 4 template positions (head, middle, tail, complete)
- [ ] Add unit tests for `cook_int_with_radix()`: verify decimal, hex (0x prefix), and binary (0b prefix) all parse correctly; verify overflow error on `9999999999999999999`
- [ ] Add unit tests for `try_compound_eq()`: verify `+` alone returns single, `+=` returns compound, for each of the 6 operator types
- [ ] Add unit test for `expect_name()`: verify Ident mode accepts identifiers only, MemberName accepts identifiers and integers, IdentOrKeyword accepts both
- [ ] Verify `timeout 150 cargo test -p ori_lexer_core` passes after 06.3
- [ ] Verify `timeout 150 cargo test -p ori_lexer` passes after 06.1, 06.2
- [ ] Verify `timeout 150 cargo test -p ori_parse` passes after 06.4
- [ ] Verify `timeout 150 ./test-all.sh` passes after all sub-sections complete

---

## 06.N Completion Checklist

- [ ] Template cooking functions parameterized (4 -> 1 + 4 wrappers)
- [ ] Numeric cooking functions parameterized (5 -> 2 + 5 wrappers)
- [ ] Compound-assignment operators extracted (6 -> 1 + 6 one-liners)
- [ ] Parser identifier acceptance unified (3 -> 1 + 3 wrappers)
- [ ] Unit tests for all new canonical functions pass
- [ ] `timeout 150 cargo test -p ori_lexer_core` passes
- [ ] `timeout 150 cargo test -p ori_lexer` passes
- [ ] `timeout 150 cargo test -p ori_parse` passes
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `./clippy-all.sh` clean
- [ ] `/tpr-review` covering Section 06
- [ ] `/impl-hygiene-review last commit`
