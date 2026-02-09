---
section: "06"
title: Structured Diagnostics
status: not-started
goal: Enhance eval error reporting with typed EvalErrorKind, call stack backtraces, context notes, and --profile instrumentation
sections:
  - id: "06.1"
    title: Call Stack and Backtraces
    status: not-started
  - id: "06.2"
    title: Structured EvalError
    status: not-started
  - id: "06.3"
    title: Diagnostic Conversion
    status: not-started
  - id: "06.4"
    title: Performance Instrumentation
    status: not-started
  - id: "06.5"
    title: Completion Checklist
    status: not-started
---

# Section 06: Structured Diagnostics

**Status:** Not Started
**Goal:** Make runtime errors as informative as compile-time errors — typed error categories, call stack backtraces, actionable context notes, and optional performance counters.

**File:** `compiler/ori_eval/src/diagnostics.rs` (new) + updates to `ori_patterns/src/errors.rs`

**Prior art:**
- **Rust CTFE** `InterpError` — categorized into `UndefinedBehavior`, `Unsupported`, `InvalidProgram`, `ResourceExhaustion`
- **Elm** — Contextual errors: what you wrote, what was inferred, what was expected, how to fix
- **Gleam** — `Problems` accumulator with `TypedExpr::Invalid` for continued analysis

---

## 06.1 Call Stack and Backtraces

Replace `call_depth: usize` with a proper `CallStack` that captures backtrace frames.

```rust
pub struct CallStack {
    frames: Vec<CallFrame>,
    max_depth: usize,  // From EvalMode.max_recursion_depth()
}

struct CallFrame {
    name: Name,
    call_span: Option<Span>,
    file: Option<Name>,   // Enriched by oric layer
    line: Option<u32>,    // Enriched by oric layer
}

pub struct EvalBacktrace {
    frames: Vec<BacktraceFrame>,
}
```

- [ ] Implement `CallStack` replacing `call_depth: usize`
  - [ ] `push(name, span)` with depth check (returns `Err` on overflow)
  - [ ] `pop()`
  - [ ] Max depth from `EvalMode.max_recursion_depth()` (connects to Section 05)
  - [ ] Clone-per-child model: child interpreter clones parent frames, pushes own frame (thread-safe, no shared mutable state)
  - [ ] O(N) clone per call acceptable at practical depths (~24 bytes per frame, ~24 KiB at 1000 frames)
- [ ] Implement `EvalBacktrace::capture(call_stack)` — snapshot frames at error site
- [ ] `EvalBacktrace::enrich(resolver)` — called by `oric` layer to add file/line info from source maps
- [ ] `EvalBacktrace::display(interner)` — human-readable backtrace string

---

## 06.2 Structured EvalError

Redesign `EvalError` with typed categories instead of just `message: String`.

```rust
pub struct EvalError {
    pub kind: EvalErrorKind,
    pub span: Option<Span>,
    pub backtrace: Option<EvalBacktrace>,
    pub notes: Vec<EvalNote>,
}

pub enum EvalErrorKind {
    // Arithmetic
    DivisionByZero,
    IntegerOverflow { op: String, left: String, right: String },
    NegativeShift,

    // Type
    TypeMismatch { expected: String, got: String },
    InvalidCast { from: String, to: String },

    // Access
    UndefinedVariable { name: String },
    UndefinedField { field: String, type_name: String },
    UndefinedMethod { method: String, type_name: String, suggestions: Vec<String> },
    IndexOutOfBounds { index: i64, length: usize },
    ImmutableBinding { name: String },

    // Pattern
    NonExhaustiveMatch { scrutinee_type: String },

    // Function
    ArityMismatch { expected: usize, got: usize, func_name: String },
    StackOverflow { depth: usize },
    NotCallable { type_name: String },

    // Assertion/Test
    AssertionFailed { message: String },
    PanicCalled { message: String },
    TodoReached { message: String },
    UnreachableReached { message: String },

    // Capability
    MissingCapability { capability: String },

    // Const Eval
    ConstEvalBudgetExceeded,
    ConstEvalSideEffect { capability: String },

    // Internal/Custom
    Internal { message: String },
    Custom { message: String },
}

pub struct EvalNote {
    pub message: String,
    pub span: Option<Span>,
}
```

- [ ] Define `EvalErrorKind` with all categories
- [ ] Builder pattern: `EvalError::division_by_zero().with_span(s).with_backtrace(bt)`
- [ ] Phased migration of existing error factories in `ori_patterns/src/errors.rs`:
  - [ ] Phase 1: Arithmetic errors
  - [ ] Phase 2: Access errors
  - [ ] Phase 3: Control flow and function errors
  - [ ] Phase 4: Assertion, capability, remaining
- [ ] Remove `ControlFlow` and `propagated_value` from `EvalError` (moved to `ControlAction` — future work)

---

## 06.3 Diagnostic Conversion

Map `EvalError` to `Diagnostic` for unified output.

- [ ] Implement `impl From<EvalError> for Diagnostic` in `oric` crate
  - [ ] Lives in `oric` due to orphan rule (depends on both `ori_patterns` and `ori_diagnostic`)
  - [ ] Maps `EvalErrorKind` → error code, severity, message, related info
- [ ] Error code ranges (E4xxx):
  - [ ] E4000–E4099: Arithmetic
  - [ ] E4100–E4199: Type/access/binding
  - [ ] E4200–E4299: Control flow/function
  - [ ] E4300–E4399: Assertion/test/panic
  - [ ] E4400–E4499: Capability
  - [ ] E4500–E4599: Const eval
  - [ ] E4900–E4999: Internal/custom

---

## 06.4 Performance Instrumentation

Optional counters activated by `--profile` flag.

```rust
#[derive(Default)]
pub struct EvalCounters {
    pub expressions_evaluated: u64,
    pub function_calls: u64,
    pub method_calls: u64,
    pub pattern_matches: u64,
    pub scope_pushes: u64,
    pub heap_allocations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub const_folded_nodes: u64,
}
```

- [ ] Define `EvalCounters` with `report() -> String`
- [ ] Store as `Option<EvalCounters>` on `ModeState` (connects to Section 05)
- [ ] Counter increment is no-op when `None` (zero cost in production)
- [ ] Add `--profile` CLI flag to `ori` → sets `counters = Some(EvalCounters::default())`
- [ ] Print summary after evaluation

---

## 06.5 Completion Checklist

- [ ] `CallStack` replaces `call_depth` with proper frame tracking
- [ ] `EvalBacktrace` captures and displays call stacks at error sites
- [ ] `EvalErrorKind` with ~25 typed categories (phased migration)
- [ ] Builder pattern for error construction
- [ ] `impl From<EvalError> for Diagnostic` in `oric` with E4xxx codes
- [ ] `EvalCounters` with `--profile` flag support
- [ ] `./test-all.sh` passes

**Exit Criteria:** Runtime errors include typed categories, call stack backtraces, and context notes. `--profile` prints evaluation statistics. Errors are as informative as compile-time diagnostics.
