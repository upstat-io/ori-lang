---
section: "09"
title: "SAFETY Comments for ori_rt"
status: not-started
reviewed: false
goal: "Add SAFETY comments to all ~510 unsafe blocks in ori_rt production code across 35 files — document invariants, preconditions, and pointer validity" <!-- reviewed: cohesion fix — 35 files with unsafe blocks, covering ~510 blocks total -->
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

**Context:** The runtime has ~512 unsafe blocks in production code (excluding tests) across all modules. Of these, ~223 already have `// SAFETY:` comments. The remaining ~289 blocks across ~17 files need documentation. This is the highest-priority EXPOSURE finding — a reader cannot verify correctness without understanding why each `unsafe` block is sound. <!-- reviewed: executability/hygiene fix — 512 total, 223 already documented, 289 remaining; verified by script -->

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

**Note:** Counts below are blocks STILL NEEDING `// SAFETY:` comments (not total unsafe blocks). Already-documented blocks are excluded. Verified 2026-04-03 via script. <!-- reviewed: executability/hygiene fix — counts now reflect remaining work, not total -->

- [ ] Add `// SAFETY:` to ~16 undocumented unsafe blocks in `cow_structural.rs`
- [ ] Add `// SAFETY:` to ~20 undocumented unsafe blocks in `cow_sort/mod.rs`
- [ ] Add `// SAFETY:` to ~9 undocumented unsafe blocks in `cow_sort/sort.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `cow.rs` (post-reorganization from Section 01; re-count after reorganization)
- [ ] Add `// SAFETY:` to ~19 undocumented unsafe blocks in `list/query.rs`
- [ ] Add `// SAFETY:` to ~9 undocumented unsafe blocks in `list/slice.rs`
- [ ] Add `// SAFETY:` to ~5 undocumented unsafe blocks in `list/reset/mod.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `list/mod.rs` (re-count — some may already have SAFETY)

---

## 09.2 COW Map/Set Files

**File(s):**
- `compiler/ori_rt/src/map/cow.rs` — ~32 unsafe blocks <!-- reviewed: accuracy fix -->
- `compiler/ori_rt/src/map/mod.rs` — ~29 unsafe blocks <!-- reviewed: accuracy fix — missing from original -->
- `compiler/ori_rt/src/set/cow/basic.rs` — ~23 unsafe blocks <!-- reviewed: accuracy fix -->
- `compiler/ori_rt/src/set/cow/algebra.rs` — ~46 unsafe blocks <!-- reviewed: accuracy fix -->
- `compiler/ori_rt/src/set/mod.rs` — ~12 unsafe blocks <!-- reviewed: accuracy fix — missing from original -->

- [ ] Add `// SAFETY:` to ~32 undocumented unsafe blocks in `map/cow.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `map/mod.rs` (re-count — some may already have SAFETY) <!-- reviewed: executability/hygiene fix -->
- [ ] Add `// SAFETY:` to ~23 undocumented unsafe blocks in `set/cow/basic.rs`
- [ ] Add `// SAFETY:` to ~46 undocumented unsafe blocks in `set/cow/algebra.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `set/mod.rs` (re-count) <!-- reviewed: executability/hygiene fix -->
- [ ] Add `// SAFETY:` to ~2 undocumented unsafe blocks in `set/cow/mod.rs` <!-- reviewed: executability/hygiene fix -->

---

## 09.3 Iterator Files

**File(s):**
- `compiler/ori_rt/src/iterator/consumers.rs` — ~76 unsafe blocks <!-- reviewed: accuracy fix -->
- `compiler/ori_rt/src/iterator/adapters.rs` — ~14 unsafe blocks <!-- reviewed: accuracy fix -->

- [ ] Add `// SAFETY:` to ~76 undocumented unsafe blocks in `consumers.rs` (this is the single largest file — consider batching by function) <!-- reviewed: executability/hygiene fix -->
- [ ] Add `// SAFETY:` to ~14 undocumented unsafe blocks in `adapters.rs`
- [ ] Add `// SAFETY:` to ~3 undocumented unsafe blocks in `iterator/mod.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `iterator/sources.rs` (re-count — some may already have SAFETY) <!-- reviewed: executability/hygiene fix -->

---

## 09.4 String, RC, Format, IO, and Top-Level Files
<!-- reviewed: feasibility fix — these files were entirely missing from the plan despite containing ~170 unsafe blocks -->
<!-- reviewed: cohesion fix — expanded to cover ALL remaining ori_rt files with unsafe blocks -->

**File(s):**
- `compiler/ori_rt/src/string/methods/mod.rs` — ~39 unsafe blocks
- `compiler/ori_rt/src/string/ops.rs` — ~22 unsafe blocks
- `compiler/ori_rt/src/string/mod.rs` — ~10 unsafe blocks
- `compiler/ori_rt/src/string/convert.rs` — ~5 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/rc/list_rc.rs` — ~14 unsafe blocks
- `compiler/ori_rt/src/rc/mod.rs` — ~8 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/rc/allocate.rs` — ~8 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/rc/set_rc.rs` — ~6 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/rc/map_rc.rs` — ~4 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/rc/elem_header.rs` — ~2 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/rc/debug.rs` — ~1 unsafe block <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/io/mod.rs` — ~10 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/io/jit_recovery.rs` — ~5 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/io/panic_state.rs` — ~1 unsafe block <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/list/slice.rs` — ~9 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/list/reset/mod.rs` — ~5 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/iterator/sources.rs` — ~5 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/iterator/mod.rs` — ~3 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/set/cow/mod.rs` — ~2 unsafe blocks <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/slice_encoding/mod.rs` — ~1 unsafe block <!-- reviewed: cohesion fix — missing file -->
- `compiler/ori_rt/src/format/mod.rs` — check for unsafe blocks
- `compiler/ori_rt/src/lib.rs` — ~13 unsafe blocks

