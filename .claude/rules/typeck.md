---
paths:
  - "**/typeck/**"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

**Expression-based — NO `return`**: Body type IS return type. Exit via `?`/`break`/`panic`.

# Type Checking

> **Note**: The core type checker lives in `ori_types/` (see `types.md`). This file covers the type checking architecture and the `oric/src/reporting/typeck/` diagnostic formatting.

## Architecture (ori_types)

- **Pool + Idx**: Interned types, O(1) equality
- **InferEngine**: Mutable state, fresh vars, union-find unification
- **Registries**: TypeRegistry, TraitRegistry, MethodRegistry
- **ModuleChecker**: `check_module()` orchestrates registration → signatures → body checking

## Inference

- Hindley-Milner with extensions
- Bidirectional: check mode (`Expected`) vs infer mode
- Unification with occurs check + path compression
- Generalization: free vars → quantified (rank-based)

## RAII Scope Guards

- `with_capability_scope(caps, |c| { ... })`
- `with_impl_scope(self_ty, |c| { ... })`
- `with_infer_env_scope(|c| { ... })`

## Derived Trait Registration (Sync Point)

`check/registration/` registers trait definitions and derived impl signatures. Every `DerivedTrait` from `ori_ir` must be registered with correct signatures.

**DO NOT** modify derived trait registration without checking eval and codegen agree. See CLAUDE.md "Adding a New Derived Trait".

## Error Codes

- E2001: Type mismatch
- E2009: Trait bound not satisfied
- E2010: Coherence violation

## Debugging / Tracing

```bash
ORI_LOG=ori_types=debug ori check file.ori          # Module-level phases
ORI_LOG=ori_types=trace ORI_LOG_TREE=1 ori check f.ori  # Per-expression call tree
ORI_DUMP_AFTER_TYPECK=1 ori check file.ori          # Dump typed IR after type checking
```

- `debug`: Module check, signature collection, body checking, type errors
- `trace`: Every `infer_expr()`/`check_expr()` — very verbose

## Key Files

- `ori_types/src/check/`: Module checker, registration, bodies, signatures
- `ori_types/src/infer/`: InferEngine, expression inference
- `ori_types/src/registry/`: Type/trait/method registries
- `ori_types/src/unify/`: Unification engine
- `oric/src/reporting/typeck/`: Type error diagnostic formatting
