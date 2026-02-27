---
plan: "compiler-diagnostics"
title: "Compiler Diagnostics Toolkit: Exhaustive Implementation Plan"
status: not-started
references:
  - "scripts/dual-exec-verify.sh"
  - "scripts/valgrind-aot.sh"
  - "scripts/perf-baseline.sh"
---

# Compiler Diagnostics Toolkit: Exhaustive Implementation Plan

## Mission

Build a comprehensive diagnostic toolkit for the Ori compiler that accelerates debugging across all phases — from LLVM IR inspection through RC lifecycle tracing to runtime memory analysis. Today, debugging a chained-operation bug (like `push(3).reverse()` producing garbage in AOT but working in the interpreter) requires manual IR dumps, reading C runtime source, and correlating values by hand. This plan delivers tools that automate these workflows: dump, diff, trace, audit, and attribute — so bugs that currently take 30+ minutes to diagnose become 2-minute script invocations.

## Architecture

```
diagnostics/                     Shell scripts (no compiler changes)
├── ir-dump.sh                   Annotated LLVM IR dump for any .ori file
├── ir-diff.sh                   Side-by-side IR comparison of two programs
├── disasm-ori.sh                Ori-aware native disassembly with demangling
├── rc-stats.sh                  Count RC operations in LLVM IR
├── codegen-audit.sh             Static analysis of RC balance in IR
├── diagnose-aot.sh              All-in-one AOT diagnostic (IR + leak + valgrind)
└── dual-exec-debug.sh           Backend comparison with intermediate state

compiler/ori_rt/src/             Runtime instrumentation (C-ABI changes)
├── rc_trace.rs                  ORI_TRACE_RC event logging
├── leak_attribution.rs          Allocation-site tracking for leak reports
└── lib.rs                       ORI_RT_DEBUG assertion mode, underflow detection

compiler/oric/src/               Phase dump system (compiler changes)
├── debug_flags.rs               Centralized flag registry (Roc pattern)
└── commands/compile_common.rs   Phase-specific IR dump hooks

compiler/ori_llvm/src/           ARC IR visibility
└── codegen/arc_emitter/         ARC IR pretty-printer for intermediate dump
```

**Data flow for a typical diagnostic session:**

```
  .ori file
     │
     ├─► ir-dump.sh ──► annotated LLVM IR on stdout
     │
     ├─► diagnose-aot.sh ──► IR + disasm + leak check + valgrind (all-in-one)
     │
     ├─► ORI_TRACE_RC=1 ──► RC event log: alloc/inc/dec/free with addresses
     │
     ├─► ORI_DUMP_AFTER=arc ──► ARC IR pretty-printed to stderr
     │
     └─► ir-diff.sh a.ori b.ori ──► side-by-side IR delta
```

## Design Principles

**1. Zero-change scripts first.** The shell scripts in Section 01 use only existing capabilities (`ORI_DEBUG_LLVM`, `ORI_CHECK_LEAKS`, `objdump`, `valgrind`). They deliver immediate value with no compiler modifications. Every subsequent section builds on these scripts.

**2. Opt-in instrumentation, zero production overhead.** Runtime tracing (Section 02) and phase dumps (Section 03) are gated behind environment variables. When disabled: zero branches, zero allocations, zero syscalls. The `dbg_set!` / `dbg_do!` macro pattern (from Roc) ensures flags are compile-time eliminated in release builds.

**3. Attribution over counting.** `ORI_CHECK_LEAKS` tells you *how many* allocations leaked. The new tools tell you *which* ones and *why*. Allocation-site IDs, source spans in RC operations, and type tags in debug headers transform "3 leaks detected" into "leak at list_push_cow:1423, type=[int], allocated by @main line 5".

## Section Dependency Graph

```
Section 01 (Shell Scripts)         ◄── no dependencies, immediate value
     │
     ├────────────────┐
     ▼                ▼
Section 02         Section 03      ◄── independent of each other
(Runtime RC)       (Phase Dumps)
     │                │
     └───────┬────────┘
             ▼
Section 04 (Codegen Audit)         ◄── uses scripts + runtime + phase dumps
             │
             ▼
Section 05 (Verification)          ◄── tests everything together
```

- **Section 01** is fully independent — pure shell scripts using existing infra.
- **Sections 02 and 03** are independent of each other but both benefit from Section 01 scripts.
- **Section 04** combines shell analysis with runtime data for codegen auditing.
- **Section 05** verifies the whole toolkit and integrates into CI.

