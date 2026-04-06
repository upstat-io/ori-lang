---
section: "02"
title: "Cooker Layer Algorithmic DRY"
status: in-progress
reviewed: true
goal: "Consolidate 4 clusters of algorithmically-duplicated functions in the cooker layer"
success_criteria:
  - "4 template cooking functions → 1 generic with 4 call sites"
  - "2 unescape functions share a scanning core while preserving context-specific fast paths, error spans, and error context"
  - "3 integer cooking functions → 1 generic with radix parameter"
  - "2 duration/size cooking functions share one canonical implementation; suffix detection is only consolidated if the helper is simpler than the duplication"
  - "No behavioral changes — all existing tests pass unchanged"
inspired_by:
  - "rustc_lexer unescape.rs — single unescape function parameterized by Mode enum"
depends_on: ["01"]
third_party_review:
  status: findings
  updated: 2026-04-05
sections:
  - id: "02.1"
    title: "Template cooking consolidation"
    status: complete
  - id: "02.2"
    title: "Unescape function consolidation"
    status: complete
  - id: "02.3"
    title: "Integer cooking consolidation"
    status: complete
  - id: "02.4"
    title: "Duration/size consolidation"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: in-progress
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Cooker Layer Algorithmic DRY

**Status:** In Progress
**Goal:** Eliminate algorithmic duplication in the cooker layer by extracting shared control-flow skeletons into parameterized functions. Four clusters of duplication are targeted, each with identical multi-step algorithms that differ only in type parameters, error kinds, or context-specific values.

**Success Criteria:**

- [x] Template cooking: `escape_cooking.rs` has 1 generic function + 4 thin wrappers (down from 4 near-identical functions)
- [x] Unescape: `cook_escape/mod.rs` has a shared scanning core called by both `unescape_string_v2` and `unescape_template_v2`, with identical fast-path behavior and error spans/contexts to today
- [x] Integer cooking: `numeric.rs` has 1 generic function with radix parameter (down from 3)
- [x] Duration/size: `duration_size.rs` has 1 canonical cooking implementation; suffix detection kept separate (clearer than consolidation)
- [x] All existing tests pass unchanged — these are pure refactors with zero behavioral change
- [ ] Satisfies mission criteria for template, unescape, integer, and duration/size consolidation

**Context:** The cooker layer has four clusters of algorithmically-duplicated functions identified during the hygiene review. Each cluster shares a multi-step control-flow skeleton where only types, error kinds, or context values differ. If the protocol changes (e.g., new escape sequence, new numeric prefix), multiple functions must be updated in lockstep — a drift risk.

