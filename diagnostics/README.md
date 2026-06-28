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
| `check-debug-flags.sh` | Validate `ORI_*` flag consistency | After adding/removing debug flags |
| `repo-hygiene.sh` | Detect/clean untracked temp files | Subsection close-out, section completion (`--check`, `--clean`) |
| `tpr-failure-summary.sh` | Summarize TPR failure patterns across runs | Investigating Gemini/Codex failures, capacity errors |
| `tpr-liveness.sh` | Classify a TPR reviewer sub-agent as alive/quiet/dead | `/tpr-review` §9 retry decision — BEFORE deciding a silent reviewer is hung |
| `state.sh` | Read/write the global repo state indicator (test totals, known-failing set, clippy, hygiene, **test dispositions**) at `.claude/state/known-state.json` (schema v2) | First query on any new session — skips the rediscover-from-scratch loop (`show`, `check`, `known-failing`, `dispositions`, `refresh --sha-only`/`--full`/`--hygiene-only`/`--dispositions-only`) |
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

Checks: stale flags (defined but unused), orphan checks (used but undefined), undocumented flags (missing from CLAUDE.md).

### repo-hygiene.sh — Worktree Cleanliness

```bash
diagnostics/repo-hygiene.sh                        # List detected temp/scratch files
diagnostics/repo-hygiene.sh --check                # Exit 1 if temp files found (CI/skill gate)
diagnostics/repo-hygiene.sh --clean                # Remove detected temp files
diagnostics/repo-hygiene.sh --gitignore            # Suggest .gitignore patterns for detected files
```

Detects untracked temp files by category: **DUMP** (debug/IR dumps), **SCRATCH** (one-off test scripts), **BACKUP** (editor merge artifacts), **ARTIFACT** (stray build outputs), **STALE** (core dumps). Integrated into `/continue-roadmap` subsection close-out and section completion checklists.

### self-test.sh — Script Self-Test

```bash
diagnostics/self-test.sh                           # Run all fixture tests
diagnostics/self-test.sh --verbose                  # Detailed output
```

### tpr-failure-summary.sh — TPR Failure Patterns

```bash
diagnostics/tpr-failure-summary.sh                    # Full summary (both reviewers)
diagnostics/tpr-failure-summary.sh --reviewer gemini  # Gemini only
diagnostics/tpr-failure-summary.sh --failures         # Only failed runs
diagnostics/tpr-failure-summary.sh --verbose          # Per-run failure details
diagnostics/tpr-failure-summary.sh --reviewer gemini --verbose --failures  # All flags
```

Scans `/tmp/ori-tpr-*/` run directories for failure patterns. Reports success rates, API capacity errors, watchdog kills, envelope repair/rescue stats, and per-run failure details. Extracts the actual API error message from JSONL result events.

### tpr-liveness.sh — TPR Reviewer Liveness Probe

```bash
diagnostics/tpr-liveness.sh /tmp/tpr-round-ori_lang-abc123 codex --human
diagnostics/tpr-liveness.sh /tmp/tpr-round-ori_lang-abc123 gemini --json
diagnostics/tpr-liveness.sh "$scratch" codex --grace-seconds 300 --tail-lines 20
```

Classifies a `/tpr-review` sub-agent as `alive` (exit 0), `quiet` (exit 1), or `dead` (exit 2) by inspecting `$scratch/{reviewer}-stdout.txt` — the tee'd CLI output guaranteed by invariant I14 (dual-path transport).

**When to use:** consult this BEFORE retrying or aborting a silent reviewer. The orchestrator in `.claude/skills/tpr-review/SKILL.md §9` invokes this probe as the first step of failure handling — it prevents the "kill deep-investigating agent" bias where reviewer silence during a long `cargo build` or `grep` looks identical to a hang.

**How it decides:**

