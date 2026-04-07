---
section: "03"
title: "Value Passing Optimization"
status: not-started
reviewed: false
goal: "Eliminate unnecessary Value clones in parameter binding, self-binding, and capture binding"
inspired_by:
  - "CPython Py_INCREF (refcount increment, not clone)"
  - "Lua stack-based argument passing (no copy, callee reads parent frame)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Parameter Binding Without Clone"
    status: not-started
  - id: "03.2"
    title: "Self-Binding and Capture Optimization"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Value Passing Optimization

**Status:** Not Started
**OPTIONAL — Profile-gated.** Like Section 02, this optimizes the tree-walker which the VM replaces. Work this ONLY if profiling justifies it.

**Goal:** Eliminate avoidable `Value::clone()` work on the function call hot path. Current hot-path clones are a mix of cheap refcount bumps and a smaller number of real structure clones. Target: remove the expensive cases proven by profiling, without destabilizing semantics that the bytecode VM will replace anyway.

**Context:** The `bind_parameters_with_defaults()` function in `compiler/ori_eval/src/exec/call/mod.rs:40-58` clones every argument via `args[i].clone()`. But the cost model is mixed:
- Heap-backed `Value` variants already use `Arc` via `Heap<T>`, so many clones are shallow refcount bumps rather than deep copies
- `FunctionValue` captures are already shared via `Arc<FxHashMap<_, _>>`
- The remaining potentially expensive clones are inline `FunctionValue`/`MemoizedFunctionValue` payload clones and repeated rebinding of captured values into fresh environments

**Reference implementations:**
- **CPython**: `Py_INCREF` per parameter — refcount increment only, no deep clone
- **Lua**: Arguments stay in caller's stack frame; callee reads via offset — zero copy

**Depends on:** Section 01 (benchmarks). Independent of Section 02 (can be done in either order).

---

## 03.1 Parameter Binding Without Clone

**File(s):** `compiler/ori_eval/src/exec/call/mod.rs`

**Current code** (`exec/call/mod.rs:40-58`):
```rust
pub fn bind_parameters_with_defaults(
    interpreter: &mut crate::Interpreter<'_>,
    func: &FunctionValue,
    args: &[Value],
) -> Result<(), ControlAction> {
    let can_defaults = func.can_defaults();
    for (i, param) in func.params.iter().enumerate() {
        let value = if i < args.len() {
            args[i].clone()  // <-- Clone per parameter
        } else if let Some(Some(can_id)) = can_defaults.get(i) {
            interpreter.eval_can(*can_id)?
        } else {
            return Err(EvalError::new(...).into());
        };
        interpreter.env.define(*param, value, Mutability::Immutable);
    }
    Ok(())
}
```

**Fix**: Only move values instead of cloning when measurement shows clone work is still material after Section 02. Otherwise, preserve API stability and spend the effort on the bytecode path.

- [ ] **Benchmark clone share first**: after Section 02, measure how much time remains in argument cloning versus environment setup and recursive dispatch. Do not do large API churn unless clone work is still material.
- [ ] **Only switch `eval_call` to owned `Vec<Value>` if profiling justifies it**:
  - Audit all call sites of `eval_call` to verify args are not needed afterward
  - Account for memoization and any other callers that would immediately recreate clones, or the rewrite will simply move the cost around
- [ ] **If ownership rewrite is justified, move only ephemeral argument vectors**: keep borrowed fast paths where the caller must retain the slice, and avoid forcing a global signature churn before the VM exists
- [ ] **Specialize for primitives**: For `Value::Int`, `Value::Float`, `Value::Bool` — these are `Copy`-sized. The enum `clone()` is already cheap, but ensure the compiler optimizes it to a memcpy.
- [ ] **Matrix tests** (in `compiler/ori_eval/src/exec/call/tests.rs` and spec tests):
  - **Dimensions**: param count (0, 1, 2, 5) x param kind (primitive int/bool, collection [int], closure, struct) x default (no defaults, partial defaults, all defaults) x variadic (non-variadic, variadic)
  - **Semantic pin**: function with 3 params where only 1 is provided (remaining 2 have defaults) produces correct result with move semantics
  - **Negative pin**: double-use of moved arg produces compile error or runtime error (not silent corruption)
  - **TDD ordering**: benchmark clone share FIRST (measure before changing), then write ownership tests if justified

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-03.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 03.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 03.2 Self-Binding and Capture Optimization

**File(s):** `compiler/ori_eval/src/interpreter/function_call.rs`, `compiler/ori_eval/src/exec/call/mod.rs`

**Current self-binding** (`function_call.rs:51-52`):
```rust
call_interpreter
    .env
    .define(self_name, func.clone(), Mutability::Immutable);
```

`func.clone()` on a `Value::Function(FunctionValue)` clones the `FunctionValue` struct which contains: `params: Vec<Name>` (clone = new Vec alloc), `can_defaults: Vec<Option<CanId>>` (clone = new Vec alloc), `capabilities: Vec<Name>` (clone = new Vec alloc), plus three cheap Arc bumps (`captures: Arc<FxHashMap>`, `arena: SharedArena`, `canon: Option<SharedCanonResult>`). This is a real cost for the Vec clones, but it must be measured before the representation is widened.

**Fix**: Store a reference or lightweight handle instead of cloning the entire FunctionValue.

- [ ] **If profiling shows self-binding is still hot, introduce a shared function handle** for `Value::Function` / `Value::MemoizedFunction` so recursive self-binding is O(1) handle cloning instead of cloning inline metadata vectors.
- [ ] **Keep closure semantics explicit**: Ori closures currently capture by value from a frozen environment. Do not introduce lazy by-reference capture binding as an optimization without a separate semantic design, because it changes mutation and lifetime behavior.
- [ ] **Move capture optimization work toward the VM representation**: for the tree-walker, shallow clones are acceptable if they are no longer dominant; for the VM, captures should become indexed closure storage rather than re-bound name maps.
- [ ] **Do not spend time on `ModeState::child()`**: it is an inline struct reset, not a heap allocation. Leave it alone unless a profiler shows otherwise.
- [ ] **Matrix tests** (in `compiler/ori_eval/src/interpreter/function_call.rs` tests and spec tests):
  - **Dimensions**: self-binding type (non-recursive, recursive, memoized-recursive) x capture count (0, 1, 5) x capture nesting (flat, closure-in-closure) x capability (no caps, 1 cap)
  - **Semantic pin**: recursive function with memoization produces correct cached results after self-binding optimization
  - **Negative pin**: mutable capture semantics unchanged (Ori captures by value -- mutation of captured variable in closure does not affect outer scope)
  - **TDD ordering**: profile self-binding cost FIRST, only proceed if material

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-03.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 03.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] Any remaining hot-path `Value::clone()` sites are justified by measurement
- [ ] If self-binding is changed, the new representation is shared-handle based and regression-tested for recursion + memoization
- [ ] `cargo bench -p oric --bench interpreter -- closure_map` shows improvement
- [ ] All spec tests pass: `cargo st` green
- [ ] All Rust tests pass: `cargo t` green
- [ ] `./test-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 03` returns 0 annotations
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** Combined with Section 02, per-call cost ≤5µs (down from 63µs). Ackermann A(3,7) completes in <10s. Zero unnecessary clones on the hot path (verified by audit of `function_call.rs` and `exec/call/mod.rs`).
