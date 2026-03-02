---
title: "Concurrency Model"
description: "Clause 22: Ori Language Specification — Concurrency Model"
order: 22
section: "Language"
---

# 22 Concurrency model

Task-based concurrency with cooperative scheduling and explicit suspension points.

> **Grammar:** See [grammar.ebnf](grammar.md) § PATTERNS (parallel, spawn, nursery)

## 22.1 Tasks

A _task_ is an independent unit of concurrent execution.

### 22.1.1 Properties

A task has:

1. **Its own call stack** — function calls within a task are sequential
2. **Isolated mutable state** — no task can directly access another task's mutable bindings
3. **Cooperative scheduling** — tasks yield control at suspension points
4. **Bounded lifetime** — tasks are created within a scope and shall complete before that scope exits

Tasks are not threads. Multiple tasks may execute on the same OS thread, or the runtime may distribute tasks across OS threads. The runtime determines task-to-thread mapping.

### 22.1.2 Task creation

Tasks are created exclusively by concurrency patterns:

| Pattern | Creates Tasks | Description |
|---------|--------------|-------------|
| `parallel(tasks: [...])` | Yes | One task per list element |
| `spawn(tasks: [...])` | Yes | Fire-and-forget tasks |
| `nursery(body: n -> ...)` | Yes, via `n.spawn()` | Structured task spawning |
| Regular function call | No | Same task, same stack |

See [Patterns](15-patterns.md) for pattern definitions.

### 22.1.3 Task isolation

Each task has private mutable bindings. Bindings captured across task boundaries shall be `Sendable` and become inaccessible in the spawning scope:

```ori
@example () -> void uses Suspend = {
    let x = 0;
    parallel(
        tasks: [
            () -> {x = 1},  // error: cannot capture mutable binding across task boundary
            () -> {x = 2}
        ]
    )
}
```

