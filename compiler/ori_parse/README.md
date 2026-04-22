# ori_parse

> **`ori_parse` exists to produce a parse tree the rest of the compiler can trust absolutely.** `grammar.ebnf` is authoritative; the parser implements it with zero deviation.
>
> Full mission: [`.claude/rules/missions.md §ori_parse`](../../.claude/rules/missions.md)

## Role in the pipeline

Phase 2 of the compiler pipeline. Consumes `TokenList` from `ori_lexer`, produces a `ParseOutput` containing the module, `ExprArena` (AST addressed by opaque `ExprId`), errors, warnings, and metadata.

Surface desugars that can be done at parse time — compound assignment, argument punning (`f(x:)`), variant-pattern punning (`Some(value:)`) — are done here as synthetic AST nodes, not deferred.

## Architecture

- **Pratt-based expression parser**: operator precedence driven by the precedence table in `operator-rules.md`.
- **Statement parser**: recursive descent for declarations, control flow, blocks.
- **Recovery**: `recovery/mod.rs` bitset-based sync points — parse errors produce diagnostics and skip to the next valid statement boundary; AST is not synthesized on error except where the lexer produced `TokenKind::Error`.
- **`ExprArena`**: flat, ID-indexed allocator — no `Box<Expr>`, only `ExprId(u32)`.

Key modules:
- `grammar/expr/` — expression parsing (Pratt loop)
- `grammar/expr/postfix.rs` — argument punning synthesis
- `grammar/expr/patterns/` — pattern parsing, punning synthesis
- `recovery/` — error-recovery sync points
- `pratt/` — operator precedence driver

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir`, `ori_diagnostic`, `ori_stack` (runtime); `ori_lexer` (dev-dep only, for test fixtures) |
| Downstream | `ori_types`, `ori_canon`, `ori_compiler`, `oric` |

## Invariants

- **Grammar-driven**: if `grammar.ebnf` changes, the parser changes to match in the same proposal cycle. Parser changes without spec changes are bugs.
- **No post-parse fixups**: any downstream phase that has to "clean up" the parse output is a parse bug; the fix lives in `ori_parse`.
- **Recovery is explicit**: error paths produce diagnostics and sync, not garbage trees.
- **Stack safety**: deeply nested expressions are handled via `ori_stack::ensure_sufficient_stack` — never blow the host stack.

## Testing

```bash
cargo test -p ori_parse
```

Benchmarks in `compiler/oric/benches/parser.rs` — full-parse throughput ~95-128 MiB/s target (benches run through the `oric` bench driver since `ori_parse` itself declares no `benches/` directory).

## Where to look

- Expression parsing: `src/grammar/expr/`
- Statement parsing: `src/grammar/stmt/`
- Error recovery: `src/recovery/`
- Desugars: `src/grammar/expr/postfix.rs` (punning), compound-assign in Pratt driver

## References

- [`.claude/rules/canon.md §1`, `§2`](../../.claude/rules/canon.md) — phase 2 + parse-time desugars
- [`.claude/rules/parse.md`](../../.claude/rules/parse.md) — parser rules
- [`docs/ori_lang/v2026/spec/grammar.ebnf`](../../docs/ori_lang/v2026/spec/grammar.ebnf) — authoritative grammar
