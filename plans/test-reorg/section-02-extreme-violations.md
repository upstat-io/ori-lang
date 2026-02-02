---
section: 2
title: Extreme Violations
status: not-started
goal: Extract the two 1000+ line inline test modules
sections:
  - id: "2.1"
    title: Extract debug.rs Tests
    status: not-started
  - id: "2.2"
    title: Extract linker/mod.rs Tests
    status: not-started
  - id: "2.3"
    title: Completion Checklist
    status: not-started
---

# Section 2: Extreme Violations (1000+ lines)

**Status:** 📋 Planned
**Goal:** Extract the two 1000+ line inline test modules

---

## 2.1 Extract debug.rs Tests

**Source:** `ori_llvm/src/aot/debug.rs` (1,099 lines, 66 tests)

- [ ] Analyze test categories in debug.rs:
  - [ ] Basic type debug info tests
  - [ ] Composite type debug info tests
  - [ ] Debug configuration tests
  - [ ] Debug level tests

- [ ] Create target files:
  - [ ] `tests/phases/codegen/debug_basic_types.rs`
  - [ ] `tests/phases/codegen/debug_composite_types.rs`
  - [ ] `tests/phases/codegen/debug_config.rs`
  - [ ] `tests/phases/codegen/debug_levels.rs`

- [ ] Move tests by category:
  - [ ] Identify which tests belong in each file
  - [ ] Copy tests to target files
  - [ ] Add necessary imports to target files
  - [ ] Update `codegen/mod.rs` to include new modules

- [ ] Keep inline unit tests for `DebugInfoBuilder` methods (< 200 lines)

- [ ] Verify all tests pass:
  ```bash
  cargo test --test phases debug
  cargo test -p ori_llvm debug
  ```

- [ ] Delete extracted tests from source file

- [ ] Verify source file is now < 200 lines of test code

---

## 2.2 Extract linker/mod.rs Tests

**Source:** `ori_llvm/src/aot/linker/mod.rs` (1,071 lines, 73 tests)

- [ ] Analyze test categories in linker/mod.rs:
  - [ ] GCC linker driver tests
  - [ ] MSVC linker driver tests
  - [ ] WASM linker tests
  - [ ] Linker discovery tests

- [ ] Create target files:
  - [ ] `tests/phases/codegen/linker_gcc.rs`
  - [ ] `tests/phases/codegen/linker_msvc.rs`
  - [ ] `tests/phases/codegen/linker_wasm.rs`
  - [ ] `tests/phases/codegen/linker_discovery.rs`

- [ ] Move tests by category:
  - [ ] Identify which tests belong in each file
  - [ ] Copy tests to target files
  - [ ] Add necessary imports to target files
  - [ ] Update `codegen/mod.rs` to include new modules

- [ ] Keep inline unit tests for `LinkerDriver` trait methods (< 200 lines)

- [ ] Verify all tests pass:
  ```bash
  cargo test --test phases linker
  cargo test -p ori_llvm linker
  ```

- [ ] Delete extracted tests from source file

- [ ] Verify source file is now < 200 lines of test code

---

## 2.3 Completion Checklist

- [ ] `debug.rs` inline tests < 200 lines
- [ ] `linker/mod.rs` inline tests < 200 lines
- [ ] All 66 debug tests pass in phase tests
- [ ] All 73 linker tests pass in phase tests
- [ ] No test regressions in CI
- [ ] `codegen/mod.rs` updated with new modules

**Exit Criteria:** Both extreme violation modules extracted; all tests passing; no module > 200 lines of inline tests.
