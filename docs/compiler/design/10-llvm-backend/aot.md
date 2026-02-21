---
title: "AOT Compilation"
description: "Ahead-of-time compilation to native executables and WebAssembly"
order: 4
section: "LLVM Backend"
---

# AOT Compilation

The AOT (Ahead-of-Time) compilation system generates native executables and WebAssembly modules from Ori source code. It extends the JIT infrastructure with target configuration, object file emission, linking, and multi-file compilation.

## Architecture

The AOT pipeline transforms typed AST to executable binaries:

```
┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
│  Parse  │───▶│  Type   │───▶│ Canon-  │───▶│   ARC   │───▶│  LLVM   │───▶│ Object  │───▶│  Link   │
│  (AST)  │    │  Check  │    │  alize  │    │Pipeline │    │   IR    │    │  File   │    │         │
└─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘    └────┬────┘
                                                                                                │
                                                                            ┌─────────────────▼─────┐
                                                                            │  Executable / Library │
                                                                            │  .exe / .so / .wasm   │
                                                                            └───────────────────────┘
```

The `check_source()` function in `compile_common.rs` returns `Option<(ParseOutput, TypeCheckResult, Pool, CanonResult)>` — canonicalization is part of the front-end pipeline, shared between check and compile paths. Returns `None` if any errors occurred.

### Key Components

| Component | Module | Purpose |
|-----------|--------|---------|
| Target Configuration | `target.rs` | Target triple, CPU features, data layout |
| Object Emission | `object.rs` | Emit LLVM IR as ELF/Mach-O/COFF/WASM |
| Symbol Mangling | `mangle.rs` | Generate unique linker symbols |
| Debug Information | `debug.rs` | DWARF/CodeView generation |
| Optimization | `passes.rs` | LLVM new pass manager |
| Linker Driver | `linker/` | Platform-agnostic linking |
| Runtime Library | `runtime.rs` | Runtime discovery and linking |
| Multi-File | `multi_file.rs` | Dependency graph and compilation |
| WebAssembly | `wasm.rs` | WASM-specific configuration |

## Target Configuration

The `TargetConfig` struct represents a compilation target:

```rust
let target = TargetConfig::native()?;              // Host system
let target = TargetConfig::from_triple("aarch64-apple-darwin")?
    .with_cpu("apple-m1")
    .with_features("+neon,+fp-armv8");
```

### Supported Targets

| Target Triple | Description |
|---------------|-------------|
| `x86_64-unknown-linux-gnu` | 64-bit Linux (glibc) |
| `x86_64-unknown-linux-musl` | 64-bit Linux (musl, static) |
| `x86_64-apple-darwin` | 64-bit macOS (Intel) |
| `aarch64-apple-darwin` | 64-bit macOS (Apple Silicon) |
| `x86_64-pc-windows-msvc` | 64-bit Windows (MSVC) |
| `x86_64-pc-windows-gnu` | 64-bit Windows (MinGW) |
| `wasm32-unknown-unknown` | WebAssembly (standalone) |
| `wasm32-wasi` | WebAssembly (WASI) |

## Symbol Mangling

The mangling scheme ensures unique, linkable names across platforms:

```
_ori_[<module>$]<function>[<suffix>]
```

Module paths are encoded by `encode_module_path`, which replaces `/`, `\`, `.`, and `:` characters with `$` (the module separator). Alphanumeric characters and `_` pass through unchanged; other characters are hex-escaped as `$XX`.

### Mangling Examples

| Ori Symbol | Mangled Name |
|------------|--------------|
| `@main` (root) | `_ori_main` |
| `math.@add` | `_ori_math$add` |
| `http/client.@connect` | `_ori_http$client$connect` |
| `int::Eq.@equals` | `_ori_int$$Eq$equals` |
| `Option.@some` | `_ori_Option$A$some` |
| `identity<int>` | `_ori_identity$Gint` |
| `[int]` extension `sum` | `_ori_$LBint$RB$$ext$sum` |
| `str` extension `to_upper` in `string_utils` | `_ori_str$$ext$string_utils$to_upper` |

### Mangler API

The `Mangler` struct provides methods for generating mangled names:

```rust
let mangler = Mangler::new();

