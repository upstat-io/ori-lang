# Proposal: Memory Model Edge Cases

**Status:** Approved
**Approved:** 2026-01-30
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Compiler, runtime, memory management

---

## Summary

This proposal addresses edge cases in Ori's memory model, including reference count atomicity across tasks, destructor guarantees, panic during destruction, and value representation rules. It also formally introduces the `Drop` trait for custom destructors.

---

## Problem Statement

The memory model spec covers the basics but leaves unclear:

1. **Refcount atomicity**: Are reference counts thread-safe across tasks?
2. **Destructor timing**: When exactly do destructors run?
3. **Panic in destructor**: What happens if cleanup code panics?
4. **Value representation**: What's the threshold for pass-by-value vs reference?
5. **Custom destructors**: How do types define cleanup logic?

---

## The Drop Trait

The `Drop` trait enables custom destruction logic for user-defined types.

### Definition

```ori
trait Drop {
    @drop (self) -> void
}
```

### Semantics

- When a value's reference count reaches zero, its `Drop.drop` method is called if implemented
- Drop is called before memory is reclaimed
- Drop methods cannot be async (see "Async Destructors" below)
- Drop methods should not panic (see "Panic During Destruction" below)

### Prelude Status

`Drop` is included in the prelude and available without import.

### Example

```ori
type FileHandle = { fd: int }

impl Drop for FileHandle {
    @drop (self) -> void = close_fd(self.fd)
}
```

---

## Reference Counting Atomicity

### Requirement

All reference count operations MUST be atomic (thread-safe):

```ori
let shared = create_large_data()
parallel(
    tasks: [
        () -> read(shared),   // Increments refcount atomically
        () -> read(shared),   // Increments refcount atomically
    ],
)
// Decrements happen atomically as tasks complete
```

### Implementation

The runtime uses atomic compare-and-swap operations:

| Operation | Atomicity |
|-----------|-----------|
| Increment refcount | Atomic fetch-add |
| Decrement refcount | Atomic fetch-sub |
| Check for zero | Part of decrement operation |

### Why This Matters

Without atomic refcounts:
- Two tasks could decrement simultaneously
- Both could see refcount = 1
- Both could attempt to free memory
- Use-after-free or double-free bugs

### Performance Note

Atomic operations have overhead compared to non-atomic. Implementations may optimize:
- Elide refcount operations when values don't escape
- Most programs are not refcount-bound

---

## Destructor Timing Guarantees

### When Destructors Run

Destructors run when a value's reference count reaches zero:

| Context | Timing |
|---------|--------|
| Local binding goes out of scope | Immediately at scope end |
| Last reference dropped | Immediately after drop |
| Field of struct dropped | After struct destructor |
| Collection element | When removed or collection dropped |

### "No Later Than" Guarantee

The spec promises destructors run "no later than scope end":

```ori
@example () -> void = run(
    let resource = acquire(),  // refcount = 1
    use_resource(resource),    // may increment temporarily
    // <- destructor runs here, before function returns
)
```

### Scope Nesting

Inner scopes are destroyed before outer:

```ori
@nested () -> void = run(
    let outer = create_outer(),
    run(
        let inner = create_inner(),
        // inner's destructor runs here
    ),
    // outer's destructor runs here
)
```

### Early Drop

Values may be dropped before scope end if no longer referenced:

```ori
@early () -> void = run(
    let x = create(),
    use(x),
    // x not used after this point
    // Compiler MAY drop x here (optimization)

    long_operation(),  // x might already be dropped

    // x's destructor runs no later than here
)
```

---

## Panic During Destruction

### The Problem

What if a destructor panics?

```ori
type BadType = { ... }

impl Drop for BadType {
    @drop (self) -> void = panic(msg: "destructor failed!")
}

@example () -> void = run(
    let bad = BadType { ... },
    // When bad is dropped, destructor panics
)
```

### Resolution: Abort on Double Panic

If a panic occurs during destructor execution while already unwinding from another panic:

1. The program **aborts** immediately
2. No further destructors run
3. Exit code indicates abnormal termination

```ori
@double_panic () -> void = run(
    let bad1 = BadType { ... },
    let bad2 = BadType { ... },
    panic(msg: "initial panic"),
    // Unwinding begins, bad2's destructor runs, panics
    // ABORT: double panic
)
```

### Single Panic in Destructor

If a destructor panics during normal execution (not unwinding):

1. That destructor's panic propagates normally
2. Other values in scope still have their destructors run
3. Each destructor runs in isolation

```ori
@single_panic () -> void = run(
    let good = GoodType { ... },
    let bad = BadType { ... },  // Destructor panics
    // bad's destructor panics
    // good's destructor still runs
    // panic propagates after all destructors complete
)
```

### Destructor Order During Panic

When unwinding, destructors run in reverse declaration order:

```ori
@unwind_order () -> void = run(
    let a = create_a(),  // Destroyed 3rd
    let b = create_b(),  // Destroyed 2nd
    let c = create_c(),  // Destroyed 1st
    may_panic(),
    // If panic: c dropped, then b, then a
)
```

---

## Value Representation Rules

### Small Value Optimization

Values meeting these criteria are passed by value (copied):

| Criterion | Threshold |
|-----------|-----------|
| Size | ≤ 32 bytes |
| Type | Primitive or simple struct |

### Primitive Types

All primitives are always passed by value:

