---
section: "02"
title: "LLVM Metadata Infrastructure"
status: not-started
reviewed: false
goal: "Build the metadata emission layer so Sections 03-04 can attach TBAA, range, and alias metadata to LLVM instructions"
success_criteria:
  - "IrBuilder has methods to create and attach TBAA, range, and invariant.load metadata nodes"
  - "ArcInstr::Project carries the containing struct's type ID (not just the result type)"
  - "A smoke test attaches TBAA metadata to a struct field load and verifies it appears in LLVM IR dump"
  - "Satisfies mission criterion: LLVM IR contains metadata (verified by ORI_DUMP_AFTER_LLVM=1)"
inspired_by:
  - "LLVM C API metadata functions (LLVMSetMetadata, LLVMMDNode, LLVMMDString)"
  - "Rust llvm-sys crate (already used in ori_llvm for pass builder and debug info)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "llvm-sys Metadata Wrapper"
    status: not-started
  - id: "02.2"
    title: "IrBuilder Metadata Helpers"
    status: not-started
  - id: "02.3"
    title: "Extend ArcInstr::Project with Struct Type"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: LLVM Metadata Infrastructure

**Status:** Not Started
**Goal:** Build the metadata emission layer that Sections 03 (AIMS export) and 04 (TBAA/range) need to attach optimization-relevant metadata to LLVM instructions. Currently, `ori_llvm` emits LLVM attributes extensively (noalias, readonly, nonnull, dereferenceable, memory()) but has zero metadata emission — no `!tbaa`, no `!range`, no `!invariant.load`. inkwell 0.8 may not expose `MDBuilder` directly, but `llvm-sys` 211 (already a dependency) provides the raw C API.

**Success Criteria:**

- [ ] `IrBuilder` has `set_tbaa_metadata`, `set_range_metadata`, `set_invariant_load_metadata` methods
- [ ] Metadata nodes are constructed via llvm-sys FFI (or inkwell if supported)
- [ ] `ArcInstr::Project` carries `struct_ty: Idx` in addition to existing `ty: Idx` (result type)
- [ ] A smoke test constructs TBAA metadata, attaches it to a load, and `ORI_DUMP_AFTER_LLVM=1` shows `!tbaa` in the output
- [ ] `./test-all.sh` green — metadata attachment does not affect existing behavior

**Context:** LLVM's optimizer uses metadata to make better decisions. `!tbaa` tells the alias analysis that loads/stores through different struct field types cannot alias. `!range` tells value tracking that a loaded integer is within a known range. `!invariant.load` tells the optimizer that a loaded value never changes. All three are attached to individual `load`/`store` instructions via `LLVMSetMetadata()`. The infrastructure needed is: (1) a way to create metadata nodes (MDNode/MDString), (2) a way to attach them to instructions, and (3) type information available at the attachment site.

**Reference implementations:**
- **LLVM C API** `llvm-c/Core.h`: `LLVMSetMetadata(Val, KindID, Node)`, `LLVMMDString()`, `LLVMMDNode()`
- **Rust llvm-sys** `core.rs`: Already used in `ori_llvm` for `LLVMRunPasses`, `LLVMPassBuilderOptionsCreate`, debug info builders

**Depends on:** Nothing (can start immediately).

---

## 02.1 llvm-sys Metadata Wrapper

**File(s):** `compiler/ori_llvm/src/codegen/ir_builder/metadata.rs` (NEW)

Create a thin Rust wrapper around llvm-sys metadata functions. This isolates unsafe FFI in one module.

