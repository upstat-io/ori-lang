---
section: "02"
title: "Roadmap Reprioritization"
status: not-started
reviewed: false
goal: "Reorder Section 21A subsections by test-unblocking impact and create an accelerated implementation sequence with LCFail milestones."
inspired_by:
  - "Rust compiler priority issues — features blocking the most downstream work get highest priority"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Impact-Ordered Resequencing"
    status: not-started
  - id: "02.2"
    title: "LCFail Milestones"
    status: not-started
  - id: "02.3"
    title: "Roadmap Section Update"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Roadmap Reprioritization

**Status:** Not Started
**Goal:** Section 21A's subsections are reordered by test-unblocking impact. An accelerated implementation sequence exists with concrete LCFail milestones. The roadmap overview reflects the new priority ordering.

**Context:** Section 21A currently has 19 subsections ordered by logical topic (type lowering → expressions → control flow → ...). This ordering doesn't reflect test-unblocking impact. Generic monomorphization (within 21.7 "Function Sequences & Expressions") is the single largest blocker but sits in the middle of the sequence. Reordering by impact means the first features implemented unblock the most tests.

**Depends on:** Section 01 (LCFail Audit & Baseline) — the categorization table provides the data for impact ordering.

---

## 02.1 Impact-Ordered Resequencing

**File(s):** `plans/roadmap/section-21A-llvm.md`

Using the categorization data from Section 01, reorder Section 21A's implementation priorities. The current subsection numbering (21.1-21.19) stays for reference stability, but add an "Implementation Priority" field to each subsection.

