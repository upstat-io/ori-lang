---
section: 4
title: Medium Violations
status: not-started
goal: Extract 200-500 line inline test modules
sections:
  - id: "4.1"
    title: ori_types Extractions
    status: not-started
  - id: "4.2"
    title: oric Extractions
    status: not-started
  - id: "4.3"
    title: ori_llvm Remaining Extractions
    status: not-started
  - id: "4.4"
    title: Other Crate Extractions
    status: not-started
  - id: "4.5"
    title: Completion Checklist
    status: not-started
---

# Section 4: Medium Violations (200-500 lines)

**Status:** 📋 Planned
**Goal:** Extract 200-500 line inline test modules

---

## 4.1 ori_types Extractions

### 4.1.1 types/lib.rs (451 lines)

**Target:** `tests/phases/typeck/types.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.1.2 context.rs (251 lines)

**Target:** `tests/phases/typeck/type_context.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

---

## 4.2 oric Extractions

### 4.2.1 commands/build.rs (399 lines)

**Target:** `tests/phases/codegen/build_command.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.2.2 test/error_matching.rs (268 lines)

**Target:** `tests/phases/typeck/error_matching.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.2.3 salsa.rs

**Target:** `tests/phases/common/salsa.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

---

## 4.3 ori_llvm Remaining Extractions

### 4.3.1 target.rs (395 lines)

**Target:** `tests/phases/codegen/targets.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.3.2 mangle.rs (362 lines)

**Target:** `tests/phases/codegen/mangling.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.3.3 runtime.rs (309 lines)

**Target:** `tests/phases/codegen/runtime.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.3.4 operators.rs (286 lines)

**Target:** `tests/phases/codegen/operators.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.3.5 wasm.rs

**Target:** `tests/phases/codegen/wasm.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.3.6 codegen.rs

**Target:** `tests/phases/codegen/ir_generation.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

---

## 4.4 Other Crate Extractions

### 4.4.1 ori_ir/builtin_methods.rs (327 lines)

**Target:** `tests/phases/eval/builtin_methods.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.4.2 ori_diagnostic/queue.rs (261 lines)

**Target:** `tests/phases/common/diagnostics.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.4.3 ori_rt/lib.rs (209 lines)

**Target:** `tests/phases/codegen/runtime_lib.rs`

- [ ] Analyze test content
- [ ] Create target file
- [ ] Move tests
- [ ] Verify and delete

### 4.4.4 ori_parse violations

**Target:** `tests/phases/parse/`

- [ ] Identify violating files
- [ ] Create target files
- [ ] Move tests
- [ ] Verify and delete

### 4.4.5 ori_eval violations

**Target:** `tests/phases/eval/`

- [ ] Identify violating files
- [ ] Create target files
- [ ] Move tests
- [ ] Verify and delete

---

## 4.5 Completion Checklist

### ori_types
- [ ] `lib.rs` inline tests < 200 lines
- [ ] `context.rs` inline tests < 200 lines

### oric
- [ ] `commands/build.rs` inline tests < 200 lines
- [ ] `test/error_matching.rs` inline tests < 200 lines
- [ ] `salsa.rs` inline tests < 200 lines

### ori_llvm (remaining)
- [ ] `target.rs` inline tests < 200 lines
- [ ] `mangle.rs` inline tests < 200 lines
- [ ] `runtime.rs` inline tests < 200 lines
- [ ] `operators.rs` inline tests < 200 lines
- [ ] `wasm.rs` inline tests < 200 lines
- [ ] `codegen.rs` inline tests < 200 lines

### Other crates
- [ ] `ori_ir/builtin_methods.rs` inline tests < 200 lines
- [ ] `ori_diagnostic/queue.rs` inline tests < 200 lines
- [ ] `ori_rt/lib.rs` inline tests < 200 lines
- [ ] `ori_parse` all violations resolved
- [ ] `ori_eval` all violations resolved

### Verification
- [ ] All tests pass in phase tests
- [ ] No test regressions in CI

**Exit Criteria:** All 18 medium-violation modules extracted; all tests passing; no module > 200 lines of inline tests.
