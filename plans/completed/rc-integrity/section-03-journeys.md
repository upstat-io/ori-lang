---
section: "03"
title: "Code Journeys — Expanded Coverage"
status: complete
goal: "Add journeys covering heap-typed loop reassignment, nested RC structures, and COW patterns — all scoring 10/10"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "J18 — String Builder (Loop Heap Reassignment)"
    status: complete
  - id: "03.2"
    title: "J19 — RC Lifecycle (Nested Heap Structures)"
    status: complete
  - id: "03.3"
    title: "J20 — COW Patterns (Shared vs Unique Mutation)"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: complete
---

# Section 03: Code Journeys — Expanded Coverage

**Status:** Not Started
**Goal:** Add 3 new code journeys (J14-J16) covering the patterns that were missing from the original 13 — specifically heap-typed loop reassignment, nested RC structures, and COW mutation patterns. All must score 10/10 and be leak-free.

**Context:** None of the original 13 journeys exercise heap-typed variable reassignment inside a loop. J7 (loops) uses `int` accumulators; J9 (strings) uses linear concat without loops; J10 (lists) iterates without reassignment. This gap let the FatValue PrimOp crash slip through to near-merge. J14-J17 (fat pointer journeys, added 2026-03-16) test fat pointer codegen basics but NOT these specific gap patterns. These new journeys (J18-J20) close the gap.

