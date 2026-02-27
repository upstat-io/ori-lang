---
section: "11"
title: "Verification"
status: not-started
goal: "All 12 journeys pass, 0 dual-exec mismatches, 0 valgrind errors, all findings resolved"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10"]
sections:
  - id: "11.1"
    title: "Re-run all 12 code journeys"
    status: not-started
  - id: "11.2"
    title: "Dual-execution verification"
    status: not-started
  - id: "11.3"
    title: "Memory safety verification"
    status: not-started
  - id: "11.4"
    title: "Test matrix"
    status: not-started
  - id: "11.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 11: Verification

**Status:** Not Started
**Goal:** Prove that all 28 findings from 12 code journeys are resolved. Eval and AOT produce identical results for all test programs. No memory safety issues.

**Context:** This section runs after all fixes are implemented. It validates the system as a whole — not individual fixes in isolation. The code journey programs are the integration test suite, and dual-execution verification is the correctness oracle.

**Depends on:** All other sections (01-10).

---

## 11.1 Re-run All 12 Code Journeys

Run `/code-journey` with all 12 journey programs and verify results.

- [ ] Journey 1 (arithmetic): Eval = 33, AOT = 33
- [ ] Journey 2 (branching): Eval = 17, AOT = 17
- [ ] Journey 3 (recursion): Eval = 61, AOT = 61
- [ ] Journey 4 (structs): Eval = 57, AOT = 57
- [ ] Journey 5 (closures): Eval = 27, AOT = 27 (was CRASH)
- [ ] Journey 6 (sum types): Eval = 41, AOT = 41
- [ ] Journey 7 (loops): Eval = 30, AOT = 30
- [ ] Journey 8 (generics): Eval = 57, AOT = 57
- [ ] Journey 9 (strings): Eval = 13, AOT = 13
- [ ] Journey 10 (lists): Eval = 33, AOT = 33
- [ ] Journey 11 (derived traits): Eval = 33, AOT = 33 (was 18)
- [ ] Journey 12 (Option/match): Eval = 33, AOT = 33 (was 144)

---

## 11.2 Dual-Execution Verification

Run `./scripts/dual-exec-verify.sh` on all spec tests.

- [ ] Run: `./scripts/dual-exec-verify.sh --verbose`
- [ ] Result: 0 mismatches between eval and AOT paths
- [ ] Any new mismatches discovered are triaged and tracked

---

## 11.3 Memory Safety Verification

Run Valgrind on programs that exercise ARC lifecycle.

- [ ] Run: `./scripts/valgrind-aot.sh` on standard test suite
- [ ] Run Valgrind on Journey 9 (strings) — ARC string lifecycle
- [ ] Run Valgrind on Journey 10 (lists) — ARC list lifecycle
- [ ] Result: 0 invalid reads, 0 invalid writes, 0 leaks (definitely lost)

---

## 11.4 Test Matrix

Verify each finding is resolved by its corresponding test.

### CRITICAL Findings

| ID | Description | Test | Status |
|----|-------------|------|--------|
| C1 | Mixed closures crash AOT | Journey 5 returns 27 | [ ] |
| C2 | List indexing crashes AOT | `xs[0]` returns correct value | [ ] |
| C3 | Payload sum type $eq not generated | Journey 11 returns 33 | [ ] |
| C4 | Option match tag inversion | Journey 12 returns 33 | [ ] |

### HIGH Findings

| ID | Description | Test | Status |
|----|-------------|------|--------|
| H1 | Empty landing pads for all calls | Journey 3 IR has no empty landing pads | [ ] |
| H2 | Unsound nounwind on runtime calls | nounwind only on provably-nounwind functions | [ ] |

### MEDIUM Findings

| ID | Description | Test | Status |
|----|-------------|------|--------|
| M1 | Prelude overhead | Assessed with measurements | [ ] |
| M2 | No nsw on arithmetic | Design decision documented | [ ] |
| M3 | Dead br label after calls | 0 dead branches in all journey IR | [ ] |
| M4 | No tail call optimization | Assessed (implemented or deferred) | [ ] |
| M5 | align 4 on i64 loads | All i64 loads use align 8 | [ ] |
| M6 | Full struct load for partial access | Assessed (implemented or deferred) | [ ] |
| M7 | Verbose variant construction | insertvalue chain, no alloca roundtrip | [ ] |
| M8 | Identical match arms not deduped | Assessed (implemented or deferred) | [ ] |
| M9 | Range overflow for ..=INT_MAX | Inclusive range uses <= condition | [ ] |
| M10 | Inconsistent nounwind on main | Consistent nounwind analysis | [ ] |
| M11 | Orphaned landing pads | 0 orphaned blocks | [ ] |
| M12 | Duplicate drop functions | 1 drop per unique layout | [ ] |
| M13 | Unnecessary Option tuple in iterator | Direct i8 check, no tuple | [ ] |
| M14 | None loads uninitialized payload | No uninitialized reads | [ ] |

### LOW Findings

| ID | Description | Test | Status |
|----|-------------|------|--------|
| L1 | Canonicalizer expansion | Assessed | [ ] |
| L2 | Prelude decision trees | Assessed | [ ] |
| L3 | branch+phi instead of select | select for trivial if/else | [ ] |
| L4 | Single-predecessor phi | No single-predecessor phis | [ ] |
| L5 | Range struct materialization | Assessed (implemented or deferred) | [ ] |
| L6 | Duplicate loop computation | Assessed (implemented or deferred) | [ ] |
| L7 | Dead phi at loop exit | Assessed (implemented or deferred) | [ ] |

---

## 11.5 Completion Checklist

- [ ] All 12 journeys: correct results in both eval and AOT
- [ ] `./scripts/dual-exec-verify.sh` — 0 mismatches
- [ ] `./scripts/valgrind-aot.sh` — 0 errors
- [ ] `./test-all.sh` — green
- [ ] `./clippy-all.sh` — green
- [ ] All 4 CRITICAL findings fixed
- [ ] All 2 HIGH findings fixed
- [ ] All 14 MEDIUM findings fixed or assessed with documented rationale
- [ ] All 7 LOW findings assessed with documented rationale
- [ ] Test matrix (11.4) fully checked

**Exit Criteria:** 28/28 findings resolved (fixed, assessed, or deferred with rationale). 12/12 journeys correct. 0 dual-exec mismatches. 0 valgrind errors. Full test suite green.
