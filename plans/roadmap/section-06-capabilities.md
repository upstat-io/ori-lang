---
section: 6
title: Capabilities System
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 2
goal: Effect tracking (moved earlier to unblock Section 8 cache and Section 11 FFI)
spec:
  - spec/20-capabilities.md
tpr_findings:
  - id: S06-01
    status: open
    description: "6.2: Roadmap claimed 12 tests (7 Rust + 5 Ori) but all tests are commented out or nonexistent"
    category: WRONG TEST
  - id: S06-02
    status: open
    description: "6.3: Roadmap claimed 4 Rust tests + test file; zero tests exist"
    category: WRONG TEST
  - id: S06-03
    status: open
    description: "6.5/6.8: Roadmap claimed 7 tests for E2014; zero such Rust tests exist"
    category: WRONG TEST
  - id: S06-04
    status: open
    description: "Prelude has `pub trait Async {}` but spec says `Suspend` (rename not applied)"
    category: BUG FOUND
  - id: S06-05
    status: open
    description: "6.9 marked not-started but basic unsafe {} IS implemented with 6 passing tests"
    category: STALE STATUS
  - id: S06-06
    status: open
    description: "Zero #compile_fail negative tests for any capability feature"
    category: NEEDS PIN
  - id: S06-07
    status: open
    description: "4 of 13 standard capabilities missing from prelude (Crypto, Print, Intrinsics, FFI)"
    category: INCOMPLETE MATRIX
  - id: S06-08
    status: open
    description: "Prelude capability trait signatures differ from spec (Cache generics, Clock types)"
    category: DRIFT
  - id: S06-09
    status: open
    description: "traits.ori and default-impl.ori contain commented-out code (violates hygiene rules)"
    category: DRIFT
  - id: S06-10
    status: open
    description: "Error codes E1200-E1207 from spec not implemented (only E2014 exists)"
    category: GAP
sections:
  - id: "6.1"
    title: Capability Declaration
    status: in-progress
  - id: "6.2"
    title: Capability Traits
    status: in-progress
  - id: "6.3"
    title: Suspend Capability
    status: in-progress
  - id: "6.4"
    title: Providing Capabilities
    status: in-progress
  - id: "6.5"
    title: Capability Propagation
    status: in-progress
  - id: "6.6"
    title: Standard Capabilities
    status: in-progress
  - id: "6.7"
    title: Testing with Capabilities
    status: in-progress
  - id: "6.8"
    title: Capability Constraints
    status: in-progress
  - id: "6.9"
    title: Unsafe Capability (FFI Prep)
    status: in-progress
  - id: "6.10"
    title: Default Implementations (def impl)
    status: in-progress
  - id: "6.11"
    title: Capability Composition
    status: not-started
  - id: "6.12"
    title: Default Implementation Resolution
    status: not-started
  - id: "6.13"
    title: Named Capability Sets (capset)
    status: not-started
  - id: "6.14"
    title: Intrinsics Capability (Generic SIMD, Byte Ops, Mask Type)
    status: not-started
  - id: "6.16"
    title: Stateful Handlers
    status: not-started
  - id: "6.17"
    title: Section Completion Checklist
    status: in-progress
---

# Section 6: Capabilities System

**Goal**: Effect tracking (moved earlier to unblock Section 8 cache and Section 11 FFI)

> **SPEC**: `spec/20-capabilities.md`
> **DESIGN**: `design/14-capabilities/index.md`

**Status**: In-progress — Core evaluator working (6.1, 6.4 verified; 6.2, 6.3 WRONG TEST; 6.5 partial; 6.9 partial, was incorrectly marked not-started; 6.10 partial); composition (6.11), resolution (6.12), intrinsics (6.14), stateful handlers (6.16) pending. LLVM tests missing. Zero `#compile_fail` negative tests. Verified 2026-03-29.

---

## 6.1 Capability Declaration

