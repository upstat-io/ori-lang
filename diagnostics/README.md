# Diagnostic Scripts

Quick-access debugging tools for the Ori compiler's AOT/codegen pipeline. These scripts extract more signal in seconds than manual investigation in minutes.

**Prerequisite**: An LLVM-enabled `ori` binary. Build with `cargo b` (debug) or `cargo b --release` (release).

## Quick Reference

| Script | Purpose | When to use |
|--------|---------|-------------|
| `diagnose-aot.sh` | All-in-one: compile + run + leak check + RC stats + IR | First tool to reach for on any AOT bug |
| `dual-exec-debug.sh` | Compare interpreter vs AOT output | Wrong output — is it eval or codegen? |
| `dual-exec-verify.sh` | Batch interpreter vs LLVM verification | CI parity gate, coverage audits |
| `codegen-audit.sh` | Static RC/COW/ABI analysis of LLVM IR | RC corruption, double-free, ABI mismatch |
| `rc-stats.sh` | RC operation count per function | Leak or over-release suspicion (`--block-level`, `--optimized`, `--rc-remarks`) |
| `ir-dump.sh` | Annotated LLVM IR with color-coded RC ops | Understanding what codegen actually emits |
| `arc-dump.sh` | Annotated ARC IR (post-lowering, pre-RC) | Debugging AIMS pipeline: alias chains, take-projects, lineage |
| `ir-diff.sh` | Side-by-side IR comparison of two programs | Regression hunting, before/after comparison |
| `disasm-ori.sh` | Native disassembly with Ori symbol demangling | Instruction-level debugging |
| `bisect-passes.sh` | Identify which AIMS pipeline phase introduced an RC or structural change | After `diagnose-aot.sh` finds a leak/crash (`--function`, `--rc-only`) |
| `burden-balance.sh` | VF-1 `verify_burden_balance` imbalance count (default, per-var) OR per-same-alloc-lineage post-lowering RC-net (`--lineage-net`, cross-var) over a corpus | Measuring faithful Phase-5 burden-emission residual (`--files`, `--raw`, `--release`); `--lineage-net` surfaces dup-alias double-frees (net<0) VF-1's per-var count is blind to (a lineage nets 0 per-var, -N cross-var); REBUILD the dev binary first — a stale binary yields false counts |
| `debug-release-compare.sh` | Compare debug vs release build output | FastISel-only bugs, optimization divergences |
| `class-ledger-census.sh` | Single-leg readiness census: per-function replaced vs fallback counts + ranked fallback-reason table over a corpus, under the gated burden-sole env; `--run` adds plain + leak-check behavior verdicts | The drain worklist for retiring the legacy fallback walk (`--limit`, `--family`, `--run`, `--timeout`) |
| `check-debug-flags.sh` | Validate `ORI_*` flag consistency | After adding/removing debug flags |
| `check-tracing-coverage.sh` | Validate direct tracing dependencies and required parser spans | After changing tracing dependencies or parser boundaries |
| `repo-hygiene.sh` | Detect/clean untracked temp files | Subsection close-out, section completion (`--check`, `--clean`) |
| `verify-build-stamp-freshness.sh` | Verify `oric`'s `build.rs` re-executes and refreshes its git-identity stamp on an ordinary rebuild | After touching `compiler/oric/build.rs` or its `git()`-based stamping logic |
| `self-test.sh` | Self-test all scripts against fixtures | After modifying any diagnostic script |

## Usage

### diagnose-aot.sh — All-in-One Diagnostic

```bash
diagnostics/diagnose-aot.sh file.ori              # Standard battery
diagnostics/diagnose-aot.sh --valgrind file.ori    # + Valgrind memory error detection
diagnostics/diagnose-aot.sh --rc-trace file.ori    # + ORI_TRACE_RC during execution
diagnostics/diagnose-aot.sh --verbose file.ori     # + native disassembly
diagnostics/diagnose-aot.sh --release file.ori     # Use release build instead of debug
diagnostics/diagnose-aot.sh --both-builds file.ori # Full battery on BOTH debug and release, then compare
```

Runs 5-7 checks in sequence: compilation, execution, leak check (`ORI_CHECK_LEAKS=1`), RC stats, LLVM IR dump, and optionally Valgrind and disassembly. With `--both-builds`, runs the full battery twice (debug then release) and shows a per-section comparison table.

### dual-exec-debug.sh — Backend Comparison