// Simple function
mangler.mangle_function("math", "add")      // "_ori_math$add"

// Trait implementation
mangler.mangle_trait_impl("int", "Eq", "equals")  // "_ori_int$$Eq$equals"

// Extension method
mangler.mangle_extension("[int]", "sum", "")  // "_ori_$LBint$RB$$ext$sum"

// Generic instantiation
mangler.mangle_generic("", "identity", &["int"])  // "_ori_identity$Gint"

// Associated function
mangler.mangle_associated_function("Option", "some")  // "_ori_Option$A$some"

// Impl method (type_name.method_name)
mangler.mangle_method("", "Point", "distance")  // "_ori_Point$distance"
```

### Demangling

The `demangle()` function converts mangled symbols back to Ori-style names:

| Mangled | Demangled |
|---------|-----------|
| `_ori_main` | `@main` |
| `_ori_math$add` | `math.@add` |
| `_ori_http$client$connect` | `http/client.@connect` |
| `_ori_int$$Eq$equals` | `int::Eq.@equals` |
| `_ori_Option$A$some` | `Option.@some` |
| `_ori_identity$Gint` | `@identity<int>` |

The `ori demangle` CLI command provides demangling for debugging and diagnostics.

### Escape Sequences

| Character | Encoded |
|-----------|---------|
| `<` | `$LT` |
| `>` | `$GT` |
| `[` | `$LB` |
| `]` | `$RB` |
| `(` | `$LP` |
| `)` | `$RP` |
| `,` | `$C` |
| `:` | `$CC` |
| `-` | `$D` |
| ` ` (space) | `_` |
| Other | `$XX` (hex) |

### Special Markers

| Marker | Meaning |
|--------|---------|
| `$$` | Trait implementation separator |
| `$$ext$` | Extension method marker |
| `$A$` | Associated function marker |
| `$G` | Generic instantiation prefix |

## Runtime Library Discovery

The runtime library (`libori_rt.a`) provides memory allocation, reference counting, string operations, and panic handling. Discovery follows the rustc sysroot pattern—walking up from the compiler binary rather than relying on environment variables.

### Discovery Algorithm

1. **Same directory as binary** (dev builds): `<exe_dir>/libori_rt.a`
2. **Installed layout**: `<exe_dir>/../lib/libori_rt.a` (FHS standard)
3. **Workspace fallback**: `$ORI_WORKSPACE_DIR/target/{release,debug}/libori_rt.a`

The `--runtime-path` CLI flag provides explicit override for custom deployments.

### Platform-Specific Names

| Platform | Library Name |
|----------|--------------|
| Linux/macOS | `libori_rt.a` |
| Windows | `ori_rt.lib` |

### Runtime Functions

| Category | Functions |
|----------|-----------|
| Memory | `ori_alloc`, `ori_free`, `ori_realloc` |
| Reference Counting | `ori_rc_alloc`, `ori_rc_free`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_count` |
| Strings | `ori_str_concat`, `ori_str_eq`, `ori_str_ne`, `ori_str_from_int`, `ori_str_from_bool`, `ori_str_from_float`, `ori_str_compare`, `ori_str_hash`, `ori_str_next_char` |
| Collections | `ori_list_new`, `ori_list_free`, `ori_list_len`, `ori_list_alloc_data`, `ori_list_free_data` |
| Panic | `ori_panic`, `ori_panic_cstr`, `ori_register_panic_handler` |
| Assertions | `ori_assert`, `ori_assert_eq_int`, `ori_assert_eq_bool`, `ori_assert_eq_str`, `ori_assert_eq_float` |
| Comparison | `ori_compare_int`, `ori_min_int`, `ori_max_int` |
| I/O | `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool` |
| Entry | `ori_run_main`, `ori_args_from_argv` |

### Runtime Data Structures

The runtime defines C-compatible data structures for interoperation:

```c
// Ori string: { i64 len, *const u8 data }
struct OriStr { int64_t len; const uint8_t* data; }

// Ori list: { i64 len, i64 cap, *mut u8 data }
struct OriList { int64_t len; int64_t cap; uint8_t* data; }

// Reference-counted header: { i64 refcount, i64 size }
struct RcHeader { int64_t refcount; int64_t size; }
```

### Linking Configuration

