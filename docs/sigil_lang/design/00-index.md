# Sigil Language Design Specification

Sigil is a general-purpose programming language built on declarative patterns and mandatory testing, designed with AI-authored code as the primary optimization target.

---

## Quick Start

```sigil
// Functions use @ prefix
@factorial (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: 1,
    .step: n * self(n - 1),
    .memo: true
)

// Config uses $ prefix
$max_retries = 3
$timeout = 30s

// Tests are mandatory
@test_factorial tests @factorial () -> void = run(
    assert_eq(factorial(0), 1),
    assert_eq(factorial(5), 120)
)

// Patterns replace boilerplate
@fetch_data (url: str) -> async Result<Data, Error> = retry(
    .op: http_get(url).await,
    .attempts: $max_retries,
    .backoff: exponential(base: 100ms, max: 5s)
)
```

---

## Document Organization

### Core Language

| Section | Description |
|---------|-------------|
| [Philosophy](01-philosophy/index.md) | AI-first design, core principles |
| [Syntax](02-syntax/index.md) | Functions, config, expressions, patterns |
| [Type System](03-type-system/index.md) | Primitives, compounds, generics, inference |

### Type Features

| Section | Description |
|---------|-------------|
| [Traits](04-traits/index.md) | Behavior abstraction, impl blocks, derive |
| [Error Handling](05-error-handling/index.md) | Result, Option, try pattern, panics |
| [Pattern Matching](06-pattern-matching/index.md) | match, destructuring, guards, exhaustiveness |

### Functions & Memory

| Section | Description |
|---------|-------------|
| [Functions](07-functions/index.md) | First-class functions, lambdas, closures |
| [Memory Model](08-memory-model/index.md) | ARC, value semantics, structural sharing |

### Modules & Concurrency

| Section | Description |
|---------|-------------|
| [Modules](09-modules/index.md) | File = module, imports, visibility |
| [Async](10-async/index.md) | async/await, structured concurrency, channels |

### Effects & Testing

| Section | Description |
|---------|-------------|
| [Capabilities](14-capabilities/index.md) | Effect management, testable side effects |
| [Testing](11-testing/index.md) | Mandatory tests, test syntax, compile-fail tests |

### Tooling & Documentation

| Section | Description |
|---------|-------------|
| [Tooling](12-tooling/index.md) | Semantic addressing, structured errors, LSP, formatter |
| [Documentation](13-documentation/index.md) | Doc comment syntax |

### Reference

| Section | Description |
|---------|-------------|
| [Appendices](appendices/) | Grammar, error codes, built-in traits, pattern reference |
| [Glossary](glossary.md) | Terminology definitions |

---

## Design Principles Summary

### 1. AI-First

Sigil is optimized for AI generation and modification:
- **Explicit** - No hidden control flow or magic behavior
- **Consistent** - One way to do common things
- **Declarative** - AI says WHAT, not HOW
- **Verifiable** - Mandatory tests validate AI output

### 2. Pattern-Based

Common operations are built-in patterns:

| Pattern | Purpose |
|---------|---------|
| `recurse` | Recursive functions with memoization |
| `map`, `filter`, `fold` | Collection operations |
| `try` | Error propagation |
| `parallel` | Concurrent execution |
| `retry`, `timeout`, `cache` | Resilience patterns |
| `match` | Pattern matching |

### 3. Mandatory Testing

Every function requires at least one test:

```sigil
@add (a: int, b: int) -> int = a + b

// Compilation fails without this test
@test_add tests @add () -> void = run(
    assert_eq(add(2, 3), 5)
)
```

### 4. Compositional Types

- No subtyping or inheritance
- Traits provide behavior sharing
- Explicit generics or `dyn` for polymorphism

### 5. Semantic Addressing

Every code element is addressable for AI edits:

```
@function_name                  // function
@function_name.attempts         // pattern property
$config_name                    // config variable
type TypeName.field             // struct field
```

---

## File Extension

Sigil source files use the `.si` extension.

---

## See Also

- [AI-First Design Philosophy](01-philosophy/01-ai-first-design.md)
- [Basic Syntax](02-syntax/01-basic-syntax.md)
- [Pattern System Overview](02-syntax/03-patterns-overview.md)
