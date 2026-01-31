# Proposal: AOT Compilation

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-31
**Affects:** Compiler, tooling, CLI

---

## Summary

This proposal formalizes the Ahead-of-Time (AOT) compilation pipeline for Ori, covering object file generation, optimization passes, linking, debug information, and target configuration.

---

## Problem Statement

The LLVM backend (Phase 21A) currently supports JIT compilation for testing and development. Production deployment requires:

1. **Native executables**: Standalone binaries without runtime compilation
2. **Libraries**: Shared/static libraries for FFI and interoperability
3. **Optimized output**: Production-grade optimization passes
4. **Debug support**: Source-level debugging with DWARF/CodeView
5. **Cross-compilation**: Building for targets other than the host
6. **WASM output**: WebAssembly modules for browser/Node.js

### Current State

| Feature | JIT (21A) | AOT (21B) |
|---------|-----------|-----------|
| LLVM IR generation | Working | Same |
| In-memory execution | Working | N/A |
| Object file output | Missing | Required |
| Linking | N/A | Required |
| Optimization | Limited | Full pipeline |
| Debug info | None | Required |

### Goals

1. Generate native executables from Ori source
2. Support multiple target platforms (Linux, macOS, Windows)
3. Enable optimized release builds
4. Provide debuggable development builds
5. Support WebAssembly output
6. Enable incremental compilation

---

## Terminology

| Term | Definition |
|------|------------|
| **AOT** | Ahead-of-Time compilation; generates machine code before execution |
| **Object file** | Intermediate compiled unit containing machine code and metadata |
| **Linker** | Tool that combines object files into executables or libraries |
| **Target triple** | Platform identifier (e.g., `x86_64-unknown-linux-gnu`) |
| **Data layout** | Memory representation specification for the target |
| **DWARF** | Debug information format for Unix-like systems |
| **CodeView** | Debug information format for Windows |

---

## Design

### Compilation Pipeline

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Source    │───▶│    Parse    │───▶│  Type Check │───▶│   LLVM IR   │
│   (.ori)    │    │    (AST)    │    │   (Types)   │    │  Generation │
└─────────────┘    └─────────────┘    └─────────────┘    └──────┬──────┘
                                                                │
                   ┌─────────────┐    ┌─────────────┐    ┌──────▼──────┐
                   │ Executable  │◀───│    Link     │◀───│   Object    │
                   │   / Lib     │    │             │    │    File     │
                   └─────────────┘    └─────────────┘    └─────────────┘
