---
plan: "ori-eh-personality"
title: "Ori Exception Handling Personality: Exhaustive Implementation Plan"
status: not-started
reviewed: false
references:
  - "plans/code-journeys/journey3-results.md"
  - "compiler/ori_llvm/src/codegen/eh_model/mod.rs"
  - "compiler/ori_llvm/src/evaluator/runtime_mappings.rs"
  - "compiler/ori_rt/src/io.rs"
  - "compiler/ori_rt/src/lib.rs"
  - "~/projects/reference_repos/lang_repos/rust/library/std/src/sys/personality/gcc.rs"
  - "~/projects/reference_repos/lang_repos/zig/lib/libunwind/src/gcc_personality_v0.c"
---

# Ori Exception Handling Personality: Exhaustive Implementation Plan

## Mission

Complete Ori-owned exception handling on Itanium EH platforms end-to-end: emit `@ori_eh_personality` in LLVM IR, raise exceptions via `_Unwind_RaiseException` (not Rust `panic_any`) for AOT Itanium paths, and free caught exception objects correctly.

This plan includes codegen integration, runtime exception lifecycle ownership, Windows MSVC compatibility boundaries, and full verification.

## Architecture

```
                            ori_llvm codegen
┌─────────────────────────────────────────────────────────────────────────────┐
│ EH model                                                                    │
│   EhModel::Itanium -> "ori_eh_personality"                                 │
│   EhModel::Seh     -> "__CxxFrameHandler3"                                 │
│                                                                             │
│ runtime_decl/RT_FUNCTIONS -> declares personality symbols for LLVM          │
│ arc_emitter -> attaches eh_model.personality_name() to invoke-bearing fns   │
│ evaluator/runtime_mappings -> JIT symbol address mapping                     │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
                          ori_rt runtime (Rust + C)
┌─────────────────────────────────────────────────────────────────────────────┐
│ eh_personality.c                                                            │
│   - ori_eh_personality (LSDA parser; cleanup + catch-all)                  │
│   - ori_raise_exception (_Unwind_RaiseException + OriException object)      │
│                                                                             │
│ io.rs                                                                        │
│   - ori_panic / ori_panic_cstr                                              │
│   - ori_catch_cleanup (_Unwind_DeleteException)                              │
│   - ori_try_call (MSVC SEH compatibility path)                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
                                 Verification
                      tests + IR audit + symbol audit + valgrind
```

## Design Principles

1. **Ori-owned symbols in Ori-generated IR.**
All Itanium unwind-capable functions emitted by Ori must reference `@ori_eh_personality`, not `@rust_eh_personality`.

2. **Runtime-owned exception lifecycle.**
If Ori raises exceptions, Ori must own allocation, exception class, cleanup callback, and catch cleanup (`_Unwind_DeleteException`) to avoid leaks/abort behavior tied to Rust panic internals.

3. **Platform-correct boundary, not wishful unification.**
This plan targets Itanium EH behavior first. Windows MSVC remains SEH + `ori_try_call` compatibility unless explicitly migrated in this plan. No silent cross-platform regressions.

## Platform Scope

- **In scope:** Linux/macOS/MinGW (Itanium EH model).
- **Compatibility scope:** Windows MSVC (SEH path kept valid; document and gate any behavior differences).
- **Out of scope:** Full SEH-native Ori exception object integration replacing `catch_unwind` on MSVC.

## Section Dependency Graph

```
Section 01 (C Personality Function)
  ├──→ Section 02 (Codegen Integration)
  └──→ Section 03 (Ori Exception Raise)
             │
             ▼
      Section 04 (Native Exception Catch)
             │
             ▼
      Section 05 (Verification)

Section 05 also depends on Section 02.
```

- Sections 02 and 03 can start in parallel after Section 01 lands.
- Section 04 requires Section 03 (new exception object + cleanup callback must exist first).
- Section 05 requires Sections 01-04 complete.

