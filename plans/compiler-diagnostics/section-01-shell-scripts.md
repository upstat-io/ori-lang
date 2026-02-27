---
section: "01"
title: Shell Script Toolkit
status: complete
goal: "Six diagnostic scripts in diagnostics/ that use existing infra — zero compiler changes"
inspired_by:
  - "Zig tools/ directory (40+ diagnostic utilities)"
  - "Go GOSSAFUNC HTML SSA visualization"
  - "Roc devtools/lldb_line_tracer.py (LLDB-based tracing)"
depends_on: []
sections:
  - id: "01.1"
    title: "ir-dump.sh"
    status: complete
  - id: "01.2"
    title: "ir-diff.sh"
    status: complete
  - id: "01.3"
    title: "disasm-ori.sh"
    status: complete
  - id: "01.4"
    title: "rc-stats.sh"
    status: complete
  - id: "01.5"
    title: "diagnose-aot.sh"
    status: complete
  - id: "01.6"
    title: "dual-exec-debug.sh"
    status: complete
  - id: "01.7"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Shell Script Toolkit

**Status:** Not Started
**Goal:** Six executable diagnostic scripts in `diagnostics/` that leverage existing infrastructure (`ORI_DEBUG_LLVM`, `ORI_CHECK_LEAKS`, `objdump`, `valgrind`, `diff`) to provide immediate debugging value with zero compiler changes.

**Context:** Debugging the `push(3).reverse()` AOT bug required: (1) manually writing `ORI_DEBUG_LLVM=1 ori build ... 2>/tmp/ir.ll`, (2) reading 80+ lines of raw IR, (3) manually tracing value flow through SSA, (4) comparing with a working program's IR by eye. Each of these steps should be a one-line script invocation.

**Reference implementations:**
- **Zig** `tools/`: 40+ diagnostic utilities including `print_zir.zig` (ZIR pretty-printer), GDB/LLDB pretty-printers
- **Go** `GOSSAFUNC`: Dumps SSA passes to interactive HTML — each transformation captured
- **Roc** `devtools/lldb_line_tracer.py`: LLDB-based line-level execution tracing

**Depends on:** Nothing — pure shell scripts using existing capabilities.

---

## 01.1 ir-dump.sh — Annotated LLVM IR Dump

**File(s):** `diagnostics/ir-dump.sh`

Compile an `.ori` file and dump its LLVM IR to stdout with annotations. Wraps `ORI_DEBUG_LLVM=1 ori build` but adds:
- Section headers separating Ori functions from runtime declarations
- Color-coded RC operations (`ori_rc_inc` = green, `ori_rc_dec` = red, `ori_rc_alloc` = blue)
- Line numbering for easy reference in discussions
- Optional `--raw` flag to skip annotations

- [x] Create `diagnostics/ir-dump.sh` with argument parsing (2026-02-27)
  ```bash
  # Usage: diagnostics/ir-dump.sh [--raw] [--color] <file.ori>
  # Output: Annotated LLVM IR to stdout
  # Deps: ori compiler binary, sed/awk for annotation
  ```
- [x] Implement IR capture via `ORI_DEBUG_LLVM=1 ori build` stderr redirect (2026-02-27)
- [x] Add section separation: split IR at `define` and `declare` boundaries (2026-02-27)
- [x] Add color-coding for RC operations (optional, detect terminal) (2026-02-27)
- [x] Add `--raw` flag to bypass all annotation (2026-02-27)
- [x] Test on 3 programs: simple (hello), medium (list ops), complex (closures + COW) (2026-02-27)

---

## 01.2 ir-diff.sh — IR Comparison Between Programs

**File(s):** `diagnostics/ir-diff.sh`

Compare the LLVM IR of two `.ori` programs side by side. This is the highest-value script — the immediate need when debugging "program A works but program B doesn't" (like push-only vs push+reverse).

- [x] Create `diagnostics/ir-diff.sh` with two file arguments (2026-02-27)
  ```bash
  # Usage: diagnostics/ir-diff.sh <a.ori> <b.ori> [--function <name>]
  # Output: Colorized diff of LLVM IR
  # Deps: ir-dump.sh, diff (or delta if available)
  ```
