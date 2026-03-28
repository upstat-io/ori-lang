# Section 21B: AOT Compilation -- Verification Results

**Verified**: 2026-03-28
**Methodology**: Ran all ori_llvm tests (lib: 466, AOT integration: 1973, 17 ignored), ori_rt tests (360 pass), WASM/linking/LTO/CLI/cross/debug/incremental/multi_file/syslib unit tests. Audited test files against roadmap items. Verified against test-all.sh (all green, 0 failures).
**Sections verified**: 21B.1-21B.16
**Total items**: 138

## Current Test Results

| Test Suite | Passed | Failed | Ignored | Notes |
|------------|--------|--------|---------|-------|
| ori_llvm lib (aot modules) | 466 | 0 | 0 | Includes linker, debug, incremental, multi_file, syslib |
| AOT integration tests | 1973 | 0 | 17 | Full AOT compile-link-run tests |
| ori_rt unit tests | 360 | 0 | 0 | Runtime library |
| AOT cross tests | 31 | 0 | 0 | Target config, platform, WASM |
| AOT linking tests | 35 | 0 | 0 | Linker driver tests |
| AOT LTO tests | 11 | 0 | 0 | LTO mode tests |
| AOT CLI tests | 41 | 0 | 1 | `ori build` command tests |
| AOT WASM tests | 44 | 0 | 0 | WASM config/linker tests |

## Summary

| Subsection | Items | Done | Partial | Not Started | Notes |
|-----------|-------|------|---------|-------------|-------|
| 21B.1 Target Configuration | 12 | 0 | 0 | 12 | All unchecked but target infra exists |
| 21B.2 Object File Emission | 21 | 0 | 0 | 21 | All unchecked but object emission works |
| 21B.3 Debug Information | 20 | 0 | 0 | 20 | All unchecked but debug info partially works |
| 21B.4 Optimization Pipeline | 14 | 0 | 0 | 14 | All unchecked but opt levels work |
| 21B.5 Linking | 20 | 0 | 0 | 20 | All unchecked but linking works |
| 21B.6 Incremental Compilation | 18 | 0 | 0 | 18 | All unchecked but infra exists |
| 21B.7 WebAssembly Backend | 16 | 0 | 0 | 16 | All unchecked but WASM infra exists |
| 21B.8 CLI Integration | 17 | 0 | 0 | 17 | All unchecked but `ori build` works |
| 21B.8.5 Multi-File Compilation | 23 | 0 | 0 | 23 | All unchecked but multi_file infra exists |
| 21B.9 Error Handling | 13 | 0 | 0 | 13 | Not started |
| 21B.10 End-to-End Pipeline Tests | 8 | 0 | 0 | 8 | Not started |
| 21B.11 Performance & Stress Tests | 4 | 0 | 0 | 4 | Not started |
| 21B.12 Platform-Specific Tests | 12 | 0 | 0 | 12 | Not started |
| 21B.13 ABI & FFI Tests | 8 | 0 | 0 | 8 | Not started |
| 21B.14 Architecture-Specific Codegen | 4 | 0 | 0 | 4 | Not started |
| 21B.15 Testing Infrastructure | 8 | 0 | 0 | 8 | Not started |
| 21B.16 Section Completion Checklist | 40 | 0 | 0 | 40 | All unchecked |

**Hidden implementations found**: Extensive in 21B.1-21B.8.5. Most infrastructure items exist and have tests.

## Detailed Findings

### 21B.1 Target Configuration

All 12 items marked [ ]. Status: `in-progress`.

**Actual state**:
- Target triple parsing and validation: EXISTS and tested. 31 cross-compilation AOT tests pass including `test_target_triple_to_string`, `test_target_config_with_opt_level`, `test_target_platform_detection`, `test_unsupported_targets`.
- Data layout configuration: EXISTS and tested. `test_x86_64_linux_data_layout`, `test_x86_64_macos_data_layout`, `test_wasm32_data_layout` all pass.
- CPU feature detection: EXISTS and tested. `test_x86_64_features`, `test_wasm_features` pass.
- Native target auto-detection: EXISTS (used by all AOT tests -- they all compile and run natively).
- Files: `compiler/ori_llvm/src/aot/target.rs`, `target_features.rs`

**Roadmap claims tests in `ori_llvm/src/aot/target.rs` (20 tests)**: No inline `#[test]` functions in target.rs. Tests are in `compiler/ori_llvm/tests/aot/cross.rs` (31 tests). Path in roadmap is inaccurate.

