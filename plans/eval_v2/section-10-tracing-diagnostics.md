---
section: "10"
title: Tracing & Diagnostics V2
status: not-started
goal: Enhance eval error reporting with backtraces, structured diagnostics, and integrated tracing
sections:
  - id: "10.1"
    title: Eval Backtrace
    status: not-started
  - id: "10.2"
    title: Structured EvalError
    status: not-started
  - id: "10.3"
    title: Tracing Integration
    status: not-started
  - id: "10.4"
    title: Performance Instrumentation
    status: not-started
---

# Section 10: Tracing & Diagnostics V2

**Status:** 📋 Planned
**Goal:** Enhance evaluation error reporting with call stack backtraces, structured error types, and integrated tracing — making runtime errors as informative as compile-time errors.

---

## Prior Art Analysis

### Rust CTFE: InterpError + Backtrace
Rust's const evaluator captures a `InterpErrorBacktrace` at each error site, optionally including the Rust call stack. Errors are categorized into `UndefinedBehavior`, `Unsupported`, `InvalidProgram`, and `ResourceExhaustion`. This categorization guides the error message and recovery strategy.

### Elm: Contextual Error Messages
Elm is famous for its error messages. Each error includes: what you wrote, what type was inferred, what type was expected, why it doesn't work, and how to fix it. For runtime errors, Elm uses Debug.todo with descriptive messages.

### Gleam: Diagnostic System
Gleam accumulates errors/warnings in a `Problems` struct. Invalid expressions (`TypedExpr::Invalid`) allow continued analysis after errors. Diagnostics include location, severity, and related information.

### TypeScript: Multi-Level Caching + Diagnostics
TypeScript caches types at multiple levels (NodeLinks, flow types) and produces diagnostic chains for complex type mismatches. Errors include related information and suggestion actions.

---

## 10.1 Eval Backtrace

Capture a call stack backtrace when evaluation errors occur:

```rust
/// A backtrace of the evaluation call stack at the point of an error.
#[derive(Clone, Debug)]
pub struct EvalBacktrace {
    frames: Vec<BacktraceFrame>,
}

#[derive(Clone, Debug)]
pub struct BacktraceFrame {
    /// Function name (or interned "<anonymous>" for lambdas)
    pub function_name: Name,
    /// Source file path (interned)
    pub file: Option<Name>,
    /// Source span of the call site
    pub span: Option<Span>,
    /// Line number (for display)
    pub line: Option<u32>,
}

impl EvalBacktrace {
    pub fn capture(call_stack: &CallStack) -> Self {
        let frames = call_stack.frames.iter().rev()
            .map(|frame| BacktraceFrame {
                function_name: frame.name,
                file: frame.file,
                span: frame.call_span,
                line: frame.line,
            })
            .collect();
        EvalBacktrace { frames }
    }

    /// Display the backtrace. Requires an interner to resolve Name -> &str.
    pub fn display(&self, interner: &impl StringLookup) -> String {
        let mut out = String::new();
        for (i, frame) in self.frames.iter().enumerate() {
            out.push_str(&format!("  {}: {}", i, interner.lookup(frame.function_name)));
            if let Some(file) = frame.file {
                out.push_str(&format!(" at {}", interner.lookup(file)));
                if let Some(line) = frame.line {
                    out.push_str(&format!(":{}", line));
                }
            }
            out.push('\n');
        }
        out
    }
}

/// Call stack tracking during evaluation.
/// Each child interpreter receives a clone of the parent's call stack frames,
/// then pushes its own frame. No shared mutable state — thread-safe by design.
pub struct CallStack {
    frames: Vec<CallFrame>,
    /// Max depth limit. On native builds, this is usize::MAX (stacker handles growth).
    /// On WASM builds, this enforces a hard limit (e.g., 1024).
    max_depth: usize,
}

struct CallFrame {
    name: Name,
    file: Option<Name>,
    call_span: Option<Span>,
    line: Option<u32>,
}

// Note: display() looks up the string representation via the interner.
// e.g., interner.lookup(frame.name) to get &str for display.

impl CallStack {
    /// Construct a CallStack with max_depth derived from the EvalMode.
    /// Connects Section 02's EvalMode.max_recursion_depth() to CallStack's limit.
    pub fn new(mode: &EvalMode) -> Self {
        CallStack {
            frames: Vec::new(),
            max_depth: mode.max_recursion_depth().unwrap_or(usize::MAX),
        }
    }

    pub fn push(&mut self, name: Name, span: Option<Span>) -> Result<(), EvalError> {
        if self.frames.len() >= self.max_depth {
            return Err(EvalError::stack_overflow(self.max_depth));
        }
        self.frames.push(CallFrame { name, file: None, call_span: span, line: None });
        Ok(())
    }

    pub fn pop(&mut self) {
        self.frames.pop();
    }
}
```

