---
section: "21B"
title: AOT Compilation
status: in-progress
tier: 8
goal: Generate native executables and WebAssembly from Ori source code
sections:
  - id: "21B.1"
    title: Target Configuration
    status: complete
  - id: "21B.2"
    title: Object File Emission
    status: complete
  - id: "21B.3"
    title: Debug Information
    status: complete
  - id: "21B.4"
    title: Optimization Pipeline
    status: complete
  - id: "21B.5"
    title: Linking
    status: complete
  - id: "21B.6"
    title: Incremental Compilation
    status: complete
  - id: "21B.7"
    title: WebAssembly Backend
    status: complete
  - id: "21B.8"
    title: CLI Integration
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

**Status:** 🔶 In Progress
**Proposal:** `proposals/approved/aot-compilation-proposal.md`
**Depends on:** Section 21A (LLVM Backend - JIT working)

**Goal:** Generate native executables and WebAssembly from Ori source code.

---

## 21B.1 Target Configuration

- [x] **Implement**: Target triple parsing and validation
  - [x] Parse `<arch>-<vendor>-<os>[-<env>]` format
  - [x] Validate against supported targets list
  - [x] Native target auto-detection
  - [x] **Rust Tests**: `ori_llvm/src/aot/target.rs` (20 tests)

- [x] **Implement**: Data layout configuration
  - [x] LLVM data layout string per target
  - [x] Pointer size, alignment, endianness
  - [x] Module configuration with target triple and data layout
  - [x] **Rust Tests**: `ori_llvm/src/aot/target.rs`

- [x] **Implement**: CPU feature detection
  - [x] `--cpu=native` auto-detection (`with_cpu_native()`)
  - [x] `--features=+avx2,-sse4` parsing
  - [x] Host CPU feature detection (`get_host_cpu_features()`)
  - [x] **Rust Tests**: `ori_llvm/src/aot/target.rs`

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

- [x] **Implement**: LLVM TargetMachine creation
  - [x] Configure target triple, CPU, features
  - [x] Set relocation model (pic, static)
  - [x] Set code model (small, medium, large)
  - [x] **Rust Tests**: `ori_llvm/src/aot/target.rs` (existing tests)

- [x] **Implement**: Object file writing
  - [x] ELF output (Linux)
  - [x] Mach-O output (macOS)
  - [x] COFF output (Windows)
  - [x] WASM output (WebAssembly)
  - [x] **Rust Tests**: `ori_llvm/src/aot/object.rs` (12 tests)

- [x] **Implement**: Symbol mangling
  - [x] `_ori_<module>_<function>` scheme
  - [x] Type suffixes for overloads (generic mangling)
  - [x] Trait method mangling
  - [x] Demangle function for `ori demangle` command
  - [x] **Rust Tests**: `ori_llvm/src/aot/mangle.rs` (15 tests)

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

- [x] **Implement**: DIBuilder integration
  - [x] Create debug compilation unit
  - [x] Create debug files and directories
  - [x] Set producer metadata
  - [x] **Rust Tests**: `ori_llvm/src/aot/debug.rs` (18 tests)

- [x] **Implement**: Source location tracking
  - [x] DILocation for each expression
  - [x] Line/column mapping from spans (LineMap)
  - [x] Scope hierarchy (file, function, block)
  - [x] **Rust Tests**: `ori_llvm/src/aot/debug.rs` (5 additional tests)

- [x] **Implement**: Type debug info
  - [x] Primitive type debug info
  - [x] Struct type debug info
  - [x] Enum/sum type debug info
  - [x] Generic type debug info (Option, Result, List)
  - [x] **Rust Tests**: `ori_llvm/src/aot/debug.rs` (9 additional tests)

- [x] **Implement**: Debug format emission
  - [x] DWARF 4 (Linux, macOS, WASM)
  - [x] dSYM bundle configuration (macOS)
  - [x] CodeView/PDB configuration (Windows)
  - [x] Debug levels: none, line-tables, full
  - [x] **Rust Tests**: `ori_llvm/src/aot/debug.rs` (10 additional tests)

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

- [x] **Implement**: Pass manager configuration
  - [x] LLVM new pass manager setup (via llvm-sys C API)
  - [x] Module pass pipeline (`LLVMRunPasses` with `default<OX>` strings)
  - [x] Function pass pipeline (via module adapters)
  - [x] **Rust Tests**: `ori_llvm/src/aot/passes.rs` (25 tests)

