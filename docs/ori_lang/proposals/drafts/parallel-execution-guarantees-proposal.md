# Proposal: Parallel Execution Guarantees

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Compiler, runtime, concurrency model

---

## Summary

This proposal specifies the execution guarantees for the `parallel` pattern, addressing ambiguity around ordering, concurrency limits, resource exhaustion, and partial completion semantics.

---

## Problem Statement

The spec states that `parallel` "may execute tasks in parallel" but leaves critical questions unanswered:

1. **Execution order**: Are tasks started in list order? Completed in any order?
2. **Concurrency limits**: What happens when `max_concurrent` is exceeded?
3. **Resource exhaustion**: What if the system cannot spawn more tasks?
4. **Result ordering**: How are results ordered in the output?
5. **Partial completion**: What happens if some tasks fail?

---

## Parallel Pattern Specification

### Syntax Recap

```ori
parallel(
    tasks: [() -> T uses Async],
    max_concurrent: int = unlimited,
    timeout: Duration = none,
) -> [Result<T, E>]
```

### Execution Order Guarantees

**Start Order**: Tasks are **started in list order**.

```ori
parallel(tasks: [task_a, task_b, task_c])
// task_a starts first, then task_b, then task_c
// (subject to max_concurrent constraint)
```

**Completion Order**: Tasks may **complete in any order**.

```ori
// If task_b is faster than task_a:
// task_b may complete before task_a
// This is expected concurrent behavior
```

**Result Order**: Results are returned **in original task order**, not completion order.

```ori
let results = parallel(tasks: [slow, fast, medium])
// results[0] = result of slow  (first task)
// results[1] = result of fast  (second task)
// results[2] = result of medium (third task)
// Even though fast completed first
```

### Concurrency Limits

The `max_concurrent` parameter limits simultaneous execution:

```ori
parallel(
    tasks: hundred_tasks,
    max_concurrent: 10,
)
// At most 10 tasks run simultaneously
// When one completes, the next pending task starts
```

**Semantics**:
- Tasks are queued in list order
- When a slot opens (task completes), the next queued task starts
- Tasks wait in the queue, not in a busy loop

**Default**: When `max_concurrent` is not specified, there is no limit (all tasks may run simultaneously).

### Resource Exhaustion

If the runtime cannot allocate resources for a task (memory, task handles, etc.):

1. The specific task fails with `Err(ResourceExhausted)`
2. Other tasks continue executing
3. The pattern does NOT panic
4. Result array contains the error for that task

```ori
let results = parallel(tasks: thousand_heavy_tasks)
// If task 500 can't be allocated:
// results[500] = Err(ResourceExhausted)
// Other tasks still run
```

### Timeout Behavior

When `timeout` expires:

1. Incomplete tasks are cancelled (see nursery-cancellation-proposal)
2. Results for cancelled tasks are `Err(CancellationError { reason: Timeout })`
3. Completed results are preserved

```ori
let results = parallel(
    tasks: [fast_task, slow_task, medium_task],
    timeout: 1s,
)
// If slow_task takes 5s:
// results[0] = Ok(fast_result)     // completed
// results[1] = Err(Timeout)        // cancelled
// results[2] = Ok(medium_result)   // completed
```

### Error Handling

**Default behavior**: Errors do not stop other tasks (equivalent to `CollectAll` for nurseries).

```ori
let results = parallel(tasks: [success, failure, success])
// results[0] = Ok(...)
// results[1] = Err(...)
// results[2] = Ok(...)
// All three tasks run
```

For early termination on error, use `nursery` with appropriate error mode.

### Empty Task List

```ori
parallel(tasks: [])
// Returns: []
// No tasks spawned, returns immediately
```

---

## Execution Model

### Task Scheduling

The runtime schedules tasks according to these rules:

1. **Fair scheduling**: No task is starved; all eventually get CPU time
2. **No priority**: All tasks have equal priority (no priority inversion)
3. **Work stealing**: The runtime may move tasks between execution contexts for load balancing

### Progress Guarantee

Every non-blocked task makes progress. A task is blocked only when:
- Waiting on a channel operation
- Waiting on another async operation
- Explicitly yielding

Compute-bound tasks do not block other tasks indefinitely — the runtime ensures fair interleaving at suspension points.

### Memory Model

Tasks in `parallel` observe the same memory model as nursery tasks:
- No shared mutable state
- Values are moved into tasks (ownership transfer)
- Captured bindings must be `Sendable`

---

## Examples

### Basic Parallel Execution

```ori
@fetch_all (urls: [str]) -> [Result<Response, Error>] uses Async =
    parallel(
        tasks: urls.map(url -> () -> fetch(url)),
        max_concurrent: 10,
        timeout: 30s,
    )
```

### Parallel with Index Tracking

```ori
@process_with_index (items: [Item]) -> [Result<Output, Error>] uses Async =
    parallel(
        tasks: items
            .iter()
            .enumerate()
            .map((i, item) -> () -> process(index: i, item: item))
            .collect(),
    )
// Results maintain original order
```

### Aggregating Results

```ori
@parallel_sum (batches: [[int]]) -> int uses Async = run(
    let results = parallel(
        tasks: batches.map(batch -> () -> batch.fold(0, (a, b) -> a + b)),
    ),
    results
        .filter(r -> r.is_ok())
        .map(r -> r.unwrap())
        .fold(0, (a, b) -> a + b),
)
```

### Handling Partial Failures

```ori
@best_effort_fetch (urls: [str]) -> [Response] uses Async = run(
    let results = parallel(
        tasks: urls.map(url -> () -> fetch(url)),
        timeout: 10s,
    ),
    // Keep only successful responses
    results.filter(r -> r.is_ok()).map(r -> r.unwrap()).collect(),
)
```

---

## Comparison with Related Patterns

| Pattern | Error Handling | Return Type | Use Case |
|---------|---------------|-------------|----------|
| `parallel` | Collect all | `[Result<T, E>]` | Independent tasks, want all results |
| `spawn` | Fire and forget | `void` | Side effects, no results needed |
| `nursery` | Configurable | `[Result<T, E>]` | Complex control over cancellation |

### When to Use Each

**Use `parallel` when**:
- You have independent tasks
- You want all results (successes and failures)
- Simple fan-out/fan-in pattern

**Use `nursery` when**:
- You need early termination on error
- You need explicit cancellation control
- Tasks have dependencies or need coordination

**Use `spawn` when**:
- You don't need results
- Fire-and-forget side effects
- Background logging, metrics, etc.

---

## Spec Changes Required

### Update `10-patterns.md`

Add detailed section on `parallel`:
- Execution order guarantees
- Result ordering specification
- Concurrency limit behavior
- Resource exhaustion handling
- Timeout interaction

### Add Examples

Add examples showing:
- Result ordering
- max_concurrent usage
- Timeout behavior
- Partial failure handling

---

## Summary

| Aspect | Guarantee |
|--------|-----------|
| Start order | Tasks start in list order |
| Completion order | Any order (concurrent) |
| Result order | Same as task list order |
| max_concurrent | Queued execution, FIFO |
| Resource exhaustion | Per-task error, others continue |
| Timeout | Incomplete tasks cancelled |
| Error handling | All tasks run (CollectAll behavior) |
| Empty list | Returns `[]` immediately |
| Memory | Same as nursery (no shared mutable state) |
