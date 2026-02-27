# Diagnostic Suite

The Ori compiler includes a diagnostic suite for debugging the AOT/codegen pipeline. This document describes the architecture, tools, and workflows.

## Overview

The diagnostic suite has three layers:

1. **Environment variables** — compiler and runtime instrumentation flags (`ORI_LOG`, `ORI_DUMP_AFTER_*`, `ORI_TRACE_RC`, etc.)
2. **Bash scripts** (`diagnostics/`) — composable tools that combine environment variables with post-processing
3. **In-pipeline audit** (`ORI_AUDIT_CODEGEN`) — Rust-level LLVM IR verification during compilation

Each layer serves a different need: env vars for quick one-off checks, scripts for structured investigations, and the in-pipeline audit for CI-grade correctness verification.

## Architecture

### Environment Variable Layers

The diagnostic flags operate at two distinct points in the pipeline:

**Compile-time flags** (checked inside the compiler binary):

| Flag | Phase | What it does |
|------|-------|-------------|
| `ORI_LOG` | All | Structured tracing via `tracing` crate. Filter syntax matches `RUST_LOG` (e.g., `ori_types=debug,ori_llvm=trace`). Falls back to `RUST_LOG` if `ORI_LOG` is unset. |
| `ORI_LOG_TREE=1` | All | Switches tracing output to hierarchical tree format via `tracing-tree`. Shows nesting of function calls and spans. |
| `ORI_DUMP_AFTER_PARSE=1` | Parser | Dumps the AST to stderr after parsing completes. |
| `ORI_DUMP_AFTER_TYPECK=1` | Type checker | Dumps the typed IR to stderr after type checking. |
| `ORI_DUMP_AFTER_ARC=1` | ARC pipeline | Dumps ARC IR with RC strategy annotations to stderr. |
| `ORI_DUMP_AFTER_LLVM=1` | LLVM codegen | Dumps annotated LLVM IR to stderr. Superset of legacy `ORI_DEBUG_LLVM`. |
| `ORI_AUDIT_CODEGEN=1` | LLVM codegen | Enables in-pipeline RC/COW/ABI verification (see below). |
| `ORI_AUDIT_STRICT=1` | LLVM codegen | Pessimistic audit mode: treats COW as always-freeing, function pointer params as RC-managed. |
| `ORI_AUDIT_FUNCTION=name` | LLVM codegen | Filters audit to functions whose name contains the substring. |

**Runtime flags** (checked inside the AOT binary via `ori_rt`):

| Flag | What it does |
|------|-------------|
| `ORI_TRACE_RC=1` | Logs every RC operation (alloc, inc, dec, free) with object address and call site. |
| `ORI_RT_DEBUG=1` | Enables runtime assertions: header validation, bounds checking, underflow detection. |
| `ORI_CHECK_LEAKS=1` | On process exit, reports any objects whose reference count is still > 0. |

**Design principle**: All flags are zero-cost when disabled. Compile-time flags use `std::env::var` checked once at startup. Runtime flags use conditional compilation (`#[cfg(debug_assertions)]`) for the check overhead, with the actual instrumentation behind the flag.

**Consistency**: `diagnostics/check-debug-flags.sh` validates that every `ORI_*` flag defined in `compiler/oric/src/debug_flags.rs` is (a) used somewhere in the codebase, (b) not orphaned as a raw `std::env::var` check, and (c) documented in `CLAUDE.md`.

### Diagnostic Scripts

The scripts in `diagnostics/` are composable tools built on top of the environment variables. They follow a shared architecture:

```
_common.sh          Shared helpers (find_ori_bin, ORI variable)
    ↓ sourced by
ir-dump.sh          Foundation: compile + dump IR
    ↓ used by
rc-stats.sh         Parse IR → count RC ops per function
ir-diff.sh          Diff two IR dumps (with normalization)
    ↓ used by
codegen-audit.sh    In-pipeline audit via ORI_AUDIT_CODEGEN
diagnose-aot.sh     All-in-one: compile + run + leak + stats + IR
dual-exec-debug.sh  Compare eval vs AOT (auto-dumps on mismatch)
    ↓ standalone
disasm-ori.sh       Compile + disassemble with demangling
check-debug-flags.sh  Flag consistency validation
self-test.sh        Self-test against fixtures
```

**`_common.sh`** provides `find_ori_bin` which searches for an LLVM-enabled binary in order: `$ORI_BIN` (env override), `target/debug/ori`, `target/release/ori`, `ori` (PATH). This means scripts work whether you're in the repo root, CI, or have `ori` installed globally.