See [Memory Model § Task Isolation](21-memory-model.md#2196-task-isolation) for isolation guarantees.

## 22.2 Suspending context

A _suspending context_ is a runtime environment that can execute suspending functions, schedule suspension and resumption, and manage multiple concurrent tasks.

### 22.2.1 Establishing suspending context

A suspending context is established by:

- **The runtime** — when `@main` declares `uses Suspend`, the runtime provides the initial suspending context
- **Concurrency patterns** — `parallel`, `spawn`, and `nursery` create nested suspending contexts for spawned tasks

A function declaring `uses Suspend` _requires_ a suspending context to execute — it does not establish one. The `Suspend` capability indicates the function may suspend, requiring a scheduler to manage resumption.

### 22.2.2 @main and Suspend

Programs using concurrency patterns shall have `@main` declare `uses Suspend`:

```ori
// Valid: main declares Suspend
@main () -> void uses Suspend = {
    parallel(tasks: [task_a(), task_b()])
}

// Invalid: main uses concurrency without Suspend
@main () -> void = {
    parallel(tasks: [task_a(), task_b()]);  // error: requires Suspend capability
}
```

The runtime establishes the suspending context when `@main uses Suspend` is declared.

## 22.3 Suspension points

A _suspension point_ is a location where a task may yield control to the scheduler.

### 22.3.1 Where suspension occurs

Suspension points occur ONLY at:

1. **Suspending function calls** — calling a function with `uses Suspend`
2. **Channel operations** — `send` and `receive` on channels
3. **Explicit yield** — within `parallel`, `spawn`, or `nursery` body evaluation

### 22.3.2 Where suspension cannot occur

Suspension NEVER occurs:

- In the middle of expression evaluation
- During non-suspending function execution
- At arbitrary points chosen by the runtime

This provides _predictable interleaving_ — atomicity boundaries are explicit.

## 22.4 Suspend propagation

A function that calls suspending code shall itself be suspending:

```ori
@caller () -> int uses Suspend =
    callee();  // OK: caller can suspend

@caller_sync () -> int =
    callee();  // error: callee uses Suspend but caller does not

@callee () -> int uses Suspend = ...;
```

The `Suspend` capability _propagates upward_ — callers of suspending functions shall declare `uses Suspend`.

## 22.5 Blocking in suspending context

A non-suspending function called from a suspending context executes synchronously, blocking that task but not other tasks:

```ori
@expensive_sync () -> int =
    heavy_math();  // Long computation, no suspension points

@main () -> void uses Suspend = {
    parallel(
        tasks: [
            () -> expensive_sync(),  // This task blocks during computation
            () -> other_work(),       // This task can run concurrently
        ]
    )
}
```

## 22.6 Capture and ownership

Task closures follow capture-by-value semantics. When a value is captured by a task closure, the original binding becomes inaccessible:

```ori
@capture_example () -> void uses Suspend = {
    let data = create_data();
    nursery(
        body: n -> {
            n.spawn(task: () -> process(data));  // data captured by value
            print(msg: data.field);  // error: data is no longer accessible
        }
    )
}
```

Bindings captured across task boundaries become inaccessible in the spawning scope to prevent data races. This uses existing capture-by-value behavior with an additional constraint.

### 22.6.1 Sendable requirement

Captured values shall implement `Sendable`:

```ori
@spawn_example () -> void uses Suspend = {
    let data = create_data();  // Data: Sendable
    nursery(
        body: n -> n.spawn(task: () -> process(data)),
    )
}
```

See [Types § Sendable Trait](08-types.md#814-sendable-trait) for `Sendable` definition.

### 22.6.2 Reference count atomicity

When values cross task boundaries, reference count operations are atomic. The compiler ensures thread-safe reference counting for values accessed by multiple tasks.

## 22.7 Cancellation

Task cancellation uses a _cooperative_ model. A cancelled task continues executing until it reaches a cancellation checkpoint, then terminates with cleanup.

### 22.7.1 Cancellation checkpoints

A task observes cancellation at these points:

| Checkpoint | Description |
|------------|-------------|
| Suspension points | Suspending calls, channel operations |
| Loop iterations | Start of each `for` or `loop` iteration |
| Pattern entry | Entry to `run`, `try`, `match`, `parallel`, `nursery` |

Between checkpoints, a task executes atomically with respect to cancellation.

### 22.7.2 Cancellation behavior

When a task reaches a checkpoint while marked for cancellation:

1. The current expression evaluates to `Err(CancellationError)`
2. Normal unwinding occurs — destructors run
3. The task terminates with `Err(CancellationError)`

### 22.7.3 CancellationError type

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
    | ResourceExhausted;
```

### 22.7.4 Error mode semantics

**FailFast:** On first error, all other tasks are marked for cancellation. The nursery waits for all tasks to reach checkpoints and terminate.

**CancelRemaining:** On first error, pending tasks (not yet started) are cancelled immediately. Running tasks continue to completion.

**CollectAll:** Errors do not trigger cancellation. All tasks run to completion.

### 22.7.5 Timeout cancellation

When a nursery timeout expires:

1. All incomplete tasks are marked for cancellation
2. Tasks reach checkpoints and terminate
3. Nursery waits for cancellation to complete
4. Returns results collected so far

Timeout always cancels incomplete tasks regardless of error mode.

### 22.7.6 Cleanup guarantees

When a task is cancelled:

1. Stack unwinding occurs from the cancellation checkpoint
2. Destructors run for all values in scope
3. Cleanup is guaranteed to complete before task terminates

A task cannot be forcibly terminated during destructor execution.

### 22.7.7 Checking cancellation status

Tasks can explicitly check cancellation status:

```ori
@is_cancelled () -> bool;

```

This built-in function returns `true` if the current task has been marked for cancellation.

```ori
@long_running_task () -> Result<Data, Error> uses Suspend = {
    for item in large_dataset do {
        if is_cancelled() then break Err(CancellationError { ... })
        process(item)
    }
}
```

### 22.7.8 Nested nurseries

When an outer nursery cancels a task containing an inner nursery:

1. The inner nursery receives cancellation
2. Inner nursery cancels its tasks per its error mode
3. Inner nursery completes (with cancellation results)
4. Outer task then completes

## 22.8 Errors

```
error[E0700]: cannot capture mutable binding across task boundary
  --> example.ori:5:15
   |
 5 |         () -> { x = 1 },
   |               ^^^^^^^^^^^
   = note: mutable bindings cannot be shared between tasks

error[E0701]: callee uses Suspend but caller does not
  --> example.ori:3:5
   |
 3 |     async_fn()
   |     ^^^^^^^^^^
   = help: add `uses Suspend` to the function signature

error[E0702]: concurrency pattern requires Suspend capability
  --> example.ori:2:5
   |
 2 |     parallel(tasks: [...])
   |     ^^^^^^^^^^^^^^^^^^^^^^
   = help: add `uses Suspend` to @main
```
