---
section: "06"
title: "Expand Fixtures + Self-Test"
status: not-started
reviewed: false
goal: "Add 7+ new diagnostic fixtures covering the code patterns that cause the most AOT/AIMS debugging churn, and update self-test.sh to exercise them"
success_criteria:
  - "At least 7 new fixture files in diagnostics/fixtures/ covering closures, iterators, nested structures, generics, trait dispatch, and failure modes"
  - "Each fixture compiles and runs successfully under AOT (except intentional failure-mode fixtures)"
  - "self-test.sh runs all new fixtures through the appropriate diagnostic scripts"
  - "All new fixtures are documented in README.md"
inspired_by:
  - "Swift SIL test fixtures — targeted programs exercising specific SIL optimizer paths"
  - "Koka FBIP test corpus — programs that stress the PARC optimization pipeline"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Create complex-pattern fixtures"
    status: not-started
  - id: "06.2"
    title: "Update self-test.sh coverage"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Expand Fixtures + Self-Test

**Status:** Not Started
**Goal:** The diagnostic toolkit's self-test suite runs against only 3 basic fixtures (`simple.ori`, `clean.ori`, `chain.ori`). These don't exercise closures, iterators, nested structures, generics, trait dispatch, or failure modes — the exact code patterns that cause the most debugging churn. New fixtures ensure diagnostic scripts produce correct output for the patterns they'll actually be used to debug.

**Success Criteria:**
- [ ] At least 7 new `.ori` fixture files in `diagnostics/fixtures/`
- [ ] Each fixture exercises a distinct code pattern relevant to AOT/AIMS debugging
- [ ] `self-test.sh` runs new fixtures through `diagnose-aot.sh`, `dual-exec-debug.sh`, `rc-stats.sh`, `ir-dump.sh`, `arc-dump.sh`
- [ ] Satisfies mission criterion: "7+ new diagnostic fixtures covering closures, iterators, nested structures, generics, trait dispatch, and failure modes"

**Context:** The current 3 fixtures (`simple.ori` — no collections/RC; `clean.ori` — collections, balanced RC; `chain.ori` — chained COW) were adequate when the toolkit was first built. But ARC/AIMS bugs predominantly appear in closure captures, iterator early-exit cleanup, nested aggregate drops, generic instantiation, and trait method dispatch — none of which are exercised. A diagnostic regression in these areas ships behind a green self-test.

**Depends on:** None.

---

## 06.1 Create complex-pattern fixtures

**File(s):** `diagnostics/fixtures/*.ori` (new files)

Each fixture must: (1) compile under AOT, (2) produce deterministic output via `print()`, (3) exercise a specific code pattern. Fixture names are descriptive of the pattern, not the section number. Check existing sibling fixtures for import patterns — `assert_eq` requires `use std.testing { assert_eq }`.

- [ ] **`closure.ori`** — closure capturing a collection, calling the closure, verifying captures are alive after the call. Tests closure RC: the captured value must be inc'd on capture, dec'd on closure drop. **Verify Ori syntax against existing test files before committing** — check `tests/spec/closures/` for correct patterns.
- [ ] **`iterator_break.ori`** — iterate with early `break`, verifying the iterator and remaining elements are properly dropped. This is the #1 ARC debugging pain point. **Verify**: check `tests/spec/expressions/loops/` for correct `for...do` + `break` syntax.
- [ ] **`nested_list.ori`** — nested `[[int]]` or `[[str]]` collection, exercising elem_dec_fn propagation for nested drops.
- [ ] **`generic.ori`** — generic function with a concrete instantiation, testing that generics don't break RC analysis. **Verify**: check `tests/spec/generics/` for correct generic function syntax.
- [ ] **`trait_dispatch.ori`** — trait method call through a concrete impl, testing that trait dispatch codegen produces balanced RC. **Note**: current compiler uses `impl Trait for Type` syntax (not `impl Type: Trait` — that's approved but not yet implemented per CLAUDE.md §Capability Unification). Check `tests/spec/traits/` for correct syntax.
- [ ] **`pattern_match.ori`** — sum type with pattern matching, exercising tag dispatch and per-variant drops. Check `tests/spec/types/enums/` for correct sum type syntax.
- [ ] **`map_iteration.ori`** — map creation + iteration, testing map RC and iterator cleanup. Check `tests/spec/collections/map/` for correct map syntax.
- [ ] Verify each fixture: `ori run <fixture>` produces expected output, `ori build <fixture> -o /tmp/test_fixture && /tmp/test_fixture` produces the same output
- [ ] Add any additional fixtures that emerge as needed during self-test integration

- [ ] **Subsection close-out (06.1)** — MANDATORY before starting 06.2:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.2 Update self-test.sh coverage

**File(s):** `diagnostics/self-test.sh`, `diagnostics/README.md`

- [ ] Add each **pass fixture** (closure, iterator_break, nested_list, generic, trait_dispatch, pattern_match, map_iteration) to the self-test matrix:
  - `ir-dump.sh --no-color <fixture>` produces non-empty IR
  - `arc-dump.sh --no-color <fixture>` produces non-empty ARC IR
  - `diagnose-aot.sh --no-color <fixture>` passes all checks
  - `dual-exec-debug.sh --no-color <fixture>` shows MATCH
  - `rc-stats.sh --no-color <fixture>` produces output
- [ ] Add each **failure fixture** (leak.ori, double_free.ori — if created) to the self-test with EXPECTED-FAIL expectations:
  - Use `run_test_expect_fail` (already exists in self-test.sh lines 91-105) for `diagnose-aot.sh` on these fixtures
  - Verify these fixtures FAIL the leak check or Valgrind, which is the expected behavior
  - Do NOT apply the pass-only matrix to failure fixtures — they exist to validate that diagnostic scripts correctly DETECT failures
- [ ] Update the fixture existence check at the top of self-test.sh (currently checks for simple/clean/chain only)
- [ ] Update `diagnostics/README.md` fixtures table to include all new fixtures with what they test
- [ ] Verify: `diagnostics/self-test.sh --verbose` passes with expanded coverage

- [ ] **Subsection close-out (06.2)** — MANDATORY before starting 06.R:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] All subsections (06.1, 06.2) complete
- [ ] All fixtures compile and run under both interpreter and AOT
- [ ] `diagnostics/self-test.sh` passes
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **`/improve-tooling` section-close sweep**
