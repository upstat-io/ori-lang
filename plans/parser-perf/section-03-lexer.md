---
section: "03"
title: "Lexer Optimizations"
status: not-started
reviewed: false
goal: "Measurable throughput improvement in ori_lexer benchmarks via targeted inline annotations, arena pre-allocation, and cooker fast-path expansion."
inspired_by:
  - "Chumsky src/stream.rs (batch token caching) — batch pull strategy"
  - "Rust rustc_lexer/src/lib.rs — selective #[inline] on hot-path functions only"
  - "Ori memory: 20-30% gain from #[inline] on parser Index trait (2026-02)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Cross-Crate Inline Audit"
    status: not-started
  - id: "03.2"
    title: "Token Pre-Allocation Tuning"
    status: not-started
  - id: "03.3"
    title: "Cooker Fast-Path Expansion"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Lexer Optimizations

**Status:** Not Started
**Goal:** Cooked lexer throughput improves from ~208-240 MiB/s baseline (Section 01) by at least 10%, verified via `cargo bench -p oric --bench lexer -- "lexer/raw"`. No regressions in raw scanner benchmarks or test suite.

**Context:** The lexer is already well-optimized with const-generic monomorphization (`WITH_METADATA`), a trivial-token fast path (`try_trivial()`), and an `IdentCache` (256-entry fixed array). However, research identified three actionable gaps: (1) not all cross-crate hot functions have `#[inline]` — the 2026-02 optimization round focused on parser functions but the lexer-to-parser boundary still has unannotated calls; (2) `LexOutput::with_capacity()` uses `source_len / 2 + 1` which may over-allocate for identifier-heavy code; (3) the cooker's `cook()` method dispatches through a match even for tokens that could be intercepted earlier.

**Reference implementations:**
- **Rust** `rustc_lexer/src/lib.rs`: Very selective `#[inline]` — only on functions called in the hot tokenize loop. Relies on LTO for cross-crate inlining. Ori can't rely on LTO (Salsa + multi-crate), so explicit `#[inline]` is necessary.
- **Chumsky** `src/stream.rs`: Batch token caching (512-item pulls). NOT applicable — Ori lexes upfront into `TokenList`, which is already a superior strategy for compilation.

**Depends on:** Section 01 (baselines established).

---

## 03.1 Cross-Crate Inline Audit

**File(s):** `compiler/ori_lexer/src/driver.rs`, `compiler/ori_lexer/src/cooker/mod.rs`, `compiler/ori_lexer/src/trivial/mod.rs`, `compiler/ori_lexer/src/output.rs`, `compiler/ori_ir/src/token/list.rs`

Audit all functions called in the lexer driver loop (`lex_driver()`) and ensure hot-path functions have `#[inline]`. The driver loop processes every token in the source file — any function call overhead here multiplies by token count.

**Note on `ori_ir` inline annotations:** This section covers `TokenList::push_with_tag/push_with_flags` in `ori_ir`. Section 04.1 covers `ExprArena::alloc_expr/get_expr` and `Expr::new` in `ori_ir`. Both sections modify `ori_ir` — coordinate if working in parallel.

- [ ] **Write benchmark baseline test**: Run `cargo bench -p oric --bench lexer -- "lexer/raw"` and record results before any changes.

- [ ] Audit and annotate functions called per-token in `lex_driver()`:
  - `RawScanner::next_token()` — called once per token. Verify `#[inline]` in `ori_lexer_core`.
  - `try_trivial(raw.tag)` — called for every non-trivia token. Currently in `trivial/mod.rs`. Verify `#[inline]`.
  - `cooker.cook(raw.tag, offset, raw.len)` — already `#[inline]`. Verify.
  - `cooker.set_last_non_trivia(raw.tag)` — called for every non-trivia token. Method on `TokenCooker` in `cooker/mod.rs:140`. Currently **missing `#[inline]`** — annotate.
  - `output.tokens.push_with_tag(token, tag, flags)` — called per token. Crosses crate boundary (`TokenList` is in `ori_ir`). **Already has `#[inline]`** (verified in `ori_ir/src/token/list.rs:118`).
  - `output.tokens.push_with_flags(token, flags)` — called for newlines. **Already has `#[inline]`** (verified in `ori_ir/src/token/list.rs:107`).
  - `finalize_flags(pending_flags)` — already `#[inline]`.
  - `make_span(offset, raw.len)` — already `#[inline]`.

- [ ] Annotate any unannotated hot-path functions with `#[inline]`. Per hygiene rules: 1-5 lines freely, 6-20 lines only if profiling shows benefit or cross-crate hot path.

- [ ] **Verify improvement**: Run benchmarks again, compare against baseline. Expected: 5-15% improvement if `TokenList::push_with_tag()` was unannotated.

**Matrix dimensions:**
- Input types: simple functions, string-heavy, expression-heavy (from Section 01)
- Sizes: 100, 500, 1000 functions
- Metric: MiB/s throughput

