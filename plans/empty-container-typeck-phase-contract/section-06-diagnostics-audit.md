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
  - "`rg '\\[\\s*\\]' tests/spec/ tests/valgrind/ library/ --glob '*.ori'` returns zero unannotated empty-list hits that would produce E2005 — all existing empty-list positions are either type-annotated, already in `#compile_fail` tests, or in positions where type context is supplied top-down (e.g. pattern-match arms, scrutinee-constrained contexts). Verified by running the comprehensive sweep and inspecting each hit."
  - "All files in `tests/spec/traits/iterator/` and `tests/spec/collections/cow/` that contain empty-list patterns compile clean after the fix — verified by `timeout 150 cargo st tests/spec/traits/iterator/` and `timeout 150 cargo st tests/spec/collections/cow/`."
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

Run a comprehensive sweep covering all syntactic positions where an empty list literal
can appear without a type annotation:

```bash
# Full empty-list sweep — covers let-bindings, argument position, operator/concat forms,
# for..in [] forms, return-from-block, nested literals, and receiver-chains.
# Includes tests/valgrind/ to catch operator-position usages (e.g. cow_list_concat.ori).
# Inspect every hit manually to determine whether type context is present.
rg '\[\s*\]' tests/spec/ tests/valgrind/ library/ --glob '*.ori'
```

The two narrower patterns from earlier rounds are subsumed by this single command and
are shown here for reference:

```bash
# Narrower forms (subsumed — kept for documentation)
rg 'let.*=\s*\[\s*\]' tests/spec/ tests/valgrind/ library/ --glob '*.ori'  # let-binding forms
rg '\[\]\.' tests/spec/ tests/valgrind/ --glob '*.ori'                       # receiver-chain forms
```

Do NOT use the narrower greps as the sole audit sweep — they miss:
- Argument position: `foo(items: [])`
- Operator/concat position: `[] + [1, 2, 3]`
- `for...in` position: `for x in [] yield x`
- Pattern context: `match e { [] -> ... }` (pattern type is constrained by the scrutinee top-down; no E2005 needed)
- Return-from-block: `{ [] }`
- Nested literals: `[[]]`, `Some([])`

For each unannotated empty list found:
1. If the test SHOULD compile → add `let x: [T] = []` annotation (T from context);
   for argument positions, either annotate at the call site or annotate the binding
   before the call
2. If the test SHOULD fail with E2005 → add `#compile_fail(code: "E2005")` attribute
3. If the test is in a file that is already marked `#compile_fail` for another reason →
   document that E2005 would also fire (multi-error case)
4. Pattern-position hits (`[] -> ...` arms) — these do NOT produce E2005 because the
   pattern type is constrained top-down by the scrutinee type; document and skip.
   (Note: empty-list patterns are refutable — `[] -> ...` fails to match a non-empty
   list — but they are exempt from E2005 because type is resolved from context, not
   inferred from the empty literal.)

Known hits to investigate (verified via repo sweep):
- `tests/spec/traits/iterator/double_ended.ori` — unannotated `let result = []` usage
  (lines ~25, ~66) and expression-position `[].iter()` (line ~167)
- `tests/spec/traits/iterator/double_ended_methods.ori` — expression-position `[].iter()`
  calls (lines ~35, ~90, ~144, ~185)
- `tests/spec/collections/cow/matrix_map_set.ori` — `[].iter().collect()` forms (line ~94)
- `tests/spec/traits/into/set_to_list.ori` — `[].iter().collect()` (line ~24)
- `tests/spec/lexical/delimiters.ori` — `[].len()` expression position (line ~151)
- `tests/spec/lexical/keywords.ori` — `[].len()` expression position (line ~222)
- `tests/spec/extensions/list_methods.ori` — `get_count(items: [])` and
  `is_empty(items: [])` argument position
- `tests/spec/traits/iterator/for_loop.ori` — `for x in [] yield x * 2` and
  `for x in [] do` forms
- `tests/valgrind/cow/cow_list_concat.ori` — `[] + [1, 2, 3]` operator position

For expression-position hits (bare `[].method()` with no type context), determine whether
the call chain provides enough type context to infer the element type. If not, the fix
is to assign to an annotated binding: `let result: [T] = [].method()...`.

---

## 06.3 Annotation Sweep — library/std/

Run: `rg '\[\s*\]' library/std/ --glob '*.ori'` and inspect each hit.

Stdlib code using unannotated empty lists must be annotated, as the stdlib is compiled
through the same typeck pipeline. The broader `\[\s*\]` pattern covers argument position,
operator position, and `for...in []` in addition to let-binding forms.

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

Round 2 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed
in this revision.

### [[TPR-06-001-codex]] [HIGH] Broaden the annotation sweep to all empty-list call patterns

