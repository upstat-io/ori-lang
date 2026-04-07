---
section: "04"
title: "TBAA, Range, and Invariant Metadata"
status: not-started
reviewed: false
goal: "Emit TBAA on struct field accesses, !range on bounded returns, and !invariant.load on immutable borrowed params"
success_criteria:
  - "Struct field loads have !tbaa metadata distinguishing field types"
  - "Ordering returns have !range !{i64 0, i64 3} (Less=0, Equal=1, Greater=2)"
  - "Bool returns have !range !{i64 0, i64 2}"
  - "Enum tag loads have !range !{i64 0, i64 N} where N = variant count"
  - "Borrowed parameter loads have !invariant.load when the parameter is not written through"
  - "Satisfies mission criterion: LLVM IR contains !tbaa, !range, !invariant.load metadata"
inspired_by:
  - "LLVM InstCombine uses !range to fold comparisons"
  - "LLVM BasicAA uses !tbaa for field-level alias disambiguation"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "TBAA Metadata on Struct Fields"
    status: not-started
  - id: "04.2"
    title: "Range Metadata on Bounded Returns"
    status: not-started
  - id: "04.3"
    title: "Invariant.load on Borrowed Params"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: TBAA, Range, and Invariant Metadata

**Status:** Not Started
**Goal:** Emit three kinds of LLVM metadata that enable better optimization of Ori code: TBAA (type-based alias analysis) on struct field accesses allows LLVM to prove loads through different field pointers don't alias; `!range` on bounded integer loads allows LLVM to fold comparisons and narrow value tracking; `!invariant.load` on immutable parameters allows LLVM to eliminate redundant loads.

**Success Criteria:**

- [ ] `ORI_DUMP_AFTER_LLVM=1` shows `!tbaa` on struct field loads/stores
- [ ] `ORI_DUMP_AFTER_LLVM=1` shows `!range !{i64 0, i64 3}` on `Ordering` returns
- [ ] `ORI_DUMP_AFTER_LLVM=1` shows `!range !{i64 0, i64 2}` on `bool` loads
- [ ] `ORI_DUMP_AFTER_LLVM=1` shows `!invariant.load` on loads from borrowed params
- [ ] All metadata is sound — no miscompilations from incorrect alias/range/invariant assumptions

**Context:** LLVM's optimizer is metadata-hungry. Without `!tbaa`, it conservatively assumes any pointer load might alias any other pointer store. Without `!range`, it cannot narrow value tracking on enum tags or comparison results. Without `!invariant.load`, it must reload values even when they're known immutable. Ori has the semantic information to populate all three — the type system knows field types, the ARC analysis knows borrowing, and the type pool knows bounded integer ranges.

**Depends on:** Section 01 (instr_dispatch.rs must be split before adding TBAA emission code — currently 587 lines) and Section 02 (IrBuilder metadata helpers, ArcInstr::Project struct_ty).

---

## 04.1 TBAA Metadata on Struct Fields

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs`, `compiler/ori_llvm/src/codegen/ir_builder/metadata.rs`

At every `struct_gep` → `load` or `struct_gep` → `store` sequence emitted for `ArcInstr::Project`, attach a TBAA metadata node that identifies the struct type and field index. LLVM's BasicAA uses this to prove that loads from different struct fields cannot alias.

- [ ] Build TBAA type hierarchy: root node → per-struct-type node → per-field node
  - Root: `!{!"Ori TBAA", null}`
  - Struct: `!{!"StructName", !root}`
  - Field: `!{!"StructName.fieldN", !struct_node, i64 0}` (0 = may alias same-type accesses)
- [ ] Cache TBAA nodes per (struct_type, field_index) in a `FxHashMap` on `ArcIrEmitter` — avoid creating duplicate nodes
- [ ] Write failing AOT test FIRST (TDD): program with two struct fields loaded in sequence → before TBAA, LLVM cannot prove the loads are independent. This test will verify reordering after TBAA metadata is emitted.
- [ ] In `instr_dispatch.rs` where `ArcInstr::Project` is emitted: after the `struct_gep` + `load`, call `builder.set_load_tbaa(load_val, tbaa_field_node)`
- [ ] Use `Project.struct_ty` (added in Section 02.3) to look up the struct name from `TypeInfoStore`
- [ ] Verify AOT test now passes: LLVM can reorder independent struct field loads
- [ ] **Soundness guard**: Do NOT emit TBAA for enum payload accesses (enum payloads share the same memory location across variants). Only emit for struct fields where layout guarantees non-overlapping.

**Matrix dimensions:**
- Types: user struct with 2+ fields, nested struct, struct with mixed scalar/ref fields
- Patterns: load field A then field B (should be reorderable), load field A then store field A (should be dependent), load from two different structs (should be independent)
- Semantic pin: IR dump showing `!tbaa` on struct field loads
<!-- resolves: plans/roadmap/section-21A-llvm.md line 859 (Tier 3 metadata) -->

---

## 04.2 Range Metadata on Bounded Returns

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/operators/mod.rs`