**Depends on:** Section 01 (bug fix must land first — we don't refactor code with known bugs).

**Recommended subsection order:** 02.1 (template, simplest) → 02.3 (integer, also simple) → 02.4 (duration/size, medium) → 02.2 (unescape, hardest — most behavioral invariants). Land easy wins first; the TPR checkpoint at the end of 02.2 covers the riskiest refactor.

**Execution note:** Treat 02.1, 02.3, 02.4, and 02.2 as separate landable steps rather than one single-session batch. 02.2 is large enough, and spec-sensitive enough, to deserve its own implementation/review pass.

**TDD protocol for pure refactors:** These subsections are behavior-preserving refactors, so existing tests serve as the regression suite. Each subsection must: (1) run `timeout 150 cargo test -p ori_lexer` BEFORE the refactor to confirm a green baseline, (2) perform the refactor, (3) run the same test command AFTER to verify zero regression. If the baseline is not green, stop and investigate before refactoring.

---

## 02.1 Template cooking consolidation

**File(s):** `compiler/ori_lexer/src/cooker/escape_cooking.rs`

Four functions — `cook_template_head` (line 55), `cook_template_middle` (line 75), `cook_template_tail` (line 95), `cook_template_complete` (line 124) — share identical skeleton:

```
errors_before = errors.len()
→ slice_source(source, offset, len) → strip delimiters [1..text.len()-1]
→ unescape_template_v2(content, content_offset, errors) →
  match Some(s) → interner.intern_owned(s)
  match None    → interner.intern(content)
→ if errors.len() > errors_before → CookResult::with_error(kind) else CookResult::new(kind)
```

They differ ONLY in the `TokenKind` variant constructor (`TemplateHead`, `TemplateMiddle`, `TemplateTail`, `TemplateFull`). `cook_string` and `cook_char` are excluded — `cook_string` calls `unescape_string_v2` (different unescape function), and `cook_char` returns a `char` (structurally different return path).
- [x] **Baseline**: `timeout 150 cargo test -p ori_lexer` green before any changes (285 pass)
- [x] Extract a shared helper:
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

- [x] Replace 4 functions with thin wrappers calling `cook_template_segment`:
  ```rust
  pub(super) fn cook_template_head(&mut self, o: u32, l: u32) -> CookResult {
      self.cook_template_segment(o, l, TokenKind::TemplateHead)
  }
  ```
  The 4 wrappers are: `cook_template_head`, `cook_template_middle`, `cook_template_tail`, `cook_template_complete`. Each is a single-line delegation.
- [x] Verify: `timeout 150 cargo test -p ori_lexer` — all template tests pass unchanged (285 pass)
- [x] File stays under 500 lines (142 → 110 lines)

**Note:** `cook_format_spec` (line 115) also lives in `escape_cooking.rs` but is structurally different — no unescape processing, strips `:` prefix instead of matching delimiters, no error accumulation. It is NOT a consolidation candidate.

---

## 02.2 Unescape function consolidation

**File(s):** `compiler/ori_lexer/src/cook_escape/mod.rs`

`unescape_string_v2` (line 182, 83 lines) and `unescape_template_v2` (line 336, 93 lines) share ~80% of their control-flow skeleton. Both:
1. Fast-path check for backslash (string) or backslash/braces (template)
2. Allocate `String::with_capacity(content.len())`
3. While loop: backslash → context-specific escape / common escape / unicode escape / invalid escape; else → ASCII fast path / multi-byte
4. Return `Some(result)`

They differ in:
- **Context-specific escape (backslash):** string handles `\"` (valid, pushes `"`), `\'` (error `single_quote_escape_in_string`, but still pushes `'` to output); template handles `` \` `` (valid, pushes `` ` ``). Note: string's `\'` error-recovery behavior (push char despite error) is observable and must be preserved.
- **Brace-pair handling (template only):** `{{` collapses to `{`, `}}` collapses to `}`. Strings have NO brace handling at all — `{{` in a string is two literal `{` characters (no collapsing). This asymmetry is intentional (spec line 108 applies only to template literals).
- **Error factory:** `LexError::invalid_string_escape` vs `LexError::invalid_template_escape`
- **LexErrorContext construction:** string builds `LexErrorContext::InsideString { start: base_offset }`, template builds `LexErrorContext::InsideTemplate { start: base_offset, nesting: 0 }` — the extra `nesting` field is template-specific.
- **Fast-path predicate:** `contains('\\')` vs `bytes.contains(&b'\\') || bytes.windows(2).any(|w| braces)`

**Approach:** Extract the shared scanning loop into a helper parameterized by a context policy, but do NOT collapse the real context differences into an underspecified "one big match" without naming the invariants that must survive the refactor.

- [x] **Baseline**: `timeout 150 cargo test -p ori_lexer` green before any changes (285 pass)
- [x] Define `EscapeContext` enum with 3 policy methods: `needs_processing()` (fast-path), `make_error_context()` (LexErrorContext), `push_invalid_escape()` (error factory). Context-specific escapes and brace handling are guarded match arms in the shared loop.
- [x] No closure soup — all policy in the enum methods + match guards. The shared loop is a single function, not 5 callbacks.
- [x] `EscapeContext` enum: `String` and `Template` variants.
- [x] Extract `unescape_with_context(content, base_offset, errors, ctx) -> Option<String>`:
  - Shared: fast-path decision, buffer allocation, while loop, common escapes, unicode escapes, ASCII fast path, multi-byte handling
  - Parameterized via match guards: `'"' if String`, `` '`' if Template ``, `'\'' if String` (error recovery), brace collapsing `if Template`
- [x] All 9 observable invariants preserved (verified by existing tests: `string_no_escapes_fast_path`, `template_no_escapes_fast_path`, `string_single_quote_escape_is_error`, `string_invalid_escape`, `template_invalid_escape`, `string_trailing_backslash`, `template_brace_escapes`, `template_trailing_single_brace`, plus unicode escape tests in both contexts)
- [x] `unescape_string_v2` and `unescape_template_v2` are now thin wrappers calling `unescape_with_context`
- [x] `unescape_char_v2` left as-is (structurally different)
- [x] **WASTE fixed:** Redundant `b2 = bytes[i]` eliminated — shared core uses `b` directly throughout
- [x] **`\\xHH` extension point verified:** `grep` confirms exactly ONE `match esc` dispatch at line 259 in `unescape_with_context`. Adding `\xHH` = one new `'x' =>` arm.
- [x] Verify: `timeout 150 cargo test -p ori_lexer` — all 285 tests pass unchanged (debug AND release)
- [x] All 9 regression coverage items confirmed present in existing test suite (`cook_escape/tests.rs`)
- [x] File stays under 500 lines (432 → 437 lines — slight increase from EscapeContext + docs)

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

