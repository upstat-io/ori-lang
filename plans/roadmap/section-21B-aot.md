---
section: "21B"
title: AOT Compilation
status: not-started
tier: 8
goal: Generate native executables and WebAssembly from Ori source code
sections:
  - id: "21B.1"
    title: Target Configuration
    status: not-started
  - id: "21B.2"
    title: Object File Emission
    status: not-started
  - id: "21B.3"
    title: Debug Information
    status: not-started
  - id: "21B.4"
    title: Optimization Pipeline
    status: not-started
  - id: "21B.5"
    title: Linking
    status: not-started
  - id: "21B.6"
    title: Incremental Compilation
    status: not-started
  - id: "21B.7"
    title: WebAssembly Backend
    status: not-started
  - id: "21B.8"
    title: CLI Integration
    status: not-started
  - id: "21B.8.5"
    title: Multi-File Compilation
    status: not-started
  - id: "21B.9"
    title: Error Handling
    status: not-started
  - id: "21B.10"
    title: End-to-End Pipeline Tests
    status: not-started
  - id: "21B.11"
    title: Performance & Stress Tests
    status: not-started
  - id: "21B.12"
    title: Platform-Specific Tests
    status: not-started
  - id: "21B.13"
    title: ABI & FFI Tests
    status: not-started
  - id: "21B.14"
    title: Architecture-Specific Codegen
    status: not-started
  - id: "21B.15"
    title: Testing Infrastructure
    status: not-started
  - id: "21B.16"
    title: Section Completion Checklist
    status: not-started
---

# Section 21B: AOT Compilation

**Status:** In Progress
**Proposal:** `proposals/approved/aot-compilation-proposal.md`
**Depends on:** Section 21A (LLVM Backend - JIT working)

**Goal:** Generate native executables and WebAssembly from Ori source code.

---

## 21B.1 Target Configuration

- [ ] **Implement**: Target triple parsing and validation
  - [ ] Parse `<arch>-<vendor>-<os>[-<env>]` format
  - [ ] Validate against supported targets list
  - [ ] Native target auto-detection
  - [ ] **Rust Tests**: `ori_llvm/src/aot/target.rs` (20 tests)

- [ ] **Implement**: Data layout configuration
  - [ ] LLVM data layout string per target
  - [ ] Pointer size, alignment, endianness
  - [ ] Module configuration with target triple and data layout
  - [ ] **Rust Tests**: `ori_llvm/src/aot/target.rs`

- [ ] **Implement**: CPU feature detection
  - [ ] `--cpu=native` auto-detection (`with_cpu_native()`)
  - [ ] `--features=+avx2,-sse4` parsing
  - [ ] Host CPU feature detection (`get_host_cpu_features()`)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/target.rs`

**Supported targets (initial):**
| Target | Description |
|--------|-------------|
| `x86_64-unknown-linux-gnu` | 64-bit Linux (glibc) |
| `x86_64-unknown-linux-musl` | 64-bit Linux (musl, static) |
| `x86_64-apple-darwin` | 64-bit macOS (Intel) |
| `aarch64-apple-darwin` | 64-bit macOS (Apple Silicon) |
| `x86_64-pc-windows-msvc` | 64-bit Windows (MSVC) |
| `x86_64-pc-windows-gnu` | 64-bit Windows (MinGW) |
| `wasm32-unknown-unknown` | WebAssembly (standalone) |
| `wasm32-wasi` | WebAssembly (WASI) |

---

## 21B.2 Object File Emission

- [ ] **Implement**: LLVM TargetMachine creation
  - [ ] Configure target triple, CPU, features
  - [ ] Set relocation model (pic, static)
  - [ ] Set code model (small, medium, large)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/target.rs` (existing tests)