**Depends on:** Section 01 (leak detection infrastructure) and Section 02 (leak fixes must be complete so journeys don't crash or leak).

---

## 03.1 J18 — String Builder (Loop Heap Reassignment)

**File(s):** `plans/code-journeys/18-string-builder.ori`, `plans/code-journeys/18-string-builder-results.md`

Journey theme: "I am a string builder." Exercises the exact pattern that caused the crash — string concatenation in a loop with heap promotion from SSO.

- [x] Create `18-string-builder.ori` with:
  - `@build_repeated(n: int, c: str) -> str` — loop appending `c` to a string `n` times
  - `@build_sequence(n: int) -> str` — loop appending `str(i)` for each `i` in `0..n`
  - `@build_with_separator(items: [str], sep: str) -> str` — loop joining strings with separator
  - `@main` — combines results, returns checkable integer
- [x] Verify interpreter produces correct result
- [x] Verify AOT produces correct result
- [x] Run `diagnostics/dual-exec-verify.sh` on J18 — interpreter and AOT produce identical output
- [x] Run `ORI_CHECK_LEAKS=1` — zero leaks
- [x] Run valgrind — zero errors, zero bytes at exit
- [x] Run `/code-journey` skill (via Skill tool) on the journey file — full pipeline: traces, deep scrutiny, scoring, results file
- [x] Create `18-string-builder-results.md` with full scoring breakdown
- [x] Target: 10/10 overall (if below 10/10, fix the underlying issue before proceeding to Section 04)

---

## 03.2 J19 — RC Lifecycle (Nested Heap Structures)

**File(s):** `plans/code-journeys/19-rc-lifecycle.ori`, `plans/code-journeys/19-rc-lifecycle-results.md`

Journey theme: "I am a lifecycle." Exercises structs containing heap fields, passing them to functions, extracting fields, and letting them go out of scope.

- [x] Create `19-rc-lifecycle.ori` with:
  - `type Container = { items: [int], name: str }` — struct with two heap fields
  - `@make_container(n: int) -> Container` — construct with heap fields
  - `@extract_and_use(c: Container) -> int` — project fields, use, let go
  - `@pass_through(c: Container) -> Container` — identity (tests ownership transfer)
  - `@nested_containers() -> int` — struct containing another struct with RC fields (exercises recursive aggregate drop)
  - `@main` — exercises all patterns, returns checkable integer
- [x] Verify interpreter produces correct result — exit 51 ✓ (2026-03-20)
- [x] Verify AOT produces correct result — exit 51 ✓ (2026-03-20). Required fix: defined-dead project-borrowed vars getting spurious RcDec (emit_defined_dead in walk_dec.rs). Fixed by skipping all borrowed defs in defined-dead context.
- [x] Run `diagnostics/dual-exec-verify.sh` on J19 — behavioral equivalence — DUAL-EXECUTION: ALL VERIFIED (2026-03-20)
- [x] Run `ORI_CHECK_LEAKS=1` — zero leaks — clean exit, no RC tracking output (2026-03-20)
- [x] Run valgrind — zero errors — 0 errors from 0 contexts, 16 allocs, 16 frees, 0 bytes in use at exit (2026-03-20)
- [x] Run `/code-journey` skill (via Skill tool) on the journey file — full pipeline: traces, deep scrutiny, scoring, results file (2026-03-20)
- [x] Create `19-rc-lifecycle-results.md` with full scoring breakdown — 10.0/10, all 7 functions OPTIMAL, zero ARC violations, 100% attribute compliance (2026-03-20)

---

## 03.3 J20 — COW Patterns (Shared vs Unique Mutation)

**File(s):** `plans/code-journeys/20-cow-patterns.ori`, `plans/code-journeys/20-cow-patterns-results.md`

Journey theme: "I am copy-on-write." Exercises the COW runtime — unique owner mutations (in-place), shared owner mutations (copy), and the transition between them.

- [x] Create `20-cow-patterns.ori` with:
  - `@unique_append() -> int` — unique string concat (should mutate in place)
  - `@shared_fork() -> int` — share a string, mutate one copy, verify original unchanged
  - `@list_cow_loop() -> int` — list push in a loop with COW fast path
  - `@slice_cow() -> int` — create slice from list, verify original unaffected, drop both cleanly
  - `@main` — combines results (returns 105)
- [x] Verify interpreter produces correct result — exit 105 (2026-03-20)
- [x] Verify AOT produces correct result — exit 105, matches eval (2026-03-20)
- [x] Run `diagnostics/dual-exec-verify.sh` on J20 — behavioral equivalence — DUAL-EXECUTION: ALL VERIFIED (2026-03-20)
- [x] Run `ORI_CHECK_LEAKS=1` — zero leaks — clean exit, no RC tracking output (2026-03-20)
- [x] Run valgrind — zero errors — 0 errors from 0 contexts, 9 allocs, 9 frees, 0 bytes in use at exit (2026-03-20)
- [x] Run `/code-journey` skill (via Skill tool) on the journey file — full pipeline: traces, deep scrutiny, scoring, results file (2026-03-20)
- [x] Create `20-cow-patterns-results.md` with full scoring breakdown — 10.0/10, all 5 functions OPTIMAL, zero defects, 100% attribute compliance (2026-03-20)

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [x] J18, J19, J20 source files created and committed (2026-03-20)
- [x] All 3 new journeys produce correct output on both eval and AOT — J18=67, J19=51, J20=105 (2026-03-20)
- [x] All 3 new journeys pass `diagnostics/dual-exec-verify.sh` (behavioral equivalence between interpreter and AOT) (2026-03-20)
- [x] All 3 new journeys have zero leaks (`ORI_CHECK_LEAKS=1`) (2026-03-20)
- [x] All 3 new journeys have zero valgrind errors (2026-03-20)
- [x] All 3 new journeys score 10/10 — J18=10.0, J19=10.0, J20=10.0 (2026-03-20)
- [x] Results files created in `plans/code-journeys/` (2026-03-20)
- [x] `plans/code-journeys/overview.md` updated with J18-J20 (2026-03-20)
- [x] Original 17 journeys still score 10/10 (no regression) — all 20 journeys at 10.0/10 (2026-03-20)

**Exit Criteria:** 20 total code journeys (17 existing + 3 new), all scoring 10/10, all leak-free, all valgrind-clean. The new journeys specifically cover heap loop reassignment, nested RC structures, and COW patterns — the three gap areas.