**Proposed priority ordering** (to be validated against Section 01's actual counts):

- [ ] **Priority 0 — The Mega-Blocker:**
  - 21.7 "Function Sequences & Expressions" — specifically the generic monomorphization items within it
  - **Why first**: `assert_eq<T>` is generic and used in virtually every test file (~3,946 call sites as of 2026-03-25). No other single feature unblocks more tests. The monomorphization architecture document exists at `docs/ori_lang/v2026/design/monomorphization-architecture.md`. A `monomorphize/` module already exists in `ori_llvm/src/` with partial implementation.
  - **Expected impact**: ~60-80% of LCFail files unblocked (most files fail ONLY because of generic function calls)
  - **Dependencies**: Needs basic type lowering (21.2 primitives — already done) and function codegen (21.7 basics — partially done)
  - **Validation**: After setting this priority, verify claim by counting LCFail files that fail ONLY due to `sig.is_generic()` skips: `grep -c "is_generic" lcfail-audit.log` (from Section 01.2)

- [ ] **Priority 1 — Prelude Unlock:**
  - 21.2 "Type Lowering" — specifically the sum type codegen items (`Ordering` and user-defined sum types)
  - **Why second**: Once sum types work, prelude functions like `compare() -> Ordering` can be compiled. Currently the JIT runner explicitly skips prelude compilation (see `llvm_backend.rs:88-94` comment block). Re-enabling prelude unlocks files that need `compare`, `min`, `max`, etc.
  - **Expected impact**: ~10-15% of remaining LCFail files

- [ ] **Priority 2 — Higher-Order Functions:**
  - 21.11 "Lambda & Closure Support"
  - **Why third**: Many test files use `.map()`, `.filter()`, and other HOF patterns. Closures are fundamental to Ori's functional style.
  - **Expected impact**: ~5-10% of remaining LCFail files

- [ ] **Priority 3 — User Types:**
  - 21.4 "Operator Trait Dispatch" (which includes user-defined impl blocks, struct methods, and operator overloading)
  - **Why fourth**: Struct methods and operator overloading affect many test files that define custom types.
  - **Expected impact**: Unblocks struct-heavy test files

- [ ] **Priority 4 — Core Features:**
  - 21.12 Built-in functions (remaining prelude)
  - 21.5 Control flow extensions (for-yield, try, catch)
  - 21.3 Expression codegen (remaining gaps: spread, coalesce, floor div, bitwise)
  - **Why here**: Each individually unblocks a moderate number of files. Can be worked in parallel.

- [ ] **Priority 5 — Advanced Features:**
  - 21.6 Pattern matching (match expressions)
  - 21.10 Collections & iterators
  - **Why later**: These are important but affect fewer files than the above priorities.

- [ ] **Priority 6 — Specialized Features:**
  - 21.8 Concurrency patterns
  - 21.9 Capabilities & with pattern
  - 21.13 FFI support
  - 21.14 Conditional compilation
  - 21.15 Memory management (ARC)
  - **Why last**: These are niche features with fewer test files. Some (concurrency, FFI) may not have many spec tests yet.

- [ ] **Priority 7 — Infrastructure & Reference (not test-blocking):**
  - 21.1 LLVM Setup & Infrastructure (prerequisite — already largely complete)
  - 21.16 Optimization Passes (improves codegen quality, not test pass/fail)
  - 21.17 Runtime Support (extends runtime functions — incremental)
  - 21.18 Architecture Reference (documentation, not code)
  - 21.19 Section Completion Checklist (meta — tracks overall progress)
  - **Why last**: These either are prerequisites already in place, documentation, or optimization — they don't block LCFail tests.

- [ ] Add `priority: P{N}` annotation to ALL 19 subsections in Section 21A. This doesn't change the subsection numbering (21.1-21.19) but adds a "work on this first" signal.

**Note**: The priority ordering above is **preliminary**, based on the overview's root cause analysis. Section 01's categorization data will provide actual counts per category. The ordering MUST be validated and potentially revised once Section 01.2 produces the categorization table. Do NOT finalize priorities until Section 01 is complete.

### Test Strategy

- **Validation**: After resequencing, verify that implementing P0 (monomorphization) alone would reduce LCFail from 3,956 to <1,500 by running a thought experiment: count how many LCFail files fail ONLY because of generic function calls (no other missing features).

---

## 02.2 LCFail Milestones

Define concrete, measurable milestones for tracking progress toward LCFail=0.

- [ ] Create milestone table based on priority ordering:

  | Milestone | Target LCFail | Feature Completed | Gate Test |
  |-----------|--------------|-------------------|-----------|
  | M0 | <1,500 | Generic monomorphization (P0) | `assert_eq(actual: [1,2], expected: [1,2])` + `assert_eq(actual: "a", expected: "a")` compile in LLVM (exercises generic instantiation with multiple types) |
  | M1 | <1,000 | Sum types + prelude (P1) | `compare(left: 1, right: 2)` compiles in LLVM |
  | M2 | <500 | Lambdas + impl blocks (P2, P3) | `.map(x -> x * 2)` compiles in LLVM |
  | M3 | <200 | Core features (P4) | for-yield, try, built-ins compile |
  | M4 | <50 | Advanced features (P5) | match, collections compile |
  | M5 | 0 | Specialized features (P6) | All spec tests compile in LLVM |

- [ ] For each milestone, define a **gate test** — a minimal Ori program that exercises the feature. The gate test should be something that can be run quickly to verify the milestone is reached:
  ```
  // Example M0 gate test (floating test — "tests _" since no specific target):
  use std.testing { assert_eq }
  @test_assert_eq_works tests _ () -> void = {
      assert_eq(actual: 42, expected: 42)
  }
  ```

- [ ] Add milestone tracking to Section 21A as a "LCFail Milestones" subsection (or append to the overview).

- [ ] Document how to verify milestone progress: `./scripts/lcfail-report.sh` (from Section 01.3) produces the current LCFail count. After each P{N} feature is implemented in Section 21A, run this script and compare against the milestone table to determine if the milestone target is met.

---

## 02.3 Roadmap Section Update

**File(s):** `plans/roadmap/section-21A-llvm.md`, `plans/roadmap/00-overview.md`

- [ ] Add priority annotations to Section 21A (`plans/roadmap/section-21A-llvm.md`). For each subsection heading (21.1-21.19), add a bold priority line immediately below the `## 21.N Title` header:
  ```markdown
  ## 21.7 Function Sequences & Expressions
  **LCFail Priority: P0 (highest)** — Generic monomorphization unblocks ~60-80% of LCFail tests.
  ```
  Also add `priority: P{N}` to each subsection entry in the YAML frontmatter's `sections:` list.

- [ ] Add a "LCFail Milestones" section to `plans/roadmap/section-21A-llvm.md`, immediately after the "Import Resolution (Unified Pipeline)" section (after line ~107). Insert the milestone table from Section 02.2.

- [ ] Update `plans/roadmap/00-overview.md` — check Section 21A's tier and position:
  - Read `plans/roadmap/00-overview.md` and locate the Section 21A entry
  - If tier is listed as 8, evaluate whether P0-P2 items warrant elevation (consult user before changing tier)
  - If the user wants LCFail=0 as a near-term goal, add a note in the overview pointing to the milestones

- [ ] Add a "LCFail Tracking" section at the bottom of `plans/roadmap/section-21A-llvm.md` (before "## 21.19 Section Completion Checklist") with:
  - Current count (from Section 01)
  - Target milestones (from 02.2)
  - "Last Updated: YYYY-MM-DD" annotation
  - Command to get current count: `./scripts/lcfail-report.sh` (from Section 01.3)
  - Alternative manual command: `cargo b --release && ./target/release/ori test --verbose --backend=llvm tests/ 2>&1 | tail -5`

### Test Strategy

- **Validation**: After roadmap update, run `./target/release/ori test --verbose --backend=llvm tests/` and verify the summary line matches the numbers recorded in the updated Section 21A.
- **Semantic pin**: The stale number (1,985) must not appear anywhere in the roadmap after this section.

---

## 02.R Third Party Review Findings

- None.

---

## 02.4 Completion Checklist

- [ ] Section 21A subsections have priority annotations (P0-P6) based on LCFail impact
- [ ] LCFail milestones table exists with concrete targets and gate tests
- [ ] Roadmap overview updated to reflect 21A's new priority (if tier/position changed)
- [ ] LCFail tracking section exists in 21A with current count, milestones, and update instructions
- [ ] Stale number 1,985 replaced everywhere with actual 3,956 (or current count at time of implementation)
- [ ] No contradictions between Section 21A and roadmap overview
- [ ] `./test-all.sh` green

**Exit Criteria:** Section 21A has a clear, impact-ordered implementation sequence. Anyone running `/continue-roadmap` on Section 21A will work on the highest-impact features first. LCFail milestones provide measurable checkpoints. The roadmap reflects accurate data.

**Handoff:** After this section is complete, the actual LCFail-to-zero work happens by executing Section 21A's subsections in the new priority order (P0 first, then P1, etc.). This plan (test-suite-health) does NOT implement the codegen features — it creates the audit, prioritization, tracking, and milestones that guide Section 21A's implementation. The `scripts/lcfail-report.sh` script (from Section 01.3) is the ongoing tracking mechanism used during Section 21A work.
