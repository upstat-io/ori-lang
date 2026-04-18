# ori_arc

> **`ori_arc` is the crate locus of AIMS** — phases 5 (ARC lowering), 6 (AIMS lattice analysis), 7 (ARC realization). Every public surface of this crate must extend the unified model, never parallel it.
>
> Full mission: [`.claude/rules/missions.md §ori_arc`](../../.claude/rules/missions.md)
>
> Sub-system mission: [`.claude/rules/missions.md §AIMS`](../../.claude/rules/missions.md)

## Role in the pipeline

Three consecutive pipeline phases live here:

- **Phase 5 — ARC lowering**: `CanExpr` → `ArcFunction` with unresolved RC / reuse / drop decisions.
- **Phase 6 — AIMS lattice analysis**: converges `AimsStateMap` over the product lattice; assigns `MemoryContract` during interprocedural extraction.
- **Phase 7 — ARC realization**: materializes RC / COW / reuse / drop instructions; certifies `FipContract`.

This is where AIMS's mission — "RC rare in emitted code, not RC ops faster" — becomes code. The product lattice, interprocedural contracts (`MemoryContract`, `ParamContract`, `ReturnContract`, `EffectSummary`), FBIP/reuse, TRMC, immortal pre-pass, and borrow inference all live in this crate.

Decision-tree primitives for match compilation are also housed here currently — consumed by `ori_canon::patterns/` via the one non-upstream pipeline edge (migration target).

## Architecture

- `lower/` — phase 5 ARC lowering (`CanExpr` → `ArcFunction`)
- `aims/` — phase 6 AIMS lattice, contracts, analysis driver
- `realize/` — phase 7 RC / COW / reuse / drop materialization
- `decision_tree/` — Maranget primitives (shared with `ori_canon`)
- `borrow/` — borrow inference
- `fip/` — FBIP / FIP contract certification

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir`, `ori_registry`, `ori_types` |
| Downstream | `ori_canon` (decision-tree primitives), `ori_llvm`, `ori_repr`, `oric` |

## Load-bearing invariants (from AIMS sub-system mission)

1. **Contracts and realization must agree** — `FipContract::Certified` ↔ zero unmatched alloc/dealloc in realized IR.
2. **Active rewrites must be sound** — `normalize_function()` must produce identical observable behavior; behavioral verification required.
3. **No pass may rely on stale summaries** — pipeline ordering is load-bearing.
4. **Every active subsystem must be end-to-end verified** — implementation + invariant enforcement + verification (structural + behavioral + regression).
5. **The unified model must stay unified** — new capabilities extend a lattice dimension, a contract field, or a typed pre-pass input. **NEVER** spawn a parallel escape enum, shadow uniqueness tracker, or independent RC emission path.

## Testing

```bash
cargo test -p ori_arc
# Lattice property tests
cargo test -p ori_arc -- lattice::prop_tests
# Contract oracle (re-derives MemoryContract from realized IR)
cargo test -p ori_arc -- oracle
# Protocol builtins matrix
cargo test -p ori_arc -- builtins::tests
```

AIMS snapshot tests live in `compiler/oric/tests/aims-snapshots/` — run via `cargo test -p oric --test aims_snapshots`.

## Where to look

- Lowering: `src/lower/`
- Lattice: `src/aims/lattice/`
- Contracts: `src/aims/contracts/`
- Realization: `src/realize/`
- FIP: `src/fip/`

## References

- [`.claude/rules/arc.md`](../../.claude/rules/arc.md) — ARC pipeline rules
- [`.claude/rules/aims-rules.md`](../../.claude/rules/aims-rules.md) — lattice dimensions + PC-* invariants
- [`.claude/rules/canon.md §1`, `§7.1`](../../.claude/rules/canon.md) — phases 5-7 + Five Load-Bearing Invariants
- `CLAUDE.md §AIMS` — sub-system mission + through-line