```bash
diagnostics/dual-exec-debug.sh file.ori            # Compare eval vs AOT
diagnostics/dual-exec-debug.sh --verbose file.ori   # + ORI_LOG=debug traces on both
diagnostics/dual-exec-debug.sh --keep-temp file.ori # Preserve diagnostic artifacts on mismatch
```

On mismatch, automatically runs `ir-dump.sh`, `arc-dump.sh`, `rc-stats.sh`, and `codegen-audit.sh` to diagnose the difference. On build failure, attempts ARC IR capture (ARC IR is emitted before codegen, so may be available even when LLVM fails).

### dual-exec-verify.sh — Batch Dual-Execution Verification

```bash
diagnostics/dual-exec-verify.sh                          # All spec tests
diagnostics/dual-exec-verify.sh tests/spec/expressions/  # Specific directory
diagnostics/dual-exec-verify.sh --test-only              # Skip @main programs
diagnostics/dual-exec-verify.sh --main-only              # Skip @test functions
diagnostics/dual-exec-verify.sh --json                   # Emit JSON report
diagnostics/dual-exec-verify.sh -v                       # Show every verified test
```

Runs all spec tests through both interpreter and LLVM backends, cross-references results to detect behavioral mismatches.

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0` | All verified — at least one test compared, no mismatches |
| `1` | Behavioral mismatches found (PASS in one backend, FAIL in other) |
| `2` | Infrastructure error (build failure, binary not found) |
| `3` | Zero verifications — no tests were actually compared across backends |

Exit code `3` guards against false confidence: a directory where all tests hit LLVM compile failures produces zero verifications, which is distinct from "all tests passed."

### codegen-audit.sh — Static IR Analysis

```bash
diagnostics/codegen-audit.sh file.ori                        # Standard analysis
diagnostics/codegen-audit.sh --strict file.ori               # Pessimistic mode
diagnostics/codegen-audit.sh --function my_func file.ori     # Filter to specific function
```

Three analysis categories:
1. **RC Balance** — alloc/inc/dec/free lifecycle per function
2. **COW Correctness** — no pointer reuse or dec before COW calls
3. **ABI Conformance** — no large aggregate loads (>16B), correct arg counts

### rc-stats.sh — RC Operation Counts

```bash
diagnostics/rc-stats.sh file.ori                             # Count RC ops per function
diagnostics/rc-stats.sh --block-level file.ori               # Per-block breakdown within each function
diagnostics/rc-stats.sh --optimized file.ori                  # After LLVM optimization passes
diagnostics/rc-stats.sh --block-level --optimized file.ori   # Per-block on optimized IR
diagnostics/rc-stats.sh --compare-awk file.ori               # Migration check: compare JSON vs legacy awk
diagnostics/rc-stats.sh --rc-remarks file.ori                # Per-function surviving-RC summary from the burden-sole remark stream
```

Consumes compiler JSON via `ORI_AUDIT_CODEGEN=1` — SSOT is `RcOpKind` in `rc_histogram.rs`. Balance = `(alloc + inc) - (dec + free)`. Positive = potential leak. Negative = potential over-release. Per-block balance is informational; only function-level balance affects exit code.

`--rc-remarks` is a separate path: it builds the file with `--emit-rc-remarks <tmp>` (which auto-sets the burden-sole gating `ORI_DISABLE_PREDICATE_STACK_RC=1` + `ORI_VERIFY_ARC=1`), then prints a per-function count of surviving RC operations parsed from the emitted JSONL stream. A surviving op is one the AIMS burden path could not prove redundant; each carries a `cause` (proof-failure + lattice dimension). The RC verdict is valid ONLY on the burden-sole path.

### RC-survivor remark stream

The compiler emits one JSONL `missed` remark per surviving RC operation when given a sink path:

```bash
ori build file.ori --emit-rc-remarks survivors.jsonl   # CLI flag (auto-composes burden-sole gating)
ORI_RC_REMARKS=survivors.jsonl ori build file.ori       # env var (compose the gating yourself)
```

The stream opens with a `header` record (`schema_version`, `compiler_sha`, `source_file`, `burden_path`) followed by per-op `missed` remarks (`rc_op`, `function`, `debug_loc`, `cause`, `cow_mode`). It is the AIMS analog of LLVM's `-fsave-optimization-record`.

Analyze the stream with the `ori-rc-remarks` tool (standalone crate at `compiler_repo/tools/ori-rc-remarks`):

```bash
cargo run --manifest-path tools/ori-rc-remarks/Cargo.toml -- survivors.jsonl              # summary
cargo run --manifest-path tools/ori-rc-remarks/Cargo.toml -- --stats survivors.jsonl      # cause-cluster ranking
cargo run --manifest-path tools/ori-rc-remarks/Cargo.toml -- --view survivors.jsonl        # source-annotated survivor view
cargo run --manifest-path tools/ori-rc-remarks/Cargo.toml -- --diff base.jsonl cand.jsonl  # two-build comparison
```

These are the opt-stats / opt-viewer / opt-diff analogs for AIMS RC survivors.

### ir-dump.sh — LLVM IR Dump

```bash
diagnostics/ir-dump.sh file.ori                    # Annotated, color-coded IR
diagnostics/ir-dump.sh --raw file.ori              # Raw IR without annotations
diagnostics/ir-dump.sh --optimized file.ori         # After LLVM optimization passes
diagnostics/ir-dump.sh --function main file.ori     # Single function only
```

### arc-dump.sh — ARC IR Dump (post-lowering, pre-RC)

```bash
diagnostics/arc-dump.sh file.ori                    # Annotated, color-coded ARC IR
diagnostics/arc-dump.sh --raw file.ori              # Raw IR without annotations
diagnostics/arc-dump.sh --function main file.ori    # Single function only
```

Captures the typed ARC IR via `ORI_DUMP_AFTER_ARC=1` — the IR after CanExpr lowering but before AIMS RC emission. Use this when debugging take-projects, alias chains, block params (phi merges), and `Project` / `Construct` / `Apply` / RC instructions. For LLVM IR (post-codegen) use `ir-dump.sh` instead.

### ir-diff.sh — IR Comparison

```bash
diagnostics/ir-diff.sh a.ori b.ori                 # Normalized diff
diagnostics/ir-diff.sh --raw a.ori b.ori           # Exact diff (no normalization)
diagnostics/ir-diff.sh --function main a.ori b.ori  # Single function comparison
```

Normalization strips debug metadata, TBAA, block label counters, and trailing whitespace.

### disasm-ori.sh — Native Disassembly

```bash
diagnostics/disasm-ori.sh file.ori                 # User functions only
diagnostics/disasm-ori.sh --all file.ori           # Include runtime functions
diagnostics/disasm-ori.sh --function main file.ori  # Single function
diagnostics/disasm-ori.sh --symbols file.ori       # Symbol list only (no disasm)
```

Demangling: `_ori_math$add` → `math.add`, `_ori_int$$Eq$eq` → `int impl Eq.eq`

### bisect-passes.sh — AIMS Pipeline Phase Bisection

```bash
diagnostics/bisect-passes.sh file.ori                      # Full per-function phase table
diagnostics/bisect-passes.sh --function main file.ori      # Filter to main function
diagnostics/bisect-passes.sh --rc-only file.ori            # Suppress structural metric columns
```

Compiles with `ORI_LOG=ori_arc::aims::pipeline=info`, captures per-phase checkpoint events, and displays a table showing how RC counts and structural metrics (block count, var count) evolve across AIMS pipeline phases. The first phase where RC balance changes from 0 is flagged as the potential divergence point; phases with structural changes (block merging, var count changes) are also highlighted. After compilation, runs the binary with `ORI_CHECK_LEAKS=1` to check for runtime leaks.

**Workflow integration**: Use after `diagnose-aot.sh` identifies a leak or crash to narrow down to the specific pipeline phase.

### class-ledger-census.sh — Class-Ledger Readiness Census

```bash
diagnostics/class-ledger-census.sh                        # Default: tests/spec @main programs (limit 100)
diagnostics/class-ledger-census.sh --limit 2500 tests/spec
diagnostics/class-ledger-census.sh compiler/ori_llvm/tests/aot/fixtures --limit 2500
diagnostics/class-ledger-census.sh --family traits -v     # Filter corpus + show every result
diagnostics/class-ledger-census.sh --run                  # Also execute: plain run + ORI_CHECK_LEAKS=1 run
```

Single-leg readiness census for the (unconditional) class-ledger emitter: builds each corpus program under the gated burden-sole env (`ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1`) with `ORI_LOG=ori_arc::aims::class_ledger=debug`, then tallies per-function `mode=replaced` vs `mode=fallback` counts and a ranked `fallback_reason` table with per-site detail — the drain worklist for retiring the legacy fallback walk. `--run` adds a behavior verdict per program: a plain run AND an `ORI_CHECK_LEAKS=1` run (leak checking can mask use-after-free, so both runs are required for a verdict).

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0` | Census completed (fallbacks are the worklist, not failures) |
| `1` | Behavior failures under `--run` |
| `2` | Infrastructure error (binary not found, bad arguments) |
| `3` | Zero programs censused — misleading "all clear" |

