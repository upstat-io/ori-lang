---
section: "07"
title: "Salsa Integration & Transition"
status: not-started
reviewed: false
goal: "Integrate the bytecode VM into the runtime execution pipeline, preserve the tree-walker for const-eval/pattern execution, and provide a feature flag for gradual rollout"
inspired_by:
  - "Zig comptime vs runtime dual execution paths"
  - "Rust MIR interpreter (const eval) vs codegen (runtime) split"
  - "CPython __pycache__ — bytecode compilation cached separately from execution"
depends_on: ["05"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "Salsa Query Integration"
    status: not-started
  - id: "07.2"
    title: "Const-Eval and Test-Mode Preservation"
    status: not-started
  - id: "07.3"
    title: "Feature Flag and Switchover"
    status: not-started
  - id: "07.4"
    title: "Tree-Walker Retirement Strategy"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Salsa Integration & Transition

**Status:** Not Started
**Goal:** Make the bytecode VM actually usable in the Ori compiler. Sections 04-05 build the VM in isolation; this section wires it into the real runtime entry points so `ori run` and the interpreter-backed `ori test` path can use it, while `ori check` and type-driven const-eval continue to rely on the preserved tree-walker where appropriate. Without this section, the VM is a standalone module that can't run programs through the compiler's real execution surfaces.

**Context:** Ori has two runtime-facing interpreter entry points today, and both must be integrated:
1. `ori run` uses `evaluated()` / `run_evaluation()` in `compiler/oric/src/query/mod.rs`
2. `ori test` uses `TestRunner::run_file_with_interner()` in `compiler/oric/src/test/runner/mod.rs`, which builds an `Evaluator` directly and calls `load_module(...)`
3. Import/module wiring lives in `Evaluator::load_module()` (`compiler/oric/src/eval/evaluator/module_loading.rs`), not only at the top-level query boundary

The bytecode VM must plug into this same pipeline. The tree-walker cannot simply be deleted because:
- **`ConstEval` mode** is used by the type checker for compile-time evaluation of `$name` constants and `#cfg` conditions. It needs budget limiting, no I/O, and deterministic behavior. The VM can serve this role, but only after the VM supports budget tracking.
- **`TestRun` mode** is part of the evaluator contract for test execution. The VM must preserve the runtime semantics needed by the test runner, but the runner itself still owns test enumeration, filtering, and pass/fail bookkeeping.
- **`PatternExecutor` trait** — `Interpreter` implements `PatternExecutor` from `ori_patterns`, used for pattern system integration. The VM must either implement this trait or the pattern system must be updated.

**Reference implementations:**
- **Rust** `const fn` evaluation uses MIR interpreter (separate from codegen). Tree-walker is the const-eval engine; LLVM is the runtime engine.
- **Zig** `comptime` uses the same Sema interpreter for both compile-time and runtime, with a mode flag.
- **CPython** caches compiled `.pyc` bytecode; the `compile()` step is separate from `exec()`.

**Depends on:** Section 05 (working bytecode VM).

---

## 07.1 Salsa Query Integration

**File(s):** `compiler/oric/src/query/mod.rs`, `compiler/oric/src/test/runner/mod.rs`, `compiler/oric/src/eval/evaluator/module_loading.rs`, `compiler/ori_eval/src/lib.rs`

Wire the bytecode VM into the real execution entry points so programs can execute through the full runtime pipeline.

**Current pipeline** (`query/mod.rs:392-448`):
```
evaluated() → parsed() → typed() → canonicalize_cached() → Evaluator::builder().build() → load + run
```

**Target runtime shape** (with VM):
```
ori run:
  evaluated() / run_evaluation() → canonicalize_cached() →
    if VM_ENABLED: compile chunk → VM::execute() → ModuleEvalResult
    else: Evaluator(tree-walker) → ModuleEvalResult

ori test:
  TestRunner → Evaluator::load_module(...) / backend dispatch →
    if VM backend enabled: compile chunk(s) + execute test body through VM
    else: existing tree-walker path
```

- [ ] **Add a `bytecode_compiled()` Salsa query or side cache** for the `ori run` path that compiles `SharedCanonResult` to `Chunk`. The cache choice (`#[salsa::tracked]` vs session-scoped side cache) is owned here, after the Section 04 IR shape is real.
- [ ] **Create `run_evaluation_vm()`** — parallel to `run_evaluation()` in `query/mod.rs` — that takes a compiled `Chunk` and executes it via the VM, producing a `ModuleEvalResult` for `ori run`.
- [ ] **Add the corresponding VM execution seam to the test runner**: `ori test --vm` cannot be implemented by toggling `evaluated()` alone because the test runner builds an `Evaluator` directly.
- [ ] **Handle `ModuleEvalResult` compatibility**: The VM must produce the same `ModuleEvalResult` structure as the tree-walker. This includes: success/failure status, captured output (for `TestRun`), error messages with spans and backtraces.
- [ ] **Handle imported modules at the module-loading seam**: the current evaluator resolves imports via `imports::resolve_imports(...)` and `Evaluator::load_module(...)`, canonicalizing imported modules and registering their functions before execution. The VM path must plug into that same module-loading contract rather than assuming imports are driven only by recursive `evaluated()` calls.
- [ ] **Verify `decision_tree` TODOs are resolved**: `compiler/ori_eval/src/exec/decision_tree/mod.rs` lines 266 and 276 have TODOs referencing "section-07" about `test_kind` usage and numeric discriminants. These should have been resolved during Section 04.3 (DecisionTree compilation). Verify the bytecode compiler's handling is correct and update or remove the TODOs. If 04.3 deferred them, resolve here: either the VM handles `TestKind` dispatch and numeric discriminants natively, or the tree-walker decision_tree code is shared via a common helper.
- [ ] **Resolve `can_eval` TODO**: `compiler/ori_eval/src/interpreter/can_eval/mod.rs` line 119 has a TODO about type reference resolution during canonicalization. Verify this is not a VM blocker -- if TypeRef resolution is already handled in canonicalization (it is, per the `CanExpr::TypeRef` comment), update the TODO to reflect current state.
- [ ] **Matrix tests** (in `compiler/oric/tests/phases/eval/` and spec tests):
  - **Dimensions**: pipeline path (ori run, ori run --vm) x program shape (simple @main, @main with args, imports, type errors) x eval mode (Interpret, ConstEval, TestRun)
  - **Semantic pin**: `ori run --vm hello_world.ori` produces identical stdout to `ori run hello_world.ori`
  - **Negative pin**: program with type errors does NOT reach the VM (caught at type-check phase, produces same diagnostic)
  - **TDD ordering**: wire up `run_evaluation_vm()` FIRST with a minimal program, verify it produces `ModuleEvalResult`, then expand to imports and test runner

- [ ] **Subsection close-out (07.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-07.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 07.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 07.2 Const-Eval and Test-Mode Preservation

**File(s):** `compiler/ori_eval/src/eval_mode/mod.rs`, `compiler/ori_eval/src/bytecode/vm.rs`

The tree-walker serves three `EvalMode`s. The VM must either replace all three or the tree-walker must be preserved for modes the VM cannot handle.

**Current modes** (`eval_mode/mod.rs`):
- `EvalMode::Interpret` — full I/O, for `ori run`. **VM target.**
- `EvalMode::ConstEval { budget }` — no I/O, budget-limited, deterministic. Used by type checker. **Must work.**
- `EvalMode::TestRun { only_attached }` — captures output and provides the evaluator-side runtime contract used by the test runner. **Must work.**

- [ ] **Add `EvalMode` support to the VM**: The VM must respect the same mode policies:
  - `ConstEval`: check budget on every `Call` opcode (equivalent to `mode_state.check_budget()` in the tree-walker). Return `BudgetExceeded` error if exceeded. Forbid all capability lookups. No I/O.
  - `TestRun`: inject print handler (buffer capture) and preserve evaluator semantics expected by the test runner. Test enumeration, `only_attached`, and result bookkeeping remain runner-owned unless that contract is explicitly redesigned.
  - `Interpret`: full capabilities, no budget, stdout print handler.
- [ ] **Handle `PatternExecutor` trait**: The tree-walker `Interpreter` implements `PatternExecutor` from `ori_patterns`. The VM must either: (a) implement `PatternExecutor` (requires the VM to expose a `match_pattern()` method), or (b) use a thin adapter that creates a temporary tree-walker `Interpreter` for pattern execution. Option (b) is pragmatic for the initial cut; option (a) is correct long-term.
- [ ] **Handle ModeState and performance counters**: The VM must increment `count_expression()`, `count_function_call()`, `count_method_call()`, `count_pattern_match()` when profiling is enabled (via `--profile`). These are inlined no-ops when profiling is off.
- [ ] **Decision: VM for ConstEval or preserve tree-walker?** The tree-walker is simple and correct for `ConstEval` (small programs, tight budget). The VM adds overhead (bytecode compilation) for programs that execute <100 operations. **Recommended**: use VM for `Interpret` and `TestRun`; keep tree-walker for `ConstEval`. This avoids the bytecode compilation cost for tiny const-eval programs and preserves a known-correct fallback.
- [ ] **Matrix tests** (in `compiler/ori_eval/src/bytecode/vm/tests.rs` and `compiler/oric/tests/phases/eval/`):
  - **Dimensions**: eval mode (Interpret, ConstEval, TestRun) x boundary condition (budget exceeded, budget OK, output captured, profiling on/off)
  - **Semantic pin**: `ConstEval` of `$x = 2 + 3` produces `Value::Int(5)` via tree-walker (not VM), with budget decremented
  - **Semantic pin**: `TestRun` via VM captures `print(msg: "hello")` output in buffer (identical to tree-walker capture)
  - **Negative pin**: `ConstEval` with budget=1 on a 2-call program returns `BudgetExceeded` error
  - **TDD ordering**: write mode-dispatch tests FIRST (verify tree-walker used for ConstEval, VM used for Interpret), then mode-specific behavior tests

- [ ] **Subsection close-out (07.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-07.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 07.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 07.3 Feature Flag and Switchover

**File(s):** `compiler/oric/src/query/mod.rs`, `compiler/oric/src/test/runner/mod.rs`, `compiler/oric/src/commands/run/mod.rs`, `compiler/oric/src/commands/test.rs`, `compiler/oric/src/main.rs`

Provide a way to gradually switch from tree-walker to VM without a big-bang cutover.

- [ ] **Add `ORI_USE_BYTECODE_VM` environment variable**: When set to `1`, `evaluated()` routes through the VM pipeline. Default: `0` (tree-walker). This allows testing the VM in production-like scenarios without committing to it.
- [ ] **Add `--vm` CLI flag to `ori run`**: `ori run --vm file.ori` forces VM execution for a single invocation. Useful for A/B comparison.
- [ ] **Add `--vm` flag to `ori test`**: Runs the interpreter-backed test suite through a VM-capable backend/switch in `TestRunner`, not just via the `evaluated()` query path. This is the primary acceptance criterion — if `ori test --vm` passes all spec tests, the VM is ready to become default.
- [ ] **Dual-execution mode**: `ori run --dual file.ori` runs both tree-walker and VM, compares results, reports mismatches. This is the transition-period safety net. Integrate with `diagnostics/bytecode-verify.sh` from Section 06.
- [ ] **Switchover sequence**:
  1. `ORI_USE_BYTECODE_VM=1` behind env var (opt-in, testing)
  2. All spec tests pass with `--vm` (Section 06 gate)
  3. Default flips to VM (tree-walker becomes fallback)
  4. Tree-walker retained only for `ConstEval` mode (see 07.4)
- [ ] **Matrix tests** (in `compiler/oric/tests/` integration tests):
  - **Dimensions**: CLI flag (no flag, --vm, --dual) x command (run, test) x program category (simple, closures, imports, error-propagation)
  - **Semantic pin**: `ori run --dual file.ori` reports "0 mismatches" for a representative program
  - **Negative pin**: `ori run --dual` with a program that triggers a known tree-walker/VM divergence (if any) reports the mismatch clearly
  - **TDD ordering**: implement env var and CLI flag parsing FIRST, then `--vm` routing, then `--dual` comparison

- [ ] **Subsection close-out (07.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-07.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 07.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 07.4 Tree-Walker Retirement Strategy

**File(s):** `compiler/ori_eval/src/interpreter/mod.rs`, `compiler/ori_eval/src/lib.rs`

Define what happens to the tree-walker after the VM is the default.

- [ ] **Do NOT delete the tree-walker.** It must be preserved for:
  - `ConstEval` mode — used by the type checker for compile-time constant evaluation. Small programs, tight budget, no I/O. The tree-walker is simpler and correct for this use case.
  - `PatternExecutor` trait implementation — used by `ori_patterns` for the pattern matching system.
  - Fallback/debugging — if the VM produces wrong results, developers need the tree-walker as a reference oracle.
- [ ] **Mark the tree-walker as "const-eval engine"**: Rename or reorganize to clarify its new role. It is no longer the primary runtime interpreter; it is the compile-time evaluator.
- [ ] **Remove only genuinely runtime-only code from the tree-walker**: after the VM handles `Interpret` and `TestRun`, trim pieces that are provably unused by const-eval/pattern execution, but do **not** assume generic function-call machinery is removable just because const-eval has a tighter recursion limit. `create_function_interpreter()` / scope cleanup may still be required for compile-time function calls unless a separate const-eval call path exists first.
- [ ] **Gate the cleanup on dual-execution parity**: Only remove tree-walker runtime code AFTER Section 06 proves the VM and tree-walker produce identical results for all programs.
- [ ] **Matrix tests** (in `compiler/oric/tests/phases/eval/` and `compiler/ori_eval/src/interpreter/mod.rs` tests):
  - **Dimensions**: preserved path (ConstEval, PatternExecutor) x program complexity (simple const, const with function call, pattern with guard)
  - **Semantic pin**: `$ANSWER = 6 * 7` evaluates to `Value::Int(42)` via tree-walker ConstEval after VM is default for runtime
  - **Semantic pin**: `PatternExecutor::call()` on tree-walker Interpreter still works (used by `ori_patterns`)
  - **Negative pin**: importing a deleted tree-walker module produces compile error (if any code was incorrectly removed)
  - **TDD ordering**: write preservation tests FIRST (verify ConstEval and PatternExecutor work), then clean up unused tree-walker code, verify preservation tests still pass

- [ ] **Subsection close-out (07.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-07.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 07.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 07.R Third Party Review Findings

- None.

---

## 07.N Completion Checklist

- [ ] `ori run file.ori` executes through the VM via the full Salsa pipeline (parse -> type-check -> canonicalize -> compile bytecode -> VM execute)
- [ ] `ori run --vm file.ori` flag works and uses the VM
- [ ] `ori test --vm` runs spec tests through the VM
- [ ] `ORI_USE_BYTECODE_VM=1` env var works
- [ ] `ConstEval` mode continues to use the tree-walker and works correctly
- [ ] `TestRun` mode works through the VM with output capture
- [ ] Imported modules resolve correctly through the VM pipeline
- [ ] `--dual` mode works and detects mismatches
- [ ] Tree-walker preserved for const-eval and pattern execution
- [ ] `./test-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 07` returns 0 annotations
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `ori run --vm` can execute any Ori program through the full Salsa pipeline. `ori test --vm` passes all 5,800+ spec tests. `ConstEval` mode unaffected. A feature flag controls the switchover. The tree-walker is preserved for compile-time evaluation.
