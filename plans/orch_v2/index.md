# CLI Orchestrator V2 Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: CompilerHost Trait
**File:** `section-01-compiler-host.md` | **Status:** Not Started

```
CompilerHost, host, I/O abstraction, file system
CliHost, TestHost, LspHost, WasmHost
read_file, write_file, file_exists, resolve_path
canonicalize, glob_files, current_dir, stdlib_path
diagnostic_output, program_output, supports_color
in-memory file system, virtual file system
TypeScript CompilerHost, host interface
dependency injection, testability, portability
```

---

### Section 02: CompilerConfig
**File:** `section-02-compiler-config.md` | **Status:** Not Started

```
CompilerConfig, config, configuration, validation
resolve, upfront validation, constraint checking
RawOptions, raw options, CLI parsing
ConfigError, conflicting flags, invalid options
BuildConfig, TestConfig, FormatConfig
command-specific config, option merging
Zig Config.resolve, typed configuration
accumulate errors, validate all options
```

---

### Section 03: Session
**File:** `section-03-session.md` | **Status:** Not Started

```
Session, compiler session, compilation context
staged initialization, spine, central struct
CompilerDb, database, Salsa, incremental
host, config, diagnostics, context
load_source, SourceFile, file loading
session lifetime, per-invocation, persistent
Rust Session, staged init, GlobalCtxt
```

---

### Section 04: DiagnosticContext
**File:** `section-04-diagnostic-context.md` | **Status:** Not Started

```
DiagnosticContext, diagnostic accumulation, error queue
accumulate-then-flush, centralized diagnostics
deduplication, severity ordering, soft-error suppression
add_lex_errors, add_parse_errors, add_type_errors
register_source, source context, snippet rendering
flush, emit, DiagnosticQueue, emitter
thread-safe, Mutex, AtomicBool, has_errors
boilerplate elimination, DRY error handling
```

---

### Section 05: Pipeline + CompilerCallbacks
**File:** `section-05-pipeline.md` | **Status:** Not Started

```
Pipeline, compilation pipeline, phase orchestration
CompilerCallbacks, callbacks, hooks, phase boundary
Phase, Lex, Parse, TypeCheck, TestVerify, Evaluate, Codegen
PhaseControl, Continue, Stop, abort
after_lex, after_parse, after_type_check, after_complete
run_through, target phase, phase ordering
boilerplate, check/run/build duplication
capability validation, test verification, contract check
Rust CompilerCallbacks, Gleam phase ordering
```

---

### Section 06: execute_safely()
**File:** `section-06-execute-safely.md` | **Status:** Not Started

```
execute_safely, panic safety, catch_unwind
ICE, internal compiler error, crash handling
diagnostic flush, guaranteed flush, panic hook
exit code, 101, bug report, version info
AssertUnwindSafe, panic info, stack trace
Rust catch_unwind, finish_diagnostics
```

---

### Section 07: Telemetry Trait
**File:** `section-07-telemetry.md` | **Status:** Not Started

```
Telemetry, telemetry, progress reporting, progress bar
NullTelemetry, TerminalTelemetry, LspTelemetry
phase_started, phase_completed, compiling_file
test_progress, spinner, verbose output
silent mode, testing, non-interactive
LSP $/progress, progress notification
Gleam Telemetry, swappable progress
```

---

### Section 08: Command Table
**File:** `section-08-command-table.md` | **Status:** Not Started

```
command table, declarative commands, command definitions
CommandDef, ArgDef, FlagDef, command metadata
COMMANDS, static, command list, single source of truth
auto-generated help, print_help, usage strings
shell completion, bash completion, zsh completion
subcommand, handler, dispatch, routing
Elm declarative commands, commands as data
```

---

### Section 09: Outcome Enum
**File:** `section-09-outcome.md` | **Status:** Not Started

```
Outcome, outcome, result type, compilation result
Ok, PartialSuccess, TotalFailure
partial failure, warnings, non-fatal errors
is_ok, value, into_outcome
Gleam Outcome, three-state result
```

---

### Section 10: Command Migration
**File:** `section-10-command-migration.md` | **Status:** Not Started

```
migration, command migration, incremental migration
check command, run command, build command, test command
fmt command, parse command, lex command
main.rs, 318 lines, 20 lines, thin dispatch
backward compatibility, regression testing
proof of concept, migrate first, migrate all
```

---

### Section 11: TestHost + LspHost
**File:** `section-11-host-implementations.md` | **Status:** Not Started

```
TestHost, in-memory, mock file system
LspHost, LSP, language server, editor buffers
WasmHost, browser, playground
with_file, output capture, test isolation
virtual file system, VFS, buffer-backed
```

---

### Section 12: Watch Mode
**File:** `section-12-watch-mode.md` | **Status:** Not Started

```
watch mode, file watching, persistent session
incremental, Salsa caching, live reload
debouncing, coalescing, file change detection
daemon, persistent database, session reuse
TypeScript createWatchProgram, watch host
ori watch, ori check --watch
```

---

## Quick Reference

| ID | Title | File | Tier |
|----|-------|------|------|
| 01 | CompilerHost Trait | `section-01-compiler-host.md` | 0 |
| 02 | CompilerConfig | `section-02-compiler-config.md` | 0 |
| 03 | Session | `section-03-session.md` | 0 |
| 04 | DiagnosticContext | `section-04-diagnostic-context.md` | 1 |
| 05 | Pipeline + CompilerCallbacks | `section-05-pipeline.md` | 1 |
| 06 | execute_safely() | `section-06-execute-safely.md` | 1 |
| 07 | Telemetry Trait | `section-07-telemetry.md` | 2 |
| 08 | Command Table | `section-08-command-table.md` | 2 |
| 09 | Outcome Enum | `section-09-outcome.md` | 2 |
| 10 | Command Migration | `section-10-command-migration.md` | 3 |
| 11 | TestHost + LspHost | `section-11-host-implementations.md` | 3 |
| 12 | Watch Mode | `section-12-watch-mode.md` | 3 |
