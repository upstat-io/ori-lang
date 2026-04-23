# ori_canon

> **`ori_canon` exists to produce the canonical IR that every backend consumes identically.** One shape per source construct, consumed identically by every backend.

## Role in the pipeline

Phase 4 of the compiler pipeline. Consumes typed IR from `ori_types`, produces `CanonResult` containing `CanExpr` (sugar eliminated, types resolved) and a populated `DecisionTreePool` (pattern-matching decision trees compiled via the Maranget algorithm).

The shared `DecisionTreePool` is consumed by BOTH `ori_eval` and `ori_arc` during ARC lowering of `CanExpr::Match`. Compilation once, consumed by both backends — dual-execution parity becomes structural at the pattern level, not a per-program runtime check.

## Architecture

- `lower/` — typed IR → `CanExpr` lowering
- `lower/expr.rs` — expression lowering (including index/field-assignment target handling)
- `desugar/` — canonical-IR desugar set (distinct from parse/typeck surface desugars; see `canon.md §4.3`)
- `patterns/` — Maranget pattern compilation front; invokes decision-tree primitives from `ori_arc::decision_tree` (the one non-upstream pipeline edge — migration target)

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir`, `ori_types`, `ori_arc` (for decision-tree primitives — non-upstream edge) |
| Downstream | `ori_eval`, `ori_arc`, `ori_compiler`, `oric` |

Note: `ori_llvm` does NOT depend on `ori_canon` — codegen consumes realized ARC IR, not `CanExpr`.

## Invariants

- **Canonical means singular**: one `CanExpr` shape per source construct. If two backends would have to implement the same semantic independently, the work belongs here.
- **Sugar is eliminated**: downstream phases see no surface-level sugar variants.
- **`DecisionTreePool` is shared**: `ori_eval` and `ori_arc` consume the same decision trees.
- **Cross-phase contract "Canon → All"**: every `CanExpr` node carries a resolved type; no `TypeId::INFER` survives to canonical IR (`impl-hygiene.md §Cross-Phase Invariant Contracts`).

## Testing

```bash
cargo test -p ori_canon
```

## Where to look

- Lowering: `src/lower/expr.rs`
- Desugars: `src/desugar/`
- Pattern compilation: `src/patterns/`

## References

- [`docs/compiler/design/07-canonicalization/pattern-compilation.md`](../../docs/compiler/design/07-canonicalization/pattern-compilation.md) — Maranget design notes