```

**AOT-specific stages:**
1. **Object generation**: Emit `.o`/`.obj` files from LLVM IR
2. **Optimization**: Run LLVM optimization passes on IR
3. **Debug info**: Embed source locations and type information
4. **Linking**: Combine objects with runtime library into final artifact

### Target Configuration

#### Target Triple

The target is specified as a triple: `<arch>-<vendor>-<os>[-<env>]`

| Component | Examples | Description |
|-----------|----------|-------------|
| arch | `x86_64`, `aarch64`, `wasm32` | CPU architecture |
| vendor | `unknown`, `apple`, `pc` | Hardware vendor |
| os | `linux`, `darwin`, `windows`, `wasi` | Operating system |
| env | `gnu`, `musl`, `msvc` | ABI/environment |

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

#### Data Layout

LLVM data layout string specifies:
- Endianness (`e` = little, `E` = big)
- Pointer size and alignment
- Type alignments
- Stack alignment

Example for `x86_64-unknown-linux-gnu`:
```
e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128
```

#### CPU Features

Optional CPU-specific features can be enabled:

```bash
ori build --target=x86_64-unknown-linux-gnu --cpu=native
ori build --target=x86_64-unknown-linux-gnu --features=+avx2,+fma
```

### Optimization Levels

| Level | Flag | Description | Use Case |
|-------|------|-------------|----------|
| O0 | `--opt=0` | No optimization | Fastest compile, debugging |
| O1 | `--opt=1` | Basic optimization | Development with some speed |
| O2 | `--opt=2` | Standard optimization | Production default |
| O3 | `--opt=3` | Aggressive optimization | Maximum performance |
| Os | `--opt=s` | Optimize for size | Embedded, WASM |
| Oz | `--opt=z` | Minimize size aggressively | Smallest binary |

**Default:** `--opt=0` for `ori run`, `--opt=2` for `ori build --release`

#### Optimization Passes

The optimization pipeline follows LLVM's standard pass manager:

**O1 passes:**
- Early CSE (common subexpression elimination)
- Simplify CFG
- Instruction combining
- Reassociate
- Dead code elimination

**O2 passes (adds):**
- Loop invariant code motion
- Global value numbering
- Aggressive dead code elimination
- Inline small functions
- Loop unrolling (limited)

**O3 passes (adds):**
- Aggressive inlining
- Loop vectorization
- SLP vectorization
- Full unrolling

### Debug Information

#### Debug Levels

| Level | Flag | Description |
|-------|------|-------------|
| None | `--debug=0` | No debug info |
| Line tables | `--debug=1` | Source locations only |
| Full | `--debug=2` | Variables, types, source |

**Default:** `--debug=2` for development, `--debug=0` for release

#### Debug Format

| Platform | Format | Standard |
|----------|--------|----------|
| Linux | DWARF 4 | Default |
| macOS | DWARF 4 + dSYM | Split debug |
| Windows | CodeView/PDB | MSVC standard |
| WASM | DWARF 4 | Source maps |

#### Source Map Generation

For debugging, the compiler emits:

```rust
// DILocation for each expression
let loc = di_builder.create_debug_location(
    line: u32,
    column: u32,
    scope: DIScope,
);
builder.set_current_debug_location(loc);
```

### Object File Generation

#### Object Format

| Platform | Format | Extension |
|----------|--------|-----------|
| Linux | ELF | `.o` |
| macOS | Mach-O | `.o` |
| Windows | COFF | `.obj` |
| WASM | WASM | `.wasm` |

#### Module-to-Object Mapping

Each Ori module produces one object file:

```
src/
├── main.ori       → build/obj/main.o
├── utils.ori      → build/obj/utils.o
└── http/
    ├── client.ori → build/obj/http/client.o
    └── server.ori → build/obj/http/server.o
```

#### Symbol Naming

Ori symbols are mangled for uniqueness:

| Ori | Mangled |
|-----|---------|
| `@main` | `_ori_main` |
| `@foo (x: int) -> int` | `_ori_foo_i` |
| `MyModule.@bar` | `_ori_MyModule_bar` |
| `impl Type.@method` | `_ori_Type_method` |
| `impl Trait for Type.@method` | `_ori_Trait_Type_method` |

**Demangling:** The `ori demangle` command converts mangled names back.

### Linking

#### Link Strategy

```
┌─────────────────────────────────────────────────────────────┐
│                      Final Executable                        │
├─────────────────────────────────────────────────────────────┤
│  User Object Files     │  Ori Runtime      │  System Libs   │
│  (main.o, utils.o)     │  (libori_rt.a)    │  (libc, libm)  │
└─────────────────────────────────────────────────────────────┘
```

#### Runtime Library

The Ori runtime (`libori_rt`) provides:

| Category | Functions |
|----------|-----------|
| Memory | `ori_alloc`, `ori_free`, `ori_realloc` |
| Reference counting | `ori_rc_inc`, `ori_rc_dec`, `ori_rc_new` |
| Strings | `ori_str_concat`, `ori_str_from_int`, etc. |
| Collections | `ori_list_new`, `ori_map_new`, etc. |
| Panic | `ori_panic`, `ori_panic_handler` |
| I/O | `ori_print`, `ori_stdin_read` |

**Linking modes:**

| Mode | Flag | Description |
|------|------|-------------|
| Static | `--link=static` | Embed runtime (default) |
| Dynamic | `--link=dynamic` | Link to `libori_rt.so` |

#### System Linker

The compiler invokes the system linker:

| Platform | Linker | Notes |
|----------|--------|-------|
| Linux | `ld` or `lld` | Via `cc` driver |
| macOS | `ld64` | Via `clang` driver |
| Windows | `link.exe` or `lld-link` | Via `cl` or direct |

**Linker selection:**
```bash
ori build --linker=lld      # Use LLVM's LLD
ori build --linker=system   # Use system default
```

#### Link-Time Optimization (LTO)

| Mode | Flag | Description |
|------|------|-------------|
| None | `--lto=off` | No LTO (default debug) |
| Thin | `--lto=thin` | Fast parallel LTO |
| Full | `--lto=full` | Maximum optimization |

### Output Artifacts

#### Build Outputs

| Command | Output | Description |
|---------|--------|-------------|
| `ori build` | `./build/debug/<name>` | Debug executable |
| `ori build --release` | `./build/release/<name>` | Release executable |
| `ori build --lib` | `./build/<name>.a` | Static library |
| `ori build --dylib` | `./build/<name>.so` | Shared library |
| `ori build --wasm` | `./build/<name>.wasm` | WebAssembly module |

#### Output Control

```bash
ori build -o myapp              # Custom output name
ori build --out-dir ./dist      # Custom output directory
ori build --emit=obj            # Stop at object files
ori build --emit=llvm-ir        # Emit LLVM IR
ori build --emit=asm            # Emit assembly
```

### Incremental Compilation

#### Caching Strategy

```
build/
├── cache/
│   ├── main.ori.hash           # Source hash
│   ├── main.o                  # Cached object
│   └── deps/
│       └── main.deps           # Dependency list
└── release/
    └── myapp                   # Final binary
