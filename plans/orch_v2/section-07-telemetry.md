---
section: "07"
title: Telemetry Trait
status: not-started
tier: 2
goal: Swappable progress reporting for CLI, LSP, and testing
sections:
  - id: "7.1"
    title: Trait Definition
    status: not-started
  - id: "7.2"
    title: Implementations
    status: not-started
  - id: "7.3"
    title: Pipeline Integration
    status: not-started
  - id: "7.4"
    title: Section Completion Checklist
    status: not-started
---

# Section 07: Telemetry Trait

**Status:** 📋 Planned
**Goal:** Define a swappable progress reporting interface that enables terminal spinners, LSP `$/progress` notifications, and silent testing.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Module 5
> **Inspired by**: Gleam's `Telemetry` trait with `NullTelemetry`
> **Location**: `compiler/oric/src/telemetry.rs`
> **Depends on**: Section 5 (Pipeline)

---

## 7.1 Trait Definition

- [ ] **Implement**: `Telemetry` trait in `compiler/oric/src/telemetry.rs`
  - [ ] `fn phase_started(&self, phase: Phase)` — compilation phase began
  - [ ] `fn phase_completed(&self, phase: Phase)` — compilation phase done
  - [ ] `fn compiling_file(&self, path: &Path)` — processing a file
  - [ ] `fn test_progress(&self, completed: usize, total: usize)` — test progress
  - [ ] `fn info(&self, message: &str)` — informational message
  - [ ] Trait must be `Send` for thread safety

- [ ] **Rust Tests**: `compiler/oric/src/telemetry.rs`
  - [ ] `test_telemetry_is_object_safe` — `Box<dyn Telemetry>` compiles

---

## 7.2 Implementations

- [ ] **Implement**: `NullTelemetry` — silent implementation for testing
  - [ ] All methods are no-ops
  - [ ] Zero allocation, zero side effects

- [ ] **Implement**: `TerminalTelemetry` — terminal output for CLI
  - [ ] `verbose: bool` flag
  - [ ] `phase_started` → `eprintln!` if verbose
  - [ ] `phase_completed` → `eprintln!` if verbose
  - [ ] `compiling_file` → always print (e.g., "  Compiling main.ori")
  - [ ] `test_progress` → `eprint!("\r  Running tests: {completed}/{total}")`
  - [ ] `info` → `eprintln!`

- [ ] **Implement**: `CapturingTelemetry` — captures events for testing
  - [ ] Stores events in `Arc<Mutex<Vec<TelemetryEvent>>>`
  - [ ] `TelemetryEvent` enum: `PhaseStarted(Phase)`, `PhaseCompleted(Phase)`, `CompilingFile(PathBuf)`, etc.
  - [ ] `events() -> Vec<TelemetryEvent>` to retrieve captured events

- [ ] **Rust Tests**: `compiler/oric/src/telemetry.rs`
  - [ ] `test_null_telemetry` — no panics, no output
  - [ ] `test_capturing_telemetry` — events captured in order
  - [ ] `test_terminal_telemetry_verbose` — prints phase info when verbose

---

## 7.3 Pipeline Integration

- [ ] **Implement**: Add telemetry calls to `Pipeline::run_through()`
  - [ ] `telemetry.phase_started(Phase::Lex)` before lexing
  - [ ] `telemetry.phase_completed(Phase::Lex)` after lexing
  - [ ] Same pattern for each phase

- [ ] **Implement**: `Pipeline::with_telemetry(self, telemetry)` builder method
  - [ ] Default: `NullTelemetry` (zero overhead in tests)

- [ ] **Rust Tests**: `compiler/oric/src/telemetry.rs`
  - [ ] `test_pipeline_telemetry_events` — verify phase events emitted in order
  - [ ] `test_pipeline_telemetry_stop` — stopped phases don't emit completed

---

## 7.4 Section Completion Checklist

- [ ] `Telemetry` trait is object-safe and Send
- [ ] `NullTelemetry` has zero overhead
- [ ] `TerminalTelemetry` outputs progress info
- [ ] `CapturingTelemetry` enables testing of telemetry events
- [ ] Pipeline emits telemetry events at each phase boundary
- [ ] No regressions: `./test-all.sh` passes

**Exit Criteria:** The pipeline reports progress through a swappable `Telemetry` trait; `NullTelemetry` is silent for tests; `TerminalTelemetry` shows progress for verbose CLI.
