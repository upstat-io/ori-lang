---
section: "02"
title: Machine Abstraction
status: not-started
goal: Parameterize the evaluation core via an EvalMode enum with match dispatch, enabling distinct interpreter, const-eval, and test-runner policies (Salsa-compatible)
sections:
  - id: "02.1"
    title: EvalMode Enum Design
    status: not-started
  - id: "02.2"
    title: Interpret Variant
    status: not-started
  - id: "02.3"
    title: ConstEval Variant
    status: not-started
  - id: "02.4"
    title: TestRun Variant
    status: not-started
  - id: "02.5"
    title: Integration with Salsa
    status: not-started
---

# Section 02: Machine Abstraction

**Status:** 📋 Planned
**Goal:** Parameterize the eval core via an `EvalMode` enum so the same interpreter logic serves `ori run`, `ori check`, and `ori test` with distinct policies — using enum dispatch (not generics) for Salsa compatibility and to avoid circular dependencies.

**Why enum dispatch instead of a trait:** Salsa queries require all types to derive `Clone, Eq, PartialEq, Hash, Debug`. Generic `Interpreter<M: EvalMode>` would require `M` to satisfy these bounds, and associated types (`M::Extra`) would leak into Salsa query signatures. Additionally, trait methods referencing `Value` and `EvalError` would create circular dependencies if `EvalMode` were defined outside `ori_eval`. An enum keeps everything in one crate, is trivially Salsa-compatible, and match dispatch has negligible overhead for the small number of variants.

---

## Prior Art Analysis

### Rust: Machine Trait (The Gold Standard)
Rust's const evaluator (`InterpCx<'tcx, M: Machine<'tcx>>`) is generic over a `Machine` trait. The same interpreter core handles:
- **CompileTimeMachine** (CTFE) — strict const safety, no I/O
- **Miri** — permissive, tracks stacked borrows, supports I/O
- **DummyMachine** — layout computation only

This allows swapping evaluation policies without duplicating the interpreter. The trait defines hooks for: memory access, function calls, intrinsics, stack frame management, and error handling.

### Go: Operand Modes
Go's type checker tracks **operand modes** (`constant_`, `value`, `builtin`, `invalid`) that determine what operations are valid. This separation of "what kind of value am I dealing with" from "how to evaluate" is the same idea expressed differently.

### Zig: Comptime Blocks
Zig's `block.isComptime()` flag switches between compile-time and runtime evaluation within the same analysis. The `comptime_reason` field tracks WHY something is comptime, enabling precise error messages. The key insight: **compile-time and runtime evaluation coexist, selected by context**.

---

## 02.1 EvalMode Enum Design

