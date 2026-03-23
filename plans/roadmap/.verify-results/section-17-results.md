# Section 17: Concurrency Extended -- Verification Results

**Verified**: 2026-03-19
**Section status**: not-started (0/358 items)
**Verdict**: Section is genuinely not started. All items correctly marked `[ ]`.

---

## Spot-Checked Items (10 items)

### 17.0 -- Closure capture-by-value semantics
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: No `tests/spec/closures/` directory exists. No capture analysis in `ori_types/src/check/closure/` (that directory does not exist). Closures work in Ori (lambdas captured by value is the language semantics), but there is no *explicit capture analysis* or *verification pass* as described in this item.
- **Classification**: VERIFIED -- capture semantics are implicit in the language design (value semantics everywhere), but no explicit capture analysis pass or tests exist. The roadmap item is about formalizing verification, not basic closure support.

### 17.1 -- Sendable trait (auto-derive)
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: `Sendable` is registered as a well-known trait in `ori_types/src/check/well_known/mod.rs` (mapped to `tb::SENDABLE`). It appears in trait sets for Duration and Size types. The trait bit `SENDABLE = 7` exists in `trait_set.rs`. However: no auto-derivation logic exists, no channel boundary checks enforce it, no `ori_eval` or `ori_llvm` code references Sendable, and no stdlib definition exists in `library/std/`.
- **Classification**: VERIFIED -- Sendable is registered as a trait name/bit in the well-known trait system, but has no implementation (no auto-derivation, no enforcement, no runtime support). This is infrastructure scaffolding only, not a working feature.

### 17.1 -- Compiler error for non-Sendable in channel context
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: Channels are fully stubbed (see Section 16 results). No Sendable enforcement exists anywhere.
- **Classification**: VERIFIED -- genuinely not started.

### 17.2 -- Producer/Consumer types
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: The registry in `ori_registry` has Producer/Consumer references in `defs/channel/tests.rs` and method definitions. The parser can parse `Producer<T>` and `Consumer<T>` as generic types. However, there are no IR type variants for these, no type checker registration (beyond method names in the registry), and no runtime value representations.
- **Classification**: VERIFIED -- registry has method definitions as documentation/scaffolding, but no actual type system or runtime implementation.

### 17.3 -- channel<T>() exclusive channel
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: `channel.rs` in `ori_patterns` returns `Err("channel patterns are not yet implemented")`.
- **Classification**: VERIFIED -- explicitly stubbed.

### 17.5 -- nursery pattern parsing
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: `nursery` appears only in `ori_fmt` packing code (formatting strategy for `nursery()` calls). No `NurseryPattern` struct in `ori_patterns`. The `ori_ir` AST mentions `nursery` as a `FunctionExpKind` variant, but no implementation exists in the evaluator or type checker beyond formatting.
- **Classification**: VERIFIED -- formatting support exists (how to pretty-print nursery calls), but no runtime, type checking, or parsing of nursery semantics.

### 17.6 -- Parallel execution guarantees (start order, result order)
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: `ParallelPattern` in `ori_patterns/src/parallel/mod.rs` uses `std::thread` with `mpsc` channels and has basic thread-based execution. It does process tasks sequentially in the result collection (indexed results via `results[idx]`). However, this is a synchronous thread implementation, not the async parallel with formal guarantees described here.
- **Classification**: VERIFIED -- the thread-based parallel pattern has some ordering behavior but no formal guarantees, no `Suspend` capability requirement, no timeout integration, and the evaluator stub runs them sequentially anyway.

### 17.7 -- Cancellation semantics (cooperative model)
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: No `CancellationError`, `CancellationReason`, or `is_cancelled()` found anywhere in the compiler codebase. No cancellation checkpoint insertion logic.
- **Classification**: VERIFIED -- genuinely not started.

### 17.8 -- timeout returns Result<T, CancellationError>
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: `TimeoutPattern` returns `Ok(Value::ok(value))` or `Ok(Value::err(Value::string(error_msg)))` -- using string errors, not `CancellationError`. No `CancellationError` type exists in the codebase.
- **Classification**: VERIFIED -- timeout returns string errors, not the specified `CancellationError` type.

### 17.8 -- spawn pattern fire-and-forget
- **Status**: `[ ]` (unchecked)
- **Codebase evidence**: `SpawnPattern` in `ori_patterns/src/spawn/mod.rs` exists with thread-based implementation using `std::thread::spawn`. It does implement fire-and-forget semantics (discards errors, returns void). However, the evaluator stub in `function_exp.rs` runs tasks synchronously with a warning.
- **Classification**: VERIFIED -- spawn pattern has a thread-based implementation in `ori_patterns`, but the evaluator stub bypasses it. No `Suspend` capability enforcement, no `max_concurrent` validation at type level.

---

## Summary

| Classification | Count |
|----------------|-------|
| VERIFIED       | 10    |
| NEEDS TESTS    | 0     |
| WEAK TESTS     | 0     |
| WRONG TEST     | 0     |
| STALE TEST     | 0     |
| REGRESSION     | 0     |
| BUG FOUND      | 0     |

**Conclusion**: All 358 items are genuinely not started. Some infrastructure exists:
- `Sendable` is registered as a well-known trait bit but has no implementation
- `parallel`/`timeout`/`spawn` have thread-based pattern implementations in `ori_patterns`, but the evaluator runs stubs that bypass them
- Channel patterns are explicitly stubbed with error returns
- `nursery` has formatting support but no runtime
- No cancellation, cooperative model, or async context tracking exists

Section status `not-started` is accurate. The existing infrastructure (trait bits, pattern stubs, formatting) represents scaffolding, not functional implementation.
