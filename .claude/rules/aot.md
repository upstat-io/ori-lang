---
paths:
  - "**/aot/**"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

# AOT Compilation

## Pipeline
Parse → TypeCheck → LLVM IR → Object → Link → Executable

## Building
```bash
cargo bl   # debug
cargo blr  # release
```
**Always build `ori_rt` alongside `oric`.**

## Runtime Discovery
1. Same directory as compiler
2. `<exe>/../lib/libori_rt.a`
3. `$ORI_WORKSPACE_DIR/target/`

## Symbol Mangling
Format: `_ori_<module>$<function>[<suffix>]`
- `@main` → `_ori_main`
- `math.@add` → `_ori_math$add`
- Trait impl: `_ori_int$$Eq$equals`
- Extension: `_ori_list_int_$$ext$count`
- Generic: `$G` suffix, associated: `$A$` marker

## Linker Drivers
- Linux/macOS: `GccLinker`
- Windows: `MsvcLinker`
- WASM: `WasmLinker` (+ `JsBindingGenerator`, `WasmOptRunner`)

## Optimization
- `OptimizationLevel`: None, Less, Default, Aggressive
- `LtoMode`: None, ThinLocal, Thin, Full
- `run_optimization_passes()`, `optimize_module()`, `run_lto_pipeline()`

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

## LLVM Debugging

For LLVM IR debugging workflow, tools, common bug categories, and verification strategy, see @llvm.md

**Quick AOT diagnostics** — use `diagnostics/` scripts for fast debugging:
- `diagnostics/diagnose-aot.sh <file.ori>` — all-in-one: build + run + leak check + RC stats + IR dump
- `diagnostics/dual-exec-debug.sh <file.ori>` — compare interpreter vs AOT; auto-dumps diagnostics on mismatch
- `diagnostics/ir-dump.sh <file.ori>` — annotated LLVM IR
- `diagnostics/rc-stats.sh <file.ori>` — RC balance per function
