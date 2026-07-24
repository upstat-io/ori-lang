# Rosetta Code Compiler Stress Test

Methodical implementation of [Rosetta Code](https://rosettacode.org) tasks in Ori, treating each program as a **deep language evaluation**. The bugs found, language insights recorded, and compiler improvements made are the primary deliverable — the Rosetta tasks are fuel.

## Folder Structure

```
rosetta/
├── README.md
├── rosetta-manifest.json          # SSOT: per-program status, findings, perf data
├── _tasks/                        # 599 task definition files (from Rosetta Code)
│   ├── 001_100_doors.md
│   ├── 002_100_prisoners.md
│   └── ...
├── 001_100_doors/                 # Per-program folder (numbered by _tasks/ index)
│   ├── task.md                    # Task definition (copied from _tasks/)
│   ├── 001_100_doors.ori          # Idiomatic implementation with @main (ergonomics subject)
│   ├── _test/
│   │   └── 001_100_doors.test.ori # Tests (use std.testing { assert_eq })
│   ├── ori/
│   │   └── 100_doors.ori          # Microbench variant: salt + N-loop + checksum
│   ├── c++/
│   │   └── 100_doors.cpp          # Hand-optimized "efficient C++" baseline (compiled gate)
│   └── python/
│       └── 100_doors.py           # Idiomatic Python3 baseline (interpreted gate)
├── 002_100_prisoners/
│   └── ...
└── ...
```

The microbench variant, C++ baseline, and Python baseline all run the SAME algorithm with the SAME salt trick (a counter-dependent extra step that defeats loop-invariant hoisting), the SAME iteration count, and print the SAME checksum. The microbench is the apples-to-apples perf subject (built to a binary for the compiled gate; run via `ori run` for the interpreted gate); the idiomatic top-level `.ori` is the readability / ergonomics subject.

## Per-Program Pipeline

Every program undergoes ALL phases — no shortcuts.

### Phase A: Language Design Analysis
- Read task definition, design the most idiomatic Ori solution
- Push the full feature set: generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`
- Write implementation + `@main` + comprehensive tests
- Document language findings: where Ori shines, where it forces workarounds

### Phase B: Compiler Correctness (Lexer → Parser → Typeck → Eval)
```bash
timeout 30 cargo run -- check <task>.ori                          # Type errors, inference failures
ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check <task>.ori   # Parser AST structure
ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check <task>.ori  # Type resolution
timeout 30 cargo run -- test <task>/_test/<task>.test.ori          # Interpreter correctness
timeout 30 cargo run -- run <task>.ori                             # @main interpreter output
```

### Phase C: LLVM Codegen & AOT
```bash
timeout 60 cargo run -- build <task>.ori -o /tmp/<task>_debug       # AOT debug build
timeout 60 cargo run --release -- build <task>.ori -o /tmp/<task>_release  # AOT release build
ORI_DUMP_AFTER_LLVM=1 cargo run -- build <task>.ori                 # LLVM IR inspection
ORI_DUMP_AFTER_ARC=1 cargo run -- build <task>.ori                  # ARC IR inspection
/tmp/<task>_debug                                                   # Run debug binary
/tmp/<task>_release                                                 # Run release binary
diagnostics/dual-exec-debug.sh <task>.ori                           # Interpreter vs AOT mismatch
diagnostics/debug-release-compare.sh <task>.ori                     # Debug vs release divergence
```

### Phase D: Memory & ARC Verification
```bash
ORI_CHECK_LEAKS=1 /tmp/<task>_debug           # Memory leaks
ORI_TRACE_RC=1 /tmp/<task>_debug              # RC event trace
ORI_RT_DEBUG=1 /tmp/<task>_debug              # Runtime assertions
ORI_VERIFY_ARC=1 cargo run -- build <task>.ori       # ARC IR correctness
ORI_VERIFY_EACH=1 cargo run -- build <task>.ori      # Per-pass LLVM IR verify
ORI_LLVM_LINT=1 cargo run -- build <task>.ori        # Likely-UB patterns
diagnostics/rc-stats.sh <task>.ori                   # Per-function RC balance
diagnostics/rc-stats.sh --block-level <task>.ori     # Per-block RC breakdown
diagnostics/codegen-audit.sh <task>.ori              # Static codegen analysis
diagnostics/codegen-audit.sh --strict <task>.ori     # Pessimistic analysis
```

### Phase E: Debug Symbols & Binary Quality
```bash
readelf --debug-dump=info /tmp/<task>_debug | grep DW_TAG_subprogram  # Function symbols
readelf --debug-dump=line /tmp/<task>_debug                           # Line number tables
stat --printf="%s" /tmp/<task>_{debug,release}                        # Binary sizes
```

### Phase F: Performance Gates — Compiled vs C++ AND Interpreted vs Python (MANDATORY)
Two gates; a program PASSES only when BOTH are green. A `loss` on either is a compiler/AIMS/interpreter defect to fix, never an accepted result.
- Build the microbench (`ori/<task>.ori`) to a release binary; build `c++/<task>.cpp` with `-O3`
- 3-way checksum gate (correctness): Ori microbench output == C++ output == Python output
- **Compiled gate**: Ori AOT release median must match-or-beat C++ -O3 median (5% wall-clock tolerance)
- **Interpreted gate**: Ori `ori run` median must match-or-beat Python3 median — it must NEVER be slower (tolerance 0)
- Also record interpreter/AOT-debug/AOT-release timings + compile times + speedup ratios for the idiomatic program
- Gate verdicts (perf comparison, checksum equality, backfill detection) are computed by the evaluation tooling, not by hand

### Phase G: Cross-Language Intelligence
- Query reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean) for prior art on the program's key features and any bug encountered
- Record whether Ori's approach matches best-of-breed

### Phase H: Ergonomics, Beauty & AI-Efficiency
- Score the idiomatic `.ori` on visual clarity, conciseness, AI-writability, AI-readability, idiom fit (1-5), grounded in reference-language sentiment
- Record friction points + syntax-improvement candidates; surface proposal-worthy candidates for discussion (interactive runs)

### Phase I: Bug Filing & Findings
- Compiler crash/ICE → `/add-bug`
- Wrong output → `/add-bug`
- Memory leak → `/add-bug`
- Perf-gate `loss` (compiled or interpreted) → `/add-bug` + fix (gate is not optional)
- Missing language feature → blocker with roadmap cross-reference
- Bad error message → `/add-bug`
- Performance anomaly → investigate → `/add-bug` if codegen issue
- Update `rosetta-manifest.json` with status and findings

> Phases J (`/tpr-review`) and K (Results Report) follow. The per-program plan section file is the authoritative phase checklist; this README is the orientation summary.

## Manifest Schema (`rosetta-manifest.json`)

Each program entry tracks:

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | `not-started`, `in-progress`, `blocked`, `complete` |
| `tier` | int | Complexity tier (1=basic, 2=intermediate, 3=advanced) |
| `features` | string[] | Ori features exercised (e.g., `["generics", "closures"]`) |
| `has_main` | bool | Has `@main` entry point |
| `has_tests` | bool | Has `_test/` with assertions |
| `aot_eligible` | bool? | Can compile to AOT binary |
| `skip_reason` | string? | Why blocked (references roadmap/bug-tracker) |
| `bugs_filed` | string[] | Tracker bug IDs filed for this program |
| `language_findings` | string[] | Language design observations |
| `perf` | object | Timing data (interp, AOT debug/release, compile times, speedup ratios) |
| `perf_vs_cpp` | object | Compiled gate: `{ori_median_ms, baseline_median_ms, ratio, tolerance, passed, verdict}` vs C++ -O3 |
| `interp_vs_python` | object | Interpreted gate: `{ori_median_ms, baseline_median_ms, ratio, tolerance, passed, verdict}` vs Python3 |
| `checksum_match` | bool | 3-way cross-language checksum equality (Ori == C++ == Python) |
| `ergonomics` | object | `{scorecard: {scores, friction_points, syntax_candidates, verdict, grounded_in_intel}}` |
| `proposals_opened` | string[] | Draft proposals opened from syntax-improvement candidates |
| `binary_sizes` | object | Debug, release, stripped binary sizes in bytes |
| `verification` | object | Pass/fail for each verification check |

## Running

```bash
# Run a specific program
cargo run -- run rosetta/001_100_doors/001_100_doors.ori

# Run tests for a program
cargo run -- test rosetta/001_100_doors/_test/001_100_doors.test.ori

# Full test suite (includes all rosetta tests)
./test-all.sh
```

## Plan

See for the full execution plan. Programs are implemented 15 at a time, with each batch informing task selection for the next.
