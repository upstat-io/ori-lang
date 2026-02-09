---
section: "05"
title: Evaluation Modes
status: not-started
goal: Parameterize the evaluator via an EvalMode enum — Interpret, ConstEval, TestRun — with match dispatch, Salsa-compatible
sections:
  - id: "05.1"
    title: EvalMode Enum
    status: not-started
  - id: "05.2"
    title: ModeState
    status: not-started
  - id: "05.3"
    title: Salsa Integration
    status: not-started
  - id: "05.4"
    title: Completion Checklist
    status: not-started
---

# Section 05: Evaluation Modes

**Status:** Not Started
**Goal:** Parameterize the eval core via an `EvalMode` enum so `ori run`, `ori check`, and `ori test` use distinct evaluation policies. Enum dispatch (not generics) for Salsa compatibility.

**File:** `compiler/ori_eval/src/machine.rs`

**Prior art:**
- **Rust** `InterpCx<M: Machine>` — CompileTimeMachine / Miri / DummyMachine (trait-generic, but Ori uses enum for Salsa compatibility)
- **Go** operand modes — `constant_`, `value`, `builtin`, `invalid`
- **Zig** `block.isComptime()` — switches compile-time vs runtime evaluation

---

## 05.1 EvalMode Enum

```rust
/// Evaluation mode — determines interpreter behavior via match dispatch.
/// Enum (not trait) for Salsa compatibility: Clone, Eq, Hash, Debug required.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EvalMode {
    /// Standard mode for `ori run` — full I/O, capabilities enabled
    Interpret,
    /// Compile-time evaluation — no I/O, budget-limited, deterministic
    ConstEval { budget: u32 },
    /// Test execution — captures output, collects results
    TestRun { only_attached: bool },
}
```

- [ ] Define `EvalMode` enum with `Interpret`, `ConstEval { budget }`, `TestRun { only_attached }`
- [ ] Policy methods via match dispatch:
  - [ ] `allows_io()` → true only for `Interpret`
  - [ ] `allows_entry_point()` → true only for `Interpret`
  - [ ] `allows_capability(cap)` → false for `ConstEval` (must be pure), true otherwise
  - [ ] `collects_tests()` → true only for `TestRun`
  - [ ] `eager_const_eval()` → true only for `ConstEval`
  - [ ] `max_recursion_depth()` → None for Interpret (native), 64 for ConstEval, 500 for TestRun, 200 for WASM
  - [ ] `before_call(func, state)` → budget tracking for ConstEval (increments call_count, errors if > budget)
- [ ] Add `mode: EvalMode` field to `Interpreter`
- [ ] All interpreter construction sites specify mode explicitly

---

## 05.2 ModeState

Per-mode mutable state stored alongside `EvalMode`:

```rust
pub struct ModeState {
    /// Test results (TestRun only)
    pub test_results: Option<Vec<TestResult>>,
    /// Call counter (ConstEval only)
    pub call_count: usize,
    /// Memo cache for pure function calls (ConstEval only)
    pub memo_cache: Option<MemoCache>,
    /// Performance counters (opt-in via --profile, see Section 06)
    pub counters: Option<EvalCounters>,
}
```

- [ ] `ModeState::new(mode)` factory initializes relevant fields per mode
- [ ] Print handling is NOT in ModeState — use existing `SharedPrintHandler` (`Arc<PrintHandlerImpl>`)
  - [ ] `Interpret` → `PrintHandlerImpl::Stdout`
  - [ ] `TestRun` → `PrintHandlerImpl::Buffer` (captures output)
  - [ ] `ConstEval` → `PrintHandlerImpl::Silent` (discards output — new variant)
- [ ] `record_test_result()` for TestRun mode
- [ ] `check_budget()` for ConstEval mode

---

## 05.3 Salsa Integration

```rust
#[salsa::tracked]
pub fn evaluated(db: &dyn Db, file: SourceFile) -> ModuleEvalResult {
    let mut evaluator = Evaluator::builder(interner, arena, db)
        .mode(EvalMode::Interpret)
        .build();
    // ...
}

#[salsa::tracked]
pub fn tested(db: &dyn Db, file: SourceFile) -> TestResults {
    let mut evaluator = Evaluator::builder(interner, arena, db)
        .mode(EvalMode::TestRun { only_attached })
        .build();
    // ...
}
```

- [ ] Update `Evaluator::builder()` to accept `EvalMode`
  - [ ] Default: `EvalMode::Interpret`
  - [ ] No generic parameter — concrete type throughout
- [ ] Update Salsa queries to pass appropriate mode
  - [ ] `evaluated()` → `EvalMode::Interpret`
  - [ ] `tested()` → `EvalMode::TestRun { only_attached }`
  - [ ] Future: `const_evaluated()` → `EvalMode::ConstEval { budget: 1000 }`
- [ ] Migrate existing test runner (`oric/src/test/runner.rs`) to use `TestRun` mode

---

## 05.4 Completion Checklist

- [ ] `EvalMode` defined with all policy methods
- [ ] `ModeState` initialized per mode
- [ ] `Interpreter` requires explicit `EvalMode` at construction
- [ ] Salsa queries pass appropriate mode
- [ ] `ori run` uses `Interpret`, `ori test` uses `TestRun`
- [ ] `./test-all.sh` passes

**Exit Criteria:** The evaluator requires an explicit `EvalMode` at every construction site. I/O, testing, and const-eval policies controlled via match dispatch. Full Salsa compatibility.