| Condition | Verdict |
|---|---|
| `<<<TPR-REPORT` sentinel in tail | `alive` (final report in progress) |
| Empty stdout, mtime < grace | `alive` (CLI cold-starting) |
| mtime < grace | `alive` |
| mtime ∈ [grace, 2·grace), tail shows `tool_call` / `thinking` | `alive` (deep work in flight) |
| mtime ∈ [grace, 2·grace), no signal | `quiet` |
| mtime ∈ [2·grace, 4·grace), tail shows `tool_call` | `quiet` (slow Bash invocation suspected) |
| mtime ≥ 2·grace, no strong signal | `dead` |

Default grace is 300s (5 min). The 45-min ceiling on the reviewer CLI (`block-banned-commands.sh`) bounds the absolute worst case externally — the probe never extends that ceiling.

### state.sh — Global State Indicator

```bash
diagnostics/state.sh show                                  # Human-readable summary
diagnostics/state.sh show --json                           # JSON for skill consumption
diagnostics/state.sh check                                 # Exit 0 fresh / 1 dirty / 2 obsolete / 3 missing
diagnostics/state.sh known-failing                         # List expected-failing files
diagnostics/state.sh known-failing --json                  # Same as JSON array
diagnostics/state.sh dispositions                          # List #[ignore]/#skip annotations + tracking_bug
diagnostics/state.sh dispositions --untracked-only         # Just the drift (no BUG-XX-NNN in reason)
diagnostics/state.sh dispositions --json                   # JSON array of entry objects
diagnostics/state.sh refresh --sha-only --by commit-push   # Cheap: update HEAD SHA only
diagnostics/state.sh refresh --hygiene-only                # Run repo-hygiene.sh + update notes
diagnostics/state.sh refresh --dispositions-only           # Sub-second scan for #[ignore]/#skip + BUG-XX-NNN
diagnostics/state.sh refresh --full --by section-close     # Slow: re-run test-all.sh + clippy-all.sh + dispositions
```

Caches the result of `./test-all.sh`, `./clippy-all.sh`, `diagnostics/repo-hygiene.sh --check`, and the disposition scan in `.claude/state/known-state.json` (schema v2) so new sessions skip the rediscover-from-scratch loop.

**When to use:**
- **First query on any fresh session** — skills that need to know "is the tree known-failing?" should consult `state.sh show --json` before running tests.
- **At every commit** (`/commit-push` post-commit hook) — refresh the cached HEAD SHA so `check` correctly reports OBSOLETE when the commit isn't yet reflected.
- **At section close** (`/continue-roadmap` close-out) — `refresh --full --by section-close` captures a fresh baseline.
- **Before any TPR or review** — reviewers should see current known-state instead of flagging expected failures as regressions.

**Freshness semantics:** `check` returns:
- `0 / fresh` — cache SHA matches HEAD, working tree clean → trust the cache
- `1 / stale` — SHA matches but working tree is dirty → consult but verify for current task
- `2 / obsolete` — SHA mismatch → run `refresh --sha-only` (cheap) or `refresh --full` (truthful)
- `3 / missing` — state file absent → run `refresh --full`

**Source of truth:** plan-documented "Known Failing Tests" sections remain the SSOT for intent. This cache is an index over that intent. `refresh --full` does NOT auto-populate `known_failing_files` from test output — that list is an editorial decision tied to plan remediation sections.

**Test disposition tracking (schema v2).** `refresh --dispositions-only` scans `compiler/`, `tests/`, `tools/`, and `library/` for `#[ignore]` and `#skip("...")` annotations and extracts a `BUG-[A-Z0-9]+-[0-9]+` token from each annotation's reason text. Entries with no matching token are flagged `tracking_bug: null` (drift). Reason-text contract: every annotation should follow `#skip("BUG-04-077: typeck regression on generic body")` — the BUG ID anywhere in the reason satisfies the regex. The scan stays repo-local — bug-existence verification against any external bug tracker is downstream enrichment, not a `state.sh` concern. `test-all.sh` invokes this mode automatically and prints `Dispositions: N total, M untracked` (with a DRIFT banner when M>0). Use `dispositions --untracked-only` to list the offenders.

**Design log:** `.claude/skills/improve-tooling/script-state-design.md` (schema v2 invariants, load-bearing rules, improvement log).

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
