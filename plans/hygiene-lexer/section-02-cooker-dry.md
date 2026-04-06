---
section: "02"
title: "Cooker Layer Algorithmic DRY"
status: not-started
reviewed: false
goal: "Consolidate 4 clusters of algorithmically-duplicated functions in the cooker layer"
success_criteria:
  - "4 template cooking functions → 1 generic with 4 call sites"
  - "2 unescape functions share a scanning core with context-specific escape sets"
  - "3 integer cooking functions → 1 generic with radix parameter"
  - "2 duration/size cooking + 2 suffix detection functions → generic versions"
  - "No behavioral changes — all existing tests pass unchanged"
inspired_by:
  - "rustc_lexer unescape.rs — single unescape function parameterized by Mode enum"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Template cooking consolidation"
    status: not-started
  - id: "02.2"
    title: "Unescape function consolidation"
    status: not-started
  - id: "02.3"
    title: "Integer cooking consolidation"
    status: not-started
  - id: "02.4"
    title: "Duration/size consolidation"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Cooker Layer Algorithmic DRY

**Status:** Not Started
**Goal:** Eliminate algorithmic duplication in the cooker layer by extracting shared control-flow skeletons into parameterized functions. Four clusters of duplication are targeted, each with identical multi-step algorithms that differ only in type parameters, error kinds, or context-specific values.

**Success Criteria:**

- [ ] Template cooking: `escape_cooking.rs` has 1 generic function + 4 thin wrappers (down from 4 near-identical functions)
- [ ] Unescape: `cook_escape/mod.rs` has a shared scanning core called by both `unescape_string_v2` and `unescape_template_v2`
- [ ] Integer cooking: `numeric.rs` has 1 generic function with radix parameter (down from 3)
- [ ] Duration/size: `duration_size.rs` has 1 generic cooking function + 1 data-driven suffix detector (down from 2+2)
- [ ] All existing tests pass unchanged — these are pure refactors with zero behavioral change
- [ ] Satisfies mission criteria for template, unescape, integer, and duration/size consolidation

**Context:** The cooker layer has four clusters of algorithmically-duplicated functions identified during the hygiene review. Each cluster shares a multi-step control-flow skeleton where only types, error kinds, or context values differ. If the protocol changes (e.g., new escape sequence, new numeric prefix), multiple functions must be updated in lockstep — a drift risk.

