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
    status: not-started
  - id: "21B.8"
    title: CLI Integration
    status: not-started
  - id: "21B.9"
    title: Error Handling
    status: not-started
  - id: "21B.10"
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

---

## 21B.7 WebAssembly Backend

- [ ] **Implement**: WASM target configuration
  - [ ] `wasm32-unknown-unknown` (standalone)
  - [ ] `wasm32-wasi` (WASI preview 2)
  - [ ] WASM-specific data layout
  - [ ] Memory import/export
  - [ ] **Rust Tests**: `ori_llvm/src/wasm/target_tests.rs`

- [ ] **Implement**: JavaScript binding generation
  - [ ] `--js-bindings` flag
  - [ ] Generate `<name>.js` glue code
  - [ ] Generate `<name>.d.ts` TypeScript declarations
  - [ ] String marshalling (TextEncoder/TextDecoder)
  - [ ] Heap slab for JsValue handles
  - [ ] **Rust Tests**: `ori_llvm/src/wasm/bindings_tests.rs`

- [ ] **Implement**: WASI support
  - [ ] WASI import declarations
  - [ ] File system shims
  - [ ] Clock/random shims
  - [ ] **Rust Tests**: `ori_llvm/src/wasm/wasi_tests.rs`

- [ ] **Implement**: WASM optimization
  - [ ] `--opt=z` for smallest size
  - [ ] `--wasm-opt` post-processor integration
  - [ ] Tree-shaking for glue code
  - [ ] **Rust Tests**: `ori_llvm/src/wasm/opt_tests.rs`

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

---

## 21B.10 Section Completion Checklist

**Target Configuration:**
- [ ] Target triple parsing and validation
- [ ] Data layout configuration
- [ ] CPU feature detection
- [ ] Native target auto-detection

**Object Emission:**
- [ ] ELF, Mach-O, COFF, WASM output
- [ ] Symbol mangling scheme
- [ ] `ori demangle` command

**Debug Information:**
- [ ] DWARF 4 emission
- [ ] dSYM bundle (macOS)
- [ ] CodeView/PDB (Windows)
- [ ] Source location tracking

**Optimization:**
- [ ] O0-O3, Os, Oz levels
- [ ] Thin LTO and Full LTO
- [ ] Pass manager configuration

**Linking:**
- [ ] System linker driver (cc/clang/link.exe)
- [ ] LLD support
- [ ] Runtime library (libori_rt)
- [ ] Static and dynamic linking

**Incremental:**
- [ ] Source hashing
- [ ] Dependency tracking
- [ ] Cache management
- [ ] Parallel compilation

**WebAssembly:**
- [ ] wasm32-unknown-unknown target
- [ ] wasm32-wasi target
- [ ] JavaScript binding generation
- [ ] TypeScript declarations

**CLI:**
- [ ] `ori build` command
- [ ] `ori targets` command
- [ ] `ori target add/remove` commands
- [ ] `ori demangle` command
- [ ] `ori run --compile` mode

**Exit Criteria:** Native executables and WASM modules can be generated from Ori source with full debug support, optimization levels, and incremental compilation.

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
