# Proposal: Closure Capture Semantics

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Approved:** 2026-01-30
**Affects:** Compiler, type system, memory model

---

## Summary

This proposal specifies precise closure capture semantics, including when captures occur, how mutable bindings interact with captures, closure behavior across task boundaries, and memory implications.

---

## Problem Statement

The spec states closures "capture by value" but leaves unclear:

1. **Capture timing**: When exactly does capture occur?
2. **Mutable captures**: What happens when a mutable binding is captured and later mutated?
3. **Task boundaries**: How do closures work with `parallel`/`nursery`?
4. **Closure memory**: What is stored in a closure?
5. **Escaping closures**: What restrictions apply to closures that outlive their scope?

---

## Capture Model

### Capture-by-Value Semantics

When a closure references a variable from an outer scope, the **current value** is captured (copied) into the closure at the point of closure creation:

```ori
let x = 10
let f = () -> x  // x=10 captured here
x = 20           // Original x reassigned
f()              // Returns 10 (captured value)
```

### Capture Timing

Capture occurs when the closure expression is evaluated:

```ori
let closures = []
for i in 0..3 do
    closures = closures + [() -> i]  // Captures current i value

closures[0]()  // 0
closures[1]()  // 1
closures[2]()  // 2
// Each closure captured i at its moment of creation
```

### What Gets Captured

A closure captures all free variables (variables referenced but not defined within the closure):

```ori
let a = 1
let b = 2
let c = 3

let f = () -> a + b  // Captures a and b, not c

// c is not captured because it's not referenced
```

---

## Mutable Bindings and Capture

### Reassignment After Capture

Reassigning a mutable binding after capture does NOT affect the captured value:

```ori
let x = 10
let f = () -> x  // Captures 10
x = 20           // Reassigns original binding; does not affect f's captured value
f()              // Returns 10
```

### Captured Values Are Immutable

The captured value itself is immutable within the closure — closures cannot modify captured state:

```ori
let x = 10
let f = () -> {
    x = 20,  // ERROR: cannot mutate captured binding
    x
}
```

### Rebinding Inside Closure

A closure can shadow a captured binding with a local one:

```ori
let x = 10
let f = () -> {
    let x = 20,  // Shadows captured x (new local binding)
    x,           // Returns 20
}
f()  // Returns 20
```

---

## Closures Across Task Boundaries

### Sendable Requirement

Closures passed to `parallel`, `spawn`, or `nursery` must capture only `Sendable` values:

```ori
@spawn_closure () -> void uses Suspend = {
    let data = create_sendable_data(),  // data: Sendable
    parallel(
        tasks: [() -> process(data)],   // OK: data is Sendable
    )
}
```

### Non-Sendable Capture Error

```ori
type Handle = { fd: FileDescriptor }  // NOT Sendable

@bad_spawn () -> void uses Suspend = {
    let h = get_handle(),  // h: Handle
    parallel(
        tasks: [() -> use_handle(h)],  // ERROR: Handle is not Sendable
    )
}
```

Error message:
```
error[E0700]: closure captures non-Sendable type
  --> src/main.ori:5:14
   |
5  |         tasks: [() -> use_handle(h)],
   |                ^^^^^^^^^^^^^^^^^^^^^ closure captures `h` of type `Handle`
   |
   = note: `Handle` does not implement `Sendable`
   = note: closures crossing task boundaries must capture only Sendable types
```

### Move Semantics for Task Closures

When a closure is passed to a task-spawning pattern (`parallel`, `spawn`, `nursery`), captured values are **moved** into the task. The original binding becomes inaccessible after the capture point:

```ori
@move_example () -> void uses Suspend = {
    let data = create_data()
    parallel(
        tasks: [() -> process(data)],  // data captured and moved
    )
    print(msg: data.field),  // ERROR: data is no longer accessible
}
```

This restriction prevents data races by ensuring no two tasks can observe the same mutable data.

