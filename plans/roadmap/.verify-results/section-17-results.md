# Section 17: Concurrency Extended -- Verification Results

**Verified**: 2026-03-28
**Methodology**: Searched codebase for Sendable trait implementation, Producer/Consumer types, nursery pattern, channel constructors, cancellation types, ownership transfer on send, parallel execution guarantees, timeout/spawn semantics. Checked IR, parser, type checker, evaluator, LLVM codegen, stdlib, tests.
**Sections verified**: 17.0-17.9
**Total items**: 78

## Summary

| Subsection | Items | Done | Partial | Not Started | Notes |
|-----------|-------|------|---------|-------------|-------|
| 17.0 Task/Async Context | 10 | 0 | 1 | 9 | Closure capture-by-value exists in eval |
| 17.1 Sendable Trait | 5 | 0 | 1 | 4 | Registered in well-known traits, not enforced |
| 17.2 Role-Based Channels | 5 | 0 | 0 | 5 | No Producer/Consumer types |
| 17.3 Channel Constructors | 5 | 0 | 1 | 4 | IR + stub eval only |
| 17.4 Ownership Transfer | 3 | 0 | 0 | 3 | No move semantics on send |
| 17.5 nursery Pattern | 8 | 0 | 0 | 8 | Not in IR or eval |
| 17.6 Parallel Execution | 6 | 0 | 1 | 5 | Stub sequential execution only |
| 17.7 Cancellation | 8 | 0 | 0 | 8 | No cancellation types |
| 17.8 Timeout/Spawn | 12 | 0 | 2 | 10 | Stub eval only |
| 17.9 Completion Checklist | 16 | 0 | 0 | 16 | N/A |

**Hidden implementations found**: 5 partial

## Detailed Findings

### 17.0 Task and Async Context Definitions

- [ ] Task definition and isolation model -- [not-started]. No task abstraction exists.
- [ ] Async context tracking -- [not-started]. No async context concept in type checker or evaluator.
- [ ] Suspension point tracking -- [not-started]. No suspension analysis.
- [ ] @main uses Async requirement -- [not-started]. No enforcement.
- [ ] Async propagation checking -- [not-started]. No propagation checking.
- [ ] Closure capture-by-value semantics -- [partial]
  - The evaluator already implements capture-by-value for closures via `environment/mod.rs` `capture()` method. Closures capture bindings by value (clone). This is the Ori design -- no capture-by-reference.
  - Type checker has capture-related code in `ori_types/src/infer/expr/blocks.rs` and `sequences.rs`.
  - However, the formal verification described in the plan (dedicated `capture.rs` test file, `capture_timing.ori` spec test) does NOT exist.
  - **Status**: The semantics are implemented; the testing/verification infrastructure is not.
- [ ] Closure type inference and coercion -- [not-started]. No dedicated closure type tests.
- [ ] Captured binding immutability check -- [not-started]. No dedicated check exists.
- [ ] Task capture ownership transfer -- [not-started]. No task concept.
- [ ] Atomic reference counting for cross-task values -- [not-started]. `ori_rt` uses non-atomic refcounting.

### 17.1 Sendable Trait

- [ ] Add Sendable marker trait -- [partial]
  - `Sendable` IS registered in the well-known trait system: `ori_types/src/check/well_known/mod.rs:108` maps "Sendable" to `tb::SENDABLE` (bit 7).
  - `TraitSet` includes `SENDABLE` in the trait sets for Duration and Size types (`trait_set.rs:176,187`).
  - `ori_types/src/infer/expr/calls/traits.rs:110,126` -- Sendable appears in trait lists for object safety or similar checks.
  - However, NO auto-implementation logic exists. No field-recursive analysis. No enforcement at channel/spawn boundaries.
  - **Status**: Partially registered as a trait name, but not semantically implemented.
- [ ] Auto-implementation for primitives -- [not-started]. No auto-impl logic.
- [ ] Auto-implementation for compound types -- [not-started]. No recursive field checking.
- [ ] Closure capture analysis for Sendable -- [not-started].
- [ ] Compiler error for non-Sendable in channel context -- [not-started].