**Dependency graph**: `diagnose-aot.sh` and `dual-exec-debug.sh` call `ir-dump.sh` and `rc-stats.sh` internally. `ir-diff.sh` calls `ir-dump.sh` twice. This means a bug in `ir-dump.sh` affects all downstream scripts — `self-test.sh` catches this.

### In-Pipeline Audit

The codegen audit (`ORI_AUDIT_CODEGEN=1`) is distinct from the bash scripts. It runs **inside the Rust compiler process** during LLVM IR generation, walking live `inkwell` IR objects rather than parsing textual IR. This gives it access to type information, calling conventions, and attribute sets that aren't visible in the text dump.

Three analysis categories:

1. **RC Balance** — Tracks `ori_rc_alloc`/`ori_rc_inc`/`ori_rc_dec`/`ori_rc_free` lifecycle per function. Detects leaks (alloc without matching dec) and double-frees (dec after COW consumption).

2. **COW Correctness** — Verifies COW operation sequencing: no pointer reuse after COW call (invalidated by realloc), no `ori_rc_dec` before COW call (freed pointer passed in).

3. **ABI Conformance** — Checks calling conventions: no large aggregate loads (>16B, which trigger the FastISel JIT bug), runtime function argument counts match declarations, no `invoke` on `nounwind` functions.

**Strict mode** (`ORI_AUDIT_STRICT=1`) makes pessimistic assumptions: COW calls are treated as always-freeing (even if ref count > 1), and function pointer parameters are tracked as RC-managed. This catches edge cases at the cost of false positives.

## Integration Points

### Bug Debugging Workflow

The diagnostic suite is integrated into the project's TDD-for-bugs protocol (see `CLAUDE.md`):

```
1. STOP — Don't jump to fixing
2. Understand — Consult the spec
3. Run diagnostics — Choose tool based on symptom (see below)
4. Reproduce with tests
5. Verify tests fail
6. Fix the code
7. Tests pass unchanged
```

Step 3 tool selection:

| Symptom | Tool |
|---------|------|
| Wrong output | `diagnostics/dual-exec-debug.sh` |
| Crash / segfault | `diagnostics/diagnose-aot.sh --valgrind` |
| Memory leak | `ORI_CHECK_LEAKS=1 ./binary` → `diagnostics/rc-stats.sh` |
| RC corruption | `ORI_TRACE_RC=1 ./binary` → `diagnostics/codegen-audit.sh --strict` |
| Type error | `ORI_LOG=ori_types=debug ori check file.ori` |
| Wrong IR | `diagnostics/ir-dump.sh` → `diagnostics/ir-diff.sh` |

### CI

`diagnostics/check-debug-flags.sh` runs as part of flag consistency validation. The `self-test.sh` script validates all diagnostic scripts against fixture programs in `diagnostics/fixtures/`.

### Code Journey Skill

The `/code-journey` skill uses the diagnostic environment variables (`ORI_DUMP_AFTER_LLVM`, `ORI_LOG`) to capture trace data for its analysis passes. Background agents read the trace files and perform the LLVM Deep Scrutiny analysis.

## Fixtures

Test fixtures in `diagnostics/fixtures/` exercise different codegen patterns:

| Fixture | Pattern | RC behavior |
|---------|---------|------------|
| `simple.ori` | Minimal program (arithmetic, no collections) | No RC operations |
| `clean.ori` | Collections + struct allocation | Balanced RC lifecycle |
| `chain.ori` | Chained COW operations | COW sequencing correctness |

## Adding a New Diagnostic Script

1. Create `diagnostics/new-script.sh` with standard header:
   ```bash
   #!/bin/bash
   # Brief description.
   #
   # Usage:
   #   diagnostics/new-script.sh [options] <file.ori>
   SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
   source "$SCRIPT_DIR/_common.sh"
   find_ori_bin
   ```

2. Support `--help`, `--no-color`, `--color` (standard options).

3. Add fixture tests in `self-test.sh`.

4. Document in:
   - `diagnostics/README.md` (quick reference table + usage section)
   - `CLAUDE.md` (Commands section, diagnostic scripts block)
   - `.claude/rules/compiler.md` (Diagnostic Scripts section)
   - Relevant domain rules files (e.g., `arc.md` for RC tools, `llvm.md` for IR tools)

5. Run `diagnostics/self-test.sh` to verify.

## Adding a New Debug Flag

1. Define the flag in `compiler/oric/src/debug_flags.rs`.
2. Use it in the relevant compiler phase.
3. Document in `CLAUDE.md` (under the appropriate category: compile-time, runtime, or audit).
4. Run `diagnostics/check-debug-flags.sh` to validate consistency.
