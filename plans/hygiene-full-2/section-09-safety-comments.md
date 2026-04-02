---
section: "09"
title: "SAFETY Comments for ori_rt"
status: not-started
reviewed: false
goal: "Add SAFETY comments to all ~80 unsafe blocks in ori_rt COW and iterator code — document invariants, preconditions, and pointer validity"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "09.1"
    title: "COW List Files"
    status: not-started
  - id: "09.2"
    title: "COW Map/Set Files"
    status: not-started
  - id: "09.3"
    title: "Iterator Files"
    status: not-started
  - id: "09.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "09.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: SAFETY Comments for ori_rt

**Status:** Not Started
**Goal:** Every `unsafe` block in ori_rt must have a `// SAFETY:` comment documenting: what invariants make this safe, what preconditions the caller must satisfy, and what pointer validity guarantees are relied upon.

**Context:** The runtime has ~80 unsafe blocks across ~2500 lines of raw pointer arithmetic with essentially zero safety documentation. This is the highest-priority EXPOSURE finding — a reader cannot verify correctness without understanding why each `unsafe` block is sound.

**Depends on:** Section 01 (COW code is reorganized before documenting it — SAFETY comments should reference the canonical functions, not the duplicated inline code).

---

## 09.1 COW List Files

**File(s):**
- `compiler/ori_rt/src/list/cow.rs` — after Section 01 reorganization
- `compiler/ori_rt/src/list/cow_structural.rs` — 14 unsafe blocks
- `compiler/ori_rt/src/list/cow_sort/mod.rs` — 20 unsafe blocks
- `compiler/ori_rt/src/list/cow_sort/sort.rs` — check for unsafe blocks

For each unsafe block, document:
- **SAFETY: why this is sound** — what invariants are relied upon
- **Preconditions** — what must be true about the arguments (non-null, aligned, within bounds, RC-allocated)
- **Pointer validity** — why the pointer dereference is valid (e.g., "data was returned by ori_rc_alloc, which guarantees 8-byte alignment and header reservation")

- [ ] Add `// SAFETY:` to all unsafe blocks in `cow.rs` (post-reorganization)
- [ ] Add `// SAFETY:` to all 14 unsafe blocks in `cow_structural.rs`
- [ ] Add `// SAFETY:` to all 20 unsafe blocks in `cow_sort/mod.rs`
- [ ] Check `cow_sort/sort.rs` for undocumented unsafe blocks

---

## 09.2 COW Map/Set Files

**File(s):**
- `compiler/ori_rt/src/map/cow.rs` — 11 unsafe blocks
- `compiler/ori_rt/src/set/cow/basic.rs` — 5 unsafe blocks
- `compiler/ori_rt/src/set/cow/algebra.rs` — 6 unsafe blocks

- [ ] Add `// SAFETY:` to all 11 unsafe blocks in `map/cow.rs`
- [ ] Add `// SAFETY:` to all 5 unsafe blocks in `set/cow/basic.rs`
- [ ] Add `// SAFETY:` to all 6 unsafe blocks in `set/cow/algebra.rs`

---

## 09.3 Iterator Files

**File(s):**
- `compiler/ori_rt/src/iterator/consumers.rs` — 23 unsafe blocks
- `compiler/ori_rt/src/iterator/adapters.rs` — 2 unsafe blocks

- [ ] Add `// SAFETY:` to all 23 unsafe blocks in `consumers.rs`
- [ ] Add `// SAFETY:` to all 2 unsafe blocks in `adapters.rs`

---

## 09.R Third Party Review Findings

- None.

---

## 09.N Completion Checklist

- [ ] Every `unsafe` block in ori_rt COW files has a `// SAFETY:` comment
- [ ] Every `unsafe` block in ori_rt iterator files has a `// SAFETY:` comment
- [ ] `grep -rn "unsafe {" compiler/ori_rt/src/ | grep -v "// SAFETY" | grep -v test | grep -v "extern"` returns 0 matches (excluding extern "C" function signatures)
- [ ] Comments accurately describe invariants (reviewed, not boilerplate)
- [ ] `timeout 150 ./test-all.sh` passes (zero behavioral changes)
- [ ] `/tpr-review` covering Section 09
- [ ] `/impl-hygiene-review last commit`