- [ ] Implement `EvalBacktrace` with frame capture
  - [ ] `capture(call_stack)` — snapshot current call stack
  - [ ] `display()` — human-readable backtrace string
- [ ] Implement `CallStack` for tracking active calls
  - [ ] `push(name, span)` — enter function (with depth check on WASM)
  - [ ] `pop()` — leave function
  - [ ] Replace current `call_depth: usize` with `CallStack`
  - [ ] **Multi-interpreter sharing**: Snapshot approach — each child interpreter clones the parent's `CallStack` frames and pushes a new frame. No `Rc<RefCell<...>>` needed. Thread-safe by design (no shared mutable state).
  - [ ] **Native builds**: `CallStack` tracks frames for backtrace display but does NOT enforce depth limits (`stacker` handles native stack growth)
  - [ ] **WASM builds**: `CallStack.push()` enforces `max_depth` (no stacker available)
- [ ] Integrate with Interpreter
  - [ ] `eval_call()` pushes/pops call stack
  - [ ] Error handling captures backtrace from call stack
  - [ ] Backtrace attached to EvalError

---

## 10.2 Structured EvalError

Redesign `EvalError` with proper categorization and structured data:

```rust
/// A structured evaluation error.
#[derive(Clone, Debug)]
pub struct EvalError {
    /// What kind of error
    pub kind: EvalErrorKind,
    /// Primary source location
    pub span: Option<Span>,
    /// Call stack backtrace (captured at error site)
    pub backtrace: Option<EvalBacktrace>,
    /// Additional context notes
    pub notes: Vec<EvalNote>,
}

/// Categorized error kinds (inspired by Rust's InterpErrorKind).
///
/// **Design note: String fields** — EvalErrorKind uses `String` (not `Name`/interned)
/// for error message fields. Rationale: error paths are cold (allocation cost negligible),
/// `String` enables direct `Display` implementation, JSON serialization, and simple test
/// assertions without interner dependency. Self-contained error values simplify threading
/// errors across evaluation boundaries.
#[derive(Clone, Debug)]
pub enum EvalErrorKind {
    // === Arithmetic ===
    DivisionByZero,
    IntegerOverflow { op: String, left: String, right: String },
    NegativeShift,

    // === Type Errors ===
    TypeMismatch { expected: String, got: String },
    InvalidCast { from: String, to: String },

    // === Access Errors ===
    UndefinedVariable { name: String },
    UndefinedField { field: String, type_name: String },
    UndefinedMethod { method: String, type_name: String, suggestions: Vec<String> },
    IndexOutOfBounds { index: i64, length: usize },

    // === Pattern Matching ===
    NonExhaustiveMatch { scrutinee_type: String },
    IrrefutablePatternFailed,

    // === Function Calls ===
    ArityMismatch { expected: usize, got: usize, func_name: String },
    StackOverflow { depth: usize },
    NotCallable { type_name: String },

    // === Assertion/Test ===
    AssertionFailed { message: String },
    TestFailed { test_name: String, message: String },
    PanicCalled { message: String },
    TodoReached { message: String },
    UnreachableReached { message: String },

    // === Capability ===
    MissingCapability { capability: String },

    // === Const Eval ===
    // NOTE: These are the canonical const-eval error variants. Section 07's
    // ConstEvalErrorKind has been merged — ConstEvalError is now a thin wrapper
    // { inner: EvalError, expr: ExprId } that uses these EvalErrorKind variants.
    // No separate ConstEvalErrorKind enum exists.
    ConstEvalBudgetExceeded,
    ConstEvalSideEffect { capability: String },

    // === Internal ===
    Internal { message: String },

    // === User-Defined ===
    Custom { message: String },
}

/// Additional context note attached to an error.
#[derive(Clone, Debug)]
pub struct EvalNote {
    pub message: String,
    pub span: Option<Span>,
}

impl EvalError {
    // Builder pattern for creating errors
    pub fn division_by_zero() -> Self {
        EvalError { kind: EvalErrorKind::DivisionByZero, span: None, backtrace: None, notes: vec![] }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_backtrace(mut self, bt: EvalBacktrace) -> Self {
        self.backtrace = Some(bt);
        self
    }

    pub fn with_note(mut self, message: impl Into<String>, span: Option<Span>) -> Self {
        self.notes.push(EvalNote { message: message.into(), span });
        self
    }

    pub fn stack_overflow(depth: usize) -> Self {
        EvalError { kind: EvalErrorKind::StackOverflow { depth }, span: None, backtrace: None, notes: vec![] }
    }

    // Factory methods for all EvalErrorKind variants follow the same pattern as
    // division_by_zero() and stack_overflow() above: construct EvalError with
    // the appropriate kind variant, no span/backtrace/notes, then chain
    // with_span()/with_backtrace()/with_note() as needed at the call site.
}
```

