---
section: "09"
title: Outcome Enum
status: not-started
tier: 2
goal: Three-state result type distinguishing total success, partial success, and total failure
sections:
  - id: "9.1"
    title: Outcome Type
    status: not-started
  - id: "9.2"
    title: Integration
    status: not-started
  - id: "9.3"
    title: Section Completion Checklist
    status: not-started
---

# Section 09: Outcome Enum

**Status:** 📋 Planned
**Goal:** Define a three-state result type that distinguishes total success, partial success (with warnings), and total failure — replacing ad-hoc `bool has_errors` tracking.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Outcome Enum
> **Inspired by**: Gleam's `Outcome<T, E>` (Ok / PartialFailure / TotalFailure)
> **Location**: `compiler/oric/src/outcome.rs`
> **Depends on**: None (pure type definition)

---

## 9.1 Outcome Type

- [ ] **Implement**: `Outcome<T>` enum in `compiler/oric/src/outcome.rs`
  - [ ] `Ok(T)` — complete success, no diagnostics
  - [ ] `PartialSuccess(T, Vec<Diagnostic>)` — produced result but with warnings/non-fatal issues
  - [ ] `TotalFailure` — no useful result produced

- [ ] **Implement**: Core methods
  - [ ] `fn is_ok(&self) -> bool` — true for Ok and PartialSuccess
  - [ ] `fn is_failure(&self) -> bool` — true only for TotalFailure
  - [ ] `fn value(&self) -> Option<&T>` — get value if not total failure
  - [ ] `fn into_value(self) -> Option<T>` — consume for value
  - [ ] `fn map<U>(self, f: impl FnOnce(T) -> U) -> Outcome<U>` — transform value
  - [ ] `fn and_then<U>(self, f: impl FnOnce(T) -> Outcome<U>) -> Outcome<U>` — chain

- [ ] **Implement**: Derive `Debug, Clone` where `T: Debug + Clone`

- [ ] **Rust Tests**: `compiler/oric/src/outcome.rs`
  - [ ] `test_outcome_ok` — is_ok true, is_failure false, value Some
  - [ ] `test_outcome_partial` — is_ok true, is_failure false, value Some
  - [ ] `test_outcome_failure` — is_ok false, is_failure true, value None
  - [ ] `test_outcome_map` — transforms Ok and PartialSuccess values
  - [ ] `test_outcome_and_then` — chains outcomes
  - [ ] `test_outcome_into_value` — consumes for value

---

## 9.2 Integration

Wire Outcome into existing infrastructure.

- [ ] **Implement**: `From<Result<T, E>>` for `Outcome<T>` where appropriate
  - [ ] `Ok(v)` → `Outcome::Ok(v)`
  - [ ] `Err(_)` → `Outcome::TotalFailure`

- [ ] **Implement**: `DiagnosticContext::into_outcome<T>(&self, value: T) -> Outcome<T>`
  - [ ] If has_errors: `Outcome::TotalFailure`
  - [ ] Else: `Outcome::Ok(value)`

- [ ] **Implement**: `Outcome<()>` → `ExitCode` conversion
  - [ ] `Ok(())` → `ExitCode::SUCCESS`
  - [ ] `PartialSuccess((), _)` → `ExitCode::SUCCESS` (warnings don't fail)
  - [ ] `TotalFailure` → `ExitCode::FAILURE`

- [ ] **Rust Tests**: `compiler/oric/src/outcome.rs`
  - [ ] `test_from_result` — Result → Outcome conversion
  - [ ] `test_to_exit_code` — Outcome → ExitCode mapping

---

## 9.3 Section Completion Checklist

- [ ] `Outcome<T>` type exists with three variants
- [ ] Core methods (is_ok, value, map, and_then) work correctly
- [ ] Integration with DiagnosticContext
- [ ] Conversion to ExitCode
- [ ] No regressions: `./test-all.sh` passes
- [ ] Public API documented with `///` doc comments

**Exit Criteria:** `Outcome<T>` replaces all ad-hoc `bool has_errors` + `Option<T> result` patterns in command handlers.