```

**Recompilation triggers:**
1. Source file changed (hash mismatch)
2. Dependency changed (transitive)
3. Compiler flags changed
4. Compiler version changed

#### Parallel Compilation

Independent modules compile in parallel:

```bash
ori build --jobs=8              # 8 parallel compilations
ori build --jobs=auto           # Use all cores (default)
```

### WebAssembly Backend

#### WASM Targets

| Target | Description | Use Case |
|--------|-------------|----------|
| `wasm32-unknown-unknown` | Standalone WASM | Embedded, plugins |
| `wasm32-wasi` | WASI preview 2 | CLI tools, servers |
| `wasm32-emscripten` | Emscripten | Browser with full API |

#### JavaScript Interop

For browser targets, generate bindings:

```bash
ori build --wasm --js-bindings  # Generate .js glue code
```

Output:
```
build/
├── myapp.wasm
├── myapp.js         # JavaScript bindings
└── myapp.d.ts       # TypeScript declarations
```

#### WASM Optimizations

```bash
ori build --wasm --opt=z        # Smallest WASM
ori build --wasm --wasm-opt     # Run wasm-opt post-processor
```

### Error Handling

#### Linker Errors

```
error[E1201]: linker failed
  --> linking myapp
   |
   = note: undefined reference to `external_function`
   = note: linker command: ld -o myapp main.o ...
   = help: ensure all external functions are available
```

#### Missing Target

```
error[E1202]: unsupported target
  --> --target=riscv64-unknown-linux-gnu
   |
   = note: target 'riscv64-unknown-linux-gnu' is not supported
   = note: supported targets: x86_64-unknown-linux-gnu, ...
   = help: run `ori targets` to list all supported targets
```

#### Object Generation Failed

```
error[E1203]: failed to generate object file
  --> src/main.ori
   |
   = note: LLVM error: <llvm message>
   = help: this may be a compiler bug; please report
```

---

## CLI Interface

### New Commands

#### `ori build`

```bash
ori build [OPTIONS] [FILE]

Options:
    --release           Build with optimizations (O2, no debug)
    --target=TARGET     Target triple (default: native)
    --opt=LEVEL         Optimization level: 0, 1, 2, 3, s, z
    --debug=LEVEL       Debug info level: 0, 1, 2
    --lib               Build as static library
    --dylib             Build as shared library
    --wasm              Build for WebAssembly
    -o, --output=FILE   Output file name
    --out-dir=DIR       Output directory
    --emit=TYPE         Emit: obj, llvm-ir, llvm-bc, asm
    --linker=LINKER     Linker: system, lld
    --link=MODE         Link mode: static, dynamic
    --lto=MODE          LTO: off, thin, full
    --jobs=N            Parallel compilation jobs
    --cpu=CPU           Target CPU (e.g., native, skylake)
    --features=FEAT     CPU features (+avx2, -sse4)
    --js-bindings       Generate JavaScript bindings (WASM)
    --wasm-opt          Run wasm-opt post-processor
    -v, --verbose       Verbose output
