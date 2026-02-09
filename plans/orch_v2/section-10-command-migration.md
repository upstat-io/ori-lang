---
section: "10"
title: Command Migration
status: not-started
tier: 3
goal: Migrate all commands from manual dispatch to Pipeline-based architecture
sections:
  - id: "10.1"
    title: Migrate check Command
    status: not-started
  - id: "10.2"
    title: Migrate run Command
    status: not-started
  - id: "10.3"
    title: Migrate build Command
    status: not-started
  - id: "10.4"
    title: Migrate test Command
    status: not-started
  - id: "10.5"
    title: Migrate Remaining Commands
    status: not-started
  - id: "10.6"
    title: Rewrite main.rs
    status: not-started
  - id: "10.7"
    title: Section Completion Checklist
    status: not-started
---

# Section 10: Command Migration

**Status:** 📋 Planned
**Goal:** Incrementally migrate all command handlers to use the new Session + Pipeline architecture, culminating in a ~20-line `main.rs`.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Migration Strategy
> **Depends on**: Sections 1-6 (all foundation + pipeline modules), Section 8 (Command Table)

---

## 10.1 Migrate check Command

First migration — proof of concept for the new architecture.

- [ ] **Implement**: `commands::check_v2(session: &mut Session) -> Outcome<()>`
  - [ ] `session.load_source(config.input_path())`
  - [ ] `Pipeline::new(session).run_through(Phase::TestVerify, file)`
  - [ ] Return outcome (diagnostics handled by pipeline)

- [ ] **Verify**: Identical behavior to current `check_file()`
  - [ ] Same error messages
  - [ ] Same exit codes
  - [ ] Same coverage checking
  - [ ] Run all spec tests that exercise `ori check`

- [ ] **Remove**: Old `check_file()` after verification
  - [ ] Or keep as `check_file_legacy()` temporarily with deprecation

- [ ] **Rust Tests**:
  - [ ] `test_check_v2_valid_file` — succeeds for well-typed file
  - [ ] `test_check_v2_type_error` — reports type errors
  - [ ] `test_check_v2_untested_function` — reports coverage errors
  - [ ] `test_check_v2_with_test_host` — works with in-memory files

---

## 10.2 Migrate run Command

- [ ] **Implement**: `commands::run_v2(session: &mut Session) -> Outcome<()>`
  - [ ] `Pipeline::new(session).run_through(Phase::Evaluate, file)`
  - [ ] Print eval result through `session.host().program_output()`
  - [ ] Handle `--compile` flag → delegate to AOT run

- [ ] **Verify**: Identical behavior to current `run_file()`
  - [ ] Same eval output
  - [ ] Same error messages
  - [ ] Same exit codes

- [ ] **Rust Tests**:
  - [ ] `test_run_v2_hello_world` — basic eval
  - [ ] `test_run_v2_type_error` — stops at type check
  - [ ] `test_run_v2_runtime_error` — reports eval errors

---

## 10.3 Migrate build Command

- [ ] **Implement**: `commands::build_v2(session: &mut Session) -> Outcome<()>`
  - [ ] `Pipeline::new(session).run_through(Phase::Codegen, file)`
  - [ ] Build config from `session.config().build().unwrap()`
  - [ ] Delegate to existing LLVM pipeline for codegen phase

- [ ] **Verify**: Identical behavior to current `build_file()`
  - [ ] Same optimization levels
  - [ ] Same output artifacts
  - [ ] Same target handling

- [ ] **Rust Tests**:
  - [ ] `test_build_v2_basic` — compiles to native executable
  - [ ] `test_build_v2_options` — respects build config

---

## 10.4 Migrate test Command

- [ ] **Implement**: `commands::test_v2(session: &mut Session) -> Outcome<()>`
  - [ ] Create `TestRunnerConfig` from `session.config().test()`
  - [ ] Use existing `TestRunner` with session's interner
  - [ ] Return outcome based on test results

- [ ] **Verify**: Identical behavior to current `run_tests()`
  - [ ] Same test discovery
  - [ ] Same parallel execution
  - [ ] Same output format

---

## 10.5 Migrate Remaining Commands

- [ ] **Implement**: `commands::fmt_v2(session: &mut Session) -> Outcome<()>`
  - [ ] Delegate to existing `run_format()` with session host

- [ ] **Implement**: `commands::parse_v2(session: &mut Session) -> Outcome<()>`
  - [ ] Pipeline through Phase::Parse, then display AST

- [ ] **Implement**: `commands::lex_v2(session: &mut Session) -> Outcome<()>`
  - [ ] Pipeline through Phase::Lex, then display tokens

- [ ] **Implement**: Simple commands (no pipeline needed)
  - [ ] `explain_v2` — error code documentation
  - [ ] `demangle_v2` — symbol demangling
  - [ ] `target_v2` — target management
  - [ ] `targets_v2` — list targets
  - [ ] `version_v2` — print version
  - [ ] `help_v2` — print help from command table

---

## 10.6 Rewrite main.rs

Replace the 318-line main.rs with ~20-line version.

- [ ] **Implement**: New `main.rs`
  ```
  fn main() -> ExitCode {
      oric::tracing_setup::init();
      let raw = RawOptions::from_env();
      let config = match CompilerConfig::resolve(raw) { ... };
      let host = Box::new(CliHost::new());
      let mut session = Session::new(host, config);
      execute_safely(&mut session, |session| {
          session.config().command().handler()(session)
      })
  }
  ```

- [ ] **Verify**: All commands work identically
  - [ ] Run `./test-all.sh`
  - [ ] Run every command manually
  - [ ] Verify error messages unchanged
  - [ ] Verify exit codes unchanged

- [ ] **Clean up**: Remove old code
  - [ ] Remove `print_usage()` (replaced by command table)
  - [ ] Remove `commands::read_file()` (replaced by host)
  - [ ] Remove per-command argument parsing from old handlers

---

## 10.7 Section Completion Checklist

- [ ] All 12+ commands migrated to new architecture
- [ ] `main.rs` is ~20 lines
- [ ] All commands work identically to before
- [ ] No regressions: `./test-all.sh` passes
- [ ] Old command handlers removed (no dead code)
- [ ] CLI integration tests all pass

**Exit Criteria:** Every `ori <command>` invocation goes through `CompilerConfig::resolve()` → `Session::new()` → `execute_safely()` → command handler, with identical user-visible behavior.
