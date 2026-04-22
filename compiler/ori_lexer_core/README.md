# ori_lexer_core

> **`ori_lexer_core` exists to provide the minimal scanning primitives and interfaces shared by the cooked lexer.** Raw scan only — no semantic state.
>
> Full mission: [`.claude/rules/missions.md §ori_lexer_core`](../../.claude/rules/missions.md)

## Role in the pipeline

This crate is the leaf of the lexing front. It owns character classification, token tagging at the byte level, and the mode-stack state machine used for string interpolation. It has no knowledge of keywords, literals, or identifiers as semantic entities — those live in `ori_lexer`.

`ori_lexer_core` exists so that `ori_lexer` can import a stable, fast scan interface without coupling to a full tokenization pipeline, and so that alternate tooling (fuzzing harnesses, syntax-aware editors) can consume raw scan output directly if needed.

## Architecture

- **Character classifier**: byte-level predicate table (is_id_start, is_id_continue, is_digit, etc.)
- **Token tagger**: emits `(TokenKind, len)` pairs for the raw byte stream
- **Mode stack**: tracks string / interpolation / nested-context transitions without semantic lookahead

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | external utility deps only |
| Downstream | `ori_lexer` |

## Invariants

- **Scan-only**: no keyword recognition, no literal parsing, no interning — all of that lives in `ori_lexer`.
- **No semantic state**: no names, no types, no scopes.
- **Output tags are stable API**: every `TokenKind` this crate emits must be recognized by `ori_lexer`.

## Testing

```bash
cargo test -p ori_lexer_core
```

## References

- [`.claude/rules/canon.md §1`](../../.claude/rules/canon.md) — phase 1 Lex position in the pipeline
- [`.claude/rules/compiler.md §Phase-Specific Purity`](../../.claude/rules/compiler.md) — lexer purity contract
- [`.claude/rules/parse.md §LB-2`, `§LB-4`](../../.claude/rules/parse.md) — parallel-array output layout that `ori_lexer` builds on top of this
