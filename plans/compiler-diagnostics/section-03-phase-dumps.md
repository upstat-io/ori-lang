---
section: "03"
title: Phase Dump System
status: complete
goal: "Centralized debug flag registry + ORI_DUMP_AFTER_* phase dumps for parse/typeck/ARC/LLVM"
inspired_by:
  - "Roc debug_flags crate (centralized env var flags + dbg_set!/dbg_do! macros)"
  - "Rust -Zdump-mir (MIR dump at specific passes)"
  - "Go GOSSAFUNC (SSA phase visualization)"
depends_on: []
sections:
  - id: "03.1"
    title: "Centralized Debug Flags Module"
    status: complete
  - id: "03.2"
    title: "ORI_DUMP_AFTER_PARSE"
    status: complete
  - id: "03.3"
    title: "ORI_DUMP_AFTER_TYPECK"
    status: complete
  - id: "03.4"
    title: "ORI_DUMP_AFTER_ARC"
    status: complete
  - id: "03.5"
    title: "ORI_DUMP_AFTER_LLVM"
    status: complete
  - id: "03.6"
    title: "Consistency Validation Script"
    status: complete
  - id: "03.7"
    title: "Completion Checklist"
    status: complete
---

# Section 03: Phase Dump System

**Status:** Not Started
**Goal:** A centralized debug flag module (following Roc's pattern) plus four phase-specific IR dump hooks that make the compiler's internal transformations visible: after parsing, after type checking, after ARC lowering, and after LLVM IR generation.

**Context:** The Ori compiler has 4 major transformation phases (Parse → TypeCheck → ARC IR → LLVM IR), but only the final LLVM IR is dumpable today (via `ORI_DEBUG_LLVM`). When the ARC emitter makes a wrong RC decision, there's no way to see the ARC IR that produced the bad LLVM IR — you have to read Rust source code. Phase dumps make each intermediate representation visible, letting you pinpoint exactly where the pipeline goes wrong.

**Reference implementations:**
- **Roc** `crates/compiler/debug_flags/src/lib.rs`: `flags!` macro defines all env var names, `dbg_set!`/`dbg_do!` macros for conditional execution, `.cargo/config.toml` integration, `ci/check_debug_vars.sh` for consistency
- **Rust** `rustc_mir_transform/src/dump_mir.rs`: `-Zdump-mir` flag dumps MIR at specific passes, `-Zdump-mir-exclude-alloc-bytes` for filtering
- **Go** `GOSSAFUNC=myfunction go build`: Generates interactive HTML showing each SSA pass transformation

**Depends on:** Nothing (but benefits from Section 01 scripts for integration).

---

## 03.1 Centralized Debug Flags Module

**File(s):** `compiler/oric/src/debug_flags.rs` (new file)

Create a single source of truth for all debugging environment variables, following Roc's `debug_flags` pattern. This replaces scattered `std::env::var("ORI_DEBUG_LLVM")` checks with a centralized, documented, validated module.

- [x] Create `compiler/oric/src/debug_flags.rs` with flag definitions (2026-02-27)
  ```rust
  //! Centralized debug flags for the Ori compiler.
  //!
  //! All compiler debugging environment variables are defined here.
  //! In debug builds, flags are checked at runtime via env vars.
  //! In release builds, all flags evaluate to `false` (zero overhead).
  //!
  //! Usage:
  //!   ORI_DUMP_AFTER_ARC=1 ori build program.ori

  /// Check if a debug flag is set. Returns `false` in release builds.
  macro_rules! dbg_set {
      ($flag:expr) => {{
          #[cfg(not(debug_assertions))]
          { false }
          #[cfg(debug_assertions)]
          {
              let flag = std::env::var($flag);
              flag.is_ok() && flag.as_deref() != Ok("0")
          }
      }};
  }

  /// Execute an expression only if a debug flag is set.
  macro_rules! dbg_do {
      ($flag:expr, $expr:expr) => {
          #[cfg(debug_assertions)]
          {
              if dbg_set!($flag) {
                  $expr
              }
          }
      };
  }

  // === Phase Dumps ===
  pub const ORI_DUMP_AFTER_PARSE: &str = "ORI_DUMP_AFTER_PARSE";
  pub const ORI_DUMP_AFTER_TYPECK: &str = "ORI_DUMP_AFTER_TYPECK";
  pub const ORI_DUMP_AFTER_ARC: &str = "ORI_DUMP_AFTER_ARC";
  pub const ORI_DUMP_AFTER_LLVM: &str = "ORI_DUMP_AFTER_LLVM";

  // === Existing (migrated) ===
  pub const ORI_DEBUG_LLVM: &str = "ORI_DEBUG_LLVM";

  // === Trace Flags ===
  pub const ORI_TRACE_RC: &str = "ORI_TRACE_RC";
  pub const ORI_RT_DEBUG: &str = "ORI_RT_DEBUG";
  pub const ORI_CHECK_LEAKS: &str = "ORI_CHECK_LEAKS";
  ```
- [x] Export macros and constants from `oric` crate (2026-02-27)
- [x] Migrate existing `ORI_DEBUG_LLVM` checks in `compile_common.rs` to use centralized flag (2026-02-27)
  - Note: `ori_llvm/evaluator/mod.rs` can't import from `oric` (dep direction reversed); deferred to 03.5
- [x] Add `mod debug_flags;` to `compiler/oric/src/lib.rs` (2026-02-27)
- [x] Test: verify existing `ORI_DEBUG_LLVM=1` behavior unchanged after migration (2026-02-27)

---

## 03.2 ORI_DUMP_AFTER_PARSE — AST Dump

**File(s):** `compiler/oric/src/commands/compile_common.rs` (add dump hook after parse phase)

Dump the parsed AST in a human-readable format. Shows the structure the parser produced before type checking.

- [x] Add dump hook after `parse_module()` call in compile pipeline (2026-02-27)
  - Hook placed in `report_frontend_errors()` in `commands/mod.rs` after `parsed(db, file)` call
- [x] Use existing `Debug` impls on AST nodes, or add a simple pretty-printer (2026-02-27)
  - Custom pretty-printer in `compiler/oric/src/ast_dump/` module (3 files: mod.rs, expr.rs, patterns.rs)
  - Covers all ~30 ExprKind variants, all BindingPattern/MatchPattern variants, all ParsedType variants
- [x] Output format: indented tree showing function signatures, expression structure, pattern forms (2026-02-27)
  ```
  === AST after parse: test.ori ===
  Function @add (a: TypeId::INT, b: TypeId::INT) -> TypeId::INT
    Binary(+)
      Ident(a)
      Ident(b)
  === END AST ===
  ```
- [x] Gate behind `dbg_do!(ORI_DUMP_AFTER_PARSE, ...)` (2026-02-27)
  - Zero overhead in release builds (compiled out via `#[cfg(debug_assertions)]`)
  - Respects `ORI_DUMP_AFTER_PARSE=0` to explicitly disable
- [x] Test: `ORI_DUMP_AFTER_PARSE=1 ori check test.ori` produces readable AST (2026-02-27)
  - Verified with multiple spec test files (int_literals.ori, loops.ori, match.ori)
  - Verified gating: no output without env var, no output with =0

---

## 03.3 ORI_DUMP_AFTER_TYPECK — Typed IR Dump

**File(s):** `compiler/oric/src/commands/compile_common.rs` (add dump hook after type check phase)

Dump the type-annotated IR after type checking. Shows inferred types for all expressions, resolved method calls, and trait implementations.

- [x] Add dump hook after type checking completes
- [x] Output format: similar to AST dump but with type annotations on every node
  ```
  === Typed IR after typeck: test.ori ===
  Function @main () -> int
    Block : int
      LetBinding xs : [int] =
        MethodCall .reverse() : [int]  [builtin: list.reverse]
          MethodCall .push(3 : int) : [int]  [builtin: list.push]
            ListLiteral [1 : int, 2 : int] : [int]
      MethodCall .length() : int  [builtin: list.length]
        Ident xs : [int]
  === END Typed IR ===
  ```
- [x] Show resolved method dispatch (builtin vs trait impl vs inherent)
- [x] Show type variable unification results
- [x] Gate behind `dbg_do!(ORI_DUMP_AFTER_TYPECK, ...)`
- [x] Test: `ORI_DUMP_AFTER_TYPECK=1 ori check test.ori` shows types on all nodes

---

## 03.4 ORI_DUMP_AFTER_ARC — ARC IR Pretty-Printer

**File(s):** `compiler/oric/src/arc_dump/` (new module: `mod.rs` + `instr.rs`)

This is the highest-value phase dump. The ARC IR is the intermediate form between typed Ori expressions and LLVM IR — it includes RC strategy decisions, drop placement, and COW operation selection. Today this IR exists only as in-memory Rust structs with no serialization.

- [x] Create a pretty-printer for ARC IR nodes (basic-block IR: `ArcFunction` → `ArcBlock` → `ArcInstr` → `ArcTerminator`) (2026-02-27)
  - `compiler/oric/src/arc_dump/mod.rs` — entry point, function-level formatting, helpers
  - `compiler/oric/src/arc_dump/instr.rs` — per-instruction and per-terminator formatting
  - Clones pre-lowered functions from `arc_cache` and runs full ARC pipeline for accurate RC display
  - Output follows LLVM IR / Rust MIR conventions: `fn @name(params) -> ret`, `bb0:`, `%var: type = instr`
- [x] Show RC strategy decisions for each value: (2026-02-27)
  - `ValueRepr` annotations on every variable: `[Scalar]`, `[RcPtr]`, `[FatVal]`, `[Aggregate]`
  - `RcStrategy` annotations on RC ops: `[HeapPtr]`, `[FatPtr]`, `[Closure]`, `[AggFields]`, `[InlineEnum]`
  - Ownership annotations on function params: `[own]`, `[borrow]`
- [x] Annotate each operation with its RC strategy: `[HeapPtr]`, `[FatPtr]`, `[Closure]`, `[AggFields]`, `[InlineEnum]` (2026-02-27)
- [x] Show explicit RC inc/dec operations and their targets (2026-02-27)
  - `RcInc %var [strategy]` with optional `xN` for batched increments
  - `RcDec %var [strategy]`
- [x] Show drop function assignments (which drop_fn is generated for which type) (2026-02-27)
  - Drop functions are implicit in `RcDec` strategy annotations (e.g., `[HeapPtr]` → `ori_rc_dec(ptr, drop_fn)`)
  - Reset/Reuse operations shown: `Reset`, `Reuse`, `IsShared`, `Set`, `SetTag`
- [x] Gate behind `dbg_do!(ORI_DUMP_AFTER_ARC, ...)` (2026-02-27)
  - Hook in `compile_common.rs::run_codegen_pipeline()` after `run_borrow_inference()` returns
  - Zero overhead in release builds (compiled out via `#[cfg(debug_assertions)]`)
  - Respects `ORI_DUMP_AFTER_ARC=0` to explicitly disable
- [x] Test: `ORI_DUMP_AFTER_ARC=1 ori build test.ori` shows ARC decisions (2026-02-27)
  - Verified with scalar-only (struct Point), RC (list push/length), and closure (map/fold with lambdas) programs
  - Verified gating: no output without env var, no output with =0
  - `./test-all.sh` green (10,366 tests, 0 failures)

---

## 03.5 ORI_DUMP_AFTER_LLVM — Enhanced LLVM IR Dump

**File(s):** `compiler/ori_llvm/src/evaluator/mod.rs`, `compiler/oric/src/commands/compile_common.rs`

Replace the existing `ORI_DEBUG_LLVM` with a richer dump that adds Ori-aware annotations to the raw LLVM IR. This is the "phase dump" version of Section 01's `ir-dump.sh`, but built into the compiler.

- [x] Migrate `ORI_DEBUG_LLVM` behavior into `ORI_DUMP_AFTER_LLVM` (2026-02-27)
- [x] Keep `ORI_DEBUG_LLVM` as an alias (backward compat) — same underlying flag (2026-02-27)
- [x] Add Ori function name annotations as comments: (2026-02-27)
  ```llvm
  ; --- @main ---
  define i64 @_ori_main() {
  ```
- [x] Add RC operation annotations: (2026-02-27)
  ```llvm
  call void @ori_rc_dec(ptr %data, ptr @drop_fn)  ; RC-- [int]
  call void @ori_rc_inc(ptr %data)                 ; RC++
  ```
- [x] Add COW operation annotations: (2026-02-27)
  ```llvm
  call void @ori_list_push_cow(...)                ; COW push list
  ```
- [x] Gate behind `llvm_dump_requested()` (checks both flags) in debug builds, env var check in evaluator (2026-02-27)
- [x] Test: `ORI_DUMP_AFTER_LLVM=1 ori build test.ori` produces annotated IR (2026-02-27)
  - Verified function name demangling: `_ori_main` → `@main`, drop functions → `drop [int]`
  - Verified RC annotations: `RC-- [int]` from drop function pool index resolution
  - Verified backward compat: `ORI_DEBUG_LLVM=1` also triggers enhanced dump
  - Verified gating: no output without env var, no output with `=0`
  - `./test-all.sh` green (10,366 tests, 0 failures)

---

## 03.6 Consistency Validation Script

**File(s):** `diagnostics/check-debug-flags.sh` (new script)

Following Roc's `ci/check_debug_vars.sh`, validate that all flags defined in `debug_flags.rs` are documented and that no stale flags exist in the codebase.

- [x] Create `diagnostics/check-debug-flags.sh` (2026-02-27)
  ```bash
  # Usage: diagnostics/check-debug-flags.sh
  # Validates: every ORI_* debug flag in debug_flags.rs is used somewhere
  # Validates: every ORI_* env var check in source references a flag in debug_flags.rs
  # Validates: CLAUDE.md documents all flags
  ```
- [x] Parse `debug_flags.rs` for defined flag names (2026-02-27)
- [x] Grep codebase for `std::env::var("ORI_` — verify all reference centralized flags (2026-02-27)
  - Correctly excludes non-diagnostic vars (ORI_STDLIB, ORI_WORKSPACE_DIR, ORI_SYSROOT, ORI_LOG*)
  - Correctly excludes test-only guard vars (ORI_RC_OVERFLOW_TEST, etc.)
- [x] Check CLAUDE.md "Commands" section lists all diagnostic env vars (2026-02-27)
  - Added phase dump, runtime debug, and consistency check documentation to CLAUDE.md
- [x] Report: stale flags (defined but unused), orphan checks (used but undefined), undocumented flags (2026-02-27)
- [x] Test: run on current codebase, verify clean output after migration (2026-02-27)
  - 8 defined flags, 0 stale, 0 orphan, 0 undocumented — all checks pass

---

## 03.7 Completion Checklist

- [x] `debug_flags.rs` defines all diagnostic env vars in one file (2026-02-27)
  - 8 flags: 4 phase dumps + 1 legacy alias + 3 runtime flags
- [x] `dbg_set!` / `dbg_do!` macros work correctly (true in debug, false in release) (2026-02-27)
  - `#[cfg(debug_assertions)]` gating verified — entire block removed in release
- [x] Existing `ORI_DEBUG_LLVM` migrated to centralized flag (2026-02-27)
  - Both `oric` (compile_common.rs) and `ori_llvm` (evaluator) now check both flags
- [x] `ORI_DUMP_AFTER_PARSE=1` produces readable AST dump (2026-02-27)
- [x] `ORI_DUMP_AFTER_TYPECK=1` produces typed IR dump with resolved methods (2026-02-27)
- [x] `ORI_DUMP_AFTER_ARC=1` produces ARC IR with RC strategy annotations (2026-02-27)
- [x] `ORI_DUMP_AFTER_LLVM=1` produces annotated LLVM IR (superset of `ORI_DEBUG_LLVM`) (2026-02-27)
  - Demangled Ori function names, drop function type resolution, RC++/RC-- with type, COW ops
- [x] `diagnostics/check-debug-flags.sh` validates flag consistency (2026-02-27)
  - 8 defined, 0 stale, 0 orphan, 0 undocumented
- [x] Zero overhead in release builds (all `dbg_do!` calls compile-time eliminated) (2026-02-27)
- [x] `./test-all.sh` green (2026-02-27)
  - 10,366 tests, 0 failures
- [x] All flags documented in CLAUDE.md and .claude/rules/ (2026-02-27)
  - CLAUDE.md: phase dumps, runtime debug, consistency sections added
  - .claude/rules/llvm.md: ORI_DUMP_AFTER_LLVM + ORI_DUMP_AFTER_ARC added
  - .claude/rules/arc.md: updated from ORI_DEBUG_LLVM to ORI_DUMP_AFTER_LLVM

**Exit Criteria:** Running `ORI_DUMP_AFTER_PARSE=1 ORI_DUMP_AFTER_TYPECK=1 ORI_DUMP_AFTER_ARC=1 ORI_DUMP_AFTER_LLVM=1 ori build test.ori` produces four clearly-separated dump sections showing the program's transformation through each compiler phase. The ARC IR dump for `[1,2].push(3).reverse()` clearly shows which RC operations are emitted and in what order, making the double-free bug immediately visible.
