---
section: "04"
title: "Late COW Compound Contraction"
status: not-started
reviewed: false
goal: "Post-emission contraction pass fuses IsShared+branch+clone+mutate sequences into compound COW operations, reducing runtime overhead and enabling backend-level optimization"
inspired_by:
  - "LLVM ObjCARCContract.cpp tryToContractReleaseIntoStoreStrong (llvm-project/llvm/lib/Transforms/ObjCARC/ObjCARCContract.cpp:320-413)"
  - "LLVM Expand/Optimize/Contract three-pass pipeline pattern"
  - "Codex analysis: 'COW contraction probably higher value than generic contraction — Ori already computes CowMode in decide.rs'"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Define Compound COW IR Instruction"
    status: not-started
  - id: "04.2"
    title: "Implement COW Contraction Pass"
    status: not-started
  - id: "04.3"
    title: "LLVM Emission for Compound COW"
    status: not-started
  - id: "04.4"
    title: "Runtime Intrinsic Evaluation"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Late COW Compound Contraction

**Status:** Not Started
**Goal:** Add a late contraction pass that recognizes COW sequences (`IsShared` + branch + clone + mutate) in the ARC IR and contracts them into compound operations. This follows LLVM's Expand/Optimize/Contract pipeline pattern — the AIMS pipeline "expands" into primitive `IsShared`/`Set` instructions, optimizes them, and now we "contract" back into efficient compound forms for the backend.

**Context:** LLVM's `ObjCARCContract.cpp` fuses `load + retain + release + store` into a single `objc_storeStrong` call. The key insight: some instruction sequences implement a higher-level semantic operation, and the backend can handle the compound form more efficiently (single call, no branch prediction overhead, better register allocation). Ori's COW protocol (`IsShared` check → branch → clone-if-shared → mutate) is exactly this pattern. The AIMS pipeline already computes `CowMode` in `decide.rs` with cross-dimensional proofs — this section extends that to the IR and backend layers.

**Reference implementations:**
- **LLVM** `ObjCARCContract.cpp:320-413`: `tryToContractReleaseIntoStoreStrong()` — pattern-matches `load + retain + release + store` and replaces with `objc_storeStrong`. Runs late (after main optimization) to avoid disrupting dataflow analysis.
- **LLVM** pipeline: `ObjCARCExpand` (canonicalize) → `ObjCARCOpts` (optimize) → `ObjCARCContract` (fuse). The contract pass runs *after* optimization because compound forms are harder to analyze.

**Depends on:** Section 01 (statistics to count contractions), Section 02 (selective barriers shape which RC ops surround COW diamond patterns; barrier changes must be in place before contraction pattern-matching is finalized).

---

## 04.1 Define Compound COW IR Instruction

**File(s):** `compiler/ori_arc/src/ir/instr.rs` (not `mod.rs` — `ArcInstr` is defined in `instr.rs`)

Add a compound COW instruction to ARC IR that represents the entire uniqueness-check + potential-clone + mutate sequence as a single instruction.

**Scope warning:** `ArcInstr` is matched in **107 files** across `ori_arc`, `ori_llvm`, `ori_repr`, and `oric`. Adding a new variant requires updating every exhaustive match. Most non-test matches use `_ => {}` wildcard for unknown variants, but many key dispatch sites (`defined_var()`, `used_vars()`, `uses_var()`, `is_owned_position()`, `substitute_var()` in `instr.rs`, plus transfer functions, verify, emit_reuse, etc.) require explicit handling. Plan for ~30-50 match arm updates.

- [ ] Add `CowMutate` variant to `ArcInstr` in `ir/instr.rs`:

  **Import requirement:** `CowMode` is defined in `crate::uniqueness::lattice` (`compiler/ori_arc/src/uniqueness/lattice.rs:87`). Add `use crate::uniqueness::CowMode;` to `ir/instr.rs` imports. `CowMode` currently derives `Clone, Copy, Debug, PartialEq, Eq, Hash` — sufficient for `ArcInstr`'s non-cache derives. However, `ArcInstr` also has `#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]`, so `CowMode` MUST also get this derive. Add it to `CowMode`'s definition in `uniqueness/lattice.rs`.

  **Structural note:** `CowMutate` introduces a `dst` field, unlike `Set` which modifies `base` in-place without a separate destination. This means `CowMutate` is both a definition (`dst`) AND a use (`src`, `value`). This mirrors the COW diamond semantics: the result may be the original or a clone.

  ```rust
  /// Compound COW mutation — contracts IsShared+branch+clone+Set.
  ///
  /// Semantics: if `src` is uniquely referenced, mutate field in-place
  /// and bind `dst = src`. If shared, clone `src` into `dst`, then
  /// mutate the clone's field.
  /// The `cow_mode` annotation from AIMS determines the strategy:
  /// - `StaticUnique`: skip the check, mutate directly
  /// - `Dynamic`: emit the full check+branch+clone+mutate sequence
  /// - `StaticShared`: always clone (no check needed)
  CowMutate {
      dst: ArcVarId,
      src: ArcVarId,
      ty: Idx,         // Note: Idx, not TypeId — ARC IR uses Idx for types
      field: u32,      // Note: u32, not usize — consistent with Set/Project
      value: ArcVarId,
      cow_mode: CowMode,
      strategy: RcStrategy,
  },
  ```

