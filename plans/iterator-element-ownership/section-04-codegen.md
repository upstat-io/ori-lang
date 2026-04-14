---
section: "04"
title: "Codegen: Parameter Plumbing"
status: not-started
reviewed: false
goal: "LLVM codegen passes elem_inc_fn and elem_dec_fn to all runtime functions that need them"
success_criteria:
  - "emit_iter_from_list passes elem_inc_fn to ori_iter_from_list"
  - "All 11 consumer emit functions pass elem_dec_fn"
  - "emit_iter_filter and emit_iter_skip pass elem_dec_fn"
  - "collect/collect_set codegen no longer passes elem_inc_fn"
  - "Runtime function declarations match updated C signatures"
sections:
  - id: "04.1"
    title: "Runtime function declarations"
    status: not-started
  - id: "04.2"
    title: "Source emission: emit_iter_from_list"
    status: not-started
  - id: "04.3"
    title: "Adapter emission: filter, skip"
    status: not-started
  - id: "04.4"
    title: "Consumer emission: all 11 functions"
    status: not-started
depends_on: ["01", "02", "03"]
third_party_review:
  status: none
  updated: null
---

# 04 Codegen: Parameter Plumbing

This section wires the LLVM codegen to pass `elem_inc_fn` and `elem_dec_fn` to the updated runtime functions. The infrastructure already exists — `get_or_generate_elem_inc_fn()` and `get_or_generate_elem_dec_fn()` in `element_fn_gen.rs` are used by `collect` and `collect_set`. This section extends their use to ALL consumer and adapter calls.

## 04.1 Runtime Function Declarations

**File:** `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

Every C-ABI runtime function is declared in this file. Update signatures to match the new parameters.

- [ ] Update `ori_iter_from_list` declaration: add `elem_inc_fn: ptr` parameter
- [ ] Update `ori_iter_filter` declaration: add `elem_dec_fn: ptr` parameter
- [ ] Update `ori_iter_skip` declaration: add `elem_dec_fn: ptr` parameter
- [ ] Update all 11 consumer declarations: add `elem_dec_fn: ptr` parameter (count, any, all, find, for_each, fold, last, rfind, rfold, join). Remove `elem_inc_fn` from collect. For collect_set: remove `elem_inc_fn`, add `elem_dec_fn`.
- [ ] Verify parameter counts match between declaration and call site (mismatch = LLVM verification error)

- [ ] **Subsection close-out (04.1)** — MANDATORY before starting 04.2:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.2 Source Emission: emit_iter_from_list

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs`

- [ ] In `emit_iter_from_list`: generate `elem_inc_fn` via `get_or_generate_elem_inc_fn(elem_ty)` and pass as additional argument to `ori_iter_from_list`
- [ ] For map/option source emitters: pass `elem_inc_fn` if their signatures were extended in Section 01.4
- [ ] Verify range emitter unchanged (int elements, no inc needed)

- [ ] **Subsection close-out (04.2)** — MANDATORY before starting 04.3:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.3 Adapter Emission: filter, skip

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs`

- [ ] In `emit_iter_filter`: generate `elem_dec_fn` via `get_or_generate_elem_dec_fn(elem_ty)` and pass as additional argument to `ori_iter_filter`
- [ ] In `emit_iter_skip`: same — pass `elem_dec_fn`
- [ ] For cycle: if `elem_inc_fn` was added to its constructor (Section 02.3), pass it here
- [ ] Verify other adapter emitters (take, chain, enumerate, zip, flatten, flat_map) don't need changes — their constructors didn't gain new parameters

- [ ] **Subsection close-out (04.3)** — MANDATORY before starting 04.4:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.4 Consumer Emission: All 11 Functions

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator_consumers.rs`

For each consumer emission function, generate `elem_dec_fn` and add it to the call.

- [ ] `emit_iter_collect`: **Remove** `elem_inc_fn` from the call. Elements are already owned. Keep the post-call `elem_dec_fn` storage in the buffer header (unchanged).
- [ ] `emit_iter_collect_set`: **Remove** `elem_inc_fn`. Add `elem_dec_fn` to the call (for duplicate cleanup).
- [ ] `emit_iter_count`: Add `elem_dec_fn` to call
- [ ] `emit_iter_any`: Add `elem_dec_fn` to call
- [ ] `emit_iter_all`: Add `elem_dec_fn` to call
- [ ] `emit_iter_find`: Add `elem_dec_fn` to call
- [ ] `emit_iter_for_each`: Add `elem_dec_fn` to call
- [ ] `emit_iter_fold`: Add `elem_dec_fn` to call
- [ ] `emit_iter_last`: Add `elem_dec_fn` to call
- [ ] `emit_iter_rfind`: Add `elem_dec_fn` to call
- [ ] `emit_iter_rfold`: Add `elem_dec_fn` to call
- [ ] `emit_iter_join`: Add `elem_dec_fn` to call
- [ ] **TDD:** Compile and run existing AOT iterator tests — all should pass with zero leaks
- [ ] Verify LLVM IR verification passes on all emitted consumer calls (`fn_val.verify(true)`)

- [ ] **Subsection close-out (04.4)** — MANDATORY before starting Section 05:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All runtime function declarations match updated C signatures
- [ ] All source/adapter/consumer emitters pass correct elem_inc/dec_fn
- [ ] LLVM IR verification passes on all emitted functions
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `/commit-push`
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` section-close sweep
