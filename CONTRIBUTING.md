# Contributing to Ori

Thank you for your interest in contributing to Ori!

## Platform Requirements

**Windows is not supported for development.** Use WSL2, Linux, or macOS.

## Getting Started

```bash
git clone https://github.com/yourusername/ori
cd ori
./setup.sh
```

This installs git hooks (for commit message linting) and verifies your environment.

Then:

1. Create a branch: `git checkout -b my-feature`
2. Make your changes
3. Run tests: `./test-all.sh`
4. Commit using conventional format (see below)
5. Push: `git push origin my-feature`
6. Open a Pull Request

## Development Commands

```bash
./test-all.sh      # Run all tests (Rust + Ori + LLVM)
./build-all.sh     # Build everything
./clippy-all.sh    # Run lints
./fmt-all.sh       # Format code

cargo t         # Run Rust tests only
cargo st        # Run Ori spec tests only
```

## Project Structure

- `compiler/oric/` - The compiler implementation
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
├── my_feature.ori          # Implementation
└── _test/
    └── my_feature.test.ori # Tests
```

### Compile-fail tests

Add a `.ori` file to `tests/compile-fail/` with a comment describing the expected error:

```ori
// Compile-fail test: description
// Expected error: the error message

@bad_code () -> int = "not an int"
```

## Code Style

- Follow existing patterns in the codebase
- Run `cargo fmt` before committing
- Ensure `cargo clippy` passes

## Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/). The git hooks enforce this.

```
feat: add new feature
fix: resolve bug
docs: update documentation
style: formatting changes
refactor: restructure code
perf: performance improvement
test: add or update tests
build: build system changes
ci: CI configuration
chore: maintenance tasks
```

With optional scope: `fix(parser): handle empty input`

- Use present tense: "add feature" not "added feature"
- Keep the first line under 72 characters
- Reference issues when relevant: `fix: resolve crash on empty input (#123)`

## Pull Request Process

1. Ensure all tests pass
2. Update documentation if needed
3. Add tests for new features
4. Request review from maintainers

## Questions?

Open an issue for discussion.