- [ ] Implement `defined_var()` (`Some(dst)`), `used_vars()` (`[src, value]`), `uses_var()`, `is_owned_position()`, and `substitute_var()` for the new variant in `instr.rs`.

- [ ] Update all exhaustive match arms on `ArcInstr`. The 107 files that reference `ArcInstr::` break into two categories:

  **Category A: Exhaustive matches that MUST be updated** (compiler error if missing):
  - `ir/instr.rs` — 5 methods: `defined_var()`, `used_vars()`, `uses_var()`, `is_owned_position()`, `substitute_var()`
  - `verify/mod.rs` — verification rules (must add verification for `CowMutate`)
  - `aims/transfer/mod.rs` — backward transfer functions
  - `aims/emit_rc/` — forward walk, dead cleanup, edge cleanup (multiple files)
  - `aims/emit_reuse/` — reuse detection
  - `aims/intraprocedural/` — block analysis
  - `block_merge/` — merge, downgrade, select
  - `liveness/mod.rs`, `drop/mod.rs`, `borrow/` — various analyses
  - `oric/src/arc_dump/instr.rs` — debug dump (must format `CowMutate`)
  - `ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` — LLVM emission (Section 04.3)
  - `ori_repr/` — narrowing/range analysis

  **Category B: Wildcard matches (`_ =>`) that compile but should be reviewed:**
  - Most `ori_repr` range analysis files use `_ => {}` for unknown instructions — no update needed but review for correctness
  - Test files that construct specific instruction sequences — no update needed

  **Strategy for finding all sites:**
  1. Add the `CowMutate` variant to `ArcInstr`
  2. Run `cargo c -p ori_arc` — the compiler reports ALL exhaustive match errors in `ori_arc`
  3. Fix all `ori_arc` matches
  4. Run `cargo c -p ori_llvm` — reports `ori_llvm` matches
  5. Run `cargo c -p ori_repr` — reports `ori_repr` matches
  6. Run `cargo c` (full build) — catches any remaining
  7. The Rust compiler is the exhaustiveness checker — no manual grep needed for non-wildcard matches

  **Conservative default for new arms:** In analysis passes that don't need special handling for `CowMutate`, treat it the same as `Set` (mutation semantics). In RC emission, treat the `src` and `value` operands as used, `dst` as defined.

---

## 04.2 Implement COW Contraction Pass

**File(s):** `compiler/ori_arc/src/aims/emit_rc/cow_contract.rs` (new file — placed in `emit_rc/` alongside `cow.rs`, NOT in `aims/contract/` which is for `MemoryContract` interprocedural summaries)

Pattern-match COW sequences and replace with `CowMutate`.

- [ ] Define the COW sequence pattern to match:
  ```
  Pattern spans 4 blocks (diamond CFG):
  
  Block A (head):
    body: [..., IsShared { dst: flag, var: src }]
    terminator: Branch { cond: flag, then_block: B_shared, else_block: B_unique }
  
  Block B_shared (shared path):
    body: [RcInc(src), Apply(clone_fn, src) → cloned, Set { base: cloned, ... }, ...]
    terminator: Jump { target: C_merge, args: [cloned] }
  
  Block B_unique (unique path):
    body: [Set { base: src, ... }]
    terminator: Jump { target: C_merge, args: [src] }
  
  Block C_merge (phi):
    params: [(result, ty)]
    ...
  ```
  
  **Important:** `Branch` is an `ArcTerminator`, not an `ArcInstr`. The pattern spans basic blocks — this is a cross-block pattern match, not a single-block peephole. The contraction replaces all 4 blocks' relevant parts with a single `CowMutate` instruction in the head block.

