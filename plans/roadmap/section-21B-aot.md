---
section: "21B"
title: AOT Compilation
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 8
goal: Generate native executables and WebAssembly from Ori source code
sections:
  - id: "21B.1"
    title: Target Configuration
    status: in-progress
  - id: "21B.2"
    title: Object File Emission
    status: in-progress
  - id: "21B.3"
    title: Debug Information
    status: in-progress
  - id: "21B.4"
    title: Optimization Pipeline
    status: in-progress
  - id: "21B.5"
    title: Linking
    status: in-progress
  - id: "21B.6"
    title: Incremental Compilation
    status: in-progress
  - id: "21B.7"
    title: WebAssembly Backend
    status: in-progress
  - id: "21B.8"
    title: CLI Integration
    status: in-progress
  - id: "21B.8.5"
    title: Multi-File Compilation
    status: in-progress
  - id: "21B.9"
    title: Error Handling
    status: not-started
  - id: "21B.10"
    title: End-to-End Pipeline Tests
    status: not-started
  - id: "21B.11"
    title: Performance & Stress Tests
    status: in-progress
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

**Proposal:** `proposals/approved/aot-compilation-proposal.md`
**Depends on:** Section 21A (LLVM Backend - JIT working)

---

## 21B.1 Target Configuration

- [x] **Implement**: Target triple parsing and validation (verified 2026-03-29)
  - [x] Parse `<arch>-<vendor>-<os>[-<env>]` format
  - [x] Validate against supported targets list
  - [x] Native target auto-detection
  - [x] **Rust Tests**: `ori_llvm/tests/aot/cross.rs` + `codegen::targets` (42 tests total)

- [x] **Implement**: Data layout configuration (verified 2026-03-29)
  - [x] LLVM data layout string per target
  - [x] Pointer size, alignment, endianness
  - [x] Module configuration with target triple and data layout
  - [x] **Rust Tests**: `ori_llvm/tests/aot/cross.rs`

- [x] **Implement**: CPU feature detection (verified 2026-03-29)
  - [x] `--cpu=native` auto-detection (`with_cpu_native()`)
  - [x] `--features=+avx2,-sse4` parsing
  - [x] Host CPU feature detection (`get_host_cpu_features()`)
  - [x] **Rust Tests**: `ori_llvm/tests/aot/cross.rs`

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

