# Contributing to Ori

Thank you for your interest in contributing to Ori!

## Platform Requirements

**Windows is not supported for development.** Use WSL2, Linux, or macOS.

## Getting Started

```bash
git clone https://github.com/yourusername/ori-lang
cd ori-lang
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

The Ori compiler is a strictly-ordered, multi-crate pipeline: lex → parse → typecheck → canonicalize → ARC lowering → AIMS analysis → ARC realization → LLVM codegen. A parallel evaluator consumes canonical IR for const-eval and `ori run`. See `.claude/rules/canon.md §1 Pipeline Overview` and `.claude/rules/compiler.md §Architecture` for the authoritative pipeline diagram and per-crate dependencies.

- `compiler/oric/` — compiler driver (CLI, Salsa orchestration, filesystem IO, LLVM integration); `src/lib.rs` + `src/main.rs` register the `ori` binary.
- `compiler/ori_lexer_core/`, `compiler/ori_lexer/` — raw + cooked lexing.
- `compiler/ori_parse/` — recursive-descent parser producing `ParseOutput`.
- `compiler/ori_types/` — Hindley-Milner inference + trait/capability checking.
- `compiler/ori_canon/` — canonicalization (AST → `CanExpr`) + Maranget pattern compilation.
- `compiler/ori_arc/` — ARC lowering + AIMS lattice analysis + ARC realization.
- `compiler/ori_repr/` — representation layer (`ReprPlan`).
- `compiler/ori_llvm/` — LLVM IR emission from realized ARC IR.
- `compiler/ori_eval/` — parallel evaluator for canonical IR (const-eval + `ori run`).
- `compiler/ori_patterns/` — runtime value model + function-pattern dispatch.
- `compiler/ori_ir/` — interned, flat, Salsa-compatible IR foundation.
- `compiler/ori_rt/` — AOT runtime (C-ABI static library).
- `compiler/ori_diagnostic/`, `compiler/ori_registry/`, `compiler/ori_stack/`, `compiler/ori_fmt/`, `compiler/ori_test_harness/`, `compiler/ori_compiler/` — supporting crates (see `.claude/rules/compiler.md §Crates`).
- `library/std/` — standard library (pure Ori).
- `tests/run-pass/` — programs that should compile and run (includes the Rosetta stress-test corpus under `tests/run-pass/rosetta/`).
- `tests/compile-fail/` — programs that should fail to compile.
- `tests/spec/` — spec conformance suite.
- `docs/ori_lang/v2026/spec/` — authoritative language spec (proposal-gated).
- `docs/compiler/design/` — design-doc rationale.
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
