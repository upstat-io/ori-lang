# ori_diagnostic

> **`ori_diagnostic` exists to turn compiler facts into diagnostic output users can act on.** Error codes are stable API; messages are user experience.
>
> Full mission: [`.claude/rules/missions.md §ori_diagnostic`](../../.claude/rules/missions.md)

## Role in the pipeline

Central diagnostic infrastructure for all compiler phases. Provides:

- **Stable error codes** (ranges per `impl-hygiene.md §Error Handling`):
  - E0xxx — lexer
  - E1xxx — parser
  - E2xxx — type checker
  - E3xxx — pattern/semantic
  - E4xxx — ARC
  - E5xxx — codegen/LLVM
  - E6xxx — runtime/eval
  - E9xxx — internal
  - W1xxx/W2xxx — warnings
- **Structured construction**: `Diagnostic::error(code).with_message(...).with_label(...)` — no `format!()` strings, no inline concatenation.
- **Accumulation**: phases collect diagnostics; nobody bails on first error.
- **Deduplication + follow-on suppression**: same-line heuristic; soft errors suppressed after hard errors; follow-on errors filtered when enabled.
- **Edit-distance suggestions**: Damerau-Levenshtein for "did you mean?" — threshold `distance <= min(name.len() - 1, max(2, name.len() / 3))`.

## Architecture

- `diagnostic/` — `Diagnostic` type, builder API
- `renderer/` — user-facing rendering (plain, JSON, rustc-style)
- `dedup/` — deduplication + follow-on suppression
- `suggest/` — edit-distance "did you mean?"
- `codes/` — canonical code-to-message lookup

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `ori_ir` (for `Span`, `Name`) |
| Downstream | every compiler crate that produces errors |

## Invariants

- **Error codes are stable API once shipped**: never reuse a code for a different condition; never change meaning.
- **Every error has a span**: spanless errors are bugs.
- **Structured construction only**: `format!("...")` as an error message is banned.
- **Tests assert on codes, not messages**: message text is UX and may evolve; codes are contract.
- **No panic on user input**: every user-facing error goes through `Diagnostic`, not `panic!()`.

## Testing

```bash
cargo test -p ori_diagnostic
```

## Where to look

- Diagnostic builder: `src/diagnostic/`
- Error codes catalog: `src/codes/`
- Renderers: `src/renderer/`

## References

- [`.claude/rules/diagnostic.md`](../../.claude/rules/diagnostic.md) — diagnostic rules + message style + deduplication contract
- [`.claude/rules/impl-hygiene.md §Error Handling`](../../.claude/rules/impl-hygiene.md) — error-handling paradigm