**Cross-section interactions:**
- **Section 01 + Section 02**: `diagnose-aot.sh` gains an `--rc-trace` flag once Section 02 lands.
- **Section 02 + Section 04**: `codegen-audit.sh` cross-references static IR analysis with runtime RC trace for ground-truth validation.

## Implementation Sequence

```
Phase 1 — Shell Script Foundation  (Section 01)
  └─ 01.1: ir-dump.sh — annotated LLVM IR dump
  └─ 01.2: ir-diff.sh — IR comparison between programs
  └─ 01.3: disasm-ori.sh — Ori-aware disassembly
  └─ 01.4: rc-stats.sh — RC operation counting in IR
  └─ 01.5: diagnose-aot.sh — all-in-one AOT diagnostic
  └─ 01.6: dual-exec-debug.sh — backend comparison with state
  Gate: All scripts executable, tested on 3+ sample programs

Phase 2a — Runtime RC Instrumentation  (Section 02)
  └─ 02.1: ORI_TRACE_RC event logging in ori_rt
  └─ 02.2: Allocation-site attribution (leak-where)
  └─ 02.3: ORI_RT_DEBUG assertion mode
  └─ 02.4: Release-mode underflow detection
  Gate: RC trace captures alloc→inc→dec→free sequence correctly

Phase 2b — Phase Dump System  (Section 03)  [parallel with 2a]
  └─ 03.1: Centralized debug_flags module (Roc pattern)
  └─ 03.2: ORI_DUMP_AFTER_PARSE — AST dump
  └─ 03.3: ORI_DUMP_AFTER_TYPECK — typed IR dump
  └─ 03.4: ORI_DUMP_AFTER_ARC — ARC IR pretty-printer
  └─ 03.5: ORI_DUMP_AFTER_LLVM — enhanced LLVM IR dump (replaces ORI_DEBUG_LLVM)
  └─ 03.6: Consistency validation script
  Gate: Each dump flag produces readable output for sample programs

Phase 3 — Codegen Audit & Analysis  (Section 04)
  └─ 04.1: Static RC balance analysis on LLVM IR
  └─ 04.2: COW operation correctness verification
  └─ 04.3: ABI conformance checking
  Gate: codegen-audit.sh catches known-bad IR patterns

Phase 4 — Verification & Integration  (Section 05)
  └─ 05.1: Test all scripts on representative programs
  └─ 05.2: Document in CLAUDE.md / .claude/rules/
  └─ 05.3: CI integration (optional diagnostic mode)
  Gate: ./test-all.sh green, all scripts pass self-tests
```

**Why this order:**
- Phase 1 delivers value immediately with no compiler changes.
- Phases 2a and 2b are independent and can be worked in parallel.
- Phase 3 uses tools from all prior phases.
- Phase 4 tests the whole toolkit as a system.

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Shell Script Toolkit | ~600 | Low | — |
|   ↳ 01.1 ir-dump.sh | ~80 | Low | — |
|   ↳ 01.2 ir-diff.sh | ~100 | Low | — |
|   ↳ 01.3 disasm-ori.sh | ~60 | Low | — |
|   ↳ 01.4 rc-stats.sh | ~50 | Low | — |
|   ↳ 01.5 diagnose-aot.sh | ~150 | Low | — |
|   ↳ 01.6 dual-exec-debug.sh | ~160 | Medium | — |
| 02 Runtime RC Instrumentation | ~400 | Medium | — |
|   ↳ 02.1 RC event tracing | ~150 | Medium | — |
|   ↳ 02.2 Leak attribution | ~120 | Medium | 02.1 |
|   ↳ 02.3 Runtime assertion mode | ~80 | Low | — |
|   ↳ 02.4 Underflow detection | ~50 | Low | — |
| 03 Phase Dump System | ~500 | Medium | — |
|   ↳ 03.1 Debug flags module | ~80 | Low | — |
|   ↳ 03.2-03.5 Phase dump hooks | ~300 | Medium | 03.1 |
|   ↳ 03.6 Validation script | ~120 | Low | 03.1 |
| 04 Codegen Audit | ~350 | High | 01, 02, 03 |
| 05 Verification | ~200 | Low | all |
| **Total new** | **~2,050** | | |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Shell Script Toolkit | `section-01-shell-scripts.md` | Not Started |
| 02 | Runtime RC Instrumentation | `section-02-runtime-instrumentation.md` | Not Started |
| 03 | Phase Dump System | `section-03-phase-dumps.md` | Not Started |
| 04 | Codegen Audit & Analysis | `section-04-codegen-audit.md` | Not Started |
| 05 | Verification & Integration | `section-05-verification.md` | Not Started |