- [ ] Create `compiler/ori_llvm/src/codegen/ir_builder/metadata.rs`
- [ ] Implement core metadata construction functions:
  ```rust
  use llvm_sys::core::*;
  use llvm_sys::prelude::*;

  /// Metadata kind IDs (cached from LLVMGetMDKindID)
  pub struct MetadataKinds {
      pub tbaa: u32,
      pub range: u32,
      pub invariant_load: u32,
      pub alias_scope: u32,
      pub noalias: u32,
  }

  impl MetadataKinds {
      pub fn new(context: LLVMContextRef) -> Self {
          unsafe {
              Self {
                  tbaa: LLVMGetMDKindIDInContext(context, b"tbaa\0".as_ptr().cast(), 4),
                  range: LLVMGetMDKindIDInContext(context, b"range\0".as_ptr().cast(), 5),
                  invariant_load: LLVMGetMDKindIDInContext(context, b"invariant.load\0".as_ptr().cast(), 14),
                  alias_scope: LLVMGetMDKindIDInContext(context, b"alias.scope\0".as_ptr().cast(), 11),
                  noalias: LLVMGetMDKindIDInContext(context, b"noalias\0".as_ptr().cast(), 7),
              }
          }
      }
  }

  /// Attach metadata to an LLVM instruction.
  pub unsafe fn set_metadata(instr: LLVMValueRef, kind_id: u32, md_node: LLVMValueRef) {
      LLVMSetMetadata(instr, kind_id, md_node);
  }

  /// Create an empty metadata node (for !invariant.load).
  pub unsafe fn empty_md_node(context: LLVMContextRef) -> LLVMValueRef {
      LLVMMDNodeInContext(context, std::ptr::null_mut(), 0)
  }

  /// Create a range metadata node: !{i64 lo, i64 hi} (value in [lo, hi)).
  pub unsafe fn range_md_node(context: LLVMContextRef, lo: i64, hi: i64) -> LLVMValueRef {
      let i64_ty = LLVMInt64TypeInContext(context);
      let lo_val = LLVMConstInt(i64_ty, lo as u64, 1);
      let hi_val = LLVMConstInt(i64_ty, hi as u64, 1);
      let mut vals = [lo_val, hi_val];
      LLVMMDNodeInContext(context, vals.as_mut_ptr(), 2)
  }
  ```
- [ ] Add `mod metadata;` to `compiler/ori_llvm/src/codegen/ir_builder/mod.rs`
- [ ] Add TBAA root and struct-field node constructors (TBAA tree structure)
- [ ] Write unit tests in `compiler/ori_llvm/src/codegen/ir_builder/metadata/tests.rs`:
  - Test `MetadataKinds::new()` returns non-zero kind IDs
  - Test `range_md_node` creates a valid 2-element node
  - Test `empty_md_node` creates a valid 0-element node
- [ ] Verify: `timeout 150 cargo t -p ori_llvm` passes

---

## 02.2 IrBuilder Metadata Helpers

**File(s):** `compiler/ori_llvm/src/codegen/ir_builder/metadata.rs` (extends the NEW file from 02.1), `compiler/ori_llvm/src/codegen/ir_builder/mod.rs` (411 lines — IrBuilder struct definition)

Add metadata attachment methods to IrBuilder so callers at `load()`/`store()` sites can attach metadata without touching llvm-sys directly. The IrBuilder already wraps inkwell's builder — these methods extend it with metadata capabilities. Methods go in `metadata.rs` (keeping the metadata module self-contained) while the `MetadataKinds` field is added to the `IrBuilder` struct in `mod.rs`.

- [ ] Add `metadata_kinds: MetadataKinds` field to `IrBuilder` in `mod.rs`, initialized during `IrBuilder::new()` (requires `LLVMContextRef` — obtain via `inkwell::Context::as_mut_ptr()` or equivalent)
- [ ] In `metadata.rs`, add `impl IrBuilder` block with metadata attachment methods:
  - `set_load_tbaa(&self, load_val: ValueId, tbaa_node: LLVMValueRef)` — attaches `!tbaa` to a load instruction
  - `set_load_range(&self, load_val: ValueId, lo: i64, hi: i64)` — attaches `!range` to a load
  - `set_load_invariant(&self, load_val: ValueId)` — attaches `!invariant.load` to a load
  - `set_instruction_alias_scope(&self, instr: ValueId, scope_node: LLVMValueRef)` — attaches `!alias.scope`
  - `set_instruction_noalias(&self, instr: ValueId, scope_node: LLVMValueRef)` — attaches `!noalias`
- [ ] Each method must: (a) resolve `ValueId` to raw `LLVMValueRef` via the value arena, (b) call the corresponding metadata wrapper function from 02.1
- [ ] Write tests in `metadata/tests.rs` verifying metadata attachment doesn't crash and appears in IR dump
- [ ] Verify: `timeout 150 cargo t -p ori_llvm` passes
- [ ] Verify: `mod.rs` stays under 500 lines (currently 411 + 1 field + 1 init line = ~413)

