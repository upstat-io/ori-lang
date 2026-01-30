---
title: "Memory Model"
description: "Automatic Reference Counting, value semantics, and cycle prevention."
order: 20
part: "Advanced Patterns"
---

# Memory Model

Ori uses Automatic Reference Counting (ARC) for memory management — no garbage collector pauses, no manual memory management, and no borrow checker.

## How ARC Works

Every value has a reference count. When you assign a value, the count increases. When a reference goes out of scope, the count decreases. When it hits zero, the memory is freed.

```ori
let a = [1, 2, 3]      // ref count = 1
let b = a              // ref count = 2 (a and b share the data)
// b goes out of scope  // ref count = 1
// a goes out of scope  // ref count = 0, memory freed
```

### Deterministic Cleanup

Unlike garbage collection, ARC frees memory immediately when the last reference is gone:

```ori
@process_file (path: str) -> void uses FileSystem = run(
    let data = FileSystem.read(path: path),  // Memory allocated
    let result = process(data: data),
    print(msg: result),
)  // data freed exactly here, not "sometime later"
```

This predictability is valuable for resource-constrained environments and real-time applications.

## Why No Garbage Collector?

| Feature | GC | ARC |
|---------|----|----|
| Pause times | Unpredictable | None |
| Memory overhead | Higher | Lower |
| Cleanup timing | Eventually | Immediate |
| Performance | Variable | Consistent |

Ori chose ARC for:
- Predictable performance
- Lower memory overhead
- Immediate cleanup
- Simpler runtime

## Preventing Reference Cycles

ARC can't handle reference cycles. If A references B and B references A, neither can ever be freed. Ori's design prevents cycles:

### 1. Sequential Data Flow

Data flows forward through `run`/`try`:

```ori
run(
    let a = create_a(),
    let b = create_b(input: a),   // b can reference a
    let c = create_c(input: b),   // c can reference b
    // No way for a to reference c (c doesn't exist when a is created)
)
```

### 2. Capture by Value

Closures capture variables by value, not reference:

```ori
let x = 10
let f = () -> x + 1  // f captures a COPY of x = 10

// Even if we could reassign x, f still has 10
f()  // Always returns 11
```

This means closures can't create cycles by capturing "self" references.

### 3. No Self-Referential Types

You can't create types that reference themselves through the same instance:

```ori
// This pattern is NOT possible in Ori:
type Node = {
    value: int,
    parent: Option<Node>,  // Can't point back to containing instance
}
```

Instead, use:
- Indices into collections
- Separate parent/child structures
- Tree patterns where children don't reference parents

## Value vs Reference Types

Ori distinguishes between value types (copied) and reference types (reference counted):

### Value Types

Copied when assigned:

```ori
let x = 42
let y = x  // y is a copy, independent of x

// Modifying y doesn't affect x
```

Value types include:
- `int`, `float`, `bool`
- `char`, `byte`
- `Duration`, `Size`
- Small structs (≤32 bytes, containing only primitives)

### Reference Types

Shared with reference counting:

```ori
let a = [1, 2, 3]
let b = a  // b and a share the same data

// Both refer to the same underlying list
```

Reference types include:
- `str`
- `[T]` (lists)
- `{K: V}` (maps)
- `Set<T>`
- Large structs or those containing references

### How to Know Which Is Which

General rule:
- Primitives and small fixed-size types → value
- Collections and dynamically-sized types → reference

When in doubt, the compiler optimizes appropriately.

## The Clone Trait

To get an independent copy of a reference type, use `.clone()`:

```ori
let a = [1, 2, 3]
let b = a.clone()  // b has its own copy of the data

// Modifying b doesn't affect a
```

### Clone Is Explicit

Ori requires explicit cloning to avoid hidden performance costs:

```ori
// This shares data (cheap)
let shared = expensive_data

// This copies data (potentially expensive)
let copy = expensive_data.clone()
```

### Clone Trait Definition

```ori
trait Clone {
    @clone (self) -> Self
}
```

### What Implements Clone

- All primitives
- All collections (when element types implement Clone)
- `Option<T>` and `Result<T, E>` (when inner types implement Clone)
- Derivable for user types

