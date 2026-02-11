# Eval Legacy Path Removal

> **Description**: Surgical removal of the legacy `eval(ExprId)` evaluation path from `ori_eval`. The eval_v2 migration introduced a canonical evaluation path (`eval_can(CanId)` in `can_eval.rs`) alongside the legacy path (`eval(ExprId)` in `mod.rs`). The eval_v2 plan was closed out (commit `cb165b71`) declaring completion, but the legacy path was never actually removed. Today, ~400+ lines of legacy eval code remain as fallback, and every entry point uses `if canon { eval_can } else { eval }` dispatch.
>
> **Primary Goal**: Delete the legacy eval path entirely. Every evaluation goes through `eval_can(CanId)`. No dual dispatch, no fallback, no dead code.

## Blockers (Must Resolve Before Deletion)

| # | Blocker | Scope | Detail |
|---|---------|-------|--------|
| B1 | **Incomplete FunctionExp patterns** | `can_eval.rs:956-1012` | Canonical `eval_can_function_exp` handles Print, Panic, Todo, Unreachable inline; Catch, Recurse lazily. Missing: **Cache, Parallel, Spawn, Timeout, With** — these error with "not yet supported". Legacy `ori_patterns/src/` implementations are themselves stubs (no real caching, no real timeouts). Resolution: add loud stub match arms with `tracing::warn!`, not full implementations (those are roadmap items). |
| B2 | **PatternExecutor trait** | `ori_patterns` | Trait signature is `eval(&mut self, ExprId)`. 28+ callsites across fusion.rs, spawn.rs, parallel.rs, with_pattern.rs, recurse.rs. Cannot be auto-converted to CanId (different arena spaces) |
| B3 | **Fallback entry points** | `oric` | `run.rs:194-197` uses `if root_for(name).is_some() { eval_can } else { eval }`. Test runner passes canonical IR via builder (line 354), but evaluator dispatches internally |
| B4 | **Dual body architecture** | `ori_patterns` | `FunctionValue` (composite.rs:109-159) carries both legacy (`body: ExprId`, `defaults`, `arena`) and canonical (`can_body: CanId`, `canon`, `can_defaults`) fields. `function_call.rs` dispatches on `has_canon()` at 7 locations |

**Key insight:** B1 is the root cause. Once ALL FunctionExp patterns are handled inline in the canonical path, `PatternExecutor::eval(ExprId)` is no longer called from canonical evaluation. That unblocks B2 (trait can be deleted or left for legacy-only consumers). B3 and B4 are then mechanical cleanup.

## Architecture Overview

```
BEFORE (dual dispatch):

  oric entry points
    ├── root_for(name).is_some()? ──→ eval_can(CanId) ──→ can_eval.rs
    │                                    └── FunctionExp fallback ──→ PatternExecutor::eval(ExprId) ──→ legacy
    └── else ──→ eval(ExprId) ──→ mod.rs eval_inner()

  function_call.rs
    ├── has_canon()? ──→ eval_can(can_body)
    └── else ──→ eval(body)

  FunctionValue { body: ExprId, can_body: CanId, arena, canon, ... }

AFTER (single path):

  oric entry points ──→ eval_can(CanId) ──→ can_eval.rs (sole evaluator)

  function_call.rs ──→ eval_can(can_body)

  FunctionValue { can_body: CanId, canon: SharedCanonResult, ... }
```

## Implementation Tiers

### Tier 1: Validation (Section 01)
- Section 01: Audit canonical coverage — confirm every function/test has canonical IR

### Tier 2: Unblock Deletion (Section 02)
- Section 02: Stub Cache, Parallel, Spawn, Timeout, With in `eval_can_function_exp` with loud `tracing::warn!` (NOT real implementations — those are roadmap items)

### Tier 3: Remove Dispatch (Sections 03-04, concurrent)
- Section 03: Remove entry-point fallbacks in oric
- Section 04: Remove function/method call fallbacks

### Tier 4: Clean Data Types (Section 05)
- Section 05: Strip dual body fields from FunctionValue/UserMethod

### Tier 5: Delete Dead Code (Section 06)
- Section 06: Delete legacy eval_inner, function_seq, dead exec functions

### Tier 6: Polish (Section 07)
- Section 07: RAII scope guards, eager collect removal, mutex upgrade, visibility audit

## Execution Order

```
Section 01 (Audit)
    ↓
Section 02 (Inline FunctionExp patterns)  ← unblocks PatternExecutor removal
    ↓
Section 03 (Remove entry-point fallbacks)  ←─┐
    ↓                                         ├── concurrent
Section 04 (Remove call dispatch fallbacks) ──┘
    ↓
Section 05 (Strip dual body fields)
    ↓
Section 06 (Delete legacy code)
    ↓
Section 07 (Hygiene pass)
```

Sections 03 and 04 can be done concurrently. Everything else is sequential.

## Verification

After each section:
```bash
cargo c -p ori_eval && cargo c -p oric    # Compile check
cargo t -p ori_eval                        # Unit tests
cargo t -p oric                            # Integration tests
```

After all sections:
```bash
./test-all.sh      # Full test suite
./clippy-all.sh    # Lint
```

## Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Some functions lack canonical roots (Section 01 fails) | Fix canonicalization gaps before proceeding |
| Pattern implementations have subtle behavior differences | Port patterns carefully from ori_patterns source, test each one |
| SharedArena still needed by canonical path | Audit before removing from FunctionValue; keep if needed |
| ExprId still needed by some non-eval subsystem | Grep exhaustively before removing from FunctionValue |
| Test suite insufficient to catch regressions | Run `./test-all.sh` + `cargo st` after every section |