```rust
/// Evaluation mode — determines interpreter behavior via match dispatch.
/// Enum (not trait) for Salsa compatibility and to avoid circular dependencies.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EvalMode {
    /// Standard mode for `ori run` — full I/O, capabilities enabled
    Interpret,
    /// Compile-time evaluation — no I/O, budget-limited, deterministic
    ConstEval {
        /// Max function calls before error (like Zig's branch_quota)
        budget: u32,
    },
    /// Test execution — captures output, collects results
    TestRun {
        /// Run only attached tests (vs all tests)
        only_attached: bool,
    },
}

/// Per-mode mutable state, stored alongside EvalMode in the interpreter.
///
/// NOTE: Print handling is intentionally NOT part of ModeState or EvalMode.
/// The existing `SharedPrintHandler` (enum dispatch, Arc-wrapped) already provides
/// a working, thread-safe print abstraction that supports print/println/capture/clear
/// across all modes. EvalMode controls evaluation *policy* (I/O permissions, budgets,
/// test collection); print *routing* is orthogonal and handled by SharedPrintHandler.
pub struct ModeState {
    /// Test results (used by TestRun)
    pub test_results: Option<Vec<TestResult>>,
    /// Const-eval call counter (used by ConstEval)
    pub call_count: usize,
    /// Memo cache for const-eval (used by ConstEval).
    /// References Section 07's `MemoCache` struct (keyed by `(func_id, args_hash)`).
    pub memo_cache: Option<MemoCache>,
}

impl EvalMode {
    /// Whether this mode allows side-effecting I/O (file system, network, etc.).
    /// Note: print output routing is handled separately by SharedPrintHandler,
    /// not by this flag. TestRun captures print output but does NOT allow
    /// general I/O side effects.
    pub fn allows_io(&self) -> bool {
        matches!(self, EvalMode::Interpret)
    }

    pub fn allows_entry_point(&self) -> bool {
        matches!(self, EvalMode::Interpret)
    }

    pub fn allows_capability(&self, _cap: Name) -> bool {
        !matches!(self, EvalMode::ConstEval { .. })
    }

    pub fn collects_tests(&self) -> bool {
        matches!(self, EvalMode::TestRun { .. })
    }

    pub fn eager_const_eval(&self) -> bool {
        matches!(self, EvalMode::ConstEval { .. })
    }

    pub fn max_recursion_depth(&self) -> Option<usize> {
        match self {
            EvalMode::Interpret => {
                #[cfg(target_arch = "wasm32")]
                { Some(200) }
                #[cfg(not(target_arch = "wasm32"))]
                { None }
            }
            EvalMode::ConstEval { .. } => Some(64),
            EvalMode::TestRun { .. } => Some(500),
        }
    }

    // NOTE: CallStack (Section 10) initializes its max_depth from
    // EvalMode.max_recursion_depth().unwrap_or(usize::MAX) at interpreter construction.
    // See Section 10.1 for CallStack::new(mode) constructor.

    // NOTE: handle_print is NOT on EvalMode. Print routing is handled by the existing
    // SharedPrintHandler (Arc<SharedPrintHandler>) which is stored on the Interpreter,
    // not on ModeState. SharedPrintHandler currently has two variants:
    //   - Stdout (default for Interpret mode)
    //   - Buffer (for TestRun — captures output, supports clear)
    // V2 adds two new variants:
    //   - Silent (for ConstEval — discards output)
    //   - Custom (for WASM/embedded targets)
    //
    // This keeps print routing orthogonal to evaluation policy. The interpreter selects
    // the appropriate SharedPrintHandler variant at construction time based on EvalMode,
    // but the two concerns remain separate.

    pub fn before_call(&self, _func: Name, state: &mut ModeState) -> EvalResult<()> {
        match self {
            EvalMode::ConstEval { budget } => {
                state.call_count += 1;
                if state.call_count > *budget as usize {
                    Err(EvalError::new("const evaluation exceeded budget"))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }
}
```

**Why enum dispatch:**
- **Salsa-compatible**: `EvalMode` derives `Clone, Eq, Hash, Debug` — trivially usable in Salsa queries
- **No circular deps**: Everything stays in `ori_eval` — no trait methods referencing `Value`/`EvalError` across crate boundaries
- **Negligible overhead**: 3 variants, match dispatch is effectively free vs vtable indirection
- **Simpler code**: No associated types, no generic parameter threading, no trait bounds propagation
- **Capability gating**: `allows_capability` enables the const-eval mode to reject I/O operations

- [ ] Define `EvalMode` enum in `ori_eval/src/machine.rs`
  - [ ] Variants: `Interpret`, `ConstEval { budget }`, `TestRun { only_attached }`
  - [ ] Methods via match dispatch: `allows_io`, `allows_capability`, `allows_entry_point`, `collects_tests`, `eager_const_eval`, `max_recursion_depth`, `before_call`
  - [ ] Derive `Clone, Debug, PartialEq, Eq, Hash` for Salsa compatibility
- [ ] Define `ModeState` struct for per-mode mutable state
  - [ ] Test results for TestRun
  - [ ] Call counter + memo cache for ConstEval
  - [ ] `ModeState::new(mode: &EvalMode)` factory initializes relevant fields
  - [ ] Print handling is NOT in ModeState — use existing `SharedPrintHandler` (Arc-wrapped enum dispatch)
