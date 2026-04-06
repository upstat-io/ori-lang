---
section: "06"
title: "Lexer/Parser DRY"
status: in-progress
reviewed: true
goal: "Finish the remaining lexer/parser DRY work in raw operator scanning and parser identifier acceptance, while recording the cooking refactors already present in tree"
success_criteria:
  - "6 compound-assignment operator functions reduced to 1 helper + 6 one-liner callers"
  - "Parser expect_ident/expect_member_name/expect_ident_or_keyword share a common ident+soft-keyword prefix helper"
  - "is_keyword_usable_as_ident derived from soft_keyword_to_name + keyword_as_name (no independent triple-maintained list)"
  - "expect_member_name uses its own error factory with member-specific wording"
  - "All existing tests pass unchanged — zero behavioral change"
  - "Cursor/mod.rs decorative banners removed (Section 07 overlap — clean what you touch)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Parameterize Template Cooking Functions"
    status: complete
  - id: "06.2"
    title: "Parameterize Numeric Cooking Functions"
    status: complete
  - id: "06.3"
    title: "Extract Compound-Assignment Operator Helper"
    status: complete
  - id: "06.4a"
    title: "Extract shared ident/soft-keyword prefix"
    status: complete
  - id: "06.4b"
    title: "Fix expect_member_name() diagnostic"
    status: complete
  - id: "06.4c"
    title: "Eliminate is_keyword_usable_as_ident() triple-maintenance"
    status: complete
  - id: "06.4d"
    title: "Remove cursor/mod.rs decorative banners"
    status: complete
  - id: "06.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "06.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 06: Lexer/Parser DRY

**Status:** In Progress
**Goal:** The cooking refactors are already present in the lexer. Remaining work is limited to the 6 `*=`, `+=`, `%=`-style raw-scanner helpers and the shared identifier/soft-keyword acceptance prefix in the parser.

**Context:** This section drifted from the codebase. In the current tree:
- `compiler/ori_lexer/src/cooker/escape_cooking.rs` already has `cook_template_segment()` and all four template cookers are one-line wrappers.
- `compiler/ori_lexer/src/cooker/numeric.rs` already has `cook_int_radix()`.
- `compiler/ori_lexer/src/cooker/duration_size.rs` already has `cook_unit_literal<U: UnitCooking>()` plus the full trait abstraction.

The only remaining work is 06.3 and 06.4. Those are still low-risk, but they are not as mechanically identical as this section previously claimed: the raw-scanner helper must respect the existing hot-path API shape, and the parser functions diverge in accepted token classes, keyword mapping, and error wording.

---

## 06.1 Parameterize Template Cooking Functions

**File(s):** `compiler/ori_lexer/src/cooker/escape_cooking.rs`

Already implemented in the current tree:
- `cook_template_segment()` exists in `compiler/ori_lexer/src/cooker/escape_cooking.rs`
- `cook_template_head`, `cook_template_middle`, `cook_template_tail`, and `cook_template_complete` are already thin wrappers
- Lexer cooker tests already cover all four template forms plus escape/error propagation

- [x] `cook_template_segment()` exists and is used by all 4 template cookers
- [x] Targeted verification: `timeout 150 cargo test -p ori_lexer cooker -- --nocapture`

---

## 06.2 Parameterize Numeric Cooking Functions

**File(s):** `compiler/ori_lexer/src/cooker/numeric.rs`, `compiler/ori_lexer/src/cooker/duration_size.rs`

Already implemented in the current tree:
- `cook_int_radix()` exists in `compiler/ori_lexer/src/cooker/numeric.rs`
- `cook_unit_literal<U: UnitCooking>()` exists in `compiler/ori_lexer/src/cooker/duration_size.rs`
- `cook_duration()` and `cook_size()` are already thin wrappers
- Lexer cooker tests already cover decimal/integer duration and size cooking, suffix detection, and overflow paths

- [x] Integer and unit cooking helpers already exist and are in use
- [x] Targeted verification: `timeout 150 cargo test -p ori_lexer cooker -- --nocapture`

---

## 06.3 Extract Compound-Assignment Operator Helper

**File(s):** `compiler/ori_lexer_core/src/raw_scanner/operators.rs`

6 functions (`plus`, `star`, `percent`, `caret`, `at`, `bang`) follow the same pattern: advance, check `b'='`, return compound variant or single variant.