The `RuntimeConfig` struct configures runtime linking:

```rust
let rt_config = RuntimeConfig::detect()?;
rt_config.configure_link(&mut input);  // Adds library paths and dependencies

// On Unix, automatically links: libc, libm, libpthread
```

## Multi-File Compilation

The multi-file system builds a dependency graph from import statements and compiles modules in topological order.

### Dependency Graph

```rust
let result = build_dependency_graph(entry_file, &config)?;
// result.order: Vec<PathBuf> in compilation order
// result.files: Set of all discovered files
```

### Module Resolution

Relative imports resolve in order:
1. `./http.ori` (file module)
2. `./http/mod.ori` (directory module)

```ori
use "./http" { get }        // Resolves to http.ori or http/mod.ori
use "../utils" { helper }   // Relative path
```

### Module-Qualified Mangling

Each module's functions receive a module-qualified mangled name:

```
main.ori:     _ori_main
helper.ori:   _ori_helper$my_func
http/mod.ori: _ori_http$get
```

## Linking

The `LinkerDriver` abstracts platform-specific linker invocation:

```rust
let driver = LinkerDriver::new(&target);
driver.link(LinkInput {
    objects: vec!["main.o".into(), "helper.o".into()],
    output: "myapp".into(),
    output_kind: LinkOutput::Executable,
    ..Default::default()
})?;
```

### Linker Selection

| Platform | Default Linker | LLD Alternative |
|----------|----------------|-----------------|
| Linux | `cc` (gcc/clang) | `ld.lld` |
| macOS | `clang` | `ld64.lld` |
| Windows | `link.exe` | `lld-link` |
| WASM | `wasm-ld` | `wasm-ld` |

### Library Linking

```rust
input.libraries.push(LinkLibrary::new("ori_rt").static_lib());
input.libraries.push(LinkLibrary::new("c"));
input.libraries.push(LinkLibrary::new("m"));
```

## Optimization Pipeline

The optimization system uses LLVM's new pass manager:

```rust
let config = OptimizationConfig::new(OptimizationLevel::Aggressive);
run_optimization_passes(&module, &target_machine, &config)?;
```

### Optimization Levels

| Level | Flag | Description |
|-------|------|-------------|
| None | `-O0` | No optimization, fastest compile |
| Less | `-O1` | Basic optimization (CSE, DCE) |
| Default | `-O2` | Standard optimization (LICM, GVN, inlining) |
| Aggressive | `-O3` | Full optimization (vectorization, unrolling) |
| Size | `-Os` | Optimize for size |
| MinSize | `-Oz` | Aggressive size optimization |

### LTO Modes

| Mode | Description |
|------|-------------|
| None | No link-time optimization |
| Thin | Parallel, fast LTO |
| Full | Maximum optimization, slower |

## Debug Information

The `DebugInfoBuilder` generates DWARF (Linux/macOS) or CodeView (Windows) debug information:

```rust
let debug = DebugInfoBuilder::new(&module, &context, config, "main.ori", "src");
// ... compile code with location tracking ...
debug.finalize();
```

### Debug Levels

| Level | Information Included |
|-------|---------------------|
| None | No debug info |
| LineTablesOnly | Line numbers only |
| Full | Full debug info (types, variables) |

## WebAssembly

WASM compilation supports both standalone and WASI targets:

```rust
let wasm_config = WasmConfig::new()
    .with_memory(WasmMemoryConfig { initial: 16, maximum: Some(256) })
    .with_js_bindings(true);
```

### JavaScript Bindings

When enabled, generates `<name>.js` glue code and `<name>.d.ts` TypeScript declarations for browser integration.

### WASI Configuration

```rust
let wasi = WasiConfig::preview2()
    .with_preopen("/data", "data")
    .with_env("DEBUG", "1");
```

## CLI Build Command

The `ori build` command provides AOT compilation with comprehensive options:

### Basic Usage

```bash
ori build main.ori              # Debug build to build/debug/main
ori build --release main.ori    # Release build to build/release/main
ori build -o myapp main.ori     # Custom output path
```

### Build Options

