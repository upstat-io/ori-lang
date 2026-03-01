---
title: "Appendix D: Debugging"
description: "Ori Compiler Design — Appendix D: Debugging"
order: 1004
section: "Appendices"
---

# Appendix D: Debugging

Structured tracing and debugging techniques for the Ori compiler.

## Tracing Infrastructure

The Ori compiler uses the `tracing` crate for structured, hierarchical logging.
All output goes to stderr, so it never interferes with program output.

Setup: `compiler/oric/src/tracing_setup.rs`, initialized in `main()`.

## Environment Variables

### Tracing

| Variable | Purpose | Example |
|----------|---------|---------|
| `ORI_LOG` | Filter string (`RUST_LOG` syntax) | `ORI_LOG=debug` |
| `ORI_LOG_TREE` | Enable hierarchical tree output | `ORI_LOG_TREE=1` |
| `RUST_LOG` | Fallback if `ORI_LOG` not set | `RUST_LOG=debug` |

When neither `ORI_LOG` nor `RUST_LOG` is set, only warnings and above are shown.
This ensures zero noise in normal usage.

### Phase Dumps

Phase dump variables emit intermediate representations to stderr. Zero cost in release builds.

| Variable | Purpose | Example |
|----------|---------|---------|
| `ORI_DUMP_AFTER_PARSE` | Dump AST after parsing | `ORI_DUMP_AFTER_PARSE=1 ori check file.ori` |
| `ORI_DUMP_AFTER_TYPECK` | Dump typed IR after type checking | `ORI_DUMP_AFTER_TYPECK=1 ori check file.ori` |
| `ORI_DUMP_AFTER_ARC` | Dump ARC IR with RC strategy annotations | `ORI_DUMP_AFTER_ARC=1 ori build file.ori` |
| `ORI_DUMP_AFTER_LLVM` | Dump annotated LLVM IR (superset of `ORI_DEBUG_LLVM`) | `ORI_DUMP_AFTER_LLVM=1 ori build file.ori` |
| `ORI_EMIT_ARC_DOT` | Emit GraphViz DOT of ARC IR CFG | `ORI_EMIT_ARC_DOT=1 ori build file.ori 2> arc.dot` |
| `ORI_DEBUG_LLVM` | Legacy alias for `ORI_DUMP_AFTER_LLVM` | `ORI_DEBUG_LLVM=1` |

### Codegen Audit

In-pipeline LLVM IR verification, gated behind environment variables. Walks the in-memory LLVM IR (via inkwell) to detect RC lifecycle bugs, COW sequencing violations, and ABI mismatches.

| Variable | Purpose | Example |
|----------|---------|---------|
| `ORI_AUDIT_CODEGEN` | Enable codegen audit pass | `ORI_AUDIT_CODEGEN=1 ori build file.ori` |
| `ORI_AUDIT_STRICT` | Pessimistic mode (elevates warnings, treats COW as always-freeing) | `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_STRICT=1 ori build file.ori` |
| `ORI_AUDIT_FUNCTION` | Filter audit to functions matching a substring | `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_FUNCTION=main ori build file.ori` |

### Runtime Debug (AOT binaries)

These variables are read by the compiled AOT binary at runtime, not by the compiler.

| Variable | Purpose | Example |
|----------|---------|---------|
| `ORI_TRACE_RC` | RC event trace (alloc/inc/dec/free with attribution) | `ORI_TRACE_RC=1 ./binary` |
| `ORI_RT_DEBUG` | Runtime assertions (header validation, bounds) | `ORI_RT_DEBUG=1 ./binary` |
| `ORI_CHECK_LEAKS` | Leak check with attribution at exit | `ORI_CHECK_LEAKS=1 ./binary` |

## Filter Syntax

Filters use `EnvFilter` syntax (same as `RUST_LOG`):

```bash
# All crates at debug level
ORI_LOG=debug ori check file.ori

# Specific crate at trace level
ORI_LOG=ori_types=trace ori check file.ori

# Multiple crates at different levels
ORI_LOG=ori_types=debug,oric::query=trace ori check file.ori

# Everything at trace with hierarchical tree output
ORI_LOG=trace ORI_LOG_TREE=1 ori check file.ori
```

## Tracing Levels

| Level | Use Case | Example |
|-------|----------|---------|
| `error` | Should never happen; internal invariant violations | — |
| `warn` | Recoverable issues worth investigating | — |
| `debug` | Phase boundaries, query execution, Salsa events | Type check passes, signature collection |
| `trace` | Per-expression inference, hot-path evaluation | `infer_expr`, `eval`, method dispatch |

## Common Debug Scenarios

### "Why is this type wrong?"

```bash
ORI_LOG=ori_types=debug ori check file.ori
```