**Excluded:** `minus_or_arrow` (has `b'>'` arrow branch), `equal` (has `b'='` and `b'>'` branches), `less` (has `b'<'` shift + `b'='` branches), `dot` (multi-dot logic), `pipe` (has `b'|'` double-pipe branch), `ampersand` (has `b'&'` double-ampersand branch), `question` (checks `b'?'` not `b'='`, not a compound assignment), `slash_or_comment` (has `b'/'` comment branch). These have extra branches that are semantically different; broadening the abstraction makes the hot path harder to read.

- [x] Extract a tiny private helper for the exact `single-or-=` pattern using the existing raw-scanner cursor API (`self.cursor.advance()`, `self.cursor.current()`, `self.cursor.pos()`). Do not introduce fictional helpers like `try_eat()`/`tok()` unless they are independently justified across the file.
  ```rust
  /// Advance past an operator char; if `=` follows, consume it and emit
  /// the compound tag, otherwise emit the single-char tag.
  #[inline]
  fn compound_eq(&mut self, start: u32, single: RawTag, compound: RawTag) -> RawToken {
      self.cursor.advance();
      if self.cursor.current() == b'=' {
          self.cursor.advance();
          RawToken { tag: compound, len: self.cursor.pos() - start }
      } else {
          RawToken { tag: single, len: self.cursor.pos() - start }
      }
  }
  ```
- [x] Rewrite only `plus`, `star`, `percent`, `caret`, `at`, and `bang` through that helper — each becomes a one-liner: `self.compound_eq(start, RawTag::Plus, RawTag::PlusEq)`, etc.
- [x] **Note**: `bang` currently emits `RawTag::BangEqual` (not `BangEq`). Verify the actual variant name before wiring — the helper's `compound` parameter must match the existing tag name exactly.
- [x] Keep the helper trivial and inlinable; `#[inline]` is acceptable here because this is a scanner hot path (per `#[inline]` rules: 1-5 lines freely), but avoid closures, trait objects, or function-pointer dispatch
- [x] Verify: `timeout 150 cargo test -p ori_lexer_core` passes

---

## 06.4 Unify Parser Identifier Acceptance

**File(s):** `compiler/ori_parse/src/cursor/mod.rs`

`expect_ident()`, `expect_member_name()`, and `expect_ident_or_keyword()` share the same identifier/soft-keyword prefix, but they diverge after that:
- `expect_member_name()` accepts any keyword plus integer literals
- `expect_ident_or_keyword()` accepts only the positional-keyword subset from `keyword_as_name()`
- `expect_ident()` accepts neither of those extra cases
- `expect_member_name()` currently reuses `make_expect_ident_error()`, so its failure path says "expected identifier" when it should say "expected member name" (diagnostic bug)

**Execution note:** Land 06.4 in this order: 06.4c -> 06.4a -> 06.4b -> 06.4d. That establishes the shared keyword-classification source first, then extracts the shared token-taking prefix on top of it, then fixes the member-name-specific diagnostic, and leaves banner cleanup as the tail cleanup step.

**Cross-section note:** `compiler/ori_parse/src/cursor/mod.rs` is 665 lines (exceeds the 500-line limit — **BLOAT**). It is already tracked for a later split in Section 08.4. Keep the 06.4 helpers narrow and colocated so Section 08.4 can later extract the identifier-acceptance block wholesale into `cursor/identifiers.rs`; do not turn 06 into a file-splitting section.

### 06.4a Shared prefix extraction

The shared prefix across all three `expect_*` functions is: (1) check for `Ident(name)` -> advance -> `Ok(name)`, (2) check `soft_keyword_str()` -> intern -> advance -> `Ok(name)`. After 06.4c lands, this prefix can call the free functions directly.

- [x] Extract the shared prefix into a helper: `take_ident_or_soft_keyword(&mut self) -> Option<Name>` that consumes and returns `Some(name)` or returns `None` without advancing
- [x] Rewrite `expect_ident()`, `expect_member_name()`, and `expect_ident_or_keyword()` to call `take_ident_or_soft_keyword()` first, then fall through to their type-specific acceptance logic (keywords, integers) and type-specific error factory
- [x] Keep the three public `expect_*` wrappers separate so each one owns its extra acceptance rules and error factory
- [x] If additional helpers are useful, keep them narrow and semantic: e.g. `take_any_keyword_name()`, `take_named_arg_keyword_name()`, `take_int_name()`
- [x] Do not introduce an `IdentAcceptMode` enum — the three functions diverge in accepted token classes, keyword mapping, and diagnostics; an enum would obscure those differences without eliminating the distinct branches