### debug-release-compare.sh — Debug vs Release Comparison

```bash
diagnostics/debug-release-compare.sh file.ori            # Compare debug vs release
diagnostics/debug-release-compare.sh --verbose file.ori   # + LLVM IR diff and RC stats on mismatch
```

Compiles and runs through both `target/debug/ori` and `target/release/ori`, comparing exit codes and stdout. On mismatch, auto-dumps LLVM IR from both builds for diffing. Catches FastISel-only bugs (e.g., the >16B aggregate load issue) and optimization-dependent codegen divergences.

**Prerequisite**: Both debug and release binaries must exist. Build with `cargo b` and `cargo b --release`.

### check-debug-flags.sh — Flag Consistency

```bash
diagnostics/check-debug-flags.sh                   # Validate all ORI_* flags
```

Checks: stale flags (defined but unused), orphan checks (used but undefined), undocumented flags (missing from project guidance).

### repo-hygiene.sh — Worktree Cleanliness

```bash
diagnostics/repo-hygiene.sh                        # List detected temp/scratch files
diagnostics/repo-hygiene.sh --check                # Exit 1 if temp files found (CI/skill gate)
diagnostics/repo-hygiene.sh --clean                # Remove detected temp files
diagnostics/repo-hygiene.sh --gitignore            # Suggest .gitignore patterns for detected files
```