- [ ] **Implement**: Object file writing
  - [ ] ELF output (Linux)
  - [ ] Mach-O output (macOS)
  - [ ] COFF output (Windows)
  - [ ] WASM output (WebAssembly)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/object.rs` (12 tests)

- [ ] **Implement**: Symbol mangling
  - [ ] `_ori_<module>_<function>` scheme
  - [ ] Type suffixes for overloads (generic mangling)
  - [ ] Trait method mangling
  - [ ] Demangle function for `ori demangle` command
  - [ ] **Rust Tests**: `ori_llvm/src/aot/mangle.rs` (15 tests)

- [ ] **Test**: Object file verification (HIGH priority)
  - [ ] ELF header validation (magic, class, endian)
  - [ ] ELF section verification (text, data, bss, rodata)
  - [ ] ELF symbol table integrity
  - [ ] Mach-O header validation
  - [ ] Mach-O load commands verification
  - [ ] COFF header validation
  - [ ] COFF section characteristics
  - [ ] Section alignment verification
  - [ ] Relocation entries verification
  - [ ] Dynamic symbol table (dynsym)

- [ ] **Test**: Symbol management (HIGH priority)
  - [ ] Weak symbol handling
  - [ ] Weak undefined symbols
  - [ ] Hidden visibility (`__attribute__((visibility("hidden")))`)
  - [ ] Protected visibility
  - [ ] Symbol export lists (version scripts)
  - [ ] Internal symbol filtering
  - [ ] Allocator symbol hiding (`__rdl_`, `__rde_`, `__rg_`)
  - [ ] Generic function export control
  - [ ] Symbol aliasing

---

## 21B.3 Debug Information

- [ ] **Implement**: DIBuilder integration
  - [ ] Create debug compilation unit
  - [ ] Create debug files and directories
  - [ ] Set producer metadata
  - [ ] **Rust Tests**: `ori_llvm/src/aot/debug.rs` (18 tests)

- [ ] **Implement**: Source location tracking
  - [ ] DILocation for each expression
  - [ ] Line/column mapping from spans (LineMap)
  - [ ] Scope hierarchy (file, function, block)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/debug.rs` (5 additional tests)

- [ ] **Implement**: Type debug info
  - [ ] Primitive type debug info
  - [ ] Struct type debug info
  - [ ] Enum/sum type debug info
  - [ ] Generic type debug info (Option, Result, List)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/debug.rs` (9 additional tests)

- [ ] **Implement**: Debug format emission
  - [ ] DWARF 4 (Linux, macOS, WASM)
  - [ ] dSYM bundle configuration (macOS)
  - [ ] CodeView/PDB configuration (Windows)
  - [ ] Debug levels: none, line-tables, full
  - [ ] **Rust Tests**: `ori_llvm/src/aot/debug.rs` (10 additional tests)

- [ ] **Test**: Debug info verification (MEDIUM priority)
  - [ ] DWARF version selection (4 vs 5)
  - [ ] Line number table accuracy
  - [ ] Column number precision
  - [ ] Function name in debug info
  - [ ] Variable location tracking
  - [ ] Type information completeness
  - [ ] Inlined function attribution
  - [ ] Split DWARF (`.dwo` files)
  - [ ] CodeView format verification (Windows)

---

## 21B.4 Optimization Pipeline

- [ ] **Implement**: Pass manager configuration
  - [ ] LLVM new pass manager setup (via llvm-sys C API)
  - [ ] Module pass pipeline (`LLVMRunPasses` with `default<OX>` strings)
  - [ ] Function pass pipeline (via module adapters)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/passes.rs` (25 tests)

- [ ] **Implement**: Optimization levels
  - [ ] O0: No optimization (fastest compile)
  - [ ] O1: Basic optimization (CSE, SimplifyCFG, DCE)
  - [ ] O2: Standard optimization (LICM, GVN, inlining)
  - [ ] O3: Aggressive optimization (vectorization, full unrolling)
  - [ ] Os: Size optimization
  - [ ] Oz: Aggressive size optimization
  - [ ] **Rust Tests**: `ori_llvm/src/aot/passes.rs`

- [ ] **Implement**: LTO support
  - [ ] Thin LTO (parallel, fast) - `thinlto-pre-link<OX>`, `thinlto<OX>`
  - [ ] Full LTO (maximum optimization) - `lto-pre-link<OX>`, `lto<OX>`
  - [ ] LTO object emission configuration
  - [ ] **Rust Tests**: `ori_llvm/src/aot/passes.rs`

- [ ] **Test**: LTO advanced (MEDIUM priority)
  - [ ] LTO with mixed Rust/C objects
  - [ ] LTO symbol internalization
  - [ ] LTO dead code elimination verification
  - [ ] LTO cache file management
  - [ ] ThinLTO import/export summary
  - [ ] ThinLTO parallelism
  - [ ] LTO bitcode compatibility

- [ ] **Test**: Code model & relocation (MEDIUM priority)
  - [ ] Small code model
  - [ ] Medium code model
  - [ ] Large code model
  - [ ] Static relocation model
  - [ ] PIC (Position Independent Code)
  - [ ] PIE (Position Independent Executable)
  - [ ] Dynamic-no-pic model
  - [ ] Relocatable object generation (`-r`)

---

## 21B.5 Linking