**Key improvements over current `EvalError`:**
- **Categorized kinds**: Each error type has its own variant with relevant data (not just `message: String`)
- **Backtrace**: Optional call stack captured at error site
- **Notes**: Additional context (like "defined here", "used here")
- **No ControlFlow**: Control flow moved to `ControlAction` (Section 05)
- **No propagated_value**: Try propagation moved to `ControlAction::Propagate` (Section 05)

- [ ] Define `EvalErrorKind` with all error categories
  - [ ] Arithmetic errors with operand details
  - [ ] Type errors with expected/got
  - [ ] Access errors with suggestions
  - [ ] Function errors with arity details
  - [ ] Test errors with test name and message
- [ ] Define `EvalNote` for additional context
- [ ] Implement builder pattern for `EvalError`
  - [ ] `with_span()`, `with_backtrace()`, `with_note()`
  - [ ] Constructor per error kind: `division_by_zero()`, `undefined_variable()`, etc.
- [ ] Migrate all error creation sites (phased approach)
  - [ ] **Phase 1**: Arithmetic errors — `DivisionByZero`, `IntegerOverflow`, `NegativeShift`
  - [ ] **Phase 2**: Access errors — `UndefinedVariable`, `UndefinedField`, `IndexOutOfBounds`, etc.
  - [ ] **Phase 3**: Control flow errors — `NonExhaustiveMatch`, `StackOverflow`, `ArityMismatch`, etc.
  - [ ] **Phase 4**: Remaining categories — assertion, capability, const eval, internal
  - [ ] During transition, `EvalError.message` is derivable from `EvalErrorKind` via `Display` impl
  - [ ] Keep existing factory function signatures as public API; change internals to construct `EvalError { kind: EvalErrorKind::... }`
  - [ ] **Note**: Additional error kinds will be added during migration to cover all 60+ existing error factories in `ori_patterns/src/errors.rs`
  - [ ] Attach spans wherever available
  - [ ] Attach backtraces for runtime errors
