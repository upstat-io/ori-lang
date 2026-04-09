---
section: "05"
title: "Journey Corpus Expansion"
status: not-started
reviewed: false
goal: "Add J21-J30+ journey programs that exercise every leak-prone pattern the current corpus misses"
success_criteria:
  - "At least 10 new journey programs (J21-J30) exist in plans/code-journeys/"
  - "Each new journey exercises a pattern from tests/valgrind/fat_matrix/ not covered by J1-J20"
  - "Patterns covered: break-in-loop with fat values, nested closures, partial iteration, ? with heap types, COW mutations with early exit, unwind cleanup, struct field reassignment, deeply nested drops"
  - "All new journeys compile and run correctly in both eval and AOT paths"
  - "At least one journey is designed to FAIL a diagnostic tool (known leak) as a canary test"
  - "Satisfies mission criterion: J21+ cover every fat_matrix pattern missing from J1-J20"
inspired_by:
  - "tests/valgrind/fat_matrix/ — systematic coverage matrix of fat pointer operations"
  - "tests/valgrind/cow/ — deep COW testing patterns"
  - "tests/valgrind/iter_rc/ — iterator + RC interaction tests"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Gap Analysis"
    status: not-started
  - id: "05.2"
    title: "Write J21-J25: Control Flow + Fat Values"
    status: not-started
  - id: "05.3"
    title: "Write J26-J30: Closures, Iterators, COW"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Journey Corpus Expansion

**Status:** Not Started
**Goal:** Add J21-J30+ journey programs targeting the leak-prone patterns that J1-J20 miss. Every pattern in `tests/valgrind/fat_matrix/` that has no journey coverage gets a dedicated journey.

**Success Criteria:**
- [ ] 10+ new journeys exist (J21-J30 minimum)
- [ ] Break-in-loop with heap strings/lists covered
- [ ] Nested closures capturing fat pointers covered
- [ ] Partial iterator consumption (take/break) covered
- [ ] `?` operator with `Option<str>` / heap types covered
- [ ] COW mutations with early exit covered
- [ ] Deeply nested drops (3+ levels) covered
- [ ] At least 1 canary journey with known leak (tests that the system detects it)
- [ ] Satisfies mission criterion for corpus expansion

**Context:** The TPR review proved J1-J8 use scalar types only and J9-J20, while better, still miss critical patterns. The valgrind fat_matrix has 20 test files covering every fat pointer operation — but code journeys never exercise those patterns. The expanded corpus closes this gap.

**Reference implementations:**
- `tests/valgrind/fat_matrix/f19_break_continue.ori` — break/continue with fat values in scope
- `tests/valgrind/fat_matrix/f04_closure_capture.ori` — 11 closure capture patterns
- `tests/valgrind/fat_matrix/f15_question_mark.ori` — `?` with Option<str>
- `tests/valgrind/cow/cow_leak_scenarios.ori` — explicit leak scenarios

**Depends on:** Section 03 (orchestrator — needed to run the new journeys).

---

## 05.1 Gap Analysis

- [ ] Map every `tests/valgrind/fat_matrix/` pattern to existing J1-J20 coverage
- [ ] Identify the uncovered patterns (from TPR findings):
  1. Break in for loop with `str`/`[str]`/`Option<str>` in scope
  2. Continue in for loop with fat values in scope
  3. Nested closures capturing heap types (2+ levels)
  4. `map().take(N).collect()` — partial iterator consumption
  5. `for x in iter do if cond then break` with fat iterator elements
  6. `?` on `Option<str>` in scope with other heap values
  7. COW string mutation inside an `if` branch (uniqueness barrier)
  8. List push in loop with early break (COW + control flow)
  9. Struct field reassignment in loop (old value must be freed)
  10. Deeply nested struct drops (Container > Nested > [str])
  11. Panic path cleanup (values in scope when panic fires)
- [ ] Assign each gap to a journey number and slug

- [ ] **Subsection close-out (05.1)** — MANDATORY before starting 05.2:
  - [ ] All tasks above are `[x]` and gap map complete
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 05.2 Write J21-J25: Control Flow + Fat Values

**File(s):** `plans/code-journeys/21-*.ori` through `25-*.ori`

Each journey follows the existing conventions: `@main () -> int`, standardized header comments, expected result comment, < 30 lines, focused on 1-2 interacting features.

