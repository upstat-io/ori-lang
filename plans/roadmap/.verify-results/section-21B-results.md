# Section 21B: AOT Compilation -- Verification Results

**Date**: 2026-03-19
**Status**: 0/536 (0%) -- roadmap says not started
**Verdict**: INACCURATE -- significant implementation exists, 0% understates progress

## Methodology

Spot-checked 8 items across subsections. Ran existing AOT tests. Inspected codebase for implementation evidence.

## Items Verified

### 21B.2 Object File Emission

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| Symbol mangling in `ori_llvm/src/aot/mangle.rs` | Implemented | STALE TEST | 9 demangle tests exist in `oric/src/commands/demangle/tests.rs` and pass. The `demangle/` directory has `mod.rs` and `tests.rs`. Roadmap marks this as unchecked. |

### 21B.5 Linking

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| Platform linker driver | Implemented | STALE TEST | 42 linker tests pass (`cargo test -p ori_llvm -- linker`), including `gcc_linker_*`, `msvc_linker_*`, `wasm_linker_*` variants. Roadmap says "68 tests, 81% coverage" but marks the item unchecked. |
| Runtime library (ori_rt) | Implemented | STALE TEST | 329 ori_rt tests pass. Runtime includes list, map, set, string, RC, COW, iterator, and panic operations. Roadmap mentions "19 tests" but 329 actually pass. |

### 21B.6 Incremental Compilation

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| Source hashing | Implemented | VERIFIED (code exists, not wired up) | Files exist at `ori_llvm/src/aot/incremental/hash/`. Tests exist. But `test_build_incremental_unchanged` is `#[ignore]` with message "Incremental compilation not yet wired up in ori build". |
| Dependency tracking | Implemented | VERIFIED (code exists, not wired up) | Files exist at `ori_llvm/src/aot/incremental/deps/`. |

### 21B.8 CLI Integration

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| `ori build` command | Implemented | STALE TEST | 24 CLI tests pass (`cli::test_build_basic`, `test_build_release`, `test_build_output_path`, `test_build_emit_llvm_ir`, `test_build_emit_assembly`, `test_build_emit_object`, `test_build_cross_compile_wasm_object`, `test_build_verbose`, etc.) + 1 ignored (incremental). Roadmap marks all unchecked. |
| `ori targets` command | Implemented | STALE TEST | 26 targets-related tests pass (`cargo test -p oric -- targets`). 8 tests in `oric/src/commands/targets/tests.rs`. |
| `ori demangle` command | Implemented | STALE TEST | 9 tests in `oric/src/commands/demangle/tests.rs`. |

### 21B.3 Debug Information

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| Debug info config | Partially implemented | VERIFIED | `test_debug_info_config_presets` passes (1 of 51 codegen tests). Some debug infrastructure exists. |

## Summary

The 0% status is significantly inaccurate. There is substantial implementation:

| Subsystem | Evidence |
|-----------|----------|
| Object emission | Mangling/demangling implemented and tested |
| Linking | 42 linker tests pass across gcc/msvc/wasm drivers |
| Runtime library | 329 ori_rt tests pass (roadmap says 19) |
| CLI integration | 24 `ori build` tests pass, `ori targets` and `ori demangle` work |
| Incremental compilation | Infrastructure code exists but not wired to `ori build` |
| Debug info | Partial implementation with at least 1 test |

**Recommendations:**
1. Re-audit all checkboxes against current test suite -- many items have been implemented without the roadmap being updated
2. The 0% figure should be revised significantly upward; a conservative estimate from this spot-check suggests at least 15-25% of items are actually done
3. The ori_rt test count in the roadmap (19 tests) is extremely stale -- actual count is 329
