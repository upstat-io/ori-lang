---
section: "06"
title: "Expand Fixtures + Self-Test"
status: in-progress
reviewed: true
goal: "Add 13+ new diagnostic fixtures covering the code patterns that cause the most AOT/AIMS debugging churn, update self-test.sh to exercise them with feature-specific assertions, and establish a fixture categorization system"
success_criteria:
  - "At least 13 new fixture files in diagnostics/fixtures/ organized by category (pass, aims-heavy, expected-fail)"
  - "Each pass fixture compiles and runs identically under interpreter and AOT, in both debug and release builds"
  - "Expected-fail fixtures are mandatory, not optional — they validate that diagnostic scripts correctly detect failures"
  - "self-test.sh runs all new fixtures through the appropriate diagnostic scripts with feature-specific output assertions"
  - "Fixture matrix documented as SSOT in diagnostics/fixtures/FIXTURES.md"
inspired_by:
  - "Swift SIL test fixtures — targeted programs exercising specific SIL optimizer paths"
  - "Koka FBIP test corpus — programs that stress the PARC optimization pipeline"
  - "Lean 4 bug-series fixtures — systematic coverage of a single subsystem (closure_bug1-8)"
depends_on: ["05"]
third_party_review:
  status: resolved
  updated: 2026-04-10
sections:
  - id: "06.1"
    title: "Create core-pattern fixtures"
    status: complete
  - id: "06.2"
    title: "Create ARC-interaction fixtures"
    status: complete
  - id: "06.3"
    title: "Create expected-fail fixtures"
    status: complete
  - id: "06.4"
    title: "Fixture matrix and categorization"
    status: not-started
  - id: "06.5"
    title: "Update self-test.sh coverage"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Expand Fixtures + Self-Test

**Status:** Not Started
**Goal:** The diagnostic toolkit's self-test suite runs against only 3 basic fixtures (`simple.ori`, `clean.ori`, `chain.ori`). These don't exercise closures, iterators, nested structures, generics, trait dispatch, or failure modes — the exact code patterns that cause the most debugging churn. New fixtures ensure diagnostic scripts produce correct output for the patterns they'll actually be used to debug. The fixture suite must also cover escape closures, `?` unwinding, recursive tree walks, COW sharing, large aggregates, and mixed sum types — all identified as blind spots by tp-help consensus.

**Success Criteria:**
- [ ] At least 13 new `.ori` fixture files in `diagnostics/fixtures/`
- [ ] Each fixture exercises a distinct code pattern relevant to AOT/AIMS debugging
- [ ] Fixtures categorized as **pass** (exit 0, clean RC), **aims-heavy** (exit 0, exercises AIMS-specific paths like COW/reuse), or **expected-fail** (exit non-zero, validates diagnostic detection)
- [ ] `self-test.sh` runs new fixtures through `diagnose-aot.sh`, `dual-exec-debug.sh`, `rc-stats.sh`, `ir-dump.sh`, `arc-dump.sh`, and `bisect-passes.sh` (added by Section 05)
- [ ] `bisect-passes.sh` exercised on at minimum `closure.ori`, `iterator_break.ori`, and `generic_mono.ori` (the AIMS-relevant fixtures)
- [ ] Self-test assertions are **feature-specific** — not just "non-empty output" but assertions on expected IR markers (e.g., `PartialApply` for closures, `Switch` for match, `RcInc`/`RcDec` for RC-heavy fixtures)
- [ ] Expected-fail fixtures use `run_test_expect_fail` with explicit exit code assertions distinguishing leak vs crash vs mismatch
- [ ] All fixtures verified under both debug and release builds (`cargo b` and `cargo b --release`)
- [ ] Satisfies mission criterion: "7+ new diagnostic fixtures covering closures, iterators, nested structures, generics, trait dispatch, and failure modes"

**Context:** The current 3 fixtures (`simple.ori` — no collections/RC; `clean.ori` — collections, balanced RC; `chain.ori` — chained COW) were adequate when the toolkit was first built. But ARC/AIMS bugs predominantly appear in closure captures, iterator early-exit cleanup, nested aggregate drops, generic instantiation, and trait method dispatch — none of which are exercised. A diagnostic regression in these areas ships behind a green self-test.

