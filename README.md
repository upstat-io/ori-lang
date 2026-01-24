<div align="center">

# Sigil

**A statically-typed, expression-based language with declarative patterns, mandatory testing, and explicit effects.**

[Getting Started](#quick-start) | [Documentation](docs/sigil_lang/design/) | [Examples](examples/) | [Contributing](CONTRIBUTING.md)

</div>

> **Experimental:** Sigil is under active development and not ready for production use. The language, APIs, and tooling may change without notice.

## Why Sigil?

Sigil prioritizes correctness, explicitness, and predictability. Every design decision serves these goals—making code easier to reason about, test, and maintain.

### Key Features

- **Declarative Patterns** — Express *what* you want, not *how*. First-class `recurse`, `map`, `filter`, `fold`, and `parallel` patterns replace error-prone manual loops.

- **Mandatory Testing** — Every function requires tests to compile. No exceptions. Testing is part of the language, not an afterthought.

- **Explicit Effects** — Side effects are tracked through capabilities (`uses Http`, `uses FileSystem`). Pure functions have no `uses` clause. Effects are injectable and testable.

- **No Null or Exceptions** — `Result<T, E>` and `Option<T>` make errors visible in function signatures and impossible to ignore.

- **Immutable by Default** — All values are immutable. Shadowing is allowed, mutation requires `mut`. This eliminates entire classes of bugs.

- **Clear Visual Markers** — `@` for functions, `$` for config, `.name:` for named parameters. Code is easy to scan and understand at a glance.

- **Type Inference** — Strong static typing with inference. Types are checked at compile time but rarely need to be written explicitly.

## Quick Start

Install Sigil:

```bash
curl -sSf https://raw.githubusercontent.com/sigil-lang/sigil/master/install.sh | sh
```

Write your first program (`hello.si`):

```sigil
@main () -> void = print(.msg: "Hello, Sigil!")
```

Run it:

```bash
sigil run hello.si
```

## Examples

### Declarative Recursion with Memoization

```sigil
@fibonacci (term: int) -> int = recurse(
    .cond: term <= 1,
    .base: term,
    .step: self(term - 1) + self(term - 2),
    .memo: true,
)

@test_fibonacci tests @fibonacci () -> void = run(
    // instant with memoization
    assert_eq(
        .actual: fibonacci(
            .term: 50,
        ),
        .expected: 12586269025,
    ),
)
```

### Data Transformation Pipelines

```sigil
@process_users (users: [User]) -> [str] = run(
    let active = filter(
        .over: users,
        .predicate: u -> u.is_active,
    ),
    let sorted = sort_by(
        .over: active,
        .key: u -> u.name,
    ),
    map(
        .over: sorted,
        .transform: u -> u.email,
    ),
)

@test_process_users tests @process_users () -> void = run(
    let users = [
        User { name: "Bob", email: "bob@x.com", is_active: true },
        User { name: "Alice", email: "alice@x.com", is_active: true },
        User { name: "Charlie", email: "charlie@x.com", is_active: false },
    ],
    assert_eq(
        .actual: process_users(
            .users: users,
        ),
        .expected: ["alice@x.com", "bob@x.com"],
    ),
)
```

### Explicit Error Handling

```sigil
@divide (numerator: int, denominator: int) -> Result<int, str> =
    if denominator == 0 then Err("division by zero")
    else Ok(numerator / denominator)

// ? propagates Err automatically
@safe_compute (x: int, y: int) -> Result<int, str> = try(
    let quotient = divide(
        .numerator: 100,
        .denominator: x,
    )?,
    let remainder = divide(
        .numerator: quotient,
        .denominator: y,
    )?,
    Ok(remainder),
)

@test_divide tests @divide () -> void = run(
    assert_eq(
        .actual: divide(
            .numerator: 10,
            .denominator: 2,
        ),
        .expected: Ok(5),
    ),
    assert_eq(
        .actual: divide(
            .numerator: 10,
            .denominator: 0,
        ),
        .expected: Err("division by zero"),
    ),
)
```

### Pattern Matching

```sigil
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)
    | Triangle(base: float, height: float)

@area (shape: Shape) -> float = match(
    shape,
    Circle { radius } -> 3.14159 * radius * radius,
    Rectangle { width, height } -> width * height,
    Triangle { base, height } -> 0.5 * base * height,
)

@test_area tests @area () -> void = run(
    assert_eq(
        .actual: area(
            .shape: Circle(
                .radius: 1.0,
            ),
        ),
        .expected: 3.14159,
    ),
    assert_eq(
        .actual: area(
            .shape: Rectangle(
                .width: 4.0,
                .height: 5.0,
            ),
        ),
        .expected: 20.0,
    ),
)
```

### Parallel Execution

```sigil
// All three requests execute concurrently
// Returns struct with profile, posts, notifications fields
@fetch_dashboard (user_id: str) -> Dashboard = parallel(
    .profile: fetch_profile(
        .user_id: user_id,
    ),
    .posts: fetch_recent_posts(
        .user_id: user_id,
    ),
    .notifications: fetch_notifications(
        .user_id: user_id,
    ),
)
```

## Installation

### Binary Distributions

Official binaries are available at [GitHub Releases](https://github.com/upstat-io/sigil-lang/releases).

**Platforms:** Linux (x86_64, aarch64), macOS (x86_64, Apple Silicon), Windows (x86_64)

### Install from Source

```bash
git clone https://github.com/upstat-io/sigil-lang
cd sigil
cargo build --release
cp target/release/sigil ~/.local/bin/
```

Requires Rust 1.70+ and a C compiler (for native compilation).

## Usage

```bash
sigil run program.si      # Run a program
sigil test                # Run all tests (parallel)
sigil test file.test.si   # Run specific test file
sigil build program.si    # Compile to native binary
sigil check program.si    # Check test coverage
sigil emit program.si     # Emit generated C code
```

## Documentation

### Language Design

- [Design Overview](docs/sigil_lang/design/00-index.md) — Complete language specification
- [Philosophy](docs/sigil_lang/design/01-philosophy/) — AI-first design principles
- [Syntax](docs/sigil_lang/design/02-syntax/) — Functions, expressions, patterns
- [Type System](docs/sigil_lang/design/03-type-system/) — Types, generics, inference
- [Error Handling](docs/sigil_lang/design/05-error-handling/) — Result, Option, try pattern
- [Pattern Matching](docs/sigil_lang/design/06-pattern-matching/) — match, destructuring, guards

### Quick References

- [Pattern Reference](docs/sigil_lang/design/02-syntax/04-patterns-reference.md) — All patterns with examples
- [Built-in Traits](docs/sigil_lang/design/appendices/C-builtin-traits.md) — Eq, Clone, Serialize, etc.
- [Glossary](docs/sigil_lang/design/glossary.md) — Terminology

## Design Philosophy

Sigil optimizes for **correctness and maintainability**:

| Traditional Approach | Sigil Approach |
|---------------------|----------------|
| Concise syntax | Explicit, predictable syntax |
| Flexible APIs | Constrained, correct-by-construction APIs |
| Runtime flexibility | Compile-time guarantees |
| Optional testing | Mandatory testing |
| Implicit effects | Explicit capabilities |

The result: errors are caught at compile time, code is self-documenting, and behavior is predictable.

## Getting Help

- **Bug reports & feature requests:** [GitHub Issues](https://github.com/upstat-io/sigil-lang/issues)
- **Discussions:** [GitHub Discussions](https://github.com/upstat-io/sigil-lang/discussions)

## Contributing

Sigil is open source and we welcome contributions. Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Running Tests

```bash
cargo test              # Compiler unit tests
sigil test              # Language test suite
```

## License

Sigil is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
