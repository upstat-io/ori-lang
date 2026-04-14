---
section: "04"
title: "Diagnostic Ordering & Suppression"
status: not-started
reviewed: false
goal: "Upgrade diagnostic dedup to (error_code, span), add child-span TyError suppression, and ensure deterministic cross-file ordering so diagnostic snapshots are stable"
success_criteria:
  - "DiagnosticQueue deduplicates by (error_code, primary_span) instead of (line, message_prefix_hash)"
  - "Child-span TyError suppression prevents cascading errors from a single root cause"
  - "Diagnostics are emitted in deterministic source-position order across all files"
  - "diagnostic.md contains the ordering and suppression rules"
  - "All existing diagnostic snapshot tests pass (or are blessed with improved output)"
inspired_by:
  - "Rust rustc_errors dedup by (error_code, primary_span) + TyError follow-on suppression"
  - "Gleam test-helpers-rs/src/lib.rs:72-115 (sort warnings before rendering, normalize for stability)"
  - "TypeScript testRunner/unittests/helpers/baseline.ts:76-115 (stable diagnostic refresh ordering)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Upgrade Dedup to (ErrorCode, Span)"
    status: not-started
  - id: "04.2"
    title: "Child-Span TyError Suppression"
    status: not-started
  - id: "04.3"
    title: "Deterministic Ordering Rule"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Diagnostic Ordering & Suppression

**Status:** Not Started
**Goal:** Make diagnostic output deterministic and suppress cascading errors. Currently `DiagnosticQueue` in `ori_diagnostic/src/queue/mod.rs` (394 lines) deduplicates by `(line, message_prefix_hash)` for non-syntax errors and `(line)` for syntax errors. It has follow-on filtering for "invalid operand"/"invalid type"/`<error>` strings and error-limit capping. Missing: (error_code, span) dedup, child-span TyError suppression, and cross-file deterministic ordering.

**Success Criteria:**
- [ ] Dedup uses (error_code, primary_span) — satisfies mission criterion "deterministic diagnostics"
- [ ] Child-span suppression eliminates 50%+ of cascading errors in error-heavy test files
- [ ] `diagnostic.md` contains the ordering rule — satisfies mission criterion "rules documented"
- [ ] All diagnostic snapshot tests pass or are blessed with objectively better output

**Context:** Research found that `DiagnosticQueue` already sorts labels within a single diagnostic by span (`sort_by_key(|l| l.span.start)` at `emitter/terminal/mod.rs:361`). The top-level diagnostic emission sorts by (line, column). But dedup uses line+message_prefix_hash instead of (error_code, span) — this means structurally identical errors at different columns on the same line can be deduplicated when they shouldn't be (false positive), and identical errors across different scopes may not be (false negative). The child-span suppression referenced in `typeck.md §ER-4` and `impl-hygiene.md §Aspirational Patterns` is documented as a goal but not fully implemented.

**Reference implementations:**
- **Rust** `rustc_errors`: dedup by `(error_code, primary_span)`, `DelayedBug` for follow-on suppression
- **Gleam** `test-helpers-rs/src/lib.rs:72-115`: sort warnings before rendering for snapshot stability

**Depends on:** Section 01.

---

## 04.1 Upgrade Dedup to (ErrorCode, Span)

**File(s):** `compiler/ori_diagnostic/src/queue/mod.rs`

- [ ] Write failing test: two diagnostics with same error code and span but different messages — should be deduplicated. Two diagnostics with same line but different error codes — should NOT be deduplicated.

- [ ] Modify `DiagnosticQueue` to use `(ErrorCode, Span)` as the dedup key instead of `(line, message_prefix_hash)`:
  - `ErrorCode` is the numeric code (E2001, E2003, etc.)
  - `Span` is the primary label's span (start + end positions)
  - Keep the existing error-limit cap and soft-error suppression logic
  - The existing `(line)` dedup for syntax errors should use `(E1xxx, span)` instead

- [ ] Verify all existing diagnostic tests pass — bless any that produce genuinely better output

- [ ] **Subsection close-out (04.1)** — MANDATORY before starting 04.2:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 04.2 Child-Span TyError Suppression

**File(s):** `compiler/ori_diagnostic/src/queue/mod.rs`, `compiler/ori_types/src/type_error/`

Implement Rust-style child-span suppression: when an error at span S produces TyError, suppress subsequent errors at spans contained within S that mention TyError. Users see the root cause, not the cascade.

- [ ] Write failing test: program with one type error that currently produces 3+ cascading errors — after fix, should produce 1

- [ ] Add span-containment check to `DiagnosticQueue`:
  - Track a `suppressed_spans: Vec<Span>` — spans where TyError was the root cause
  - Before adding a new diagnostic, check if its primary span is contained within any suppressed span AND the diagnostic's message references `<error>` or TyError
  - If yes: drop the diagnostic (don't add to queue)
  - `typeck.md §ER-4` already specifies this behavior — this implements it

- [ ] Count suppressed diagnostics in test output (for verification — "suppressed N cascading errors")

- [ ] Verify error-heavy test files produce fewer, more focused diagnostics

- [ ] **Subsection close-out (04.2)** — MANDATORY before starting 04.3:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 04.3 Deterministic Ordering Rule

**File(s):** `.claude/rules/diagnostic.md`, `compiler/ori_diagnostic/src/queue/mod.rs`

- [ ] Add rule to `diagnostic.md` requiring:
  - Diagnostics SHALL be emitted in a canonical order: primary span start position (ascending), then error code (ascending) for same-position ties
  - Labels and notes within a diagnostic SHALL be ordered by span position (already true — `sort_by_key(|l| l.span.start)` at terminal/mod.rs:361)
  - Cross-file diagnostics SHALL be ordered by file path (lexicographic), then by span within file
  - Snapshot tests SHALL compare normalized, deterministic diagnostic output

- [ ] Verify `DiagnosticQueue::flush()` already sorts by (line, column) — if yes, upgrade to (file, line, column, error_code) for full determinism

- [ ] Add a semantic pin test: compile a multi-error program and assert the exact error order in the output

- [ ] **Subsection close-out (04.3)** — MANDATORY before completing section:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All subsections (04.1-04.3) complete
- [ ] Dedup upgraded to (error_code, span)
- [ ] Child-span suppression implemented and tested
- [ ] Deterministic ordering verified with semantic pin test
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] Debug AND release builds pass
- [ ] **`/tpr-review`** — independent dual-source review clean
- [ ] **`/impl-hygiene-review`** — implementation hygiene clean
- [ ] **`/improve-tooling`** — section-close sweep
- [ ] **`/sync-claude`** — section-close doc sync
