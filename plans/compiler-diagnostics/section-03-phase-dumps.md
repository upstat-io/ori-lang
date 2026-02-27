---
section: "03"
title: Phase Dump System
status: not-started
goal: "Centralized debug flag registry + ORI_DUMP_AFTER_* phase dumps for parse/typeck/ARC/LLVM"
inspired_by:
  - "Roc debug_flags crate (centralized env var flags + dbg_set!/dbg_do! macros)"
  - "Rust -Zdump-mir (MIR dump at specific passes)"
  - "Go GOSSAFUNC (SSA phase visualization)"
depends_on: []
sections:
  - id: "03.1"
    title: "Centralized Debug Flags Module"
    status: not-started
  - id: "03.2"
    title: "ORI_DUMP_AFTER_PARSE"
    status: not-started
  - id: "03.3"
    title: "ORI_DUMP_AFTER_TYPECK"
    status: not-started
  - id: "03.4"
    title: "ORI_DUMP_AFTER_ARC"
    status: not-started
  - id: "03.5"
    title: "ORI_DUMP_AFTER_LLVM"
    status: not-started
  - id: "03.6"
    title: "Consistency Validation Script"
    status: not-started
  - id: "03.7"
    title: "Completion Checklist"
    status: not-started
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

- [ ] Create `compiler/oric/src/debug_flags.rs` with flag definitions
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
- [ ] Export macros and constants from `oric` crate
- [ ] Migrate existing `ORI_DEBUG_LLVM` checks in `evaluator/mod.rs` and `compile_common.rs` to use centralized flag
- [ ] Add `mod debug_flags;` to `compiler/oric/src/lib.rs`
- [ ] Test: verify existing `ORI_DEBUG_LLVM=1` behavior unchanged after migration

---

## 03.2 ORI_DUMP_AFTER_PARSE — AST Dump

**File(s):** `compiler/oric/src/commands/compile_common.rs` (add dump hook after parse phase)

Dump the parsed AST in a human-readable format. Shows the structure the parser produced before type checking.

- [ ] Add dump hook after `parse_module()` call in compile pipeline
- [ ] Use existing `Debug` impls on AST nodes, or add a simple pretty-printer
- [ ] Output format: indented tree showing function signatures, expression structure, pattern forms
  ```
  === AST after parse: test.ori ===
  Function @main () -> int
    Block
      LetBinding xs =
        MethodCall .reverse()
          MethodCall .push(3)
            ListLiteral [1, 2]
      MethodCall .length()
        Ident xs
  === END AST ===
  ```
- [ ] Gate behind `dbg_do!(ORI_DUMP_AFTER_PARSE, ...)`
- [ ] Test: `ORI_DUMP_AFTER_PARSE=1 ori check test.ori` produces readable AST

---

## 03.3 ORI_DUMP_AFTER_TYPECK — Typed IR Dump

**File(s):** `compiler/oric/src/commands/compile_common.rs` (add dump hook after type check phase)

Dump the type-annotated IR after type checking. Shows inferred types for all expressions, resolved method calls, and trait implementations.

- [ ] Add dump hook after type checking completes
- [ ] Output format: similar to AST dump but with type annotations on every node
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
- [ ] Show resolved method dispatch (builtin vs trait impl vs inherent)
- [ ] Show type variable unification results
- [ ] Gate behind `dbg_do!(ORI_DUMP_AFTER_TYPECK, ...)`
- [ ] Test: `ORI_DUMP_AFTER_TYPECK=1 ori check test.ori` shows types on all nodes

---