- [ ] **J21: Break in for loop with heap string** (`21-break-fat-for.ori`)
  - For loop over `[str]`, break when condition met, verify remaining elements freed
  - Difficulty: complex, features: [loops, strings, break_continue, arc]
  - Inspired by: `tests/valgrind/fat_matrix/f19_break_continue.ori`

- [ ] **J22: Question mark with heap option** (`22-question-mark-heap.ori`)
  - Function returns `Option<str>`, caller uses `?`, verify cleanup on None path
  - Difficulty: complex, features: [option_type, strings, error_propagation, arc]
  - Inspired by: `tests/valgrind/fat_matrix/f15_question_mark.ori`

- [ ] **J23: Nested closure capture** (`23-nested-closure-capture.ori`)
  - Outer closure captures `str`, inner closure captures outer closure, verify env cleanup
  - Difficulty: complex, features: [closures, strings, capture, higher_order, arc]
  - Inspired by: `tests/valgrind/fat_matrix/f04_closure_capture.ori`

- [ ] **J24: Struct field reassignment** (`24-field-reassign-loop.ori`)
  - Loop reassigns a struct field holding `str`, verify old value freed each iteration
  - Difficulty: complex, features: [struct_construction, strings, loops, arc]
  - Inspired by: `tests/valgrind/aims_drop_reuse.ori`

- [ ] **J25: Deeply nested drop** (`25-deep-nested-drop.ori`)
  - 3-level nesting: Outer { inner: Inner { items: [str] } } — verify cascading drops
  - Difficulty: complex, features: [nested_structs, lists, strings, arc]
  - Inspired by: `tests/valgrind/recursion_stress.ori`

- [ ] Run all 5 new journeys through `/code-journey` to verify they produce valid results
- [ ] Verify eval and AOT paths produce identical output for each

- [ ] **Subsection close-out (05.2)** — MANDATORY before starting 05.3:
  - [ ] All tasks above are `[x]` and J21-J25 produce valid results
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 05.3 Write J26-J30: Closures, Iterators, COW

**File(s):** `plans/code-journeys/26-*.ori` through `30-*.ori`

- [ ] **J26: Partial iterator consumption** (`26-partial-iter.ori`)
  - `iter.take(3).collect()` on a 10-element list of strings — verify remaining 7 freed
  - Difficulty: complex, features: [iterators, strings, lists, arc]
  - Inspired by: `tests/valgrind/iter_rc/`

- [ ] **J27: COW mutation with early exit** (`27-cow-break.ori`)
  - List push in loop with conditional break, verify COW buffer freed on exit
  - Difficulty: complex, features: [cow, lists, loops, break_continue, arc]
  - Inspired by: `tests/valgrind/cow/cow_leak_scenarios.ori`

- [ ] **J28: Continue in for-yield with fat values** (`28-continue-yield-fat.ori`)
  - `for x in items if cond yield transform(x)` — verify skipped elements properly cleaned
  - Difficulty: complex, features: [loops, strings, for_yield, arc]

- [ ] **J29: Multiple shared references** (`29-multi-shared.ori`)
  - Create 3 aliases to same list, mutate one (COW), verify others unchanged, all freed
  - Difficulty: complex, features: [cow, lists, strings, arc]
  - Inspired by: `tests/valgrind/cow/cow_map_operations.ori`

- [ ] **J30: Canary — known leak program** (`30-canary-leak.ori`)
  - A program designed to trigger a specific known leak pattern
  - This journey's EXPECTED verdict is "findings" not "clean" — it tests that the system detects issues
  - If the system reports "clean" on this journey, the detection is broken
  - Difficulty: complex, features: [arc, canary]

- [ ] Run all 5 new journeys through `/code-journey`
- [ ] Verify J30 canary produces findings (not clean)

- [ ] **Subsection close-out (05.3)** — MANDATORY before marking section complete:
  - [ ] All tasks above are `[x]` and J26-J30 produce valid results
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] J21-J30 (10 journeys) exist and compile
- [ ] Each journey exercises a previously uncovered leak-prone pattern
- [ ] All journeys produce valid JSON results via `/code-journey`
- [ ] J30 canary correctly detects the intended leak
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `/tpr-review` — dual-source review of new journey programs
- [ ] `/impl-hygiene-review` — verify naming conventions, no DRIFT from existing journey patterns
- [ ] `/improve-tooling` section-close sweep