- [ ] Implement `impl From<EvalError> for Diagnostic` conversion
  - [ ] `EvalError` and `Diagnostic` remain separate types
  - [ ] Conversion maps `EvalErrorKind` to diagnostic severity, message, and related info
  - [ ] Enables unified output: evaluator produces `EvalError`, CLI converts to `Diagnostic` for display
  - [ ] Map each `EvalErrorKind` to an `ErrorCode` in the **E4xxx** range (E3xxx is already allocated for pattern errors in `ori_diagnostic`):
    - **E4000–E4099**: Arithmetic errors (`DivisionByZero`, `IntegerOverflow`, `NegativeShift`)
    - **E4100–E4199**: Type and access errors (`TypeMismatch`, `InvalidCast`, `UndefinedVariable`, `UndefinedField`, `UndefinedMethod`, `IndexOutOfBounds`)
    - **E4200–E4299**: Control flow errors (`NonExhaustiveMatch`, `IrrefutablePatternFailed`, `ArityMismatch`, `StackOverflow`, `NotCallable`)
    - **E4300–E4399**: Assertion, test, and panic errors (`AssertionFailed`, `TestFailed`, `PanicCalled`, `TodoReached`, `UnreachableReached`)
    - **E4400–E4499**: Capability errors (`MissingCapability`)
    - **E4500–E4599**: Const eval errors (`ConstEvalBudgetExceeded`, `ConstEvalSideEffect`)
    - **E4900–E4999**: Internal and custom errors (`Internal`, `Custom`)
  - [ ] Define `ErrorCode` variants for each E4xxx code
- [ ] Remove ControlFlow and propagated_value from EvalError
  - [ ] Depends on Section 05 completion

---

## 10.3 Tracing Integration

Enhance tracing with structured events:

```rust
/// Tracing events emitted during evaluation.
/// Controlled by ORI_LOG EnvFilter: e.g., ORI_LOG=ori_eval=trace
use tracing::{debug, trace, instrument, span, Level};

// **Transition note**: Tracing examples below use `Interpreter` as the current target.
// After Section 02 (EvalMode), the `#[instrument]` annotations stay on `Interpreter`
// (which holds `EvalMode` as a field). There is no separate `EvalMachine` type —
// `EvalMode` parameterizes the `Interpreter` via enum dispatch (Section 02).
// The `eval(ir_id, ir_arena)` signature reflects the post-Section 08 interface (EvalIR-based).
impl<'a> Interpreter<'a> {
    #[instrument(level = "trace", skip(self, ir_arena), fields(node_kind))]
    pub fn eval(&mut self, ir_id: EvalIrId, ir_arena: &EvalIrArena) -> EvalResult {
        let node = ir_arena.get(ir_id);
        tracing::Span::current().record("node_kind", format!("{:?}", node.kind_name()));
        // Dispatch to node-specific evaluation
        match node {
            // ... (see Section 08.4 for full dispatch)
            _ => todo!(),
        }
    }

    #[instrument(level = "debug", skip(self))]
    pub fn eval_call(&mut self, func_name: &str, arg_count: usize) -> EvalResult {
        // ...
    }

    #[instrument(level = "debug", skip(self))]
    pub fn eval_method_call(&mut self, type_name: &str, method: &str) -> EvalResult {
        // ...
    }
}
```

**Tracing levels**:
- `error`: Should never happen (internal errors, invariant violations)
- `warn`: Recoverable issues (fallback method resolution, deprecated patterns)
- `debug`: Phase boundaries (function entry/exit, scope push/pop, method dispatch)
- `trace`: Per-expression evaluation (very verbose, for debugging)

- [ ] Add `#[instrument]` to all major evaluation entry points
  - [ ] `eval()`, `eval_call()`, `eval_method_call()`
  - [ ] `eval_match()`, `eval_for()`, `eval_loop()`
  - [ ] Pattern matching, method dispatch
- [ ] Use structured fields in tracing spans
  - [ ] `expr_kind`, `func_name`, `method_name`, `type_name`
  - [ ] Avoid string formatting in the common (non-traced) path
- [ ] Document eval-specific tracing via existing `ORI_LOG` EnvFilter syntax
  - [ ] `ORI_LOG=ori_eval=trace` — full per-expression tracing
  - [ ] `ORI_LOG=ori_eval=debug` — function entry/exit, scope push/pop
  - [ ] `ORI_LOG=ori_eval[eval_call]=trace` — trace only call evaluation
  - [ ] No new env var needed — `ORI_LOG` already supports target-level filtering

---

## 10.4 Performance Instrumentation

Add optional performance tracking (for profiling and optimization):

