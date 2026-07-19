# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project uses **Calendar Versioning (CalVer)** — versions track the year-month-day of the build plus an increment, not SemVer API-stability semantics. See `docs/ori_lang/versioning.md` and `docs/development/versioning.md` for the full versioning scheme; the current build number lives in the `BUILD_NUMBER` file at the project root.

## [Unreleased]

## [0.1.0] - 2025-01-20

### Added

- Initial release
- Core language features:
  - Function definitions with `@` prefix
  - Config variables with `$` prefix
  - Strict static type system
  - Pattern-based operations (map, filter, fold, recurse, parallel)
  - Anonymous record types
  - Lambda expressions with type inference
- Compiler infrastructure:
  - Lexer (logos-based)
  - Recursive descent parser
  - Bidirectional type checker
  - Tree-walking interpreter
- Test runner with mandatory coverage
- Parallel test execution
- Rosetta stress-test corpus (initial 18 problems; expanded to 600+ in subsequent builds)

### Note on codegen backend

Earlier iterations of this project used a C code generator. LLVM remains the
shipped compiled projection, but it is not the definition or destination of
AIMS. The production architecture runs backend-neutral ARC/AIMS realization
once and binds the same logical ownership plan to sibling VM, LLVM/native,
compiled-WebAssembly, and JIT projections as each reaches its promotion gates;
the evaluator remains the canonical behavioral oracle. Moving the current
LLVM-owned invocation fully upstream is a tracked seam migration, not an
exception to that authority. See `.claude/rules/canon.md §1` and the Project
Structure section of `CONTRIBUTING.md`.
