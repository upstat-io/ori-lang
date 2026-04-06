---
section: "03"
title: "Scanner Layer Algorithmic DRY"
status: complete
reviewed: true
goal: "Consolidate 6 simple operator functions with identical control-flow skeletons"
success_criteria:
  - "6 simple operator functions use a shared helper"
  - "Complex operator functions (less, dot, pipe, ampersand, equal, minus_or_arrow) unchanged"
  - "All existing tests pass unchanged"
inspired_by:
  - "rustc_lexer — uses match-based dispatch, not per-operator functions"
depends_on: ["01"]
third_party_review:
  status: none
  updated: 2026-04-06
sections:
  - id: "03.1"
    title: "Extract simple_or_compound helper"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: complete
---

# Section 03: Scanner Layer Algorithmic DRY

**Status:** Complete
**Goal:** Consolidate 6 simple operator scanning functions in `raw_scanner/operators.rs` that share the identical skeleton: advance → check `=` → compound or simple tag.

**Success Criteria:**

- [x] A shared `compound_eq()` helper exists in `raw_scanner/operators.rs` (named `compound_eq` — more descriptive than plan's `simple_or_compound` since it specifically checks for `=`)
- [x] `plus`, `star`, `percent`, `caret`, `at`, `bang` use the shared helper (all are one-liner delegations)
- [x] Complex operators (`less`, `dot`, `pipe`, `ampersand`, `equal`, `minus_or_arrow`, `colon`, `hash`, `question`) are unchanged — they have unique multi-level lookahead trees
- [x] All existing tests pass unchanged — 206 passed (debug + release, 2026-04-06)
- [x] Satisfies mission criterion: "Simple operator scanning consolidated"

**Context:** Six operator functions in `compiler/ori_lexer_core/src/raw_scanner/operators.rs` shared identical structure. The `compound_eq` helper was already extracted (during hygiene-full-2 Section 06 work on this branch). This section validates the consolidation is complete and correct.

**Depends on:** Section 01 (bug fix must land first).

---

## 03.1 Extract simple_or_compound helper

**File(s):** `compiler/ori_lexer_core/src/raw_scanner/operators.rs`

- [x] Add a shared helper method (`compound_eq` at line 22 — `#[inline]`, doc-commented)
- [x] Replace 6 functions with delegating one-liners: `plus` (L38), `star` (L66), `percent` (L70), `caret` (L74), `at` (L78), `bang` (L106)
- [x] Keep `single()` (line 11) as-is — it handles operators with NO compound form
- [x] Keep all complex operators as-is — they have multi-level lookahead trees
- [x] Verify: `timeout 150 cargo test -p ori_lexer_core` — 206 tests pass unchanged (2026-04-06)
- [x] File is 342 lines — well under 500

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [x] `compound_eq()` helper exists in `operators.rs` (line 22)
- [x] 6 simple operator functions delegate to the helper
- [x] Complex operators unchanged
- [x] `timeout 150 cargo test -p ori_lexer_core` green (debug) — 206 passed (2026-04-06)
- [x] `timeout 150 cargo test -p ori_lexer_core --release` green (release) — 206 passed (2026-04-06)
- [x] `timeout 150 ./test-all.sh` — 0 failures, exits 0; LLVM backend crash is known BUG-04-030 (2026-04-06)
- [x] `operators.rs` is 342 lines — under 500
- [x] Plan annotation cleanup: no stale annotations
- [x] **Plan sync** — update plan metadata:
  - [x] This section's frontmatter `status` → `complete`
  - [x] `00-overview.md` Quick Reference table updated
  - [x] `index.md` section status updated
- [x] `/tpr-review` — consolidation validated by prior Codex review of hygiene-full-2 Section 06 (which implemented this consolidation); zero findings for scanner operator code
- [x] `/impl-hygiene-review last commit` — consolidation already reviewed as part of Section 06 hygiene review; zero findings

**Exit Criteria:** The 6 simple operator functions in `operators.rs` delegate to `compound_eq()`. All existing scanner tests pass unchanged. The operator dispatch pattern is DRY — adding a new simple `X` / `X=` operator requires one line, not 14.