Detects untracked temp files by category: **DUMP** (debug/IR dumps), **SCRATCH** (one-off test scripts), **BACKUP** (editor merge artifacts), **ARTIFACT** (stray build outputs), **STALE** (core dumps). Integrated into close-out and completion checks.

### verify-build-stamp-freshness.sh — Build-Stamp Regression Check

```bash
diagnostics/verify-build-stamp-freshness.sh                # Static gate + informational live-rebuild demo
diagnostics/verify-build-stamp-freshness.sh --no-color      # Disable color output
```

Static check (gating): `compiler/oric/build.rs` must emit no `cargo:rerun-if-changed=` / `cargo:rerun-if-env-changed=` line — the absence of any such line is what makes Cargo's default whole-package rebuild fallback govern. Informational check (non-gating): a warm build, an unstaged mtime touch on a tracked file, and a rebuild, reporting whether the build script visibly reran and whether the stamped `ORI_GIT_DIRTY` matches a live `git status --porcelain` reading; a miss reports exit 3 without failing the script. Exit 1 (the static regression) is the only failure.

### self-test.sh — Script Self-Test

```bash
diagnostics/self-test.sh                           # Run all fixture tests
diagnostics/self-test.sh --verbose                  # Detailed output
```

## Environment Variables

All scripts auto-detect the `ori` binary. Override with `ORI_BIN`:

```bash
ORI_BIN=./target/release/ori diagnostics/diagnose-aot.sh file.ori
```

### Compiler Debug Flags

These environment variables control the compiler and runtime instrumentation. They are zero-cost when disabled.

| Variable | Where | Purpose |
|----------|-------|---------|
| `ORI_LOG` | Compiler | Tracing filter (`RUST_LOG` syntax). Targets: `ori_types`, `ori_eval`, `ori_llvm`, `ori_arc`, `oric` |
| `ORI_LOG_TREE=1` | Compiler | Hierarchical tree output with indentation |
| `ORI_DUMP_AFTER_PARSE=1` | Compiler | Dump AST after parse phase |
| `ORI_DUMP_AFTER_TYPECK=1` | Compiler | Dump typed IR after type checking |
| `ORI_DUMP_AFTER_ARC=1` | Compiler | Dump ARC IR with RC strategy annotations |
| `ORI_DUMP_AFTER_LLVM=1` | Compiler | Dump annotated LLVM IR (superset of `ORI_DEBUG_LLVM`) |
| `ORI_TRACE_IDX=<n>` | Compiler | Provenance DAG (structure/resolution/mono edges, generic-leaf divergence, drop-glue attribution) for type-pool index `<n>` to stderr. CLI equivalent: `ori explain idx <n> <file.ori>` (DAG to stdout). Discover `<n>` with `ORI_DUMP_AFTER_TYPECK=1 ORI_DUMP_TYPE_IDX=1` |
| `ORI_AUDIT_CODEGEN=1` | Compiler | In-pipeline RC/COW/ABI verification |
| `ORI_AUDIT_STRICT=1` | Compiler | Pessimistic audit mode (with `ORI_AUDIT_CODEGEN`) |
| `ORI_AUDIT_FUNCTION=name` | Compiler | Filter audit to functions matching substring |
| `ORI_TRACE_RC=1` | Runtime | Log every RC operation (alloc/inc/dec/free) |
| `ORI_RT_DEBUG=1` | Runtime | Enable runtime assertions (header validation, bounds) |
| `ORI_CHECK_LEAKS=1` | Runtime | Report live RC objects on exit |

