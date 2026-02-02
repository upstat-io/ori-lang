---
section: 1
title: Infrastructure
status: not-started
goal: Create tests/phases/ directory structure and shared test utilities
sections:
  - id: "1.1"
    title: Directory Structure
    status: not-started
  - id: "1.2"
    title: Common Test Utilities
    status: not-started
  - id: "1.3"
    title: Cargo Configuration
    status: not-started
  - id: "1.4"
    title: Completion Checklist
    status: not-started
---

# Section 1: Infrastructure

**Status:** 📋 Planned
**Goal:** Create `tests/phases/` directory structure and shared test utilities

---

## 1.1 Directory Structure

- [ ] Create top-level directory structure:
  ```bash
  mkdir -p tests/phases/{parse,typeck,eval,codegen,common}
  ```

- [ ] Create `mod.rs` files for each phase:
  - [ ] `tests/phases/parse/mod.rs`
  - [ ] `tests/phases/typeck/mod.rs`
  - [ ] `tests/phases/eval/mod.rs`
  - [ ] `tests/phases/codegen/mod.rs`
  - [ ] `tests/phases/common/mod.rs`

- [ ] Verify directory structure matches target:
  ```
  tests/phases/
  ├── common/
  │   ├── mod.rs
  │   ├── parse.rs
  │   ├── typecheck.rs
  │   ├── eval.rs
  │   └── codegen.rs
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

- [ ] Create `tests/phases/common/mod.rs`:
  ```rust
  //! Shared test utilities for phase tests.

  mod parse;
  mod typecheck;
  mod eval;
  mod codegen;

  pub use parse::*;
  pub use typecheck::*;
  pub use eval::*;
  pub use codegen::*;
  ```

- [ ] Create `tests/phases/common/parse.rs`:
  - [ ] `parse(source: &str)` — parse source, return result
  - [ ] `parse_ok(source: &str)` — parse and assert success
  - [ ] `parse_err(source: &str, expected: &str)` — parse and assert specific error

- [ ] Create `tests/phases/common/typecheck.rs`:
  - [ ] `typecheck(source: &str)` — type check source, return result
  - [ ] `typecheck_ok(source: &str)` — type check and assert success
  - [ ] `typecheck_err(source: &str, expected: &str)` — type check and assert error
  - [ ] `assert_return_type(source: &str, func: &str, expected: &str)` — validate function return type

- [ ] Create `tests/phases/common/eval.rs`:
  - [ ] `eval(source: &str)` — evaluate source, return value
  - [ ] `eval_ok(source: &str)` — evaluate and assert success
  - [ ] `eval_eq(source: &str, expected: Value)` — evaluate and assert value

- [ ] Create `tests/phases/common/codegen.rs`:
  - [ ] `compile_to_ir(source: &str)` — compile to LLVM IR string
  - [ ] `compile_and_run(source: &str)` — compile, link, and execute
  - [ ] `assert_ir_contains(source: &str, pattern: &str)` — compile and check IR

---

## 1.3 Cargo Configuration

- [ ] Add test configuration to root `Cargo.toml` or create `tests/phases/Cargo.toml`

- [ ] Configure feature flags for phase tests:
  - [ ] `llvm` feature for codegen tests
  - [ ] Conditional compilation for optional phases

- [ ] Verify all crates are accessible from test harness:
  - [ ] ori_lexer
  - [ ] ori_parse
  - [ ] ori_types
  - [ ] ori_typeck
  - [ ] ori_eval
  - [ ] ori_patterns
  - [ ] ori_llvm (feature-gated)
  - [ ] ori_diagnostic
  - [ ] ori_ir

---

## 1.4 Completion Checklist

- [ ] Directory structure created
- [ ] All `mod.rs` files in place
- [ ] Common utilities implemented and tested
- [ ] Cargo configuration updated
- [ ] `cargo test --test phases` runs without errors
- [ ] Documentation added to README or CLAUDE.md

**Exit Criteria:** `tests/phases/` exists with working test utilities; `cargo test --test phases` succeeds (even with no tests yet).
