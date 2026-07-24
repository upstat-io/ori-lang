# ori_repr

> **`ori_repr` exists to compute the `ReprPlan` once** — the layout, alignment, discriminant encoding, and ABI decisions for every type. One layout per type, computed once, consumed everywhere.

## Role in the pipeline

Sub-layer 7a inside `ori_llvm` codegen. Runs after phase 7 ARC realization, before phase 8 LLVM IR emission. Consumes realized `ArcFunction` + the type pool, produces a `ReprPlan` that is the single source of truth for:

- Struct layout (field offsets, padding, alignment)
- Enum discriminant encoding
- ABI decisions (pass-by-value vs pass-by-ref, alignment guarantees)
- Interaction with `#repr("c")`, `#repr("packed")`, `#repr("transparent")`, `#repr("aligned", N)` pragmas

Every codegen consumer queries `ReprPlan` rather than re-deriving layout independently.

## Architecture

- `plan.rs`, `plan/` — `ReprPlan` state, decisions, attributes, and queries
- `layout/` — layout computation (field offsets, alignment, padding)
- `canonical/`, `enum_repr.rs` — canonical machine and enum representations
- `narrowing/` — integer and floating-point representation narrowing
- `executable/` — frozen callable, effect, drop, and runtime facts
- `pipeline/` — representation-plan construction and metadata propagation

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir`, `ori_types`, `ori_arc` |
| Downstream | `ori_llvm`, `oric` |

## Invariants

- **One layout per type**: if two codegen sites both need the size of a struct, they query the same `ReprPlan` entry — never recompute.
- **`#repr` pragmas are inputs**: pragmas feed a single computation; they are never a reason for parallel `ReprPlan`s.
- **Cached via Salsa or interning**: same `TypeId` = same layout, always. Non-determinism is a bug.
- **Consumers query, never compute**: direct layout computation at codegen sites is a `LEAK:scattered-knowledge`.

## Testing

```bash
cargo test -p ori_repr
```

## Where to look

- `ReprPlan` entry point: `src/plan.rs`
- Layout algorithm: `src/layout/`
- Representation attributes: `src/plan/repr_attr.rs`
