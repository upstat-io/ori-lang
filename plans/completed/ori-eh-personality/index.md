---
reroute: true
name: "EH Personality"
full_name: "Ori EH Personality"
status: resolved
---

# Ori EH Personality Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: C Personality Function
**File:** `section-01-personality-fn.md` | **Status:** Complete

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
**File:** `section-02-codegen-integration.md` | **Status:** Complete

```
rust_eh_personality, ori_eh_personality, symbol rename, codegen
runtime_decl, RT_FUNCTIONS, eh_model, personality_name
evaluator/runtime_mappings.rs, jit_symbol_mappings, verify/tests.rs
codegen/eh_model/mod.rs, codegen/eh_model/tests.rs
arc_emitter, set_personality, invoke, landingpad, resume
nounwind, jit_allowed, mapping parity
```

---

### Section 03: Ori Exception Raise
**File:** `section-03-exception-raise.md` | **Status:** Complete

```
ori_raise_exception, _Unwind_RaiseException, ORI_EXCEPTION_CLASS
OriException, exception object, exception_cleanup, malloc, free
panic_any, std::panic, Rust panic, extern "C-unwind", noreturn
ori_panic, ori_panic_cstr, OriPanic, io.rs, lib.rs
target gating, windows-msvc, compatibility path
eh_personality.c, Itanium ABI, unwind
```

---

### Section 04: Native Exception Catch
**File:** `section-04-exception-catch.md` | **Status:** Complete

```
ori_catch_cleanup, _Unwind_DeleteException, exception lifecycle
memory leak, catch(expr:), landingpad catch null, ori_catch_recover
ori_try_call, catch_unwind, Windows MSVC, SEH, platform compatibility
exception_cleanup callback, free, OriException
```

---

### Section 05: Verification
**File:** `section-05-verification.md` | **Status:** Complete

```
test-all.sh, clippy-all.sh, fmt-all.sh, cargo build
ORI_DUMP_AFTER_LLVM, personality audit, JIT/AOT parity
valgrind, valgrind-aot.sh, catch lifecycle, zero leak
libori_rt.a, nm, rg, symbol/source audit
panic_any (msvc-only allowance), ori_raise_exception, ORI_EXCEPTION_CLASS
forced unwind, aarch64, x86-64, cfg skip behavior
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | C Personality Function | `section-01-personality-fn.md` |
| 02 | Codegen Integration | `section-02-codegen-integration.md` |
| 03 | Ori Exception Raise | `section-03-exception-raise.md` |
| 04 | Native Exception Catch | `section-04-exception-catch.md` |
| 05 | Verification | `section-05-verification.md` |

## Dependency Graph

```
01 C Personality ──→ 02 Codegen Integration ──→ 05 Verification
       │                                              ↑
       └──→ 03 Exception Raise ──→ 04 Exception Catch ┘
```

Section 01 is the foundation (personality + build system). Sections 02 and 03 can proceed in parallel after 01. Section 04 depends on 03 (needs the Ori exception object). Section 05 verifies everything.
