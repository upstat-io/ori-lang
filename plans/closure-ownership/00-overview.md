---
plan: "closure-ownership"
title: "ApplyIndirect Closure Ownership Model"
status: in-progress
references:
  - "plans/bug-tracker/section-04-codegen-llvm.md"
  - "plans/jit-exception-handling/section-04b-lambda-mono.md"
---

# ApplyIndirect Closure Ownership Model

## Mission

Fix the architectural gap where `ApplyIndirect`/`InvokeIndirect` (indirect calls through closures) lack per-argument ownership semantics. Currently, `Apply`/`Invoke` carry `arg_ownership` that tells the AIMS system whether the caller or callee manages RC cleanup, but `ApplyIndirect`/`InvokeIndirect` have no such field. This forces an inconsistent hybrid: the caller always emits `RcDec` for indirect call arguments, but `PartialApply` inside the callee uses ownership transfer semantics (suppresses callee-side `RcDec`). The result is double-drop or RC leak for every closure that captures an argument passed through `ApplyIndirect`.

## Mission Success Criteria

- [ ] `ApplyIndirect` and `InvokeIndirect` carry `arg_ownership: Vec<ArgOwnership>` fields in the ARC IR
- [ ] `is_owned_position()` respects `arg_ownership` for indirect calls (not hardcoded `false`)
- [ ] `annotate_arg_ownership()` populates ownership for `ApplyIndirect`/`InvokeIndirect` from closure contracts
- [ ] `collect_borrowed_call_args()` handles `ApplyIndirect` via `arg_ownership` (not conservative override) AND handles `InvokeIndirect` terminator
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on ALL AOT tests (currently 6 leak)
- [ ] The 3 `#[ignore = "BUG-04-035"]` tests pass without ignore
- [ ] The 3 pre-existing nested closure leaks (`borrowed_list_param`, `borrowed_str_param`, `triple_nested`) pass without leaks
- [ ] `./test-all.sh` green — no regressions
- [ ] All section success criteria met

## Architecture

```
                    ┌─────────────────────────────────────┐
                    │         ARC IR Instructions          │
                    │                                     │
                    │  Apply { arg_ownership: [...] }     │ ← has ownership
                    │  Invoke { arg_ownership: [...] }    │ ← has ownership
                    │  ApplyIndirect { ??? }              │ ← MISSING (Section 01)
                    │  InvokeIndirect { ??? }             │ ← MISSING (Section 01)
                    └──────────────┬──────────────────────┘
                                   │
                    ┌──────────────▼──────────────────────┐
                    │       AIMS Ownership Pipeline        │
                    │                                     │
                    │  1. Interprocedural contracts        │
                    │     (MemoryContract per function)    │
                    │  2. emit_arg_ownership()             │ ← skips indirect
                    │     (populates arg_ownership)        │   (Section 02)
                    │  3. realize_rc_reuse()               │
                    │     (emits RcInc/RcDec)              │
                    └──────────────┬──────────────────────┘
                                   │
                    ┌──────────────▼──────────────────────┐
                    │        LLVM Codegen Emission         │
                    │                                     │
                    │  emit_apply_indirect()               │
                    │  build_closure_env()                 │
                    │  generate_closure_wrapper()          │ ← RC correct
                    │  generate_env_drop_fn()              │ ← RC correct
                    │  collect_borrowed_call_args()        │ ← conservative
                    │                                     │   (Section 03)
                    └─────────────────────────────────────┘
```

## Design Principles

1. **Extend, don't invent.** `Apply`/`Invoke` already have `arg_ownership` with a complete propagation pipeline. Extend the same mechanism to `ApplyIndirect`/`InvokeIndirect` rather than creating a parallel system.

2. **Semantic truth in ARC IR, mechanical lowering in LLVM.** The ARC IR is the authority on RC decisions. `ori_llvm` should lower whatever the ARC IR says — no LLVM-level workarounds for missing ARC IR semantics.

