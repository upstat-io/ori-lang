# ori_registry

> **`ori_registry` is the single source of truth for builtin type behavior.** Pure data, not logic.
>
> Full mission: [`.claude/rules/missions.md §ori_registry`](../../.claude/rules/missions.md)

## Role in the pipeline

Defines every builtin type's methods, operator dispatch, memory characteristics (`heap`/`value`/`sendable`), and derivation eligibility as **pure data** — no logic, no side effects. Every compiler phase that needs to know "does this type implement `Eq`?" or "what does `str.split()` look like?" queries the registry; no phase maintains a parallel lookup table.

This is the canonical home of builtin type behavior per `impl-hygiene.md §SSOT` — the test "where is this type's behavior defined?" always answers with a single file path in this crate.

## Architecture

Data-only crate, organized by type family:

- `str/`, `int/`, `float/`, `bool/`, `char/`, `byte/` — primitives
- `list/`, `map/`, `set/`, `range/`, `duration/`, `size/` — compound and special types
- `iterator/`, `option/`, `result/`, `ordering/` — prelude types
- `operators/` — binary / unary operator dispatch definitions
- `derives/` — per-type derivability (Eq, Clone, Debug, etc.)
- Entry point: `find_type()`, `find_method()`, `OpDefs`

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir` (for TypeId, Name) |
| Downstream | `ori_types`, `ori_arc`, `ori_eval`, `ori_llvm`, `ori_compiler`, `oric` |

## Invariants

- **Pure data, no logic**: if the registry starts containing behavior that couldn't be expressed as a data entry, the behavior has leaked in from somewhere else and should be returned.
- **SSOT**: adding a new builtin method adds it HERE first. Consumers pick it up from registry lookups — never retrofitted at consumption sites.
- **Alphabetically sorted methods per type**: enforced by `registry_methods_sorted_per_type` test.
- **Consumers query, never parallel-table**: hardcoded `if type == Str { ... }` at a call site is a `LEAK:scattered-knowledge`.

## Testing

```bash
cargo test -p ori_registry
```

## Where to look

- Per-type files: `src/<type>/mod.rs`
- Method registration: `src/<type>/methods.rs`
- Operator dispatch: `src/operators/`

## References

- [`.claude/rules/registry.md`](../../.claude/rules/registry.md) — registry rules + method/type addition workflow
- [`.claude/rules/impl-hygiene.md §SSOT`](../../.claude/rules/impl-hygiene.md) — architectural centers
- `CLAUDE.md §Key Patterns` — Method Dispatch
