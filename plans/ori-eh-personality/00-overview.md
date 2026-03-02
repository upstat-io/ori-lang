---
plan: "ori-eh-personality"
title: "Ori Exception Handling Personality: Exhaustive Implementation Plan"
status: not-started
references:
  - "plans/code-journeys/journey3-results.md"
  - "~/projects/reference_repos/lang_repos/rust/library/std/src/sys/personality/gcc.rs"
  - "~/projects/reference_repos/lang_repos/zig/lib/libunwind/src/gcc_personality_v0.c"
---

# Ori Exception Handling Personality: Exhaustive Implementation Plan

## Mission

Replace the dependency on Rust's `rust_eh_personality` with Ori's own `ori_eh_personality` function, implemented in C within `ori_rt`. This makes Ori's generated LLVM IR and compiler source symbolically independent from Rust's exception handling infrastructure — the emitted IR references `@ori_eh_personality`, and the runtime provides a minimal Itanium EH ABI personality that handles cleanup and catch-all landing pads. (Note: `rust_eh_personality` may still appear in `libori_rt.a` from embedded Rust std internals — this is expected and does not affect Ori's generated code.)

**Motivation:** Code Journey 3 (J3) identified that every AOT function containing `invoke`/`landingpad` carries `personality ptr @rust_eh_personality`, making Ori binaries visibly dependent on Rust's unwind infrastructure. Ori is a standalone compiler — its generated code should reference its own symbols.

## Architecture

```
                      LLVM Codegen (ori_llvm)
                      ┌─────────────────────────┐
                      │  arc_emitter/mod.rs      │
  ARC IR functions    │    set_personality(       │
  with invoke/        │      "ori_eh_personality" │  ← was "rust_eh_personality"
  landingpad          │    )                      │
                      │                           │
                      │  runtime_decl/             │
                      │    runtime_functions.rs:    │
                      │    "ori_eh_personality"    │  ← declaration for LLVM
                      └───────────┬───────────────┘
                                  │
                    ┌─────────────┴─────────────┐
                    │                           │
              JIT path                    AOT path
              (evaluator/                 (linker)
               runtime_mappings.rs)
                    │                           │
                    ▼                           ▼
        jit_symbol_mappings()           libori_rt.a
        maps "ori_eh_personality"       contains ori_eh_personality()
        → address of C function         (compiled from eh_personality.c)
        in ori_rt rlib                  via cc crate in build.rs
```

## Design Principles

**1. Symbol independence — Ori references only Ori symbols.**
The LLVM IR emitted by Ori's codegen must not contain references to symbols from other language runtimes. `@ori_eh_personality` instead of `@rust_eh_personality` is both a practical necessity (distribution without Rust) and an identity statement (Ori is its own language).

**2. Minimum viable personality — only what Ori actually uses.**
Ori's landing pads use exactly two patterns: `landingpad cleanup` (ARC cleanup + resume) and `landingpad catch ptr null` (catch-all for `catch()` pattern). The personality function only needs to handle these two cases — no type matching, no exception specs, no C++ interop. This keeps the implementation at ~150 lines of C instead of the 1500+ lines a full C++ personality requires.

**3. Platform unwinder, not language unwinder.**
The personality function delegates stack walking to the platform's `libunwind` (provided by the system or compiler toolchain). Ori owns the LSDA parsing and action dispatch, but not the low-level frame unwinding. This is the same split used by Rust, Go, and Zig.

## Section Dependency Graph

```
Section 01 (C Personality Function)
    │
    ▼
Section 02 (Codegen Integration)
    │
    ▼
Section 03 (Verification)
```

- Section 01 must be complete before Section 02 (the C function must exist before codegen can reference it).
- Section 02 must be complete before Section 03 (all references updated before testing).
- No parallelizable sections — this is a strictly sequential pipeline.

## Implementation Sequence

```
Phase 1 — C Personality Function (Section 01)
  └─ 01.1: Write ori_eh_personality in C (LSDA parser + action dispatch)
  └─ 01.2: Add cc crate build.rs to compile C into libori_rt.a
  └─ 01.3: Export address getter via extern "C" declaration + pub fn
  └─ 01.4: C-level forced-unwind test harness (catch skipped, cleanup runs)
  Gate: `nm libori_rt.a | grep ori_eh_personality` + `cargo test -p ori_rt` passes

Phase 2 — Codegen Integration (Section 02)
  └─ 02.1: Update RT_FUNCTIONS table (runtime_decl/runtime_functions.rs)
  └─ 02.2: Update arc_emitter personality attachment (arc_emitter/mod.rs)
  └─ 02.3: Update JIT symbol mapping (evaluator/runtime_mappings.rs)
  └─ 02.4: Remove rust_eh_personality_addr() + update verify/tests.rs
  Gate: `ORI_DEBUG_LLVM=1 ori build test.ori 2>&1 | grep personality`
         shows `@ori_eh_personality`, zero mentions of `rust_eh_personality`

Phase 3 — Verification (Section 03)
  └─ 03.1: Full test suite passes (./test-all.sh)
  └─ 03.2: Code journey re-run (J3 confirms no rust_eh_personality)
  └─ 03.3: Symbol audit (nm/objdump on AOT binary)
  └─ 03.4: Valgrind clean (diagnostics/valgrind-aot.sh)
  Gate: Zero references to rust_eh_personality in compiler source and generated LLVM IR
```

**Why this order:**
- Phase 1 is pure addition — no existing behavior changes.
- Phase 2 is the swap — changes codegen output but personality function is ready.
- Phase 3 proves everything works end-to-end.

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 C Personality Function | ~420 (C + build.rs + 2× asm + test) | Medium | — |
|   ↳ 01.1 eh_personality.c | ~150 | Medium | — |
|   ↳ 01.2 build.rs | ~25 | Low | 01.1 |
|   ↳ 01.3 Rust FFI bridge | ~10 | Low | 01.1 |
|   ↳ 01.4 Forced-unwind test harness | ~230 (2× asm + C + Rust) | Medium | 01.1 |
| 02 Codegen Integration | ~30 (changes) | Low | 01 |
|   ↳ 02.1-02.4 Symbol swap | ~30 | Low | 01 |
| 03 Verification | ~0 (testing) | Low | 02 |
| **Total new** | **~450** | | |
| **Total deleted** | **~15** | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| C4: Option match tag inversion | Codegen switch maps wrong tag→arm | ori_llvm codegen | Not Started |
| H1: Empty landing pads everywhere | Over-conservative invoke usage | nounwind analysis | Not Started |

These are not affected by this plan but are noted because they interact with the same `invoke`/`landingpad` infrastructure.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | C Personality Function | `section-01-personality-fn.md` | Not Started |
| 02 | Codegen Integration | `section-02-codegen-integration.md` | Not Started |
| 03 | Verification | `section-03-verification.md` | Not Started |
