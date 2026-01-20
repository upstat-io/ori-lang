# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
  - C code generator
- Test runner with mandatory coverage
- Parallel test execution
- 18 Rosetta Code implementations
