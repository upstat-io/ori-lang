# ori_types

> **`ori_types` exists to prove, before the next phase runs, that every expression in a module has a single resolved type and every trait dispatch has a known target.** The crate's non-negotiable output contract is PC-2.

## Role in the pipeline

Phase 3 of the compiler pipeline. Consumes the AST from `ori_parse`, produces a `TypeCheckResult` / `TypedModule` where every `ExprId` has a resolved `Idx` into the type pool. Performs Hindley-Milner inference, trait dispatch resolution via the registry, structural-capability checking, and surface operator desugars (`a + b` → `Add::add(a, b)`, `a == b` → `Eq::equals(a, b)`, etc.).

This phase is the gate for the compiler's "type safety is non-negotiable" invariant — any expression that reaches canonicalization without a resolved type is a bug in `ori_types`, not a concern for downstream phases.

## Architecture

- **V2 type checker**: `InferEngine`, `Pool`, registries, `ModuleChecker`
- **Inference**: `infer/` — per-expression-kind inference logic; `infer/expr/identifiers.rs` holds built-in type signatures
- **Checking**: `check/` — registration (`check/registration/`), validators (`check/validators/`), signature construction (`check/signatures/`)
- **Type pool**: `pool/` — interned type storage; `TypeId` / `Idx`
- **Registries**: `registry/` — method and trait registries; dispatch lookups
- **Substitution**: `pool/substitute/`, `unify/substitute.rs`

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir`, `ori_registry`, `ori_diagnostic`, `ori_stack` |
| Downstream | `ori_canon`, `ori_llvm`, `ori_repr`, `ori_compiler`, `oric` |

Note: `ori_eval` does NOT depend on `ori_types` — the evaluator consumes canonical IR, where types are already resolved by `ori_canon`.

## Invariants

- **PC-2 output contract**: no `Tag::Var` / `Tag::Infer` / `Tag::Projection` / `Tag::SelfType` in typed IR; every `ExprId` has a resolved `Idx`. Enforced by `validate_body_types` and upstream inference/resolution passes.
- **Registry-driven dispatch**: trait dispatch is never hardcoded for primitives; all method lookups go through the registry. Parallel dispatch paths are a `LEAK:scattered-knowledge` violation of the unified trait/capability model.
- **Precision over speed**: imprecise inference that requires downstream fixups breaks PC-2 and is a correctness violation, not a performance decision.
- **Operator desugars land here**: `ori_canon` and downstream phases see uniform call-site structure, not surface operators.

## Testing

```bash
cargo test -p ori_types
```

## Where to look

- Expression inference: `src/infer/expr/`
- Trait registration: `src/check/registration/`
- PC-2 validator: `src/check/validators/mod.rs`
- Type pool: `src/pool/`
- Method registry: `src/registry/methods/`

## References

- [`docs/ori_lang/v2026/spec/operator-rules.md`](../../docs/ori_lang/v2026/spec/operator-rules.md) — operator semantics