| Type | Size | Pass-by |
|------|------|---------|
| `int` | 8 bytes | Value |
| `float` | 8 bytes | Value |
| `bool` | 1 byte | Value |
| `char` | 4 bytes | Value |
| `byte` | 1 byte | Value |
| `Duration` | 8 bytes | Value |
| `Size` | 8 bytes | Value |

### Reference-Counted Types

Types that exceed size threshold or have identity semantics:

| Type | Pass-by |
|------|---------|
| `str` | Reference (pointer + length + refcount) |
| `[T]` | Reference |
| `{K: V}` | Reference |
| Large structs | Reference |

### Struct Classification

```ori
// Passed by value (small, no references)
type Point = { x: float, y: float }  // 16 bytes

// Passed by value (at threshold)
type AABB = { min: Point, max: Point }  // 32 bytes

// Passed by reference (over threshold)
type Transform = { position: Point, rotation: Point, scale: Point }  // 48 bytes
```

### Clone vs Copy

Ori doesn't distinguish Copy vs Clone at the language level:
- All value-passing is conceptually "copy"
- Reference types increment refcount (cheap)
- Large value types are stored as references automatically

---

## Destruction Order Guarantees

### Struct Fields

When a struct is destroyed, fields are destroyed in reverse declaration order:

```ori
type Container = {
    first: Resource,   // Destroyed 3rd
    second: Resource,  // Destroyed 2nd
    third: Resource,   // Destroyed 1st (last declared, first destroyed)
}
```

### Collection Elements

List elements are destroyed back-to-front (reverse index order):

```ori
let items = [a, b, c]
// When items dropped: c, then b, then a (reverse index order)
```

Map entries have no guaranteed destruction order (hash-based).

### Tuple Elements

Tuple elements are destroyed right-to-left (reverse order):

```ori
let tuple = (first, second, third)
// When dropped: third, then second, then first
```

---

## Interaction with Async

### Destructor Execution Context

Destructors run in the task that drops the value:

```ori
@task1 () uses Suspend = run(
    let resource = acquire(),
    // resource's destructor runs in task1
)

@task2 () uses Suspend = run(
    let resource = acquire(),
    // resource's destructor runs in task2
)
```

### Async Destructors

Destructors CANNOT be async — they run synchronously:

```ori
impl Drop for Resource {
    @drop (self) -> void uses Suspend = ...  // ERROR: drop cannot be async
}
```

If cleanup requires async operations, use explicit cleanup methods:

```ori
type AsyncResource = { ... }

impl AsyncResource {
    @close (self) -> void uses Suspend = ...  // Explicit async cleanup
}

impl Drop for AsyncResource {
    @drop (self) -> void = ()  // Synchronous, minimal
}

// Usage:
let resource = acquire()
resource.close()  // Explicit async cleanup
// drop runs, but does nothing significant
```

### Task Cancellation and Destructors

When a task is cancelled, destructors still run:

```ori
@cancellable_task () uses Suspend = run(
    let resource = acquire(),
    long_async_operation(),  // Task cancelled here
    // resource's destructor STILL runs during cancellation
)
```

---

## Examples

### Safe Resource Management

```ori
type FileHandle = { fd: int }

impl Drop for FileHandle {
    @drop (self) -> void = close_fd(self.fd)  // Synchronous OS call
}

@process_file (path: str) -> Result<void, Error> = run(
    let file = open(path)?,
    process_contents(file)?,
    // file automatically closed here
    Ok(()),
)
```

### Avoiding Destructor Panics

```ori
type Connection = { ... }

impl Drop for Connection {
    @drop (self) -> void = run(
        // Don't panic in destructor — handle errors gracefully
        match(self.close_internal(),
            Ok(_) -> (),
            Err(e) -> log_error(e),  // Log, don't panic
        ),
    )
}
```

### Reference Counting in Action

```ori
@shared_data_example () uses Suspend = run(
    let data = create_large_data(),  // refcount = 1

    parallel(
        tasks: [
            () -> run(
                use(data),  // refcount = 2 during use
                // refcount decremented when task ends
            ),
            () -> run(
                use(data),  // refcount = 2 or 3 depending on timing
            ),
        ],
    ),
    // Both tasks done, refcount = 1

    // data's destructor runs when this scope ends
)
```

---

## Spec Changes Required

### Update `15-memory-model.md`

Add:
1. Drop trait definition
2. Refcount atomicity requirement
3. Destructor timing guarantees
4. Panic during destruction behavior
5. Value representation thresholds

### Add Destruction Order Section

Document:
1. Struct field destruction order (reverse declaration order)
2. Collection element destruction order (reverse index order for lists)
3. Tuple destruction order (right-to-left)
4. Async interaction with destructors

### Update Prelude Documentation

Add `Drop` to the prelude traits list.

---

## Summary

| Aspect | Guarantee |
|--------|-----------|
| Drop trait | `trait Drop { @drop (self) -> void }` in prelude |
| Refcount atomicity | All operations atomic (thread-safe) |
| Destructor timing | No later than scope end |
| Destruction order | Reverse declaration order (LIFO) |
| Panic in destructor | Single: propagates; Double: abort |
| Value threshold | ≤32 bytes = by value |
| Primitives | Always by value |
| Collections | Always by reference |
| Async destructors | Not allowed; use explicit cleanup |
| Cancellation | Destructors still run |
