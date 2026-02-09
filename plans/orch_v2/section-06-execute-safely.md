---
section: "06"
title: execute_safely()
status: not-started
tier: 1
goal: Panic-safe execution wrapper with guaranteed diagnostic flush
sections:
  - id: "6.1"
    title: Core Wrapper
    status: not-started
  - id: "6.2"
    title: ICE Reporting
    status: not-started
  - id: "6.3"
    title: Section Completion Checklist
    status: not-started
---

# Section 06: execute_safely()

**Status:** 📋 Planned
**Goal:** Wrap compilation in `catch_unwind` so that panics (ICE) produce a helpful error message and flush all accumulated diagnostics instead of silently crashing.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Module 7
> **Inspired by**: Rust's `catch_unwind` + guaranteed `finish_diagnostics()`
> **Location**: `compiler/oric/src/execute.rs`
> **Depends on**: Section 3 (Session)

---

## 6.1 Core Wrapper

- [ ] **Implement**: `execute_safely()` in `compiler/oric/src/execute.rs`
  - [ ] Signature: `fn execute_safely(session: &mut Session, f: impl FnOnce(&mut Session) -> Outcome<()> + UnwindSafe) -> ExitCode`
  - [ ] Install custom panic hook to capture panic info
  - [ ] Wrap `f(session)` in `std::panic::catch_unwind`
  - [ ] On `Ok(outcome)`: flush diagnostics, return success/failure based on outcome + has_errors
  - [ ] On `Err(_)` (panic): flush diagnostics, print ICE message, return exit code 101
  - [ ] Restore original panic hook after execution

- [ ] **Implement**: Exit code conventions
  - [ ] `0` — success (no errors)
  - [ ] `1` — compilation error (user's code has problems)
  - [ ] `101` — internal compiler error (ICE, our bug)

- [ ] **Rust Tests**: `compiler/oric/src/execute.rs`
  - [ ] `test_execute_safely_success` — normal execution returns 0
  - [ ] `test_execute_safely_error` — compilation error returns 1
  - [ ] `test_execute_safely_panic` — panic returns 101
  - [ ] `test_execute_safely_flushes_on_panic` — diagnostics flushed before ICE message
  - [ ] `test_execute_safely_panic_info` — panic message included in ICE output

---

## 6.2 ICE Reporting

Human-readable error message when the compiler panics.

- [ ] **Implement**: ICE message format
  - [ ] "error: internal compiler error (ICE)"
  - [ ] "The Ori compiler encountered an unexpected error."
  - [ ] "This is a bug in the compiler, not in your code."
  - [ ] Panic info (if available)
  - [ ] "Please report this at: <issue URL>"
  - [ ] Compiler version and build number

- [ ] **Implement**: Write ICE output to `session.host().diagnostic_output()`
  - [ ] Use host output, not raw stderr, for testability

- [ ] **Rust Tests**: `compiler/oric/src/execute.rs`
  - [ ] `test_ice_message_contains_version` — version info in output
  - [ ] `test_ice_message_contains_report_url` — issue URL in output
  - [ ] `test_ice_diagnostics_flushed_first` — accumulated errors appear before ICE
  - [ ] `test_ice_with_test_host` — verify output captured by TestHost

---

## 6.3 Section Completion Checklist

- [ ] `execute_safely()` catches panics and flushes diagnostics
- [ ] ICE message includes version, build number, and report URL
- [ ] Exit codes are distinct: 0 (success), 1 (error), 101 (ICE)
- [ ] Diagnostics accumulated before panic are still emitted
- [ ] No regressions: `./test-all.sh` passes
- [ ] Public API documented with `///` doc comments

**Exit Criteria:** If any compilation phase panics, the user sees all errors that were found before the panic, plus a clear ICE message asking them to file a bug report.
