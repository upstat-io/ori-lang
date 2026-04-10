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
- **Expected context**: every "expected X, got Y" MUST include WHY — annotation, return type, parameter, operator context
- **Suggestions**: use Damerau-Levenshtein edit distance for "did you mean?" — threshold: `distance <= max(2, name.len() / 3)`

## Error Code Stability
- Error codes are **permanent stable API** — once assigned, never reuse or change meaning
- New errors get the next available code in their range
- Each code documented in `errors/EXXX.md` with spec reference + example

## Deduplication
- Hash emitted diagnostics; suppress exact duplicates
- Suppress follow-on errors when earlier error on same span already explains the problem
- "invalid operand" style cascading errors suppressed if prior error exists on same expression

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

## Diagnostic Scripts (`diagnostics/`)

All support `--help`, `--no-color`/`--color`.

| Script | Purpose | Key flags |
|--------|---------|-----------|
| `diagnose-aot.sh` | All-in-one: build+run+leak+RC+IR | `--valgrind`, `--rc-trace`, `--verbose`, `--release`, `--both-builds` |
| `dual-exec-debug.sh` | Interpreter vs AOT comparison | `--verbose`, `--keep-temp` |
| `dual-exec-verify.sh` | Batch interpreter vs LLVM | `--test-only`, `--main-only`, `--json` |
| `rc-stats.sh` | RC balance per function | `--block-level`, `--optimized`, `--compare-awk` |
| `codegen-audit.sh` | Static RC/COW/ABI analysis | `--strict`, `--function` |
| `ir-dump.sh` | LLVM IR | `--raw`, `--optimized`, `--function` |
| `arc-dump.sh` | ARC IR post-lowering | `--raw`, `--function` |
| `ir-diff.sh` | Compare two programs' IR | `--raw`, `--optimized`, `--function`, `--context` |
| `disasm-ori.sh` | Native disassembly | `--all`, `--function`, `--symbols` |
| `bisect-passes.sh` | AIMS pipeline phase bisection | `--function`, `--rc-only` |
| `debug-release-compare.sh` | Debug vs release comparison | `--verbose` |
| `valgrind-aot.sh` | Valgrind memory errors | defaults to `tests/valgrind/` |
| `check-debug-flags.sh` | Validate `ORI_*` flag consistency | |
| `self-test.sh` | Self-test all scripts against fixtures | |

**Data sources:**
- `rc-stats.sh` consumes compiler JSON via `ORI_AUDIT_CODEGEN=1` (SSOT: `RcOpKind` in `rc_histogram.rs`)
- `codegen-audit.sh` consumes `codegen audit:` lines from `ORI_AUDIT_CODEGEN=1`
- `bisect-passes.sh` consumes `ori_arc::aims::pipeline` tracing events via `ORI_LOG=ori_arc::aims::pipeline=info`
- `ir-dump.sh` / `arc-dump.sh` use `ORI_DUMP_AFTER_LLVM=1` / `ORI_DUMP_AFTER_ARC=1`

**Environment:**
- `ORI_BIN` — override path to ori binary (used by most scripts)
- `ORI_AUDIT_CODEGEN=1` — enable in-pipeline audit (add `ORI_AUDIT_STRICT=1` | `ORI_AUDIT_FUNCTION=name`)

**Self-test:** `diagnostics/self-test.sh` — runs all scripts against fixtures
