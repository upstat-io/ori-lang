# ori_arc

> **`ori_arc` is the crate locus of AIMS** — phases 5 (ARC lowering), 6 (AIMS lattice analysis), 7 (ARC realization). Every public surface of this crate must extend the unified model, never parallel it.

## Role in the pipeline

Three consecutive pipeline phases live here:

- **Phase 5 — ARC lowering**: `CanExpr` → `ArcFunction` with unresolved logical ownership / reuse / drop decisions.
- **Phase 6 — AIMS lattice analysis**: converges `AimsStateMap` over the product lattice; assigns `MemoryContract` during interprocedural extraction.
- **Phase 7 — ARC realization**: freezes ownership / COW / reuse / drop operations in the shared carrier; certifies `FipContract`.

This is where AIMS freezes one backend-neutral ownership, lifetime, cleanup,
transfer, COW/reuse, effect, unwind, and provenance plan. The product lattice,
interprocedural contracts (`MemoryContract`, `ParamContract`, `ReturnContract`,
`EffectSummary`), FBIP/reuse, TRMC, immortal pre-pass, and borrow inference all
live in this crate. The current `Rc*` and burden-op spellings are carrier details
for the compiled-counter adapter, not the definition or destination of AIMS.

Decision-tree primitives for match compilation are also housed here currently — consumed by `ori_canon::patterns/` via the one non-upstream pipeline edge (migration target).

## Architecture

- `lower/` — phase 5 ARC lowering (`CanExpr` → `ArcFunction`)
- `aims/` — phase 6 AIMS lattice, contracts, analysis driver
- `realize/` — phase 7 logical ownership / COW / reuse / drop realization
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
5. **The unified model must stay unified** — new capabilities extend a lattice dimension, a contract field, or a typed pre-pass input. **NEVER** spawn a parallel escape enum, shadow uniqueness tracker, or independent ownership-event placement path. VM, LLVM/native, compiled-WebAssembly, and JIT remain sibling physical projections of the same exact facts.

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

- — sub-system mission + through-line
