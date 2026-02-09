---
section: "05"
title: Evaluation Modes
status: complete
completed: 2026-02-09
goal: Parameterize the evaluator via an EvalMode enum — Interpret, ConstEval, TestRun — with match dispatch, Salsa-compatible
sections:
  - id: "05.1"
    title: EvalMode Enum
    status: complete
  - id: "05.2"
    title: ModeState
    status: complete
  - id: "05.3"
    title: Salsa Integration
    status: complete
  - id: "05.4"
    title: Completion Checklist
    status: complete
---

# Section 05: Evaluation Modes

**Status:** Complete (2026-02-09)
**Goal:** Parameterize the eval core via an `EvalMode` enum so `ori run`, `ori check`, and `ori test` use distinct evaluation policies. Enum dispatch (not generics) for Salsa compatibility.

**File:** `compiler/ori_eval/src/eval_mode.rs`

**Prior art:**
- **Rust** `InterpCx<M: Machine>` — CompileTimeMachine / Miri / DummyMachine (trait-generic, but Ori uses enum for Salsa compatibility)
- **Go** operand modes — `constant_`, `value`, `builtin`, `invalid`
- **Zig** `block.isComptime()` — switches compile-time vs runtime evaluation

---

## 05.1 EvalMode Enum

```rust
/// Evaluation mode — determines interpreter behavior via match dispatch.
/// Enum (not trait) for Salsa compatibility: Clone, Eq, Hash, Debug required.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum EvalMode {
    /// Standard mode for `ori run` — full I/O, capabilities enabled
    #[default]
    Interpret,
    /// Compile-time evaluation — no I/O, budget-limited, deterministic
    ConstEval { budget: u32 },
    /// Test execution — captures output, collects results
    TestRun { only_attached: bool },
}
```

- [x] Define `EvalMode` enum with `Interpret`, `ConstEval { budget }`, `TestRun { only_attached }`
- [x] Policy methods via match dispatch:
  - [x] `allows_io()` → true only for `Interpret`
  - [x] `allows_entry_point()` → true only for `Interpret`
  - [x] `allows_capability(cap)` → false for `ConstEval` (must be pure), true otherwise
  - [x] `collects_tests()` → true only for `TestRun`
  - [x] `eager_const_eval()` → true only for `ConstEval`
  - [x] `max_recursion_depth()` → None for Interpret (native), 64 for ConstEval, 500 for TestRun, 200 for WASM
  - [x] Budget tracking via `ModeState::check_budget()` for ConstEval
- [x] Add `mode: EvalMode` field to `Interpreter`
- [x] All interpreter construction sites specify mode explicitly (default `Interpret`)
- [x] Unified `check_recursion_limit()` — removed `#[cfg(target_arch)]` duplication, uses `mode.max_recursion_depth()`
- [x] Unified `create_function_interpreter()` — single version propagates mode, removed `#[cfg]` duplication
- [x] Removed `DEFAULT_MAX_CALL_DEPTH` constant — replaced by `EvalMode::Interpret.max_recursion_depth()`

---

## 05.2 ModeState

Per-mode mutable state stored alongside `EvalMode`:

```rust
pub struct ModeState {
    /// Call counter (ConstEval only)
    pub call_count: usize,
    /// Maximum call budget (ConstEval only)
    budget: Option<u32>,
}
```

- [x] `ModeState::new(mode)` factory initializes relevant fields per mode
- [x] Print handling is NOT in ModeState — use existing `SharedPrintHandler` (`Arc<PrintHandlerImpl>`)
  - [x] `Interpret` → `PrintHandlerImpl::Stdout`
  - [x] `TestRun` → `PrintHandlerImpl::Buffer` (captures output)
  - [x] `ConstEval` → `PrintHandlerImpl::Silent` (discards output — **new variant**)
- [x] `check_budget()` for ConstEval mode with `BudgetExceeded` error type

**Design decision:** `ModeState` is leaner than the plan proposed. `test_results`, `memo_cache`, and `counters` fields were deferred:
- `test_results` — the test runner already collects results externally; adding it to ModeState would duplicate
- `memo_cache` — not needed until ConstEval is actually wired (Section 07)
- `counters` — belongs to Section 06 (Performance Instrumentation)

---

## 05.3 Salsa Integration

- [x] Update `Evaluator::builder()` to accept `EvalMode` via `.mode()` method
  - [x] Default: `EvalMode::Interpret`
  - [x] No generic parameter — concrete type throughout
- [x] Salsa `evaluated()` query uses default `Interpret` mode
- [x] Test runner (`oric/src/test/runner.rs`) uses `TestRun { only_attached: false }` mode
- [x] WASM playground updated — removed `DEFAULT_MAX_CALL_DEPTH` dependency, uses `EvalMode`
- [ ] Future: `tested()` Salsa query with `TestRun` mode (deferred to Section 07)
- [ ] Future: `const_evaluated()` → `EvalMode::ConstEval { budget: 1000 }` (deferred to Section 07)

---

## 05.4 Completion Checklist

- [x] `EvalMode` defined with all policy methods (7 methods, 24 unit tests)
- [x] `ModeState` initialized per mode with `check_budget()`
- [x] `PrintHandlerImpl::Silent` variant added with `silent_handler()` factory
- [x] `Interpreter` carries `mode` and `mode_state` fields
- [x] `InterpreterBuilder` has `.mode()` method, unified build (no `#[cfg]` duplication)
- [x] `EvaluatorBuilder` (oric) has `.mode()` method, passes through to `InterpreterBuilder`
- [x] Salsa queries pass appropriate mode
- [x] `ori run` uses `Interpret`, `ori test` uses `TestRun`
- [x] WASM playground updated
- [x] `./test-all.sh` passes (8,394 tests, 0 failures)
- [x] `./clippy-all.sh` passes

**Exit Criteria:** The evaluator requires an explicit `EvalMode` at every construction site. I/O, testing, and const-eval policies controlled via match dispatch. Full Salsa compatibility. ✅