- [ ] Implement `contract_cow_sequences()`:
  ```rust
  /// Scan the CFG for COW diamond patterns and contract them into CowMutate.
  ///
  /// Returns the number of contractions performed.
  pub fn contract_cow_sequences(func: &mut ArcFunction) -> usize {
      // Walk blocks looking for:
      // 1. Block ending with Branch whose cond is defined by IsShared in same block
      // 2. Both successors have Set on the same field
      // 3. One successor also has a clone (shared path)
      // 4. Both successors Jump to the same merge block
      // Replace the diamond with a CowMutate in the head block.
  }
  ```

- [ ] Place in pipeline: after `realize_annotations()` (step 10), before final `verify()` (step 11). This is the "contract" phase — runs after all AIMS decisions are made.

  **Pipeline interaction:** `realize_annotations()` computes `CowAnnotations` (a `(block_idx, instr_idx) -> CowMode` map) for each `Set` instruction at a COW site. The contraction pass consumes these annotations: it reads the `CowMode` from the annotation map for each `Set` that participates in a COW diamond, embeds it into the `CowMutate` instruction, and removes the annotation entry (since the CowMode now lives in the instruction). The LLVM emitter handles `CowMutate` directly (reading CowMode from the instruction fields) rather than consulting the annotation map.

  **Annotation index invalidation:** Replacing a 4-block diamond with a single `CowMutate` instruction changes block indices and instruction indices. The contraction pass MUST update `CowAnnotations` indices for any remaining (non-contracted) annotations. Process contractions in reverse block order to avoid invalidating earlier indices.

  **Prerequisite: `aims_pipeline.rs` splitting** (see overview Prerequisites table). The pipeline file is 590 lines — at the 500-line limit. Before adding contraction invocation, complete the prerequisite extraction task. If Section 03 or 05 is implemented first, they will have already done this.

- [ ] Track `cow_contractions` in `SynergyMetrics`.

- [ ] **TPR checkpoint** — `/tpr-review` covering 04.1–04.2 implementation work

- [ ] Unit tests (TDD: write BEFORE implementing `contract_cow_sequences()`):
  - Simple COW diamond → contracts to `CowMutate`
  - Non-COW diamond (different variables) → no contraction
  - `StaticUnique` annotation → `CowMutate` with `StaticUnique` mode
  - `StaticShared` → `CowMutate` with `StaticShared` mode
  - Multiple COW diamonds in one function → all contracted
  - COW diamond with extra instructions in head block → correctly preserves non-COW instructions
  - **Negative pin:** Diamond where the two paths mutate DIFFERENT fields → must NOT contract (different semantics)

**Semantic pin:** Test that a COW mutation program produces identical output with and without contraction enabled. Use a flag (or the `ORI_SKIP_ARC_OPTS` env var from Section 06) to disable contraction and compare outputs.

---