- [x] **Baseline**: `timeout 150 cargo test -p ori_lexer` green before any changes (285 pass)
- [x] Extract a shared helper:
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

- [x] **`#[inline]` consideration:** Applied approach (a) — `#[inline]` on the shared helper `cook_int_radix`, preserving existing inlining behavior for `cook_int`.
- [x] Replace 3 functions with thin wrappers: `cook_int` (radix 10, prefix 0, `int_overflow`), `cook_hex_int` (radix 16, prefix 2, `hex_int_overflow`), `cook_bin_int` (radix 2, prefix 2, `bin_int_overflow`)
- [x] Keep `cook_float` as-is (different return type, different parse function)
- [x] Verify: `timeout 150 cargo test -p ori_lexer` — all numeric tests pass unchanged (285 pass)
- [x] File stays under 500 lines (57 → 58 lines)

---

## 02.4 Duration/size consolidation

**File(s):** `compiler/ori_lexer/src/cooker/duration_size.rs`

**Cooking functions:** `cook_duration` (line 10) and `cook_size` (line 54) share identical skeleton:

```
detect suffix → extract num_part → if decimal { parse_decimal_unit_value → validate } 
  else { parse_int → validate } → error paths
```

They differ in: suffix detector, unit type, validation method, `TokenKind` variant.

**Critical semantic asymmetry (must NOT be unified away):**
- **Decimal path (symmetric):** Both `cook_duration` and `cook_size` call `parse_decimal_unit_value` then check `i64::try_from(result).is_ok()`. This path IS shared and can be consolidated safely.
- **Integer path (asymmetric):** `cook_duration` checks `unit.to_nanos(value).is_some()` — `to_nanos` returns `Option<i64>` (i64 check is **inside** the method). `cook_size` checks `unit.to_bytes(value).is_some_and(|b| i64::try_from(b).is_ok())` — `to_bytes` returns `Option<u64>` (i64 check is **outside**, in the caller). The extra `i64::try_from` in `cook_size` exists because LLVM codegen casts bytes to `i64`. A consolidated helper must preserve this extra check for size but not for duration.

**Suffix detectors:** `detect_duration_suffix` (line 155) and `detect_size_suffix` (line 178) share similar control flow, but they are short and encode different unit domains.

- [x] **Baseline**: `timeout 150 cargo test -p ori_lexer` green before any changes (285 pass)
- [x] Suffix detectors kept separate — `detect_duration_suffix` and `detect_size_suffix` are 20/16 lines, domain-specific, and simpler than any generic alternative.

- [x] Extract shared cooking logic via Approach B (trait):
  Defined `UnitCooking` trait with 4 methods: `multiplier()`, `validate_integer()`, `make_integer_kind()`, `make_decimal_kind()`. Implemented for both `DurationUnit` and `SizeUnit`. The `validate_integer` impl for `SizeUnit` includes the extra `i64::try_from` check, preserving the semantic asymmetry.
  The shared `cook_unit_literal<U: UnitCooking>` takes only `(offset, len, detect_suffix)` — clean 3-parameter API.

- [x] Semantic asymmetry preserved:
  - `DurationUnit::validate_integer` calls `self.to_nanos(value).is_some()` — i64 check internal
  - `SizeUnit::validate_integer` calls `self.to_bytes(value).is_some_and(|b| i64::try_from(b).is_ok())` — i64 check external
  - Decimal paths use `U::make_decimal_kind` (normalizes to base unit)
  - Integer paths use `unit.make_integer_kind` (preserves original unit)

- [x] Verify: `timeout 150 cargo test -p ori_lexer` — all duration/size tests pass unchanged (285 pass)
- [x] File stays under 500 lines (194 → 225 lines — increase from trait definition + docs)

---

## 02.R Third Party Review Findings

- [x] `[TPR-02-001][high]` The current unescape refactor sketch is underspecified at exactly the places most likely to regress: fast-path behavior, `LexErrorContext`, byte-accurate error spans, and future extension points for spec-required escape forms such as `\\xHH`.
  Resolved: Addressed during plan review (Agents 1-4) on 2026-04-05. Plan expanded with 8 invariants, 9 regression items, explicit EscapeContext policy contract, and verified `\xHH` extension point. Implementation preserves all invariants.

- [x] `[TPR-02-002][medium]` The section overcommits to generic suffix-detector consolidation even though the validated duplication lives mainly in the duration/size cooking skeleton, not in the short suffix helpers.
  Resolved: Addressed during plan review on 2026-04-05. Suffix detectors kept separate in implementation — only cooking skeleton consolidated via `UnitCooking` trait.

