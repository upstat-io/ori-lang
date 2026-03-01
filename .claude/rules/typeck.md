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
- `check/registration/` registers trait definitions and derived impl signatures
- Every `DerivedTrait` from `ori_ir` must be registered with correct signatures
- **DO NOT** modify without checking eval and codegen agree | see CLAUDE.md "Adding a New Derived Trait"

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