**Semantic pin:** `TokenList::push_with_tag()` and `push_with_flags()` already have `#[inline]`. The primary gain is from annotating `set_last_non_trivia()` (currently unannotated, called per non-trivia token). Benchmark improvement on 1000-function workload should be measurable (> 1%).

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-03.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 03.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 03.2 Token Pre-Allocation Tuning

**File(s):** `compiler/ori_lexer/src/output.rs`

Current pre-allocation uses `source_len / 2 + 1` tokens. This is a reasonable estimate (~1 token per 2-3 bytes) but may waste memory for identifier-heavy code or under-allocate for operator-heavy code.

- [ ] Profile actual token counts for different workload types. **WHERE:** Add temporary instrumentation to `lex_driver()` in `compiler/ori_lexer/src/driver.rs` (line ~34, after the function entry) to log `output.tokens.len()` and `source.len()` at the end of lexing. Run each benchmark workload and record:
  - Simple functions: tokens per byte ratio (expected: ~0.3-0.5 tokens/byte)
  - String-heavy: tokens per byte ratio (expected: fewer tokens/byte due to long string literals)
  - Expression-heavy: tokens per byte ratio (expected: more tokens/byte due to many operators)

- [ ] **WHERE:** `compiler/ori_lexer/src/output.rs`, `LexOutput::with_capacity()` (line ~40) and `LexOutput::with_token_capacity()` (line ~55). Tune based on profiling:
  - If max ratio across workloads / min ratio < 2x: keep current `source_len / 2 + 1` heuristic, document the measured ratio
  - If ratio variance > 2x: tighten default to measured average, e.g., `source_len / 3 + 1`
  - **Decision criterion:** over-allocation wastes memory, under-allocation causes reallocation. Prefer slight over-allocation (10-20%) over any reallocation.

- [ ] Verify no reallocation occurs: after profiling, assert `output.tokens.capacity() >= output.tokens.len()` for all benchmark workloads (can add `debug_assert!` at end of `lex_driver()` or check via benchmark harness). Remove temporary instrumentation after profiling.

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-03.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 03.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 03.3 Cooker Fast-Path Expansion

**File(s):** `compiler/ori_lexer/src/cooker/mod.rs`, `compiler/ori_lexer/src/trivial/mod.rs`

The `try_trivial()` function intercepts operators and delimiters before they reach `cook()`. This avoids the cost of the `cook()` match dispatch for simple tokens. Investigate whether additional token types can bypass `cook()`.

- [ ] Profile which `RawTag` variants reach `cook()` most frequently in typical Ori code. Expected: `Ident`, `Int`, `Newline` dominate after trivial interception.

- [ ] Evaluate whether `Semicolon` can be moved into `try_trivial()`:
  - Currently excluded from `try_trivial()` but handled as `CookResult::new(TokenKind::Semicolon)` in `cook()` — a direct mapping with no data.
  - Moving it to `try_trivial()` saves the `cook()` function call overhead for every semicolon.
  - **Verified safe:** `set_last_non_trivia()` is called in the driver loop (`driver.rs:195`) AFTER both the `try_trivial()` and `cook()` branches, so moving Semicolon to `try_trivial()` does NOT affect `set_last_non_trivia()` tracking.

- [ ] Evaluate whether the `IdentCache` hit rate can be improved:
  - Current: 256-entry direct-mapped cache (hash collisions evict).
  - Profile hit rate on real Ori files. If hit rate < 80%, consider 512-entry or 2-way set-associative.
  - If hit rate > 90%, no changes needed.

- [ ] Benchmark all changes: `cargo bench -p oric --bench lexer -- "lexer/raw"`.

**Semantic pin:** Semicolon bypass in `try_trivial()` should show measurable improvement (> 1%) on code with many short statements.

- [ ] **Subsection close-out (03.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-03.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 03.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] All per-token functions in `lex_driver()` audited for `#[inline]`
- [ ] Cross-crate `TokenList::push_with_tag()` and `push_with_flags()` confirmed `#[inline]`
- [ ] Token pre-allocation heuristic profiled and tuned (or confirmed adequate)
- [ ] Semicolon bypass in `try_trivial()` evaluated (implemented or rejected with evidence)
- [ ] `IdentCache` hit rate profiled (tuned or confirmed adequate)
- [ ] Benchmark improvement measured against Section 01 baselines
- [ ] `timeout 150 cargo t -p ori_lexer` passes
- [ ] `timeout 150 cargo st` passes
- [ ] `./test-all.sh` green
- [ ] Debug AND release builds pass: `timeout 150 cargo t -p ori_lexer --release`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Correctness verification:** All changes in this section are performance-only (adding `#[inline]`, tuning allocation sizes, moving token kinds between fast paths). No behavioral change is expected. The existing lexer test suite (unit tests + spec tests) serves as the correctness regression guard. Run the full suite BEFORE any optimization to establish a green baseline, then after each change to verify zero regressions. If any test fails after an optimization, the optimization introduced a bug -- revert and investigate.

**Exit Criteria:** `cargo bench -p oric --bench lexer -- "lexer/raw"` shows throughput improvement compared to baselines. All test suites green. Final improvement measurement deferred to Section 06 (unified verification).