## 03.4 ORI_DUMP_AFTER_ARC — ARC IR Pretty-Printer

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/` (add pretty-printer for ARC IR)

This is the highest-value phase dump. The ARC IR is the intermediate form between typed Ori expressions and LLVM IR — it includes RC strategy decisions, drop placement, and COW operation selection. Today this IR exists only as in-memory Rust structs with no serialization.

- [ ] Create a pretty-printer for ARC IR nodes (`CanExpr` / ARC-lowered form)
- [ ] Show RC strategy decisions for each value:
  ```
  === ARC IR after lowering: @main ===
  %0 = list_alloc [1, 2] : [int]  → RC(heap), rc=1
  %1 = cow_push %0, 3 : [int]     → RC(heap), cow_mut
  %2 = rc_dec %0                   → drop original (if not reused by cow)
  %3 = cow_reverse %1 : [int]     → RC(heap), cow_mut
  %4 = rc_dec %1                   → drop push result
  %5 = list_length %3 : int       → trivial (no RC)
  %6 = rc_dec %3                   → drop reversed
  return %5
  === END ARC IR ===
  ```
- [ ] Annotate each operation with its RC strategy: `trivial`, `RC(heap)`, `RC(inline)`, `cow_mut`, `cow_copy`
- [ ] Show explicit RC inc/dec operations and their targets
- [ ] Show drop function assignments (which drop_fn is generated for which type)
- [ ] Gate behind `dbg_do!(ORI_DUMP_AFTER_ARC, ...)`
- [ ] Test: `ORI_DUMP_AFTER_ARC=1 ori build test.ori` shows ARC decisions

---

## 03.5 ORI_DUMP_AFTER_LLVM — Enhanced LLVM IR Dump

**File(s):** `compiler/ori_llvm/src/evaluator/mod.rs`, `compiler/oric/src/commands/compile_common.rs`

Replace the existing `ORI_DEBUG_LLVM` with a richer dump that adds Ori-aware annotations to the raw LLVM IR. This is the "phase dump" version of Section 01's `ir-dump.sh`, but built into the compiler.

- [ ] Migrate `ORI_DEBUG_LLVM` behavior into `ORI_DUMP_AFTER_LLVM`
- [ ] Keep `ORI_DEBUG_LLVM` as an alias (backward compat) — same underlying flag
- [ ] Add Ori function name annotations as comments:
  ```llvm
  ; === @main : () -> int ===
  define i64 @_ori_main() {
  ```
- [ ] Add RC operation annotations:
  ```llvm
  call void @ori_rc_dec(ptr %data, ptr @drop_fn)  ; RC-- list [int]
  call void @ori_rc_inc(ptr %data)                 ; RC++ list [int]
  ```
- [ ] Add COW operation annotations:
  ```llvm
  call void @ori_list_push_cow(...)                ; COW push [int], elem=i64
  ```
- [ ] Gate behind `dbg_do!(ORI_DUMP_AFTER_LLVM, ...)` in debug builds, env var check in release
- [ ] Test: `ORI_DUMP_AFTER_LLVM=1 ori build test.ori` produces annotated IR

---

## 03.6 Consistency Validation Script

**File(s):** `diagnostics/check-debug-flags.sh` (new script)

Following Roc's `ci/check_debug_vars.sh`, validate that all flags defined in `debug_flags.rs` are documented and that no stale flags exist in the codebase.

- [ ] Create `diagnostics/check-debug-flags.sh`
  ```bash
  # Usage: diagnostics/check-debug-flags.sh
  # Validates: every ORI_* debug flag in debug_flags.rs is used somewhere
  # Validates: every ORI_* env var check in source references a flag in debug_flags.rs
  # Validates: CLAUDE.md documents all flags
  ```
- [ ] Parse `debug_flags.rs` for defined flag names
- [ ] Grep codebase for `std::env::var("ORI_` — verify all reference centralized flags
- [ ] Check CLAUDE.md "Commands" section lists all diagnostic env vars
- [ ] Report: stale flags (defined but unused), orphan checks (used but undefined), undocumented flags
- [ ] Test: run on current codebase, verify clean output after migration

---

## 03.7 Completion Checklist

- [ ] `debug_flags.rs` defines all diagnostic env vars in one file
- [ ] `dbg_set!` / `dbg_do!` macros work correctly (true in debug, false in release)
- [ ] Existing `ORI_DEBUG_LLVM` migrated to centralized flag
- [ ] `ORI_DUMP_AFTER_PARSE=1` produces readable AST dump
- [ ] `ORI_DUMP_AFTER_TYPECK=1` produces typed IR dump with resolved methods
- [ ] `ORI_DUMP_AFTER_ARC=1` produces ARC IR with RC strategy annotations
- [ ] `ORI_DUMP_AFTER_LLVM=1` produces annotated LLVM IR (superset of `ORI_DEBUG_LLVM`)
- [ ] `diagnostics/check-debug-flags.sh` validates flag consistency
- [ ] Zero overhead in release builds (all `dbg_do!` calls compile-time eliminated)
- [ ] `./test-all.sh` green
- [ ] All flags documented in CLAUDE.md and .claude/rules/

**Exit Criteria:** Running `ORI_DUMP_AFTER_PARSE=1 ORI_DUMP_AFTER_TYPECK=1 ORI_DUMP_AFTER_ARC=1 ORI_DUMP_AFTER_LLVM=1 ori build test.ori` produces four clearly-separated dump sections showing the program's transformation through each compiler phase. The ARC IR dump for `[1,2].push(3).reverse()` clearly shows which RC operations are emitted and in what order, making the double-free bug immediately visible.
