# ori_patterns

> **`ori_patterns` exists to own the runtime value model and the function-pattern dispatch system.** Evaluator-abstracted, not evaluator-coupled.

## Role in the pipeline

`ori_patterns` owns two distinct but related concerns:

1. **The runtime value model** — `Value`, `Heap<T>`, `FunctionValue`, `RangeValue`, `IteratorValue`, and related types, with enforced Arc discipline (every heap allocation goes through `Value::` factory methods; `Heap<T>` enforces the invariant).
2. **The function-pattern dispatch system** — `PatternDefinition` trait + static ZST `Pattern` enum registering built-in patterns: `recurse`, `parallel`, `spawn`, `timeout`, `cache`, `with`, `catch`, `todo`, `unreachable`, `print`, `panic`, channel patterns.

Patterns invoke the evaluator only through the `PatternExecutor` trait — never a direct evaluator reference. This keeps new patterns addable without touching the evaluator core (Open/Closed).

**Not this crate**: match-arm pattern compilation (Maranget decision trees) lives in `ori_canon::patterns/` with primitives currently in `ori_arc::decision_tree/`.

## Architecture

- `value/` — runtime value types, iterators, heap discipline
- `registry/` — `PatternRegistry`, `Pattern` enum, `FunctionExpKind` dispatch
- `builtins/`, `cache/`, `channel/`, `parallel/`, `recurse/`, etc. — per-pattern implementations
- `method_key.rs` / `user_methods.rs` — user-method dispatch support
- `fusion.rs` — pattern fusion for `can_fuse_with` / `fuse_with`
- `errors/` — evaluator error types (`EvalError`, `EvalResult`)

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir`, `ori_diagnostic` |
| Downstream | `ori_eval`, `ori_compiler`, `oric` |

## Invariants

- **Arc discipline is absolute**: all heap allocations go through `Value::` factory methods. Constructing `Heap<T>` outside these factories is a LEAK.
- **Open/Closed extension**: new patterns are added by implementing `PatternDefinition` and registering in the `Pattern` enum. Existing code does not change.
- **No direct evaluator reference**: patterns invoke the evaluator via `PatternExecutor` only. Direct evaluator coupling is a layering violation.
- **Static ZST dispatch**: no vtables, no trait objects; `Pattern` enum is the canonical dispatch table.

## Testing

```bash
cargo test -p ori_patterns
```

## Where to look

- `PatternDefinition` trait: `src/lib.rs`
- `Pattern` enum + registry: `src/registry/mod.rs`
- Pattern implementations: `src/recurse/`, `src/parallel/`, etc.
- Value model: `src/value/`
