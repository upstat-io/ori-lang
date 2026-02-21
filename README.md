<div align="center">

# Ori

**Code That Proves Itself**

A statically-typed, expression-based language with mandatory testing, causality tracking, and explicit effects.

[Website](https://ori-lang.com) | [Playground](https://ori-lang.com/playground) | [Getting Started](#quick-start) | [Specification](https://ori-lang.com/docs/spec/01-notation) | [Examples](examples/) | [Contributing](CONTRIBUTING.md)

</div>

> **Experimental:** Ori is under active development and not ready for production use. The language, APIs, and tooling may change without notice.

## Why Ori?

**Code that proves itself.**

If it compiles, it has tests. If it has tests, they pass. If you change it, you'll know what broke.

### The Problem

Code without tests is just a hypothesis. Developers forget to write tests, skip them under deadline pressure, and ignore failures. AI assistants are even worse — they write code, forget tests, and "fix" failures by weakening assertions.

### The Solution

Ori makes verification automatic. The compiler enforces what discipline cannot.

- **Every function tested** — No tests, no compile. The compiler ensures coverage.
- **Tests bound to code** — `@test tests @target` creates a compiler-enforced bond
- **Change propagates** — Modify a function, and tests for its callers run automatically
- **Mocking is trivial** — Capabilities make dependency injection built-in

## Core Features

### Mandatory Testing

Every function requires tests. No exceptions. No skipping. No "I'll add tests later."

```ori
@fibonacci (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: fibonacci(n: n - 1) + fibonacci(n: n - 2),
    memo: true,
)

@test_fibonacci tests @fibonacci () -> void = {
    assert_eq(fibonacci(n: 0), 0);
    assert_eq(fibonacci(n: 1), 1);
    assert_eq(fibonacci(n: 10), 55);
}
```

### Dependency-Aware Test Execution

Tests are in the dependency graph. Change `@parse`, and tests for `@compile` (which calls `@parse`) run too.

```ori
@parse (input: str) -> Result<Ast, Error> = ...
@test_parse tests @parse () -> void = ...

@compile (input: str) -> Result<Binary, Error> = {
    let ast = parse(input: input)?;
    generate_code(ast: ast)
}
@test_compile tests @compile () -> void = ...
```

Change `@parse` → compiler runs `@test_parse` AND `@test_compile`.

### Causality Tracking

Ori tracks the impact of every change through your codebase.

**Before you change — know the blast radius:**

```bash
$ ori impact @parse
If @parse changes:
  @compile        → uses @parse directly
  @run_program    → uses @compile
  @format_output  → uses @compile

  12 functions affected
```

**After something breaks — trace it to the source:**

```bash
$ ori why @compile
@compile broke because:
  → @parse changed (src/parser.ori:42)
    - line 42: changed return type from Ast to Result<Ast, Error>
```

Know what breaks before you break it. Know why it broke after.

### Explicit Effects & Trivial Mocking

Side effects are tracked through capabilities. Mocking is just providing a different implementation.

```ori
@fetch_user (id: UserId) -> Result<User, Error> uses Http =
    Http.get("/users/" + str(id))

@test_fetch_user tests @fetch_user () -> void =
    with Http = MockHttp(responses: {"/users/1": mock_user}) in {
        let result = fetch_user(id: 1);
        assert_ok(result);
        assert_eq(result.unwrap().name, "Alice");
    }
```

No test framework. No mocking library. Just the language.

### Contracts

Functions declare and enforce their invariants.

```ori
@sqrt (x: float) -> float
    pre(x >= 0.0)
    post(r -> r >= 0.0)
= newton_raphson(x)

@test_sqrt tests @sqrt () -> void = {
    assert_eq(sqrt(x: 4.0), 2.0);
    assert_panics(sqrt(x: -1.0));
}
```

### Declarative Patterns

Express *what* you want, not *how*. First-class patterns replace error-prone loops.

```ori
@process_users (users: [User]) -> [str] = {
    let active = filter(over: users, predicate: u -> u.is_active);
    let sorted = sort_by(over: active, key: u -> u.name);
    map(over: sorted, transform: u -> u.email)
}

@test_process_users tests @process_users () -> void = {
    let users = [
        User { name: "Bob", email: "bob@x.com", is_active: true },
        User { name: "Alice", email: "alice@x.com", is_active: true },
        User { name: "Charlie", email: "charlie@x.com", is_active: false },
    ];
    assert_eq(process_users(users: users), ["alice@x.com", "bob@x.com"]);
}
```

### No Null or Exceptions

`Result<T, E>` and `Option<T>` make errors visible and impossible to ignore.

```ori
@divide (a: int, b: int) -> Result<int, str> =
    if b == 0 then Err("division by zero")
    else Ok(a / b)

@safe_compute (x: int, y: int) -> Result<int, str> = try {
    let quotient = divide(a: 100, b: x)?;
    let result = divide(a: quotient, b: y)?;
    Ok(result)
}

@test_divide tests @divide () -> void = {
    assert_eq(divide(a: 10, b: 2), Ok(5));
    assert_eq(divide(a: 10, b: 0), Err("division by zero"));
}
```

## Quick Start

Install Ori (latest nightly):

```bash
curl -fsSL https://raw.githubusercontent.com/upstat-io/ori-lang/master/install.sh | sh
```

Write your first program (`hello.ori`):

```ori
@main () -> void = print("Hello, Ori!")
```

Run it:

```bash
ori run hello.ori
```

## Usage

```bash
ori run program.ori      # Run a program
ori test                # Run all tests (parallel)
ori test file.test.ori   # Run specific test file
ori build program.ori    # Compile to native binary
ori check program.ori    # Check test coverage
ori emit program.ori     # Emit generated C code
```

## Installation

### Quick Install (Recommended)

```bash
# Install latest nightly (default during alpha)
curl -fsSL https://raw.githubusercontent.com/upstat-io/ori-lang/master/install.sh | sh

# Install specific version
curl -fsSL https://raw.githubusercontent.com/upstat-io/ori-lang/master/install.sh | sh -s -- --version v0.1.0-alpha.2
```

### Binary Distributions

Official binaries are available at [GitHub Releases](https://github.com/upstat-io/ori-lang/releases).

**Platforms:** Linux (x86_64, aarch64), macOS (x86_64, Apple Silicon), Windows (x86_64)

### Install from Source

```bash
git clone https://github.com/upstat-io/ori-lang
cd ori-lang
cargo build --release
cp target/release/ori ~/.local/bin/
```

Requires Rust 1.70+.

## Documentation

- [Website](https://ori-lang.com) — Official website with guides and documentation
- [Playground](https://ori-lang.com/playground) — Try Ori in your browser
- [Language Specification](https://ori-lang.com/docs/spec/01-notation) — Formal language definition
- [Compiler Design](https://ori-lang.com/docs/compiler-design/01-architecture) — Compiler architecture and internals
- [Roadmap](https://ori-lang.com/roadmap) — Development roadmap and progress
- [Proposals](docs/ori_lang/proposals/) — Design decisions and rationale

## Design Philosophy

**Code that proves itself.** Every function tested. Every change traced. Every effect explicit.

Ori makes verification automatic — the compiler enforces what discipline alone cannot.

| Traditional Approach | Ori Approach |
|---------------------|----------------|
| Tests are optional | Tests are mandatory |
| Tests are external | Tests are in the dependency graph |
| Change and hope | Change and know what broke |
| Mock with frameworks | Mock with capabilities |
| Runtime errors | Compile-time guarantees |
| Hidden effects | Explicit capabilities |

### The Virtuous Cycle

```
Capabilities make mocking easy
    → Tests are fast
        → Dependency-aware testing is practical
            → Mandatory testing isn't painful
                → Code integrity is enforced
                    → Code that works, stays working
```

## Getting Help

- **Website:** [ori-lang.com](https://ori-lang.com)
- **Bug reports & feature requests:** [GitHub Issues](https://github.com/upstat-io/ori-lang/issues)
- **Discussions:** [GitHub Discussions](https://github.com/upstat-io/ori-lang/discussions)

## Contributing

Ori is open source and we welcome contributions. Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Running Tests

```bash
cargo test              # Compiler unit tests
ori test              # Language test suite
```

## License

Ori is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
