# Phase 21B: AOT Compilation

**Status:** 📋 Planned
**Proposal:** `proposals/approved/aot-compilation-proposal.md`
**Depends on:** Phase 21A (LLVM Backend - JIT working)

**Goal:** Generate native executables and WebAssembly from Ori source code.

---

## 21B.1 Target Configuration

- [ ] **Implement**: Target triple parsing and validation
  - [ ] Parse `<arch>-<vendor>-<os>[-<env>]` format
  - [ ] Validate against supported targets list
  - [ ] Native target auto-detection
  - [ ] **Rust Tests**: `ori_llvm/src/aot/target_tests.rs`

- [ ] **Implement**: Data layout configuration
  - [ ] LLVM data layout string per target
  - [ ] Pointer size, alignment, endianness
  - [ ] **Rust Tests**: `ori_llvm/src/aot/layout_tests.rs`

- [ ] **Implement**: CPU feature detection
  - [ ] `--cpu=native` auto-detection
  - [ ] `--features=+avx2,-sse4` parsing
  - [ ] Target feature validation
  - [ ] **Rust Tests**: `ori_llvm/src/aot/features_tests.rs`

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
  - [ ] **Rust Tests**: `ori_llvm/src/aot/machine_tests.rs`

- [ ] **Implement**: Object file writing
  - [ ] ELF output (Linux)
  - [ ] Mach-O output (macOS)
  - [ ] COFF output (Windows)
  - [ ] WASM output (WebAssembly)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/object_tests.rs`

- [ ] **Implement**: Symbol mangling
  - [ ] `_ori_<module>_<function>` scheme
  - [ ] Type suffixes for overloads
  - [ ] Trait method mangling
  - [ ] `ori demangle` command
  - [ ] **Rust Tests**: `ori_llvm/src/aot/mangle_tests.rs`

---

## 21B.3 Debug Information

- [ ] **Implement**: DIBuilder integration
  - [ ] Create debug compilation unit
  - [ ] Create debug files and directories
  - [ ] Set producer metadata
  - [ ] **Rust Tests**: `ori_llvm/src/aot/debug_tests.rs`

- [ ] **Implement**: Source location tracking
  - [ ] DILocation for each expression
  - [ ] Line/column mapping from spans
  - [ ] Scope hierarchy (file, function, block)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/location_tests.rs`

- [ ] **Implement**: Type debug info
  - [ ] Primitive type debug info
  - [ ] Struct type debug info
  - [ ] Enum/sum type debug info
  - [ ] Generic type debug info
  - [ ] **Rust Tests**: `ori_llvm/src/aot/type_debug_tests.rs`

- [ ] **Implement**: Debug format emission
  - [ ] DWARF 4 (Linux, macOS, WASM)
  - [ ] dSYM bundle (macOS, default)
  - [ ] CodeView/PDB (Windows)
  - [ ] Debug levels: none, line-tables, full
  - [ ] **Rust Tests**: `ori_llvm/src/aot/format_tests.rs`

---

## 21B.4 Optimization Pipeline

- [ ] **Implement**: Pass manager configuration
  - [ ] LLVM new pass manager setup
  - [ ] Module pass pipeline
  - [ ] Function pass pipeline
  - [ ] **Rust Tests**: `ori_llvm/src/aot/passes_tests.rs`

- [ ] **Implement**: Optimization levels
  - [ ] O0: No optimization (fastest compile)
  - [ ] O1: Basic optimization (CSE, SimplifyCFG, DCE)
  - [ ] O2: Standard optimization (LICM, GVN, inlining)
  - [ ] O3: Aggressive optimization (vectorization, full unrolling)
  - [ ] Os: Size optimization
  - [ ] Oz: Aggressive size optimization
  - [ ] **Rust Tests**: `ori_llvm/src/aot/opt_level_tests.rs`

- [ ] **Implement**: LTO support
  - [ ] Thin LTO (parallel, fast)
  - [ ] Full LTO (maximum optimization)
  - [ ] LTO object emission
  - [ ] **Rust Tests**: `ori_llvm/src/aot/lto_tests.rs`

---

## 21B.5 Linking

- [ ] **Implement**: Linker driver
  - [ ] Linux: invoke via `cc` or `ld`
  - [ ] macOS: invoke via `clang` or `ld64`
  - [ ] Windows: invoke `link.exe` or `lld-link`
  - [ ] LLD support (`--linker=lld`)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/linker_tests.rs`

- [ ] **Implement**: Runtime library (libori_rt)
  - [ ] Consolidate Phase 21A runtime functions
  - [ ] Memory: `ori_alloc`, `ori_free`, `ori_realloc`
  - [ ] Reference counting: `ori_rc_inc`, `ori_rc_dec`, `ori_rc_new`
  - [ ] Strings: `ori_str_concat`, `ori_str_from_int`, etc.
  - [ ] Collections: `ori_list_new`, `ori_map_new`, etc.
  - [ ] Panic: `ori_panic`, `ori_panic_handler`
  - [ ] I/O: `ori_print`, `ori_stdin_read`
  - [ ] Static linking (default)
  - [ ] Dynamic linking (--link=dynamic)
  - [ ] **Rust Tests**: `ori_llvm/src/aot/runtime_tests.rs`

- [ ] **Implement**: System library detection
  - [ ] Platform-specific library paths
  - [ ] Sysroot support for cross-compilation
  - [ ] Library search order
  - [ ] **Rust Tests**: `ori_llvm/src/aot/syslib_tests.rs`

---

## 21B.6 Incremental Compilation

- [ ] **Implement**: Source hashing
  - [ ] Content hash per source file
  - [ ] Store hashes in `build/cache/`
  - [ ] Detect hash mismatches
  - [ ] **Rust Tests**: `ori_llvm/src/aot/hash_tests.rs`

- [ ] **Implement**: Dependency tracking
  - [ ] Import graph analysis
  - [ ] Transitive dependency detection
  - [ ] Store deps in `build/cache/deps/`
  - [ ] **Rust Tests**: `ori_llvm/src/aot/deps_tests.rs`

- [ ] **Implement**: Cache management
  - [ ] Cache validation (source + deps + flags + version)
  - [ ] Cache hit: skip recompilation
  - [ ] Cache miss: recompile and update cache
  - [ ] Parallel cache access
  - [ ] **Rust Tests**: `ori_llvm/src/aot/cache_tests.rs`

- [ ] **Implement**: Parallel compilation
  - [ ] `--jobs=N` flag
  - [ ] Auto-detect core count (`--jobs=auto`)
  - [ ] Thread pool for module compilation
  - [ ] **Rust Tests**: `ori_llvm/src/aot/parallel_tests.rs`

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

## 21B.10 Phase Completion Checklist

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