---

## 02.3 Extend ArcInstr::Project with Struct Type

**File(s):** `compiler/ori_arc/src/ir/instr.rs`

Currently `ArcInstr::Project { dst, ty, value, field }` (instr.rs lines 58-64) stores `ty` (the *result* type of the projection) but not the containing struct's type. To emit TBAA metadata at `struct_gep` sites, the codegen needs to know "this is field 2 of struct Point" — not just "this loads an int." Add `struct_ty: Idx` to carry the source struct's type through the pipeline.

- [ ] Add `struct_ty: Idx` field to `ArcInstr::Project` variant in `compiler/ori_arc/src/ir/instr.rs`
- [ ] Update all construction sites of `ArcInstr::Project` in `compiler/ori_arc/src/lower/`:
  - `lower/expr/mod.rs` — struct field access lowering (populate `struct_ty` from the source expression's type — note: the source is `value: ArcVarId`, so look up `func.var_types[value]` for the struct type)
  - `lower/expr/collections.rs` — collection field access (if applicable)
  - `lower/builder/emission.rs` — builder emission (has Project construction)
  - Search: `grep -rn "Project {" compiler/ori_arc/src/` to find all construction sites (count may have changed — re-verify at implementation time)
- [ ] Update all pattern matches on `ArcInstr::Project` — add `struct_ty` to destructuring:
  - `compiler/ori_arc/src/aims/` (transfer functions, emission)
  - `compiler/ori_arc/src/` (verification, liveness, drop analysis)
  - `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` (LLVM emission)
  - Search: `grep -rn "Project {" compiler/` to find all match sites
- [ ] For non-struct projections (tuple, enum payload), set `struct_ty` to the source aggregate's type — TBAA will use it for type disambiguation regardless
- [ ] Write test: create an `ArcFunction` with a `Project` instruction, verify `struct_ty` is populated correctly
- [ ] Verify: `timeout 150 cargo t` passes (all crates)

**Sync points:** `ArcInstr::Project` is matched in ~96 locations across ~30 files in `ori_arc` and `ori_llvm` (verified via `grep -c "Project {" compiler/ori_arc/ compiler/ori_llvm/`). Most are destructuring matches that will need `struct_ty` added. The bulk are in `ori_arc` (~82 occurrences in 24 files) with `ori_llvm` contributing ~14 in 6 files. Use `grep -rn "Project {" compiler/ori_arc/ compiler/ori_llvm/` to find every site. Consider a staged approach: (1) add field with `_struct_ty` in all match sites first to compile, (2) populate meaningfully at construction sites, (3) use at consumption sites (Section 04).

**This section provides to later sections:** Section 04 uses `struct_ty` at `struct_gep` emission sites to construct TBAA metadata nodes identifying "field N of struct S."

**Matrix dimensions:** Not applicable (structural refactor, no behavioral change).

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `metadata.rs` module exists with `MetadataKinds`, TBAA/range/invariant constructors
- [ ] `IrBuilder` has 5+ metadata attachment methods
- [ ] `ArcInstr::Project` carries `struct_ty: Idx`
- [ ] All `Project` construction/match sites updated (grep returns 0 mismatches)
- [ ] Smoke test: metadata appears in `ORI_DUMP_AFTER_LLVM=1` output
- [ ] `timeout 150 ./test-all.sh` green (debug AND release — `cargo b --release && timeout 150 ./test-all.sh`)
- [ ] No spurious warnings
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 02` returns 0
- [ ] **Plan sync** — update plan metadata
  - [ ] This section `status` → `complete`
  - [ ] `00-overview.md` Quick Reference updated
  - [ ] `index.md` status updated
  - [ ] Sections 03/04 `depends_on` verified
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `IrBuilder` can attach TBAA, range, invariant.load, alias.scope, and noalias metadata to instructions. `ArcInstr::Project` carries struct type information. All existing tests pass unchanged in both debug and release builds. A smoke test demonstrates metadata in the IR dump.
