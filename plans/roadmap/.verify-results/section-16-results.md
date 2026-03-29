# Section 16: Async Support -- Verification Results

**Verified**: 2026-03-28
**Methodology**: Searched codebase for async capability declarations, structured concurrency types, channel implementations, async error traces. Checked IR, parser, type checker, evaluator, LLVM codegen, tests.
**Sections verified**: 16.1-16.5
**Total items**: 17

## Summary

| Subsection | Items | Done | Partial | Not Started | Notes |
|-----------|-------|------|---------|-------------|-------|
| 16.1 Async via Capability | 2 | 0 | 0 | 2 | `uses Async` parses but no semantic implementation |
| 16.2 Structured Concurrency | 2 | 0 | 0 | 2 | No implementation |
| 16.3 Concurrency Patterns | 3 | 0 | 1 | 2 | parallel/timeout/spawn have eval stubs; channels are stubs |
| 16.4 Async Error Traces | 4 | 0 | 0 | 4 | No implementation |
| 16.5 Completion Checklist | 6 | 0 | 0 | 6 | N/A |

**Hidden implementations found**: 1 partial

## Detailed Findings

### 16.1 Async via Capability

- [ ] `uses Async` declaration -- [not-started]
  - Parser recognizes `uses Async` in function signatures (`CapabilityRef` in `ori_ir/src/ast/items/function.rs`). The `uses` clause parsing works (confirmed by `ori_parse/src/tests/parser.rs`).
  - However, NO semantic checking of `Async` capability exists in `ori_types`. The `ori_types/src/flags/mod.rs` mentions `async` only in flag names, not as capability enforcement.
  - No async runtime, no coroutine generation, no LLVM codegen.
  - No tests in `tests/spec/async/`.

- [ ] Sync vs async distinction -- [not-started]
  - No distinction mechanism implemented. Functions with `uses Async` are parsed but treated identically to sync functions.

### 16.2 Structured Concurrency

- [ ] Structured concurrency -- [not-started]
  - No structured concurrency implementation exists.

- [ ] No shared mutable state -- [not-started]
  - No shared mutability detection for async contexts.

### 16.3 Concurrency Patterns

- [ ] `parallel` pattern -- [partial]
  - IR: `FunctionExpKind::Parallel` variant exists in `ori_ir/src/ast/patterns/exp/mod.rs`.
  - Parser: `parallel` is a lexer keyword (`TokenKind::Parallel`), fully parsed.
  - Evaluator: **stub implementation** in `function_exp.rs:219-237` -- executes tasks sequentially, returns `[Result<T, E>]`. Logs warning "pattern 'parallel' is a stub."
  - Type checker: Some type checking exists for the `parallel` pattern arguments.
  - LLVM: No codegen.
  - **Status**: Partially implemented (parse + stub eval), but no actual concurrency.

- [ ] `timeout` pattern -- [partial]
  - IR: `FunctionExpKind::Timeout` exists.
  - Parser: `timeout` is a lexer keyword, fully parsed.
  - Evaluator: **stub** in `function_exp.rs:249-253` -- executes the operation without timeout enforcement, wraps in `Ok()`. Logs warning "pattern 'timeout' is a stub."
  - LLVM: No codegen.
  - **Status**: Partially implemented (parse + stub eval), no actual timeout enforcement.

- [ ] Channels -- [partial]
  - IR: `FunctionExpKind::Channel`, `ChannelIn`, `ChannelOut`, `ChannelAll` variants exist. `BuiltinType::Channel` exists.
  - Evaluator: **stub** in `function_exp.rs:267-275` -- returns `Value::Void` with warning "channels are not yet implemented."
  - No `Producer<T>`/`Consumer<T>` types implemented.
  - No channel runtime.
  - **Status**: Partially implemented (IR + stub eval), channels are pure stubs.

### 16.4 Async Error Traces

All 4 items are [not-started]. No async trace implementation, no task boundary markers, no parallel/nursery trace preservation.

### 16.5 Completion Checklist

All 6 items are [not-started].

## Accuracy Assessment

The section's `not-started` status is **slightly inaccurate** -- some concurrency patterns (parallel, timeout, spawn, channels) have IR definitions and stub evaluator implementations. However, no actual async/concurrency functionality exists (everything runs synchronously). The stubs are scaffolding, not implementations. The `not-started` status is reasonable as a top-level characterization.

**Recommended status**: not-started (stubs do not constitute meaningful progress toward async support)
