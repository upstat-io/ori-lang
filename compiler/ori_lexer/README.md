# ori_lexer

> **`ori_lexer` exists to turn source bytes into a `TokenList` the parser consumes with zero fixups.** Scan-only; no parsing, no semantic validation; output is three parallel arrays optimized for cache locality.
>
> Full mission: [`.claude/rules/missions.md §ori_lexer`](../../.claude/rules/missions.md)

## Role in the pipeline

Phase 1 of the compiler pipeline (`canon.md §1`). Layers keyword recognition, literal cooking, identifier interning (via `Name`), and string interpolation on top of `ori_lexer_core`'s raw scan. The output is a `TokenList` the parser consumes directly without any transformation pass in between.

Lexing is performance-sensitive: cooked-throughput target is ~208-240 MiB/s.

## Architecture

- **Keyword recognizer**: matches reserved words against the raw tag stream.
- **Literal cooker**: parses numeric literals, string literals (with escapes), char/byte literals, duration/size literals.
- **Name interner**: identifiers → `Name(u32)` via the `ori_ir` interner.
- **String interpolation**: handles `` `hello {name}` `` segment transitions via the mode stack from `ori_lexer_core`.
- **`TokenList`**: three parallel arrays — `tokens`, `tags`, `flags` — laid out contiguously for cache-locality during parse.

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir`, `ori_lexer_core` |
| Downstream | `ori_parse`, `oric`, `ori_compiler` |

## Invariants

- **Zero semantic knowledge**: the lexer does not know what an identifier means, only that it is an identifier. Anything the lexer "knows" about names, types, or scopes is phase-bleeding.
- **Output is parser-ready**: the parser must not need to do any pre-transformation pass on the `TokenList`.
- **Interning is mandatory**: every identifier is `Name(u32)`, never `String`.

## Testing

```bash
cargo test -p ori_lexer
```

Benchmark harness lives in `compiler/oric/benches/lexer.rs` and `compiler/oric/benches/lexer_core.rs` (throughput tests against cooked/raw tiers run through the `oric` bench driver).

## References

- [`.claude/rules/canon.md §1`](../../.claude/rules/canon.md) — phase 1 position
- [`.claude/rules/parse.md §LB-2`, `§LB-4`](../../.claude/rules/parse.md) — parallel array layout
- [`.claude/rules/compiler.md §Phase-Specific Purity`](../../.claude/rules/compiler.md) — lexer purity
- MEMORY `§Parser Performance` — throughput benchmarks
