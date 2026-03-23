# Section 16: Async Support -- Verification Results

**Verified**: 2026-03-19
**Section status**: not-started (0/72 items)
**Verdict**: Section is genuinely not started. All items correctly marked `[ ]`.

---

## Spot-Checked Items (7 items)

### 16.1 -- `uses Async` declaration
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: Parser test `test_uses_clause_multiple_capabilities` in `compiler/ori_parse/src/tests/parser.rs` parses `uses FileSystem, Async` -- the `Async` keyword is parseable as a capability name. However, no `Async` capability type is registered in `ori_types`, no runtime handling exists in `ori_eval`, and no LLVM codegen exists.
- **Classification**: VERIFIED -- correctly marked incomplete. Parser accepts `Async` as a capability identifier (basic string parsing), but no actual async semantics exist.

### 16.1 -- Sync vs async distinction
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: No code distinguishing sync vs async contexts found anywhere. `Suspend` capability exists conceptually in spec but has no enforcement logic.
- **Classification**: VERIFIED -- genuinely not started.

### 16.2 -- Structured concurrency
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: No `tests/spec/async/` directory exists. No structured concurrency implementation in `ori_eval`. The evaluator's `function_exp.rs` has a stub for `parallel` that logs a warning: "pattern 'parallel' is a stub -- tasks are executed sequentially".
- **Classification**: VERIFIED -- genuinely not started. The evaluator's parallel stub runs tasks sequentially.

### 16.3 -- `parallel` pattern (async)
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: `compiler/ori_patterns/src/parallel/mod.rs` has a real `ParallelPattern` implementation with thread-based concurrency (using `std::thread`, `mpsc`, `Arc<Mutex>`). However, this is synchronous thread-parallelism, not async capability-based parallelism as described in Section 16. The evaluator stub in `function_exp.rs` runs tasks sequentially anyway.
- **Classification**: VERIFIED -- the parallel pattern has thread-based execution but no async integration. The Section 16 items about async-aware parallelism are genuinely incomplete.

### 16.3 -- `timeout` pattern (async)
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: `compiler/ori_patterns/src/timeout/mod.rs` has a `TimeoutPattern` that wraps the operation result in `Ok(value)` or `Err(error_string)` but does NOT enforce any actual timeout. Comment: "In the interpreter, timeout is not enforced." The evaluator stub says "pattern 'timeout' is a stub -- no timeout enforcement".
- **Classification**: VERIFIED -- timeout pattern exists as a passthrough wrapper only. No actual timeout enforcement.

### 16.3 -- Channels
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: `compiler/ori_patterns/src/channel.rs` is an explicit stub that returns `Err("channel patterns are not yet implemented")`. The registry routes `Channel`/`ChannelIn`/`ChannelOut`/`ChannelAll` to this stub.
- **Classification**: VERIFIED -- channels are explicitly stubbed out.

### 16.4 -- Async error traces
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: No async trace, task boundary marker, or parallel trace code found.
- **Classification**: VERIFIED -- genuinely not started.

---

## Summary

| Classification | Count |
|----------------|-------|
| VERIFIED       | 7     |
| NEEDS TESTS    | 0     |
| WEAK TESTS     | 0     |
| WRONG TEST     | 0     |
| STALE TEST     | 0     |
| REGRESSION     | 0     |
| BUG FOUND      | 0     |

**Conclusion**: All 72 items are genuinely not started. The parser can accept `uses Async` as a capability identifier, but no actual async runtime, type system integration, or codegen support exists. The `parallel`, `timeout`, and `spawn` patterns have synchronous implementations/stubs but lack any async integration. Section status `not-started` is accurate.