- [x] **Implement**: Optimization levels
  - [x] O0: No optimization (fastest compile)
  - [x] O1: Basic optimization (CSE, SimplifyCFG, DCE)
  - [x] O2: Standard optimization (LICM, GVN, inlining)
  - [x] O3: Aggressive optimization (vectorization, full unrolling)
  - [x] Os: Size optimization
  - [x] Oz: Aggressive size optimization
  - [x] **Rust Tests**: `ori_llvm/src/aot/passes.rs`

- [x] **Implement**: LTO support
  - [x] Thin LTO (parallel, fast) - `thinlto-pre-link<OX>`, `thinlto<OX>`
  - [x] Full LTO (maximum optimization) - `lto-pre-link<OX>`, `lto<OX>`
  - [x] LTO object emission configuration
  - [x] **Rust Tests**: `ori_llvm/src/aot/passes.rs`

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

- [x] **Implement**: Linker driver
  - [x] Linux: invoke via `cc` or `ld`
  - [x] macOS: invoke via `clang` or `ld64`
  - [x] Windows: invoke `link.exe` or `lld-link`
  - [x] LLD support (`--linker=lld`)
  - [x] **Rust Tests**: `ori_llvm/src/aot/linker.rs` (68 tests, 81% coverage)

- [x] **Implement**: Runtime library (libori_rt)
  - [x] Consolidate Section 21A runtime functions
  - [x] Memory: `ori_alloc`, `ori_free`, `ori_realloc`
  - [x] Reference counting: `ori_rc_inc`, `ori_rc_dec`, `ori_rc_new`
  - [x] Strings: `ori_str_concat`, `ori_str_from_int`, etc.
  - [x] Collections: `ori_list_new`, `ori_map_new`, etc.
  - [x] Panic: `ori_panic`, `ori_panic_handler`
  - [x] I/O: `ori_print`, `ori_stdin_read`
  - [x] Static linking (default)
  - [x] Dynamic linking (--link=dynamic)
  - [x] **Rust Tests**: `ori_rt/src/lib.rs` (19 tests), `ori_llvm/src/aot/runtime.rs` (4 tests)

- [x] **Implement**: System library detection
  - [x] Platform-specific library paths
  - [x] Sysroot support for cross-compilation
  - [x] Library search order
  - [x] **Rust Tests**: `ori_llvm/src/aot/syslib.rs` (14 tests)

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

- [x] **Implement**: Source hashing
  - [x] Content hash per source file (FxHash algorithm)
  - [x] Store hashes in `build/cache/`
  - [x] Detect hash mismatches
  - [x] **Rust Tests**: `ori_llvm/src/aot/incremental/hash.rs` (14 tests)

- [x] **Implement**: Dependency tracking
  - [x] Import graph analysis
  - [x] Transitive dependency detection
  - [x] Topological ordering for compilation
  - [x] **Rust Tests**: `ori_llvm/src/aot/incremental/deps.rs` (12 tests)

- [x] **Implement**: Cache management
  - [x] Cache validation (source + deps + flags + version)
  - [x] Cache hit: skip recompilation
  - [x] Cache miss: recompile and update cache
  - [x] Parallel cache access
  - [x] **Rust Tests**: `ori_llvm/src/aot/incremental/cache.rs` (11 tests)

- [x] **Implement**: Parallel compilation
  - [x] `--jobs=N` flag
  - [x] Auto-detect core count (`--jobs=auto`)
  - [x] Thread pool for module compilation
  - [x] **Rust Tests**: `ori_llvm/src/aot/incremental/parallel.rs` (12 tests)

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

- [x] **Implement**: WASM target configuration
  - [x] `wasm32-unknown-unknown` (standalone)
  - [x] `wasm32-wasi` (WASI preview 2)
  - [x] WASM-specific data layout
  - [x] Memory import/export
  - [x] **Rust Tests**: `ori_llvm/src/aot/wasm.rs` (70 tests)

- [x] **Implement**: JavaScript binding generation
  - [x] `--js-bindings` flag support via `WasmConfig`
  - [x] Generate `<name>.js` glue code
  - [x] Generate `<name>.d.ts` TypeScript declarations
  - [x] String marshalling (TextEncoder/TextDecoder)
  - [x] Heap slab for JsValue handles
  - [x] **Rust Tests**: `ori_llvm/src/aot/wasm.rs`

- [x] **Implement**: WASI support
  - [x] WASI import declarations (`WasiConfig::undefined_symbols()`)
  - [x] File system configuration
  - [x] Clock/random shim configuration
  - [x] **Rust Tests**: `ori_llvm/src/aot/wasm.rs`

