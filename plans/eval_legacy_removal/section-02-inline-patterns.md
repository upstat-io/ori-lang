---
section: "02"
title: Stub Remaining FunctionExp Patterns in Canonical Path
status: complete
goal: Add loud placeholder stubs for Cache, Parallel, Spawn, Timeout, With in eval_can_function_exp so the legacy PatternExecutor is no longer reachable
sections:
  - id: "02.1"
    title: Audit Current Pattern Status
    status: complete
  - id: "02.2"
    title: Stub All Five Patterns
    status: complete
  - id: "02.3"
    title: Completion Checklist
    status: complete
---

# Section 02: Stub Remaining FunctionExp Patterns in Canonical Path

**Status:** Complete (2026-02-10)
**Goal:** Add explicit, loud placeholder stubs for Cache, Parallel, Spawn, Timeout, With in `eval_can_function_exp` (can_eval.rs:956-1012). These are NOT real implementations — they are honest stubs that evaluate args and emit `tracing::warn!` so nobody mistakes them for working features.

**Context:** The canonical path already handles 6 patterns inline:
- **Eager (pre-evaluated args):** Print, Panic, Todo, Unreachable
- **Lazy (special control flow):** Catch, Recurse

Five patterns currently fall through to a legacy `PatternExecutor` error. The legacy implementations in `ori_patterns/src/` are themselves stubs:

| Pattern | Legacy behavior | Real? |
|---------|----------------|-------|
| Cache | Calls fn directly, no memoization | Stub — no caching |
| Parallel | `thread::scope` with max_concurrent | Most complete, but spec tests commented out |
| Spawn | `thread::scope` fire-and-forget, ignores max_concurrent | Partial stub |
| Timeout | Calls fn directly, ignores timeout duration | Stub — no timeout |
| With | Acquire/action/release RAII | Complete, but blocked on type checker |

**Strategy:** Instead of porting silent stubs that pretend to work, each canonical stub will:
1. Evaluate args via `self.eval_can()` (so the canonical path is self-contained)
2. Emit a **`tracing::warn!`** on every invocation: `"pattern 'cache' is a stub — no memoization is performed"`
3. Execute the minimal correct behavior (call the operation fn, return its result)
4. **NOT** silently swallow semantics (no pretending to cache, no pretending to timeout)

This unblocks legacy removal. Real implementations are roadmap items.

**Reference files (legacy stubs):**
- `ori_patterns/src/cache.rs` — Cache stub
- `ori_patterns/src/parallel.rs` — Parallel (most complete)
- `ori_patterns/src/spawn.rs` — Spawn stub
- `ori_patterns/src/timeout.rs` — Timeout stub
- `ori_patterns/src/with_pattern.rs` — With (RAII, complete but blocked)

---

## 02.1 Audit Current Pattern Status

- [x] Read each pattern's implementation in `ori_patterns/src/` to confirm stub vs real:
  - `cache.rs` — confirmed: just calls operation, no memoization
  - `parallel.rs` — confirmed: thread::scope implementation, spec tests commented out
  - `spawn.rs` — confirmed: thread::scope fire-and-forget, ignores max_concurrent
  - `timeout.rs` — confirmed: just calls operation, ignores duration
  - `with_pattern.rs` — confirmed: acquire/action/release RAII
- [x] For each, document the minimal correct behavior to preserve
- [x] Verify no spec test currently exercises these patterns (all commented out / nonexistent)

---

## 02.2 Stub All Five Patterns

All 5 match arms added to `eval_can_function_exp` in `can_eval.rs`. Each follows the pattern: `tracing::warn!` → evaluate props → minimal correct behavior.

**Per-pattern stubs:**

- [x] **Cache** — Warns "no memoization". Evaluates `operation` arg. If callable, calls with no args; otherwise returns value directly.
- [x] **Parallel** — Warns "sequential execution". Evaluates `tasks` list, calls each sequentially, wraps results in `Ok`/`Err` (all-settled semantics). `max_concurrent` and `timeout` props evaluated but ignored.
- [x] **Spawn** — Warns "synchronous execution". Evaluates `tasks` list, calls each sequentially, discards results. Returns `Void`.
- [x] **Timeout** — Warns "no timeout enforcement". Pre-evaluates `operation`, wraps value in `Ok`. `after` prop evaluated but ignored.
- [x] **With** — Warns "stub resource management". Evaluates `acquire` → resource, `action` → function. Calls `action(resource)`. Always calls `release(resource)` if provided (RAII guarantee preserved even on error).

---

## 02.3 Completion Checklist

- [x] All 5 FunctionExpKind variants have match arms in `eval_can_function_exp`
- [x] Each stub emits `tracing::warn!` identifying itself as a stub
- [x] No stub silently pretends to implement real semantics
- [x] No FunctionExpKind variant falls through to legacy PatternExecutor
- [x] Args evaluated via `self.eval_can()`, not `PatternExecutor::eval(ExprId)`
- [x] `cargo c -p ori_eval` compiles
- [x] `cargo t -p ori_eval` passes
- [x] `cargo t -p oric` passes
- [x] `cargo st` passes (3052 passed, 0 failed, 58 skipped)
- [x] Debug assertions from Section 01 still pass

**Exit Criteria:** The `eval_can_function_exp` match is exhaustive for all FunctionExpKind variants. Each unimplemented pattern is an honest, loud stub. Zero calls from canonical evaluation to `PatternExecutor::eval(ExprId)`. Legacy PatternExecutor unreachable from canonical path.

**Future:** When roadmap items for these patterns are implemented, the stubs are replaced with real implementations directly in `can_eval.rs` — no legacy path involved.
