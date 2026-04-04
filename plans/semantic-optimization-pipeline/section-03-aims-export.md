---
section: "03"
title: "AIMS State Export to Codegen"
status: not-started
reviewed: false
goal: "Thread AIMS ownership and uniqueness facts to LLVM codegen so standard LLVM passes benefit from Ori's memory analysis"
success_criteria:
  - "ArcIrEmitter receives MemoryContract for the current function"
  - "Fresh allocations (Uniqueness::Unique returns) get noalias on call-site return"
  - "Call sites annotated with effect summary from MemoryContract (memory(read) for pure callees)"
  - "Satisfies mission criterion: AIMS facts survive into LLVM as noalias/alias.scope/memory() attributes"
inspired_by:
  - "MemoryContract horizontal flow pattern (ori_arc interprocedural → AimsPipelineConfig → per-function)"
  - "Nounwind two-pass pattern (declare → analyze → emit attributes)"
depends_on: ["02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Thread MemoryContract to ArcIrEmitter"
    status: not-started
  - id: "03.2"
    title: "Emit noalias on Fresh Allocations"
    status: not-started
  - id: "03.3"
    title: "Emit Effect-Based Call Site Annotations"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: AIMS State Export to Codegen

**Status:** Not Started
**Goal:** Thread AIMS's interprocedural analysis results (MemoryContract, ParamContract, ReturnContract, EffectSummary) through to the LLVM emitter so that standard LLVM optimization passes (LICM, GVN, SROA) have richer aliasing and effect information. Currently, `FunctionCompiler` holds `aims_contracts: FxHashMap<Name, MemoryContract>` (line 78 of `function_compiler/mod.rs`) but never passes it to `ArcIrEmitter` (line 176 of `define_phase.rs`). The contracts are computed, stored, and then ignored during LLVM emission.

**Success Criteria:**

- [ ] `ArcIrEmitter` has access to the current function's `MemoryContract`
- [ ] At call sites where callee's `ReturnContract.uniqueness == Unique`, return value gets `noalias` attribute
- [ ] At call sites where callee's `EffectSummary.may_allocate == false && may_deallocate == false`, the call gets `memory(read)` or `memory(none)` annotation
- [ ] `ORI_DUMP_AFTER_LLVM=1` shows new attributes on appropriate call sites

**Context:** AIMS computes rich per-function contracts via SCC fixpoint (`compute_aims_contracts()`). Each `MemoryContract` contains: per-parameter access/consumption/cardinality/locality/uniqueness, return value uniqueness/freshness, and function-level effect summary (may_allocate, may_deallocate, may_share, may_throw). These facts are consumed by the ARC pipeline (passed to `run_arc_pipeline()` at `define_phase.rs:317` for RC placement decisions and param ownership) but never reach the LLVM emission layer — `ArcIrEmitter::new()` at `define_phase.rs:176` does NOT receive the contracts map. Threading them to the emitter enables call-site-specific optimization attributes that LLVM's standard passes (LICM, GVN, SROA) can exploit.

**Reference implementations:**
- **Current pattern**: `FunctionCompiler.aims_contracts` field (mod.rs:78) — data exists but isn't threaded
- **Nounwind pattern**: `is_arc_function_nounwind()` (nounwind/analyze.rs:163) — post-analysis attribute application

**Depends on:** Section 02 (metadata infrastructure for alias.scope).

---

## 03.1 Thread MemoryContract to ArcIrEmitter

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

- [ ] Add `callee_contracts: &'a FxHashMap<Name, MemoryContract>` field to `ArcIrEmitter`
- [ ] Update `ArcIrEmitter::new()` signature to accept the contracts map
- [ ] Update the construction site at `define_phase.rs:176` to pass `&self.aims_contracts`
- [ ] Add `fn callee_contract(&self, name: Name) -> Option<&MemoryContract>` helper to ArcIrEmitter
- [ ] Verify: `timeout 150 cargo t -p ori_llvm` passes (no behavioral change yet)

---

## 03.2 Emit noalias on Fresh Allocations

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/apply.rs` or equivalent call-site emission

When emitting a function call, if the callee's `ReturnContract.uniqueness == Unique`, the return value is guaranteed to be a fresh allocation not aliased by anything. Apply `noalias` to the call-site return.

- [ ] At call-site emission (Apply instruction handling), look up callee in `callee_contracts`
- [ ] Write failing AOT test FIRST (TDD): function that returns a freshly allocated list → before the fix, IR dump does NOT show `noalias` on the return. This test will verify the attribute after the fix.
- [ ] If `return_info.uniqueness == Uniqueness::Unique`, add `noalias` return attribute to the call instruction
- [ ] Use `IrBuilder::add_call_site_return_attribute()` (may need to add this method if it doesn't exist — attributes.rs already has parameter-level call-site attributes)
- [ ] Verify AOT test now passes: `noalias` appears in IR dump on fresh allocation returns
- [ ] Verify: `ORI_DUMP_AFTER_LLVM=1` shows `noalias` on return of allocation functions
- [ ] For functions with disjoint parameters (both borrowed, different allocation domains), emit `!alias.scope` + `!noalias` metadata pair on parameter loads: each parameter gets its own scope, and its loads are `!noalias` with respect to the other parameter's scope. This enables LLVM to prove that loads from parameter A do not alias stores through parameter B.
- [ ] Verify: no test regressions (noalias/alias.scope are hints, not behavioral)

**Matrix dimensions:**
- Types: `[int]` (list allocation), `str` (string allocation), `{str: int}` (map allocation), user struct
- Patterns: direct call returning fresh, call returning borrowed (should NOT get noalias), call returning field projection (should NOT get noalias), two disjoint borrowed params (should get alias.scope pair)
- Semantic pin: IR dump test showing noalias on fresh allocation return
- Semantic pin: IR dump test showing `!alias.scope` and `!noalias` on disjoint parameter loads

---

## 03.3 Emit Effect-Based Call Site Annotations

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/apply.rs`

When emitting a function call, if the callee's `EffectSummary` indicates limited effects, annotate the call site with LLVM `memory()` attributes. This enables LICM (loop-invariant code motion) when calling pure functions in loops.

- [ ] At call-site emission, look up callee in `callee_contracts`
- [ ] If `effects.may_allocate == false && effects.may_deallocate == false && effects.may_throw == false`:
  - If also no writes to parameters → `memory(none)` (pure function)
  - If reads but no writes → `memory(read)` (readonly function)
- [ ] If `effects.may_allocate == true` but no deallocation → `memory(argmem: readwrite)` (allocating but not freeing)
- [ ] Use existing `IrBuilder::add_memory_none_attribute()` / `add_memory_read_attribute()` patterns (attributes.rs already has these for function-level, adapt for call-site level)
- [ ] Write failing AOT test FIRST (TDD): call a pure function inside a loop → before the fix, the call is NOT hoisted. This test will verify LICM after the fix.
- [ ] Verify AOT test now passes: IR shows the pure function call hoisted above the loop header
- [ ] Verify: no test regressions

**Matrix dimensions:**
- Types: pure int function, allocating function, function that reads but doesn't allocate
- Patterns: call outside loop (no change), call inside loop (should be hoistable if pure), call with side effects (should NOT get memory(none))
- Semantic pin: loop containing pure function call → IR shows the call hoisted above the loop header

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] `ArcIrEmitter` receives and queries `MemoryContract` for callees
- [ ] Fresh allocation returns get `noalias`
- [ ] Pure/readonly call sites get `memory(none)`/`memory(read)`
- [ ] `ORI_DUMP_AFTER_LLVM=1` demonstrates new attributes
- [ ] `timeout 150 ./test-all.sh` green (debug AND release — `cargo b --release && timeout 150 ./test-all.sh`)
- [ ] Dual-exec parity: `diagnostics/dual-exec-verify.sh tests/spec/` passes (eval and LLVM produce identical results — new attributes are hints, must not change observable behavior)
- [ ] `ORI_CHECK_LEAKS=1` clean on test programs exercising new attributes (noalias/memory() are optimization hints but verify RC ops remain balanced)
- [ ] No spurious warnings
- [ ] Plan annotation cleanup
- [ ] **Plan sync** — update plan metadata
  - [ ] This section `status` → `complete`
  - [ ] `00-overview.md` updated
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** AIMS ownership and effect facts are visible in LLVM IR output. Pure function calls in loops are hoistable. Fresh allocations are marked noalias. All tests pass unchanged.