- [ ] Add `mode: EvalMode`, `mode_state: ModeState`, and `print_handler: Arc<SharedPrintHandler>` fields to `Interpreter`
  - [ ] Thread `mode` through eval_inner, eval_call, etc.
  - [ ] `print_handler` selected based on EvalMode at construction (Stdout for Interpret, Buffer for TestRun, Silent for ConstEval)
  - [ ] No generic parameter — concrete type throughout
- [ ] Update all call sites to pass `EvalMode` at interpreter construction
  - [ ] Every interpreter construction specifies its mode explicitly

---

## 02.2 Interpret Variant

The standard mode for `ori run` — full evaluation with I/O. This is `EvalMode::Interpret` — the simplest variant with no extra configuration:

```rust
// Construction:
let mode = EvalMode::Interpret;

// Behavior (via match dispatch in EvalMode methods):
// - allows_io: true (only mode that permits real I/O side effects)
// - allows_entry_point: true
// - allows_capability: true (all non-ConstEval modes allow capabilities)
// - collects_tests: false
// - print routing: SharedPrintHandler::Stdout (or Custom for WASM)
// - max_recursion_depth: None (platform stack) or 200 (WASM)
```

- [ ] Verify `EvalMode::Interpret` match arms cover all methods
  - [ ] Wraps current print handler behavior
  - [ ] Full I/O and capability access
  - [ ] WASM recursion limit preserved
- [ ] Verify: `ori run file.ori` produces identical output with new mode

---

## 02.3 ConstEval Variant

For compile-time evaluation of constant expressions (Section 07):

```rust
// Construction:
let mode = EvalMode::ConstEval { budget: 1000 };
// ModeState auto-initializes: call_count = 0, memo_cache = Some(FxHashMap::default())

// Behavior (via match dispatch in EvalMode methods):
// - allows_io: false
// - allows_entry_point: false
// - allows_capability: always false (no capabilities in const-eval)
// - collects_tests: false
// - eager_const_eval: true
// - print routing: SharedPrintHandler::Silent (discards output)
// - before_call: increments call_count, errors if > budget
// - max_recursion_depth: Some(64)
```

**Inspired by:**
- **Zig's `branch_quota`**: Prevents infinite loops in compile-time evaluation
- **Rust's CompileTimeMachine**: Rejects I/O, enforces strict determinism

- [ ] Verify `EvalMode::ConstEval` match arms cover all methods
  - [ ] Budget tracking with configurable limit (stored in enum variant)
  - [ ] Call counter in ModeState (mutable state)
  - [ ] Memoization cache in ModeState for pure function calls
  - [ ] Reject all I/O and capability access
  - [ ] Tight recursion limit (64 levels)
- [ ] Const eval failures produce compile errors (not runtime errors)

---

## 02.4 TestRun Variant

For `ori test` — collects test results, captures output:

```rust
// Construction:
let mode = EvalMode::TestRun { only_attached: false };
// ModeState auto-initializes: test_results = Some(Vec::new())
// SharedPrintHandler set to Buffer variant (captures output)

// Behavior (via match dispatch in EvalMode methods):
// - allows_io: false (tests should not perform real I/O side effects)
// - allows_entry_point: false
// - allows_capability: true (all non-ConstEval modes allow capabilities)
// - collects_tests: true
// - print routing: SharedPrintHandler::Buffer (captures output, supports clear)
// - max_recursion_depth: Some(500)
```

Test failure handling is done via a separate method on `ModeState`:

```rust
impl ModeState {
    pub fn record_test_failure(
        &mut self,
        test_name: &str,
        error: &EvalError,
        print_handler: &SharedPrintHandler,
    ) {
        if let Some(results) = &mut self.test_results {
            results.push(TestResult::Failure(TestFailure {
                name: test_name.to_string(),
                error: error.clone(),
                // Captured output retrieved from SharedPrintHandler (not ModeState)
                output: print_handler.drain_buffer().unwrap_or_default(),
            }));
        }
    }
}
```