```ori
#derive(Clone)
type Point = { x: int, y: int }

let p1 = Point { x: 10, y: 20 }
let p2 = p1.clone()  // Independent copy
```

### Deep Clone

Cloning is recursive — cloning a container clones its elements:

```ori
let lists = [[1, 2], [3, 4]]
let copy = lists.clone()  // Both outer and inner lists are cloned
```

## Closures and Capture

Closures capture variables by value at creation time:

```ori
@make_adder (n: int) -> (int) -> int = run(
    let add_n = x -> x + n,  // Captures n by value
    add_n,
)

let add_5 = make_adder(n: 5)
let add_10 = make_adder(n: 10)

add_5(3)   // 8
add_10(3)  // 13
```

### Snapshot Semantics

The closure sees a snapshot of values at creation:

```ori
let x = 10
let f = () -> x  // Captures x = 10

// Later changes don't affect f's captured value
let x = 20  // Shadowing, creates new binding
f()  // Still returns 10 (captured value)
```

### No Outer Mutation

Closures cannot mutate outer scope:

```ori
// This won't work as you might expect
let counter = 0
let increment = () -> run(
    counter = counter + 1,  // ERROR: can't mutate outer scope
)
```

Instead, return the new value or use explicit state:

```ori
@make_counter () -> () -> int = run(
    let count = { value: 0 },
    () -> run(
        count.value = count.value + 1,
        count.value,
    ),
)
```

## Tail Call Optimization

Ori guarantees tail call optimization (TCO) for recursive functions:

```ori
@countdown (n: int) -> void =
    if n <= 0 then () else countdown(n: n - 1)

countdown(n: 1000000)  // No stack overflow
```

A call is in tail position if it's the last thing before the function returns.

### Tail Position Examples

```ori
// Tail call — optimized
@factorial (n: int, acc: int) -> int =
    if n <= 1 then acc else factorial(n: n - 1, acc: n * acc)

// NOT tail call — multiplication happens after the recursive call
@factorial_not_tail (n: int) -> int =
    if n <= 1 then 1 else n * factorial_not_tail(n: n - 1)
```

### Converting to Tail Recursive

Use an accumulator parameter:

```ori
// Not tail recursive
@sum (numbers: [int]) -> int =
    if is_empty(collection: numbers) then
        0
    else
        numbers[0] + sum(numbers: numbers.skip(count: 1))

// Tail recursive (with accumulator)
@sum_tail (numbers: [int], acc: int) -> int =
    if is_empty(collection: numbers) then
        acc
    else
        sum_tail(numbers: numbers.skip(count: 1), acc: acc + numbers[0])
```

## ARC Safety Invariants

The Ori language maintains these invariants to ensure ARC safety:

### 1. No Shared Mutable References

Only one reference can mutate data at a time:

```ori
let a = [1, 2, 3]
let b = a        // Shares data
a[0] = 10        // Creates new list for a, b still has [1, 2, 3]
```

### 2. Closures Capture by Value

No closure can hold a mutable reference to outer scope:

```ori
let x = 10
let f = () -> x  // Copies x, doesn't reference it
```

### 3. No Self-Referential Structures

Types cannot contain references to their own instances:

```ori
// Not allowed: Node can't point to itself
type Node = { value: int, next: Option<Node> }

// Allowed: Indices instead of references
type NodeIndex = int
type Graph = { nodes: [Node], edges: [(NodeIndex, NodeIndex)] }
```

### 4. Immutable by Default

Module-level bindings must use `$`:

```ori
pub let $CONFIG = { ... }  // Immutable, safe to share
```

## Memory Patterns

### Avoid Unnecessary Cloning

```ori
// Expensive: clones for each iteration
for item in items.clone() do
    process(item: item)

// Cheap: iterates without cloning
for item in items do
    process(item: item)
```

### Share Immutable Data

```ori
// If you don't need to modify, share
let shared = large_data
use_data(data: shared)
use_data_again(data: shared)

// Only clone when you need independence
let modified = large_data.clone()
modified[0] = new_value
```

### Use Structural Sharing

Ori collections use structural sharing internally:

