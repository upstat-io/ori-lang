# Sigil

A general-purpose language built on declarative patterns and mandatory testing.

> **Experimental:** Sigil is under active development and not ready for production use. The language, APIs, and tooling may change without notice.

## Overview

Sigil treats common computational patterns—recursion, mapping, filtering, parallel execution—as first-class language constructs rather than library functions. Combined with mandatory testing, Sigil encourages code that is clear, correct, and easy to reason about.

- **Declarative patterns** - Express *what* you want, not *how* to do it
- **Mandatory testing** - Every function requires tests to compile
- **Explicit syntax** - `@` for functions, `$` for config, `.name:` for parameters
- **Strict typing** - All types known at compile time, with inference where obvious
- **Built-in concurrency** - `parallel` pattern for concurrent execution

## Quick Example

```sigil
// Function definition with @ prefix
@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true
)

// Test definition
@test_fib tests @fibonacci () -> void = run(
    assert_eq(fibonacci(0), 0),
    assert_eq(fibonacci(10), 55)
)
```

## Installation

### From Source

```bash
git clone https://github.com/yourusername/sigil
cd sigil
cargo build --release
cp target/release/sigil ~/.local/bin/
```

### Requirements

- Rust 1.70+ (for building)
- C compiler (for `sigil build`)

## Usage

```bash
# Run a program
sigil run program.si

# Run all tests
sigil test

# Compile to native binary (via C)
sigil build program.si -o program

# Emit C code
sigil emit program.si -o program.c

# Check test coverage
sigil check program.si

# Interactive REPL
sigil repl
```

## Language Features

### Functions

```sigil
@function_name (param: type) -> return_type = expression
```

### Config Variables

```sigil
$timeout = 30
$api_url = "https://api.example.com"
```

### Pattern-Based Operations

```sigil
// Map over a collection
@squares (nums: [int]) -> [int] = map(nums, n -> n * n)

// Filter elements
@evens (nums: [int]) -> [int] = filter(nums, n -> n % 2 == 0)

// Fold/reduce
@sum (nums: [int]) -> int = fold(nums, 0, (acc, n) -> acc + n)

// Parallel execution
@fetch_all (a: int, b: int) -> { first: int, second: int } = parallel(
    .first: expensive_calc(a),
    .second: expensive_calc(b)
)
```

### Conditionals

```sigil
@sign (n: int) -> str =
    if n > 0 :then "positive"
    else if n < 0 :then "negative"
    else "zero"
```

## Project Structure

```
sigil/
├── compiler/sigilc/    # The Sigil compiler (Rust)
├── library/            # Standard library (Sigil)
├── docs/               # Documentation
├── examples/           # Example programs
└── tests/              # Test suites
    ├── run-pass/       # Tests that should pass
    └── compile-fail/   # Tests that should fail
```

## Documentation

- [Language Design](docs/language-design.md)
- [Type System](docs/type-system.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
