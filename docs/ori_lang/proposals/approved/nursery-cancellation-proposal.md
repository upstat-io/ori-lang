# Proposal: Nursery Cancellation Semantics

**Status:** Approved
**Approved:** 2026-01-30
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Compiler, runtime, concurrency model

---

## Summary

This proposal specifies the exact semantics of task cancellation within nurseries, addressing gaps in the current spec around what happens when tasks are cancelled due to errors, timeouts, or early termination.

---

## Problem Statement

The spec defines `NurseryErrorMode` with three modes (`CancelRemaining`, `CollectAll`, `FailFast`) but leaves critical questions unanswered:

1. **How is cancellation signaled?** Is it cooperative or preemptive?
2. **What happens to in-flight work?** Do tasks complete their current operation or stop immediately?
3. **Are there cancellation checkpoints?** Where does a task observe cancellation?
4. **What about cleanup?** Do destructors run for cancelled tasks?
5. **What is returned for cancelled tasks?** Error? Partial result? Nothing?

---

## Cancellation Model

### Cooperative Cancellation

Ori uses **cooperative cancellation**. A cancelled task:

1. Is **marked** for cancellation
2. Continues executing until it reaches a **cancellation checkpoint**
3. At the checkpoint, observes the cancellation and terminates
4. Runs cleanup/destructors during termination

This differs from preemptive cancellation (immediate termination) which can leave resources in inconsistent states.

### Cancellation Checkpoints

A task observes cancellation at these points:

| Checkpoint | Description |
|------------|-------------|
| **Suspension points** | Async calls, channel operations |
| **Loop iterations** | Start of each `for` or `loop` iteration |
| **Pattern entry** | Entry to `run`, `try`, `match`, `parallel`, `nursery` |

Between checkpoints, a task executes atomically with respect to cancellation.

### Cancellation Behavior

When a task reaches a checkpoint while marked for cancellation:

1. The current expression evaluates to a **cancellation error** (`Result<T, CancellationError>`)
2. Normal unwinding occurs — destructors run, `run` blocks exit early
3. The task terminates with `Err(CancellationError)`

---

## Error Mode Semantics

### FailFast

```ori
nursery(body: n -> ..., on_error: FailFast)
```

On first error (task failure or panic):
1. **All other tasks** are marked for cancellation
2. The nursery waits for all tasks to reach checkpoints and terminate
3. Returns immediately after all tasks terminate
4. Result contains results collected so far (errors for cancelled tasks)

```ori
// Example: FailFast behavior
let results = nursery(
    body: n -> run(
        n.spawn(task: () -> slow_success()),     // Will be cancelled
        n.spawn(task: () -> immediate_fail()),   // Fails first
        n.spawn(task: () -> medium_success()),   // Will be cancelled
    ),
    on_error: FailFast,
)
// results: [Err(CancellationError), Err(original_error), Err(CancellationError)]
// Order matches spawn order
```

### CancelRemaining

```ori
nursery(body: n -> ..., on_error: CancelRemaining)
```

On first error:
1. **Pending tasks** (not yet started) are cancelled immediately
2. **Running tasks** continue to completion
3. Nursery waits for running tasks
4. Returns all results (both successes and errors)

```ori
// Example: CancelRemaining behavior
let results = nursery(
    body: n -> run(
        n.spawn(task: () -> slow_success()),     // Continues running
        n.spawn(task: () -> immediate_fail()),   // Fails
        n.spawn(task: () -> queued_task()),      // Cancelled (was pending)
    ),
    on_error: CancelRemaining,
    max_concurrent: 2,  // Only 2 run at a time
)
// results: [Ok(success), Err(error), Err(CancellationError)]
```

### CollectAll

```ori
nursery(body: n -> ..., on_error: CollectAll)
```

Errors do not trigger cancellation:
1. All tasks run to completion regardless of errors
2. Nursery waits for all tasks
3. Returns all results

```ori
// Example: CollectAll behavior
let results = nursery(
    body: n -> run(
        n.spawn(task: () -> success_1()),
        n.spawn(task: () -> fail_1()),
        n.spawn(task: () -> success_2()),
        n.spawn(task: () -> fail_2()),
    ),
    on_error: CollectAll,
)
// results: [Ok(r1), Err(e1), Ok(r2), Err(e2)]
// All tasks complete
```

---

## Timeout Semantics

```ori
nursery(body: n -> ..., timeout: 5s)
```

When timeout expires:
1. All incomplete tasks are marked for cancellation
2. Tasks reach checkpoints and terminate (same as error-triggered cancellation)
3. Nursery waits for all cancellation to complete
4. Returns results collected so far

Timeout cancellation interacts with error modes:
- With `FailFast`: Timeout behaves same as error
- With `CancelRemaining`: Running tasks continue, pending cancelled
- With `CollectAll`: Timeout still cancels (otherwise nursery would never exit)