**Depends on:** Section 01 (bug fix must land first — we don't refactor code with known bugs).

---

## 02.1 Template cooking consolidation

**File(s):** `compiler/ori_lexer/src/cooker/escape_cooking.rs`

Four functions — `cook_template_head` (line 55), `cook_template_middle` (line 75), `cook_template_tail` (line 95), `cook_template_complete` (line 124) — share identical skeleton:

```
strip delimiters → unescape_template_v2(content, offset, errors) →
  match Some(s) → intern_owned(s)
  match None    → intern(content)
→ check errors.len() > errors_before → CookResult::with_error / CookResult::new
```

They differ ONLY in the `TokenKind` variant constructor.

- [ ] Extract a shared helper:
  ```rust
  fn cook_template_segment(
      &mut self, offset: u32, len: u32,
      kind_fn: impl FnOnce(Name) -> TokenKind,
  ) -> CookResult {
      let errors_before = self.errors.len();
      let text = slice_source(self.source, offset, len);
      let content = &text[1..text.len() - 1];
      let content_offset = offset + 1;
      let name = match unescape_template_v2(content, content_offset, &mut self.errors) {
          Some(unescaped) => self.interner.intern_owned(unescaped),
          None => self.interner.intern(content),
      };
      let kind = kind_fn(name);
      if self.errors.len() > errors_before {
          CookResult::with_error(kind)
      } else {
          CookResult::new(kind)
      }
  }
  ```

- [ ] Replace 4 functions with thin wrappers calling `cook_template_segment`:
  ```rust
  pub(super) fn cook_template_head(&mut self, o: u32, l: u32) -> CookResult {
      self.cook_template_segment(o, l, TokenKind::TemplateHead)
  }
  ```

- [ ] Verify: `timeout 150 cargo test -p ori_lexer` — all template tests pass unchanged
- [ ] File stays under 500 lines (currently 142 → will shrink to ~60)

---

## 02.2 Unescape function consolidation

**File(s):** `compiler/ori_lexer/src/cook_escape/mod.rs`

`unescape_string_v2` (line 182, 84 lines) and `unescape_template_v2` (line 336, 94 lines) share ~80% of their control-flow skeleton. Both:

1. Fast-path check for backslash (string) or backslash/braces (template)
2. Allocate `String::with_capacity(content.len())`
3. While loop: backslash → context-specific escape / common escape / unicode escape / invalid escape; else → ASCII fast path / multi-byte
4. Return `Some(result)`

They differ in:
- Context-specific escapes: string handles `\"` valid + `\'` error; template handles `` \` `` valid + `{{`/`}}`
- Error factory: `invalid_string_escape` vs `invalid_template_escape`
- Fast-path predicate: `contains('\\')` vs `contains('\\') || windows(2).any(braces)`

**Approach:** Extract the shared scanning loop into a helper parameterized by an `EscapeContext` trait or enum:

- [ ] Define an escape context enum or use closures:
  ```rust
  enum EscapeContext { String, Template }
  ```

- [ ] Extract `unescape_with_context(content, base_offset, errors, ctx) -> Option<String>`:
  - Shared: fast-path decision, buffer allocation, while loop, common escapes, unicode escapes, ASCII fast path, multi-byte handling
  - Parameterized: context-specific escape match arm, error factory, brace handling (template only)

- [ ] `unescape_string_v2` and `unescape_template_v2` become thin wrappers calling the shared core

- [ ] `unescape_char_v2` (line 275) is structurally different (single-char, returns `char` not `Option<String>`) — leave it as-is

- [ ] Verify: `timeout 150 cargo test -p ori_lexer` — all escape tests pass unchanged
- [ ] File stays under 500 lines (currently 432 → should shrink to ~300)

- [ ] **TPR checkpoint** — `/tpr-review` covering 02.1–02.2 implementation work

---

## 02.3 Integer cooking consolidation

**File(s):** `compiler/ori_lexer/src/cooker/numeric.rs`

Three functions — `cook_int` (line 11), `cook_hex_int` (line 21), `cook_bin_int` (line 34) — share identical skeleton:

```
slice_source → strip prefix (optional) → parse_int_skip_underscores(text, radix) →
  Some(n) → CookResult::new(TokenKind::Int(n))
  None    → push overflow error → CookResult::with_error(TokenKind::Error)
```

They differ only in: radix (10/16/2), prefix length (0/2/2), error factory (int_overflow/hex_int_overflow/bin_int_overflow).

- [ ] Extract a shared helper:
  ```rust
  fn cook_int_radix(
      &mut self, offset: u32, len: u32,
      prefix_len: usize, radix: u32,
      overflow_error: fn(Span) -> LexError,
  ) -> CookResult {
      let text = slice_source(self.source, offset, len);
      let digits = &text[prefix_len..];
      if let Some(n) = parse_int_skip_underscores(digits, radix) {
          CookResult::new(TokenKind::Int(n))
      } else {
          self.errors.push(overflow_error(span(offset, len)));
          CookResult::with_error(TokenKind::Error)
      }
  }
  ```

- [ ] Replace 3 functions with thin wrappers
- [ ] Keep `cook_float` as-is (different return type, different parse function)
- [ ] Verify: `timeout 150 cargo test -p ori_lexer` — all numeric tests pass unchanged
- [ ] File stays under 500 lines (currently 57 → will shrink to ~30)

---

## 02.4 Duration/size consolidation

**File(s):** `compiler/ori_lexer/src/cooker/duration_size.rs`

**Cooking functions:** `cook_duration` (line 10) and `cook_size` (line 54) share identical skeleton:

```
detect suffix → extract num_part → if decimal { parse_decimal_unit_value → validate } 
  else { parse_int → validate } → error paths
```

They differ in: suffix detector, unit type, validation method (`to_nanos`/`to_bytes`), `TokenKind` variant.

**Suffix detectors:** `detect_duration_suffix` (line 155) and `detect_size_suffix` (line 178) share identical pattern: check 2-byte suffix from end → check 1-byte suffix → return `(0, default)`.

- [ ] Create a data-driven suffix lookup:
  ```rust
  struct SuffixEntry { text: &'static [u8], len: usize, unit_index: usize }
  ```
  or keep the two small functions (they're only 20 lines each). The algorithmic duplication is real but small — pragmatic call: if extraction would be MORE code than the duplication, keep as-is and document with a `// Note: parallel structure with detect_size_suffix`.

- [ ] Extract shared cooking logic:
  ```rust
  fn cook_unit_literal(
      &mut self, offset: u32, len: u32,
      detect_suffix: fn(&str) -> (usize, U),
      multiplier: fn(&U) -> u64,
      to_base: fn(&U, u64) -> Option<u64>,
      make_kind: fn(u64, U) -> TokenKind,
  ) -> CookResult
  ```
  Or use a trait: `trait UnitLiteral { type Unit; fn detect_suffix(...); fn to_base_units(...); fn make_token(...); }`

- [ ] Verify: `timeout 150 cargo test -p ori_lexer` — all duration/size tests pass unchanged
- [ ] File stays under 500 lines (currently 194 → should stay ~180 or shrink)

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] Template cooking: 4 functions → 1 generic + 4 call sites in `escape_cooking.rs`
- [ ] Unescape: shared scanning core in `cook_escape/mod.rs`
- [ ] Integer cooking: 3 functions → 1 generic in `numeric.rs`
- [ ] Duration/size: consolidated in `duration_size.rs` (cooking + optionally suffix detection)
- [ ] All existing tests pass unchanged (pure refactoring — zero behavioral change)
- [ ] `timeout 150 cargo test -p ori_lexer` green (debug)
- [ ] `timeout 150 cargo test -p ori_lexer --release` green (release)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] No files exceed 500-line limit
- [ ] Plan annotation cleanup: no stale annotations
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** All four algorithmic duplication clusters in the cooker layer are consolidated. Each cluster has exactly one canonical implementation. Changing the protocol (new escape, new radix, new unit suffix) requires updating exactly one function. All existing tests pass unchanged, proving zero behavioral change.
