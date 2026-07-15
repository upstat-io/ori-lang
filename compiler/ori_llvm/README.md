# ori_llvm

> **`ori_llvm` is one physical projection of the shared post-AIMS executable artifact.** It emits LLVM IR without re-deriving semantic, ownership, drop, effect, callable, or representation policy. Faithful projection is the deliverable.

## Role in the pipeline

Phase 8 of the shipped compiler pipeline. It consumes realized `ArcFunction` values plus upstream representation and executable facts, then emits LLVM IR. The production seam supplies one validated `ExecutableProgram` and a compiled-layout/ABI projection; LLVM does not own AIMS or recompute its policy. Subsequent LLVM optimization + emission (phase 9) runs only on the emitted IR.

`ori_llvm` depends on `ori_arc` but **not** on `ori_canon` — codegen consumes ARC IR, not `CanExpr`. Every memory-management operation must trace to a compiled physical-plan choice satisfying an exact frozen AIMS obligation; `ori_llvm` projects faithfully and does not reconstruct policy.

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
- **AIMS is backend-neutral**: VM, LLVM, native, compiled-WebAssembly, and JIT consumers receive the same typed ownership/drop/effect facts. LLVM attributes and instructions are one physical spelling, never the fact authority.
- **AIMS facts are consumed, not challenged**: the current counter projection's RC operations implement validated physical-plan choices that satisfy the shared calculus; they are not AIMS facts or codegen's opinion.

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