Shows type checker passes, signature collection, and body checking.
For per-expression detail:

```bash
ORI_LOG=ori_types=trace ORI_LOG_TREE=1 ori check file.ori
```

### "Why is Salsa recomputing?"

```bash
ORI_LOG=oric::db=debug ori run file.ori
```

Shows Salsa `WillExecute` events (cache misses). At trace level, also shows cache hits.

### "What's the query pipeline doing?"

```bash
ORI_LOG=oric::query=debug ori run file.ori
```

Shows when each Salsa query (tokens, parsed, typed, evaluated) executes.

### "What's happening during evaluation?"

```bash
ORI_LOG=ori_eval=debug ori run file.ori
```

Shows function calls and method dispatch at debug level.
Use `trace` for per-expression evaluation.

## Hierarchical Tree Output

Set `ORI_LOG_TREE=1` to get indented, hierarchical output that shows the
call tree of instrumented spans:

```bash
ORI_LOG=ori_types=debug ORI_LOG_TREE=1 ori check file.ori
```

## Instrumentation Guide

When adding tracing to new compiler code:

- **Public API entry points**: `#[tracing::instrument(level = "debug", skip_all)]`
- **Per-expression functions**: `#[tracing::instrument(level = "trace", skip(engine, arena))]`
- **Salsa tracked functions**: Manual `tracing::debug!()` events (not `#[instrument]`)
- **Error accumulation**: `tracing::debug!(kind = ?error.kind, "type error recorded")`
- **Phase completion**: `tracing::debug!("phase X complete")`

Always `skip` large or non-Debug arguments (arenas, engines, pools).

## Diagnostic Scripts

The `diagnostics/` directory provides standalone scripts for common debugging workflows. All support `--help`, `--no-color`/`--color`.

| Script | Purpose | Example |
|--------|---------|---------|
| `diagnose-aot.sh` | All-in-one: build + run + leak check + RC stats + IR | `diagnostics/diagnose-aot.sh file.ori --valgrind` |
| `dual-exec-debug.sh` | Interpreter vs AOT comparison (auto-dumps on mismatch) | `diagnostics/dual-exec-debug.sh file.ori --verbose` |
| `dual-exec-verify.sh` | Batch interpreter vs LLVM verification | `diagnostics/dual-exec-verify.sh --test-only` |
| `rc-stats.sh` | RC balance per function (flags imbalances) | `diagnostics/rc-stats.sh file.ori` |
| `codegen-audit.sh` | Static RC + COW + ABI analysis | `diagnostics/codegen-audit.sh file.ori --strict` |
| `ir-dump.sh` | Annotated LLVM IR (`--raw` for undecorated) | `diagnostics/ir-dump.sh file.ori` |
| `ir-diff.sh` | Side-by-side IR comparison of two programs | `diagnostics/ir-diff.sh a.ori b.ori` |
| `disasm-ori.sh` | Native disassembly with Ori demangling | `diagnostics/disasm-ori.sh file.ori` |
| `valgrind-aot.sh` | Valgrind memory error detection | `diagnostics/valgrind-aot.sh file.ori` |
| `check-debug-flags.sh` | Validates all `ORI_*` flag consistency | `diagnostics/check-debug-flags.sh` |

### ARC DOT Visualization

The `ORI_EMIT_ARC_DOT=1` flag emits GraphViz DOT format of the ARC IR control flow graph. The `oric/src/arc_dot/` module generates DOT output with:

- Basic blocks as graph nodes with instruction listings
- Control flow edges (branches, jumps, switches) as directed edges
- RC operations highlighted for visual inspection

```bash
ORI_EMIT_ARC_DOT=1 ori build file.ori 2> arc.dot
dot -Tsvg arc.dot -o arc.svg
```

### Performance Baseline

```bash
./scripts/perf-baseline.sh [--release]
```

Records benchmark results for regression tracking.

## Panic Debugging

Enable backtraces:

```bash
RUST_BACKTRACE=1 ori run file.ori
RUST_BACKTRACE=full ori run file.ori
```

## Performance Profiling

Using perf:

```bash
perf record target/release/ori run large_file.ori
perf report
```

## IDE Integration

For VS Code debugging, launch.json:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Debug Compiler",
      "type": "lldb",
      "request": "launch",
      "program": "${workspaceFolder}/target/debug/ori",
      "args": ["run", "${file}"],
      "env": {
        "ORI_LOG": "debug",
        "ORI_LOG_TREE": "1",
        "RUST_BACKTRACE": "1"
      }
    }
  ]
}
```

## Test Debugging

Debug specific test:

```bash
ORI_LOG=debug cargo test test_type_inference -- --nocapture
```
