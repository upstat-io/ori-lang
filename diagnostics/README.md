# Diagnostic Scripts

Quick-access debugging tools for the Ori compiler's AOT/codegen pipeline. These scripts extract more signal in seconds than manual investigation in minutes.

**Prerequisite**: An LLVM-enabled `ori` binary. Build with `cargo b` (debug) or `cargo b --release` (release).

## Quick Reference

| Script | Purpose | When to use |
|--------|---------|-------------|
| `diagnose-aot.sh` | All-in-one: compile + run + leak check + RC stats + IR | First tool to reach for on any AOT bug |
| `dual-exec-debug.sh` | Compare interpreter vs AOT output | Wrong output — is it eval or codegen? |
| `codegen-audit.sh` | Static RC/COW/ABI analysis of LLVM IR | RC corruption, double-free, ABI mismatch |
| `rc-stats.sh` | RC operation count per function | Leak or over-release suspicion |
| `ir-dump.sh` | Annotated LLVM IR with color-coded RC ops | Understanding what codegen actually emits |
| `ir-diff.sh` | Side-by-side IR comparison of two programs | Regression hunting, before/after comparison |
| `disasm-ori.sh` | Native disassembly with Ori symbol demangling | Instruction-level debugging |
| `check-debug-flags.sh` | Validate `ORI_*` flag consistency | After adding/removing debug flags |
| `self-test.sh` | Self-test all scripts against fixtures | After modifying any diagnostic script |

## Usage

### diagnose-aot.sh — All-in-One Diagnostic

```bash
diagnostics/diagnose-aot.sh file.ori              # Standard battery
diagnostics/diagnose-aot.sh --valgrind file.ori    # + Valgrind memory error detection
diagnostics/diagnose-aot.sh --rc-trace file.ori    # + ORI_TRACE_RC during execution
diagnostics/diagnose-aot.sh --verbose file.ori     # + native disassembly
```

Runs 5-7 checks in sequence: compilation, execution, leak check (`ORI_CHECK_LEAKS=1`), RC stats, LLVM IR dump, and optionally Valgrind and disassembly.

### dual-exec-debug.sh — Backend Comparison

```bash
diagnostics/dual-exec-debug.sh file.ori            # Compare eval vs AOT
diagnostics/dual-exec-debug.sh --verbose file.ori   # + ORI_LOG=debug traces on both
```

On mismatch, automatically runs `ir-dump.sh` and `rc-stats.sh` to diagnose the difference.

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
diagnostics/rc-stats.sh file.ori                   # Count RC ops per function
diagnostics/rc-stats.sh --optimized file.ori        # After LLVM optimization passes
```

Balance = `(alloc + inc) - (dec + free)`. Positive = potential leak. Negative = potential over-release.

### ir-dump.sh — LLVM IR Dump

```bash
diagnostics/ir-dump.sh file.ori                    # Annotated, color-coded IR
diagnostics/ir-dump.sh --raw file.ori              # Raw IR without annotations
diagnostics/ir-dump.sh --optimized file.ori         # After LLVM optimization passes
diagnostics/ir-dump.sh --function main file.ori     # Single function only
```

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

### check-debug-flags.sh — Flag Consistency

```bash
diagnostics/check-debug-flags.sh                   # Validate all ORI_* flags
```

Checks: stale flags (defined but unused), orphan checks (used but undefined), undocumented flags (missing from CLAUDE.md).

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
ORI_CHECK_LEAKS=1 ./binary                          # Quick check
diagnostics/rc-stats.sh file.ori                     # Which function is imbalanced?
ORI_TRACE_RC=1 ./binary 2>&1 | grep -v inc | head    # What's allocated but never freed?
```

### "Codegen looks wrong"

```bash
diagnostics/ir-dump.sh file.ori                      # See what we emit
diagnostics/ir-dump.sh --optimized file.ori           # See what LLVM makes of it
diagnostics/codegen-audit.sh --strict file.ori        # Static correctness check
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

Test fixtures in `fixtures/` exercise different codegen patterns:

| Fixture | What it tests |
|---------|--------------|
| `simple.ori` | Minimal program — no collections, no RC |
| `clean.ori` | Collections + RC, all balanced |
| `chain.ori` | Chained COW operations |

## Common Options

All scripts support:
- `--help` / `-h` — usage information
- `--no-color` — disable color output (for piping/logging)
- `--color` — force color output (overrides auto-detection)
