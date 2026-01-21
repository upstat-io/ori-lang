# Rust Best Practices

Comprehensive guidelines for Rust development in the Sigil compiler project.

## Quick Navigation

| Document | Description |
|----------|-------------|
| [01-project-structure](01-project-structure.md) | File layout, modules, workspaces |
| [02-naming-conventions](02-naming-conventions.md) | Naming rules and patterns |
| [03-testing](03-testing.md) | Unit, integration, doc tests |
| [04-error-handling](04-error-handling.md) | Result, Option, thiserror, anyhow |
| [05-documentation](05-documentation.md) | Rustdoc, comments, API docs |
| [06-linting-formatting](06-linting-formatting.md) | Clippy, rustfmt configuration |
| [07-api-design](07-api-design.md) | API guidelines checklist |
| [08-ownership-borrowing](08-ownership-borrowing.md) | Memory patterns and idioms |
| [09-performance](09-performance.md) | Optimization techniques |
| [10-concurrency](10-concurrency.md) | Async, Tokio, Rayon patterns |
| [11-cargo-dependencies](11-cargo-dependencies.md) | Workspace, features, versioning |

## Quick Reference Checklist

### Before Committing Code

- [ ] `cargo fmt` - Code is formatted
- [ ] `cargo clippy` - No warnings
- [ ] `cargo test` - All tests pass
- [ ] `cargo doc` - Documentation builds

### New File Checklist

- [ ] File name is `snake_case.rs`
- [ ] Module declared in parent `mod.rs` or `lib.rs`
- [ ] Public items have `///` documentation
- [ ] Unit tests in `#[cfg(test)]` module or separate file

### New Type Checklist

- [ ] Name is `CamelCase`
- [ ] Derives appropriate traits (`Debug`, `Clone`, etc.)
- [ ] Has `///` documentation with examples
- [ ] Consider implementing `Default` if sensible

### New Function Checklist

- [ ] Name is `snake_case`
- [ ] Has `///` documentation for public functions
- [ ] Returns `Result<T, E>` for fallible operations
- [ ] Uses `&self` over `self` when ownership not needed

## Essential Links

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/)
- [Clippy Lints](https://rust-lang.github.io/rust-clippy/master/)
- [The Cargo Book](https://doc.rust-lang.org/cargo/)

## Sigil Project Structure

```
sigil/
├── compiler/
│   └── sigilc/            # Main compiler crate
│       └── src/
│           ├── lib.rs     # Library interface
│           ├── main.rs    # CLI entry point
│           ├── lexer/     # Tokenizer
│           ├── parser/    # AST generation
│           ├── ast/       # AST definitions
│           ├── types/     # Type checker
│           ├── eval/      # Interpreter
│           ├── errors/    # Diagnostic system
│           └── codegen/   # C code generator
├── library/               # Standard library
├── docs/                  # Documentation
├── examples/              # Example programs
└── tests/                 # Integration tests
```
