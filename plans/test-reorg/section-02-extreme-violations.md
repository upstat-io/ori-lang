---
section: 2
title: Extreme Violations
status: completed
goal: Extract the two 1000+ line inline test modules
sections:
  - id: "2.1"
    title: Extract debug.rs Tests
    status: completed
  - id: "2.2"
    title: Extract linker/mod.rs Tests
    status: completed
  - id: "2.3"
    title: Completion Checklist
    status: completed
---

# Section 2: Extreme Violations (1000+ lines)

**Status:** ✅ Completed
**Goal:** Extract the two 1000+ line inline test modules

---

## 2.1 Extract debug.rs Tests

**Source:** `ori_llvm/src/aot/debug.rs` (was 2,350 lines, now 1,266 lines)

- [x] Analyze test categories in debug.rs:
  - [x] Debug configuration tests (DebugLevel, DebugInfoConfig, DebugFormat, DebugInfoError)
  - [x] Debug builder tests (creation, basic types, functions, scopes)
  - [x] Composite type tests (struct, enum, pointer, array, etc.)
  - [x] Debug context and line map tests

- [x] Create target files (organized by concern):
  - [x] `tests/phases/codegen/debug_config.rs` (23 tests)
  - [x] `tests/phases/codegen/debug_builder.rs` (12 tests)
  - [x] `tests/phases/codegen/debug_types.rs` (18 tests)
  - [x] `tests/phases/codegen/debug_context.rs` (13 tests)

- [x] Move tests by category:
  - [x] Identified tests using public vs private methods
  - [x] Moved 66 tests to phase test files
  - [x] Added imports via `ori_llvm::inkwell` and `ori_llvm::aot::*`
  - [x] Updated `codegen/mod.rs` to include new modules

- [x] Kept inline unit test for private method (17 lines):
  - [x] `test_debug_level_emission_kind` - tests private `to_emission_kind()`

- [x] Verified all tests pass:
  ```bash
  cargo test -p oric --test phases --features oric/llvm debug  # 66 tests pass
  cargo test -p ori_llvm debug  # 1 inline test passes
  ```

- [x] Deleted extracted tests from source file

- [x] Source file now has 17 lines of test code (< 200 line limit)

---

## 2.2 Extract linker/mod.rs Tests

**Source:** `ori_llvm/src/aot/linker/mod.rs` (was 1,745 lines, now 669 lines)

- [x] Analyze test categories in linker/mod.rs:
  - [x] Core linker infrastructure (flavor, output, library, error, driver)
  - [x] GCC linker driver tests
  - [x] MSVC linker driver tests
  - [x] WASM linker tests

- [x] Create target files (organized by linker type):
  - [x] `tests/phases/codegen/linker_core.rs` (21 tests)
  - [x] `tests/phases/codegen/linker_gcc.rs` (19 tests)
  - [x] `tests/phases/codegen/linker_msvc.rs` (10 tests)
  - [x] `tests/phases/codegen/linker_wasm.rs` (10 tests)

- [x] Move tests by category:
  - [x] All linker methods used in tests are public
  - [x] Moved all 60 tests to phase test files
  - [x] Added imports via `ori_llvm::aot::linker::*`
  - [x] Updated `codegen/mod.rs` to include new modules

- [x] No inline tests needed (all methods used are public)

- [x] Verified all tests pass:
  ```bash
  cargo test -p oric --test phases --features oric/llvm linker  # 60 tests pass
  cargo test -p ori_llvm linker  # 0 inline tests (none needed)
  ```

- [x] Deleted all tests from source file

- [x] Source file now has 0 lines of test code (< 200 line limit)

---

## 2.3 Completion Checklist

- [x] `debug.rs` inline tests: 17 lines (< 200 line limit)
- [x] `linker/mod.rs` inline tests: 0 lines (< 200 line limit)
- [x] 66 debug tests pass in phase tests
- [x] 60 linker tests pass in phase tests
- [x] `codegen/mod.rs` updated with 8 new modules
- [x] `phases.rs` updated to enable codegen module

**Exit Criteria:** ✅ Met — Both extreme violation modules extracted; 138 phase tests passing
(including 8 existing common tests); source files well under 200 line limit.

---

## Test Files Created

| File | Tests | Purpose |
|------|-------|---------|
| `debug_config.rs` | 23 | DebugLevel, DebugInfoConfig, DebugFormat, DebugInfoError |
| `debug_builder.rs` | 12 | DebugInfoBuilder creation, basic types, functions |
| `debug_types.rs` | 18 | Composite types (struct, enum, array, Option, Result) |
| `debug_context.rs` | 13 | DebugContext, LineMap, offset-to-location |
| `linker_core.rs` | 21 | LinkerFlavor, LinkOutput, LinkLibrary, LinkerDriver |
| `linker_gcc.rs` | 19 | GccLinker (Unix linker driver) |
| `linker_msvc.rs` | 10 | MsvcLinker (Windows linker driver) |
| `linker_wasm.rs` | 10 | WasmLinker (WebAssembly linker driver) |

---

## Running Phase Tests

```bash
# Run all codegen phase tests
cargo test -p oric --test phases --features oric/llvm codegen

# Run specific test file
cargo test -p oric --test phases --features oric/llvm debug_config

# Run tests with filter
cargo test -p oric --test phases --features oric/llvm gcc_linker
```
