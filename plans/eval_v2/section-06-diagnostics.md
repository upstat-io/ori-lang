---
section: "06"
title: Structured Diagnostics
status: in-progress
goal: Enhance eval error reporting with typed EvalErrorKind, call stack backtraces, context notes, and --profile instrumentation
sections:
  - id: "06.1"
    title: Call Stack and Backtraces
    status: complete
  - id: "06.2"
    title: Structured EvalError
    status: in-progress
  - id: "06.3"
    title: Diagnostic Conversion
    status: complete
  - id: "06.4"
    title: Performance Instrumentation
    status: complete
  - id: "06.5"
    title: Completion Checklist
    status: in-progress
---

# Section 06: Structured Diagnostics

**Status:** Nearly Complete (ControlAction refactor deferred to Section 07)
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
    max_depth: Option<usize>,  // From EvalMode.max_recursion_depth()
}

pub struct CallFrame {
    pub name: Name,
    pub call_span: Option<Span>,
}

pub struct EvalBacktrace {
    frames: Vec<BacktraceFrame>,
}
```

- [x] Implement `CallStack` replacing `call_depth: usize`
  - [x] `push(name, span)` with depth check (returns `Err` on overflow)
  - [x] `pop()`
  - [x] Max depth from `EvalMode.max_recursion_depth()` (connects to Section 05)
  - [x] Clone-per-child model: child interpreter clones parent frames, pushes own frame (thread-safe, no shared mutable state)
  - [x] O(N) clone per call acceptable at practical depths (~24 bytes per frame, ~24 KiB at 1000 frames)
- [x] Implement `EvalBacktrace::capture(call_stack)` — snapshot frames at error site
- [x] Backtrace enrichment with file/line info — implemented as `snapshot_to_diagnostic()` in `oric::problem::eval` using `LineOffsetTable` for span→line:col conversion
- [x] `EvalBacktrace::display(interner)` — human-readable backtrace string (also `Display` impl)

**Implementation notes:**
- `CallStack` lives in `compiler/ori_eval/src/diagnostics.rs`
- `BacktraceFrame` and `EvalBacktrace` live in `compiler/ori_patterns/src/errors.rs`
- `create_function_interpreter()` accepts a `call_name: Name` for frame names
- Method dispatch call sites pass real method names; function calls use placeholder ("self") — proper names in Section 07

---

## 06.2 Structured EvalError

Redesign `EvalError` with typed categories instead of just `message: String`.

- [x] Define `EvalErrorKind` with 24 typed categories
- [x] Builder pattern: `.with_span(s)`, `.with_backtrace(bt)`, `.with_note(note)`
- [x] Phased migration of existing error factories in `ori_patterns/src/errors.rs`:
  - [x] All factory functions use `from_kind()` for structured kinds
  - [x] `Custom` variant for uncategorized errors
- [x] `EvalErrorSnapshot` — Salsa-compatible snapshot type that captures diagnostic fields (message, kind_name, span, backtrace frames, notes) without `Value`
- [x] `ModuleEvalResult::runtime_error()` preserves full `EvalError` context at Salsa query boundary
- [ ] Remove `ControlFlow` and `propagated_value` from `EvalError` (moved to `ControlAction` — deferred to Section 07)

**Implementation notes:**
- Both `kind: EvalErrorKind` and `message: String` are kept for backward compatibility
- `from_kind()` computes `message` from `kind.to_string()`, ensuring consistency
- `EvalErrorKind` lives in `ori_patterns` (crate dependency constraint)
- `EvalErrorSnapshot` is `Clone + Eq + Hash + Debug` (Salsa-compatible) — strips `Value` and `ControlFlow` from `EvalError`

---

## 06.3 Diagnostic Conversion

Map `EvalError` to `Diagnostic` for unified output.

- [x] Implement `eval_error_to_diagnostic()` in `oric::problem::eval` module
  - [x] Lives in `oric` due to orphan rule (depends on both `ori_patterns` and `ori_diagnostic`)
  - [x] Maps `EvalErrorKind` → error code, severity, message, labels, notes, backtrace, suggestions
- [x] Error code ranges (E6xxx — E4xxx was already taken by ARC analysis):
  - [x] E6001–E6009: Arithmetic
  - [x] E6010–E6019: Type/operator
  - [x] E6020–E6029: Access (variable, function, field, method, index, key, immutable)
  - [x] E6030–E6039: Function (arity, stack overflow, not callable)
  - [x] E6040–E6049: Pattern/match
  - [x] E6050–E6059: Assertion/test
  - [x] E6060–E6069: Capability
  - [x] E6070–E6079: Const eval
  - [x] E6080–E6089: Not implemented
  - [x] E6099: Custom/catch-all

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
    pub const_folded_nodes: u64,
}
```

- [x] Define `EvalCounters` with `report() -> String`
- [x] Store as `Option<EvalCounters>` on `ModeState` (connects to Section 05)
- [x] Counter increment is no-op when `None` (zero cost in production)
- [x] Convenience counter methods on `ModeState` (`count_expression()`, `count_function_call()`, etc.)
- [x] Add `--profile` CLI flag to `ori run` → sets `counters = Some(EvalCounters::default())`
- [x] Print counter summary to stderr after evaluation via `evaluator.counters_report()`
- [x] Wire counter increments at existing dispatch sites: `count_expression()` in `eval_inner()`, `count_function_call()` in `eval_call()`, `count_method_call()` in `eval_method_call()`, `count_pattern_match()` in `eval_match()`
- Note: `--profile` bypasses Salsa `evaluated()` query to access counters directly (Salsa drops evaluator after query completes)

---

## 06.5 Completion Checklist

- [x] `CallStack` replaces `call_depth` with proper frame tracking
- [x] `EvalBacktrace` captures and displays call stacks at error sites
- [x] `EvalErrorKind` with 24 typed categories
- [x] Builder pattern for error construction (`.with_span()`, `.with_backtrace()`, `.with_note()`)
- [x] `eval_error_to_diagnostic()` in `oric` with E6xxx codes
- [x] `EvalCounters` struct and `ModeState` integration
- [x] `EvalErrorSnapshot` preserves full error context at Salsa boundary
- [x] `snapshot_to_diagnostic()` enriches backtraces with file:line:col via `LineOffsetTable`
- [x] `--profile` CLI flag with counter report on stderr
- [x] Counter increments wired at `eval_inner()`, `eval_call()`, `eval_method_call()`, `eval_match()`
- [x] `./test-all.sh` passes (8448 tests, 0 failures)

**Deferred to Section 07:**
- [ ] Remove `ControlFlow`/`propagated_value` from `EvalError` (→ `ControlAction`) — pervasive refactor, fits Section 07's dispatch rewrite
- [ ] Counter wiring in new `eval_canon()` dispatch with `CanExpr`-aware granularity

**Exit Criteria:** Runtime errors include typed categories, call stack backtraces, and context notes. `--profile` prints evaluation statistics. Errors are as informative as compile-time diagnostics. ✅ All criteria met except `ControlAction` refactor (deferred).
