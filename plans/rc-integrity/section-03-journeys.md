---
section: "03"
title: "Code Journeys — Expanded Coverage"
status: not-started
goal: "Add journeys covering heap-typed loop reassignment, nested RC structures, and COW patterns — all scoring 10/10"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "J14 — String Builder (Loop Heap Reassignment)"
    status: not-started
  - id: "03.2"
    title: "J15 — RC Lifecycle (Nested Heap Structures)"
    status: not-started
  - id: "03.3"
    title: "J16 — COW Patterns (Shared vs Unique Mutation)"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Code Journeys — Expanded Coverage

**Status:** Not Started
**Goal:** Add 3 new code journeys (J14-J16) covering the patterns that were missing from the original 13 — specifically heap-typed loop reassignment, nested RC structures, and COW mutation patterns. All must score 10/10 and be leak-free.

**Context:** None of the original 13 journeys exercise heap-typed variable reassignment inside a loop. J7 (loops) uses `int` accumulators; J9 (strings) uses linear concat without loops; J10 (lists) iterates without reassignment. This gap let the FatValue PrimOp crash slip through to near-merge. These new journeys close that gap.

**Depends on:** Section 01 (leak detection infrastructure) and Section 02 (leak fixes must be complete so journeys don't crash or leak).

---

## 03.1 J14 — String Builder (Loop Heap Reassignment)

**File(s):** `plans/code-journeys/14-string-builder.ori`, `plans/code-journeys/14-string-builder-results.md`

Journey theme: "I am a string builder." Exercises the exact pattern that caused the crash — string concatenation in a loop with heap promotion from SSO.

- [ ] Create `14-string-builder.ori` with:
  - `@build_repeated(n: int, c: str) -> str` — loop appending `c` to a string `n` times
  - `@build_sequence(n: int) -> str` — loop appending `str(i)` for each `i` in `0..n`
  - `@build_with_separator(items: [str], sep: str) -> str` — loop joining strings with separator
  - `@main` — combines results, returns checkable integer
- [ ] Verify interpreter produces correct result
- [ ] Verify AOT produces correct result
- [ ] Run `diagnostics/dual-exec-verify.sh` on J14 — interpreter and AOT produce identical output
- [ ] Run `ORI_CHECK_LEAKS=1` — zero leaks
- [ ] Run valgrind — zero errors, zero bytes at exit
- [ ] Run `.claude/skills/code-journey/extract-metrics.py` — score all 7 categories
- [ ] Create `14-string-builder-results.md` with full scoring breakdown
- [ ] Target: 10/10 overall (if below 10/10, fix the underlying issue before proceeding to Section 04)

---

## 03.2 J15 — RC Lifecycle (Nested Heap Structures)

**File(s):** `plans/code-journeys/15-rc-lifecycle.ori`, `plans/code-journeys/15-rc-lifecycle-results.md`

Journey theme: "I am a lifecycle." Exercises structs containing heap fields, passing them to functions, extracting fields, and letting them go out of scope.

- [ ] Create `15-rc-lifecycle.ori` with:
  - `type Container = { items: [int], name: str }` — struct with two heap fields
  - `@make_container(n: int) -> Container` — construct with heap fields
  - `@extract_and_use(c: Container) -> int` — project fields, use, let go
  - `@pass_through(c: Container) -> Container` — identity (tests ownership transfer)
  - `@nested_containers() -> int` — struct containing another struct with RC fields (exercises recursive aggregate drop)
  - `@main` — exercises all patterns, returns checkable integer
- [ ] Verify interpreter produces correct result
- [ ] Verify AOT produces correct result
- [ ] Run `diagnostics/dual-exec-verify.sh` on J15 — behavioral equivalence
- [ ] Run `ORI_CHECK_LEAKS=1` — zero leaks
- [ ] Run valgrind — zero errors
- [ ] Run `.claude/skills/code-journey/extract-metrics.py` — score all 7 categories
- [ ] Create `15-rc-lifecycle-results.md` with full scoring breakdown

---

## 03.3 J16 — COW Patterns (Shared vs Unique Mutation)

**File(s):** `plans/code-journeys/16-cow-patterns.ori`, `plans/code-journeys/16-cow-patterns-results.md`

Journey theme: "I am copy-on-write." Exercises the COW runtime — unique owner mutations (in-place), shared owner mutations (copy), and the transition between them.

- [ ] Create `16-cow-patterns.ori` with:
  - `@unique_append() -> int` — unique string concat (should mutate in place)
  - `@shared_fork() -> int` — share a string, mutate one copy, verify original unchanged
  - `@list_cow_loop() -> int` — list push in a loop with COW fast path
  - `@slice_cow() -> int` — create slice from list, verify original unaffected, drop both cleanly
  - `@main` — combines results
- [ ] Verify interpreter produces correct result
- [ ] Verify AOT produces correct result
- [ ] Run `diagnostics/dual-exec-verify.sh` on J16 — behavioral equivalence
- [ ] Run `ORI_CHECK_LEAKS=1` — zero leaks
- [ ] Run valgrind — zero errors
- [ ] Run `.claude/skills/code-journey/extract-metrics.py` — score all 7 categories
- [ ] Create `16-cow-patterns-results.md` with full scoring breakdown

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] J14, J15, J16 source files created and committed
- [ ] All 3 new journeys produce correct output on both eval and AOT
- [ ] All 3 new journeys pass `diagnostics/dual-exec-verify.sh` (behavioral equivalence between interpreter and AOT)
- [ ] All 3 new journeys have zero leaks (`ORI_CHECK_LEAKS=1`)
- [ ] All 3 new journeys have zero valgrind errors
- [ ] All 3 new journeys score 10/10 (if below 10/10, the blocking issue must be fixed before Section 05 verification)
- [ ] Results files created in `plans/code-journeys/`
- [ ] `plans/code-journeys/overview.md` updated with J14-J16
- [ ] Original 13 journeys still score 10/10 (no regression)

**Exit Criteria:** 16 total code journeys (13 original + 3 new), all scoring 10/10, all leak-free, all valgrind-clean. The new journeys specifically cover heap loop reassignment, nested RC structures, and COW patterns — the three gap areas.
