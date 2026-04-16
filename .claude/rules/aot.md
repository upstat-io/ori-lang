---
paths:
  - "compiler/ori_llvm/**/aot*/**"
  - "scripts/**/*aot*"
  - "diagnostics/**/*aot*"
  - "plans/aot-perf/**"
---

# AOT Compilation

## Pipeline

- Parse → TypeCheck → Canon → ARC Lower → AIMS → Repr → LLVM IR → Object → Link → Executable
- Build: `cargo b` (debug) | `cargo b --release` (release)
- **Always build `ori_rt` alongside `oric`**

## Runtime Discovery

`candidate_directories()` in `aot/runtime.rs` collects candidates; `detect()` returns the first match. Search order:

1. Same directory as the compiler binary (`exe_dir`)
2. Sibling profile directories: `target_dir/{release,debug}` where `target_dir = exe_dir.parent()` (covers debug↔release cross-lookup, skips if equals `exe_dir`)
3. Installed layout: `exe_dir/../lib/` (e.g., `/usr/local/bin/ori` → `/usr/local/lib/`)
4. Standalone `ori_rt` build: `<workspace_root>/compiler/ori_rt/target/{release,debug}`
5. `$ORI_WORKSPACE_DIR/target/{release,debug}` (development via `cargo run`)

Override: `--runtime-path` CLI flag. Platform lib names: `libori_rt.a` (Linux/macOS) | `ori_rt.lib` (Windows). ASan variant: `libori_rt_asan.a` / `ori_rt_asan.lib`.

## Symbol Mangling

Canonical home: `aot/mangle/emit.rs`. Prefix: `_ori_`. Identifier encoding: alphanumeric + `_` pass through; special chars → `$XX` escapes (`<`→`$LT`, `>`→`$GT`, `,`→`$C`, `[`→`$LB`, `]`→`$RB`, `(`→`$LP`, `)`→`$RP`, `:`→`$CC`, `-`→`$D`). Module path separators (`/`, `\`, `.`, `:`) → `$`.

| Pattern | Format | Example |
|---------|--------|---------|
| Free function | `_ori_[<module>$]<fn>` | `math.@add` → `_ori_math$add` |
| Trait impl | `_ori_<type>$$<trait>$<method>` | `_ori_int$$Eq$equals` |
| Extension | `_ori_<type>$$ext$[<module>$]<method>` | `_ori_list$$ext$my_mod$count` |
| Inherent method | `_ori_[<module>$]<type>$<method>` | `_ori_Point$distance` |
| Generic | `_ori_[<module>$]<fn>$G<t0>_<t1>...` | `_ori_identity$Gint` |
| Associated fn | `_ori_<type>$A$<fn>` | `_ori_Option$A$some` |

## Linker Drivers

- Linux/macOS: `GccLinker` | Windows: `MsvcLinker` | WASM: `WasmLinker` (+ `JsBindingGenerator`, `WasmOptRunner`)

## Optimization

- `OptimizationLevel`: `O0` | `O1` | `O2` | `O3` | `Os` | `Oz` (maps to LLVM `default<OX>` pipelines; defined in `aot/passes/config.rs`)
- `LtoMode`: `Off` | `Thin` | `Full` (defined in `aot/passes/config.rs`)
- `run_optimization_passes()` | `optimize_module()` | `run_lto_pipeline()`

## Key Subsystems

| Directory | Purpose |
|-----------|---------|
| `target.rs` | Target triple, feature support |
| `object.rs` | Object emission (`OutputFormat` enum) |
| `mangle/` | Symbol mangling/demangling (`mod.rs` constants, `emit.rs` functions) |
| `passes/` | Optimization pass pipeline (`mod.rs` entry, `config.rs` for `OptimizationLevel`/`LtoMode`/`OptimizationConfig`, `sanitizer.rs`) |
| `runtime.rs` | Runtime discovery (5-step candidate search) |
| `linker/` | Platform linker drivers (gcc, msvc, wasm) |
| `debug/` | Debug info generation (DWARF/CodeView) — `DebugInfoBuilder`, `DebugLevel` |
| `wasm/` | WebAssembly target (`mod.rs` entry, `config.rs` for `WasmConfig`/`WasiConfig`, `wasi.rs`, `optimize.rs`) |
| `incremental/` | Incremental compilation (caching, deps, parallel) |
| `multi_file/` | Multi-file compilation, module dependency graphs |
| `syslib/` | System library discovery |

## Debugging

- For LLVM IR debugging workflow, tools, and verification strategy, see @llvm.md
- **Diagnostic scripts**: see @diagnostic.md §Diagnostic Scripts for full list and flags
