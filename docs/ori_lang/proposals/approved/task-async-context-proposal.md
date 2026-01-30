# Proposal: Task and Async Context Definitions

**Status:** Approved
**Approved:** 2026-01-29
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Compiler, runtime, concurrency model

---

## Summary

This proposal formally defines what a "task" is in Ori and specifies async context semantics. Currently, the spec uses these terms without precise definitions, making it impossible to reason about concurrency guarantees.

---

## Problem Statement

The specification references "tasks" in multiple places without defining them:

- "Types can safely cross task boundaries" (Sendable trait)
- "All spawned tasks complete before nursery exits" (nursery pattern)
- "Tasks may execute in parallel" (parallel pattern)

**What is a task?** The spec doesn't say.

Similarly, `uses Async` declares "may suspend" but:

- What exactly is suspension?
- Where can suspension occur?
- How do async and non-async functions interact?

---

## Definitions

### Task

A **task** is an independent unit of concurrent execution with:

1. **Its own call stack** — function calls within a task are sequential
2. **Isolated mutable state** — no task can directly access another task's mutable bindings
3. **Cooperative scheduling** — tasks yield control at suspension points
4. **Bounded lifetime** — tasks are created within a scope and must complete before that scope exits

Tasks are NOT threads. Multiple tasks may execute on the same thread (green threads/coroutines), or the runtime may distribute tasks across OS threads.

### Async Context

An **async context** is a runtime environment that can:

1. Execute async functions (those with `uses Async`)
2. Schedule suspension and resumption
3. Manage multiple concurrent tasks

An async context is established by:

- **The runtime** — when `@main` declares `uses Async`, the runtime provides the initial async context
- **Concurrency patterns** — `parallel`, `spawn`, and `nursery` create nested async contexts for their spawned tasks

A function declaring `uses Async` **requires** an async context to execute — it does not establish one. The `Async` capability indicates the function may suspend, requiring a scheduler to manage resumption.

### Suspension Point

A **suspension point** is a location where a task may yield control to the scheduler. Suspension points occur ONLY at:

1. **Async function calls** — calling a function with `uses Async`
2. **Channel operations** — `send` and `receive` on channels
3. **Explicit yield** — within `parallel`, `spawn`, or `nursery` body evaluation

Suspension NEVER occurs:

- In the middle of expression evaluation
- During non-async function execution
- At arbitrary points chosen by the runtime

This provides **predictable interleaving** — developers can reason about atomicity.

---

## Semantics

### @main and Async

Programs using concurrency patterns (`parallel`, `spawn`, `nursery`) must have `@main` declare `uses Async`:

```ori
// Correct: main declares Async
@main () -> void uses Async = run(
    parallel(tasks: [task_a(), task_b()]),
)

// ERROR: main uses concurrency without Async
@main () -> void = run(
    parallel(tasks: [task_a(), task_b()]),  // Error: requires Async capability
)
```

The runtime establishes the async context when `@main uses Async` is declared.

### Task Creation

Tasks are created by:

| Pattern | Creates Tasks? | Description |
|---------|---------------|-------------|
| `parallel(tasks: [...])` | Yes | One task per list element |
| `spawn(tasks: [...])` | Yes | Fire-and-forget tasks |
| `nursery(body: n -> ...)` | Yes, via `n.spawn()` | Structured task spawning |
| Regular function call | No | Same task, same stack |

### Task Isolation

Each task has:

- **Private mutable bindings** — `let x = ...` in one task is invisible to others
- **Shared immutable data** — values passed to tasks are immutable from the task's perspective (ownership transferred)
- **No shared mutable state** — Ori's memory model prevents this

```ori
@example () -> void uses Async = run(
    let x = 0,
    parallel(
        tasks: [
            () -> run(x = 1),  // ERROR: cannot capture mutable binding across task boundary
            () -> run(x = 2),
        ],
    ),
)
```

### Async Propagation

A function that calls async code must itself be async:

```ori
@caller () -> int uses Async =
    callee()  // OK: caller is async

@caller_sync () -> int =
    callee()  // ERROR: callee uses Async but caller does not

@callee () -> int uses Async = ...
```

The async capability **propagates upward** — if you call async code, you must declare it.

### Blocking in Async Context

