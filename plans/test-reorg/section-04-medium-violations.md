---
section: 4
title: Medium Violations
status: completed
goal: Extract 200-500 line inline test modules
sections:
  - id: "4.1"
    title: ori_types Extractions
    status: completed
  - id: "4.2"
    title: oric Extractions
    status: completed
  - id: "4.3"
    title: ori_llvm Remaining Extractions
    status: completed
  - id: "4.4"
    title: Other Crate Extractions
    status: completed
  - id: "4.5"
    title: Completion Checklist
    status: completed
---

# Section 4: Medium Violations (200-500 lines)

**Status:** ✅ Completed
**Goal:** Extract 200-500 line inline test modules

---

## 4.1 ori_types Extractions

### 4.1.1 types/lib.rs (451 lines)

**Target:** `tests/phases/typeck/types.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests
- [x] Verify and delete

### 4.1.2 type_interner.rs (251 lines)

**Target:** `tests/phases/typeck/type_interner.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests
- [x] Verify and delete

---

## 4.2 oric Extractions

### 4.2.1 commands/build.rs (399 lines)

**Target:** `tests/phases/codegen/build_command.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests
- [x] Verify and delete

### 4.2.2 test/error_matching.rs (142 lines)

**Target:** `tests/phases/common/error_matching.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests (6 tests)
- [x] Verify and delete

### 4.2.3 salsa.rs

**Status:** Skipped - no inline tests > 200 lines

---

## 4.3 ori_llvm Remaining Extractions

### 4.3.1 target.rs (395 lines)

**Target:** `tests/phases/codegen/targets.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests
- [x] Verify and delete

### 4.3.2 mangle.rs (362 lines)

**Target:** `tests/phases/codegen/mangling.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests (24 tests, 282 lines)
- [x] Verify and delete

### 4.3.3 runtime.rs (52 lines)

**Target:** `tests/phases/codegen/runtime.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests (5 tests)
- [x] Verify and delete

### 4.3.4 operators.rs (286 lines)

**Status:** Skipped - no inline tests > 200 lines after review

### 4.3.5 wasm.rs

**Target:** `tests/phases/codegen/wasm.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests
- [x] Verify and delete

### 4.3.6 codegen.rs

**Status:** Skipped - no significant inline tests

---

## 4.4 Other Crate Extractions

### 4.4.1 ori_ir/visitor.rs (389 lines)

**Target:** `tests/phases/common/visitor.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests (14 tests)
- [x] Verify and delete

### 4.4.2 ori_diagnostic/queue.rs (326 lines)

**Target:** `tests/phases/common/diagnostics.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests (13 tests)
- [x] Verify and delete

### 4.4.3 ori_rt/lib.rs (209 lines)

**Target:** `tests/phases/codegen/runtime_lib.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests
- [x] Verify and delete

### 4.4.4 ori_parse/lexer (494 lines)

**Target:** `tests/phases/parse/lexer.rs`

- [x] Analyze test content
- [x] Create target file
- [x] Move tests
- [x] Verify and delete

### 4.4.5 ori_patterns extractions

**Target:** `tests/phases/eval/`

- [x] scalar_int.rs → `tests/phases/eval/scalar_int.rs`
- [x] errors.rs → `tests/phases/eval/pattern_errors.rs`

---

## 4.5 Completion Checklist

### ori_types
- [x] `lib.rs` inline tests < 200 lines
- [x] `type_interner.rs` inline tests < 200 lines

### oric
- [x] `commands/build.rs` inline tests < 200 lines

### ori_llvm
- [x] `target.rs` inline tests < 200 lines
- [x] `mangle.rs` inline tests < 200 lines
- [x] `wasm.rs` inline tests < 200 lines

### Other crates
- [x] `ori_ir/visitor.rs` inline tests < 200 lines
- [x] `ori_diagnostic/queue.rs` inline tests < 200 lines
- [x] `ori_rt/lib.rs` inline tests < 200 lines
- [x] `ori_lexer` all violations resolved
- [x] `ori_patterns` all violations resolved

### Verification
- [x] All tests pass in phase tests (637 tests)
- [x] No test regressions in CI

---

## Summary of Extracted Test Files

| Source | Target | Tests |
|--------|--------|-------|
| `ori_types/lib.rs` | `phases/typeck/types.rs` | Type system tests |
| `ori_types/type_interner.rs` | `phases/typeck/type_interner.rs` | 22 interning tests |
| `oric/commands/build.rs` | `phases/codegen/build_command.rs` | 36 build option tests |
| `ori_llvm/aot/target.rs` | `phases/codegen/targets.rs` | Target config tests |
| `ori_llvm/aot/mangle.rs` | `phases/codegen/mangling.rs` | 24 mangling tests |
| `ori_llvm/aot/wasm.rs` | `phases/codegen/wasm.rs` | WASM config tests |
| `ori_ir/visitor.rs` | `phases/common/visitor.rs` | 14 AST visitor tests |
| `ori_diagnostic/queue.rs` | `phases/common/diagnostics.rs` | 13 queue tests |
| `ori_rt/lib.rs` | `phases/codegen/runtime_lib.rs` | Runtime tests |
| `ori_lexer/lib.rs` | `phases/parse/lexer.rs` | Lexer tests |
| `ori_patterns/scalar_int.rs` | `phases/eval/scalar_int.rs` | ScalarInt tests |
| `ori_patterns/errors.rs` | `phases/eval/pattern_errors.rs` | Pattern error tests |
| `oric/test/error_matching.rs` | `phases/common/error_matching.rs` | 6 error matching tests |
| `ori_llvm/aot/runtime.rs` | `phases/codegen/runtime.rs` | 5 runtime config tests |

**Exit Criteria:** ✅ All medium-violation modules extracted; all 637 tests passing; no module > 200 lines of inline tests.
