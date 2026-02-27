---
paths:
  - "**/diagnostic/**"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

# Diagnostics

## Error Codes
- **E0xxx**: Lexer
- **E1xxx**: Parser
- **E2xxx**: Type checker
- **E3xxx**: Pattern
- **E08xx**: Evaluator
- **E9xxx**: Internal

New codes: increment within range, add doc in `errors/EXXX.md`.

## Diagnostic Structure
- `Diagnostic { code, severity, message, labels, notes, suggestions }`
- Builder: `Diagnostic::error(code).with_message().with_label().with_fix()`
- Applicability: `MachineApplicable` | `MaybeIncorrect` | `HasPlaceholders`

## Message Style
- Backticks for code: `` `variable` ``
- No periods in main message
- Imperative: "try using X"
- Three-part: problem → context → guidance

## Emitters (`emitter/`)
- `terminal/`: Terminal (Ariadne-based)
- `json/`: JSON
- `sarif/`: SARIF

## Debugging / Tracing

**Always use `ORI_LOG` first when debugging diagnostic issues.** The `ori_diagnostic` crate does not currently use tracing, but diagnostics are emitted by other crates that do.

```bash
ORI_LOG=ori_types=debug ori check file.ori          # See type errors as they're pushed
ORI_LOG=debug ori check file.ori                    # See all phase-level diagnostic activity
ORI_DUMP_AFTER_TYPECK=1 ori check file.ori          # Inspect typed IR to verify diagnostic context
```

### Codegen Verification
```bash
ORI_AUDIT_CODEGEN=1 ori build file.ori              # In-pipeline RC audit (detects codegen issues)
diagnostics/codegen-audit.sh file.ori               # Shell wrapper with color output
```

**Tips**:
- Missing error? Use `ori_types=debug` to confirm `push_error()` is called
- Wrong span? Check the expression's `Span` in the IR — use `ORI_LOG=ori_types=trace` to see which expr triggers the error
- Diagnostic not displayed? Check emitter selection in `oric` command handling

## Key Files
- `error_code.rs`: Error codes
- `diagnostic.rs`: Builder
- `emitter/`: Output formats (terminal, json, sarif)
- `queue.rs`: Accumulation
