---
paths:
  - "**/ori_llvm/**"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

# LLVM Backend

LLVM 17 required. Path in `.cargo/config.toml`.

## Commands

- Build: `cargo bl` (debug), `cargo blr` (release)
- Clippy: `cargo cll`
- Tests: `./llvm-test.sh` (unit), `./test-all.sh` (full)
- **Always build both `oric` AND `ori_rt`** — Cargo only builds rlib; staticlib must be explicit

## MANDATORY: Test with Release Binary

**After ANY `ori_llvm`/`ori_rt` changes, test release:** `cargo blr` → `./test-all.sh`
Debug and release can differ due to FastISel behavior (see below). Never consider LLVM work done after debug-only testing.

## Architecture

**Two-pass compilation:**
1. **Declare**: Walk functions → compute `FunctionAbi` → declare with calling conventions/attributes
2. **Define**: Walk again → lower through ARC pipeline (CanExpr → ARC IR → `ArcIrEmitter` → LLVM IR)

**Pipeline**: `declare_all()` → `define_all()` → `compile_tests()` → `compile_impls()` → `compile_derives()` → `generate_main_wrapper()`

**ARC codegen is the only path.** All functions (JIT and AOT) go through `CanExpr → ARC IR → ArcIrEmitter → LLVM IR` with full RC lifecycle management.

## Key Abstractions

| Abstraction | Purpose |
|-------------|---------|
| `FunctionCompiler` | Two-pass declare/define orchestrator |
| `ArcIrEmitter` | ARC IR → LLVM IR emission (with RC) |
| `IrBuilder` | ID-based inkwell wrapper, hides `'ctx` lifetime |
| `FunctionAbi` | Parameter/return passing modes (Direct/Indirect/Sret/Void) |
| `ValueArena` | Opaque IDs (`ValueId`, `BlockId`, `FunctionId`) |

## Critical Rules

### FastISel Aggregate Bug
**NEVER `load %BigStruct, ptr` for structs >16 bytes in JIT code.** Use per-field `struct_gep` + `load` + `insert_value`. See `FunctionCompiler::load_indirect_param()`.
- **Symptom**: SIGSEGV in release only, identical IR in both builds
- **Cause**: FastISel mishandles large aggregate spills; release runtime callees expose overlap
- Entry-block allocas, `noredzone`, calling convention changes do NOT fix this

### Loop Latch Pattern
```
entry → header → body → latch → header (or exit)
```
- **`continue` → latch** (NOT header) — skipping latch = infinite loop
- **`break` → exit**

### Inkwell Pitfalls
- `build_*` fails without `position_at_end(block)` — always position first
- `build_gep` is `unsafe` — first index = pointer deref (almost always `0`), subsequent = aggregate navigation
- Struct return by value from JIT can corrupt last field — use `Sret` return passing

## Derive Codegen

`codegen/derive_codegen/` — sync point with evaluator/type-checker. All 7 derived traits via strategy dispatch:
- `ForEachField` → Eq, Comparable, Hashable
- `FormatFields` → Printable, Debug
- `CloneFields` → Clone | `DefaultConstruct` → Default

## Type-Qualified Mangling

`Point.distance` → `_ori_Point$distance` | `Line.distance` → `_ori_Line$distance`

## Debugging

| Variable | Purpose |
|----------|---------|
| `ORI_DEBUG_LLVM=1` | Dump full IR to stderr before verification |
| `ORI_LOG=ori_llvm=debug` | Codegen event log (function-level) |
| `ORI_LOG=ori_llvm=trace` | Per-instruction detail (very verbose) |

**Diagnostic scripts** (`diagnostics/`) — use these FIRST for LLVM/AOT bugs:
- `diagnostics/ir-dump.sh <file.ori>` — annotated LLVM IR dump
- `diagnostics/ir-diff.sh <a.ori> <b.ori>` — compare IR between two programs
- `diagnostics/rc-stats.sh <file.ori>` — RC operation balance per function (flags over-release/leaks)
- `diagnostics/diagnose-aot.sh <file.ori>` — all-in-one: build + run + leak check + RC stats + IR (add `--valgrind` for memory errors)
- `diagnostics/dual-exec-debug.sh <file.ori>` — compare interpreter vs AOT output; auto-dumps IR + RC stats on mismatch

**Triage**: Verification fail = our codegen bug. Optimization crash = `opt -verify-each -opt-bisect-limit=N`. Runtime segfault = check ABI/GEP/aggregate loads. Compare with `clang -emit-llvm -S -O0` for reference IR.

Tests run **sequentially** (not parallel) due to `Context::create()` contention.

## Verification

Verify at multiple points: per-function (`fn_val.verify(true)`), pre-optimization, post-optimization. Dump IR on failure.

## Key Files

| File | Purpose |
|------|---------|
| `codegen/mod.rs` | Codegen entry |
| `codegen/function_compiler/` | Two-pass declare/define |
| `codegen/ir_builder/` | ID-based instruction emission |
| `codegen/arc_emitter/` | ARC IR → LLVM IR emission |
| `codegen/derive_codegen/` | Derived trait IR generation |
| `codegen/abi/` | ABI computation |
| `aot/` | AOT pipeline (linking, mangling, target) |
| `evaluator.rs` | JIT execution + IR verification |
| `runtime.rs` | Runtime function declarations |