- [ ] Verify `EvalMode::TestRun` match arms cover all methods
  - [ ] Print output captured via `SharedPrintHandler::Buffer` (not ModeState)
  - [ ] Test failure collection via ModeState (receives captured output from SharedPrintHandler)
  - [ ] Attached vs floating test filtering (from `only_attached` field)
  - [ ] `allows_io` returns false (tests must not perform real I/O side effects)
- [ ] `ModeState` test support
  - [ ] Results accumulation
  - [ ] Failure details with captured output (drained from SharedPrintHandler)
  - [ ] Statistics (pass/fail/skip counts)
- [ ] Migrate existing test runner (`oric/src/test/runner.rs`) to use this mode

---

## 02.5 Integration with Salsa

The `EvalMode` enum derives `Clone, Eq, PartialEq, Hash, Debug`, making it trivially compatible with Salsa queries. No generic parameters, no trait bounds to propagate:

```rust
#[salsa::tracked]
pub fn evaluated(db: &dyn Db, file: SourceFile) -> ModuleEvalResult {
    // ... parse, type check ...
    let mut evaluator = Evaluator::builder(interner, arena, db)
        .mode(EvalMode::Interpret)  // ← concrete enum value
        .expr_types(&type_result.types)
        .pattern_resolutions(&type_result.pattern_resolutions)
        .build();
    // ...
}

#[salsa::tracked]
pub fn tested(db: &dyn Db, file: SourceFile) -> TestResults {
    // ... parse, type check ...
    let mut evaluator = Evaluator::builder(interner, arena, db)
        .mode(EvalMode::TestRun { only_attached })  // ← different variant
        .build();
    // ...
}
```

- [ ] Update `Evaluator::builder()` to accept `EvalMode`
  - [ ] `mode(EvalMode)` method on builder — no generic parameter needed
  - [ ] Default to `EvalMode::Interpret` if not specified
- [ ] Update Salsa queries to pass appropriate mode
  - [ ] `evaluated()` → `EvalMode::Interpret`
  - [ ] Add `tested()` query → `EvalMode::TestRun { only_attached }`
  - [ ] Future: `const_evaluated()` → `EvalMode::ConstEval { budget: 1000 }`
  - **Note**: The current test runner operates imperatively (`oric/src/test/runner.rs`). Wrapping in a Salsa query requires careful design — consider one tracked query per test function, or use Salsa only for test discovery while keeping execution imperative.
- [ ] Salsa caching: mode is part of the query key (different modes = different cache entries)
  - [ ] `EvalMode` derives `Hash` + `Eq` — works as query input automatically

---

## 02.6 Completion Checklist

- [ ] `EvalMode` enum defined with `Interpret`, `ConstEval`, `TestRun` variants
- [ ] `ModeState` struct defined for per-mode mutable state
- [ ] All policy methods implemented via match dispatch on `EvalMode`
- [ ] `EvalMode` derives `Clone, Debug, PartialEq, Eq, Hash` (Salsa-compatible)
- [ ] `Interpreter` stores `mode: EvalMode`, `mode_state: ModeState`, and `print_handler: Arc<SharedPrintHandler>` (no generic parameter)
- [ ] Salsa queries updated to pass appropriate `EvalMode` variant
- [ ] All existing tests pass (updated to specify `EvalMode::Interpret` explicitly)
- [ ] No mode-unaware `Interpreter` usage remains in codebase

**Exit Criteria:** The evaluator requires an explicit `EvalMode` enum value at every construction site. I/O, testing, and const-eval policies are controlled via match dispatch on the mode enum. Print routing uses existing `SharedPrintHandler` (separate from EvalMode). No trait bounds, no generic parameters, full Salsa compatibility.