## Common Debugging Workflows

### "The program outputs the wrong value"

```bash
diagnostics/dual-exec-debug.sh file.ori
# If eval is correct but AOT is wrong → codegen bug
# If both are wrong → evaluator bug (or spec misunderstanding)
```

### "The program crashes or segfaults"

```bash
diagnostics/diagnose-aot.sh --valgrind file.ori
# Check: use-after-free, double-free, stack overflow
# Then: diagnostics/rc-stats.sh to check RC balance
```

### "Memory leak suspected"

```bash
ORI_CHECK_LEAKS=1 ./binary                                    # Quick check
diagnostics/rc-stats.sh file.ori                               # Which function is imbalanced?
diagnostics/rc-stats.sh --block-level file.ori                 # Which block within that function?
ORI_TRACE_RC=1 ./binary 2>&1 | grep -v inc | head              # What's allocated but never freed?
```

### "Codegen looks wrong"

```bash
diagnostics/ir-dump.sh file.ori                      # See what we emit
diagnostics/ir-dump.sh --optimized file.ori           # See what LLVM makes of it
diagnostics/codegen-audit.sh --strict file.ori        # Static correctness check
```

### "Debug works but release crashes/differs"

```bash
diagnostics/debug-release-compare.sh --verbose file.ori
# Shows exit code + stdout comparison, then LLVM IR diff and RC stats
# Common cause: FastISel (debug) handles something that the full pipeline (release) does not
```

### "Regression between two versions"

```bash
diagnostics/ir-diff.sh old_version.ori new_version.ori
# Or save IR from before your change, compare after:
diagnostics/ir-dump.sh --raw file.ori > before.ll
# ... make changes, rebuild ...
diagnostics/ir-dump.sh --raw file.ori > after.ll
diff before.ll after.ll
```

## Fixtures

Test fixtures in `fixtures/` exercise different codegen patterns. See `fixtures/FIXTURES.md` for the canonical SSOT.

**Pass fixtures** (exit 0, balanced RC):

| Fixture | What it tests |
|---------|--------------|
| `simple.ori` | Minimal program — no collections, no RC (baseline) |
| `clean.ori` | Collections + balanced RC, list ops |
| `chain.ori` | Chained COW ops, sequential mutation |
| `closure.ori` | Closure capture + call, closure env RC |
| `closure_escape.ori` | Escaping closures, lifetime beyond scope |
| `iterator_break.ori` | Iterator early exit, elem cleanup |
| `iterator_complex.ori` | Nested/yield/guard iteration, partial collect |
| `nested_list.ori` | Nested collections, elem_dec_fn propagation |
| `trait_dispatch.ori` | Trait method dispatch, vtable codegen |
| `pattern_match.ori` | Sum type mixed variants, per-variant drop |
| `map_iteration.ori` | Map create + iterate, iterator cleanup |

**AIMS-heavy fixtures** (exit 0, exercises AIMS-specific paths):

| Fixture | What it tests |
|---------|--------------|
| `question_mark.ori` | `?` with fat values, early-exit unwinding |
| `recursive_tree.ori` | Recursive fat pointer passing, stack-frame RC |
| `generic_mono.ori` | Multi-type generic instantiation, monomorphization RC |
| `large_aggregate.ori` | >16B struct pass/return, ABI compliance |
| `cow_sharing.ori` | COW sharing/fork, is_unique barrier |

**Expected-fail fixtures** (exit non-zero, validates failure detection):

| Fixture | What it tests |
|---------|--------------|
| `leak.ori` | Panic with fat values, leak detection path |
| `mismatch.ori` | Interpreter vs AOT mismatch detection (via `mismatch-wrapper.sh`) |
| `build-fail-parse.ori` | Parse error, build failure detection |

**Infrastructure** (supporting wrappers, not standalone fixtures):

| Fixture | What it tests |
|---------|--------------|
| `mismatch-wrapper.sh` | ORI_BIN wrapper for mismatch — injects deterministic divergence |

## Common Options

All scripts support:
- `--help` / `-h` — usage information
- `--no-color` — disable color output (for piping/logging)
- `--color` — force color output (overrides auto-detection)
