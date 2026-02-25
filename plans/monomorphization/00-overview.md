---
plan: "monomorphization"
title: "Generic Monomorphization for AOT Compilation"
status: in-progress
references:
  - "docs/ori_lang/0.1-alpha/design/monomorphization-architecture.md"
  - "docs/ori_lang/proposals/approved/capability-unification-generics-proposal.md"
  - "docs/ori_lang/proposals/approved/const-generics-proposal.md"
  - "plans/roadmap/section-21A-llvm.md (21.7)"
---

# Generic Monomorphization for AOT Compilation

## Mission

Enable generic functions to compile through AOT, producing identical results to the interpreter. Stamp out one LLVM function per concrete type argument combination (full monomorphization, like Rust/Zig/Roc). This is the natural fit for Ori's ARC memory management — each specialization needs type-specific RC inc/dec/drop.

## Context

Generic functions (e.g., `@identity <T> (x: T) -> T = x`) are completely skipped during AOT/LLVM compilation. `FunctionCompiler` checks `sig.is_generic()` and continues past them in both `declare_all()` and `define_all()`. Call sites emit `Apply(int, "identity", [arg])` but the function was never declared, so the call fails silently.

This is the **CRITICAL blocker** in the AOT Codegen Pipeline (Section 21.7), blocking 2,472+ call sites and the `str()` prelude function.

## Architecture

```
Type Checker (ori_types)            LLVM Pipeline (ori_llvm)
     │                                  │
     │ Phase 1: Discovery               │ Phase 2: Collection
     │ Records MonoInstance             │ collect_mono_functions()
     │ { fn_name, generic_args,         │ produces MonoFunction with
     │   body_type_map }                │ mangled name + concrete sig
     │                                  │
     ▼                                  │ Phase 3: ARC Lowering
TypedModule.mono_instances ────────────►│ lower_function_can() with
                                        │ type_subst map applied
                                        │
                                        │ Phase 4: LLVM Codegen
                                        │ declare + define as normal
                                        │ (non-generic) functions
                                        │
                                        ▼
                                  Call site resolution:
                                  emit_apply() resolves "identity" →
                                  "identity$m$int" via arg types
```

## Design Decisions

**GenericArg enum (future-proof).** Studied Rust (`GenericArgKind`), Swift (`SubstitutionMap`), Zig (`InternPool.Index`), and Lean 4 (selective monomorphization). All four use a unified discriminated union for generic arguments. Ori's `GenericArg::Type(Idx) | Const(ConstValue)` handles both type and const value substitution through all five generics phases from the capability-unification proposal:

1. Type parameters (Phase 1 — this plan)
2. Const generic values (`$N: int`)
3. Expanded const eligibility (any type with `Eq + Hashable`)
4. Associated consts in traits
5. Const functions in type positions

See `docs/ori_lang/0.1-alpha/design/monomorphization-architecture.md` for the full architecture document including reference compiler comparison and phase evolution details.

## Scope Limitations (Phase 1 / 0.1-alpha)

- **Direct type params only**: `generic_param_mapping[i] = Some(...)`. Indirect params (T inside `[T]`, `(T, U)`) deferred.
- **Free functions only**: Generic trait methods deferred.
- **No generic recursion discovery**: Only top-level call sites from non-generic functions.
- **No const generics**: Only type parameter monomorphization.

## Section Dependency Graph

```
  01 Type Checker Infrastructure ──→ 02 ARC Lowering ──→ 03 LLVM Pipeline ──→ 04 Verification
```

Linear dependency chain — each section requires the previous one.

## Name Mangling Scheme

Consistent across collection (Section 03.1) and call resolution (Section 03.4):

```
{fn_name}$m${type1}_{type2}_{typeN}
```

| Type | Encoding | Example |
|------|----------|---------|
| `int` | `int` | `identity$m$int` |
| `float` | `float` | `identity$m$float` |
| `bool` | `bool` | `identity$m$bool` |
| `str` | `str` | `identity$m$str` |
| `char` | `char` | `identity$m$char` |
| `byte` | `byte` | `identity$m$byte` |
| `()` (unit) | `void` | `identity$m$void` |
| `[T]` (List) | `L{elem}` | `filter$m$Lint` |
| `Option<T>` | `O{inner}` | `unwrap$m$Oint` |
| `Result<T, E>` | `R{ok}_{err}` | `try$m$Rint_str` |
| `(T, U)` (Tuple) | `T{e1}_{e2}` | `swap$m$Tint_bool` |
| `{name}` (Struct) | `S{name}` | `process$m$SPoint` |
| `{name}` (Enum) | `E{name}` | `handle$m$EColor` |
| `(A) -> B` (Function) | `F{params}_R{ret}` | — |

Phase 2 adds const value encoding: `c{n}` for positive int, `cn{n}` for negative, `ctrue`/`cfalse` for bool.

## Files Modified

| File | Change | Section |
|------|--------|---------|
| `ori_types/src/output/mod.rs` | `GenericArg`, `ConstValue`, `MonoInstance`, `FunctionSig.scheme_var_ids`, `TypedModule.mono_instances` | 01 |
| `ori_types/src/check/signatures/mod.rs` | Store var_ids in FunctionSig | 01 |
| `ori_types/src/infer/mod.rs` | `mono_instances` field + methods on InferEngine | 01 |
| `ori_types/src/pool/substitute.rs` | **NEW**: `substitute_in_pool()` recursive type substitution | 01 |
| `ori_types/src/infer/expr/calls.rs` | Record mono instances after generic call checking | 01 |
| `ori_types/src/check/bodies/mod.rs` | Extract mono instances from InferEngine | 01 |
| `ori_types/src/check/mod.rs` | Accumulate + dedup mono instances in ModuleChecker | 01 |
| `ori_arc/src/lower/mod.rs` | `type_subst` param on `lower_function_can()` | 02 |
| `ori_arc/src/lower/expr/mod.rs` | `type_subst` on ArcLowerer, `resolve_body_type()` | 02 |
| `ori_llvm/src/monomorphize.rs` | **NEW**: `MonoFunction`, `collect_mono_functions()` | 03 |
| `ori_llvm/src/evaluator.rs` | Lower mono functions, pass to FunctionCompiler | 03 |
| `ori_llvm/src/codegen/function_compiler/mod.rs` | Declare/define mono functions | 03 |
| `ori_llvm/src/codegen/arc_emitter/mod.rs` | `resolve_mono_call()` for generic call sites | 03 |