```ori
let a = [1, 2, 3, 4, 5]
let b = [...a, 6]  // Shares structure with a where possible
```

## Complete Example

```ori
// Immutable tree using indices instead of references
type NodeId = int

type TreeNode<T> = {
    id: NodeId,
    value: T,
    children: [NodeId],
}

type Tree<T> = {
    nodes: {NodeId: TreeNode<T>},
    root: Option<NodeId>,
    next_id: NodeId,
}

impl<T> Tree<T> {
    @new () -> Tree<T> =
        Tree { nodes: {}, root: None, next_id: 0 }

    @add_node (self, value: T, parent: Option<NodeId>) -> (Tree<T>, NodeId) = run(
        let id = self.next_id,
        let node = TreeNode { id, value, children: [] },

        // Add node to nodes map
        let nodes = { ...self.nodes, id: node },

        // Update parent's children if parent exists
        let nodes = match(
            parent,
            Some(parent_id) -> run(
                let parent_node = nodes[parent_id],
                match(
                    parent_node,
                    Some(p) -> {
                        ...nodes,
                        parent_id: TreeNode { ...p, children: [...p.children, id] },
                    },
                    None -> nodes,
                ),
            ),
            None -> nodes,
        ),

        // Set root if this is first node
        let root = if is_none(option: self.root) then Some(id) else self.root,

        (Tree { nodes, root, next_id: id + 1 }, id),
    )

    @get_node (self, id: NodeId) -> Option<TreeNode<T>> =
        self.nodes[id]

    @children (self, id: NodeId) -> [TreeNode<T>] = run(
        let node = self.nodes[id],
        match(
            node,
            Some(n) -> for child_id in n.children yield match(
                self.nodes[child_id],
                Some(c) -> c,
                None -> continue,
            ),
            None -> [],
        ),
    )
}

@test_tree tests _ () -> void = run(
    let tree = Tree<str>.new(),

    let (tree, root) = tree.add_node(value: "root", parent: None),
    let (tree, child1) = tree.add_node(value: "child1", parent: Some(root)),
    let (tree, child2) = tree.add_node(value: "child2", parent: Some(root)),

    assert_some(option: tree.get_node(id: root)),
    assert_eq(actual: len(collection: tree.children(id: root)), expected: 2),
)

// Demonstrates safe closure capture
@make_processor<T: Clone> (config: Config) -> (T) -> Result<T, Error> = run(
    // config is captured by value
    let process = item -> run(
        if config.validate then
            validate(item: item)?,
        Ok(transform(item: item, config: config)),
    ),
    process,
)

// Tail recursive processing
@process_list<T> (items: [T], processor: (T) -> T, acc: [T]) -> [T] =
    if is_empty(collection: items) then
        acc
    else
        process_list(
            items: items.iter().skip(count: 1).collect(),
            processor: processor,
            acc: [...acc, processor(items[0])],
        )

@test_process_list tests @process_list () -> void = run(
    let items = [1, 2, 3, 4, 5],
    let result = process_list(
        items: items,
        processor: x -> x * 2,
        acc: [],
    ),
    assert_eq(actual: result, expected: [2, 4, 6, 8, 10]),
)
```

## Quick Reference

### Reference Counting

```ori
let a = value         // ref count = 1
let b = a             // ref count = 2
// b drops            // ref count = 1
// a drops            // ref count = 0, freed
```

### Clone

```ori
let copy = original.clone()  // Independent copy
```

### Value vs Reference

| Value Types | Reference Types |
|-------------|-----------------|
| `int`, `float`, `bool` | `str`, `[T]`, `{K: V}` |
| `char`, `byte` | `Set<T>` |
| Small structs | Large structs |

### Closure Capture

```ori
let x = 10
let f = () -> x  // Captures x by value (snapshot)
```

### Tail Call

```ori
// Tail position — optimized
@fn (n: int, acc: int) -> int =
    if done then acc else fn(n: n - 1, acc: acc + n)
```

### Safety Invariants

1. No shared mutable references
2. Closures capture by value
3. No self-referential structures
4. Immutable module-level bindings

## What's Next

Now that you understand the memory model:

- **[Formatting Rules](/guide/21-formatting)** — Code style guidelines

