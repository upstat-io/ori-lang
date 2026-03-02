# Core Design Principles

This document describes the four core principles that guide Ori's design: Explicitness, Consistency, Minimalism, and Pragmatism.

---

## Explicitness

### Make All Behavior Visible

Every execution path should be visible in source code:

```ori
// Clear: try propagates errors
@process () -> Result<Data, Error> = try(
    let data = fetch()?,
    let parsed = parse(data)?,
    Ok(transform(parsed)),
)
```

### Explicit Over Implicit

Prefer explicit over implicit:

| Avoid | Prefer |
|-------|--------|
| Implicit value conversions | Explicit `int(value)`, `str(value)` |
| Hidden side effects | Visible in function signature |
| Magic behavior | Clear, predictable operations |
| Implicit dependencies | Explicit `use` imports |

**Note:** Some contextual adaptations are allowed:
- Concrete types coerce to `dyn Trait` at function boundaries
- Operators desugar to trait method calls (`==` calls `equals`)

### Surface All Side Effects

Side effects should be visible:

```ori
// Return type makes it clear this can fail
@read_file (path: str) -> Result<str, IoError>

// uses clause indicates suspension and dependencies
@fetch (url: str) -> Result<Data, Error> uses Http, Async
```

### No Magic

If code doesn't look like it calls a function, it shouldn't call a function:

- No automatic method synthesis
- No implicit operator overloading
- No hidden decorator effects
- No runtime reflection magic

---

## Consistency

### One Way to Do Common Things

Like Python's "one obvious way," Ori provides single canonical patterns:

```ori
// One way to define functions
@add (left: int, right: int) -> int = left + right

// One way to define types
type Point = { x: int, y: int }

// One way to handle errors
@process () -> Result<Data, Error> = try(...)
```

### Uniform Syntax Patterns

Similar things look similar:

```ori
// All patterns follow the same named property structure
@sum (numbers: [int]) -> int = fold(
    .over: numbers,
    .initial: 0,
    .operation: +,
)

@doubled (numbers: [int]) -> [int] = map(
    .over: numbers,
    .transform: number -> number * 2,
)

// Consistent across all patterns
retry(
    .operation: ...,
    .attempts: 3,
)

recurse(
    .condition: ...,
    .base: ...,
    .step: ...,
)

parallel(
    .profile: fetch_profile(),
    .posts: fetch_posts(),
)
```

### Predictable Behavior

Same construct behaves the same everywhere:

```ori
// Binding works identically in all contexts
let value = 5
let result = compute()
let { name, age } = user
let [head, ..tail] = items
```

### Orthogonal Features

Features compose without surprises:

```ori
// Patterns work with capabilities
@fetch (url: str) -> Result<Data, Error> uses Http, Async = retry(
    .operation: Http.get(
        .url: url,
    ),
    .attempts: 3,
)

// Generic types work with traits
@sort<T> (items: [T]) -> [T] where T: Comparable = ...
```

---

## Minimalism

### Every Feature Justifies Its Complexity

Before adding a feature, ask:
- What problem does it solve?
- Is there an existing way to solve it?
- What's the complexity cost?
- How does it interact with other features?
- Can it be added later if needed?

### Small Core + Libraries

Prefer a small, powerful core over a large feature set:

```ori
// Core patterns
recurse, map, filter, fold, match, run, try

// Everything else in libraries
use std.math { sin, cos, sqrt }
use std.string { split, join, trim }
```

### Fewer Keywords

Context-sensitive keywords reduce the reserved word count:

```ori
// map, filter, fold are keywords in pattern context
@sum (numbers: [int]) -> int = fold(
    .over: numbers,
    .initial: 0,
    .operation: +,
)

// But can be used as identifiers elsewhere
// This is a map variable
let map = { "key": "value" }
```

### Remove Over Workaround

If a feature causes problems, remove it rather than adding workarounds.

---

## Pragmatism

### Real-World Use Over Theory

Practical utility trumps theoretical purity:

```ori
// ARC memory management - not theoretically optimal,
// but simple and works well in practice
// (vs. ownership/borrow checking complexity)
```

### Fast Compilation Matters

Quick feedback loops are more valuable than micro-optimizations in compilation:

- Incremental compilation
- Parallel compilation
- Cached intermediate results

### Readable Error Messages

Error messages should help fix problems, not explain implementation details:

```
error[E0308]: mismatched types
  --> src/main.ori:15:10
   |
15 |     result = count + "hello"
   |              ^^^^^ expected int, found str
   |
   = help: try: str(count) + "hello"
```

### Support Tooling from Day One

Design for IDE support, formatting, and analysis:

- Parseable without type information
- Deterministic formatting
- Incremental analysis
- Rich error metadata

---

## How Principles Interact

### Explicitness + Consistency

Explicit syntax is applied consistently:

```ori
// @ always means function definition
@greet (name: str) -> str

// call (no @ in calls)
greet(
    .name: "Alice",
)

// $ always means config
$timeout = 30s
```

### Minimalism + Pragmatism

Keep the core small, but include what's practically needed:

```ori
// Option and Result are built-in (not library)
// because they're used everywhere
// no import needed
Option<T>
Result<T, E>
```

### Consistency + Minimalism

One way means fewer concepts to learn:

```ori
// All type definitions use 'type'
// struct
type Point = { x: int, y: int }
// newtype
type UserId = str
// sum type
type Status = Pending | Running | Done

// Not different keywords for each
```

---

## Applying the Principles

When designing or evaluating a feature:

1. **Is it explicit?** Can you see what it does by reading the code?
2. **Is it consistent?** Does it follow established patterns?
3. **Is it minimal?** Is it the simplest solution that works?
4. **Is it pragmatic?** Does it solve real problems?

### Example: Error Handling

**Explicit:** Error types are in signatures, `try` makes propagation visible

**Consistent:** All patterns work the same way, one syntax for errors

**Minimal:** Just `Result<T, E>` and `try`, no exceptions, no multiple systems

**Pragmatic:** Patterns handle the boilerplate, errors are practical to use

---

## See Also

- [AI-First Design](01-ai-first-design.md)
- [Main Index](../00-index.md)
