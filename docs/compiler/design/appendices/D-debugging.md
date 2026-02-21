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

| Variable | Purpose | Example |
|----------|---------|---------|
| `ORI_LOG` | Filter string (`RUST_LOG` syntax) | `ORI_LOG=debug` |
| `ORI_LOG_TREE` | Enable hierarchical tree output | `ORI_LOG_TREE=1` |
| `RUST_LOG` | Fallback if `ORI_LOG` not set | `RUST_LOG=debug` |
| `ORI_DEBUG_LLVM` | Print LLVM IR to stderr before JIT compilation | `ORI_DEBUG_LLVM=1` |

When neither `ORI_LOG` nor `RUST_LOG` is set, only warnings and above are shown.
This ensures zero noise in normal usage.

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
