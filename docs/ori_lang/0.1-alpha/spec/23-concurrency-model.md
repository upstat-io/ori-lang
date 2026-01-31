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
@example () -> void uses Suspend = run(
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

## Suspending Context

A _suspending context_ is a runtime environment that can execute suspending functions, schedule suspension and resumption, and manage multiple concurrent tasks.

### Establishing Suspending Context

A suspending context is established by:

- **The runtime** — when `@main` declares `uses Suspend`, the runtime provides the initial suspending context
- **Concurrency patterns** — `parallel`, `spawn`, and `nursery` create nested suspending contexts for spawned tasks

A function declaring `uses Suspend` _requires_ a suspending context to execute — it does not establish one. The `Suspend` capability indicates the function may suspend, requiring a scheduler to manage resumption.

### @main and Suspend

Programs using concurrency patterns must have `@main` declare `uses Suspend`:

```ori
// Valid: main declares Suspend
@main () -> void uses Suspend = run(
    parallel(tasks: [task_a(), task_b()]),
)

// Invalid: main uses concurrency without Suspend
@main () -> void = run(
    parallel(tasks: [task_a(), task_b()]),  // error: requires Suspend capability
)
```

The runtime establishes the suspending context when `@main uses Suspend` is declared.

## Suspension Points

A _suspension point_ is a location where a task may yield control to the scheduler.

### Where Suspension Occurs

Suspension points occur ONLY at:

1. **Suspending function calls** — calling a function with `uses Suspend`
2. **Channel operations** — `send` and `receive` on channels
3. **Explicit yield** — within `parallel`, `spawn`, or `nursery` body evaluation

### Where Suspension Cannot Occur

Suspension NEVER occurs:

- In the middle of expression evaluation
- During non-suspending function execution
- At arbitrary points chosen by the runtime

This provides _predictable interleaving_ — atomicity boundaries are explicit.

## Suspend Propagation

A function that calls suspending code must itself be suspending:

```ori
@caller () -> int uses Suspend =
    callee()  // OK: caller can suspend

@caller_sync () -> int =
    callee()  // error: callee uses Suspend but caller does not

@callee () -> int uses Suspend = ...
```

The `Suspend` capability _propagates upward_ — callers of suspending functions must declare `uses Suspend`.

## Blocking in Suspending Context

A non-suspending function called from a suspending context executes synchronously, blocking that task but not other tasks:

```ori
@expensive_sync () -> int =
    heavy_math()  // Long computation, no suspension points

@main () -> void uses Suspend = run(
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
@capture_example () -> void uses Suspend = run(
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
@spawn_example () -> void uses Suspend = run(
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
