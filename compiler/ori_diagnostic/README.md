# ori_diagnostic

> **`ori_diagnostic` exists to turn compiler facts into diagnostic output users can act on.** Error codes are stable API; messages are user experience.

## Role in the pipeline

Central diagnostic infrastructure for all compiler phases. Provides:

- **Stable error codes**:
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

- `diagnostic/` — `Diagnostic` type, builder API, factory functions, `Suggestion`
- `emitter/` — output formats: `terminal/` (human-readable, optional ANSI color), `json/`, `sarif/`
- `error_code/` — `ErrorCode` enum + phase-range classification + `--explain` lookups
- `errors/` — embedded per-code `EXXXX.md` extended-explanation docs
- `queue/` — accumulation, deduplication, follow-on suppression, error limits
- `fixes/` — code-fix (quick-fix) trait + registry for `ori fix`
- `guarantee/` — `ErrorGuaranteed` type-level proof an error was emitted
- `span_utils/` — line/column computation from spans

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
- Error codes catalog: `src/error_code/`
- Emitters: `src/emitter/`