**Note:** Many string, rc, io, and lib.rs files already have SAFETY comments on most or all blocks. The following checklist covers files with REMAINING undocumented blocks. Run `grep -B3 "unsafe {" <file> | grep -v SAFETY` per-file to identify exactly which blocks need work. <!-- reviewed: executability/hygiene fix — added per-file verification approach -->

**String files:**
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `string/methods/mod.rs` (audit — some may already have SAFETY)
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `string/ops.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `string/mod.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `string/convert.rs`

**RC files:**
- [ ] Add `// SAFETY:` to ~2 undocumented unsafe blocks in `rc/mod.rs`
- [ ] Add `// SAFETY:` to ~1 undocumented unsafe block in `rc/allocate.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `rc/list_rc.rs` (audit)
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `rc/set_rc.rs` (audit)
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `rc/map_rc.rs` (audit)
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `rc/elem_header.rs` (audit)
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `rc/debug.rs` (audit)

**IO files:**
- [ ] Add `// SAFETY:` to ~5 undocumented unsafe blocks in `io/jit_recovery.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `io/mod.rs` (audit)
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `io/panic_state.rs` (audit)

**Format and top-level:**
- [ ] Add `// SAFETY:` to ~7 undocumented unsafe blocks in `format/mod.rs`
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `lib.rs` (audit)
- [ ] Add `// SAFETY:` to undocumented unsafe blocks in `slice_encoding/mod.rs` (audit)

---

## 09.R Third Party Review Findings

- None.

---

## 09.N Completion Checklist

- [ ] Every `unsafe` block in ori_rt COW files (list/cow*, map/cow*, set/cow*) has a `// SAFETY:` comment
- [ ] Every `unsafe` block in ori_rt list files (mod, query, slice, reset) has a `// SAFETY:` comment <!-- reviewed: cohesion fix -->
- [ ] Every `unsafe` block in ori_rt iterator files (consumers, adapters, sources, mod) has a `// SAFETY:` comment <!-- reviewed: cohesion fix -->
- [ ] Every `unsafe` block in ori_rt string files (methods, ops, mod, convert) has a `// SAFETY:` comment <!-- reviewed: cohesion fix -->
- [ ] Every `unsafe` block in ori_rt rc files (list_rc, mod, allocate, set_rc, map_rc, elem_header, debug) has a `// SAFETY:` comment <!-- reviewed: cohesion fix -->
- [ ] Every `unsafe` block in ori_rt io files (mod, jit_recovery, panic_state) has a `// SAFETY:` comment <!-- reviewed: cohesion fix -->
- [ ] Every `unsafe` block in ori_rt format, set/mod, map/mod, slice_encoding, and lib.rs has a `// SAFETY:` comment <!-- reviewed: cohesion fix -->
- [ ] Verification script: count undocumented unsafe blocks with context (check 3 preceding lines for SAFETY):
  ```
  python3 -c "
  import os
  for root, _, files in os.walk('compiler/ori_rt/src'):
    for f in files:
      if not f.endswith('.rs') or 'test' in f.lower(): continue
      path = os.path.join(root, f)
      with open(path) as fh: lines = fh.readlines()
      for i, line in enumerate(lines):
        if 'unsafe {' in line:
          if 'SAFETY' not in ''.join(lines[max(0,i-3):i+1]):
            print(f'{path}:{i+1}')
  "
  ```
  Output must be empty (zero undocumented blocks). <!-- reviewed: executability/hygiene fix — more precise than grep -B1 -->
- [ ] Comments accurately describe invariants (reviewed, not boilerplate) -- spot-check at least 10 SAFETY comments for quality

## 09.T Test Strategy

This section adds only comments with zero code changes. The test strategy is verification-only.

- [ ] Verify `timeout 150 ./test-all.sh` passes after each batch of SAFETY comment additions (per-file or per-sub-section)
- [ ] Verify `./clippy-all.sh` clean (no warnings introduced by comment changes)
- [ ] Run the Python verification script from 09.N to confirm zero undocumented unsafe blocks remain
- [ ] Quality spot-check: review at least 10 SAFETY comments across different files to verify they describe actual invariants, not boilerplate like "SAFETY: this is safe"

---

- [ ] `timeout 150 ./test-all.sh` passes (zero behavioral changes)
- [ ] `/tpr-review` covering Section 09
- [ ] `/impl-hygiene-review last commit`
