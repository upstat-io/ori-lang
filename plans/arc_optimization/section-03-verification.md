---
section: "03"
title: Verification & Enforcement
status: not-started
goal: Add compile-time FBIP enforcement and dual-execution RC verification
sections:
  - id: "3.1"
    title: FBIP Enforcement Annotation
    status: not-started
  - id: "3.2"
    title: Dual-Execution RC Verification
    status: not-started
  - id: "3.3"
    title: Capability-Aware Borrow Inference
    status: not-started
  - id: "3.4"
    title: RC Operation Statistics
    status: not-started
  - id: "3.5"
    title: Cross-Function RC Elimination
    status: not-started
  - id: "3.6"
    title: Completion Checklist
    status: not-started
---

# Section 03: Verification & Enforcement

**Status:** Not Started
**Goal:** Turn FBIP from diagnostic to enforcement, add dual-execution verification, and lay groundwork for cross-function RC elimination.

**Design Reference:** `plans/dpr_arc-optimization_02212026.md` — Phase 3: Polish

**Depends on:** Section 01 (correct codegen) and Section 02 (enhanced elimination)

---

## 3.1 FBIP Enforcement Annotation

Add `@fbip` as a function annotation that promotes missed reuse from warning to compile error (Koka's `@fip` pattern).

- [ ] Add `@fbip` as a recognized attribute in `ori_ir`
- [ ] Thread through parser -> type checker -> ARC pipeline
- [ ] Implement `FbipEnforcement` enum (`Diagnostic` vs `Required`)
- [ ] In `run_arc_pipeline`, check annotation and call `check_fbip_enforcement` with `Required`
- [ ] Emit `E-level` diagnostic for each `MissedReuse` when enforced
- [ ] Add tests:
  - Function with `@fbip` and full reuse achieved -> no error
  - Function with `@fbip` and missed reuse -> compile error with suggestion
  - Function without `@fbip` and missed reuse -> warning only (existing behavior)

---

## 3.2 Dual-Execution RC Verification

Add integration test mode that runs each `@test` function through both JIT (conservative RC) and AOT (full pipeline), comparing outputs.

- [ ] Create test harness that runs functions through both execution paths
  - JIT: `insert_rc_ops` only (no elimination, no reset/reuse)
  - AOT: full pipeline (insertion + elimination + reset/reuse)
- [ ] Compare outputs — any divergence indicates an optimization bug
- [ ] Wire into `./test-all.sh` as optional verification pass
- [ ] Add tests:
  - Known-correct function produces identical output under both modes
  - Function with complex RC patterns (nested structs, closures) matches

---

## 3.3 Capability-Aware Borrow Inference

When the capability system is implemented, extend `infer_borrows` to recognize capability-free functions for more aggressive optimization.

- [ ] Detect capability-free callees (no `uses` clause)
- [ ] A capability-free callee borrowing a parameter is guaranteed not to cause deallocation
  - No side-effecting callback can run during the function's execution
  - Caller can skip `RcInc` for non-escaping contexts
- [ ] Add tests (requires capability system — may be deferred):
  - Capability-free function with borrowed param -> caller skips Inc
  - Function with capabilities -> conservative behavior preserved

**Note:** This item depends on Section 06 (Capabilities System). May be deferred until capabilities are implemented.

---

## 3.4 RC Operation Statistics

Add `--emit-arc-stats` flag for regression tracking and optimization tuning.

- [ ] Implement per-function RC operation counting in the pipeline
  - Track: insertions, eliminations, reuse achieved, reuse missed
- [ ] Output in JSON format for programmatic consumption
- [ ] Wire into compiler CLI (`ori build --emit-arc-stats`)
- [ ] Add tests:
  - Simple function produces expected stats
  - Stats change predictably when optimization is disabled

---

## 3.5 Cross-Function RC Elimination (Future)

After inter-function borrow caching (2.5) is stable, implement cross-call-boundary elimination.

- [ ] When callee immediately Decs a parameter that caller just Inc'd, elide both
  - Visible through borrow signature: parameter is `Borrowed` in callee
  - Caller's Inc at call site + callee's immediate Dec are redundant
- [ ] Annotate call sites with callee parameter ownership
- [ ] Add tests:
  - Caller Inc + callee immediate Dec -> both elided
  - Callee stores parameter (not immediate Dec) -> both preserved
  - Recursive call with self-borrows handled correctly

**Note:** This is the most complex optimization in the pipeline. Requires 2.5 to be stable first.

---

## 3.6 Completion Checklist

- [ ] `@fbip` annotation works end-to-end (parse -> check -> enforce)
- [ ] Dual-execution verification catches RC optimization bugs
- [ ] `--emit-arc-stats` provides per-function RC operation counts
- [ ] All existing tests pass (`./test-all.sh`)
- [ ] New tests cover each subsection

**Exit Criteria:** The ARC pipeline has compile-time enforcement (`@fbip`), runtime verification (dual-execution), and observability (`--emit-arc-stats`). Cross-function elimination is implemented or documented as a tracked future item.