3. **Conservative default for unknown callees.** When the callee's contract is unknown (opaque closure), default to all-`Borrowed` (caller retains cleanup). This is safe: the callee might internally `RcInc` if it needs to keep a reference, and the caller's `RcDec` handles final cleanup.

## Critical Gotchas

1. **`used_vars()` ordering asymmetry**: `ApplyIndirect.used_vars()` returns `[closure, ...args]` (closure FIRST), but `InvokeIndirect.used_vars()` returns `[...args, closure]` (closure LAST). Any code that converts `used_vars()` positions to `arg_ownership` indices must account for this difference. See Section 01.3 for details.

2. **Env drop must RcDec ALL captures**: The env physically owns all captures. "Borrowed" means "the lambda body borrows from the env" (wrapper skips `RcInc`), NOT "the env doesn't own it." The env drop function correctly `RcDec`s all captures. See Section 03.2.

3. **Empty `arg_ownership` semantics**: Before annotation, `arg_ownership` is empty (not-yet-annotated). After annotation, it must match `args.len()`. Add `debug_assert!` to enforce this post-annotation.

## Section Dependency Graph

```
Section 01: ARC IR Shape
    │
    ▼
Section 02: Ownership Propagation
    │
    ▼
Section 03: LLVM Cleanup & Verification
```

Strictly sequential — each section builds on the prior.

## Implementation Sequence

```
Phase 1 - ARC IR Foundation
  └─ 01: Add arg_ownership fields, update is_owned_position,
         update used_vars, substitute_var, verifiers, dumps,
         update emit_terminator_rc for InvokeIndirect project-borrowed RcInc,
         update is_var_defined_in_block for InvokeIndirect

Phase 2 - AIMS Integration
  └─ 02: Teach annotate_arg_ownership to populate indirect calls,
         seed from PartialApply target contracts, handle InvokeIndirect

Phase 3 - LLVM Cleanup & Verification
  └─ 03: Replace drop_hints ApplyIndirect workaround with arg_ownership logic,
         add InvokeIndirect terminator to collect_borrowed_call_args,
         verify env drop correctness (RcDec ALL captures is correct),
         fix 4 stale doc comments, un-ignore tests, verify zero leaks
```

## Estimated Effort

| Section | Files | Est. Lines | Risk |
|---------|-------|-----------|------|
| 01 | 10 | ~150 | Low — mechanical field addition + forward_walk.rs + emit_unified.rs + verifier + tests |
| 02 | 4 | ~150 | Medium — contract lookup for indirect callees + alias chain tracing |
| 03 | 6 | ~80 | Low — replace workaround + add InvokeIndirect + fix 4 docs + verify |

## Resolves

- `[BUG-04-035]` — Nested closure RC leaks (plans/bug-tracker/section-04-codegen-llvm.md) — cross-link: `<!-- resolved-by:plans/closure-ownership -->`
- TPR-04B-013/014 RC leak component (plans/jit-exception-handling/section-04b-lambda-mono.md) — cross-link: `<!-- resolved-by:plans/closure-ownership -->`

## Cleanup (post-completion)

Every plan must strip its code annotations on completion. On completion:
- [ ] Verify no source-code annotations referencing this plan exist (e.g., `closure-ownership`, `Section 01-03` plan refs)
- [ ] Mark BUG-04-035 resolved in `plans/bug-tracker/section-04-codegen-llvm.md` with `<!-- resolved-by:plans/closure-ownership -->`
- [ ] Update TPR-04B-014 resolution note in `plans/jit-exception-handling/section-04b-lambda-mono.md` with `<!-- resolved-by:plans/closure-ownership -->`

## Prior Art

- **Lean 4**: Functions stored in closures use "standard" (owned) calling convention. `lean.h` distinguishes standard vs borrowed.
- **Swift SIL**: `partial_apply` explicitly encodes `@owned`/`@guaranteed` per captured value. Retains are inserted separately per convention.
