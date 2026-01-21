<div align="center">

# Sigil

**A general-purpose language built on declarative patterns and mandatory testing, designed for AI-assisted development.**

[Getting Started](#quick-start) | [Documentation](docs/sigil_lang/design/) | [Examples](examples/) | [Contributing](CONTRIBUTING.md)

</div>

> **Experimental:** Sigil is under active development and not ready for production use. The language, APIs, and tooling may change without notice.

## Why Sigil?

Sigil is designed from the ground up with AI-authored code as the primary optimization target. Every design decision prioritizes correctness, explicitness, and predictability—the qualities that make AI-generated code reliable.

### Key Features

- **Declarative Patterns** — Express *what* you want, not *how*. First-class `recurse`, `map`, `filter`, `fold`, and `parallel` patterns replace error-prone manual loops.

- **Mandatory Testing** — Every function requires tests to compile. No exceptions. Testing is part of the language, not an afterthought.

- **Explicit Error Handling** — No exceptions. `Result<T, E>` and `Option<T>` make errors visible in function signatures and impossible to ignore.

- **Immutable by Default** — All values are immutable. Shadowing is allowed, mutation is not. This eliminates entire classes of bugs.

- **Clear Visual Markers** — `@` for functions, `$` for config, `.name:` for named parameters. Code is easy to scan and understand at a glance.

## Quick Start

Install Sigil:

```bash
curl -sSf https://raw.githubusercontent.com/sigil-lang/sigil/master/install.sh | sh
```

Write your first program (`hello.si`):

```sigil
@main () -> void = print("Hello, Sigil!")
```

Run it:

```bash
sigil run hello.si
```

## Examples

### Declarative Recursion with Memoization

```sigil
@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true
)

@test_fibonacci tests @fibonacci () -> void = run(
    assert_eq(fibonacci(0), 0),
    assert_eq(fibonacci(1), 1),
    assert_eq(fibonacci(10), 55),
    assert_eq(fibonacci(50), 12586269025)  // instant with memoization
)
```

### Data Transformation Pipelines

```sigil
@process_users (users: [User]) -> [str] = run(
    active = users.filter(u -> u.is_active),
    sorted = active.sort_by(u -> u.name),
    sorted.map(u -> u.email)
)

@test_process_users tests @process_users () -> void = run(
    users = [
        User { name: "Bob", email: "bob@x.com", is_active: true },
        User { name: "Alice", email: "alice@x.com", is_active: true },
        User { name: "Charlie", email: "charlie@x.com", is_active: false }
    ],
    assert_eq(process_users(users), ["alice@x.com", "bob@x.com"])
)
```

### Explicit Error Handling

```sigil
@divide (a: int, b: int) -> Result<int, str> =
    if b == 0 then Err("division by zero")
    else Ok(a / b)

@safe_compute (x: int, y: int) -> Result<int, str> = try(
    quotient = divide(100, x),   // propagates Err automatically
    remainder = divide(quotient, y),
    Ok(remainder)
)

@test_divide tests @divide () -> void = run(
    assert_eq(divide(10, 2), Ok(5)),
    assert_eq(divide(10, 0), Err("division by zero"))
)
```

### Pattern Matching

```sigil
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)
    | Triangle(base: float, height: float)

@area (shape: Shape) -> float = match(shape,
    .Circle: { radius } -> 3.14159 * radius * radius,
    .Rectangle: { width, height } -> width * height,
    .Triangle: { base, height } -> 0.5 * base * height
)

@test_area tests @area () -> void = run(
    assert_eq(area(Circle(radius: 1.0)), 3.14159),
    assert_eq(area(Rectangle(width: 4.0, height: 5.0)), 20.0)
)
```

### Parallel Execution

```sigil
@fetch_dashboard (user_id: str) -> Dashboard = parallel(
    .profile: fetch_profile(user_id),
    .posts: fetch_recent_posts(user_id),
    .notifications: fetch_notifications(user_id)
)
// Returns struct with profile, posts, notifications fields
// All three requests execute concurrently
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

Sigil optimizes for **AI code generation quality**:

| Traditional Priority | Sigil Priority |
|---------------------|----------------|
| Concise syntax | Explicit, predictable syntax |
| Flexible APIs | Constrained, correct-by-construction APIs |
| Runtime flexibility | Compile-time guarantees |
| Optional testing | Mandatory testing |
| Implicit behavior | Everything explicit |

The result: AI-generated Sigil code is more likely to be correct on the first try, and errors are caught at compile time rather than in production.

## Getting Help

- **Bug reports & feature requests:** [GitHub Issues](https://github.com/upstat-io/sigil-lang/issues)
- **Discussions:** [GitHub Discussions](https://github.com/upstat-io/sigil-lang/discussions)

## Contributing

Sigil is open source and we welcome contributions. Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Project Structure

```
sigil/
├── compiler/sigilc/    # Compiler implementation (Rust)
│   └── src/
│       ├── lexer/      # Tokenizer
│       ├── parser/     # Recursive descent parser
│       ├── types/      # Type checker
│       ├── eval/       # Tree-walking interpreter
│       └── codegen/    # C code generator
├── library/std/        # Standard library (Sigil)
├── docs/               # Documentation
│   └── sigil_lang/design/  # Language design specification
├── examples/           # Example programs
└── tests/              # Test suites
    ├── run-pass/       # Tests that should compile and run
    └── compile-fail/   # Tests that should fail to compile
```

### Running Tests

```bash
cargo test              # Compiler unit tests
sigil test              # Language test suite
```

## License

Sigil is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