**Assessment**: Core target configuration is implemented. STALE CHECKBOXES. Test path references are wrong (should reference `tests/aot/cross.rs`).

### 21B.2 Object File Emission

All 21 items marked [ ]. Status: `in-progress`.

**Actual state**:
- LLVM TargetMachine creation: EXISTS and works (all AOT tests create target machines)
- Object file writing: ELF output works (1973 AOT tests produce ELF objects and link them)
- Symbol mangling: EXISTS in `compiler/ori_llvm/src/aot/mangle/` (mod.rs, parse.rs, emit.rs). 6 mangle-related tests found in lib tests.
- `object.rs` exists but has 0 inline tests
- Object file verification tests: 0 dedicated tests (no `util/object.rs` tests verify headers)
- Symbol management tests: 0 dedicated tests

**Roadmap claims tests in `ori_llvm/src/aot/object.rs` (12 tests)**: No tests exist in object.rs. Path is aspirational.
**Roadmap claims tests in `ori_llvm/src/aot/mangle.rs` (15 tests)**: No tests in mangle/ directory. Some mangle tests exist in `function_compiler/tests.rs` (1 test) and `monomorphize/tests.rs` (6 tests). Paths are aspirational.

**Assessment**: Object emission works for ELF. Mangle scheme exists. No dedicated verification or symbol management tests. STALE CHECKBOXES; test paths are aspirational.

### 21B.3 Debug Information

All 20 items marked [ ]. Status: `in-progress`.

**Actual state**:
- DIBuilder integration: EXISTS. 10 debug info unit tests pass in `aot::debug::tests`:
  - `create_parameter_variable_produces_valid_metadata`
  - `create_auto_variable_produces_valid_metadata`
  - `composite_type_cache_deduplicates`
  - `debug_context_emit_declare_for_alloca_convenience`
  - `debug_context_set_location_from_offset`
  - `debug_none_level_returns_none_builder`
  - `create_rc_heap_type_produces_two_field_struct`
  - `emit_dbg_declare_on_alloca_passes_verify`
  - `line_map_offset_to_line_col`
  - `test_debug_level_emission_kind`
- Source location tracking: `LineMap`, `DebugLevel` exist and are tested
- Type debug info: `composite_type_cache_deduplicates`, `create_rc_heap_type_produces_two_field_struct` test type debug info
- Debug format emission: `test_debug_level_emission_kind` tests debug levels
- File: `compiler/ori_llvm/src/aot/debug/mod.rs`

**Roadmap claims 18+5+9+10 tests in `ori_llvm/src/aot/debug.rs`**: Actual is 10 tests. Count is overstated -- likely aspirational.

**Assessment**: Basic debug info works. STALE CHECKBOXES. Test counts aspirational.

### 21B.4 Optimization Pipeline

All 14 items marked [ ]. Status: `in-progress`.

**Actual state**:
- Pass manager: EXISTS. Optimization levels work (AOT tests compile with various opt levels)
- LTO support: EXISTS and tested. 11 LTO tests pass including:
  - `test_full_lto_link_passes`, `test_full_lto_prelink_passes`
  - `test_thin_lto_link_passes`, `test_thin_lto_prelink_passes`
  - `test_lto_all_optimization_levels`, `test_lto_mode_variants`
  - `test_lto_at_o0`, `test_lto_at_oz`, `test_thin_vs_full_lto`
  - `test_no_lto_no_passes`, `test_lto_mode_display`
- O0/O1/O2/O3/Os/Oz: covered by `test_lto_all_optimization_levels`
- No dedicated pass manager config tests (no `passes.rs` test file)

**Roadmap claims 25 tests in `ori_llvm/src/aot/passes.rs`**: passes.rs does not exist as a file. Pass management is integrated elsewhere. Aspirational.

**Assessment**: Optimization pipeline works. LTO tested. STALE CHECKBOXES. File paths aspirational.

### 21B.5 Linking

All 20 items marked [ ]. Status: `in-progress`.

