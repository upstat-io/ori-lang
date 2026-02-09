---
section: "05"
title: Pipeline + CompilerCallbacks
status: not-started
tier: 1
goal: Unified phase orchestration with extensible hooks
sections:
  - id: "5.1"
    title: Phase Enum
    status: not-started
  - id: "5.2"
    title: CompilerCallbacks Trait
    status: not-started
  - id: "5.3"
    title: Pipeline Struct
    status: not-started
  - id: "5.4"
    title: Ori-Specific Phases
    status: not-started
  - id: "5.5"
    title: Section Completion Checklist
    status: not-started
---

# Section 05: Pipeline + CompilerCallbacks

**Status:** 📋 Planned
**Goal:** Orchestrate compilation phases in dependency order with callback hooks at each phase boundary, replacing duplicated logic across command handlers.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Module 4
> **Inspired by**: Rust's `CompilerCallbacks` + Gleam's phase ordering
> **Location**: `compiler/oric/src/pipeline.rs`
> **Depends on**: Section 3 (Session), Section 4 (DiagnosticContext)

---

## 5.1 Phase Enum

Define the compilation phases in dependency order.

- [ ] **Implement**: `Phase` enum in `compiler/oric/src/pipeline.rs`
  - [ ] `Lex` — tokenization
  - [ ] `Parse` — AST construction
  - [ ] `TypeCheck` — type inference and checking
  - [ ] `TestVerify` — mandatory test coverage (Ori-specific)
  - [ ] `Evaluate` — interpreter execution
  - [ ] `Codegen` — LLVM code generation
  - [ ] Derive: `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord`
  - [ ] `Phase::display_name(&self) -> &'static str` — human-readable name

- [ ] **Implement**: `PhaseControl` enum
  - [ ] `Continue` — proceed to next phase
  - [ ] `Stop` — stop compilation after this phase
  - [ ] Derive: `Debug, Clone, Copy, PartialEq, Eq`

- [ ] **Rust Tests**: `compiler/oric/src/pipeline.rs`
  - [ ] `test_phase_ordering` — `Lex < Parse < TypeCheck < ...`
  - [ ] `test_phase_display_names` — all phases have names

---

## 5.2 CompilerCallbacks Trait

Hooks at each phase boundary for tools, linters, and LSP.

- [ ] **Implement**: `CompilerCallbacks` trait in `compiler/oric/src/pipeline.rs`
  - [ ] `fn after_lex(&mut self, tokens: &TokenList, errors: &[LexError]) -> PhaseControl`
  - [ ] `fn after_parse(&mut self, output: &ParseOutput) -> PhaseControl`
  - [ ] `fn after_type_check(&mut self, result: &TypeCheckResult) -> PhaseControl`
  - [ ] `fn after_test_verify(&mut self, untested: &[String]) -> PhaseControl`
  - [ ] `fn after_complete(&mut self, has_errors: bool)`
  - [ ] All methods default to `PhaseControl::Continue`

- [ ] **Implement**: `NoopCallbacks` — default implementation that does nothing
  - [ ] Useful for standard compilation without hooks

- [ ] **Rust Tests**: `compiler/oric/src/pipeline.rs`
  - [ ] `test_noop_callbacks` — all methods return Continue
  - [ ] `test_custom_callbacks_stop` — custom callback that stops at Parse
  - [ ] `test_custom_callbacks_inspect` — callback that collects phase data

---

## 5.3 Pipeline Struct

The core orchestrator that runs compilation phases in order.

- [ ] **Implement**: `Pipeline<'s>` struct in `compiler/oric/src/pipeline.rs`
  - [ ] `session: &'s mut Session`
  - [ ] `callbacks: Box<dyn CompilerCallbacks>`
  - [ ] `telemetry: Box<dyn Telemetry>` (defaults to `NullTelemetry` until Section 7)

- [ ] **Implement**: `Pipeline::new(session)` — create with NoopCallbacks
- [ ] **Implement**: `Pipeline::with_callbacks(session, callbacks)` — create with custom hooks
- [ ] **Implement**: `Pipeline::with_telemetry(self, telemetry)` — builder pattern