A non-async function called from an async context executes synchronously, blocking that task (but not other tasks):

```ori
@expensive_sync () -> int =
    // Long computation, no suspension points
    heavy_math()

@main () -> void uses Async = run(
    parallel(
        tasks: [
            () -> expensive_sync(),  // This task blocks during computation
            () -> other_work(),       // This task can run concurrently
        ],
    ),
)
```

---

## Task Memory Model

### Capture and Ownership Transfer

Task closures follow Ori's standard capture-by-value semantics. When a value is captured by a task closure, the original binding becomes inaccessible — ownership transfers to the task:

```ori
@capture_example () -> void uses Async = run(
    let data = create_data(),
    nursery(
        body: n -> run(
            n.spawn(task: () -> process(data)),  // data captured by value
            // data cannot be used here — ownership transferred to spawned task
            print(msg: data.field),  // ERROR: data is no longer accessible
        ),
    ),
)
```

This is not a new "move" mechanism — it uses the existing capture-by-value behavior with an additional constraint: bindings captured across task boundaries become inaccessible in the spawning scope to prevent data races.

### Sendable Requirement

When spawning a task, captured values must be `Sendable`:

```ori
@spawn_example () -> void uses Async = run(
    let data = create_data(),  // data: Data, where Data: Sendable
    nursery(
        body: n -> n.spawn(task: () -> process(data)),  // OK: data is Sendable
    ),
)
```

### Reference Count Atomicity

When values cross task boundaries (via spawn or channel send), reference count operations are **atomic**. This is an implementation requirement, not a language-level concern — the compiler/runtime must ensure thread-safe reference counting for values accessed by multiple tasks.

---

## Examples

### Valid Async Patterns

```ori
// Task creates subtasks
@fan_out (items: [Item]) -> [Result] uses Async =
    parallel(
        tasks: items.map(item -> () -> process(item: item)),
    )

// Async function calling async function
@outer () -> int uses Async = inner()
@inner () -> int uses Async = fetch_data()

// Non-async helper in async context
@async_with_sync () -> int uses Async = run(
    let raw = fetch_raw(),   // async call — suspension point
    let parsed = parse(raw),  // sync call — no suspension
    validate(parsed),         // sync call — no suspension
)
```

### Invalid Patterns

```ori
// ERROR: sync function cannot call async
@bad_sync () -> int = fetch_data()

// ERROR: capturing mutable binding across task boundary
@bad_capture () -> void uses Async = run(
    let counter = 0,
    parallel(tasks: [() -> run(counter = counter + 1)]),
)

// ERROR: main uses concurrency without Async
@bad_main () -> void = parallel(tasks: [task_a()])
```

---

## Spec Changes Required

### New Section: `XX-concurrency-model.md`

Create new spec section covering:
1. Task definition
2. Async context definition
3. Suspension points
4. Task creation patterns
5. Task isolation guarantees
6. Async propagation rules

### Updates to `14-capabilities.md`

Clarify that `Async` is a marker capability indicating suspension possibility, with reference to the concurrency model spec.

### Updates to `10-patterns.md`

Add references to task definitions for `parallel`, `spawn`, and `nursery`.

---

## Design Rationale

### Why Explicit Suspension Points?

Languages with implicit suspension (like Go) make it hard to reason about data races. By limiting suspension to explicit points (async calls, channel ops), developers can understand where interleaving occurs.

### Why Propagate Async?

This is the same rationale as Rust's `async fn` — making suspension visible in the type system catches errors at compile time rather than runtime.

### Why Ownership Transfer for Task Capture?

Preventing shared mutable state between tasks eliminates data races. The ownership transfer ensures the spawning code cannot accidentally modify data the task is using.

---

## Summary

| Concept | Definition |
|---------|------------|
| Task | Independent concurrent execution unit with own stack and isolated mutable state |
| Async context | Runtime environment capable of scheduling async execution |
| Suspension point | Explicit location where task may yield (async calls, channel ops) |
| Task creation | Via `parallel`, `spawn`, or `nursery` patterns only |
| Capture | Values captured by value; binding becomes inaccessible; must be `Sendable` |
| Async propagation | Callers of async functions must be async |
| @main requirement | Must declare `uses Async` to use concurrency patterns |