| Flag | Description |
|------|-------------|
| `--release` | Optimize with O2, no debug info |
| `--target=<triple>` | Cross-compile target |
| `--opt=<level>` | Optimization: 0, 1, 2, 3, s, z |
| `--debug=<level>` | Debug info: 0 (none), 1 (lines), 2 (full) |
| `-o=<path>` | Output file path |
| `--out-dir=<dir>` | Output directory |
| `--emit=<type>` | Emit: obj, llvm-ir, llvm-bc, asm |
| `--lib` | Build static library |
| `--dylib` | Build shared library |
| `--wasm` | Build WebAssembly |
| `--linker=<name>` | Linker: system, lld, msvc |
| `--link=<mode>` | Link mode: static, dynamic |
| `--lto=<mode>` | LTO: off, thin, full |
| `--jobs=<n>` | Parallel compilation jobs |
| `--cpu=<name>` | Target CPU (e.g., native, haswell) |
| `--features=<list>` | CPU features (e.g., +avx2,-sse4) |
| `--js-bindings` | Generate JS bindings for WASM |
| `--wasm-opt` | Run wasm-opt post-processor |
| `-v, --verbose` | Verbose output |

### Output Organization

```
build/
├── debug/           # Debug builds (default)
│   └── main
└── release/         # Release builds (--release)
    └── main
```

### Multi-File Compilation

When source files contain imports (`use "./..."` or `use "../..."`), the build command automatically:

1. Builds a dependency graph from import statements
2. Resolves modules in topological order
3. Compiles each module with proper import declarations
4. Links all object files into the final executable

```bash
# Automatically handles imports
ori build --release main.ori    # Compiles main.ori and all dependencies
```

## Incremental Compilation

The incremental system caches compiled objects and tracks dependencies:

```
build/cache/
├── <hash1>.o        # Cached object files
├── <hash2>.o
└── manifest.json    # Dependency and hash tracking
```

### Cache Key Components

- Source file content hash
- Import signatures
- Compiler flags
- Compiler version

### Parallel Compilation

```bash
ori build --jobs=4 main.ori    # 4 parallel compilations
ori build --jobs=auto main.ori # Auto-detect core count
```

## Source Files

### AOT Module (`aot/`)

| File | Purpose |
|------|---------|
| `mod.rs` | AOT module re-exports and organization |
| `target.rs` | Target triple parsing, CPU/feature detection |
| `object.rs` | Object file emission (ELF/Mach-O/COFF/WASM) |
| `mangle.rs` | Symbol mangling (`Mangler`) and demangling |
| `debug/` | DWARF/CodeView debug info generation (7 files: builder, builder_scope, config, context, line_map, mod, tests) |
| `passes.rs` | LLVM new pass manager optimization pipeline |
| `runtime.rs` | Runtime library discovery (`RuntimeConfig`) |
| `syslib.rs` | System library detection |
| `multi_file.rs` | Dependency graph and multi-file compilation |
| `wasm.rs` | WebAssembly configuration |

### Linker (`aot/linker/`)

| File | Purpose |
|------|---------|
| `mod.rs` | `LinkerDriver` abstraction, `LinkInput`, `LinkOutput` |
| `gcc.rs` | GCC/Clang linker backend |
| `msvc.rs` | MSVC linker backend |
| `wasm.rs` | WebAssembly linker (wasm-ld) |

### Incremental Compilation (`aot/incremental/`)

| Directory | Purpose |
|-----------|---------|
| `mod.rs` | Incremental compilation coordination |
| `arc_cache/` | ARC-aware object caching (mod, tests) |
| `cache/` | Object file caching (mod, tests) |
| `deps/` | Dependency graph tracking (mod, tests) |
| `function_deps/` | Per-function dependency tracking (mod, tests) |
| `function_hash/` | Per-function content hashing (mod, tests) |
| `hash/` | Content hashing for cache keys (mod, tests) |
| `parallel/` | Parallel job scheduling (mod, tests) |

### Runtime Library (`ori_rt/`)

| File | Purpose |
|------|---------|
| `lib.rs` | C-ABI runtime functions (memory, strings, panic, etc.) |

### CLI Commands (`oric/src/commands/`)

| File | Purpose |
|------|---------|
| `build.rs` | `ori build` command with all build options |
| `demangle.rs` | `ori demangle` command for symbol demangling |
| `compile_common.rs` | Shared compilation utilities |