- [ ] **Subsection close-out (21B.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.2 Object File Emission

- [x] **Implement**: LLVM TargetMachine creation (verified 2026-03-29)
  - [x] Configure target triple, CPU, features
  - [x] Set relocation model (pic, static)
  - [x] Set code model (small, medium, large)
  - [x] **Rust Tests**: covered by target configuration tests (42 tests in 21B.1)

- [x] **Implement**: Object file writing (verified 2026-03-29)
  - [x] ELF output (Linux)
  - [x] Mach-O output (macOS)
  - [x] COFF output (Windows)
  - [x] WASM output (WebAssembly)
  - [x] **Rust Tests**: `ori_llvm/tests/aot/cli.rs` (emit tests); no dedicated object.rs unit tests -- NEEDS TESTS

- [x] **Implement**: Symbol mangling — `_ori_<module>$<function>` scheme in `ori_llvm/src/aot/mangle/` (verified 2026-03-29)
  - [x] `_ori_<module>_<function>` scheme
  - [x] Type suffixes for overloads (generic mangling)
  - [x] Trait method mangling
  - [x] Demangle function for `ori demangle` command
  - [x] **Rust Tests**: 29 tests across `ori_llvm/tests/aot/` + `codegen::mangling` + `oric::demangle`

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

- [ ] **Subsection close-out (21B.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.3 Debug Information

- [x] **Implement**: DIBuilder integration (verified 2026-03-29)
  - [x] Create debug compilation unit
  - [x] Create debug files and directories
  - [x] Set producer metadata
  - [x] **Rust Tests**: `ori_llvm/tests/aot/debug/tests.rs` (10 tests total across all debug subsections)

- [x] **Implement**: Source location tracking (verified 2026-03-29)
  - [x] DILocation for each expression
  - [x] Line/column mapping from spans (LineMap)
  - [x] Scope hierarchy (file, function, block)
  - [x] **Rust Tests**: included in debug/tests.rs above

- [x] **Implement**: Type debug info (verified 2026-03-29)
  - [x] Primitive type debug info
  - [x] Struct type debug info
  - [x] Enum/sum type debug info
  - [x] Generic type debug info (Option, Result, List)
  - [x] **Rust Tests**: included in debug/tests.rs above

- [x] **Implement**: Debug format emission (verified 2026-03-29)
  - [x] DWARF 4 (Linux, macOS, WASM)
  - [x] dSYM bundle configuration (macOS)
  - [x] CodeView/PDB configuration (Windows)
  - [x] Debug levels: none, line-tables, full
  - [x] **Rust Tests**: included in debug/tests.rs above

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

- [ ] **Subsection close-out (21B.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.4 Optimization Pipeline

- [x] **Implement**: Pass manager configuration (verified 2026-03-29)
  - [x] LLVM new pass manager setup (via llvm-sys C API)
  - [x] Module pass pipeline (`LLVMRunPasses` with `default<OX>` strings)
  - [x] Function pass pipeline (via module adapters)
  - [ ] **Rust Tests**: passes/mod.rs (424 lines) and config.rs (403 lines) have ZERO unit tests -- WEAK TESTS, tested only indirectly via LTO and CLI

- [x] **Implement**: Optimization levels — O0/O1/O2/O3/Os/Oz pass pipeline selection in `ori_llvm/src/aot/passes/` (verified 2026-03-29)
  - [x] O0: No optimization (fastest compile)
  - [x] O1: Basic optimization (CSE, SimplifyCFG, DCE)
  - [x] O2: Standard optimization (LICM, GVN, inlining)
  - [x] O3: Aggressive optimization (vectorization, full unrolling)
  - [x] Os: Size optimization
  - [x] Oz: Aggressive size optimization
  - [ ] **Rust Tests**: no direct unit tests -- WEAK TESTS

- [x] **Implement**: LTO support (verified 2026-03-29)
  - [x] Thin LTO (parallel, fast) - `thinlto-pre-link<OX>`, `thinlto<OX>`
  - [x] Full LTO (maximum optimization) - `lto-pre-link<OX>`, `lto<OX>`
  - [x] LTO object emission configuration
  - [x] **Rust Tests**: 17 LTO tests in `ori_llvm/tests/aot/lto.rs` + codegen namespace

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

- [ ] **Subsection close-out (21B.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.5 Linking

- [x] **Implement**: Platform linker driver — `ori_llvm/src/aot/linker/` dispatch to cc/clang/link.exe/lld (verified 2026-03-29)
  - [x] Linux: invoke via `cc` or `ld`
  - [x] macOS: invoke via `clang` or `ld64`
  - [x] Windows: invoke `link.exe` or `lld-link`
  - [x] LLD support (`--linker=lld`)
  - [x] **Rust Tests**: 35 linking integration + 20 WASM linker + 14 syslib = 69 tests total

- [x] **Implement**: Runtime library (libori_rt) (verified 2026-03-29)
  - [x] Consolidate Section 21A runtime functions
  - [x] Memory: `ori_alloc`, `ori_free`, `ori_realloc`
  - [x] Reference counting: `ori_rc_inc`, `ori_rc_dec`, `ori_rc_new`
  - [x] Strings: `ori_str_concat`, `ori_str_from_int`, etc.
  - [x] Collections: `ori_list_new`, `ori_map_new`, etc.
  - [x] Panic: `ori_panic`, `ori_panic_handler`
  - [x] I/O: `ori_print`, `ori_stdin_read`
  - [x] Static linking (default)
  - [x] Dynamic linking (--link=dynamic)
  - [x] **Rust Tests**: `ori_rt` (360 tests); runtime.rs has 0 unit tests -- WEAK TESTS (indirect only via phase test)

- [x] **Implement**: Runtime library discovery (verified 2026-03-29)
  - **Proposal**: `proposals/approved/runtime-library-discovery-proposal.md` APPROVED 2026-02-02
  - [x] Walk up from binary to find `libori_rt.a` (like rustc sysroot) -- 5-strategy binary-relative search
  - [x] Dev layout: same directory as compiler binary
  - [x] Installed layout: `<exe>/../lib/libori_rt.a`
  - [x] Workspace dev: `$ORI_WORKSPACE_DIR/target/{release,debug}/`
  - [ ] CLI override: `--runtime-path` flag (pending CLI integration)
  - [x] Remove environment variables (ORI_LIB_DIR, ORI_RT_PATH) from current implementation -- verified removed
  - [x] **Unblocks**: Multi-file AOT compilation (21B.8.5), End-to-end tests (21B.10)

- [x] **Implement**: System library detection (verified 2026-03-29)
  - [x] Platform-specific library paths
  - [x] Sysroot support for cross-compilation
  - [x] Library search order
  - [x] **Rust Tests**: `ori_llvm/src/aot/syslib/` (14 tests)

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

- [ ] **Subsection close-out (21B.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.6 Incremental Compilation

- [x] **Implement**: Source hashing — content-based change detection in `ori_llvm/src/aot/incremental/hash.rs` (verified 2026-03-29)
  - [x] Content hash per source file (FxHash algorithm)
  - [x] Store hashes in `build/cache/`
  - [x] Detect hash mismatches
  - [x] **Rust Tests**: 14 tests

- [x] **Implement**: Dependency tracking — import graph for incremental invalidation in `ori_llvm/src/aot/incremental/deps.rs` (verified 2026-03-29)
  - [x] Import graph analysis
  - [x] Transitive dependency detection
  - [x] Topological ordering for compilation
  - [x] **Rust Tests**: 12 tests

- [x] **Implement**: Compilation cache management — validation/hit/miss/parallel access in `ori_llvm/src/aot/incremental/cache.rs` (verified 2026-03-29)
  - [x] Cache validation (source + deps + flags + version)
  - [x] Cache hit: skip recompilation
  - [x] Cache miss: recompile and update cache
  - [x] Parallel cache access
  - [x] **Rust Tests**: 11 tests

- [x] **Implement**: Parallel compilation — thread pool for multi-module builds in `ori_llvm/src/aot/incremental/parallel.rs` (verified 2026-03-29)
  - [x] `--jobs=N` flag
  - [x] Auto-detect core count (`--jobs=auto`)
  - [x] Thread pool for module compilation
  - [x] **Rust Tests**: 20 tests

- [ ] **Integrate**: Wire up cache to `ori build` command -- GAP-1 CRITICAL: infrastructure complete (77 tests, 7 submodules, ~2500 lines) but NOT wired into `ori build`
  - [ ] Add cache lookup before compilation in `build_file()`
  - [ ] Store compiled objects in cache after successful build
  - [ ] Add `--no-cache` flag to bypass incremental compilation
  - [ ] Add verbose output for cache hits/misses
  - [ ] **Blocks**: 21B.8 incremental test (`test_build_incremental_unchanged` is `#[ignore]`), 21B.8.5.4 cache integration

- [ ] **Test**: Incremental compilation advanced (MEDIUM priority)
  - [ ] Source hash computation
  - [ ] Dependency graph tracking
  - [ ] Cache key generation (source + deps + flags + version)
  - [ ] Cache hit detection (skip recompile)
  - [ ] Cache invalidation on change
  - [ ] Parallel compilation (`-j` flag)
  - [ ] Incremental debug info
  - [ ] Incremental metadata

- [ ] **Subsection close-out (21B.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.7 WebAssembly Backend

- [x] **Implement**: WASM target configuration (verified 2026-03-29) -- STALE STATUS corrected from not-started
  - [x] `wasm32-unknown-unknown` (standalone)
  - [x] `wasm32-wasi` (WASI preview 2)
  - [x] WASM-specific data layout
  - [x] Memory import/export
  - [x] **Rust Tests**: 73 WASM tests (44 integration + 20 linker unit + 6 cross-compilation + 3 others)

- [x] **Implement**: JavaScript binding generation (verified 2026-03-29)
  - [x] `--js-bindings` flag support via `WasmConfig`
  - [x] Generate `<name>.js` glue code
  - [x] Generate `<name>.d.ts` TypeScript declarations
  - [x] String marshalling (TextEncoder/TextDecoder)
  - [x] Heap slab for JsValue handles
  - [x] **Rust Tests**: included in WASM tests above

- [x] **Implement**: WASI support (verified 2026-03-29)
  - [x] WASI import declarations (`WasiConfig::undefined_symbols()`)
  - [x] File system configuration
  - [x] Clock/random shim configuration
  - [x] **Rust Tests**: included in WASM tests above

- [x] **Implement**: WASM optimization (verified 2026-03-29)
  - [x] `--opt=z` for smallest size (`WasmOptLevel::Oz`)
  - [x] `--wasm-opt` post-processor integration (`WasmOptRunner`)
  - [x] Tree-shaking support via wasm-opt
  - [x] **Rust Tests**: included in WASM tests above

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

- [ ] **Subsection close-out (21B.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.8 CLI Integration

- [x] **Implement**: `ori build` command (verified 2026-03-29)
  - [x] Parse all flags (--release, --target, --opt, etc.)
  - [x] Output path handling (-o, --out-dir)
  - [x] Emit mode (--emit=obj, llvm-ir, llvm-bc, asm)
  - [x] Library modes (--lib, --dylib)
  - [x] Verbose output (-v)
  - [x] **Rust Tests**: 53 build tests (11 build + 42 build_options)
  - [x] **CLI Tests**: `ori_llvm/tests/aot/cli.rs` (42 tests, 1 ignored for incremental)

- [x] **Implement**: `ori targets` command (verified 2026-03-29)
  - [x] List all supported targets
  - [x] `--installed` flag for targets with sysroots
  - [x] **Rust Tests**: `oric/src/commands/targets/` (9 tests)

- [x] **Implement**: `ori target` command (cross-compilation) (verified 2026-03-29)
  - [x] `ori target add <target>` - download sysroot
  - [x] `ori target remove <target>` - remove sysroot
  - [x] `ori target list` - list installed targets
  - [x] Sysroot management
  - [x] **Rust Tests**: `oric/src/commands/target.rs` (7 tests)

- [x] **Implement**: `ori demangle` command (verified 2026-03-29)
  - [x] Parse mangled symbol names
  - [x] Output demangled Ori names
  - [x] **Rust Tests**: `oric/src/commands/demangle/` (9 tests)

- [x] **Implement**: `ori run --compile` mode (verified 2026-03-29)
  - [x] AOT compile then execute
  - [x] Faster repeated runs
  - [x] Cache compiled binary (hash-based in ~/.cache/ori/compiled/)
  - [x] **Rust Tests**: `oric/src/commands/run/` (5 tests)

- [x] **Test**: CLI integration (42 tests in `ori_llvm/tests/aot/cli.rs`, 1 ignored for incremental) (verified 2026-03-29)
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

- [ ] **Subsection close-out (21B.8)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.8 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.8: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.8.5 Multi-File Compilation

**Proposal:** `proposals/approved/multi-file-aot-proposal.md`

Enable AOT compilation of Ori programs with imports. Currently, `ori build` produces broken binaries when the source uses `use` statements.

### 21B.8.5.1 Dependency Graph Infrastructure

- [x] **Implement**: `build_dependency_graph()` in `ori_llvm/src/aot/multi_file/mod.rs` (verified 2026-03-29) -- STALE STATUS corrected from not-started
  - [x] Build import graph from entry file using import extraction
  - [x] Handle relative imports (`./helper`, `../utils`)
  - [x] Handle directory modules (`./http` → `http/mod.ori`)
  - [x] Handle stdlib imports (`std.math` via `ORI_STDLIB`)
  - [x] **Rust Tests**: 15 unit tests

- [x] **Implement**: Topological sorting for compilation order (verified 2026-03-29)
  - [x] Sort modules so dependencies compile before dependents (reuses `DependencyGraph::topological_order()`)
  - [x] Integrate with cycle detection via `GraphBuildContext`
  - [x] **Rust Tests**: included in 15 unit tests above

### 21B.8.5.2 Per-Module Compilation

- [x] **Implement**: Per-module compilation in `build_file_multi()` (verified 2026-03-29)
  - [x] Compile single module to object file
  - [x] Use module-qualified name mangling (`_ori_<module>$<function>`)
  - [x] Generate `declare` for imported symbols via `declare_external_fn_mangled()`
  - [x] **Rust Tests**: `ori_llvm/src/declare.rs`

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

- [ ] **Subsection close-out (21B.8.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.8.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.8.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

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

- [ ] **Subsection close-out (21B.9)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.9 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.9: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.10 End-to-End Pipeline Tests

> Proposed test infrastructure (formal `AotTestExecutor`, `--backend=aot`) is not started. However, 1977 AOT integration tests exercise the full parse-typeck-codegen-link-execute pipeline. `diagnostics/dual-exec-verify.sh` exists for interpreter vs AOT comparison. (verified 2026-03-29)

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
  - [ ] **Rust Tests**: `oric/src/test/runner/tests.rs`

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
  - [ ] Compare results: Evaluator vs JIT vs AOT
  - [ ] Document any backend-specific differences

- [ ] **Subsection close-out (21B.10)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.10 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.10: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.11 Performance & Stress Tests

> STALE STATUS corrected from not-started. `compiler/ori_llvm/tests/aot/stress.rs` has 34 tests; `memory_stress.rs` also exists. (verified 2026-03-29)

- [ ] **Test**: Performance benchmarks
  - [ ] Compile large module (10K+ lines)
  - [ ] Compile many small modules (100+ files)
  - [ ] Parallel compilation scaling (1, 2, 4, 8 cores)
  - [ ] Memory usage under large compilation
  - [ ] Incremental rebuild time (small change)
  - [ ] Full rebuild time (clean build)
  - [ ] LTO compilation time
  - [ ] Debug build vs release build time

- [ ] **Subsection close-out (21B.11)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.11 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.11: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

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

- [ ] **Subsection close-out (21B.12)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.12 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.12: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

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

- [ ] **Subsection close-out (21B.13)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.13 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.13: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

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

- [ ] **Subsection close-out (21B.14)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.14 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.14: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

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

- [ ] **Implement**: AOT test infrastructure — parameterized tests, platform skip, multi-language overlays
  - [ ] Parameterized tests with Options
  - [ ] Platform skip directives
  - [ ] Overlay system for multi-language tests

- [ ] **Subsection close-out (21B.15)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.15 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.15: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21B.16 Section Completion Checklist

**Target Configuration (21B.1):** (verified 2026-03-29)
- [x] Target triple parsing and validation
- [x] Data layout configuration
- [x] CPU feature detection
- [x] Native target auto-detection

**Object Emission (21B.2):** (verified 2026-03-29 -- core emission done, verification tests not started)
- [x] ELF, Mach-O, COFF, WASM output
- [x] Symbol mangling scheme
- [x] `ori demangle` command (with tests)
- [ ] Object file verification tests (10 scenarios)
- [ ] Symbol management tests (9 scenarios)

**Debug Information (21B.3):** (verified 2026-03-29 -- implementation done, 10 tests; verification tests not started)
- [x] DWARF 4 emission
- [x] dSYM bundle (macOS)
- [x] CodeView/PDB (Windows)
- [x] Source location tracking
- [ ] Debug info verification tests (9 scenarios)

**Optimization (21B.4):** (verified 2026-03-29 -- implementation done; pass manager has ZERO unit tests)
- [x] O0-O3, Os, Oz levels
- [x] Thin LTO and Full LTO
- [x] Pass manager configuration -- WEAK TESTS (0 direct unit tests, 827 lines untested)
- [ ] LTO advanced tests (7 scenarios)
- [ ] Code model tests (8 scenarios)

**Linking (21B.5):** (verified 2026-03-29 -- core linking done, 69 tests)
- [x] System linker driver (cc/clang/link.exe)
- [x] LLD support
- [x] Runtime library (libori_rt)
- [x] Static and dynamic linking
- [x] Runtime library discovery (binary-relative, like rustc sysroot)
- [ ] Linker error handling tests (8 scenarios)
- [ ] Linker feature tests (9 scenarios)

**Incremental (21B.6):** (verified 2026-03-29 -- infrastructure done with 77 tests; GAP-1 CRITICAL: not wired to `ori build`)
- [x] Source hashing
- [x] Dependency tracking
- [x] Cache management
- [x] Parallel compilation
- [ ] Wire up cache to `ori build` command (blocks 21B.8 incremental test) -- GAP-1 CRITICAL
- [ ] Incremental advanced tests (8 scenarios)

**WebAssembly (21B.7):** (verified 2026-03-29 -- substantially implemented, 73 tests)
- [x] wasm32-unknown-unknown target
- [x] wasm32-wasi target
- [x] JavaScript binding generation
- [x] TypeScript declarations
- [ ] WASM advanced tests (10 scenarios)

**CLI (21B.8):** (verified 2026-03-29 -- all commands implemented, 125 tests total)
- [x] `ori build` command (with tests)
- [x] `ori targets` command (with tests)
- [x] `ori target add/remove` commands (with tests)
- [x] `ori demangle` command (with tests)
- [x] `ori run --compile` mode (with tests)
- [x] CLI integration tests (42 end-to-end tests)
- [ ] Build incremental test (blocked on 21B.6 integration)

**Multi-File Compilation (21B.8.5):** (verified 2026-03-29 -- infrastructure done, 15 tests)
- [x] Dependency graph infrastructure
- [x] Per-module compilation with name mangling
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

**End-to-End Pipeline (21B.10):** (verified 2026-03-29 -- proposed infrastructure not started; 1977 AOT integration tests cover pipeline functionally)
- [ ] End-to-end execution tests (12 scenarios)

**Performance & Stress (21B.11):** (verified 2026-03-29 -- 34 stress tests exist in stress.rs + memory_stress.rs)
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
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Test Coverage Summary:** (verified 2026-03-29)
| Priority | Category | Plan Scenarios | Actual Tests | Status |
|----------|----------|---------------|-------------|--------|
| CRITICAL | CLI Integration (21B.8) | 12 | 125 | [done] |
| CRITICAL | Multi-File Compilation (21B.8.5) | 13 | 15 | [partial] |
| CRITICAL | Error Handling (21B.9) | 20 | 0 | [todo] |
| CRITICAL | End-to-End Pipeline (21B.10) | 12 | 0 (proposed infra) | [todo] |
| CRITICAL | Performance/Stress (21B.11) | 8 | 34 | [partial] |
| HIGH | Linker Tests (21B.5) | 17 | 69 | [done] |
| HIGH | Object File Tests (21B.2) | 19 | 0 | [todo] |
| MEDIUM | Platform-Specific (21B.12) | 25 | 0 | [todo] |
| MEDIUM | WASM Advanced (21B.7) | 10 | 0 (73 core) | [todo] |
| MEDIUM | LTO Advanced (21B.4) | 15 | 0 (17 core) | [todo] |
| MEDIUM | Incremental (21B.6) | 8 | 0 (77 infra) | [todo] |
| MEDIUM | Debug Info (21B.3) | 9 | 0 (10 core) | [todo] |
| LOW | ABI/FFI (21B.13) | 15 | 0 | [todo] |
| LOW | Architecture (21B.14) | 7 | 0 | [todo] |
| **Total** | | **~190 scenarios** | **400+ actual** | |

**Exit Criteria:** Native executables and WASM modules can be generated from Ori source with full debug support, optimization levels, incremental compilation, and multi-file import support. All test scenarios pass with comprehensive coverage.

- [ ] **Subsection close-out (21B.16)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21B.16 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21B.16: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## LLVM Version Requirement

**Required:** LLVM 21 or later

Rationale:
- Best WASM support with Component Model preview
- Newest pass manager (default since LLVM 14)
- Improved debug info generation
- No legacy compatibility burden

---

## Running Tests

```bash
# Run all AOT tests (1977 tests)
cargo test -p ori_llvm

# Run AOT-specific tests
cargo test -p ori_llvm --lib aot

# Run WASM-specific tests
cargo test -p ori_llvm --lib wasm

# Build and run an executable
ori build src/main.ori -o myapp && ./myapp

# Build for WASM
ori build --wasm src/main.ori -o myapp.wasm
```