Attach `!range` metadata to loads of values with known bounded ranges. Three main sources:

1. **Ordering values**: `compare()` returns `Ordering` (Less=0, Equal=1, Greater=2) → range [0, 3)
2. **Bool values**: i1 extended to i64 → range [0, 2)
3. **Enum tags**: tag load for an enum with N variants → range [0, N)

- [ ] At Ordering comparison result sites (after `emit_comparison_via_trait` or `emit_element_compare`): attach `!range !{i64 0, i64 3}` to the result load
- [ ] At bool-to-i64 extension sites: attach `!range !{i64 0, i64 2}` (LLVM may already know this for i1, but explicit range on i64 helps after extension)
- [ ] At enum tag load sites (`instr_dispatch.rs` where tag is extracted via `struct_gep` index 0): compute variant count from `TypeInfoStore`, attach `!range !{i64 0, i64 N}`
- [ ] Create helper: `fn bounded_range_for_type(&self, ty: Idx) -> Option<(i64, i64)>` in `ArcIrEmitter` — returns range bounds for types with known value ranges
- [ ] Write failing AOT test FIRST (TDD): match on a 3-variant enum → before range metadata, IR has a default branch. This test verifies LLVM eliminates it after range metadata.
- [ ] Verify AOT test now passes: switch with exact 3 cases, no unreachable default

**Matrix dimensions:**
- Types: Ordering (3 values), bool (2 values), 2-variant enum, 5-variant enum, Option (2 variants)
- Patterns: comparison followed by branch (range enables branch folding), enum match (range enables unreachable elimination)
- Semantic pin: IR dump showing `!range` metadata on tag loads

---

## 04.3 Invariant.load on Borrowed Params

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs` or equivalent param setup

When a function parameter is borrowed (not owned), the callee guarantees it won't modify the value. Loads from borrowed parameters are invariant — they can be hoisted, CSE'd, and eliminated without concern for mutation.

- [ ] During function emission setup, identify borrowed parameters from `ArcFunction.params` (each `ArcParam` has `ownership: Ownership`)
- [ ] Track borrowed parameter `ValueId`s in a set on `ArcIrEmitter`
- [ ] When emitting a `load` from a location that traces back to a borrowed parameter, attach `!invariant.load` metadata
- [ ] **Soundness guard**: Only attach `!invariant.load` if the parameter is truly immutable. Borrowed params in Ori's ARC model are read-only by the callee — this is guaranteed by the borrow inference (`ori_arc/src/borrow/`). However, if a borrowed param's interior is mutated via COW (e.g., the caller has a unique ref that the callee projects into), `!invariant.load` would be unsound. Restrict to parameters where `ParamContract.may_share == false` (the param is exclusively borrowed, no interior mutation possible).
- [ ] Write failing AOT test FIRST (TDD): function that loads a borrowed struct field twice → before invariant.load, IR shows two loads. This test verifies CSE after invariant metadata.
- [ ] Verify AOT test now passes: IR shows single load (second load CSE'd away)

**Matrix dimensions:**
- Types: borrowed `int` param, borrowed `str` param, borrowed struct param, borrowed `[int]` param
- Patterns: single load (baseline), double load of same field (should CSE), load before and after non-modifying call (should CSE if callee is pure)
- Semantic pin: IR dump showing `!invariant.load` on borrowed param loads

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] TBAA metadata emitted on struct field loads/stores
- [ ] Range metadata emitted on Ordering, bool, and enum tag loads
- [ ] Invariant.load metadata emitted on borrowed parameter loads
- [ ] All metadata is sound (no miscompilations)
- [ ] `timeout 150 ./test-all.sh` green (debug AND release — `cargo b --release && timeout 150 ./test-all.sh`)
- [ ] Dual-exec parity: `diagnostics/dual-exec-verify.sh tests/spec/` passes (metadata is hints only, must not change observable behavior)
- [ ] No spurious warnings
- [ ] Plan annotation cleanup
- [ ] **Plan sync** — update plan metadata
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `ORI_DUMP_AFTER_LLVM=1` shows TBAA, range, and invariant.load metadata on appropriate instructions. LLVM's optimizer produces better code (observable via fewer loads, better branch elimination). Zero test regressions.
