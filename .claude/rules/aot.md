---
paths:
  - "**aot**"
---

# AOT Compilation

## Pipeline

- Parse → TypeCheck → LLVM IR → Object → Link → Executable
- Build: `cargo bl` (debug) | `cargo blr` (release)
- **Always build `ori_rt` alongside `oric`**

## Runtime Discovery

1. Same directory as compiler
2. `<exe>/../lib/libori_rt.a`
3. `$ORI_WORKSPACE_DIR/target/`

## Symbol Mangling

- Format: `_ori_<module>$<function>[<suffix>]`
- `@main` → `_ori_main` | `math.@add` → `_ori_math$add`
- Trait impl: `_ori_int$$Eq$equals` | Extension: `_ori_list_int_$$ext$count`
- Generic: `$G` suffix | Associated: `$A$` marker

## Linker Drivers

- Linux/macOS: `GccLinker` | Windows: `MsvcLinker` | WASM: `WasmLinker` (+ `JsBindingGenerator`, `WasmOptRunner`)

## Optimization

- `OptimizationLevel`: None | Less | Default | Aggressive
- `LtoMode`: None | ThinLocal | Thin | Full
- `run_optimization_passes()` | `optimize_module()` | `run_lto_pipeline()`

## Key Subsystems

| Directory | Purpose |
|-----------|---------|
| `target.rs` | Target triple, feature support |
| `object.rs` | Object emission (`OutputFormat` enum) |
| `mangle.rs` | Symbol mangling/demangling |
| `passes.rs` | Optimization pass pipeline |
| `runtime.rs` | Runtime discovery |
| `linker/` | Platform linker drivers (gcc, msvc, wasm) |
| `debug/` | Debug info generation (DWARF/CodeView) — `DebugInfoBuilder`, `DebugLevel` |
| `wasm.rs` | WebAssembly config (`WasmConfig`, `WasiConfig`) |
| `incremental/` | Incremental compilation (caching, deps, parallel) |
| `multi_file/` | Multi-file compilation, module dependency graphs |
| `syslib/` | System library discovery |

## Debugging

- For LLVM IR debugging workflow, tools, and verification strategy, see @llvm.md
- **Diagnostic scripts**: `diagnostics/diagnose-aot.sh` | `dual-exec-debug.sh` | `ir-dump.sh` | `ir-diff.sh` | `rc-stats.sh` | `codegen-audit.sh` | `disasm-ori.sh` (see compiler.md for full list)