**Actual state**:
- Platform linker driver: EXISTS and tested. 35 linking AOT tests pass including:
  - GCC linker tests (Linux)
  - MSVC linker tests (Windows): `test_msvc_linker_basic`, `test_msvc_linker_dll`, `test_msvc_linker_export_symbols`, `test_msvc_linker_gc_sections`, `test_msvc_linker_library_path`, `test_msvc_linker_strip_symbols`, `test_msvc_linker_with_lld`
  - macOS linker: `test_macos_arm64_linker`
  - Linker flavor dispatch: `test_linker_flavor_for_target`, `test_linker_impl_dispatch`, `test_linker_impl_all_methods`
  - Windows GNU: `test_windows_gnu_uses_gcc_linker`
  - WASM linker: 20 tests in `aot::linker::wasm::tests` (lib)
- Runtime library (libori_rt): EXISTS and tested. 360 ori_rt tests. Runtime discovery tested.
- System library detection: EXISTS. 14 syslib tests pass in `aot::syslib::tests`

**Roadmap claims 68 tests, 81% coverage in `ori_llvm/src/aot/linker.rs`**: Linker tests are in `tests/aot/linking.rs` (35 tests) + `src/aot/linker/wasm/tests.rs` (20 tests) = 55 total. Close to claimed count. Coverage claim unverified.
**Roadmap claims 4 tests in `ori_llvm/src/aot/runtime.rs`**: Not verified -- runtime.rs may have inline tests.
**Roadmap claims 14 tests in `ori_llvm/src/aot/syslib.rs`**: CONFIRMED. 14 tests pass.

**Assessment**: Linking infrastructure is comprehensive. STALE CHECKBOXES.

### 21B.6 Incremental Compilation

All 18 items marked [ ]. Status: `in-progress`.

**Actual state**:
- Source hashing: EXISTS. `aot::incremental::hash::tests` -- tests pass
- Dependency tracking: EXISTS. `aot::incremental::deps::tests` -- tests pass
- Cache management: EXISTS. `aot::incremental::cache::tests` -- tests pass
- Parallel compilation: EXISTS. `aot::incremental::parallel::tests` -- tests pass
- Total incremental tests: 77 lib tests pass
- Additional: `function_deps/tests.rs`, `function_hash/tests.rs`, `arc_cache/tests.rs` all exist

**Roadmap claims 14+12+11+12 = 49 tests**: Actual is 77 total. More tests than claimed.

**Not wired up**: Cache not yet integrated into `ori build` command (confirmed by ignored `test_build_incremental_unchanged` test).

**Assessment**: Infrastructure exists and is well-tested. Integration is the gap. STALE CHECKBOXES.

### 21B.7 WebAssembly Backend

All 16 items marked [ ]. Status: `not-started`.

**Actual state**:
- WASM target config: EXISTS. 44 WASM AOT tests pass + 20 WASM linker lib tests
- Tests cover: `wasm32-unknown-unknown`, `wasm32-wasi`, WASM type mappings, data layout, features
- JS binding generation: `WasmConfig`, `JsBindingGenerator` infrastructure exists (tested via `test_wasm_opt_runner_config`, `test_wasm_opt_runner_custom_path`)
- WASI support: `WasiConfig` exists (tested via `test_wasm_linker_import_export_memory`)
- WASM optimization: `WasmOptRunner` exists and tested

**Roadmap claims 70 tests in `ori_llvm/src/aot/wasm.rs`**: Actual is 44 AOT integration + 20 lib = 64 total. Close to claimed.

**Assessment**: Status says `not-started` but substantial infrastructure and 64 tests exist. STALE STATUS AND CHECKBOXES.

### 21B.8 CLI Integration

All 17 items marked [ ]. Status: `in-progress`.

**Actual state**:
- `ori build` command: EXISTS and tested. 41 CLI AOT tests pass (1 ignored = incremental):
  - `test_build_basic`, `test_build_output_path`, `test_build_output_dir`, `test_build_verbose`
  - `test_main_args_*` (multiple tests covering args, void return, panic, SSO strings, heap strings, valgrind)
  - Main wrapper IR tests: `test_main_no_args_wrapper_uses_call_ir`, `test_main_args_wrapper_uses_invoke_ir`
- `ori targets`: not verified as separate command
- `ori demangle`: mangle infrastructure exists but `ori demangle` command not verified
- `ori run --compile`: not verified

**Roadmap claims 36+25 tests**: 41 actual CLI tests. Close to one of the claimed counts.

**Assessment**: `ori build` works. Many other CLI commands may not be wired up yet. STALE CHECKBOXES.

### 21B.8.5 Multi-File Compilation