```

#### `ori targets`

```bash
ori targets                     # List all supported targets
ori targets --installed         # List targets with toolchains
```

#### `ori demangle`

```bash
ori demangle _ori_MyModule_foo  # → MyModule.@foo
```

### Modified Commands

#### `ori run`

Adds AOT mode for faster repeated runs:

```bash
ori run src/main.ori            # JIT (default, fast startup)
ori run --compile src/main.ori  # AOT (slower startup, faster run)
```

#### `ori check`

No changes; type checking is independent of codegen.

---

## Implementation Architecture

### New Crate Structure

```
compiler/
├── ori_llvm/
│   ├── src/
│   │   ├── aot/                    # New: AOT-specific code
│   │   │   ├── mod.rs
│   │   │   ├── object.rs           # Object file emission
│   │   │   ├── linker.rs           # Linker invocation
│   │   │   ├── target.rs           # Target configuration
│   │   │   ├── debug_info.rs       # DWARF/CodeView emission
│   │   │   └── passes.rs           # Optimization pass manager
│   │   ├── wasm/                   # New: WASM-specific
│   │   │   ├── mod.rs
│   │   │   ├── bindings.rs         # JS binding generation
│   │   │   └── wasi.rs             # WASI support
│   │   └── ... (existing JIT code)
│   └── Cargo.toml
└── oric/
    └── src/
        └── commands/
            ├── build.rs            # New: AOT build command
            └── ... (existing)
```

### Key Types

```rust
/// Target configuration for AOT compilation
pub struct TargetConfig {
    pub triple: String,
    pub cpu: Option<String>,
    pub features: Vec<String>,
    pub data_layout: String,
}

/// Compilation options
pub struct CompileOptions {
    pub target: TargetConfig,
    pub opt_level: OptLevel,
    pub debug_level: DebugLevel,
    pub lto: LtoMode,
    pub emit: EmitKind,
}

/// Build output configuration
pub struct BuildConfig {
    pub output_type: OutputType,      // Executable, StaticLib, DynLib, WASM
    pub output_path: PathBuf,
    pub link_mode: LinkMode,          // Static, Dynamic
    pub incremental: bool,
    pub parallel_jobs: usize,
}

pub enum OptLevel { O0, O1, O2, O3, Os, Oz }
pub enum DebugLevel { None, LineTablesOnly, Full }
pub enum LtoMode { Off, Thin, Full }
pub enum EmitKind { Object, LlvmIr, LlvmBc, Asm, Exe }
pub enum OutputType { Executable, StaticLib, DynLib, Wasm }
```

### Compilation Flow

```rust
pub fn compile_aot(
    sources: &[PathBuf],
    options: &CompileOptions,
    build: &BuildConfig,
) -> Result<(), CompileError> {
    // 1. Parse and type-check (existing)
    let modules = parse_and_check(sources)?;

    // 2. Configure LLVM target
    let target = configure_target(&options.target)?;

    // 3. Generate LLVM IR (existing, with debug info)
    let llvm_modules = modules.iter()
        .map(|m| generate_ir(m, &target, options.debug_level))
        .collect::<Result<Vec<_>, _>>()?;

    // 4. Run optimization passes
    for module in &llvm_modules {
        run_passes(module, options.opt_level, options.lto)?;
    }

    // 5. Emit object files
    let objects = llvm_modules.iter()
        .map(|m| emit_object(m, &target, options.emit))
        .collect::<Result<Vec<_>, _>>()?;

    // 6. Link
    if build.output_type != OutputType::Object {
        link(&objects, &build.output_path, &options.target, build.link_mode)?;
    }

    Ok(())
}
```

---

## Migration from JIT

### Shared Code

Most LLVM codegen is shared between JIT and AOT:
- Type lowering
- Expression compilation
- Control flow
- Pattern matching
- Runtime function declarations

### AOT-Specific Code

New code required for AOT:
- Target machine creation
- Object file emission
- Debug info generation
- Linker driver
- Incremental caching

### Testing Strategy

| Test Type | JIT | AOT |
|-----------|-----|-----|
| Unit tests | Primary | Verify parity |
| Spec tests | Both (parallel) | Both (parallel) |
| Performance | N/A | Benchmarks |
| Debug | N/A | Debugger tests |

---

## Interaction with Other Features

### Conditional Compilation

`#target()` and `#cfg()` are evaluated at compile time:

