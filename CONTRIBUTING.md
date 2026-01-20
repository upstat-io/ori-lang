# Contributing to Sigil

Thank you for your interest in contributing to Sigil!

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/yourusername/sigil`
3. Create a branch: `git checkout -b my-feature`
4. Make your changes
5. Run tests: `cargo test && cargo run --bin sigil -- test`
6. Commit: `git commit -m "Add my feature"`
7. Push: `git push origin my-feature`
8. Open a Pull Request

## Development Setup

```bash
# Build the compiler
cargo build

# Run the compiler tests (Rust)
cargo test

# Run the language tests (Sigil)
cargo run --bin sigil -- test

# Build release version
cargo build --release
```

## Project Structure

- `compiler/sigilc/` - The compiler implementation
  - `src/lexer/` - Tokenizer (uses logos)
  - `src/parser/` - Recursive descent parser
  - `src/ast/` - Abstract syntax tree definitions
  - `src/types/` - Type checker
  - `src/eval/` - Tree-walking interpreter
  - `src/codegen/` - C code generator
- `tests/run-pass/` - Programs that should compile and run
- `tests/compile-fail/` - Programs that should fail to compile
- `docs/` - Documentation
- `examples/` - Example programs

## Adding a Test

### Run-pass tests

Add your test to `tests/run-pass/`. Follow the existing structure:

```
tests/run-pass/my_feature/
├── my_feature.si          # Implementation
└── _test/
    └── my_feature.test.si # Tests
```

### Compile-fail tests

Add a `.si` file to `tests/compile-fail/` with a comment describing the expected error:

```sigil
// Compile-fail test: description
// Expected error: the error message

@bad_code () -> int = "not an int"
```

## Code Style

- Follow existing patterns in the codebase
- Run `cargo fmt` before committing
- Ensure `cargo clippy` passes

## Commit Messages

- Use present tense: "Add feature" not "Added feature"
- Keep the first line under 72 characters
- Reference issues when relevant: "Fix #123"

## Pull Request Process

1. Ensure all tests pass
2. Update documentation if needed
3. Add tests for new features
4. Request review from maintainers

## Questions?

Open an issue for discussion.