- [x] Compile both files and capture IR using `ir-dump.sh --raw` (2026-02-27)
- [x] Normalize IR for meaningful diff (strip alloca names with counters, normalize block labels) (2026-02-27)
- [x] Optional `--function <name>` to extract and compare a single function (2026-02-27)
- [x] Use `diff --color` or `delta` (if available) for output (2026-02-27)
- [x] Test: diff `[1,2].push(3)` vs `[1,2].push(3).reverse()` — should highlight reverse call and extra RC ops (2026-02-27)

---

## 01.3 disasm-ori.sh — Ori-Aware Native Disassembly

**File(s):** `diagnostics/disasm-ori.sh`

Compile an `.ori` file to a native binary, then disassemble with Ori-aware demangling. Filters out libc/runtime noise to show only Ori functions.

- [x] Create `diagnostics/disasm-ori.sh` with file argument (2026-02-27)
  ```bash
  # Usage: diagnostics/disasm-ori.sh <file.ori> [--all] [--function <name>]
  # Output: Demangled disassembly of Ori functions
  # Deps: ori compiler, objdump
  ```
- [x] Compile to native binary via `ori build` (2026-02-27)
- [x] Run `objdump -d` and filter to `_ori_*` symbols (2026-02-27)
- [x] Demangle Ori symbols: `_ori_main` → `@main`, `_ori_math$add` → `math.add`, `_ori_int$$Eq$equals` → `int impl Eq.equals` (2026-02-27)
- [x] Optional `--all` to include runtime functions (`ori_rc_*`, `ori_list_*`, etc.) (2026-02-27)
- [x] Optional `--function` to show only one function (2026-02-27)
- [x] Test on a simple AOT binary (2026-02-27)

---

## 01.4 rc-stats.sh — RC Operation Counting in IR

**File(s):** `diagnostics/rc-stats.sh`

Count and summarize RC operations in LLVM IR. Answers: "How many retains/releases does this program generate? Where are the hotspots?"

- [x] Create `diagnostics/rc-stats.sh` with file argument (2026-02-27)
  ```bash
  # Usage: diagnostics/rc-stats.sh <file.ori>
  # Output: RC operation summary (alloc/inc/dec/free counts per function)
  # Deps: ir-dump.sh, grep, awk
  ```
- [x] Capture IR and grep for `ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_free` (2026-02-27)
- [x] Also count COW operations: `ori_list_push_cow`, `ori_list_reverse_cow`, etc. (2026-02-27)
- [x] Group counts per function (parse `define` blocks) (2026-02-27)
- [x] Output summary table: (2026-02-27)
  ```
  Function            alloc  inc  dec  free  cow_ops  balance
  @main                  2    1    3     2       2      -2
  _ori_drop$96           0    0    0     1       0      -1
  TOTAL                  2    1    3     3       2      -3
  ```
- [x] Flag imbalanced functions (alloc+inc != dec+free) with warning (2026-02-27)
- [x] Test on programs with known RC patterns (2026-02-27)

---

## 01.5 diagnose-aot.sh — All-in-One AOT Diagnostic

**File(s):** `diagnostics/diagnose-aot.sh`

The "run everything" diagnostic. Takes an `.ori` file and runs every available diagnostic tool in sequence, generating a comprehensive report.

- [x] Create `diagnostics/diagnose-aot.sh` with file argument (2026-02-27)
  ```bash
  # Usage: diagnostics/diagnose-aot.sh <file.ori> [--verbose] [--valgrind]
  # Output: Multi-section diagnostic report
  # Deps: ir-dump.sh, rc-stats.sh, disasm-ori.sh, ori, valgrind (optional)
  ```
