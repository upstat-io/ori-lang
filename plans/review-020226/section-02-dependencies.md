---
section: "02"
title: Dependency Cleanup
status: completed
priority: critical
goal: Remove all unused dependencies from Cargo.toml files
files:
  - compiler/oric/Cargo.toml
  - compiler/ori_eval/Cargo.toml
  - compiler/ori_llvm/Cargo.toml
  - compiler/ori_patterns/Cargo.toml
  - website/playground-wasm/Cargo.toml
  - tools/ori-lsp/Cargo.toml
---

# Section 02: Dependency Cleanup

**Status:** ✅ Completed
**Priority:** CRITICAL — Dead dependencies increase build time and maintenance burden
**Goal:** Remove 12 unused dependencies across 6 Cargo.toml files

---

## 02.1 Compiler Crate Cleanup

### oric/Cargo.toml

- [x] Remove `logos` — Used by ori_lexer, not oric directly (transitive)
- [x] Remove `ori_stack` — Used by ori_parse/ori_typeck/ori_eval, not oric directly (transitive)
- [x] Remove `ori_macros` — Currently unused; will be properly integrated in Section 03

### ori_eval/Cargo.toml

- [x] Remove `ori_diagnostic` — Not used in ori_eval source
- [x] Remove `ori_types` — Not used in ori_eval source
- [x] Remove `rayon` — Parallel execution uses async, not thread pool

### ori_llvm/Cargo.toml

- [x] Remove `parking_lot` — Concurrent compilation not implemented

### ori_patterns/Cargo.toml

- [x] Remove `rustc-hash` — FxHashMap not used in patterns

---

## 02.2 Tool/Website Cleanup

### website/playground-wasm/Cargo.toml

- [x] Remove `ori_patterns` — Playground uses interpreter, doesn't expose pattern API
- [x] Remove `ori_types` — Types come through ori_typeck transitively

### tools/ori-lsp/Cargo.toml

- [x] Remove `serde` — Not used in current LSP implementation
- [x] Remove `serde_json` — Not used in current LSP implementation

---

## 02.3 Final Verification

- [x] Run `cargo machete` — reports no unused dependencies
- [x] Run `cargo check --workspace` — compiles successfully
- [x] Run `./test-all` — passes (6,367 tests, 0 failures)

---

## 02.N Completion Checklist

- [x] 12 unused dependencies removed
- [x] `cargo machete` reports clean
- [x] `cargo check --workspace` passes
- [x] `./test-all` passes
- [x] Build times potentially improved

**Exit Criteria:** ✅ `cargo machete` reports no unused dependencies in compiler/
