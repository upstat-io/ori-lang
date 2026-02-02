---
section: 1
title: Infrastructure
status: completed
goal: Create compiler/oric/tests/phases/ directory structure and shared test utilities
sections:
  - id: "1.1"
    title: Directory Structure
    status: completed
  - id: "1.2"
    title: Common Test Utilities
    status: completed
  - id: "1.3"
    title: Cargo Configuration
    status: completed
  - id: "1.4"
    title: Completion Checklist
    status: completed
---

# Section 1: Infrastructure

**Status:** ✅ Completed
**Goal:** Create `compiler/oric/tests/phases/` directory structure and shared test utilities

> **Note:** Tests are located in `compiler/oric/tests/phases/` (not `tests/phases/`) because
> integration tests in Rust need access to crate dependencies. The `oric` crate already
> depends on all compiler crates, making it the ideal host for cross-crate phase tests.

---

## 1.1 Directory Structure

- [x] Create directory structure in oric crate:
  ```bash
  mkdir -p compiler/oric/tests/phases/{parse,typeck,eval,codegen,common}
  ```

- [x] Create `mod.rs` files for each phase:
  - [x] `compiler/oric/tests/phases/parse/mod.rs`
  - [x] `compiler/oric/tests/phases/typeck/mod.rs`
  - [x] `compiler/oric/tests/phases/eval/mod.rs`
  - [x] `compiler/oric/tests/phases/codegen/mod.rs`
  - [x] `compiler/oric/tests/phases/common/mod.rs`

- [x] Verify directory structure matches target:
  ```
  compiler/oric/tests/
  ├── phases.rs                    # Test entry point
  └── phases/
      ├── common/
      │   ├── mod.rs
      │   ├── parse.rs
      │   └── typecheck.rs
      ├── parse/
      │   └── mod.rs
      ├── typeck/
      │   └── mod.rs
      ├── eval/
      │   └── mod.rs
      └── codegen/
          └── mod.rs
  ```

---

## 1.2 Common Test Utilities

- [x] Create `tests/phases/common/mod.rs`:
  ```rust
  //! Shared test utilities for phase tests.
  #![allow(unused)]  // Until tests are migrated

  mod parse;
  mod typecheck;

  pub use parse::*;
  pub use typecheck::*;
  ```

- [x] Create `tests/phases/common/parse.rs`:
  - [x] `parse_source(source: &str)` — parse source, return result
  - [x] `parse_ok(source: &str)` — parse and assert success
  - [x] `parse_err(source: &str, expected: &str)` — parse and assert specific error
  - [x] `test_interner()` — create a string interner for tests

- [x] Create `tests/phases/common/typecheck.rs`:
  - [x] `typecheck_source(source: &str)` — type check source, return result
  - [x] `typecheck_ok(source: &str)` — type check and assert success
  - [x] `typecheck_err(source: &str, expected: &str)` — type check and assert error

- [ ] Create `tests/phases/common/eval.rs` (deferred):
  - [ ] `eval(source: &str)` — evaluate source, return value
  - [ ] `eval_ok(source: &str)` — evaluate and assert success
  - [ ] `eval_eq(source: &str, expected: Value)` — evaluate and assert value

- [ ] Create `tests/phases/common/codegen.rs` (deferred, requires llvm feature):
  - [ ] `compile_to_ir(source: &str)` — compile to LLVM IR string
  - [ ] `compile_and_run(source: &str)` — compile, link, and execute
  - [ ] `assert_ir_contains(source: &str, pattern: &str)` — compile and check IR

---

## 1.3 Cargo Configuration

- [x] Tests located in oric crate (no Cargo.toml changes needed)
- [x] Crate dependencies already available:
  - [x] ori_lexer ✓
  - [x] ori_parse ✓
  - [x] ori_types ✓
  - [x] ori_typeck ✓
  - [x] ori_eval ✓
  - [x] ori_patterns ✓
  - [x] ori_llvm (feature-gated) ✓
  - [x] ori_diagnostic ✓
  - [x] ori_ir ✓

---

## 1.4 Completion Checklist

- [x] Directory structure created
- [x] All `mod.rs` files in place
- [x] Common utilities implemented and tested (8 tests passing)
- [x] No Cargo configuration changes needed (uses oric crate)
- [x] `cargo test -p oric --test phases` runs without errors

**Exit Criteria:** ✅ Met — `compiler/oric/tests/phases/` exists with working test utilities;
`cargo test -p oric --test phases` succeeds with 8 passing tests.

---

## Running Phase Tests

```bash
# Run all phase tests
cargo test -p oric --test phases

# Run specific phase
cargo test -p oric --test phases parse

# Run with LLVM codegen tests (requires llvm feature)
cargo test -p oric --test phases --features llvm
```