- [ ] **Implement**: `Pipeline::run_through(&mut self, target_phase: Phase, file: SourceFile) -> Outcome<CompileOutcome>`
  - [ ] Phase: Lex
    - [ ] Call `lex_errors(db, file)`
    - [ ] Add lex errors to DiagnosticContext
    - [ ] Call `callbacks.after_lex()`
    - [ ] Stop if target_phase == Lex or PhaseControl::Stop
  - [ ] Phase: Parse
    - [ ] Call `parsed(db, file)`
    - [ ] Add parse errors to DiagnosticContext
    - [ ] Call `callbacks.after_parse()`
    - [ ] Stop if target_phase == Parse or PhaseControl::Stop
  - [ ] Phase: TypeCheck
    - [ ] Call `typeck::type_check_with_imports_and_pool()`
    - [ ] Add type errors to DiagnosticContext
    - [ ] Call `callbacks.after_type_check()`
    - [ ] Stop if target_phase == TypeCheck or PhaseControl::Stop
  - [ ] Phase: TestVerify
    - [ ] Check test coverage (extract from current `check.rs:76-110`)
    - [ ] Add coverage errors to DiagnosticContext
    - [ ] Call `callbacks.after_test_verify()`
    - [ ] Stop if target_phase == TestVerify or PhaseControl::Stop
  - [ ] Phase: Evaluate
    - [ ] Call `evaluated(db, file)` or build evaluator directly
    - [ ] Handle eval errors
  - [ ] Phase: Codegen (gated on `#[cfg(feature = "llvm")]`)
    - [ ] Delegate to existing `compile_common.rs` infrastructure

- [ ] **Implement**: `CompileOutcome` struct
  - [ ] `parse_result: Option<ParseOutput>`
  - [ ] `type_result: Option<TypeCheckResult>`
  - [ ] `eval_result: Option<ModuleEvalResult>`
  - [ ] Access to intermediate results for callers that need them

- [ ] **Rust Tests**: `compiler/oric/src/pipeline.rs`
  - [ ] `test_pipeline_lex_only` — stops after lexing
  - [ ] `test_pipeline_parse_only` — stops after parsing
  - [ ] `test_pipeline_type_check` — runs through type checking
  - [ ] `test_pipeline_evaluate` — full pipeline to evaluation
  - [ ] `test_pipeline_with_errors` — accumulates errors across phases
  - [ ] `test_pipeline_callback_stop` — callback can halt pipeline
  - [ ] `test_pipeline_callback_inspect` — callback receives correct data
  - [ ] `test_pipeline_test_verify` — coverage check catches untested functions

---

## 5.4 Ori-Specific Phases

Pipeline phases unique to Ori's design pillars.

- [ ] **Implement**: TestVerify phase (extracted from `check.rs:76-110`)
  - [ ] Collect tested function names from `module.tests`
  - [ ] Find functions without tests (excluding `@main`)
  - [ ] Return list of untested function names

- [ ] **Design**: CapabilityCheck phase (future — Section 6 of roadmap)
  - [ ] Document where this phase will slot in (between TypeCheck and Evaluate)
  - [ ] Placeholder in Phase enum with `#[cfg]` gate

- [ ] **Design**: ContractCheck phase (future — pre_check/post_check)
  - [ ] Document where this phase will slot in
  - [ ] Placeholder in Phase enum with `#[cfg]` gate

---

## 5.5 Section Completion Checklist

- [ ] Pipeline runs all phases in correct order
- [ ] Callbacks can inspect and stop at any phase boundary
- [ ] DiagnosticContext accumulates errors from all phases
- [ ] Test coverage verification extracted from check.rs into pipeline
- [ ] `ori check` can be expressed as `pipeline.run_through(Phase::TypeCheck)`
- [ ] `ori run` can be expressed as `pipeline.run_through(Phase::Evaluate)`
- [ ] No regressions: `./test-all.sh` passes
- [ ] Public API documented with `///` doc comments

**Exit Criteria:** The `check`, `run`, and `build` commands can be expressed as `Pipeline::run_through(target_phase)` with identical behavior to the current implementations.