- [ ] **Implement**: Linker driver
  - [ ] Linux: invoke via `cc` or `ld`
  - [ ] macOS: invoke via `clang` or `ld64`
  - [ ] Windows: invoke `link.exe` or `lld-link`
  - [ ] LLD support (`--linker=lld`)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/linker.rs` (68 tests, 81% coverage)

- [ ] **Implement**: Runtime library (libori_rt)
  - [ ] Consolidate Section 21A runtime functions
  - [ ] Memory: `ori_alloc`, `ori_free`, `ori_realloc`
  - [ ] Reference counting: `ori_rc_inc`, `ori_rc_dec`, `ori_rc_new`
  - [ ] Strings: `ori_str_concat`, `ori_str_from_int`, etc.
  - [ ] Collections: `ori_list_new`, `ori_map_new`, etc.
  - [ ] Panic: `ori_panic`, `ori_panic_handler`
  - [ ] I/O: `ori_print`, `ori_stdin_read`
  - [ ] Static linking (default)
  - [ ] Dynamic linking (--link=dynamic)
  - [ ] **Rust Tests**: `ori_rt/src/lib.rs` (19 tests), `ori_llvm/src/aot/runtime.rs` (4 tests)

- [ ] **Implement**: Runtime library discovery
  - **Proposal**: `proposals/approved/runtime-library-discovery-proposal.md` APPROVED 2026-02-02
  - [ ] Walk up from binary to find `libori_rt.a` (like rustc sysroot)
  - [ ] Dev layout: same directory as compiler binary
  - [ ] Installed layout: `<exe>/../lib/libori_rt.a`
  - [ ] Workspace dev: `$ORI_WORKSPACE_DIR/target/{release,debug}/`
  - [ ] CLI override: `--runtime-path` flag (pending CLI integration)
  - [ ] Remove environment variables (ORI_LIB_DIR, ORI_RT_PATH) from current implementation
  - [ ] **Unblocks**: Multi-file AOT compilation (21B.8.5), End-to-end tests (21B.10)

- [ ] **Implement**: System library detection
  - [ ] Platform-specific library paths
  - [ ] Sysroot support for cross-compilation
  - [ ] Library search order
  - [ ] **Rust Tests**: `ori_llvm/src/aot/syslib.rs` (14 tests)

- [ ] **Test**: Linker error handling (HIGH priority)
  - [ ] Undefined symbol error messages
  - [ ] Symbol duplication/conflict errors
  - [ ] Circular dependency detection
  - [ ] Missing library error handling
  - [ ] Broken/corrupted object file handling
  - [ ] Wrong bitcode version in archives
  - [ ] Linker stderr capture and formatting
  - [ ] Helpful suggestions in error messages

- [ ] **Test**: Linker features (HIGH priority)
  - [ ] Link script support (LD scripts)
  - [ ] Linker map file generation
  - [ ] Whole archive linking (`--whole-archive`)
  - [ ] As-needed linking (`--as-needed`)
  - [ ] Rpath configuration
  - [ ] SONAME/install_name configuration
  - [ ] DT_NEEDED ordering
  - [ ] Symbol versioning (glibc)
  - [ ] Two-level namespace (macOS)

---

## 21B.6 Incremental Compilation

- [ ] **Implement**: Source hashing
  - [ ] Content hash per source file (FxHash algorithm)
  - [ ] Store hashes in `build/cache/`
  - [ ] Detect hash mismatches
  - [ ] **Rust Tests**: `ori_llvm/src/aot/incremental/hash.rs` (14 tests)

- [ ] **Implement**: Dependency tracking
  - [ ] Import graph analysis
  - [ ] Transitive dependency detection
  - [ ] Topological ordering for compilation
  - [ ] **Rust Tests**: `ori_llvm/src/aot/incremental/deps.rs` (12 tests)

- [ ] **Implement**: Cache management
  - [ ] Cache validation (source + deps + flags + version)
  - [ ] Cache hit: skip recompilation
  - [ ] Cache miss: recompile and update cache
  - [ ] Parallel cache access
  - [ ] **Rust Tests**: `ori_llvm/src/aot/incremental/cache.rs` (11 tests)

- [ ] **Implement**: Parallel compilation
  - [ ] `--jobs=N` flag
  - [ ] Auto-detect core count (`--jobs=auto`)
  - [ ] Thread pool for module compilation
  - [ ] **Rust Tests**: `ori_llvm/src/aot/incremental/parallel.rs` (12 tests)

- [ ] **Integrate**: Wire up cache to `ori build` command
  - [ ] Add cache lookup before compilation in `build_file()`
  - [ ] Store compiled objects in cache after successful build
  - [ ] Add `--no-cache` flag to bypass incremental compilation
  - [ ] Add verbose output for cache hits/misses
  - [ ] **Blocks**: 21B.8 incremental test (`test_build_incremental_unchanged`)

- [ ] **Test**: Incremental compilation advanced (MEDIUM priority)
  - [ ] Source hash computation
  - [ ] Dependency graph tracking
  - [ ] Cache key generation (source + deps + flags + version)
  - [ ] Cache hit detection (skip recompile)
  - [ ] Cache invalidation on change
  - [ ] Parallel compilation (`-j` flag)
  - [ ] Incremental debug info
  - [ ] Incremental metadata

---

## 21B.7 WebAssembly Backend

- [ ] **Implement**: WASM target configuration
  - [ ] `wasm32-unknown-unknown` (standalone)
  - [ ] `wasm32-wasi` (WASI preview 2)
  - [ ] WASM-specific data layout
  - [ ] Memory import/export
  - [ ] **Rust Tests**: `ori_llvm/src/aot/wasm.rs` (70 tests)

- [ ] **Implement**: JavaScript binding generation
  - [ ] `--js-bindings` flag support via `WasmConfig`
  - [ ] Generate `<name>.js` glue code
  - [ ] Generate `<name>.d.ts` TypeScript declarations
  - [ ] String marshalling (TextEncoder/TextDecoder)
  - [ ] Heap slab for JsValue handles
  - [ ] **Rust Tests**: `ori_llvm/src/aot/wasm.rs`

- [ ] **Implement**: WASI support
  - [ ] WASI import declarations (`WasiConfig::undefined_symbols()`)
  - [ ] File system configuration
  - [ ] Clock/random shim configuration
  - [ ] **Rust Tests**: `ori_llvm/src/aot/wasm.rs`

- [ ] **Implement**: WASM optimization
  - [ ] `--opt=z` for smallest size (`WasmOptLevel::Oz`)
  - [ ] `--wasm-opt` post-processor integration (`WasmOptRunner`)
  - [ ] Tree-shaking support via wasm-opt
  - [ ] **Rust Tests**: `ori_llvm/src/aot/wasm.rs`

- [ ] **Test**: WASM advanced (MEDIUM priority)
  - [ ] Custom section embedding
  - [ ] Data segment placement verification
  - [ ] Start function configuration
  - [ ] Table initialization
  - [ ] Global initialization
  - [ ] Memory limits enforcement
  - [ ] Import namespace verification
  - [ ] Multi-memory support
  - [ ] Exception handling sections
  - [ ] Name section for debugging

---

## 21B.8 CLI Integration

- [ ] **Implement**: `ori build` command
  - [ ] Parse all flags (--release, --target, --opt, etc.)
  - [ ] Output path handling (-o, --out-dir)
  - [ ] Emit mode (--emit=obj, llvm-ir, llvm-bc, asm)
  - [ ] Library modes (--lib, --dylib)
  - [ ] Verbose output (-v)
  - [ ] **Rust Tests**: `oric/src/commands/build.rs` (36 tests)
  - [ ] **CLI Tests**: `ori_llvm/tests/aot/cli.rs` (25 tests)

- [ ] **Implement**: `ori targets` command
  - [ ] List all supported targets
  - [ ] `--installed` flag for targets with sysroots
  - [ ] **Rust Tests**: `oric/src/commands/targets.rs` (8 tests, requires LLVM feature)

- [ ] **Implement**: `ori target` command (cross-compilation)
  - [ ] `ori target add <target>` - download sysroot
  - [ ] `ori target remove <target>` - remove sysroot
  - [ ] `ori target list` - list installed targets
  - [ ] Sysroot management
  - [ ] **Rust Tests**: `oric/src/commands/target.rs` (7 tests)

- [ ] **Implement**: `ori demangle` command
  - [ ] Parse mangled symbol names
  - [ ] Output demangled Ori names
  - [ ] **Rust Tests**: `oric/src/commands/demangle.rs` (9 tests, requires LLVM feature)

- [ ] **Implement**: `ori run --compile` mode
  - [ ] AOT compile then execute
  - [ ] Faster repeated runs
  - [ ] Cache compiled binary (hash-based in ~/.cache/ori/compiled/)
  - [ ] **Rust Tests**: `oric/src/commands/run.rs` (5 tests, requires LLVM feature)

- [ ] **Test**: CLI integration (25 tests in `ori_llvm/tests/aot/cli.rs`)
  - [ ] `ori build` basic compilation
  - [ ] `ori build --target` cross-compilation (WASM object emission)
  - [ ] `ori build --release` optimization mode
  - [ ] `ori build --emit=obj,asm,llvm-ir` output types
  - [ ] `ori build -o <path>` output path
  - [ ] `ori build --verbose` verbose output
  - [ ] `ori targets` list supported targets
  - [ ] `ori targets --installed` list installed targets
  - [ ] `ori target list/add/remove` target management
  - [ ] Build with missing dependencies error
  - [ ] Build with invalid source error
  - [ ] Build with unsupported target error
  - [ ] Build incremental (unchanged = no rebuild) — blocked on 21B.6 integration

---

## 21B.8.5 Multi-File Compilation

**Proposal:** `proposals/approved/multi-file-aot-proposal.md`

Enable AOT compilation of Ori programs with imports. Currently, `ori build` produces broken binaries when the source uses `use` statements.

### 21B.8.5.1 Dependency Graph Infrastructure

- [ ] **Implement**: `build_dependency_graph()` in `ori_llvm/src/aot/multi_file.rs`
  - [ ] Build import graph from entry file using import extraction
  - [ ] Handle relative imports (`./helper`, `../utils`)
  - [ ] Handle directory modules (`./http` → `http/mod.ori`)
  - [ ] Handle stdlib imports (`std.math` via `ORI_STDLIB`)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/multi_file.rs` (15 tests)