**Depends on:** Section 05 (bisect-passes.sh must exist for self-test integration).

**README ownership:** Section 07 owns the `diagnostics/README.md` fixtures table update (see `section-07-integration.md` 07.4). This section creates the fixtures and the `FIXTURES.md` categorization file; Section 07 integrates the final table into the user-facing README.

---

## 06.1 Create core-pattern fixtures

**File(s):** `diagnostics/fixtures/*.ori` (new files)

Each fixture must: (1) compile under AOT, (2) produce deterministic output via exit code (0 = success, 1 = logic failure), (3) exercise a specific code pattern, (4) pass both `ori run` and AOT binary execution with identical results. Fixture names are descriptive of the pattern, not the section number. Reference existing test files in `tests/valgrind/fat_matrix/` for correct Ori syntax patterns.

**Category: pass** — all exit 0, balanced RC.

- [x] **`closure.ori`** — Closure capturing a collection (`[int]`), calling the closure, verifying captures are alive after the call. Tests closure RC: the captured value must be inc'd on capture, dec'd on closure drop. **Must also include:** closure passed as function argument, closure called twice (RC balance after multiple invocations). Reference syntax: `tests/valgrind/fat_matrix/f04_closure_capture.ori`
- [x] **`closure_escape.ori`** — Closures that escape their creation scope: stored in a list, passed as a parameter to another function, returned from a function, and called after the creating scope has exited. This is a GAP identified by tp-help — capture-only coverage is insufficient for RC correctness because escaping closures stress the lifetime of captured values beyond lexical scope. Reference syntax: `tests/valgrind/fat_matrix/f04_closure_capture.ori` (for capture patterns), `tests/spec/expressions/lambdas.ori` (for lambda syntax)
- [x] **`iterator_break.ori`** — Iterate over `[str]` with early `break`, verifying the iterator and remaining elements are properly dropped. This is the #1 ARC debugging pain point. Must include: full iteration (no break), break on first element, break on middle element, `continue` skipping elements. Reference syntax: `tests/valgrind/fat_matrix/f19_break_continue.ori`
- [x] **`iterator_complex.ori`** — Iterator patterns beyond simple break: nested `for` loops with fat values in both levels, `for...yield` with break producing partial collection, `continue` with guard filtering, map iteration and cleanup. tp-help identified single `iterator_break.ori` as insufficient — iterator coverage must be deeper. Reference syntax: `tests/valgrind/fat_matrix/f19_break_continue.ori`, `tests/spec/traits/iterator/for_loop.ori`
- [x] **`nested_list.ori`** — Nested `[[str]]` collection, exercising `elem_dec_fn` propagation for nested drops. Include: creating nested lists, accessing inner elements, passing nested lists to functions. Reference syntax: `tests/valgrind/fat_matrix/f14_list_element.ori`
- [x] **`trait_dispatch.ori`** — Trait method call through a concrete `impl Trait for Type` (current compiler syntax), testing that trait dispatch codegen produces balanced RC. Include: trait with required method, trait with default method, calling trait method on a value that owns fat pointers. **Note**: current compiler uses `impl Trait for Type` syntax (not `impl Type: Trait` — that's approved but not yet implemented per CLAUDE.md). Reference syntax: `tests/spec/traits/declaration.ori`
- [x] **`pattern_match.ori`** — Sum type with 3+ variants including mixed scalar and fat-pointer payloads (e.g., `A(x: int) | B(s: str) | C(xs: [int])`), exercising tag dispatch and per-variant drops. tp-help identified this as a gap: mixed scalar/ref variants stress the decision tree codegen differently than uniform variants. Reference syntax: `tests/valgrind/fat_matrix/f06_pattern_matching.ori`, `tests/valgrind/fat_matrix/f12_sum_payload.ori`
- [x] **`map_iteration.ori`** — Map creation with string keys, iteration over entries, map lookup, verifying RC for both keys and values during iteration. Reference syntax: `tests/valgrind/iter_rc/map_str_iteration.ori`, `tests/valgrind/iter_rc/map_str_for_do.ori` (active executable map examples; NOT `tests/spec/types/map_types.ori` which is a disabled TODO corpus)
- [x] Verify each fixture: `cargo run -- run <fixture>` produces expected exit code, `cargo run -- build <fixture> -o /tmp/test_fixture && /tmp/test_fixture` produces the same exit code

- [x] **Subsection close-out (06.1)** — MANDATORY before starting 06.2:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.2 Create ARC-interaction fixtures

**File(s):** `diagnostics/fixtures/*.ori` (new files)

These fixtures exercise ARC-specific interaction patterns that tp-help identified as blind spots. They are **pass** fixtures (exit 0) but are categorized as **aims-heavy** because they specifically stress AIMS pipeline phases.

**Category: aims-heavy** — all exit 0, but exercise AIMS-specific paths (COW, reuse, `?` unwinding, recursion).

- [x] **`question_mark.ori`** — `?` operator propagation with fat values in scope (heap `str`, `[int]`, struct-with-fat-field). Must include: `?` on `Option<str>` returning `None`, `?` on `Option<[int]>` returning `Some`, chained `?` with multiple fat locals in scope that must be cleaned up on early exit. tp-help identified this as mandatory ARC interaction coverage — `?` triggers early-exit unwinding that must drop all live fat values. Reference syntax: `tests/valgrind/fat_matrix/f15_question_mark.ori`
- [x] **`recursive_tree.ori`** — Recursive function passing fat pointer types through recursive call frames: heap `str` through `N` levels, `[int]` through recursion, struct with fat field returned from recursive base case. Exercises stack-frame RC correctness across recursive depth. Reference syntax: `tests/valgrind/fat_matrix/f16_recursion.ori`
- [x] **`generic_mono.ori`** — Generic function instantiated with **multiple concrete types**: scalar (`int`), heap string (`str`), list (`[int]`), and struct-with-fat-field. tp-help identified single-type generic coverage as insufficient — monomorphization must be tested across the type matrix to verify RC analysis is correct for each instantiation. Reference syntax: `tests/valgrind/fat_matrix/f10_generics.ori`
- [x] **`large_aggregate.ori`** — Struct with 3+ `int` fields (>16 bytes) passed to and returned from functions, exercising ABI compliance for large aggregates. Must verify that pass-by-reference codegen does not trigger unnecessary RC operations. Catches FastISel vs full pipeline regressions. Reference syntax: `tests/valgrind/fat_matrix/f10_generics.ori` (for struct patterns)
- [x] **`cow_sharing.ori`** — COW sharing barrier exercise: create a list, alias it (shared), mutate through one alias (triggers COW clone), verify original is unchanged. Also: multi-fork (3+ references to same backing), and push-after-share on both sides. Exercises `is_unique` check and COW clone path. Reference syntax: `tests/valgrind/cow/cow_list_push.ori`

- [x] Verify each fixture: `cargo run -- run <fixture>` and `cargo run -- build <fixture> -o /tmp/test_fixture && /tmp/test_fixture` produce identical exit code 0
- [x] Verify each fixture under release build: `cargo run --release -- build <fixture> -o /tmp/test_fixture && /tmp/test_fixture` produces exit code 0

- [x] **Subsection close-out (06.2)** — MANDATORY before starting 06.3:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.3 Create expected-fail fixtures

**File(s):** `diagnostics/fixtures/*.ori` (new files)

tp-help identified that failure fixtures were "optional and underspecified" — this is a coverage gap. Diagnostic scripts must be validated in failure mode, not just success mode. These fixtures are **mandatory**.

**Category: expected-fail** — designed to trigger specific diagnostic failures.

- [x] **`leak.ori`** — Program that intentionally leaks an RC value (e.g., create a circular reference or allocate without drop path). `ORI_CHECK_LEAKS=1` must report a leak. `diagnose-aot.sh` must detect the leak. This validates that the leak detection path in diagnostic scripts actually works.
  - Safe Ori code cannot create true RC leaks (no circular references, ARC manages all allocations). Created best-effort fixture: panic with fat values in scope causes `diagnose-aot.sh` to report FAIL (execution exit=1) + WARN (RC Stats imbalanced: over-releases from incomplete cleanup). `ORI_CHECK_LEAKS=1` does not report leaks because the panic handler bypasses `ori_run_main`'s return path where the leak check runs.
- [x] **`mismatch_compute.ori`** — Program that (via the mismatch-wrapper.sh infrastructure already in `diagnostics/fixtures/`) produces different interpreter vs AOT output. This validates that `dual-exec-debug.sh` correctly detects and reports mismatches with auto-diagnostic output. **Note:** The existing `mismatch.ori` + `mismatch-wrapper.sh` already serves this purpose — verify it is sufficient or extend it.
  - Verified: existing `mismatch.ori` + `mismatch-wrapper.sh` is sufficient. `ORI_BIN=mismatch-wrapper.sh dual-exec-debug.sh mismatch.ori` correctly detects MISMATCH (stdout "INTERP" vs "AOT"), exits 1, and produces auto-diagnostic output. No separate `mismatch_compute.ori` needed.

- [x] **Subsection close-out (06.3)** — MANDATORY before starting 06.4:
  - [x] All tasks above are `[x]` and verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.4 Fixture matrix and categorization

**File(s):** `diagnostics/fixtures/FIXTURES.md` (new file)

tp-help identified scattered fixture knowledge as a LEAK — fixture names are repeated per-script in self-test with no single source of truth for what each fixture covers. This subsection creates the SSOT.

- [ ] Create `diagnostics/fixtures/FIXTURES.md` with a categorization table:

  | Fixture | Category | Pattern | Key ARC/AIMS Paths | Expected Exit | bisect-passes? |
  |---------|----------|---------|-------------------|---------------|----------------|
  | `simple.ori` | pass | No collections, no RC | Baseline (no RC ops) | 0 | Yes |
  | `clean.ori` | pass | Collections + balanced RC | RC alloc/dec, list ops | 0 | Yes |
  | `chain.ori` | pass | Chained COW ops | COW clone path, sequential mutation | 0 | Yes |
  | `closure.ori` | pass | Closure capture + call | PartialApply, closure env RC | 0 | Yes |
  | `closure_escape.ori` | pass | Escaping closures | Closure lifetime beyond scope | 0 | Yes |
  | `iterator_break.ori` | pass | Iterator early exit | Iterator drop, elem cleanup | 0 | Yes |
  | `iterator_complex.ori` | pass | Nested/yield/guard iteration | Nested loop RC, partial collect | 0 | Yes |
  | `nested_list.ori` | pass | Nested collections | elem_dec_fn propagation | 0 | Yes |
  | `generic_mono.ori` | aims-heavy | Multi-type generic instantiation | Monomorphization RC correctness | 0 | Yes |
  | `trait_dispatch.ori` | pass | Trait method dispatch | Trait vtable codegen, method RC | 0 | Yes |
  | `pattern_match.ori` | pass | Sum type mixed variants | Decision tree, per-variant drop | 0 | Yes |
  | `map_iteration.ori` | pass | Map create + iterate | Map RC, iterator cleanup | 0 | Yes |
  | `question_mark.ori` | aims-heavy | `?` with fat values | Early-exit unwinding, drop all live | 0 | Yes |
  | `recursive_tree.ori` | aims-heavy | Recursive fat pointer passing | Stack-frame RC across depth | 0 | Yes |
  | `large_aggregate.ori` | aims-heavy | >16B struct pass/return | ABI compliance, large aggregate load | 0 | Yes |
  | `cow_sharing.ori` | aims-heavy | COW sharing/fork | is_unique, COW clone barrier | 0 | Yes |
  | `leak.ori` | expected-fail | Intentional leak | Leak detection path | non-zero | Yes (expect exit 1) |
  | `mismatch_compute.ori` | expected-fail | Interpreter vs AOT mismatch | Mismatch detection path | non-zero | No |

- [ ] In `FIXTURES.md`, document the self-test contract for each category:
  - **pass**: `ir-dump.sh` (non-empty), `arc-dump.sh` (non-empty), `diagnose-aot.sh` (exit 0), `dual-exec-debug.sh` (MATCH), `rc-stats.sh` (produces output), `bisect-passes.sh` (phase table)
  - **aims-heavy**: same as pass, PLUS `bisect-passes.sh` must show RC operations (not trivially empty), AND assertions on feature-specific IR markers
  - **expected-fail**: `diagnose-aot.sh` must report failure (`run_test_expect_fail`), specific exit code documented per fixture

- [ ] **Subsection close-out (06.4)** — MANDATORY before starting 06.5:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.5 Update self-test.sh coverage

**File(s):** `diagnostics/self-test.sh`

- [ ] Update the fixture existence check at the top of `self-test.sh` (currently checks `simple.ori`, `clean.ori`, `chain.ori` only) to also require all new fixtures. Group by category (pass, aims-heavy, expected-fail) with comments. **SSOT note**: fixture lists in self-test.sh must be verifiable against `diagnostics/fixtures/FIXTURES.md` — add a comment referencing FIXTURES.md as the canonical source. If feasible, parse FIXTURES.md to generate the fixture arrays rather than hardcoding them (eliminates the LEAK:scattered-knowledge risk identified by TPR).

- [ ] Add each **pass fixture** (`closure`, `closure_escape`, `iterator_break`, `iterator_complex`, `nested_list`, `trait_dispatch`, `pattern_match`, `map_iteration`) to the self-test matrix:
  - `ir-dump.sh --no-color <fixture>` produces non-empty IR
  - `arc-dump.sh --no-color <fixture>` produces non-empty ARC IR
  - `diagnose-aot.sh --no-color <fixture>` passes all checks (exit 0)
  - `dual-exec-debug.sh --no-color <fixture>` shows MATCH
  - `rc-stats.sh --no-color <fixture>` produces output containing "Function"
  - `bisect-passes.sh --no-color <fixture>` produces phase table containing "Phase" (exercises AIMS pipeline tracing checkpoints)

- [ ] Add **feature-specific assertions** for aims-heavy and select pass fixtures (tp-help identified "non-empty IR" as too weak):
  - `closure.ori`: `arc-dump.sh` output contains `PartialApply` (closure construction in ARC IR)
  - `closure_escape.ori`: `arc-dump.sh` output contains `PartialApply`
  - `iterator_break.ori`: `bisect-passes.sh` output shows non-zero RC operations (not `inc:0 dec:0`)
  - `pattern_match.ori`: `arc-dump.sh` output contains `Switch` (decision tree in ARC IR)
  - `generic_mono.ori`: `arc-dump.sh` output contains at least 2 different function entries (multiple monomorphizations)
  - `question_mark.ori`: `arc-dump.sh` output contains `RcDec` (cleanup on early exit)
  - `cow_sharing.ori`: `arc-dump.sh` output contains `IsShared` (COW uniqueness check)

- [ ] Add **aims-heavy fixture** feature-specific assertions:
  - `recursive_tree.ori`: `arc-dump.sh` output contains at least 2 function entries (recursive + base case)
  - `large_aggregate.ori`: `ir-dump.sh` output does NOT contain `load { i64, i64, i64 }` (large aggregates must be passed by pointer, not by value)
- [ ] Add each **aims-heavy fixture** (`generic_mono`, `question_mark`, `recursive_tree`, `cow_sharing`, `large_aggregate`) to the self-test matrix with the same pass-fixture checks PLUS the feature-specific assertions above.

- [ ] Add each **expected-fail fixture** to the self-test with **specific exit code assertions** (not generic `run_test_expect_fail`):
  - `leak.ori`: assert `diagnose-aot.sh` exits non-zero AND output contains "leak" or "imbalance" (validates leak detection)
  - `leak.ori`: assert `bisect-passes.sh --rc-only` output does NOT contain "Leak check: clean" (validates bisection distinguishes leak from normal RC activity)
  - `mismatch_compute.ori` (or existing `mismatch.ori` + wrapper): assert `dual-exec-debug.sh` exits non-zero AND output contains "MISMATCH" (validates mismatch detection)
  - Each assertion must distinguish the *failure mode* (leak vs mismatch vs crash) via output pattern, not just exit code

- [ ] Handle `bisect-passes.sh` exit code semantics: `bisect-passes.sh` exits 1 for ANY phase delta (including normal RC insertions), so exit code 0 is unreliable for non-trivial programs. Fresh verification confirmed: `bisect-passes.sh --rc-only clean.ori` and `chain.ori` both exit 1 despite balanced final RC and clean leak check. Self-test must:
  - For pass/aims-heavy fixtures: assert `bisect-passes.sh --rc-only <fixture>` **produces phase table output** containing "Phase" (proves the tool ran and parsed tracing events). Do NOT assert exit 0 — normal RC-using programs trigger exit 1. Assert that output contains "Leak check: clean" (proves final RC balance).
  - For expected-fail `leak.ori`: assert `bisect-passes.sh --rc-only` output does NOT contain "Leak check: clean" (distinguishes leak from normal RC activity)

- [ ] **Release build coverage** (tp-help GAP): Add a conditional section (gated on `target/release/ori` existence, like the existing `debug-release-compare.sh` section) that runs `diagnose-aot.sh --release` on at least 3 representative fixtures (`closure.ori`, `iterator_break.ori`, `generic_mono.ori`). This catches optimization-dependent regressions (FastISel vs full pipeline).
  - If release binary not found, SKIP with a message (not FAIL)

- [ ] Verify: `diagnostics/self-test.sh --verbose` passes with expanded coverage
- [ ] Verify: all new self-test assertions pass in CI-equivalent conditions (clean build)

- [ ] **Subsection close-out (06.5)** — MANDATORY before starting 06.R:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.R Third Party Review Findings

- [x] `[TPR-06-001-codex][high]` `section-06-fixtures.md:142` — Centralize fixture categories to remove LEAK and DRIFT. generic_mono.ori inconsistency, self-test.sh as second registry.
  Resolved: Fixed on 2026-04-10. Moved generic_mono.ori to 06.2 (aims-heavy), added SSOT note to 06.5 fixture list requiring FIXTURES.md cross-reference.
- [x] `[TPR-06-002-codex][medium]` `section-06-fixtures.md:48` — Add large aggregate coverage promised by the goal.
  Resolved: Fixed on 2026-04-10. Added large_aggregate.ori fixture to 06.2 with >16B struct pattern and IR assertion.
- [x] `[TPR-06-003-codex][medium]` `section-06-fixtures.md:200` — Complete expected-fail matrix with exact exit-code assertions.
  Resolved: Fixed on 2026-04-10. Added mismatch_compute.ori to FIXTURES.md table, replaced generic run_test_expect_fail with specific exit code + output pattern assertions.
- [x] `[TPR-06-001-gemini][medium]` `section-06-fixtures.md:195` — Add mismatch_compute.ori to FIXTURES.md table.
  Resolved: Fixed on 2026-04-10. Same fix as [TPR-06-003-codex].
- [x] `[TPR-06-002-gemini][low]` `section-06-fixtures.md:79` — Harmonize generic_mono.ori categorization.
  Resolved: Fixed on 2026-04-10. Same fix as [TPR-06-001-codex] — moved to 06.2 aims-heavy.
- [x] `[TPR-06-003-gemini][medium]` `section-06-fixtures.md:214` — Use --rc-only flag for bisect-passes self-test assertions.
  Resolved: Fixed on 2026-04-10. Updated 06.5 to specify `--rc-only` flag and explain why it's load-bearing.
- [x] `[TPR-06-004-gemini][low]` `section-06-fixtures.md:180` — Correct bisect-passes coverage for simple.ori in SSOT table.
  Resolved: Fixed on 2026-04-10. Changed simple.ori bisect-passes from "No (trivial)" to "Yes".
- [x] `[TPR-06-005-gemini][medium]` `section-06-fixtures.md:225` — Exercise leak.ori with bisect-passes.sh to verify detection.
  Resolved: Fixed on 2026-04-10. Added leak.ori to bisect-passes coverage with exit 1 assertion, updated table.

---

## 06.N Completion Checklist

- [ ] All subsections (06.1, 06.2, 06.3, 06.4, 06.5) complete
- [ ] All pass/aims-heavy fixtures compile and run under both interpreter and AOT
- [ ] All pass/aims-heavy fixtures produce identical results under debug and release builds
- [ ] Expected-fail fixtures correctly trigger diagnostic detection
- [ ] `diagnostics/fixtures/FIXTURES.md` exists and is the SSOT for fixture categorization
- [ ] `diagnostics/self-test.sh` passes with all new fixtures
- [ ] Feature-specific assertions validate real IR markers, not just "non-empty"
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **`/improve-tooling` section-close sweep**
- [ ] **Strip plan annotations** — remove any `Section 06` / `§06` code comments from implemented files
