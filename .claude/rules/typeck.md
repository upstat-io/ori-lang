---
paths:
  - "**typeck**"
---

# Type Checking

> Core type checker lives in `ori_types/` (see types.md). This file covers architecture and `oric/src/reporting/typeck/` diagnostic formatting.

## Architecture (ori_types)
- **Pool + Idx**: Interned types, O(1) equality
- **InferEngine**: Mutable state, fresh vars, union-find unification
- **Registries**: TypeRegistry, TraitRegistry, MethodRegistry
- **ModuleChecker**: `check_module()` orchestrates registration -> signatures -> body checking

## Inference
- Hindley-Milner with extensions
- Bidirectional: check mode (`Expected`) vs infer mode
- Unification with occurs check + path compression
- Generalization: free vars -> quantified (rank-based)

## RAII Scope Guards
- `with_capability_scope(caps, |c| { ... })`
- `with_impl_scope(self_ty, |c| { ... })`
- `with_infer_env_scope(|c| { ... })`

## Derived Trait Registration (Sync Point)

See `ir.md` §DerivedTrait for the canonical sync point list and full checklist. This crate's sync point: `check/registration/` registers trait definitions and derived impl signatures.

## Subsystem Hygiene

- **Unification soundness**: occurs check must never be bypassed. Infinite types (recursive unification) are always bugs. Path compression must preserve the occurs check invariant.
- **Generalization correctness**: only variables at the current rank or higher are generalized. Under-generalization = monomorphism bug (function less polymorphic than intended). Over-generalization = unsoundness (escaped skolem).
- **Trait resolution determinism**: trait resolution must be deterministic given the same set of impls. Non-deterministic resolution = coherence bug. If two impls could apply, the conflict must be caught at registration time, not silently resolved by iteration order.
- **Error recovery monotonicity**: TyError must poison silently (see `impl-hygiene.md` §Error Recovery Monotonicity). A type error in function `f` must not generate cascading errors in unrelated function `g`. If it does, the error recovery is too aggressive.
- **Bidirectional mode discipline**: check-mode (`Expected`) must propagate inward (from context to subexpression), infer-mode must propagate outward (from subexpression to context). A function that mixes modes without explicit switching is a correctness risk — the direction determines which side of a constraint gets the error message.
- **Inference variable hygiene**: no inference variable (`Var`) may escape its scope. Before emitting typed IR, all `Var` tags must be resolved to concrete types. A `Var` in typed IR is a phase contract violation (see `impl-hygiene.md` §Cross-Phase Invariant Contracts).

## Error Codes
- E2001: Type mismatch | E2009: Trait bound not satisfied | E2010: Coherence violation

## Tracing
- Target: `ori_types` | `ORI_LOG=ori_types=debug` (module phases, type errors) | `=trace ORI_LOG_TREE=1` (per-expression call tree)
- Phase dump: `ORI_DUMP_AFTER_TYPECK=1` | see compiler.md for full debugging reference

## Key Files
- `ori_types/src/check/`: Module checker, registration, bodies, signatures
- `ori_types/src/infer/`: InferEngine, expression inference
- `ori_types/src/registry/`: Type/trait/method registries
- `ori_types/src/unify/`: Unification engine
- `oric/src/reporting/typeck/`: Type error diagnostic formatting