**Location:** `plans/empty-container-typeck-phase-contract/section-06-diagnostics-audit.md:70`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 06 originally instructed only `rg 'let .* = \[\]' tests/spec/` and
listed two known hits under `tests/spec/collections/cow/double_ended*.ori` — but those
files do not exist. A broader repo scan found current empty-list sites that this grep
misses, including direct-receiver forms in `tests/spec/lexical/delimiters.ori:151`,
`tests/spec/lexical/keywords.ori:222`, `tests/spec/traits/iterator/double_ended.ori:167`,
`tests/spec/traits/iterator/double_ended_methods.ori:35`, and
`tests/spec/collections/cow/matrix_map_set.ori:94`.

**Fix:** Replaced the narrow `let x = []` grep with a two-command sweep: one for
`let.*=\s*\[\s*\]` (let-binding forms, tolerates whitespace variation) and one for
`\[\]\.` (expression-position bare `[]` receiver chains). Updated the known-hit list to
real file paths under `tests/spec/traits/iterator/` and other locations discovered by
the sweep.

---

### [[TPR-06-002-gemini]] [LOW] Improve unannotated empty list sweep regex to handle whitespace

**Location:** `plans/empty-container-typeck-phase-contract/section-06-diagnostics-audit.md:70`
**Reviewer:** Gemini | **Status:** Fixed

**Evidence:** The original regex `rg 'let .* = \[\]'` strictly requires exactly one space
around `=` and no spaces inside `[]`. Valid Ori code like `let x=[]` or `let y = [ ]`
would be silently missed.

**Fix:** Changed to `rg 'let.*=\s*\[\s*\]'` (allows any whitespace around `=` and inside
`[]`). Also updated the success_criteria regex in the frontmatter to match.

Round 3 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed
in this revision.

### [[TPR-06-R3-001-codex+gemini]] [HIGH] Broaden sweep beyond let-bindings and receiver chains

**Location:** `plans/empty-container-typeck-phase-contract/section-06-diagnostics-audit.md:§06.2`
**Reviewers:** Codex + Gemini | **Status:** Fixed

**Evidence:** The two-command sweep (`let.*=\s*\[\s*\]` + `\[\]\.`) was added in Round 2 to
catch let-binding forms and receiver chains. Both reviewers independently verified via repo
grep that this still misses empty-list usage in argument position (`foo(items: [])`), operator/
concatenation position (`[] + [1, 2, 3]`), `for...in []` position, and return-from-block. Codex
additionally found: `tests/spec/extensions/list_methods.ori` (argument position),
`tests/spec/traits/iterator/for_loop.ori` (`for...in []`), `tests/valgrind/cow/cow_list_concat.ori`
(operator position). None of these are matched by the prior two-command sweep.

**Fix:** Replaced the two-command sweep with a single comprehensive `rg '\[\s*\]' tests/spec/ library/ --glob '*.ori'` that covers all syntactic positions. Added documentation explaining which positions the narrower patterns missed and guidance on which hits (pattern-match arms) are exempt from E2005. Updated the success_criteria in the frontmatter to use the broader pattern. Updated 06.3 to use the same broader pattern.

---

Round 4 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed
in this revision.

### [[TPR-06-R4-001-codex]] [MEDIUM] Comprehensive sweep misses tests/valgrind/ operator-position hit

**Location:** `plans/empty-container-typeck-phase-contract/section-06-diagnostics-audit.md:77`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** The known-hit list already cited `tests/valgrind/cow/cow_list_concat.ori`
(operator-position `[] + [1, 2, 3]`), but the comprehensive sweep command
`rg '\[\s*\]' tests/spec/ library/ --glob '*.ori'` did not include `tests/valgrind/` as a
search root. Fresh verification confirmed this is the only `[] +` hit in the repo and it
sits outside the advertised sweep scope.

**Fix:** Added `tests/valgrind/` to the comprehensive sweep command:
`rg '\[\s*\]' tests/spec/ tests/valgrind/ library/ --glob '*.ori'`. Updated the
success_criteria to use the three-root form. Updated the narrower reference patterns to
also include `tests/valgrind/` for consistency.

---

### [[TPR-06-R4-002-gemini]] [LOW] Empty list pattern-match arms described as "irrefutable" — incorrect

**Location:** `plans/empty-container-typeck-phase-contract/section-06-diagnostics-audit.md:93,104`
**Reviewer:** Gemini | **Status:** Fixed

**Evidence:** Lines 93 and 104 described `[] -> ...` arms as "irrefutable pattern matching".
This is incorrect: an empty-list pattern is refutable — it fails to match a non-empty list.
The reason E2005 does not fire is that the pattern type is constrained top-down by the
scrutinee type, not because the pattern is irrefutable.

**Fix:** Changed line 93 to "pattern type is constrained by the scrutinee top-down; no
E2005 needed." Expanded point 4 to clarify the refutability distinction and the correct
reason for E2005 exemption.

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
