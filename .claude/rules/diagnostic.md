---
paths:
  - "**diagnostic**"
---

# Diagnostics

## Error Codes
- **E0xxx**: Lexer | **E1xxx**: Parser | **E2xxx**: Type checker | **E3xxx**: Pattern | **E08xx**: Evaluator | **E9xxx**: Internal
- New codes: increment within range, add doc in `errors/EXXX.md`

## Diagnostic Structure
- `Diagnostic { code, severity, message, labels, notes, suggestions }`
- Builder: `Diagnostic::error(code).with_message().with_label().with_fix()`
- Applicability: `MachineApplicable` | `MaybeIncorrect` | `HasPlaceholders`

## Message Style
- Backticks for code: `` `variable` ``
- No periods in main message
- Imperative: "try using X" | three-part: problem -> context -> guidance

## Emitters (`emitter/`)
- `terminal/`: Terminal (Ariadne-based) | `json/`: JSON | `sarif/`: SARIF

## Tracing
- `ori_diagnostic` has no direct tracing | debug via producing crates: `ORI_LOG=ori_types=debug` (type errors) | `ORI_LOG=debug` (all phases)
- Codegen audit: `ORI_AUDIT_CODEGEN=1 ori build file.ori` | `diagnostics/codegen-audit.sh file.ori`

## Key Files
- `error_code.rs`: Error codes
- `diagnostic.rs`: Builder
- `emitter/`: Output formats (terminal, json, sarif)
- `queue.rs`: Accumulation
