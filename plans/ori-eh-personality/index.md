# Ori EH Personality Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: C Personality Function
**File:** `section-01-personality-fn.md` | **Status:** Not Started

```
ori_eh_personality, personality function, Itanium EH ABI, DWARF, LSDA
exception handling, unwinding, landingpad, invoke, cleanup, catch-all
_Unwind_RaiseException, _Unwind_SetGR, _Unwind_SetIP, _URC_INSTALL_CONTEXT
_UA_FORCE_UNWIND, _Unwind_ForcedUnwind, forced unwind, ip - 1
ULEB128, SLEB128, call-site table, action table, ttype_index
gcc_personality_v0.c, rust_eh_personality, __gxx_personality_v0
ori_rt, build.rs, cc crate, staticlib, forced_unwind.rs, test_frames_x86_64.S, test_frames_aarch64.S
```

---

### Section 02: Codegen Integration
**File:** `section-02-codegen-integration.md` | **Status:** Not Started

```
rust_eh_personality, ori_eh_personality, symbol rename, codegen
runtime_decl, RT_FUNCTIONS, arc_emitter, set_personality
evaluator/runtime_mappings.rs, jit_symbol_mappings, verify/tests.rs
ir_builder, calls.rs, invoke, landingpad, resume
nounwind, nounwind_functions, InvokeMode
```

---

### Section 03: Verification
**File:** `section-03-verification.md` | **Status:** Not Started

```
test-all.sh, llvm-test.sh, cargo st, AOT, JIT
code-journey, panic, catch, landing pad, unwind
valgrind, valgrind-aot.sh, dual-exec-verify.sh
libori_rt.a, nm, objdump, symbol table
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | C Personality Function | `section-01-personality-fn.md` |
| 02 | Codegen Integration | `section-02-codegen-integration.md` |
| 03 | Verification | `section-03-verification.md` |
