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
│   ├── 001_100_doors.ori          # Implementation with @main
│   └── _test/
│       └── 001_100_doors.test.ori # Tests (use std.testing { assert_eq })
├── 002_100_prisoners/
│   └── ...
└── ...
```

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

### Phase F: Performance Benchmarking
- `time ori run <task>.ori` (3 runs, median) — interpreter speed
- `time /tmp/<task>_debug` (3 runs, median) — AOT debug speed
- `time /tmp/<task>_release` (3 runs, median) — AOT release speed
- AOT compile times (debug + release)
- Speedup ratios: AOT-vs-interpreter, debug-vs-release

### Phase G: Bug Filing & Findings
- Compiler crash/ICE → `/add-bug`
- Wrong output → `/add-bug`
- Memory leak → `/add-bug`
- Missing language feature → blocker with roadmap cross-reference
- Bad error message → `/add-bug`
- Performance anomaly → investigate → `/add-bug` if codegen issue
- Update `rosetta-manifest.json` with status and findings

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
| `bugs_filed` | string[] | Bug IDs filed (e.g., `["BUG-03-042"]`) |
| `language_findings` | string[] | Language design observations |
| `perf` | object | Timing data (interp, AOT debug/release, compile times, speedup ratios) |
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