- [x] **Section 1: Compilation** — Build with `ori build`, capture exit code, build time (2026-02-27)
- [x] **Section 2: Execution** — Run binary, capture exit code, stdout, stderr (2026-02-27)
- [x] **Section 3: Leak Check** — Run with `ORI_CHECK_LEAKS=1`, report RC balance (2026-02-27)
- [x] **Section 4: RC Stats** — Run `rc-stats.sh`, show summary and any imbalances (2026-02-27)
- [x] **Section 5: LLVM IR** — Run `ir-dump.sh`, save to temp file, show path (2026-02-27)
- [x] **Section 6: Valgrind** (if `--valgrind` flag) — Run under Valgrind, report errors (2026-02-27)
- [x] **Section 7: Disassembly** (if `--verbose` flag) — Run `disasm-ori.sh`, save to temp file (2026-02-27)
- [x] Output a clear pass/fail summary at the end: (2026-02-27)
  ```
  ═══ Diagnostic Report: test.ori ═══
  ✓ Compilation     (0.24s)
  ✓ Execution       exit=0
  ✗ Leak Check      2 RC allocations not freed
  ⚠ RC Balance      @main: alloc=3 free=1 (imbalance: +2)
  ─ LLVM IR         saved to /tmp/diag-test-ir.ll
  ─ Valgrind        skipped (use --valgrind)
  ```
- [x] Test on: clean program (all pass), leaky program (leak detected), crashing program (execution fails) (2026-02-27)

---

## 01.6 dual-exec-debug.sh — Backend Comparison with State

**File(s):** `diagnostics/dual-exec-debug.sh`

Extends the existing `scripts/dual-exec-verify.sh` concept. Runs a program through both interpreter and AOT, comparing not just exit codes but also stdout output. When results differ, dumps diagnostic info for both paths.

- [x] Create `diagnostics/dual-exec-debug.sh` with file argument (2026-02-27)
  ```bash
  # Usage: diagnostics/dual-exec-debug.sh <file.ori> [--verbose]
  # Output: Side-by-side comparison of interpreter vs AOT execution
  # Deps: ori (interpreter), ori build (AOT), diff
  ```
- [x] Run `ori run <file>` — capture exit code + stdout + stderr (2026-02-27)
- [x] Run `ori build <file> -o /tmp/...` then execute — capture exit code + stdout + stderr (2026-02-27)
- [x] Compare exit codes: match = pass, mismatch = investigate (2026-02-27)
- [x] Compare stdout: diff output line by line (2026-02-27)
- [x] On mismatch: automatically run `ir-dump.sh` and `rc-stats.sh` on the file (2026-02-27)
- [x] Optional `--verbose` adds `ORI_LOG=debug` to both runs for trace comparison (2026-02-27)
- [x] Output summary: (2026-02-27)
  ```
  ═══ Dual Execution: test.ori ═══
  Interpreter:  exit=0  stdout="3\n"  (0.05s)
  AOT:          exit=1  stdout=""     (0.02s)
  MISMATCH: exit codes differ (interpreter=0, AOT=1)

  Auto-diagnostics:
  - LLVM IR saved to /tmp/dual-diag-ir.ll
  - RC Stats: @main alloc=2 free=0 (imbalance!)
  ```
- [x] Test on: matching program, known-mismatching program (like the push+reverse bug) (2026-02-27)

---

## 01.7 Completion Checklist

- [x] All 6 scripts created in `diagnostics/` directory (2026-02-27)
- [x] All scripts are executable (`chmod +x`) (2026-02-27)
- [x] All scripts have `--help` / usage output (2026-02-27)
- [x] All scripts handle missing arguments gracefully (helpful error, not crash) (2026-02-27)
- [x] All scripts handle compilation failures gracefully (report error, not crash) (2026-02-27)
- [x] Tested `ir-dump.sh` on 3+ programs with different complexity levels (2026-02-27)
- [x] Tested `ir-diff.sh` on a pair of programs with known IR differences (2026-02-27)
- [x] Tested `diagnose-aot.sh` on clean, leaky, and crashing programs (2026-02-27)
- [x] Tested `dual-exec-debug.sh` on matching and mismatching programs (2026-02-27)
- [x] `./test-all.sh` still green (scripts don't interfere with existing tests) (2026-02-27)
- [x] Scripts documented in CLAUDE.md (Commands + Key Paths), .claude/rules/{llvm,aot,arc,runtime}.md (2026-02-27)

**Exit Criteria:** All 6 scripts in `diagnostics/` are executable, self-documenting (--help), handle errors gracefully, and produce useful output for the `push(3).reverse()` debugging scenario that motivated this plan. Running `diagnostics/diagnose-aot.sh` on the failing test case produces a report that identifies the RC imbalance.