### 17.2 Role-Based Channel Types

All 5 items are [not-started]. No `Producer<T>`, `Consumer<T>`, `CloneableProducer<T>`, `CloneableConsumer<T>` types exist in the type system. The `BuiltinType::Channel` exists but is the old undifferentiated channel type, not the role-based split.

### 17.3 Channel Constructors

- [ ] `channel<T>()` exclusive -- [partial]
  - `FunctionExpKind::Channel` exists in IR. Parser recognizes it. Evaluator has a stub that returns `Value::Void` with "channels are not yet implemented" warning. No actual channel runtime.
- [ ] `channel_in<T>()` fan-in -- [partial] (same stub pattern as above)
- [ ] `channel_out<T>()` fan-out -- [partial] (same stub pattern)
- [ ] `channel_all<T>()` broadcast -- [partial] (same stub pattern)
- [ ] Deprecate old Channel<T> type -- [not-started].

Note: All four channel constructors share the same stub code path in `function_exp.rs:267-275`.

### 17.4 Ownership Transfer on Send

All 3 items are [not-started]. No move semantics on channel send.

### 17.5 nursery Pattern

All 8 items are [not-started].
- `nursery` is NOT a `FunctionExpKind` variant -- it does not exist in `ori_ir/src/ast/patterns/exp/mod.rs`.
- The formatter (`ori_fmt/src/packing/construct.rs`) recognizes `ConstructKind::Nursery` for formatting purposes, but this is formatting scaffolding only, not semantic implementation.
- No `Nursery` type, no `NurseryErrorMode` sum type, no nursery evaluation.
- No tests.

### 17.6 Parallel Execution Guarantees

- [ ] Start order guarantee -- [partial]
  - The stub implementation in `function_exp.rs:219-237` iterates tasks in list order, which trivially satisfies start order. But this is sequential execution, not parallel.
- [ ] Result order guarantee -- [not-started] in the concurrency sense. The sequential stub returns results in order by construction.
- [ ] `max_concurrent` parameter -- [not-started]. Not parsed or handled.
- [ ] `timeout` parameter -- [not-started]. Not handled in parallel.
- [ ] Resource exhaustion handling -- [not-started].
- [ ] Empty task list handling -- [not-started]. The stub would fail (expects list, no empty check).

### 17.7 Nursery Cancellation Semantics

All 8 items are [not-started]. No `CancellationError`, `CancellationReason` types. No `is_cancelled()` builtin. No cooperative cancellation model.

### 17.8 Timeout and Spawn Pattern Semantics

- [ ] `timeout(op:, after:)` return type -- [partial]
  - Stub exists: executes operation, wraps in `Ok()`, ignores timeout. No `CancellationError` return.
- [ ] Cooperative cancellation on timeout -- [not-started].
- [ ] Timeout cancellation checkpoints -- [not-started].
- [ ] Nested timeout support -- [not-started].
- [ ] Error E1010 -- [not-started].
- [ ] `spawn(tasks:, max_concurrent:)` -- [partial]
  - Stub exists: executes tasks sequentially, ignores errors, returns `Void`. No actual fire-and-forget.
- [ ] Fire-and-forget semantics -- [not-started] (stub discards errors, but not via proper async).
- [ ] Task escapes spawning scope -- [not-started].
- [ ] `max_concurrent` parameter -- [not-started].
- [ ] Resource exhaustion handling -- [not-started].
- [ ] Tasks cancelled on program exit -- [not-started].
- [ ] Error E1011 -- [not-started].

### 17.9 Completion Checklist

All 16 items are [not-started].

## Accuracy Assessment

The section's `not-started` status is **accurate**. While some infrastructure scaffolding exists:
- `Sendable` is registered as a well-known trait name (but not enforced)
- Channel constructors have IR variants and eval stubs (but return Void)
- Parallel/timeout/spawn have IR variants and eval stubs (but run synchronously)
- Closure capture-by-value works in the interpreter (but isn't formally tested for concurrency)
- `nursery` formatting exists (but no IR, no eval)

None of these constitute meaningful progress toward concurrency. The stubs are synchronous placeholders.

**Recommended status**: not-started