- [ ] `[TPR-02-003][high]` The touched escape path still rejects spec-required `\\xHH` escapes. <!-- blocked-by:15C.13 -->
  Status: **Blocked** — `\xHH` is a cross-pipeline feature (scanner + cooker + type checker + evaluator + codegen) tracked with concrete `- [ ]` items in roadmap section 15C.13. Section 02 is a DRY refactoring plan, not a feature implementation plan. The refactoring *improves* the situation: adding `\xHH` now requires 1 match arm (verified) vs. the pre-refactoring 2. Code has `TODO(lexer)` with spec citation and roadmap cross-reference. This finding remains open until roadmap 15C.13 implements `\xHH`.

- [x] `[TPR-02-004][medium]` Section 02’s plan metadata drifted out of sync with the implementation that already landed.
  Resolved: Fixed on 2026-04-05. Body status text updated "Not Started" → "In Progress". Success criteria checked off (5/6 done). Overview Quick Reference table and index.md status updated to "In Progress".

- [x] `[TPR-02-005][medium]` `compiler/ori_lexer/src/cooker/escape_cooking.rs:59` — Section 02.1 had no regression coverage for the new template-segment wrappers or `FormatSpec` cooking.
  Resolved: Fixed on 2026-04-05. Added 6 regression tests in `cooker/tests.rs`: `cook_template_head_strips_delimiters_and_interns`, `cook_template_middle_strips_delimiters_and_interns`, `cook_template_tail_strips_delimiters_and_interns`, `cook_template_complete_strips_backticks_and_interns`, `cook_template_segment_with_escape`, `cook_format_spec_strips_colon_prefix`. All 4 template segment kinds + escape handling + FormatSpec now have direct cooker-level coverage (291 tests total).

- [x] `[TPR-02-006][low]` Plan metadata dates appear future-dated relative to commit timestamps.
  Resolved: Fixed on 2026-04-05. The previous review log used future-dated `2026-04-06` entries even though the local repo state and commit timestamps for this work are still on April 5, 2026. Section metadata and finding timestamps were corrected to the current local date.

- [x] `[TPR-02-007][high]` Section 02 still marks the spec-required `\\xHH` escape gap as resolved even though the production unescape path still has no `'x'` handling.
  Resolved: Fixed on 2026-04-05. TPR-02-003 reopened as explicitly **blocked** by roadmap 15C.13 (no longer marked resolved). The finding now correctly shows as open with `<!-- blocked-by:15C.13 -->` until the cross-pipeline `\xHH` feature is implemented.

- [x] `[TPR-02-008][medium]` The new Section 02.1 regression tests did not exercise `cook_template_segment()`'s error-propagation branch.
  Resolved: Fixed on 2026-04-05. Added 3 error-propagation tests in `cooker/tests.rs`: `cook_template_head_with_invalid_escape_sets_had_error`, `cook_template_middle_with_invalid_escape_sets_had_error`, `cook_template_tail_with_invalid_escape_sets_had_error`. Each verifies: (1) correct `TokenKind` variant returned despite error, (2) `had_error` is true, (3) exactly 1 `InvalidTemplateEscape` error accumulated, (4) replacement char `\u{FFFD}` in output. Total: 294 tests.

---

## 02.N Completion Checklist

- [ ] Template cooking: 4 functions → 1 generic + 4 call sites in `escape_cooking.rs`
- [ ] Unescape: shared scanning core in `cook_escape/mod.rs`
- [ ] Unescape invariants preserved: fast-path `None`, error spans, error contexts, unicode recovery, template brace behavior
- [ ] Integer cooking: 3 functions → 1 generic in `numeric.rs`
- [ ] Duration/size: cooking skeleton consolidated in `duration_size.rs`; suffix detection consolidated only if the resulting code is simpler
- [ ] All existing tests pass unchanged (pure refactoring — zero behavioral change)
- [ ] Unescape shared core has an obvious `\xHH` insertion point — verified by grep: exactly ONE match location handles escape dispatch in production code (not two separate functions). Adding `\\xHH` (roadmap 15C.13, grammar.ebnf lines 116-118) requires editing ONE match arm.
- [ ] Redundant `b2` variable eliminated from template unescape path (WASTE finding from codebase audit)
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

**Exit Criteria:** All four algorithmic duplication clusters in the cooker layer are consolidated. Each cluster has exactly one canonical implementation. Changing the protocol (new escape, new radix, new unit suffix) requires updating exactly one function. Specifically: adding `\xHH` hex byte escapes (roadmap 15C.13, grammar.ebnf lines 116-118) requires editing one match arm in the shared unescape core — verified by grep showing exactly ONE escape dispatch location in production code. All existing tests pass unchanged, proving zero behavioral change.
