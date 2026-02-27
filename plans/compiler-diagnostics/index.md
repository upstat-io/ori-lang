# Compiler Diagnostics Toolkit Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F / Cmd+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Shell Script Toolkit
**File:** `section-01-shell-scripts.md` | **Status:** Not Started

```
ir-dump, ir-diff, LLVM IR, dump IR, compare IR
disasm, disassembly, objdump, demangling, symbol
rc-stats, RC count, refcount operations, retain, release
diagnose-aot, all-in-one, diagnostic, AOT debug
dual-exec-debug, backend comparison, interpreter vs LLVM
ORI_DEBUG_LLVM, valgrind, leak check, memory
diagnostics/, shell scripts, quick diagnostics
```

---

### Section 02: Runtime RC Instrumentation
**File:** `section-02-runtime-instrumentation.md` | **Status:** Not Started

```
ORI_TRACE_RC, RC trace, refcount log, event log
alloc, inc, dec, free, ori_rc_alloc, ori_rc_dec
leak attribution, allocation site, leak-where
ORI_RT_DEBUG, assertion mode, bounds check
underflow detection, double-free, use-after-free
ori_rt, runtime library, C ABI, extern "C"
RC header, strong_count, 8-byte header
```

---

### Section 03: Phase Dump System
**File:** `section-03-phase-dumps.md` | **Status:** Not Started

```
ORI_DUMP_AFTER, phase dump, IR dump, pipeline visibility
debug_flags, debug flags module, Roc pattern, dbg_set, dbg_do
ORI_DUMP_AFTER_PARSE, AST dump, parse tree
ORI_DUMP_AFTER_TYPECK, typed IR, type check dump
ORI_DUMP_AFTER_ARC, ARC IR, arc_emitter, RC strategy
ORI_DUMP_AFTER_LLVM, LLVM IR dump, enhanced dump
consistency validation, flag registry, check_debug_vars
oric, compile_common.rs, debug_flags.rs
```

---

### Section 04: Codegen Audit & Analysis
**File:** `section-04-codegen-audit.md` | **Status:** Not Started

```
codegen-audit, RC balance, static analysis
COW correctness, copy-on-write verification
ABI conformance, calling convention, Sret, Indirect
ori_rc_inc, ori_rc_dec, paired, balanced
double-free detection, missing drop, leak detection
LLVM IR analysis, grep IR, instruction count
```

---

### Section 05: Verification & Integration
**File:** `section-05-verification.md` | **Status:** Not Started

```
test scripts, self-test, diagnostic tests
CLAUDE.md, documentation, .claude/rules
CI integration, diagnostic mode, automated
test-all.sh, script testing, verification
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Shell Script Toolkit | `section-01-shell-scripts.md` |
| 02 | Runtime RC Instrumentation | `section-02-runtime-instrumentation.md` |
| 03 | Phase Dump System | `section-03-phase-dumps.md` |
| 04 | Codegen Audit & Analysis | `section-04-codegen-audit.md` |
| 05 | Verification & Integration | `section-05-verification.md` |
