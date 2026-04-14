---
paths:
  - "**diagnostic**"
---

# Diagnostics

## Error Codes
- **E0xxx**: Lexer | **E1xxx**: Parser | **E2xxx**: Type checker | **E3xxx**: Pattern/semantic | **E4xxx**: ARC | **E5xxx**: Codegen/LLVM | **E6xxx**: Runtime/eval | **E9xxx**: Internal | **W1xxx/W2xxx**: Warnings
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
| `alive2-verify.sh` | Alive2 translation validation | `--corpus`, `--all-codegen`, `--function`, `--json`, `--check-survival`, `--review-suppressions`, `--strict` |
| `check-debug-flags.sh` | Validate `ORI_*` flag consistency | |
| `repo-hygiene.sh` | Detect/clean untracked temp files | `--check`, `--clean`, `--gitignore` |
| `self-test.sh` | Self-test all scripts against fixtures | |

**Data sources:**
- `rc-stats.sh` consumes compiler JSON via `ORI_AUDIT_CODEGEN=1` (SSOT: `RcOpKind` in `rc_histogram.rs`)
- `codegen-audit.sh` consumes `codegen audit:` lines from `ORI_AUDIT_CODEGEN=1`
- `bisect-passes.sh` consumes `ori_arc::aims::pipeline` tracing events via `ORI_LOG=ori_arc::aims::pipeline=info`
- `ir-dump.sh` / `arc-dump.sh` use `ORI_DUMP_AFTER_LLVM=1` / `ORI_DUMP_AFTER_ARC=1`
- `alive2-verify.sh` consumes `ORI_ALIVE2_CAPTURE=1` IR files (`build/alive2-results/*.preopt.ll`, `*.postopt.ll`) and runs `alive-tv`

**Environment:**
- `ORI_BIN` — override path to ori binary (used by most scripts)
- `ORI_AUDIT_CODEGEN=1` — enable in-pipeline audit (add `ORI_AUDIT_STRICT=1` | `ORI_AUDIT_FUNCTION=name`)

**Self-test:** `diagnostics/self-test.sh` — runs all scripts against fixtures

## Verification Flags (from LLVM Verification Tooling Plan)

These environment variables enable deeper verification during compilation/execution:

| Flag | Purpose | Performance impact |
|------|---------|-------------------|
| `ORI_VERIFY_ARC=1` | ARC IR correctness checks + per-function LLVM IR verification at all emission sites | ~10-20% slower |
| `ORI_VERIFY_EACH=1` | LLVM IR verification after every optimization pass — catches which pass breaks IR | ~30-60% slower |
| `ORI_LLVM_LINT=1` | LLVM `function(lint)` pass: division by zero, suspicious alignment, unreachable. Auto-enabled by `ORI_AUDIT_CODEGEN=1` | ~5% slower |
| `ORI_SANITIZE=address,undefined` | ASan/UBSan on generated AOT binaries via Clang delegation | 2-10x slower |
| `ORI_BLESS=1` | Bless mode for snapshot tests — write actual as new baseline (only `"1"` accepted) | N/A |
| `ORI_DUMP_PREOPT_LLVM=1` | Dump pre-optimization LLVM IR to `.preopt.ll` file (after verify, before opt) | Negligible |
| `ORI_DUMP_POSTOPT_LLVM=1` | Dump post-optimization LLVM IR to `.postopt.ll` file (after opt, before emit) | Negligible |
| `ORI_ALIVE2_CAPTURE=1` | Both pre/post-opt IR into `build/alive2-results/` for alive-tv | Negligible |

**Combining flags for deep verification:**
```bash
ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1 ORI_AUDIT_CODEGEN=1 ori build file.ori  # maximum verification
ORI_CHECK_LEAKS=1 ORI_RT_DEBUG=1 ./binary                                   # maximum runtime checks
ORI_ALIVE2_CAPTURE=1 ori build file.ori --opt=2                              # alive-tv IR capture
```

## Verification Test Suites

Built by `plans/llvm-verification-tooling/`. Run these when touching ARC, LLVM codegen, or optimization.

| Suite | Command | Tests | What it catches |
|-------|---------|-------|----------------|
| AIMS Snapshots | `cargo test -p oric --test aims_snapshots` | 22 | Per-pass ARC IR regressions (reuse, merge, normalize, realize, tail calls) |
| FileCheck IR | `cargo test -p ori_llvm --test codegen_checks` | 44+ | LLVM IR pattern regressions (RC emission, COW, ABI, iterators, closures) |
| Lattice Properties | `cargo test -p ori_arc -- lattice::prop_tests` | 36 | Join law violations, partial-order axiom breaks, fixpoint divergence |
| Contract Oracle | `cargo test -p ori_arc -- oracle` | 8 | Analysis/realization mismatches in MemoryContract |
| Protocol Builtins | `cargo test -p ori_arc -- builtins::tests` | 11 | Protocol builtin ownership matrix consistency |
| Sanitizer Smoke | `scripts/sanitizer-smoke.sh` | 17 programs | ASan/UBSan runtime memory safety violations |
| Alive2 Curated | `diagnostics/alive2-verify.sh --corpus` | 8 functions | LLVM optimization correctness via SMT (translation validation) |
| Alive2 Full Sweep | `diagnostics/alive2-verify.sh --all-codegen` | all codegen | Weekly: full codegen test set through alive-tv |

**Test corpus locations:**
- `compiler/oric/tests/aims-snapshots/` — AIMS snapshot `.ori` files and `.arc` baselines
- `compiler/ori_llvm/tests/codegen/` — FileCheck `.ori` files with `// CHECK:` directives
- `compiler/ori_test_harness/` — shared harness crate (directives, bless, CHECK matching)
- `tests/alive2/` — Alive2 corpus (`.ori` files, `curated-corpus.txt`, `suppressed.json`, `results-schema.json`)
