---
section: "09"
title: "SAFETY Comments for ori_rt"
status: not-started
reviewed: false
goal: "Add SAFETY comments to all ~512 unsafe blocks in ori_rt production code — document invariants, preconditions, and pointer validity"
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
  - id: "09.4"
    title: "String, RC, Format, and Top-Level Files"
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

**Context:** The runtime has ~512 unsafe blocks in production code (excluding tests) across all modules: list/, map/, set/, string/, iterator/, rc/, format/, lib.rs, and more. The original estimate of ~80 only counted a subset of COW and iterator files. This is the highest-priority EXPOSURE finding — a reader cannot verify correctness without understanding why each `unsafe` block is sound. <!-- reviewed: accuracy fix — 512 unsafe blocks, not ~80; scope expanded to all ori_rt production code -->

**Depends on:** Section 01 (COW code is reorganized before documenting it — SAFETY comments should reference the canonical functions, not the duplicated inline code).

---

## 09.1 COW List Files

**File(s):**
- `compiler/ori_rt/src/list/cow.rs` — after Section 01 reorganization
- `compiler/ori_rt/src/list/cow_structural.rs` — ~16 unsafe blocks <!-- reviewed: accuracy fix -->
- `compiler/ori_rt/src/list/cow_sort/mod.rs` — ~20 unsafe blocks
- `compiler/ori_rt/src/list/cow_sort/sort.rs` — check for unsafe blocks

For each unsafe block, document:
- **SAFETY: why this is sound** — what invariants are relied upon
- **Preconditions** — what must be true about the arguments (non-null, aligned, within bounds, RC-allocated)
- **Pointer validity** — why the pointer dereference is valid (e.g., "data was returned by ori_rc_alloc, which guarantees 8-byte alignment and header reservation")

- [ ] Add `// SAFETY:` to all unsafe blocks in `cow.rs` (~19 blocks, post-reorganization)
- [ ] Add `// SAFETY:` to all ~16 unsafe blocks in `cow_structural.rs` <!-- reviewed: accuracy fix — 16 not 14 -->
- [ ] Add `// SAFETY:` to all ~20 unsafe blocks in `cow_sort/mod.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `cow_sort/sort.rs`
- [ ] Add `// SAFETY:` to all ~17 unsafe blocks in `list/mod.rs` <!-- reviewed: accuracy fix — missing file -->
- [ ] Add `// SAFETY:` to all ~19 unsafe blocks in `list/query.rs` <!-- reviewed: accuracy fix — missing file -->

---

## 09.2 COW Map/Set Files

**File(s):**
- `compiler/ori_rt/src/map/cow.rs` — ~32 unsafe blocks <!-- reviewed: accuracy fix -->
- `compiler/ori_rt/src/map/mod.rs` — ~29 unsafe blocks <!-- reviewed: accuracy fix — missing from original -->
- `compiler/ori_rt/src/set/cow/basic.rs` — ~23 unsafe blocks <!-- reviewed: accuracy fix -->
- `compiler/ori_rt/src/set/cow/algebra.rs` — ~46 unsafe blocks <!-- reviewed: accuracy fix -->
- `compiler/ori_rt/src/set/mod.rs` — ~12 unsafe blocks <!-- reviewed: accuracy fix — missing from original -->

- [ ] Add `// SAFETY:` to all ~32 unsafe blocks in `map/cow.rs` <!-- reviewed: accuracy fix — 32 not 11 -->
- [ ] Add `// SAFETY:` to all ~29 unsafe blocks in `map/mod.rs` <!-- reviewed: accuracy fix — missing file -->
- [ ] Add `// SAFETY:` to all ~23 unsafe blocks in `set/cow/basic.rs` <!-- reviewed: accuracy fix — 23 not 5 -->
- [ ] Add `// SAFETY:` to all ~46 unsafe blocks in `set/cow/algebra.rs` <!-- reviewed: accuracy fix — 46 not 6 -->
- [ ] Add `// SAFETY:` to all ~12 unsafe blocks in `set/mod.rs` <!-- reviewed: accuracy fix — missing file -->

---

## 09.3 Iterator Files

**File(s):**
- `compiler/ori_rt/src/iterator/consumers.rs` — ~76 unsafe blocks <!-- reviewed: accuracy fix -->
- `compiler/ori_rt/src/iterator/adapters.rs` — ~14 unsafe blocks <!-- reviewed: accuracy fix -->

- [ ] Add `// SAFETY:` to all ~76 unsafe blocks in `consumers.rs` <!-- reviewed: accuracy fix — 76 not 23 -->
- [ ] Add `// SAFETY:` to all ~14 unsafe blocks in `adapters.rs` <!-- reviewed: accuracy fix — 14 not 2 -->

---

## 09.4 String, RC, Format, and Top-Level Files
<!-- reviewed: feasibility fix — these files were entirely missing from the plan despite containing ~170 unsafe blocks -->

**File(s):**
- `compiler/ori_rt/src/string/methods/mod.rs` — ~39 unsafe blocks
- `compiler/ori_rt/src/string/ops.rs` — ~22 unsafe blocks
- `compiler/ori_rt/src/string/mod.rs` — ~10 unsafe blocks
- `compiler/ori_rt/src/rc/list_rc.rs` — ~14 unsafe blocks
- `compiler/ori_rt/src/format/mod.rs` — check for unsafe blocks
- `compiler/ori_rt/src/lib.rs` — ~13 unsafe blocks

- [ ] Add `// SAFETY:` to all unsafe blocks in `string/methods/mod.rs`
- [ ] Add `// SAFETY:` to all unsafe blocks in `string/ops.rs`
- [ ] Add `// SAFETY:` to all unsafe blocks in `string/mod.rs`
- [ ] Add `// SAFETY:` to all unsafe blocks in `rc/list_rc.rs`
- [ ] Add `// SAFETY:` to all unsafe blocks in `format/` files
- [ ] Add `// SAFETY:` to all unsafe blocks in `lib.rs`
- [ ] Sweep remaining ori_rt production files for any missed unsafe blocks

---

## 09.R Third Party Review Findings

- None.

---

## 09.N Completion Checklist

- [ ] Every `unsafe` block in ori_rt COW files has a `// SAFETY:` comment
- [ ] Every `unsafe` block in ori_rt iterator files has a `// SAFETY:` comment
- [ ] Every `unsafe` block in ori_rt string, rc, format, and top-level files has a `// SAFETY:` comment <!-- reviewed: feasibility fix -->
- [ ] `grep -rn "unsafe {" compiler/ori_rt/src/ | grep -v "// SAFETY" | grep -v test | grep -v "extern"` returns 0 matches (excluding extern "C" function signatures)
- [ ] Comments accurately describe invariants (reviewed, not boilerplate)
- [ ] `timeout 150 ./test-all.sh` passes (zero behavioral changes)
- [ ] `/tpr-review` covering Section 09
- [ ] `/impl-hygiene-review last commit`
