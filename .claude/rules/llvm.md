---
paths:
  - "**ori_llvm**"
---

# LLVM Backend

- LLVM 17 required | path in `.cargo/config.toml`
- `ori_llvm` and `ori_rt` are workspace members but not in `default-members` — use `-p` or `cargo bl`
- Build: `cargo bl` (debug) | `cargo blr` (release)
- Clippy: `cargo cll` (`-p ori_llvm`) | Tests: `cargo test -p ori_llvm` | `./llvm-test.sh` | `./test-all.sh`
- **Always build both `oric` AND `ori_rt`** — Cargo only builds rlib; staticlib must be explicit

## MANDATORY: Test with Release Binary

- **After ANY `ori_llvm`/`ori_rt` changes**: `cargo blr` then `./test-all.sh`
- Debug and release differ due to FastISel behavior (see below) — never consider LLVM work done after debug-only testing

## Architecture

- **Two-pass compilation**: (1) Declare: walk functions → `FunctionAbi` → declare with calling conventions/attributes. (2) Define: walk again → ARC pipeline (CanExpr → ARC IR → `ArcIrEmitter` → LLVM IR)
- **Pipeline**: `declare_all()` → `define_all()` → `compile_tests()` → `compile_impls()` → `compile_derives()` → `generate_main_wrapper()`
- **ARC codegen is the only path** — all functions go through `CanExpr → ARC IR → ArcIrEmitter → LLVM IR`

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
- **NEVER `load %BigStruct, ptr` for structs >16 bytes in JIT** — use per-field `struct_gep` + `load` + `insert_value`. See `FunctionCompiler::load_indirect_param()`
- Symptom: SIGSEGV in release only, identical IR in both builds
- Cause: FastISel mishandles large aggregate spills; entry-block allocas / `noredzone` / calling convention changes do NOT fix

### Loop Latch Pattern
- `entry → header → body → latch → header (or exit)`
- **`continue` → latch** (NOT header) — skipping latch = infinite loop
- **`break` → exit**

### ARM/aarch64 Portability
- **`c_char` not `i8`** in `ori_rt`: C string pointers MUST use `std::ffi::c_char`. `c_char` is `i8` on x86_64 but `u8` on aarch64 — hardcoding `i8` breaks ARM. LLVM opaque `ptr` is unaffected (Rust-side only).
- Test AOT on ARM via GCP `t2a-standard-2` instance or macOS CI (Apple Silicon)

### Inkwell Pitfalls
- `build_*` fails without `position_at_end(block)` — always position first
- `build_gep` is `unsafe` — first index = pointer deref (almost always `0`), subsequent = aggregate navigation
- Struct return by value from JIT can corrupt last field — use `Sret` return passing

## Derive Codegen

- `codegen/derive_codegen/` — sync point with evaluator/type-checker | all 7 derived traits via strategy dispatch:
  - `ForEachField` → Eq, Comparable, Hashable
  - `FormatFields` → Printable, Debug
  - `CloneFields` → Clone | `DefaultConstruct` → Default

## Type-Qualified Mangling

- `Point.distance` → `_ori_Point$distance` | `Line.distance` → `_ori_Line$distance`

## Debugging

| Variable | Purpose |
|----------|---------|
| `ORI_DUMP_AFTER_LLVM=1` | Annotated LLVM IR with Ori function names, RC/COW ops |
| `ORI_DEBUG_LLVM=1` | Legacy alias for `ORI_DUMP_AFTER_LLVM` |
| `ORI_DUMP_AFTER_ARC=1` | ARC IR with RC strategy annotations |
| `ORI_LOG=ori_llvm=debug` | Codegen event log (function-level) |
| `ORI_LOG=ori_llvm=trace` | Per-instruction detail (very verbose) |
| `ORI_AUDIT_CODEGEN=1` | In-pipeline RC/COW/ABI audit (add `ORI_AUDIT_STRICT=1` for pessimistic) |

- **Runtime**: `ORI_TRACE_RC=1` | `ORI_RT_DEBUG=1` | `ORI_CHECK_LEAKS=1` (on compiled binary)
- **Diagnostic scripts**: `diagnostics/ir-dump.sh` | `ir-diff.sh` | `rc-stats.sh` | `codegen-audit.sh` | `diagnose-aot.sh` | `dual-exec-debug.sh` (see compiler.md for full list)
- **Triage**: Verification fail = our codegen bug | optimization crash = `opt -verify-each -opt-bisect-limit=N` | runtime segfault = check ABI/GEP/aggregate loads | compare with `clang -emit-llvm -S -O0`
- Tests run **sequentially** (not parallel) due to `Context::create()` contention

## Verification

- Verify at multiple points: per-function (`fn_val.verify(true)`), pre-optimization, post-optimization | dump IR on failure

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