**Cross-section interactions (must be co-implemented):**
- **Section 02 + Section 03:** swapping personality symbol without switching raise path leaves mixed exception behavior and stale runtime assumptions.
- **Section 03 + Section 04:** introducing Ori exception objects without catch cleanup leaks memory; catch cleanup must land with new raise path.
- **Section 02 + Section 05:** EH model tests and runtime mapping parity must verify both JIT and AOT paths.

## Implementation Sequence

```
Phase 0 - Baseline Audit
  └─ Record current symbol/state baselines (`rust_eh_personality`, `panic_any` counts)

Phase 1 - Runtime Personality Foundation (Section 01)
  └─ Add C personality + LSDA parsing + build integration + forced-unwind tests
  Gate: `ori_eh_personality` exported; forced-unwind tests pass/skipped by target

Phase 2 - Codegen Symbol Integration (Section 02)
  └─ Update RT_FUNCTIONS, eh_model personality name, JIT mapping, verify fixture
  Gate: emitted IR uses `@ori_eh_personality` on Itanium paths

Phase 3 - Raise Path Migration (Section 03)
  └─ Add `ori_raise_exception`; migrate Itanium AOT panic path from `panic_any`
  Gate: Itanium AOT panic path raises via `_Unwind_RaiseException`

Phase 4 - Catch Cleanup & MSVC Boundary (Section 04)
  └─ Implement `_Unwind_DeleteException` cleanup + lock down ori_try_call role
  Gate: catch path frees exceptions; MSVC behavior explicitly documented/guarded

Phase 5 - Full Verification (Section 05)
  └─ Full tests, IR/symbol audits, valgrind, lifecycle checks
  Gate: all exit criteria met with no regressions
```

**Why this order:**
- Phase 1 creates runtime primitives before codegen references them.
- Phase 2 ensures compiler output points to Ori symbols before changing raise semantics.
- Phase 3 and 4 complete exception lifecycle ownership.
- Phase 5 validates behavior across JIT/AOT and target EH models.

**Known transitional failures (expected mid-plan):**
- After Section 02 but before Section 01 complete: unresolved `ori_eh_personality` symbol.
- After Section 03 without Section 04: caught exceptions may leak if `_Unwind_DeleteException` path not landed.
- During MSVC gating refactor: temporary test adjustments may be required around `ori_try_call`/`catch_unwind` assumptions.

## Metrics (Current State)

| Metric | Baseline |
|--------|----------|
| `rust_eh_personality` references in `compiler/ori_llvm/src` + `compiler/ori_rt/src` | 11 |
| `panic_any` references in `compiler/ori_rt/src` | 5 |
| Relevant production LOC touched by this plan (`ori_rt` + `ori_llvm` core files) | ~3,591 |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 C Personality Function | ~420 | Medium | — |
| 02 Codegen Integration | ~60 | Medium | 01 |
| 03 Ori Exception Raise | ~120 | Medium | 01 |
| 04 Native Exception Catch | ~60 | Medium | 01, 03 |
| 05 Verification | ~0 code / high validation effort | Medium | 01-04 |
| **Total new/changed code** | **~660** | | |

## Known Bugs / Risks (Pre-implementation)

| Bug / Risk | Root Cause | Fix Location | Status |
|------------|-----------|-------------|--------|
| `ori_catch_cleanup` leak | Rust panic exception object cannot be safely deleted externally | Section 04 | Not Started |
| MSVC catch compatibility coupling | `ori_try_call` depends on `catch_unwind` semantics | Section 04 | Not Started |
| EH symbol drift | `runtime_decl`, `eh_model`, JIT mapping, tests can diverge | Section 02 + 05 | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | C Personality Function | `section-01-personality-fn.md` | Not Started |
| 02 | Codegen Integration | `section-02-codegen-integration.md` | Not Started |
| 03 | Ori Exception Raise | `section-03-exception-raise.md` | Not Started |
| 04 | Native Exception Catch | `section-04-exception-catch.md` | Not Started |
| 05 | Verification | `section-05-verification.md` | Not Started |