## Implementation Progress

> Last updated: 2026-02-10.

| Section | Status | Primary Crate |
|---------|--------|---------------|
| 01 | **Complete** | `oric`, `ori_eval`, `ori_types` |
| 02 | **Complete** | `ori_eval` |
| 03 | **Complete** | `oric` |
| 04 | **Complete** | `ori_eval` |
| 05 | **Complete** | `ori_patterns`, `ori_eval` |
| 06 | **Complete** | `ori_eval`, `oric` |
| 07 | **Complete** | `ori_eval` |

### Section 01 Results

Assertions placed at all 7 fallback sites. Import pipeline fixed to thread canonical IR. Ordering type duality bug found and **fixed**: two sites (`registration.rs:38`, `typeck.rs:142`) created `Named("Ordering")` via `pool.named()` instead of using `Idx::ORDERING`. Fix applied to both sites plus preventive shortcuts in `resolve_parsed_type_simple` for Ordering/Duration/Size. All 5 dispatch-site assertions upgraded from `tracing::warn!` to `debug_assert!`. Zero legacy fallback branches taken. See `section-01-audit.md` for full analysis.

### Section 02 Results

Five FunctionExp patterns (Cache, Parallel, Spawn, Timeout, With) stubbed with `tracing::warn!` and minimal correct behavior in `can_eval.rs`. No more legacy `PatternExecutor` fallback from canonical evaluation. See `section-02-inline-patterns.md`.

### Section 03+04 Results

**Section 03:** All entry points (run.rs, runner.rs, query/mod.rs) call `eval_can()` unconditionally. `harness.rs` converted from legacy `eval(func.body)` to full canonical pipeline. `Evaluator::eval()` wrapper removed (dead code). `Interpreter::eval()` remains `pub` only for `PatternExecutor` trait (deferred to Section 06).

**Section 04:** All `has_canon()` checks removed from `function_call.rs` (7 sites) and `method_dispatch.rs` (2 sites). Legacy `bind_parameters_with_defaults` deleted; `bind_parameters_with_can_defaults` renamed to `bind_parameters_with_defaults`. 3 legacy unit tests removed (created `UserMethod` without canonical IR — covered by 3052 spec tests). `clippy::cloned_ref_to_slice_refs` fix in With stub.

### Section 05 Results

Removed `defaults: Vec<Option<ExprId>>` field from `FunctionValue`. Removed `has_canon()` from both `FunctionValue` and `UserMethod`. Removed `has_can_defaults()` from `FunctionValue`. Deleted `with_defaults()` and `with_shared_captures_and_defaults()` constructors. Fixed named-arg default evaluation: `eval_call_named` now uses `can_defaults()` + `eval_can()` instead of legacy `defaults` + `eval()`. Canon is set BEFORE the parameter binding loop. Remaining legacy fields (`body: ExprId`, `arena: SharedArena`, `canon: Option<>`) deferred to Section 06 when the legacy eval path is fully deleted.

### Section 06 Results

~1900 lines of legacy eval code deleted across `ori_eval` and `oric`. Key deletions: `eval_inner()` (~390 lines), `function_seq.rs` (159 lines), all legacy exec/ helpers (`eval_if`, `eval_match`, `eval_loop`, `eval_block`, `eval_assign`, `try_match`, `bind_pattern`, `eval_literal`, `eval_binary`), `eval_call_named`, `types_match`, `is_mixed_primitive_op`, `ForIterator`. Removed dead fields `registry` and `expr_types` from both Interpreter and EvaluatorBuilder. Removed `context` field from oric's EvaluatorBuilder (dead). `PatternExecutor::eval()` now panics. `mod.rs` went from ~1847 → 749 lines. `control.rs` went from 672 → 33 lines (only `LoopAction` + `to_loop_action` remain). `CompilerContext` identified as fully dead (deferred to Section 07).

### Section 07 Results

Hygiene pass complete. **RAII scope guards:** Replaced all 12 manual `push_scope()`/`pop_scope()` calls across 5 functions (`eval_can_block`, `eval_can_for`, `eval_can_match`, `WithCapability`, `eval_can_recurse`) with `scoped()`, `with_binding()`, and `with_match_bindings()` guards. **Performance:** Replaced eager `.collect::<Vec<_>>()` in `eval_can_for` with lazy `Box<dyn Iterator>`; inlined range evaluation to eliminate `CanId→ExprId→CanId` roundtrip and deleted dead `eval_range()`. **Consistency:** Switched `BufferPrintHandler` from `std::sync::Mutex` to `parking_lot::Mutex`. **Visibility:** Tightened `ModeState::call_count` (private), `Interpreter::imported_arena` (pub(crate)), `Interpreter::print_handler` (pub(crate)), `TypeNames` struct (pub(crate)). 8419 tests pass, 0 clippy warnings.

**The eval legacy removal plan is now fully complete.** All 7 sections delivered. The legacy `eval(ExprId)` path has been completely removed.

## Dependencies

```
Section 01 (audit)       — standalone, prerequisite for all
Section 02 (patterns)    — depends on 01 passing
Section 03 (entry pts)   — depends on 02
Section 04 (call dsp)    — depends on 02
Section 05 (dual body)   — depends on 03 + 04
Section 06 (delete)      — depends on 05
Section 07 (hygiene)     — depends on 06
```
