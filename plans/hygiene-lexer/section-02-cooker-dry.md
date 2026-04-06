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
    status: not-started
  - id: "02.3"
    title: "Integer cooking consolidation"
    status: complete
  - id: "02.4"
    title: "Duration/size consolidation"
    status: complete
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
- [ ] Unescape: `cook_escape/mod.rs` has a shared scanning core called by both `unescape_string_v2` and `unescape_template_v2`, with identical fast-path behavior and error spans/contexts to today
- [ ] Integer cooking: `numeric.rs` has 1 generic function with radix parameter (down from 3)
- [ ] Duration/size: `duration_size.rs` has 1 canonical cooking implementation; suffix detection is consolidated only if the resulting code is clearer than the current two helpers
- [ ] All existing tests pass unchanged — these are pure refactors with zero behavioral change
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

- [ ] **Baseline**: `timeout 150 cargo test -p ori_lexer` green before any changes
- [ ] Define a context policy (`enum`, zero-sized type, or small trait) that explicitly covers:
  - fast-path predicate (string: `contains('\\')`, template: backslash OR adjacent braces)
  - context-specific simple escape (`\"` for string, `` \` `` for template)
  - context-specific error escape (`\'` for string — pushes char despite error; absent for template)
  - brace-pair handling (`{{` / `}}`, template only — strings have NO brace collapsing)
  - invalid-escape factory (`invalid_string_escape` vs `invalid_template_escape`)
  - `LexErrorContext` construction (`InsideString { start }` vs `InsideTemplate { start, nesting: 0 }`)
  - an obvious insertion point for `\\xHH` hex byte escapes — spec-defined (07-lexical-elements.md line 292, grammar.ebnf lines 116-118), tracked in roadmap section-15C.13; char/byte contexts also need `\\xHH`

- [ ] Avoid over-generic closure soup in the inner loop. The refactor should reduce drift, not spread the policy across 5 independent callbacks that are harder to audit than the duplication.

- [ ] Define an escape context enum or use a small policy object:
  ```rust
  enum EscapeContext { String, Template }
  ```

- [ ] Extract `unescape_with_context(content, base_offset, errors, ctx) -> Option<String>`:
  - Shared: fast-path decision, buffer allocation, while loop, common escapes, unicode escapes, ASCII fast path, multi-byte handling
  - Parameterized: context-specific escape match arm, error factory, brace handling (template only), and error-context construction

- [ ] Preserve the current observable invariants:
  - string fast path returns `None` when `!content.contains('\\')` — same condition as today
  - template fast path returns `None` when no backslash AND no adjacent brace pairs — same condition as today
  - malformed escape spans must remain byte-accurate (backslash offset + escape char length)
  - malformed unicode escapes must keep the same greedy recovery behavior (skip past invalid chars to `}`)
  - template lone braces (`{` and `}` not doubled) pass through unchanged as literal characters
  - template `{{` collapses to `{`, `}}` collapses to `}` (spec line 108)
  - string `\'` pushes error `single_quote_escape_in_string` BUT also pushes `'` to output (error recovery)
  - unrecognized escape in both contexts pushes error AND pushes `\u{FFFD}` (replacement character) to output — this is symmetric and must stay symmetric in the shared core
  - trailing backslash in both contexts pushes error AND pushes `\\` to output

- [ ] `unescape_string_v2` and `unescape_template_v2` become thin wrappers calling the shared core

- [ ] `unescape_char_v2` (line 275) is structurally different (single-char, returns `char` not `Option<String>`) — leave it as-is

- [ ] **Fix along the way (WASTE):** In current `unescape_template_v2` (line 416), `let b2 = bytes[i];` is a redundant rebinding — `b` from line 357 is already `bytes[i]` at this point in the else branch. The shared core should use `b` directly, not introduce a shadow variable.
- [ ] **Verify `\\xHH` extension point is real, not aspirational:** After implementing the shared core, manually verify that adding `\\xHH` support would require exactly ONE new match arm in the shared escape-handling logic (the `'x'` case alongside existing `'u'`, `'"'`/`` '`' ``). The verification is: grep the production code for `resolve_common_escape` or the main escape match — there must be exactly ONE location where a new `'x' =>` arm would go. If there are two or more, the consolidation did not achieve its goal.
- [ ] Verify: `timeout 150 cargo test -p ori_lexer` — all escape tests pass unchanged
- [ ] Add or keep focused regression coverage for:
  - string fast-path `None` behavior (no backslash -> returns None, intern source directly)
  - template fast-path `None` behavior (no backslash AND no adjacent braces -> returns None)
  - malformed unicode escape span + context in both string and template modes
  - template brace handling (`{{` -> `{`, `}}` -> `}`, lone `{` passthrough, lone `}` passthrough)
  - string `\'` error recovery (pushes error AND pushes `'` char to output)
  - string `\"` valid escape vs template `` \` `` valid escape (context-specific delimiter)
  - `LexErrorContext` construction (string: `InsideString { start }`, template: `InsideTemplate { start, nesting: 0 }`)
  - trailing backslash error + output in both contexts
  - unrecognized escape output: both contexts push error AND push `\u{FFFD}` (replacement character) — must stay symmetric in the shared core
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

- [ ] `[TPR-02-001][high]` The current unescape refactor sketch is underspecified at exactly the places most likely to regress: fast-path behavior, `LexErrorContext`, byte-accurate error spans, and future extension points for spec-required escape forms such as `\\xHH`.
  Evidence: `unescape_string_v2` and `unescape_template_v2` currently differ not only in escape acceptance, but also in fast-path predicates, brace handling, and context construction in [compiler/ori_lexer/src/cook_escape/mod.rs](/home/eric/projects/ori_lang/.claude/worktrees/lexer-hygiene/compiler/ori_lexer/src/cook_escape/mod.rs). The spec also defines `\\xHH` escapes for strings, templates, and chars in [07-lexical-elements.md](/home/eric/projects/ori_lang/.claude/worktrees/lexer-hygiene/docs/ori_lang/v2026/spec/07-lexical-elements.md#L281).
  Impact: A generic helper designed only around the current String/Template split can preserve today's duplication while still making the next correctness fix harder. It also risks subtle behavior drift that existing broad "tests still pass" wording would not localize.
  Required plan fix: make the context-policy contract explicit, preserve current observable invariants, and require focused regression checks for fast path, unicode error spans, and template brace handling.

- [ ] `[TPR-02-002][medium]` The section overcommits to generic suffix-detector consolidation even though the validated duplication lives mainly in the duration/size cooking skeleton, not in the short suffix helpers.
  Evidence: the heavy duplication is in `cook_duration` / `cook_size`, while `detect_duration_suffix` / `detect_size_suffix` are each small, domain-specific end-of-string matches in [duration_size.rs](/home/eric/projects/ori_lang/.claude/worktrees/lexer-hygiene/compiler/ori_lexer/src/cooker/duration_size.rs).
  Impact: forcing a generic detector risks replacing straightforward code with a harder-to-audit table/trait layer that saves little and obscures the remaining semantic asymmetry between duration and size validation.
  Required plan fix: make suffix-detector consolidation optional and success-based on clarity, not on eliminating every repeated branch.

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
