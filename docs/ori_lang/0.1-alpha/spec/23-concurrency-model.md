---
title: "Concurrency Model"
description: "Ori Language Specification — Concurrency Model"
order: 23
section: "Concurrency"
---

# Concurrency Model

Task-based concurrency with cooperative scheduling and explicit suspension points.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § PATTERNS (parallel, spawn, nursery)

## Tasks

A _task_ is an independent unit of concurrent execution.

### Properties

A task has:

1. **Its own call stack** — function calls within a task are sequential
2. **Isolated mutable state** — no task can directly access another task's mutable bindings
3. **Cooperative scheduling** — tasks yield control at suspension points
4. **Bounded lifetime** — tasks are created within a scope and must complete before that scope exits

Tasks are not threads. Multiple tasks may execute on the same OS thread, or the runtime may distribute tasks across OS threads. The runtime determines task-to-thread mapping.

### Task Creation

Tasks are created exclusively by concurrency patterns:

| Pattern | Creates Tasks | Description |
|---------|--------------|-------------|
| `parallel(tasks: [...])` | Yes | One task per list element |
| `spawn(tasks: [...])` | Yes | Fire-and-forget tasks |
| `nursery(body: n -> ...)` | Yes, via `n.spawn()` | Structured task spawning |
| Regular function call | No | Same task, same stack |

See [Patterns](10-patterns.md) for pattern definitions.

### Task Isolation

Each task has private mutable bindings. Bindings captured across task boundaries must be `Sendable` and become inaccessible in the spawning scope:

```ori
@example () -> void uses Async = run(
    let x = 0,
    parallel(
        tasks: [
            () -> run(x = 1),  // error: cannot capture mutable binding across task boundary
            () -> run(x = 2),
        ],
    ),
)
```

See [Memory Model § Task Isolation](15-memory-model.md#task-isolation) for isolation guarantees.

## Async Context

An _async context_ is a runtime environment that can execute async functions, schedule suspension and resumption, and manage multiple concurrent tasks.

### Establishing Async Context

An async context is established by:

- **The runtime** — when `@main` declares `uses Async`, the runtime provides the initial async context
- **Concurrency patterns** — `parallel`, `spawn`, and `nursery` create nested async contexts for spawned tasks

A function declaring `uses Async` _requires_ an async context to execute — it does not establish one. The `Async` capability indicates the function may suspend, requiring a scheduler to manage resumption.

### @main and Async

Programs using concurrency patterns must have `@main` declare `uses Async`:

```ori
// Valid: main declares Async
@main () -> void uses Async = run(
    parallel(tasks: [task_a(), task_b()]),
)

// Invalid: main uses concurrency without Async
@main () -> void = run(
    parallel(tasks: [task_a(), task_b()]),  // error: requires Async capability
)
```

The runtime establishes the async context when `@main uses Async` is declared.

## Suspension Points

A _suspension point_ is a location where a task may yield control to the scheduler.

### Where Suspension Occurs

Suspension points occur ONLY at:

1. **Async function calls** — calling a function with `uses Async`
2. **Channel operations** — `send` and `receive` on channels
3. **Explicit yield** — within `parallel`, `spawn`, or `nursery` body evaluation

### Where Suspension Cannot Occur

Suspension NEVER occurs:

- In the middle of expression evaluation
- During non-async function execution
- At arbitrary points chosen by the runtime

This provides _predictable interleaving_ — atomicity boundaries are explicit.

## Async Propagation

A function that calls async code must itself be async:

```ori
@caller () -> int uses Async =
    callee()  // OK: caller is async

@caller_sync () -> int =
    callee()  // error: callee uses Async but caller does not

@callee () -> int uses Async = ...
```

The `Async` capability _propagates upward_ — callers of async functions must declare `uses Async`.

## Blocking in Async Context

A non-async function called from an async context executes synchronously, blocking that task but not other tasks:

```ori
@expensive_sync () -> int =
    heavy_math()  // Long computation, no suspension points

@main () -> void uses Async = run(
    parallel(
        tasks: [
            () -> expensive_sync(),  // This task blocks during computation
            () -> other_work(),       // This task can run concurrently
        ],
    ),
)
```

## Capture and Ownership

Task closures follow capture-by-value semantics. When a value is captured by a task closure, the original binding becomes inaccessible:

```ori
@capture_example () -> void uses Async = run(
    let data = create_data(),
    nursery(
        body: n -> run(
            n.spawn(task: () -> process(data)),  // data captured by value
            print(msg: data.field),  // error: data is no longer accessible
        ),
    ),
)
```

Bindings captured across task boundaries become inaccessible in the spawning scope to prevent data races. This uses existing capture-by-value behavior with an additional constraint.

### Sendable Requirement

Captured values must implement `Sendable`:

```ori
@spawn_example () -> void uses Async = run(
    let data = create_data(),  // Data: Sendable
    nursery(
        body: n -> n.spawn(task: () -> process(data)),  // OK
    ),
)
```

See [Properties of Types § Sendable](07-properties-of-types.md#sendable) for `Sendable` definition.

### Reference Count Atomicity

When values cross task boundaries, reference count operations are atomic. The compiler ensures thread-safe reference counting for values accessed by multiple tasks.

## Errors

```
error[E0700]: cannot capture mutable binding across task boundary
  --> example.ori:5:15
   |
 5 |         () -> run(x = 1),
   |               ^^^^^^^^^^
   = note: mutable bindings cannot be shared between tasks

error[E0701]: callee uses Async but caller does not
  --> example.ori:3:5
   |
 3 |     async_fn()
   |     ^^^^^^^^^^
   = help: add `uses Async` to the function signature

error[E0702]: concurrency pattern requires Async capability
  --> example.ori:2:5
   |
 2 |     parallel(tasks: [...])
   |     ^^^^^^^^^^^^^^^^^^^^^^
   = help: add `uses Async` to @main
```