### 06.4b Fix `expect_member_name()` diagnostic (bug)

`expect_member_name()` (line 557) reuses `make_expect_ident_error()`, which says "expected identifier, found X". In member-name context (after `.`), the correct message is "expected member name, found X" because member names accept keywords and integers.

**TDD order**: write the diagnostic test FIRST (it will fail with "expected identifier"), then fix the error factory.

- [x] **Test first**: add a diagnostic test in `cursor/tests.rs` that positions a cursor after a `.` token (e.g., `TestCtx::new("foo.!")` then advance past `foo` and `.`), calls `expect_member_name()`, and asserts the error message contains "expected member name"
- [x] **Negative pin**: add a companion assertion that the error message does NOT contain "expected identifier" — this forbid-output pin proves the old wording is gone, not just that the new wording is present
- [x] **Verify test fails**: the test should fail because the current code says "expected identifier"
- [x] Add a dedicated `make_expect_member_name_error()` (model on `make_expect_ident_error()` at line 564, `#[cold] #[inline(never)]`) that produces "expected member name, found X" (still E1004 — same error code, better wording)
- [x] Wire `expect_member_name()` to use the new error factory
- [x] **Verify test passes unchanged**

### 06.4c Eliminate `is_keyword_usable_as_ident()` triple-maintenance

`is_keyword_usable_as_ident()` (free function, lines 637-662) manually duplicates the union of `soft_keyword_to_name()` and `keyword_as_name()`. Adding a keyword requires editing three places. The existing `keyword_as_ident_consistency` test catches drift but does not prevent it.

- [x] Extract keyword-classification logic from the `&self` Cursor methods into two free functions:
  - `soft_keyword_str(kind: &TokenKind) -> Option<&'static str>` — extracted from `soft_keyword_to_name()` (lines 376-395), which accesses only `self.current_kind()` and no other `self` state
  - `positional_keyword_str(kind: &TokenKind) -> Option<&'static str>` — extracted from `keyword_as_name()` (lines 599-609), which also accesses only `self.current_kind()`
- [x] Rewrite `soft_keyword_to_name(&self)` as: `soft_keyword_str(self.current_kind())`
- [x] Rewrite `keyword_as_name(&self)` as: `positional_keyword_str(self.current_kind())`
- [x] Rewrite `is_keyword_usable_as_ident(kind)` to delegate: `soft_keyword_str(kind).is_some() || positional_keyword_str(kind).is_some()` — eliminates the independent 18-variant match list
- [x] Update the `keyword_as_ident_consistency` tests if needed (they may simplify now that the free function delegates, but `KEYWORD_AS_IDENT_TOKENS` is still valuable as an independent validation list)
- [x] Verify: `timeout 150 cargo test -p ori_parse` passes

### 06.4d Remove decorative banners (Section 07 overlap)

`cursor/mod.rs` has decorative `// ─────...` banners (lines 218, 424). Per hygiene rules: "if you touch a file with decorative banners, remove them."

- [x] Treat this as cleanup attached to the last 06.4 code change in `cursor/mod.rs`, not as a standalone refactor with independent design work
- [x] Replace the two `// ─────...` banners in `cursor/mod.rs` (line 218 "TokenFlags Access", line 424 "Token Capture") with plain `// TokenFlags Access` / `// Token Capture` comments (or remove if the section break adds no value)
- [x] Also remove the decorative `// ─────...` banner in `cursor/tests.rs` (lines 177-179, "TokenFlags tests") — same file family, same cleanup
- [x] Verify: `timeout 150 cargo test -p ori_parse` passes

---

## 06.R Third Party Review Findings

- [x] 06.1 and 06.2 were already implemented in the current tree; the plan previously duplicated completed work.
- [x] The old 06.3 helper sketch referenced non-existent raw-scanner helpers (`try_eat`, `tok`) and needed to be rewritten against the actual API.
- [x] The old 06.4 `IdentAcceptMode` proposal over-abstracted functions that differ in accepted token kinds, keyword mapping, and diagnostics.
- [x] `expect_member_name()` currently reuses `make_expect_ident_error()`, which produces an imprecise diagnostic for member-name contexts.