- [x] **Implement**: WASM optimization
  - [x] `--opt=z` for smallest size (`WasmOptLevel::Oz`)
  - [x] `--wasm-opt` post-processor integration (`WasmOptRunner`)
  - [x] Tree-shaking support via wasm-opt
  - [x] **Rust Tests**: `ori_llvm/src/aot/wasm.rs`

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
  - [ ] **Rust Tests**: `oric/src/commands/build_tests.rs`
  - [ ] **Ori Tests**: `tests/spec/tooling/build.ori`

- [ ] **Implement**: `ori targets` command
  - [ ] List all supported targets
  - [ ] `--installed` flag for targets with sysroots
  - [ ] **Rust Tests**: `oric/src/commands/targets_tests.rs`

- [ ] **Implement**: `ori target` command (cross-compilation)
  - [ ] `ori target add <target>` - download sysroot
  - [ ] `ori target remove <target>` - remove sysroot
  - [ ] `ori target list` - list installed targets
  - [ ] Sysroot management
  - [ ] **Rust Tests**: `oric/src/commands/target_tests.rs`

- [ ] **Implement**: `ori demangle` command
  - [ ] Parse mangled symbol names
  - [ ] Output demangled Ori names
  - [ ] **Rust Tests**: `oric/src/commands/demangle_tests.rs`

- [ ] **Implement**: `ori run --compile` mode
  - [ ] AOT compile then execute
  - [ ] Faster repeated runs
  - [ ] Cache compiled binary
  - [ ] **Rust Tests**: `oric/src/commands/run_aot_tests.rs`

- [ ] **Test**: CLI integration (CRITICAL - 0% coverage)
  - [ ] `ori build` basic compilation
  - [ ] `ori build --target` cross-compilation
  - [ ] `ori build --release` optimization mode
  - [ ] `ori build --emit=obj,asm,llvm-ir` output types
  - [ ] `ori build -o <path>` output path
  - [ ] `ori build --verbose` verbose output
  - [ ] `ori targets` list supported targets
  - [ ] `ori targets --installed` list installed targets
  - [ ] `ori targets --add <target>` add target support
  - [ ] Build with missing dependencies error
  - [ ] Build with invalid source error
  - [ ] Build incremental (unchanged = no rebuild)

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
- [x] Target triple parsing and validation
- [x] Data layout configuration
- [x] CPU feature detection
- [x] Native target auto-detection

**Object Emission (21B.2):**
- [x] ELF, Mach-O, COFF, WASM output
- [x] Symbol mangling scheme
- [ ] `ori demangle` command
- [ ] Object file verification tests (10 scenarios)
- [ ] Symbol management tests (9 scenarios)

**Debug Information (21B.3):**
- [x] DWARF 4 emission
- [x] dSYM bundle (macOS)
- [x] CodeView/PDB (Windows)
- [x] Source location tracking
- [ ] Debug info verification tests (9 scenarios)

**Optimization (21B.4):**
- [x] O0-O3, Os, Oz levels
- [x] Thin LTO and Full LTO
- [x] Pass manager configuration
- [ ] LTO advanced tests (7 scenarios)
- [ ] Code model tests (8 scenarios)

**Linking (21B.5):**
- [x] System linker driver (cc/clang/link.exe)
- [x] LLD support
- [x] Runtime library (libori_rt)
- [x] Static and dynamic linking
- [ ] Linker error handling tests (8 scenarios)
- [ ] Linker feature tests (9 scenarios)

**Incremental (21B.6):**
- [x] Source hashing
- [x] Dependency tracking
- [x] Cache management
- [x] Parallel compilation
- [ ] Incremental advanced tests (8 scenarios)

**WebAssembly (21B.7):**
- [x] wasm32-unknown-unknown target
- [x] wasm32-wasi target
- [x] JavaScript binding generation
- [x] TypeScript declarations
- [ ] WASM advanced tests (10 scenarios)

**CLI (21B.8):**
- [ ] `ori build` command
- [ ] `ori targets` command
- [ ] `ori target add/remove` commands
- [ ] `ori demangle` command
- [ ] `ori run --compile` mode
- [ ] CLI integration tests (12 scenarios)

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
| **Total** | | **~177 scenarios** |

**Exit Criteria:** Native executables and WASM modules can be generated from Ori source with full debug support, optimization levels, and incremental compilation. All test scenarios pass with comprehensive coverage.

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