- [ ] **Implement**: Topological sorting for compilation order
  - [ ] Sort modules so dependencies compile before dependents (reuses `DependencyGraph::topological_order()`)
  - [ ] Integrate with cycle detection via `GraphBuildContext`
  - [ ] **Rust Tests**: `ori_llvm/src/aot/multi_file.rs`

### 21B.8.5.2 Per-Module Compilation

- [ ] **Implement**: Per-module compilation in `build_file_multi()`
  - [ ] Compile single module to object file
  - [ ] Use module-qualified name mangling (`_ori_<module>$<function>`)
  - [ ] Generate `declare` for imported symbols via `declare_external_fn_mangled()`
  - [ ] **Rust Tests**: `ori_llvm/src/declare.rs`

- [ ] **Implement**: Update `ori demangle` for module paths
  - [ ] Parse `_ori_helper$my_assert` → `helper.@my_assert`
  - [ ] Handle nested paths (`_ori_http$client$connect` → `http/client.@connect`)
  - [ ] **Rust Tests**: `oric/src/commands/demangle.rs` (9 tests)

### 21B.8.5.3 Linking Integration

- [ ] **Implement**: Multi-file linking in `build_file_multi()`
  - [ ] Collect all object files from dependency graph
  - [ ] Pass to existing linker infrastructure via `link_and_finish()`
  - [ ] Handle stdlib library paths via `ORI_STDLIB`
  - [ ] **Rust Tests**: Covered by existing linker tests

