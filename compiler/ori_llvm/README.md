# ori_llvm

> **`ori_llvm` exists to emit LLVM IR from realized ARC IR without re-deriving any information an earlier phase already owns.** Faithful emission is the deliverable.

## Role in the pipeline

Phase 8 of the compiler pipeline. Consumes realized `ArcFunction` from phase 7 ARC realization, computes a `ReprPlan` via the `ori_repr` sub-layer (7a), and emits LLVM IR. Subsequent LLVM optimization + emission (phase 9) runs the LLVM pipeline on the emitted IR.

`ori_llvm` depends on `ori_arc` but **not** on `ori_canon` — codegen consumes ARC IR, not `CanExpr`. Every RC operation emitted corresponds to a specific AIMS proof failure; `ori_llvm` emits faithfully, it does not re-optimize.

## Architecture

- `codegen/` — primary IR emission
- `codegen/derive_codegen/` — derived-trait method emission
- `aot/` — ahead-of-time compilation driver
- `tests/codegen/` — FileCheck IR assertion corpus
- LLVM IR lint + verification: `ORI_LLVM_LINT`, `ORI_VERIFY_ARC`, `ORI_VERIFY_EACH` env vars

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_arc`, `ori_ir`, `ori_registry`, `ori_repr`, `ori_rt`, `ori_stack`, `ori_types` (runtime); `ori_test_harness` (dev-dep only, for FileCheck IR suites) |
| Downstream | `oric` (via default `llvm` feature) |

Note: does NOT depend on `ori_canon` — codegen consumes ARC IR, not `CanExpr`.

## Invariants

- **Faithful emission**: no re-derivation of type facts, ownership, or repr decisions that upstream phases already own. Querying is the rule; re-inference is a layering violation.
- **Dual-execution parity**: every language construct is lowered in both `ori_llvm` and `ori_eval` with identical observable behavior, or it is a documented GAP.
- **ABI agrees with `ori_rt`**: runtime function signatures and codegen call sites match exactly; changes to either require a matched commit to the other.
- **AIMS facts are consumed, not challenged**: RC operations emitted reflect the lattice's proof-failure list, not codegen's opinion.

## Testing

```bash
# Full crate
cargo test -p ori_llvm
# FileCheck IR assertions (44+ tests)
cargo test -p ori_llvm --test codegen_checks
```

AIMS snapshot tests: `cargo test -p oric --test aims_snapshots`.

## Where to look

- Codegen entry: `src/codegen/mod.rs`
- Derive codegen: `src/codegen/derive_codegen/`
- AOT driver: `src/aot/`
- FileCheck corpus: `tests/codegen/`
