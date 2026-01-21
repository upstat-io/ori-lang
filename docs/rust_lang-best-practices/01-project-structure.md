# Project Structure

Guidelines for organizing Rust projects and modules based on official Cargo documentation.

## Quick Reference

- [ ] Use standard Cargo directory layout
- [ ] Use `foo.rs` for modules (Rust 2018+ style) or `foo/mod.rs` for modules with submodules
- [ ] Keep `lib.rs` as a thin facade with re-exports
- [ ] Binary crates should be thin wrappers around library crates
- [ ] Use workspaces for multi-crate projects

## Standard Cargo Layout

From [The Cargo Book - Package Layout](https://doc.rust-lang.org/cargo/guide/project-layout.html):

```
my_crate/
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── lib.rs          # Library crate root
│   ├── main.rs         # Default binary crate root
│   └── bin/            # Additional binary crates
│       ├── named-executable.rs
│       └── another-executable.rs
├── tests/              # Integration tests
│   └── integration_test.rs
├── examples/           # Example programs
│   └── example.rs
├── benches/            # Benchmarks
│   └── benchmark.rs
└── build.rs            # Build script (optional)
```

Cargo automatically discovers:
- `src/lib.rs` → library crate
- `src/main.rs` → binary with same name as package
- `src/bin/*.rs` → additional binaries
- `tests/*.rs` → integration tests
- `examples/*.rs` → examples
- `benches/*.rs` → benchmarks

## Module Organization

### Single-File Modules

For simple modules without submodules, use a single file:

```
src/
├── lib.rs
├── lexer.rs      # mod lexer;
├── parser.rs     # mod parser;
└── codegen.rs    # mod codegen;
```

```rust
// lib.rs
mod lexer;
mod parser;
mod codegen;
```

### Modules with Submodules

When a module has submodules, use a directory with `mod.rs`:

```
src/
├── lib.rs
└── parser/
    ├── mod.rs        # Module root
    ├── expr.rs       # Submodule: parser::expr
    └── stmt.rs       # Submodule: parser::stmt
```

```rust
// parser/mod.rs
mod expr;
mod stmt;

pub use expr::parse_expression;
pub use stmt::parse_statement;
```

### Module Declaration

In `lib.rs` or `mod.rs`:

```rust
// Public modules (part of public API)
pub mod ast;
pub mod lexer;
pub mod parser;

// Private modules (implementation details)
mod helpers;
mod utils;
```

### Re-exports

Keep `lib.rs` as a facade that re-exports key types:

```rust
// lib.rs
pub mod ast;
pub mod lexer;
pub mod parser;

// Re-export commonly used items at crate root
pub use ast::Module;
pub use lexer::tokenize;
pub use parser::parse;
```

Users can then import directly:
```rust
use my_crate::{Module, tokenize, parse};
```

## Workspace Structure

For projects with multiple related crates, use a workspace:

```
my_workspace/
├── Cargo.toml          # Workspace root
├── crate_a/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
└── crate_b/
    ├── Cargo.toml
    └── src/
        └── lib.rs
```

Workspace `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crate_a",
    "crate_b",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
serde = "1.0"
```

Member crate `Cargo.toml`:

```toml
[package]
name = "crate_a"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
crate_b = { path = "../crate_b" }
```

## Library + Binary Pattern

Separate library and binary for better testability and reusability:

```toml
# Cargo.toml
[lib]
name = "mylib"
path = "src/lib.rs"

[[bin]]
name = "mycli"
path = "src/main.rs"
```

```rust
// main.rs - thin wrapper that uses the library
use mylib::Config;

fn main() {
    let config = Config::from_args();
    if let Err(e) = mylib::run(config) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
```

```rust
// lib.rs - all the logic
pub struct Config { /* ... */ }

impl Config {
    pub fn from_args() -> Self { /* ... */ }
}

pub fn run(config: Config) -> Result<(), Error> {
    // main logic here
}
```

Benefits:
- Library can be tested independently
- Library can be used by other crates
- Binary is a thin wrapper, easy to maintain

## Feature Flags

Use features for optional functionality:

```toml
[features]
default = []
serde = ["dep:serde"]
parallel = ["dep:rayon"]

[dependencies]
serde = { version = "1.0", optional = true }
rayon = { version = "1.0", optional = true }
```

```rust
#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Config {
    pub name: String,
}
```

## Guidelines

### Do

- Keep binaries thin; put logic in library crate
- Use workspaces for multi-crate projects
- Share dependencies via `[workspace.dependencies]`
- Use feature flags for optional dependencies
- Place integration tests in `tests/` directory
- Re-export commonly used types at crate root

### Don't

- Don't put significant logic in `main.rs`
- Don't create deeply nested module hierarchies
- Don't have circular dependencies between modules
- Don't mix test code with production code (except `#[cfg(test)]`)

## Resources

- [Package Layout - The Cargo Book](https://doc.rust-lang.org/cargo/guide/project-layout.html)
- [Modules - The Rust Book](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html)
- [Cargo Workspaces - The Rust Book](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
- [Features - The Cargo Book](https://doc.rust-lang.org/cargo/reference/features.html)