---

## Cancellation Error Type

```ori
type CancellationError = {
    reason: CancellationReason,
    task_id: int,
}

type CancellationReason =
    | Timeout
    | SiblingFailed
    | NurseryExited
    | ExplicitCancel
    | ResourceExhausted
```

Cancelled tasks return `Err(CancellationError)` in the results array.

---

## Cleanup Guarantees

### Destructor Execution

When a task is cancelled:
1. Stack unwinding occurs from the cancellation checkpoint
2. Destructors run for all values in scope
3. Cleanup is **guaranteed** to complete before the task terminates

```ori
@task_with_cleanup () -> void uses Suspend = run(
    let resource = acquire_resource(),  // Has destructor
    do_work(),                           // <- Cancellation observed here
    // resource's destructor runs even if cancelled
)
```

### No Partial Cleanup

A task cannot be forcibly terminated during destructor execution. If a destructor itself makes async calls, those are cancellation checkpoints, but the destructor completes before the task terminates.

---

## API for Explicit Cancellation

### Checking Cancellation Status

Tasks can check if they've been cancelled:

```ori
@long_running_task () -> Result<Data, Error> uses Suspend = run(
    let result = [],
    for item in large_dataset do run(
        if is_cancelled() then break Err(CancellationError { ... }),
        result = result + [process(item)],
    ),
    Ok(result),
)
```

`is_cancelled()` is a built-in function available in async contexts that returns `bool`.

### Cancellation-Aware Loops

The `for` loop automatically checks cancellation at each iteration when inside an async context:

```ori
// Equivalent to explicit check above
@long_running_task () -> [Data] uses Suspend =
    for item in large_dataset yield process(item)
    // Automatically exits with CancellationError if cancelled
```

---

## Edge Cases

### Nested Nurseries

When an outer nursery cancels a task containing an inner nursery:
1. The inner nursery receives cancellation
2. Inner nursery cancels its tasks per its error mode
3. Inner nursery completes (with cancellation results)
4. Outer task then completes

```ori
@outer () -> void uses Suspend = nursery(
    body: n -> n.spawn(task: () -> inner()),
    timeout: 5s,  // If this triggers...
)

@inner () -> void uses Suspend = nursery(
    body: n -> run(
        n.spawn(task: () -> task_a()),  // ...these are also cancelled
        n.spawn(task: () -> task_b()),
    ),
)
```

### Panic During Cancellation

If a destructor panics during cancellation unwinding:
1. The panic is captured
2. Unwinding continues for remaining destructors
3. The task terminates with the panic as its result (not CancellationError)

---

## Examples

### Graceful Shutdown Pattern

```ori
@fetch_all (urls: [str]) -> [Result<Response, Error>] uses Suspend = nursery(
    body: n -> for url in urls do n.spawn(task: () -> fetch(url)),
    on_error: CollectAll,
    timeout: 30s,
)
// Returns partial results on timeout, all results if fast enough
```

### First Success Pattern

```ori
@first_success<T> (tasks: [() -> T uses Suspend]) -> Result<T, Error> uses Suspend = run(
    let results = nursery(
        body: n -> for task in tasks do n.spawn(task: task),
        on_error: CancelRemaining,
    ),
    results.find(r -> r.is_ok()).unwrap_or(Err(AllFailed)),
)
```

### Explicit Cancellation Check

```ori
@batch_process (items: [Item]) -> [Result<Output, Error>] uses Suspend = nursery(
    body: n -> for item in items do n.spawn(task: () -> run(
        // Heavy computation
        let partial = phase1(item),

        // Check if we should continue
        if is_cancelled() then Err(CancellationError { ... })?,

        // More heavy computation
        phase2(partial),
    )),
)
```

---

## Spec Changes Required

### Update `10-patterns.md`

Add detailed cancellation semantics to nursery pattern section.

### Update Prelude

Add:
- `CancellationError` type
- `CancellationReason` type
- `is_cancelled()` built-in function

### New Section in Concurrency Spec

Add "Cancellation" section covering:
- Cooperative cancellation model
- Checkpoint locations
- Error mode behaviors
- Cleanup guarantees

---

## Summary

| Aspect | Specification |
|--------|--------------|
| Model | Cooperative (not preemptive) |
| Checkpoints | Suspension points, loop iterations, pattern entry |
| FailFast | Cancel all on first error |
| CancelRemaining | Cancel pending, let running complete |
| CollectAll | No cancellation (except timeout) |
| Timeout | Always cancels incomplete tasks |
| Cleanup | Guaranteed destructor execution |
| API | `is_cancelled()` for explicit checking |
| Result type | `CancellationError` with reason |