---

## 06.T Test Strategy

06.1 and 06.2 are already covered by existing cooker tests. Remaining risk is concentrated in token acceptance boundaries and diagnostic drift for 06.3/06.4.

### Pre-existing coverage (verified)

- [x] Confirm current completed work remains covered: `timeout 150 cargo test -p ori_lexer cooker -- --nocapture`
- [x] Confirm current scanner coverage: `timeout 150 cargo test -p ori_lexer_core raw_scanner -- --nocapture`
- [x] Confirm current cursor coverage: `timeout 150 cargo test -p ori_parse cursor -- --nocapture`

### 06.3 tests (compound-assignment helper)

- [x] Add or extend raw-scanner tests only if needed to pin the helper rewrite mechanically; existing `compound_assignment_tokens()` already covers the affected operator set
- [x] Verify `timeout 150 cargo test -p ori_lexer_core` passes after 06.3

### 06.4a tests (shared prefix helper)

- [x] Add cursor tests for the actual acceptance matrix:
  - `expect_ident()` accepts `Ident` and soft keywords, rejects reserved keywords and integers
  - `expect_member_name()` accepts `Ident`, soft keywords, reserved keywords after `.`, and integer tuple fields
  - `expect_ident_or_keyword()` accepts `Ident`, soft keywords, and the `keyword_as_name()` subset, but rejects integer literals and unrelated reserved keywords
  - Note: existing tests (`soft_keyword_covers_canonical_subset`, `keyword_as_name_covers_canonical_subset`, `keyword_as_ident_consistency_{positive,negative}`) already cover this matrix.

### 06.4b tests (member-name diagnostic fix)

- [x] Diagnostic test is written FIRST in 06.4b (TDD: test before fix) — verify it fails with "expected identifier" before the fix, passes with "expected member name" after
- [x] Negative forbid-output pin: assert the error message does NOT contain "expected identifier" after the fix — prevents regression to the old wording

### 06.4c tests (is_keyword_usable_as_ident delegation)

- [x] Verify `keyword_as_ident_consistency_positive` and `keyword_as_ident_consistency_negative` still pass after the delegation rewrite
- [x] `KEYWORD_AS_IDENT_TOKENS` retained as independent validation list (valuable even with delegation)

### Verification gates

- [x] Verify `timeout 150 cargo test -p ori_parse cursor -- --nocapture` passes after 06.4 before expanding to the full crate
- [x] Verify `timeout 150 cargo test -p ori_lexer` passes after any lexer-side follow-up
- [x] Verify `timeout 150 cargo test -p ori_parse` passes after 06.4
- [x] Verify `timeout 150 ./test-all.sh` passes after all sub-sections complete

---

## 06.N Completion Checklist

- [x] Template cooking functions already parameterized in the current tree
- [x] Numeric cooking functions already parameterized in the current tree
- [x] Compound-assignment operators extracted (6 -> 1 helper + 6 one-liner callers)
- [x] Parser identifier acceptance shares the common ident/soft-keyword prefix without obscuring the distinct acceptance rules or diagnostics
- [x] `expect_member_name()` uses dedicated `make_expect_member_name_error()` with "expected member name" wording
- [x] `is_keyword_usable_as_ident()` delegates to the keyword-classification functions (no independent match list)
- [x] Decorative `// ─────...` banners removed from `cursor/mod.rs` and `cursor/tests.rs`
- [x] Cursor tests cover the acceptance matrix for `expect_ident`, `expect_member_name`, and `expect_ident_or_keyword`
- [x] Diagnostic test pins "expected member name" wording (positive pin + negative forbid-output pin)
- [x] `timeout 150 cargo test -p ori_lexer_core` passes
- [x] `timeout 150 cargo test -p ori_lexer` passes
- [x] `timeout 150 cargo test -p ori_parse` passes
- [x] `timeout 150 ./test-all.sh` passes
- [x] `./clippy-all.sh` clean
- [ ] Update frontmatter `status: complete` in this file
- [ ] Update `00-overview.md` Quick Reference table: Section 06 status -> Complete
- [ ] Update `index.md`: Section 06 status -> Complete
- [ ] `/tpr-review` covering Section 06
- [ ] `/impl-hygiene-review last commit`