All 23 items marked [ ]. Status: `not-started`.

**Actual state**:
- Dependency graph: EXISTS. 15 multi_file tests pass in `aot::multi_file::tests`:
  - `test_extract_imports_basic`, `test_extract_imports_skips_module_imports`
  - `test_resolve_relative_import_*` (6 tests covering file modules, directory modules, not found, parent path, extension, prefer-file-over-directory)
  - `test_graph_build_context_cycle_detection`
  - `test_multi_file_config`, `test_multi_file_error_display`
  - `test_extract_quoted_path`
- Per-module compilation: `declare_external_fn_mangled` infrastructure exists
- Topological sorting: `DependencyGraph::topological_order()` referenced in tests

**Roadmap claims 15 tests in `ori_llvm/src/aot/multi_file.rs`**: CONFIRMED. 15 tests pass.

**Assessment**: Status says `not-started` but dependency graph infrastructure is implemented and well-tested. STALE STATUS.

### 21B.9 Error Handling

All 13 items marked [ ]. Status: `not-started`.

No dedicated error handling test files for AOT pipeline error reporting. Error codes E1201-E1203 not verified.

**Assessment**: Status appears accurate. Not started.

### 21B.10 End-to-End Pipeline Tests

All 8 items marked [ ]. Status: `not-started`.

No `AotTestExecutor` or `--backend=aot` CLI flag found. However, the 1973 AOT integration tests ARE effectively end-to-end tests (compile -> emit -> link -> run). The infrastructure described here (formal test executor, runtime panic detection API) does not exist, but functional end-to-end testing is extensive.

**Assessment**: Formal infrastructure not started, but functional E2E testing exists via AOT integration tests (1973 tests). Status is misleading.

### 21B.11-21B.15

All items marked [ ]. Status: `not-started`.

- Performance/stress tests: 34 stress tests exist in `tests/aot/stress.rs`, 20 memory_stress tests exist. Not zero coverage as claimed.
- Platform-specific: 31 cross tests cover multiple platforms conceptually (Linux, macOS, Windows, WASM)
- ABI/FFI: No dedicated tests
- Architecture codegen: No dedicated tests
- Testing infrastructure: No dedicated utilities as described

**Assessment**: 21B.11 status is partially wrong (stress/memory tests exist). Others mostly accurate.

### 21B.16 Section Completion Checklist

All 40 items marked [ ].

The test coverage summary table claims ~190 scenarios. Actual scenarios tested: significantly more than 190 via the 1973 AOT integration tests + 466 lib tests, though many of the SPECIFIC scenarios listed (e.g., "weak symbol handling", "split DWARF", "GNU hash vs SYSV hash") are not tested.

**TPR review item**: `[done]` claim at bottom of checklist not verified.

## Ignored Tests Audit (AOT relevant)

1 ignored test directly relates to 21B:
- `cli::test_build_incremental_unchanged` -- "Incremental compilation not yet wired up in ori build (see 21B.6)"

This correctly reflects the gap between 21B.6 infrastructure (77 unit tests) and 21B.8 CLI integration.

## Accuracy Assessment

The section's `in-progress` status is accurate but the level of completion is **severely understated**:

1. **21B.7 WASM** claims `not-started` but has 64 tests and working infrastructure
2. **21B.8.5 Multi-File** claims `not-started` but has 15 tests and working dependency graph
3. **21B.1-21B.6 and 21B.8** are all more complete than checkboxes suggest
4. **Test path references** throughout the section often point to aspirational locations (`ori_llvm/src/aot/target.rs (20 tests)`, `ori_llvm/src/aot/passes.rs (25 tests)`) rather than actual test files (`tests/aot/cross.rs`, `tests/aot/lto.rs`)
5. **Test count claims** are sometimes aspirational rather than actual -- but actual counts are often close or exceed the claims

**Major gaps that ARE accurately identified**:
- Incremental compilation not wired into `ori build` (21B.6 integration)
- No formal E2E test backend infrastructure (21B.10)
- No dedicated error handling for AOT pipeline (21B.9)
- No platform-specific linking tests (21B.12)
- No ABI compliance tests (21B.13)
- Many specialized scenarios (weak symbols, split DWARF, LTO cache management) untested

**Recommendation**: Update status of 21B.7 and 21B.8.5 from `not-started` to `in-progress`. Perform a comprehensive checkbox update pass. Correct test file paths from aspirational to actual locations.