- [x] **Implement**: `uses` clause [done] (verified 2026-03-29) WEAK TESTS
  - [x] **Rust Tests**: `ori_parse/src/tests/parser.rs` — uses clause parsing (4 tests: single, multiple, with where, no uses) (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/capabilities/declaration.ori` (3 tests: pure function, with capability, generic with capability) (verified 2026-03-29)
  - [ ] **Negative Tests**: No `#compile_fail` tests for missing capabilities (S06-06)
  - [ ] **LLVM Support**: ARC lowering treats `WithCapability` as transparent passthrough; no dedicated LLVM/AOT tests verify this path
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Multiple capabilities [done] (verified 2026-03-29) WEAK TESTS
  - [x] **Rust Tests**: `ori_parse/src/tests/parser.rs` — `test_uses_clause_multiple_capabilities` (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — `fetch_and_log` with `uses Http, Logger` (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for multiple capabilities in function signatures
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (6.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.2 Capability Traits

- [ ] **Implement**: Capability traits WRONG TEST — reopened (was [done] 2026-02-10, failed verification 2026-03-29, S06-01)
  - [ ] **Rust Tests**: WRONG TEST — roadmap claimed "7 tests for capability trait validation" in `ori_types/src/check/tests.rs`; these tests DO NOT EXIST. `module_checker_function_scope` tests `has_capability()` but is not a capability trait validation test.
  - [ ] **Ori Tests**: WRONG TEST — `tests/spec/capabilities/traits.ori` is 126 lines ALL COMMENTED OUT (TODO: "Type checker needs capability support"). Zero active tests. Violates impl-hygiene.md: "No commented-out code ever." (S06-09)
  - [ ] **Fix**: Delete or uncomment `traits.ori` — commented-out code is a hygiene violation
  - [ ] **Fix**: Write actual capability trait validation tests (Rust + Ori)
  - [ ] **LLVM Support**: LLVM codegen for capability trait dispatch
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (6.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.3 Suspend Capability

> **Note**: Renamed from `Async` to `Suspend` per `proposals/approved/rename-async-to-suspend-proposal.md`

- [ ] **Implement**: Explicit suspension declaration STALE TEST — reopened (was [done] 2026-02-10, failed verification 2026-03-29, S06-02)
  - [ ] **Rust Tests**: WRONG TEST — roadmap claimed "4 tests (marker trait, signature storage, combined capabilities, sync function)" in `ori_types/src/check/tests.rs`; ZERO such tests exist.
  - [ ] **Ori Tests**: STALE TEST — `tests/spec/capabilities/async.ori` is 7 lines with only a comment: "This file is intentionally empty - async is not a language feature." Zero executable tests.
  - [ ] **BUG**: Prelude defines `pub trait Async {}` (line 250) but spec section 20.3 and approved `rename-async-to-suspend-proposal.md` say it should be `Suspend`. Rename not applied. (S06-04)
  - [ ] **Fix**: Rename `Async` to `Suspend` in prelude
  - [ ] **Fix**: Write actual suspension declaration tests (Rust + Ori)
  - [ ] **LLVM Support**: LLVM codegen for explicit suspension declaration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Sync vs suspending behavior STALE TEST — reopened (was [done] 2026-02-10, failed verification 2026-03-29)
  - [ ] **Rust Tests**: WRONG TEST — `test_sync_function_no_suspend_capability` does not exist in `ori_types/src/check/tests.rs`
  - [ ] **Ori Tests**: STALE TEST — `tests/spec/capabilities/async.ori` has zero executable tests

- [x] **Implement**: No `async` type modifier [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/tests/parser.rs` — 3 tests pass: `test_no_async_type_modifier`, `test_async_as_identifier`, `test_uses_async_capability_parses` (verified 2026-03-29)
  - [x] **Ori Tests**: Design notes document this (verified 2026-03-29)
  - [ ] **Note**: Roadmap previously cited `test_async_keyword_reserved` which does not exist — actual test is `test_async_as_identifier` (opposite semantics: `async` is NOT reserved)

- [ ] **Implement**: No `await` expression STALE TEST — reopened (was [done] 2026-02-10, failed verification 2026-03-29)
  - [ ] **Rust Tests**: WRONG TEST — `test_await_syntax_not_supported` does not exist in `ori_types/src/check/tests.rs`. Evaluator does have `CanExpr::Await(_) => await_not_supported()` but no test verifies this.
  - [ ] **Fix**: Write test for await rejection

- [ ] **Implement**: Concurrency with `parallel` — spec/20-capabilities.md § Suspend Capability
  - [ ] **Deferred to Section 8**: `parallel` pattern evaluation
  - [ ] **Ori Tests**: `tests/spec/patterns/parallel.ori` (Section 8)
  - [ ] **Note**: Evaluator has a loud stub for parallel in `can_eval.rs` — replace when Suspend capability is implemented

- [ ] **Subsection close-out (6.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.4 Providing Capabilities

- [x] **Implement**: `with...in` expression [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/tests/parser.rs` — with expression parsing (3 tests: expression, struct provider, nested) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/interpreter/mod.rs` — `CanExpr::WithCapability` uses `with_binding()` at `can_eval/mod.rs:350` (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/capabilities/providing.ori` — 17 tests, all pass (basic provision, struct provider, scoping, nested, shadowing, different types, conditionals, method calls, capability through function) (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/expressions/with_expr.ori` — 14 tests (11 pass, 3 skipped) (verified 2026-03-29)
  - [ ] **Negative Tests**: No `#compile_fail` negative tests for `with...in` (S06-06)
  - [ ] **LLVM Support**: LLVM codegen for `with...in` capability binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Scoping [done] (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/capabilities/providing.ori` — scoping, shadowing, three-level nesting, closure interaction (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for capability scoping (push/pop)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (6.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.5 Capability Propagation

- [ ] **Implement**: Runtime capability propagation [partial] (verified 2026-03-29)
  - [x] **Changes**: `prepare_call_env()` in `interpreter/function_call.rs:149-156` passes capabilities from calling scope to called function (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/capabilities/traits.ori` — ALL COMMENTED OUT, zero active tests (S06-01, S06-09)
  - [x] **Ori Tests**: `tests/spec/capabilities/providing.ori::test_capability_through_function` — passes via scope-based name lookup (verified 2026-03-29)
  - [ ] **Implement**: `with...in` capability provision propagates to called functions — `with Cap = impl in callee()` should make `Cap` available inside `callee()`
  - [ ] **Ori Tests**: `tests/spec/expressions/with_expr.ori` — 2 tests skipped: `test_basic_with` and `test_multiple_capabilities` ("capability provision to called functions not implemented")
  - [ ] **Investigate**: Skip reason says "not implemented" but `prepare_call_env` DOES pass capabilities — skipped tests may pass now (issue may be trait method dispatch vs simple name lookup)
  - [ ] **LLVM Support**: LLVM codegen for runtime capability propagation through calls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Static transitive requirements WEAK TESTS — reopened (was [done] 2026-02-10, failed verification 2026-03-29, S06-03)
  - [ ] **Rust Tests**: WRONG TEST — roadmap claimed "7 tests for E2014 propagation errors" in `ori_types/src/check/tests.rs`; ZERO such tests exist. E2014 error code IS implemented and registered.
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — 7 tests, all pass (valid propagation patterns) (verified 2026-03-29)
  - [ ] **Negative Pin**: No `#compile_fail("E2014")` test exists anywhere — need negative test verifying E2014 fires on missing capability
  - [ ] **Fix**: Write Rust tests and `#compile_fail("E2014")` negative pins

- [x] **Implement**: Providing vs requiring [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/expr/calls/constraints.rs` — `check_capability_propagation` function exists (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — tests with...in providing capabilities (verified 2026-03-29)

- [ ] **Subsection close-out (6.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.6 Standard Capabilities

> **STATUS**: 9 of 13 standard capability traits defined in `library/std/prelude.ori` (INCOMPLETE MATRIX, S06-07)
> Real implementations deferred to Section 7 (Stdlib).

- [x] **Define**: Trait interfaces [partial] (verified 2026-03-29) INCOMPLETE MATRIX
  - [x] **Location**: `library/std/prelude.ori` — trait definitions (verified 2026-03-29)
  - [x] **Traits defined**: Http, FileSystem, Cache, Clock, Random, Logger, Env, Async (should be Suspend), Unsafe — 9 traits in prelude (verified 2026-03-29)
  - [ ] **Missing from prelude** (spec section 20.8 lists 13): `Crypto` (spec 20.8.3 has full trait), `Print` (spec lists it), `Intrinsics` (spec 20.8.4 has detailed trait), `FFI` (spec lists it) (S06-07)
  - [ ] **DRIFT**: `Cache` signature uses `(key: str) -> Option<str>` but spec uses generics `<K: Hashable + Eq, V: Clone>` (S06-08)
  - [ ] **DRIFT**: `Clock` signature uses `@now () -> int, @today () -> str` but spec uses `@now () -> Instant, @local_timezone () -> Timezone` (S06-08)
  - [ ] **BUG**: Prelude has `pub trait Async {}` but should be `pub trait Suspend {}` per approved rename proposal (S06-04)

- [ ] **Implement** (Section 7): Real capability implementations
  - [ ] `std.net.http` — Http capability impl
  - [ ] `std.fs` — FileSystem capability impl
  - [ ] `std.time` — Clock capability impl
  - [ ] `std.math.rand` — Random capability impl
  - [ ] `std.cache` — Cache capability impl (new module)
  - [ ] `std.log` — Logger capability impl
  - [ ] `std.env` — Env capability impl
  - [ ] `std.crypto` — Crypto capability impl (missing from prelude)
  - [ ] `std.io` — Print capability impl (missing from prelude)

- [ ] **Subsection close-out (6.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.7 Testing with Capabilities

> **STATUS**: Complete — mocking works via trait implementations, demonstrated in propagation.ori

- [x] **Implement**: Mock implementations [done] (verified 2026-03-29) WEAK TESTS
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — MockHttp and MockLogger demonstrate `with...in` mocking pattern (verified 2026-03-29)
  - [ ] **Note**: Uses string mocks, not trait-implementing structs — weak demonstration of real mocking
  - [ ] **LLVM Support**: LLVM codegen for mock capability implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Test example [done] (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — shows test patterns with `with...in` (verified 2026-03-29)

- [ ] **Subsection close-out (6.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.8 Capability Constraints

> **STATUS**: Partially implemented — E2014 code exists but NEEDS PIN (S06-03, S06-06)

- [ ] **Implement**: Compile-time enforcement NEEDS PIN — reopened (was [done] 2026-02-10, failed verification 2026-03-29, S06-03)
  - [ ] **Rust Tests**: WRONG TEST — roadmap claimed "7 tests for E2014 propagation errors" in `ori_types/src/check/tests.rs`; ZERO such tests exist.
  - [x] **Implementation**: `check_capability_propagation` exists in `constraints.rs`, emits E2014 (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — positive tests pass (verified 2026-03-29)
  - [ ] **Negative Pin**: No `#compile_fail("E2014")` test exists anywhere — completely untested with negative pins (S06-06)
  - [ ] **Fix**: Write `#compile_fail("E2014")` test verifying error fires when capability is missing
  - [ ] **GAP**: Error codes E1200-E1207 from spec not implemented (only E2014 exists) (S06-10)

- [ ] **Subsection close-out (6.8)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.8 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.8: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.9 Unsafe Capability (FFI Prep)

**Proposal**: `proposals/approved/unsafe-semantics-proposal.md`

> **PREREQUISITE FOR**: Section 11 (FFI)
> The Unsafe capability is required for FFI. Implement this before starting FFI work.
> **STALE STATUS**: Was marked "not-started" but basic `unsafe { }` IS implemented and tested (S06-05). Updated to in-progress.

- [ ] **Implement**: `Unsafe` marker capability (compiler intrinsic, like `Suspend`)
  - [ ] Add `Unsafe` to standard capabilities list in type checker
  - [x] **Prelude**: `pub trait Unsafe {}` defined (line 254) (verified 2026-03-29)
  - [ ] `Unsafe` not treated as marker capability (no E1203 for `with Unsafe = ... in`)
  - [ ] No `UnsafeContext` tracking in type checker
  - [ ] No E1250 diagnostic
  - [ ] No enforcement that unsafe operations require `unsafe { }`
  - [ ] Generalize E1203 to cover all marker capabilities (not just `Suspend`)
  - [ ] **Ori Tests**: `tests/spec/capabilities/unsafe/` — basic tests, E1203 binding error
  - [ ] **LLVM Support**: LLVM codegen for `unsafe { }` blocks (transparent — same as inner expr)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` — unsafe block codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `unsafe { }` block expression (basic parsing + eval) [partial] (verified 2026-03-29)
  - [x] `ExprKind::Unsafe(ExprId)` in IR (`ori_ir/src/ast/expr.rs:306`) (verified 2026-03-29)
  - [x] Parser: `parse_unsafe_expr()` in `ori_parse/src/grammar/expr/primary/specials.rs:111` (verified 2026-03-29)
  - [x] Type checker: transparent pass-through (`ori_types/src/infer/expr/mod.rs:216`) (verified 2026-03-29)
  - [x] Evaluator: transparent pass-through (`ori_eval/src/interpreter/can_eval/mod.rs:337`) (verified 2026-03-29)
  - [x] ARC lowering: transparent (`ori_arc/src/lower/expr/mod.rs:298`) (verified 2026-03-29)
  - [x] Visitor: supported in `ori_ir/src/visitor/walk_expr.rs` (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/capabilities/unsafe_block.ori` — 6 passing tests: single expr, multi stmt, nested, in block, type, bool (verified 2026-03-29)
  - [ ] **Not implemented**: `UnsafeContext` tracking in type checker, E1250 diagnostic
  - [ ] **Not implemented**: Enforcement that unsafe operations require `unsafe { }`
  - [ ] **LLVM Tests**: No LLVM codegen tests

- [ ] **Implement**: Unsafe capability requirements (deferred to Section 11)
  - [ ] Required for: raw pointer operations (future)
  - [ ] Required for: C variadic function calls (future)
  - [ ] Required for: transmute operations (future)
  - [ ] Tests added when FFI implemented

> **Note:** `Unsafe` is a marker capability — it cannot be bound via `with...in`. There is no `AllowUnsafe` provider type. Unsafe code is tested by testing safe wrappers.

- [ ] **Subsection close-out (6.9)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.9 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.9: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.10 Default Implementations (`def impl`)

**Proposal**: `proposals/approved/default-impl-proposal.md`
**Status**: Partially implemented — parser + evaluator registration work, type checking + name resolution + end-to-end NOT working (verified 2026-03-29)

Introduce `def impl` syntax to declare a default implementation for a trait. Importing a trait with a `def impl` automatically binds the default.

### Implementation

- [x] **Implement**: Add `def` keyword to lexer — grammar.ebnf § DECLARATIONS (verified 2026-03-29)
  - [x] **Rust Tests**: Lexer has `TokenKind::Def` (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/capabilities/default-impl.ori` — ENTIRELY COMMENTED OUT (S06-09, violates hygiene rules)

- [x] **Implement**: Parse `def impl Trait { ... }` — grammar.ebnf § DECLARATIONS (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/item/impl_def/` — 5 parser tests (basic, public, multiple methods, empty, multiple def impls) (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/capabilities/default-impl.ori` — ENTIRELY COMMENTED OUT (S06-09)

- [x] **Implement**: IR representation for DefImpl (verified 2026-03-29)
  - [x] `DefImplDef` type exists, tracked in `Module.def_impls` (verified 2026-03-29)

- [ ] **Implement**: Type checking for `def impl`
  - [ ] **Rust Tests**: `ori_types/src/check/registration/` — register_def_impls
  - [ ] Verify trait exists
  - [ ] Method signatures converted to ImplMethodDef
  - [ ] Methods are associated (no self parameter)
  - [ ] One `def impl` per trait per module (coherence check)

- [x] **Implement**: Evaluator registration for def impl (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/module_registration.rs` — `collect_def_impl_methods` (2 tests) (verified 2026-03-29)
  - [ ] Methods registered under trait name for `TraitName.method()` calls — end-to-end dispatch NOT working

- [ ] **Implement**: Module export with default — 12-modules.md
  - [ ] Mark exports as "has default" when `def impl` exists
  - [ ] Bind default when importing trait

- [ ] **Implement**: Name resolution — 14-capabilities.md
  - [ ] Check `with...in` binding first
  - [ ] Check imported default second
  - [ ] Check module-local `def impl` third
  - [ ] **Ori Tests**: `tests/spec/capabilities/default-impl.ori`

- [ ] **Implement**: Evaluator dispatch for `def impl` — route method calls through default impl resolution in `ori_eval`
  - [ ] Dispatch method calls to bound default
  - [ ] Override via `with...in` works
  - [ ] **Ori Tests**: `tests/spec/capabilities/default-impl.ori`

- [ ] **Fix**: Delete or uncomment `tests/spec/capabilities/default-impl.ori` — entirely commented out, hygiene violation (S06-09)

- [ ] **Implement**: LLVM backend support
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_impl_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet
  - [ ] Codegen for `def impl` methods

- [ ] **Subsection close-out (6.10)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.10 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.10: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.11 Capability Composition

**Proposal**: `proposals/approved/capability-composition-proposal.md`

Specifies how capabilities compose: partial provision, nested binding semantics, capability variance, and resolution priority order.

### Implementation

- [ ] **Implement**: Multi-binding `with` syntax — grammar.ebnf § EXPRESSIONS
  - [ ] **Parser**: Extend `with_expr` to support comma-separated bindings
  - [ ] **Rust Tests**: `ori_parse/src/lib.rs` — multi-binding with expression parsing
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Partial provision — providing some capabilities while others use defaults
  - [ ] **Rust Tests**: `ori_types/src/check/tests.rs` — partial capability provision
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Nested `with...in` shadowing — inner bindings shadow outer
  - [ ] **Rust Tests**: `ori_types/src/check/tests.rs` — capability shadowing
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Capability variance — more caps can call fewer, not reverse
  - [ ] **Rust Tests**: `ori_types/src/check/tests.rs` — variance checking
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Resolution priority order — inner with → outer with → imported def impl → local def impl → error
  - [ ] **Rust Tests**: `ori_types/src/check/tests.rs` — resolution priority
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Suspend binding prohibition — `with Suspend = ...` is compile error
  - [ ] **Rust Tests**: `ori_types/src/check/tests.rs` — suspend prohibition (E1203)
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Error codes E1200-E1203
  - [ ] E1200: missing capability
  - [ ] E1201: unbound capability
  - [ ] E1202: type doesn't implement capability trait
  - [ ] E1203: Suspend cannot be explicitly bound

- [ ] **Implement**: LLVM backend support
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_composition_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (6.11)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.11 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.11: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.12 Default Implementation Resolution

**Proposal**: `proposals/approved/default-impl-resolution-proposal.md`

Specifies resolution rules for `def impl`: conflict handling, `without def` import syntax, re-export behavior, and resolution order.

### Implementation

- [ ] **Implement**: `without def` import syntax — grammar.ebnf § IMPORTS
  - [ ] Parse `use "module" { Trait without def }` to import trait without its default
  - [ ] **Rust Tests**: `ori_parse/src/lib.rs` — `without def` import modifier parsing
  - [ ] **Ori Tests**: `tests/spec/capabilities/def-impl-resolution.ori`

- [ ] **Implement**: Import conflict detection — one `def impl` per trait per scope
  - [ ] Error E1000: conflicting default implementations when two imports bring same trait's `def impl`
  - [ ] **Rust Tests**: `ori_types/src/check/tests.rs` — import conflict detection
  - [ ] **Ori Tests**: `tests/spec/capabilities/def-impl-resolution.ori`

- [ ] **Implement**: Duplicate `def impl` detection — one per trait per module
  - [ ] Error E1001: duplicate default implementation in same module
  - [ ] **Rust Tests**: `ori_types/src/check/tests.rs` — duplicate def impl detection
  - [ ] **Ori Tests**: `tests/spec/capabilities/def-impl-resolution.ori`

- [ ] **Implement**: Resolution order — with...in > imported def > module-local def
  - [ ] Innermost `with...in` binding takes precedence
  - [ ] Imported `def impl` overrides module-local
  - [ ] **Rust Tests**: `ori_types/src/check/tests.rs` — resolution priority
  - [ ] **Ori Tests**: `tests/spec/capabilities/def-impl-resolution.ori`

- [ ] **Implement**: Re-export with `without def` — permanently strips default from export path
  - [ ] `pub use "module" { Trait without def }` re-exports trait without default
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module/import.rs` — re-export stripping
  - [ ] **Ori Tests**: `tests/spec/capabilities/def-impl-resolution.ori`

- [ ] **Implement**: Error messages with help text
  - [ ] E1000: "use `Logger without def` to import trait without default"
  - [ ] E1001: show location of first definition
  - [ ] E1002: "`def impl` methods cannot have `self` parameter"
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — error formatting tests

- [ ] **Implement**: LLVM backend support
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/def_impl_resolution_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (6.12)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.12 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.12: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.13 Named Capability Sets (`capset`)

**Proposal**: `proposals/approved/capset-proposal.md`

Transparent aliases for capability sets. Expanded during name resolution before type checking. Reduces signature noise and creates stable dependency surfaces.

### Implementation

- [ ] **Implement**: Add `capset` keyword to lexer — grammar.ebnf § DECLARATIONS
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — `capset` token recognition
  - [ ] **Ori Tests**: `tests/spec/capabilities/capset.ori`

- [ ] **Implement**: Parse `capset_decl` — grammar.ebnf § DECLARATIONS
  - [ ] **Rust Tests**: `ori_parse/src/grammar/item/capset.rs` — CapsetDecl AST node parsing
  - [ ] **Ori Tests**: `tests/spec/capabilities/capset.ori`

- [ ] **Implement**: Name resolution expansion
  - [ ] Expand capset names in `uses` clauses to constituent capabilities
  - [ ] Transitive expansion (capsets containing capsets)
  - [ ] Deduplication (set semantics)
  - [ ] **Rust Tests**: `ori_types/src/check/` — capset expansion tests

- [ ] **Implement**: Capset cycle detection — topological sort in `ori_types` during capset expansion
  - [ ] Topological sort of capset definitions
  - [ ] Error E1220 for cyclic definitions
  - [ ] **Ori Tests**: `tests/spec/capabilities/capset-errors.ori`

- [ ] **Implement**: Capset validation rules — error reporting in `ori_types/src/check/`
  - [ ] Error E1221: empty capset
  - [ ] Error E1222: name collision with trait
  - [ ] Error E1223: member is not capability or capset
  - [ ] Warning W1220: redundant capability in `uses`
  - [ ] **Ori Tests**: `tests/spec/capabilities/capset-errors.ori`

- [ ] **Implement**: Capset visibility checking — `pub` capset members must be accessible
  - [ ] `pub` capset must not reference non-accessible capabilities
  - [ ] **Ori Tests**: `tests/spec/capabilities/capset-visibility.ori`

- [ ] **Implement**: Enhanced E1200 error messages
  - [ ] Show capset expansion context in "missing capability" errors
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — error formatting tests

- [ ] **Implement**: LSP support
  - [ ] Show capset expansion on hover
  - [ ] Autocomplete capset names in `uses` clauses

- [ ] **Subsection close-out (6.13)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.13 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.13: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.14 Intrinsics Capability

**Proposal**: `proposals/approved/intrinsics-capability-proposal.md` (v1)
**Proposal**: `proposals/approved/intrinsics-v2-byte-simd-proposal.md` (v2 — generic API, byte SIMD, Mask type)

Generic SIMD API, byte-level SIMD, `Mask<$N>` type, bit manipulation, and hardware feature detection. Atomics deferred to separate proposal.

### Implementation

- [ ] **Implement**: Generic SIMD type validation — spec/20-capabilities.md
  - [ ] Validate T x N combinations (byte: 16/32/64, int: 2/4/8, float: 2/4/8)
  - [ ] Validate operation availability by lane type (div/sqrt/abs float-only, shuffle byte-only)
  - [ ] Error E1063 for invalid T x N combinations
  - [ ] Error E1064 for operation not available for lane type
  - [ ] **Rust Tests**: `ori_types/src/check/` — SIMD type validation tests
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-type-validation.ori`

- [ ] **Implement**: `Mask<$N>` type — spec/20-capabilities.md
  - [ ] Add `Mask` as compiler-known type with const-generic parameter
  - [ ] Methods: `bits`, `any`, `all`, `count`, `first_set`
  - [ ] Implement `BitAnd`, `BitOr`, `BitNot` for `Mask<$N>`
  - [ ] Valid N values: 2, 4, 8, 16, 32, 64
  - [ ] **Rust Tests**: `ori_types/src/check/` — Mask type tests
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-mask.ori`

- [ ] **Implement**: Add `Intrinsics` trait to prelude — spec/20-capabilities.md
  - [ ] Generic SIMD operations (`simd_add<T, $N>`, etc.)
  - [ ] Comparison operations returning `Mask<N>`
  - [ ] Byte-specific: `simd_shuffle<$N>`
  - [ ] Aligned loads: `simd_load_aligned<T, $N>`
  - [ ] Select: `simd_select<T, $N>` (mask-driven)
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics.ori`

- [ ] **Implement**: Generic SIMD operations — float (128/256/512-bit)
  - [ ] `simd_add`, `simd_sub`, `simd_mul`, `simd_div`, `simd_sqrt`, `simd_abs`
  - [ ] `simd_cmpeq`, `simd_cmplt`, `simd_cmpgt` (return `Mask<N>`)
  - [ ] `simd_min`, `simd_max`, `simd_sum`, `simd_splat`
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-simd-float.ori`

- [ ] **Implement**: Generic SIMD operations — int (128/256/512-bit)
  - [ ] `simd_add`, `simd_sub`, `simd_mul`
  - [ ] `simd_cmpeq`, `simd_cmplt`, `simd_cmpgt` (return `Mask<N>`)
  - [ ] `simd_min`, `simd_max`, `simd_sum`, `simd_splat`
  - [ ] `simd_and`, `simd_or`, `simd_xor`, `simd_andnot`
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-simd-int.ori`

- [ ] **Implement**: Generic SIMD operations — byte (128/256/512-bit)
  - [ ] `simd_load`, `simd_load_aligned`, `simd_add`, `simd_sub`, `simd_mul`
  - [ ] `simd_cmpeq`, `simd_cmplt`, `simd_cmpgt` (return `Mask<N>`)
  - [ ] `simd_min`, `simd_max`, `simd_sum`, `simd_splat`
  - [ ] `simd_and`, `simd_or`, `simd_xor`, `simd_andnot`
  - [ ] `simd_shuffle` (byte-only), `simd_select`
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-simd-byte.ori`

- [ ] **Implement**: V1 deprecated aliases
  - [ ] Map `simd_add_f32x4` → `simd_add<float, 2>`, etc.
  - [ ] Emit deprecation warning for v1 names
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-v1-compat.ori`

- [ ] **Implement**: Bit manipulation operations
  - [ ] `count_leading_zeros`, `count_trailing_zeros`, `count_ones`
  - [ ] `rotate_left`, `rotate_right`
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-bits.ori`

- [ ] **Implement**: Hardware feature detection
  - [ ] `cpu_has_feature` with valid feature strings (add `ssse3`, `avx512bw`)
  - [ ] Error E1062 for unknown features
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-feature-detect.ori`

- [ ] **Implement**: `def impl Intrinsics` (NativeWithFallback)
  - [ ] Native SIMD when platform supports
  - [ ] Scalar emulation fallback
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-fallback.ori`

- [ ] **Implement**: `EmulatedIntrinsics` provider
  - [ ] Always uses scalar operations
  - [ ] For testing and portability
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-emulated.ori`

- [ ] **Implement**: Intrinsics error diagnostics
  - [ ] E1060: requires Intrinsics capability
  - [ ] E1062: unknown CPU feature
  - [ ] E1063: invalid SIMD type x width combination
  - [ ] E1064: operation not available for lane type
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — error formatting tests

- [ ] **Implement**: LLVM backend SIMD codegen
  - [ ] Generic dispatch: monomorphize to platform vector intrinsics
  - [ ] `Mask<$N>` codegen: `<N x i1>` in LLVM IR
  - [ ] Byte SIMD: `<16 x i8>`, `<32 x i8>`, `<64 x i8>`
  - [ ] Float SIMD: `<2 x double>`, `<4 x double>`, `<8 x double>`
  - [ ] Int SIMD: `<2 x i64>`, `<4 x i64>`, `<8 x i64>`
  - [ ] `count_ones` → `llvm.ctpop.i64`
  - [ ] `count_leading_zeros` → `llvm.ctlz.i64`
  - [ ] Runtime CPUID for feature detection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/intrinsics_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (6.14)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.14 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.14: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.16 Stateful Handlers

**Proposal**: `proposals/approved/stateful-mock-testing-proposal.md`

Extend `with...in` to support stateful effect handlers. The `handler(state: expr) { ... }` construct threads local mutable state through handler operations, enabling stateful capability mocking while preserving value semantics.

### Implementation

- [ ] **Implement**: Add `handler` as context-sensitive keyword — grammar.ebnf § EXPRESSIONS
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — `handler` token recognition (context-sensitive)
  - [ ] **Ori Tests**: `tests/spec/capabilities/stateful-handlers.ori`

- [ ] **Implement**: Parse `handler(state: expr) { op: expr, ... }` — grammar.ebnf § EXPRESSIONS
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr/with_expr.rs` — handler expression parsing
  - [ ] **Ori Tests**: `tests/spec/capabilities/stateful-handlers.ori`

- [ ] **Implement**: IR representation for handler expressions
  - [ ] Add `HandlerExpr` to expression AST (state initializer, operation map)
  - [ ] **Rust Tests**: `ori_ir/src/ast/expr/tests.rs` — handler AST node

- [ ] **Implement**: Type checker — verify handler operations match trait signature
  - [ ] State replaces `self` in operation signatures
  - [ ] Operations return `(S, R)` where S is state type, R is trait return type
  - [ ] State type inferred from initializer, consistent across all operations
  - [ ] All required trait methods must have handler operations
  - [ ] Default trait methods used if not overridden
  - [ ] **Rust Tests**: `ori_types/src/check/tests.rs` — handler type checking
  - [ ] **Ori Tests**: `tests/spec/capabilities/stateful-handlers.ori`

- [ ] **Implement**: Error codes E1204-E1207
  - [ ] E1204: handler missing required operation
  - [ ] E1205: handler operation signature mismatch
  - [ ] E1206: handler state type inconsistency
  - [ ] E1207: handler operation for non-existent trait method
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — handler error formatting
  - [ ] **Ori Tests**: `tests/spec/capabilities/stateful-handler-errors.ori`

- [ ] **Implement**: Evaluator — handler frame state threading
  - [ ] Create handler frame with initial state on `with...in` entry
  - [ ] Thread state through capability dispatch calls
  - [ ] Independent state per handler (nested handlers)
  - [ ] `with...in` returns body result only (state is internal)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/with_expr.rs` — handler evaluation
  - [ ] **Ori Tests**: `tests/spec/capabilities/stateful-handlers.ori`

- [ ] **Implement**: LLVM codegen for stateful handlers
  - [ ] Handler frame state allocation
  - [ ] State threading through operation calls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/stateful_handler_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (6.16)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.16 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.16: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.17 Section Completion Checklist

- [ ] 6.1 declaration — `uses` clause verified WEAK TESTS, needs negative pins and LLVM tests (reopened, verified 2026-03-29)
- [ ] 6.2 traits — WRONG TEST, zero active tests, traits.ori all commented out (reopened, verified 2026-03-29)
- [ ] 6.3 suspend — STALE TEST (explicit suspension, sync vs suspending, await rejection have zero tests), `No async modifier` verified (reopened, verified 2026-03-29)
- [x] 6.4 providing — `with...in` and scoping verified with 31+ passing tests (verified 2026-03-29)
- [ ] 6.5 propagation — partial: providing vs requiring verified, static transitive WRONG TEST (needs negative pins), runtime partial (reopened, verified 2026-03-29)
- [ ] 6.6 trait definitions — 9 of 13 defined, 4 missing (Crypto, Print, Intrinsics, FFI), signature drift vs spec (verified 2026-03-29)
- [x] 6.7 testing/mocking — mock pattern verified WEAK TESTS (verified 2026-03-29)
- [ ] 6.8 compile-time enforcement — implementation exists but WRONG TEST (zero Rust tests) and no negative pins (reopened, verified 2026-03-29)
- [ ] 6.9 Unsafe — basic `unsafe { }` block IS implemented (6 passing tests), but marker capability enforcement NOT implemented (was not-started, now in-progress, verified 2026-03-29)
- [ ] 6.10 Default implementations (`def impl`) — parser + IR + eval registration work, type checking + name resolution + end-to-end NOT working, test files all commented out (verified 2026-03-29)
- [ ] 6.11 Capability Composition — not started (verified 2026-03-29)
- [ ] 6.12 Default Implementation Resolution — not started (verified 2026-03-29)
- [ ] 6.13 Named Capability Sets (`capset`) — not started (verified 2026-03-29)
- [ ] 6.14 Intrinsics Capability (Generic SIMD, Byte Ops, Mask Type) — not started (verified 2026-03-29)
- [ ] 6.16 Stateful Handlers — not started (verified 2026-03-29)
- [ ] LLVM codegen for capabilities — zero LLVM/AOT test files exist
- [ ] `#compile_fail` negative pins — zero exist for any capability feature (S06-06)
- [ ] Prelude `Async` -> `Suspend` rename (S06-04)
- [ ] Fix commented-out test files: traits.ori, default-impl.ori (S06-09)
- [ ] Implement error codes E1200-E1207 from spec (only E2014 exists) (S06-10)
- [ ] Full test suite: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Effect tracking works per spec (6.1-6.8 evaluator complete, 6.9-6.14, 6.16 pending)
**Status**: Verified 2026-03-29. Significant accuracy problems found: phantom Rust tests cited in 6.2/6.3/6.5/6.8, stale status on 6.9, commented-out test files.

**Remaining for Section 7 (Stdlib)**:
- Real capability implementations (Http, FileSystem, etc.)
- Add missing standard capabilities to prelude (Crypto, Print, Intrinsics, FFI)
- Fix signature drift (Cache generics, Clock types)
- Integration with stdlib modules

**Remaining for Section 11 (FFI)**:
- Unsafe capability enforcement for extern functions

- [ ] **Subsection close-out (6.17)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.17 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.17: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

