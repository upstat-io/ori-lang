<div align="center">

# Sigil

**A general-purpose language built on declarative patterns and mandatory testing.**

[Getting Started](#quick-start) | [Documentation](docs/) | [Examples](examples/) | [Contributing](CONTRIBUTING.md)

</div>

> **Experimental:** Sigil is under active development and not ready for production use. The language, APIs, and tooling may change without notice.

## Why Sigil?

- **Declarative patterns:** Express *what* you want with first-class `recurse`, `map`, `filter`, `fold`, and `parallel` constructs—not *how* to do it with manual loops.

- **Mandatory testing:** Every function requires tests to compile. Testing isn't an afterthought; it's enforced by the language.

- **Explicit syntax:** Clear visual markers—`@` for functions, `$` for config, `.name:` for named parameters—make code easy to scan and understand.

## Quick Start

Install Sigil:

```bash
curl -sSf https://raw.githubusercontent.com/upstat-io/sigil-lang/master/install.sh | sh
```

Write your first program (`hello.si`):

```sigil
@main () -> void = print("Hello, Sigil!")
```

Run it:

```bash
sigil run hello.si
```

## Example

```sigil
@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true
)

@test_fib tests @fibonacci () -> void = run(
    assert_eq(fibonacci(0), 0),
    assert_eq(fibonacci(10), 55)
)
```

## Installation

### Binary Distributions

Official binaries are available at [GitHub Releases](https://github.com/upstat-io/sigil-lang/releases).

**Platforms:** Linux (x86_64, aarch64), macOS (x86_64, Apple Silicon), Windows (x86_64)

### Install from Source

See [Building from Source](#building-from-source) below.

## Usage

```bash
sigil run program.si      # Run a program
sigil test                # Run all tests
sigil build program.si    # Compile to native binary
sigil check program.si    # Check test coverage
```

## Documentation

- [Language Design](docs/language-design.md) — Syntax, patterns, and semantics
- [Type System](docs/type-system.md) — Static typing and inference

## Getting Help

- **Bug reports & feature requests:** [GitHub Issues](https://github.com/upstat-io/sigil-lang/issues)
- **Discussions:** [GitHub Discussions](https://github.com/upstat-io/sigil-lang/discussions)

## Contributing

Sigil is open source and we welcome contributions. Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Building from Source

```bash
git clone https://github.com/upstat-io/sigil-lang
cd sigil-lang
cargo build --release
cp target/release/sigil ~/.local/bin/
```

Requires Rust 1.70+ and a C compiler (for `sigil build`).

### Project Structure

```
sigil-lang/
├── compiler/sigilc/    # Compiler (Rust)
├── library/std/        # Standard library (Sigil)
├── docs/               # Documentation
├── examples/           # Example programs
└── tests/              # Test suites
```

### Running Tests

```bash
cargo test              # Compiler tests
sigil test              # Language tests
```

## License

Sigil is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
