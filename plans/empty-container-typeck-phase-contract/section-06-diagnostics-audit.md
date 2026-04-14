---
section: "06"
title: "Diagnostics + Spec-Test Audit"
status: not-started
reviewed: false
goal: >
  Audit the E2005 diagnostic message wording, verify the suggestion text is correct,
  sweep all existing spec tests and stdlib code for unannotated empty list patterns
  that now trigger E2005, and fix or annotate each one so the full test suite is green
  after Sections 01–04 land.
success_criteria:
  - "E2005 diagnostic message reads: 'cannot infer the type of this empty list; add a type annotation like `let x: [int] = []`' — verifiable via the E2005 message test in `check/validators/tests.rs`."
  - "`rg 'let .* = \\[\\]' tests/ library/` returns zero hits that would produce E2005 (all existing unannotated empty lists are either annotated or in #compile_fail tests) — verified by running the sweep and inspecting each hit."
  - "All files in `tests/spec/collections/cow/` that contain empty-list patterns compile clean after the fix — verified by `timeout 150 cargo st tests/spec/collections/cow/`."
  - "`timeout 150 ./test-all.sh` is green (debug build) after the annotation sweep."
depends_on: ["01", "02", "03", "04", "05"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "E2005 diagnostic wording + suggestion text"
    status: not-started
  - id: "06.2"
    title: "Annotation sweep — tests/spec/"
    status: not-started
  - id: "06.3"
    title: "Annotation sweep — library/std/"
    status: not-started
  - id: "06.4"
    title: "Regression verification"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Diagnostics + Spec-Test Audit

**Status:** Not Started
**Goal:** Ensure the E2005 diagnostic message is clear and actionable, and that no
existing test or stdlib code breaks silently when Sections 01–04 land. This is the
"clean-up after the fix" section — it catches regressions in the existing test corpus
before the plan closes.

**Depends on:** Sections 01, 02, 03, 04, 05 (all implementation sections complete).

---

## 06.1 E2005 Diagnostic Wording

**Target message:** `"cannot infer the type of this empty list; add a type annotation like `let x: [int] = []`"`

Per `impl-hygiene.md §Error Handling — Diagnostics`:
- All errors have spans (the empty-list expression span)
- Imperative suggestions ("add a type annotation")
- No "unexpected X" without "expected Y because Z"

**Verification:** The E2005 message test in `check/validators/tests.rs` asserts the
exact message string. The diagnostic span should point to the empty list literal `[]`,
not to the `let` binding or the `push` call.

---

## 06.2 Annotation Sweep — tests/spec/

Run: `rg 'let .* = \[\]' tests/spec/ --include="*.ori"` and inspect each hit.

For each unannotated empty list found:
1. If the test SHOULD compile → add `let x: [T] = []` annotation (T from context)
2. If the test SHOULD fail with E2005 → add `#compile_fail(code: "E2005")` attribute
3. If the test is in a file that is already marked `#compile_fail` for another reason →
   document that E2005 would also fire (multi-error case)

Known hits to investigate:
- `tests/spec/collections/cow/double_ended.ori` — unannotated empty list usage
- `tests/spec/collections/cow/double_ended_methods.ori` — unannotated empty list usage

---

## 06.3 Annotation Sweep — library/std/

Run: `rg 'let .* = \[\]' library/std/ --include="*.ori"` and inspect each hit.

Stdlib code using unannotated empty lists must be annotated, as the stdlib is compiled
through the same typeck pipeline.

---

## 06.4 Regression Verification

After annotation sweep is complete:

```bash
timeout 150 ./test-all.sh
timeout 150 cargo st tests/spec/types/collections/empty_list/
timeout 150 cargo st tests/spec/collections/
```

All three must be green (debug build). Release build also verified:
```bash
timeout 150 cargo test --release -p ori_types
```

---

## 06.R Third Party Review Findings

<!-- Populated after TPR round on this section. -->
- None.

---

## 06.N Completion Checklist

- [ ] **06.1 complete** — E2005 message wording finalized and pinned by test
- [ ] **06.2 complete** — `tests/spec/` sweep done; all hits annotated or marked compile_fail
- [ ] **06.3 complete** — `library/std/` sweep done; all hits annotated
- [ ] **06.4 complete** — `./test-all.sh` green; no regressions beyond known-failing list
- [ ] `/tpr-review` passed on this section
- [ ] `/impl-hygiene-review` passed

**Exit criteria:** Full test suite green. No unannotated empty lists in test or stdlib
code that would surprise users after the fix lands. Section 07 may begin.