### 21B.8.5.4 Cache Integration

- [ ] **Implement**: Wire incremental cache (21B.6) to multi-file builds
  - [ ] Check cache for each module before compilation
  - [ ] Store module hash including import signatures
  - [ ] Invalidate dependents when module changes
  - [ ] **Rust Tests**: `ori_llvm/src/aot/multi_file.rs`

### 21B.8.5.5 Error Handling

- [ ] **Implement**: Multi-file error reporting
  - [ ] E5004: Import target not found (searched paths in note)
  - [ ] E5005: Imported item not found (with "did you mean?" suggestions)
  - [ ] E5006: Imported item is private (suggest `::` prefix)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/multi_file.rs`

### 21B.8.5.6 Testing

- [ ] **Test**: Basic multi-file compilation
  - [ ] `use "./helper" { func }` compiles and runs
  - [ ] Transitive imports (A imports B imports C)
  - [ ] Module alias (`use "./mod" as m`)

- [ ] **Test**: Directory modules
  - [ ] `use "./http"` resolves to `http/mod.ori`
  - [ ] Re-exports via `pub use`

- [ ] **Test**: Error cases
  - [ ] Circular import detection (E5003)
  - [ ] Missing import target (E5004)
  - [ ] Missing item in module (E5005)
  - [ ] Private item without `::` (E5006)

- [ ] **Test**: Stdlib imports
  - [ ] `use std.math { abs }` with `ORI_STDLIB` set

- [ ] **Test**: Incremental builds
  - [ ] Change one module → only that module recompiles
  - [ ] Change import signature → dependents recompile

---

## 21B.9 Error Handling

- [ ] **Implement**: Linker error reporting
  - [ ] Error code E1201: linker failed
  - [ ] Capture linker stderr
  - [ ] Suggest fixes for common errors
  - [ ] **Rust Tests**: `ori_llvm/src/aot/error_tests.rs`

- [ ] **Implement**: Target error reporting
  - [ ] Error code E1202: unsupported target
  - [ ] List supported targets in help
  - [ ] **Rust Tests**: `ori_llvm/src/aot/error_tests.rs`

- [ ] **Implement**: Object generation error reporting
  - [ ] Error code E1203: failed to generate object file
  - [ ] Capture LLVM error messages
  - [ ] Suggest filing bug report
  - [ ] **Rust Tests**: `ori_llvm/src/aot/error_tests.rs`

- [ ] **Test**: Error handling (CRITICAL - ~5% coverage)
  - [ ] Linker not found error (cc, lld, link.exe)
  - [ ] Linker execution failed (exit code)
  - [ ] Linker stderr parsing and formatting
  - [ ] Target not supported error
  - [ ] Target machine creation failed error
  - [ ] Invalid triple format error
  - [ ] Object file write error (disk full, permissions)
  - [ ] Object file read error (corrupted, wrong format)
  - [ ] LTO bitcode incompatibility error
  - [ ] Debug info generation error
  - [ ] Response file creation error
  - [ ] Helpful error suggestions ("did you mean X?")
  - [ ] Error codes (E0001, E0002, etc.)

- [ ] **Test**: Error diagnostics
  - [ ] LLVM error propagation
  - [ ] Unsupported target error
  - [ ] Unsupported CPU feature error
  - [ ] Architecture mismatch detection
  - [ ] Suggested fixes in errors
  - [ ] List supported targets in help
  - [ ] Sysroot hints for cross-compilation

---

## 21B.10 End-to-End Pipeline Tests

> **CRITICAL** - 0% coverage. No tests for full: parse → typeck → codegen → link → execute

**Proposal:** `proposals/approved/aot-test-backend-proposal.md`

### 21B.10.1 AOT Test Backend Infrastructure

- [ ] **Implement**: Runtime panic detection API (`ori_rt`)
  - [ ] Add `ori_rt_had_panic() -> bool`
  - [ ] Add `ori_rt_reset_panic() -> void`
  - [ ] **Rust Tests**: `ori_rt/src/panic.rs`

- [ ] **Implement**: `AotTestExecutor` (`ori_llvm/src/aot/test_executor.rs`)
  - [ ] `AotTestExecutor::native()` — create executor for host target
  - [ ] Test wrapper generation (main function with panic check)
  - [ ] `execute_test()` — full compile → emit → link → run flow
  - [ ] **Rust Tests**: `ori_llvm/src/aot/test_executor.rs`

- [ ] **Implement**: Test runner integration
  - [ ] Add `Backend::AOT` enum variant
  - [ ] Add `--backend=aot` CLI flag
  - [ ] Wire up `run_file_aot()` in test runner
  - [ ] **Rust Tests**: `oric/src/test/runner_aot_tests.rs`

### 21B.10.2 End-to-End Test Scenarios

- [ ] **Test**: End-to-end execution via AOT backend
  - [ ] Compile and run "hello world"
  - [ ] Compile and run with arguments
  - [ ] Compile and run with exit code
  - [ ] Compile and run with stdout capture
  - [ ] Compile and run with stderr capture
  - [ ] Compile shared library and load dynamically
  - [ ] Compile static library and link
  - [ ] Compile with FFI and call C function
  - [ ] Compile with multiple source files
  - [ ] Compile with dependencies
  - [ ] Cross-compile and verify binary format

- [ ] **Test**: Spec test validation
  - [ ] Run `tests/spec/` through `--backend=aot`
  - [ ] Compare results: Interpreter vs JIT vs AOT
  - [ ] Document any backend-specific differences

---

## 21B.11 Performance & Stress Tests

> **CRITICAL** - 0% coverage. No large module or parallel compilation tests

- [ ] **Test**: Performance benchmarks
  - [ ] Compile large module (10K+ lines)
  - [ ] Compile many small modules (100+ files)
  - [ ] Parallel compilation scaling (1, 2, 4, 8 cores)
  - [ ] Memory usage under large compilation
  - [ ] Incremental rebuild time (small change)
  - [ ] Full rebuild time (clean build)
  - [ ] LTO compilation time
  - [ ] Debug build vs release build time

---

## 21B.12 Platform-Specific Tests

### Linux (MEDIUM priority)
> Reference: Rust `tests/run-make/`, Zig `test/link/`

- [ ] **Test**: Linux-specific linking
  - [ ] glibc vs musl linking differences
  - [ ] COPYREL relocations
  - [ ] GNU hash vs SYSV hash
  - [ ] Stack executable flag (PT_GNU_STACK)
  - [ ] RELRO (Relocation Read-Only)
  - [ ] Now binding (`-z now`)
  - [ ] Lazy binding
  - [ ] Init/fini arrays

### macOS (MEDIUM priority)
> Reference: Rust macOS-specific run-make tests

- [ ] **Test**: macOS-specific linking
  - [ ] Framework linking (`-framework`)
  - [ ] Code signing requirements
  - [ ] dSYM bundle structure verification
  - [ ] Deployment target (`-mmacosx-version-min`)
  - [ ] SDK version specification
  - [ ] Universal binary (fat binary) support
  - [ ] `@rpath`, `@loader_path`, `@executable_path`
  - [ ] Two-level namespace vs flat namespace

### Windows (MEDIUM priority)
> Reference: Rust Windows run-make tests

- [ ] **Test**: Windows-specific linking
  - [ ] Import library generation (`.lib`)
  - [ ] Export definition files (`.def`)
  - [ ] Subsystem specification (`/SUBSYSTEM:CONSOLE`)
  - [ ] Manifest file embedding
  - [ ] SafeSEH configuration
  - [ ] DEP (Data Execution Prevention)
  - [ ] ASLR configuration
  - [ ] Function table (pdata/xdata)
  - [ ] Debug directory (PDB path)

---

## 21B.13 ABI & FFI Tests

### ABI Compliance (LOW priority)
> Reference: Rust ABI tests, Zig calling convention tests

- [ ] **Test**: ABI compliance
  - [ ] C ABI struct passing (by value vs pointer)
  - [ ] C ABI return value handling
  - [ ] System V AMD64 ABI compliance
  - [ ] Windows x64 ABI compliance
  - [ ] ARM AAPCS compliance
  - [ ] Variadic function argument passing
  - [ ] Struct alignment in ABI
  - [ ] Union layout verification

### FFI Type Verification (LOW priority)
> Reference: Rust FFI tests

- [ ] **Test**: FFI type verification
  - [ ] `c_int`, `c_long` size per platform
  - [ ] Pointer size consistency
  - [ ] `size_t`, `ptrdiff_t` mapping
  - [ ] Struct padding rules
  - [ ] Bitfield layout
  - [ ] Enum representation
  - [ ] Function pointer ABI

---

## 21B.14 Architecture-Specific Codegen

> LOW priority - Reference: Rust codegen tests, Zig behavior tests

- [ ] **Test**: Architecture codegen
  - [ ] x86_64 AVX/AVX2/AVX512 codegen
  - [ ] ARM64 NEON codegen
  - [ ] SIMD operation correctness
  - [ ] Atomic operation codegen
  - [ ] Memory ordering codegen
  - [ ] Inline assembly handling
  - [ ] CPU feature detection at runtime

---

## 21B.15 Testing Infrastructure

> Required utilities for comprehensive testing

- [ ] **Implement**: LLVM test utilities
  - [ ] `llvm_ar` - archive manipulation
  - [ ] `llvm_nm` - symbol table inspection
  - [ ] `llvm_readobj` - object file inspection
  - [ ] `llvm_objdump` - disassembly verification
  - [ ] `diff_output` - output comparison
  - [ ] `run_make_support` - composable test helpers

- [ ] **Implement**: Test infrastructure
  - [ ] Parameterized tests with Options
  - [ ] Platform skip directives
  - [ ] Overlay system for multi-language tests

---

## 21B.16 Section Completion Checklist

**Target Configuration (21B.1):**
- [ ] Target triple parsing and validation
- [ ] Data layout configuration
- [ ] CPU feature detection
- [ ] Native target auto-detection

**Object Emission (21B.2):**
- [ ] ELF, Mach-O, COFF, WASM output
- [ ] Symbol mangling scheme
- [ ] `ori demangle` command (with tests)
- [ ] Object file verification tests (10 scenarios)
- [ ] Symbol management tests (9 scenarios)

**Debug Information (21B.3):**
- [ ] DWARF 4 emission
- [ ] dSYM bundle (macOS)
- [ ] CodeView/PDB (Windows)
- [ ] Source location tracking
- [ ] Debug info verification tests (9 scenarios)

**Optimization (21B.4):**
- [ ] O0-O3, Os, Oz levels
- [ ] Thin LTO and Full LTO
- [ ] Pass manager configuration
- [ ] LTO advanced tests (7 scenarios)
- [ ] Code model tests (8 scenarios)

**Linking (21B.5):**
- [ ] System linker driver (cc/clang/link.exe)
- [ ] LLD support
- [ ] Runtime library (libori_rt)
- [ ] Static and dynamic linking
- [ ] Runtime library discovery (binary-relative, like rustc sysroot)
- [ ] Linker error handling tests (8 scenarios)
- [ ] Linker feature tests (9 scenarios)

**Incremental (21B.6):**
- [ ] Source hashing
- [ ] Dependency tracking
- [ ] Cache management
- [ ] Parallel compilation
- [ ] Wire up cache to `ori build` command (blocks 21B.8 incremental test)
- [ ] Incremental advanced tests (8 scenarios)

**WebAssembly (21B.7):**
- [ ] wasm32-unknown-unknown target
- [ ] wasm32-wasi target
- [ ] JavaScript binding generation
- [ ] TypeScript declarations
- [ ] WASM advanced tests (10 scenarios)

**CLI (21B.8):**
- [ ] `ori build` command (with tests)
- [ ] `ori targets` command (with tests)
- [ ] `ori target add/remove` commands (with tests)
- [ ] `ori demangle` command (with tests)
- [ ] `ori run --compile` mode (with tests)
- [ ] CLI integration tests (25 end-to-end tests)
- [ ] Build incremental test (blocked on 21B.6 integration)

**Multi-File Compilation (21B.8.5):**
- [ ] Dependency graph infrastructure
- [ ] Per-module compilation with name mangling
- [ ] Linking integration
- [ ] `ori demangle` Ori-style output (`module.@function`)
- [ ] Cache integration (reuse 21B.6)
- [ ] Error handling (E5004-E5006)
- [ ] Multi-file tests (13 scenarios)

**Error Handling (21B.9):**
- [ ] Linker error reporting
- [ ] Target error reporting
- [ ] Object generation error reporting
- [ ] Error handling tests (13 scenarios)
- [ ] Error diagnostics tests (7 scenarios)

**End-to-End Pipeline (21B.10):**
- [ ] End-to-end execution tests (12 scenarios)

**Performance & Stress (21B.11):**
- [ ] Performance benchmark tests (8 scenarios)

**Platform-Specific (21B.12):**
- [ ] Linux-specific tests (8 scenarios)
- [ ] macOS-specific tests (8 scenarios)
- [ ] Windows-specific tests (9 scenarios)

**ABI & FFI (21B.13):**
- [ ] ABI compliance tests (8 scenarios)
- [ ] FFI type verification tests (7 scenarios)

**Architecture Codegen (21B.14):**
- [ ] Architecture codegen tests (7 scenarios)

**Testing Infrastructure (21B.15):**
- [ ] LLVM test utilities (6 tools)
- [ ] Test infrastructure (3 features)

**Test Coverage Summary:**
| Priority | Category | Scenarios |
|----------|----------|-----------|
| CRITICAL | CLI Integration (21B.8) | 12 |
| CRITICAL | Multi-File Compilation (21B.8.5) | 13 |
| CRITICAL | Error Handling (21B.9) | 20 |
| CRITICAL | End-to-End Pipeline (21B.10) | 12 |
| CRITICAL | Performance/Stress (21B.11) | 8 |
| HIGH | Linker Tests (21B.5) | 17 |
| HIGH | Object File Tests (21B.2) | 19 |
| MEDIUM | Platform-Specific (21B.12) | 25 |
| MEDIUM | WASM Advanced (21B.7) | 10 |
| MEDIUM | LTO Advanced (21B.4) | 15 |
| MEDIUM | Incremental (21B.6) | 8 |
| MEDIUM | Debug Info (21B.3) | 9 |
| LOW | ABI/FFI (21B.13) | 15 |
| LOW | Architecture (21B.14) | 7 |
| **Total** | | **~190 scenarios** |

**Exit Criteria:** Native executables and WASM modules can be generated from Ori source with full debug support, optimization levels, incremental compilation, and multi-file import support. All test scenarios pass with comprehensive coverage.

---

## LLVM Version Requirement

**Required:** LLVM 17 or later

Rationale:
- Best WASM support with Component Model preview
- Newest pass manager (default since LLVM 14)
- Improved debug info generation
- No legacy compatibility burden

---

## Running Tests

```bash
# Run AOT-specific tests
./docker/llvm/run.sh cargo test -p ori_llvm --lib aot

# Run WASM-specific tests
./docker/llvm/run.sh cargo test -p ori_llvm --lib wasm

# Build and run an executable
./docker/llvm/run.sh ori build src/main.ori -o myapp && ./myapp

# Build for WASM
./docker/llvm/run.sh ori build --wasm src/main.ori -o myapp.wasm
```
