# Fix Section Template

This template is used by `/fix-bug` to create `plans/bug-tracker/fix-BUG-XX-NNN.md` files. Each fix section provides plan-section rigor for a single bug or cluster of related bugs.

---

```markdown
---
bug: "BUG-{section}-{ordinal}"
title: "{Bug title from the bug entry}"
severity: "{critical|high|medium|low}"
status: not-started
goal: "{One-line measurable goal — not 'fix X' but 'X correctly produces Y under conditions Z'}"
success_criteria:
  - "{Criterion 1 — specific, testable}"
  - "{Criterion 2 — with verification command}"
subsystem: "{crate/file path}"
found: "{YYYY-MM-DD}"
source: "{tpr-review|code-journey|manual|continue-roadmap|review-work}"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-{section}-{ordinal} — {Title}

**Status:** Not Started
**Severity:** {severity}
**Goal:** {Expanded goal — what must be true when this fix is complete.}

**Success Criteria:**
- [ ] {Criterion — specific behavioral outcome with verification method}
- [ ] {Criterion — test name, command, or observable result}

**Context:** {Why this bug exists. What pain point it causes. How it was discovered.
Cite the original bug entry. 2-4 sentences.}

---

## 1. Root Cause Analysis

- **Symptom**: {What was observed — error message, wrong output, crash}
- **Proximate cause**: {What code produced the wrong behavior}
- **Root cause**: {Why that code does the wrong thing — the architectural/logical flaw}
- **Blast radius**: {What else is affected — other tests, features, code paths}
- **Affected files**:
  - `{file1}` — {what needs to change and why}
  - `{file2}` — {what needs to change and why}

**Reference implementations** (if applicable):
- **{Language}** `{file path}`: {How they handle this case}

---

## 2. TDD — Test Matrix

Write ALL tests BEFORE the fix. Verify they fail against current code.

### Exact failing case
- [ ] {The specific input that triggered the bug — from the repro}

### Edge cases
- [ ] {Empty, single-element, boundary conditions relevant to this bug}
- [ ] {Additional edge cases}

### Cross-type coverage (if type-dependent)
- [ ] {Test with str}
- [ ] {Test with [int]}
- [ ] {Test with Option<T>}
- [ ] {Test with structs, closures, maps, sets as applicable}

### Cross-pattern coverage (if pattern-dependent)
- [ ] {Test each relevant control-flow pattern}

### Cross-feature interactions
- [ ] {Test interaction with closures, generics, ?, pattern matching, traits as applicable}

### Semantic pin
- [ ] {Test that ONLY passes with the correct/new semantics — the permanent regression guard}

### Negative pin
- [ ] {Test that REJECTS the old/broken behavior — proves the compiler actively prevents regression}

### Verify tests fail before fix
- [ ] All new tests fail against current code (confirming they test the right thing)

---

## 3. Implementation

- [ ] {Describe the fix approach — what changes, where, why}
  ```rust
  // Code sketch showing the target fix (types, signatures, key logic)
  ```
- [ ] {Additional implementation steps if multi-file}
- [ ] {Any co-implementation requirements with other subsystems}

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix (no test modifications needed)
- [ ] Matrix completeness verified — every cell in type x pattern x feature grid has a test
- [ ] Debug AND release builds pass (`cargo b && cargo b --release`)
- [ ] Interpreter and LLVM produce identical results for all new tests (dual-execution parity)
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on affected test programs (for memory-touching fixes)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p {affected_crate}` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Bug entry in `plans/bug-tracker/section-{NN}-*.md` updated: `- [x]` with resolution details
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] Bug-tracker `00-overview.md` Quick Reference open bug count updated
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (critical/high severity: MANDATORY; medium: expected; low: recommended but not required)
- [ ] `/impl-hygiene-review last commit` passed — MUST run AFTER `/tpr-review` is clean (critical/high: MANDATORY; medium: recommended; low: optional)

**Exit Criteria:** {Paragraph describing the measurable, testable condition
that proves this fix is complete. Include specific test names, commands,
and what output they produce. Not "X works" but "X produces Y output
when Z command is run, with 0 regressions across the full test suite."}
```