**Note:** This behavior differs from regular closures, where the original binding remains accessible (though reassignment after capture doesn't affect the closure). Task closures have stricter requirements to ensure thread safety.

---

## Closure Types

### Type Inference and Coercion

A closure's type is inferred from its parameter types and return type. Closures are compatible with function types of matching signature:

```ori
let f: (int) -> int = x -> x + 1    // Closure coerces to function type
let g: () -> str = () -> "hello"
```

Two closures with identical signatures have distinct types (due to different captured environments), but both coerce to the same function type.

---

## Closure Representation

### Memory Layout

A closure is represented as a struct containing captured values:

```ori
let x = 10
let y = "hello"
let f = () -> `{y}: {x}`

// f is approximately:
// type _Closure_f = { captured_x: int, captured_y: str }
```

### Captured Reference Types

For reference-counted types (lists, maps, custom types), the closure stores the reference (incrementing the reference count), not a deep copy of the data.

---

## Escaping Closures

### Definition

An **escaping closure** is one that outlives the scope in which it was created:

```ori
@make_adder (n: int) -> (int) -> int =
    x -> x + n  // Escapes: returned from function

let adder = make_adder(n: 5)
adder(10)  // 15
```

### Escaping is Always Safe

Because closures capture by value, escaping is always safe — the closure owns its captured data:

```ori
@safe_escape () -> () -> int = {
    let local = compute_value(),  // local exists in this scope
    () -> local,                  // Closure captures local's value
}  // local goes out of scope, but closure has its own copy

let f = safe_escape()
f()  // Safe: closure has its own copy of the value
```

### No Lifetime Annotations

Unlike Rust, Ori closures don't need lifetime annotations because:
- No references (capture by value only)
- No borrowing (ARC handles memory)
- No dangling pointers possible

---

## Higher-Order Functions

### Closures as Parameters

Functions can accept closures as parameters:

```ori
@apply<T, U> (f: (T) -> U, value: T) -> U =
    f(value)

apply(f: x -> x * 2, value: 5)  // 10
```

### Closures as Return Values

Functions can return closures:

```ori
@compose<A, B, C> (f: (A) -> B, g: (B) -> C) -> (A) -> C =
    x -> g(f(x))

let double_then_str = compose(f: x -> x * 2, g: int_to_str)
double_then_str(5)  // "10"
```

### Stored Closures

Closures can be stored in data structures:

```ori
type Handler = { on_success: (Data) -> void, on_error: (Error) -> void }

let handler = Handler {
    on_success: data -> print(msg: data.message),
    on_error: err -> log_error(err),
}
```

---

## Closure Equality

### No Structural Equality

Closures do not support equality comparison:

```ori
let f = x -> x + 1
let g = x -> x + 1
f == g  // ERROR: closures do not implement Eq
```

### Rationale

Two closures with identical code may capture different values, and comparing captures would be confusing and expensive.

---

## Examples

### Counter Pattern (Doesn't Work)

This common pattern from other languages doesn't work in Ori due to capture-by-value:

```ori
// This does NOT create a working counter
@make_counter () -> () -> int = {
    let count = 0
    () -> {
        count = count + 1,  // ERROR: cannot mutate captured binding
        count
    }
}
```

### Correct Counter (Using State)

Use explicit state passing instead:

```ori
type Counter = { value: int }

@increment (c: Counter) -> (int, Counter) =
    (c.value, Counter { value: c.value + 1 })

let c = Counter { value: 0 }
let (v1, c) = increment(c)  // v1 = 0
let (v2, c) = increment(c)  // v2 = 1
```

### Partial Application

```ori
@add (a: int, b: int) -> int = a + b

@partial_add (a: int) -> (int) -> int =
    b -> add(a: a, b: b)

let add5 = partial_add(a: 5)
add5(3)  // 8
```

### Event Handler Registration

```ori
@setup_handlers (config: Config) -> [Handler] = [
    Handler {
        event: "click",
        action: e -> handle_click(config: config, event: e),  // config captured
    },
    Handler {
        event: "submit",
        action: e -> handle_submit(config: config, event: e),
    },
]
```

---

## Spec Changes Required

### Update `17-blocks-and-scope.md`

Add comprehensive closure section:
1. Capture-by-value semantics
2. Capture timing
3. Mutable binding interaction
4. Escaping closure rules

### Update `15-memory-model.md`

Add:
1. Closure memory representation
2. Reference counting for captured values

### Update Concurrency Sections

Add:
1. Sendable requirement for task closures
2. Move semantics for task boundary crossing

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Capture mechanism | By value (copy) |
| Capture timing | At closure creation |
| Reassignment after capture | Does not affect closure |
| Mutation inside closure | Not allowed |
| Task boundary | Must capture Sendable only |
| Move to task | Values moved, original inaccessible |
| Escaping | Always safe (owns captured data) |
| Equality | Not supported |
| Closure memory | Captured values (references for ARC types) |
| Lifetime annotations | Not needed |