```rust
/// Performance counters for evaluation (opt-in via ModeState.counters).
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
    /// Number of expressions folded to constants during IR lowering (Section 07)
    pub const_folded_nodes: u64,
    /// Number of EvalIR nodes evaluated at runtime
    pub ir_nodes_evaluated: u64,
}

impl EvalCounters {
    /// Report format clearly distinguishes real evaluations from cache activity.
    /// `expressions_evaluated` counts only real (non-cached) expression evaluations.
    /// `cache_hits` and `cache_misses` are tracked separately.
    pub fn report(&self) -> String {
        format!(
            "Eval stats: {} exprs evaluated (excl. cache), {} IR nodes evaluated, \
             {} const-folded, {} calls, {} methods, {} matches, {} scopes, \
             {} allocs, {} cache hits, {} cache misses",
            self.expressions_evaluated,
            self.ir_nodes_evaluated,
            self.const_folded_nodes,
            self.function_calls,
            self.method_calls,
            self.pattern_matches,
            self.scope_pushes,
            self.heap_allocations,
            self.cache_hits,
            self.cache_misses,
        )
    }
}
```

- [ ] Define `EvalCounters` struct
  - [ ] Expression, call, method, match, scope, allocation counters
  - [ ] `const_folded_nodes` — number of expressions folded during IR lowering (Section 07)
  - [ ] `ir_nodes_evaluated` — number of EvalIR nodes evaluated at runtime
  - [ ] `report()` — human-readable summary
- [ ] Integrate via `Option<EvalCounters>` field on `ModeState` (from Section 02)
  - [ ] `ModeState.counters: Option<EvalCounters>` — `None` in production, `Some(...)` when profiling
  - [ ] Counter increment methods are no-ops when `counters` is `None`
  - [ ] No generic parameter needed — runtime check via `Option`
  - [ ] **Transitional guidance**: Before Section 02 (`ModeState`) is implemented, use `Option<EvalCounters>` as a field directly on `Interpreter`. Migrate to `ModeState.counters` when Section 02 lands.
  - [ ] **Increment locations**: `expressions_evaluated` in `eval()`, `function_calls` in `eval_call()`, `method_calls` in `eval_method_call()`, `pattern_matches` in `eval_match()`, `scope_pushes` in `push_scope()`, `heap_allocations` in `Value` heap-allocating factories (e.g., `Value::list()`, `Value::str()`). Tracks allocations for heap-allocated values (see Section 09 for the `needs_rc()` classification).
- [ ] Add `--profile` flag to `ori` CLI
  - [ ] Sets `ModeState.counters = Some(EvalCounters::default())` (or `Interpreter.counters` during transition)
  - [ ] Propagation path: CLI parses `--profile` → sets field on `EvalConfig`/`InterpreterBuilder` → `Interpreter` receives `Some(EvalCounters::default())`
  - [ ] Prints summary after evaluation

---

## 10.5 Completion Checklist

- [ ] `EvalBacktrace` captures call stack at error sites (uses `Name` for interned strings)
- [ ] `CallStack` replaces `call_depth` counter (snapshot approach: child clones parent frames, no shared mutable state)
- [ ] `EvalErrorKind` with typed error categories (phased migration of 60+ factories)
- [ ] `EvalNote` for additional error context
- [ ] `impl From<EvalError> for Diagnostic` conversion (separate types, unified output)
- [ ] All error creation sites use typed constructors (phased: arithmetic, access, control flow, remaining)
- [ ] ControlFlow/propagated_value removed from EvalError
- [ ] Tracing `#[instrument]` on all major entry points
- [ ] Structured tracing fields (expr_kind, func_name, etc.)
- [ ] Eval-specific tracing via `ORI_LOG=ori_eval=<level>` (no new env vars)
- [ ] `EvalCounters` for optional performance tracking (via `Option<EvalCounters>` on `ModeState`)
- [ ] Counter report distinguishes real evaluations from cache hits/misses
- [ ] `--profile` flag prints eval statistics

**Exit Criteria:** Evaluation errors include call stack backtraces, typed error categories, and actionable context notes. Tracing provides structured observability at all levels.