## 04.3 LLVM Emission for Compound COW

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` (instruction dispatch is in `instr_dispatch.rs`, not `mod.rs`)

Emit efficient LLVM IR for `CowMutate` instructions.

- [ ] Add dispatch arm for `CowMutate` in `instr_dispatch.rs`:

  The instruction dispatch file (`codegen/arc_emitter/instr_dispatch.rs`) has the main match on `ArcInstr` variants. Add a `ArcInstr::CowMutate { .. } => self.emit_cow_mutate(..)` arm.

- [ ] Implement `emit_cow_mutate()` method on `ArcIrEmitter`:

  **Architecture note:** The `Dynamic` mode generates new LLVM basic blocks (unique_bb, shared_bb, merge_bb) with a phi node at the merge point. This is the same pattern used by `IsShared` + `Branch` in the existing emitter, but now encapsulated in a single instruction handler. Look at the existing `IsShared` emission and `Branch` terminator emission for the exact LLVM builder API calls (`build_conditional_branch`, `build_phi`, etc.).

  ```rust
  fn emit_cow_mutate(&mut self, dst: ArcVarId, src: ArcVarId, ty: Idx,
                      field: u32, value: ArcVarId, cow_mode: CowMode,
                      strategy: RcStrategy) {
      match cow_mode {
          CowMode::StaticUnique => {
              // Direct in-place mutation — no check, no branch
              self.emit_set_field(src, ty, field, value);
              self.set_var(dst, self.get_var(src));
          }
          CowMode::Dynamic => {
              // Full COW: check + branch + clone-if-shared + mutate
              let is_shared = self.emit_is_shared(src);
              // Create unique_bb, shared_bb, merge_bb
              // branch on is_shared → shared_bb / unique_bb
              // unique_bb: mutate in-place, br merge_bb
              // shared_bb: clone, mutate clone, br merge_bb
              // merge_bb: phi(src from unique, clone from shared)
              // set_var(dst, phi_result)
          }
          CowMode::StaticShared => {
              // Always clone — skip the check
              let cloned = self.emit_clone(src, ty, strategy);
              self.emit_set_field(cloned, ty, field, value);
              self.set_var(dst, cloned);
          }
      }
  }
  ```

  **Note:** `emit_set_field`, `emit_is_shared`, and `emit_clone` are not existing method names — they represent the LLVM operations that must be assembled from the emitter's existing primitives (GEP, load, store, call to `ori_clone_*`, etc.). Review the existing `Set` and `IsShared` instruction handlers in `instr_dispatch.rs` for the exact primitives.

- [ ] AOT tests in `ori_llvm/tests/aot/` (all 3 CowMode variants must be covered):
  - COW mutation on uniquely-owned struct (`StaticUnique`) → no clone, no branch
  - COW mutation on shared struct (`Dynamic`) → check + clone-if-shared + mutate
  - COW mutation on always-shared struct (`StaticShared`) → unconditional clone + mutate
  - COW mutation in loop → correct behavior across iterations (dynamic uniqueness may change per iteration)
  - `ORI_CHECK_LEAKS=1` on all AOT tests → zero leaks
  - Dual-exec parity: interpreter and LLVM produce identical results for all COW test programs (interpreter has no CowMutate -- verifies semantic equivalence)
  - Debug and release builds produce identical results

- [ ] **TPR checkpoint** -- `/tpr-review` covering 04.3 LLVM emission implementation

---

## 04.4 Runtime Intrinsic Evaluation

**File(s):** `compiler/ori_rt/src/` (if needed)

Evaluate whether inlined LLVM IR or a runtime intrinsic is more efficient for compound COW. This is a mandatory profiling step, not an optional bonus.

- [ ] **Profile the emitted LLVM IR from 04.3** on benchmark programs:
  - Measure branch prediction miss rate on COW `Dynamic` mode (use `perf stat`)
  - Measure code size impact of inlined COW sequence vs a function call
  - Compare against `ori_rt`'s existing COW functions (`ori_cow_mutate_list`, etc.)

- [ ] **Record the decision** with measurements:
  - If inlined IR is equal or better: close this subsection with a note documenting the measurements. No runtime intrinsic needed.
  - If a runtime intrinsic is measurably better (>5% improvement on COW-heavy benchmarks): implement `ori_cow_mutate_field(data, field_offset, new_value, elem_size)` in `ori_rt` with `extern "C"` ABI and update the LLVM emitter to call it for `Dynamic` mode.

**This subsection is a profiling gate, not deferred work.** The deliverable is either (a) measurements proving inlined IR is sufficient, or (b) the runtime intrinsic implementation.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] `CowMutate` variant added to `ArcInstr` with correct `defined_var()`/`used_vars()`
- [ ] `contract_cow_sequences()` correctly pattern-matches COW diamonds
- [ ] Contraction produces `CowMutate` with correct `cow_mode`
- [ ] `ArcIrEmitter::emit_cow_mutate()` generates correct LLVM IR for all 3 modes
- [ ] `StaticUnique` mode emits no branch — direct in-place mutation
- [ ] `Dynamic` mode emits check+branch+clone+mutate sequence
- [ ] `StaticShared` mode emits unconditional clone+mutate
- [ ] `cow_contractions` tracked in `SynergyMetrics`
- [ ] AOT tests verify correct behavior for all 3 modes
- [ ] Contraction-enabled and contraction-disabled produce identical program output
- [ ] Runtime intrinsic evaluation complete: profiling data recorded, decision documented (inline IR sufficient OR intrinsic implemented)
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on all test programs
- [ ] `./test-all.sh` green (debug + release)
- [ ] No spurious warnings in normal compilation
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 04` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues
- [ ] `/impl-hygiene-review` passed — hygiene review clean
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `ORI_LOG=ori_arc=info ori build` shows `cow_contractions > 0` on programs with COW mutations. All 3 `CowMode` variants emit correct LLVM IR verified by AOT tests. `ORI_CHECK_LEAKS=1` reports zero leaks. Program behavior identical with and without contraction.