```ori
#target(os: "linux")
@platform_name () -> str = "Linux"

#target(os: "windows")
@platform_name () -> str = "Windows"
```

Only the matching variant is compiled into the object file.

### FFI

External functions resolve at link time:

```ori
extern "c" from "mylib" {
    @_native_call (x: int) -> int as "native_call"
}
```

Linker command includes `-lmylib`.

### Capabilities

Capability resolution happens at compile time; no runtime impact on AOT.

---

## Performance Considerations

### Compile Time

| Factor | Impact | Mitigation |
|--------|--------|------------|
| LLVM optimization | High | Incremental, parallel |
| Debug info | Medium | Optional levels |
| LTO | Very high | Thin LTO, optional |
| Linking | Medium | LLD, parallel |

**Expected compile times (100k LOC project):**

| Mode | Time |
|------|------|
| Debug (O0) | ~10s |
| Release (O2) | ~30s |
| Release + LTO | ~60s |

### Runtime Performance

AOT-compiled code should match or exceed JIT performance:
- Same LLVM optimization passes
- No JIT compilation overhead at startup
- Better cache locality (code in executable)

---

## Spec Changes Required

None. AOT compilation is an implementation detail; the language semantics are unchanged.

---

## Roadmap Changes Required

### Update `phase-21B-aot.md`

Replace placeholder content with detailed implementation tasks:

1. **21B.1: Target Configuration**
   - Target triple parsing and validation
   - Data layout configuration
   - CPU feature detection

2. **21B.2: Object File Emission**
   - LLVM TargetMachine creation
   - Object file writing (ELF/Mach-O/COFF)
   - Symbol mangling

3. **21B.3: Debug Information**
   - DIBuilder integration
   - Source location tracking
   - Type debug info
   - DWARF/CodeView emission

4. **21B.4: Optimization Pipeline**
   - Pass manager configuration
   - Optimization levels (O0-O3, Os, Oz)
   - LTO support

5. **21B.5: Linking**
   - Linker driver (cc/clang/link.exe)
   - Runtime library (libori_rt)
   - System library detection

6. **21B.6: Incremental Compilation**
   - Source hashing
   - Dependency tracking
   - Cache management

7. **21B.7: WebAssembly Backend**
   - WASM target configuration
   - JavaScript binding generation
   - WASI support

8. **21B.8: CLI Integration**
   - `ori build` command
   - `ori targets` command
   - Flag parsing

---

## Summary Table

| Aspect | Design Decision |
|--------|-----------------|
| Object format | Platform-native (ELF/Mach-O/COFF) |
| Default optimization | O0 debug, O2 release |
| Default debug info | Full debug, none release |
| Linking | Static runtime by default |
| Linker | System default, LLD optional |
| LTO | Off by default, thin recommended |
| Incremental | Hash-based, parallel |
| WASM | Standalone and WASI targets |
| Symbol mangling | `_ori_<module>_<function>` |

---

## Related Proposals

- **Phase 21A (LLVM Backend)**: JIT implementation (prerequisite)
- **FFI Proposal**: External function linking
- **Conditional Compilation**: Target-specific code
- **WASM FFI Proposal**: JavaScript interop (future)

---

## Open Questions

1. **Should debug symbols be separate files by default on macOS (dSYM)?**
   - Pro: Smaller binaries, standard practice
   - Con: Extra file management

2. **Should we support cross-compilation out of the box?**
   - Requires target sysroots
   - Consider: `ori target add x86_64-unknown-linux-gnu`

3. **Should incremental compilation cache be shared across projects?**
   - Pro: Faster builds for shared dependencies
   - Con: Cache invalidation complexity

4. **What's the minimum LLVM version to support?**
   - Recommend: LLVM 15+ (better WASM, newer pass manager)
   - Currently using: LLVM 17
