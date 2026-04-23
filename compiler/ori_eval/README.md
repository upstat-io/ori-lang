# ori_eval

> **`ori_eval` exists to execute canonical IR with identical observable behavior to the LLVM backend**, serving both const-evaluation and `ori run`. Parity with LLVM is the load-bearing invariant.

## Role in the pipeline

The evaluator runs in parallel to the LLVM backend (see `canon.md §1` "Evaluator (parallel)" row). It consumes `CanExpr` + `DecisionTreePool` directly — no re-typechecking, no re-canonicalization, no codegen — and produces runtime values.

Two primary use cases:
1. **Const-evaluation** — compile-time execution of `$name`-bound constants and `$fn()` const functions.
2. **`ori run`** — direct interpretation for development loop (no AOT compile).

The evaluator and `ori_llvm` are held to an absolute parity contract: any program run on both must produce identical observable results.

## Architecture

- `interpreter/` — top-level evaluator
- `interpreter/can_eval/` — `CanExpr` dispatch
- `interpreter/method_dispatch/` — method lookup + routing
- `interpreter/derived_methods.rs` — derived-trait runtime dispatch (Eq, Clone, Hashable, etc.)
- `derives/` — derive processing pipeline
- `function_val.rs` — function-value factory methods (e.g., `repeat`, `hash_combine`)

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir`, `ori_registry`, `ori_patterns`, `ori_stack` |
| Downstream | `ori_compiler`, `oric` |

Note: does NOT depend on `ori_types` — the evaluator consumes `CanExpr`, where types are already resolved.

## Invariants

- **Dual-execution parity with LLVM**: every `CanExpr` variant handled by the evaluator is also handled by `ori_llvm` with identical observable behavior. An eval-only or LLVM-only feature is a GAP.
- **No phase bleeding**: the evaluator does no re-typechecking, no re-canonicalization, no codegen.
- **Pattern dispatch via `ori_patterns`**: function patterns invoke `PatternExecutor`; the evaluator never hardcodes pattern-specific logic.
- **Registry-driven method dispatch**: builtin-first via `resolve_builtin_method()`, then impl lookup via `TraitRegistry::lookup_method()`.
- **Stack safety**: deep recursion wrapped via `ori_stack::ensure_sufficient_stack`.

## Testing

```bash
cargo test -p ori_eval
# Ori spec tests (dual-executed)
cargo st
```

## Where to look

- Entry: `src/interpreter/mod.rs`
- Expression dispatch: `src/interpreter/can_eval/`
- Method dispatch: `src/interpreter/method_dispatch/`
- Derived trait dispatch: `src/interpreter/derived_methods.rs`

## References

- `CLAUDE.md §Fix Completeness` — "Interpreter and LLVM produce identical results" contract
