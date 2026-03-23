# Proposal: Capability Propagation Completion

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-22
**Affects:** Compiler (type checker, evaluator, LLVM)
**Related:** `capability-composition-proposal.md` (approved), `capset-proposal.md` (approved), `stateful-mock-testing-proposal.md` (approved)
**Research:** `plans/deep-safety/research.md` (Part 9 — prerequisite infrastructure identified by third-party review)

---

## Summary

Complete the implementation of capability propagation as already specified in the capabilities spec (§20.6-20.7). Currently, capability provision to called functions is partially implemented — several spec-mandated behaviors are skipped or stubbed. This proposal identifies the gaps and defines the completion criteria.

This is not a new language feature. It is completing an already-approved design to establish the foundation for all future capability work, including the `without` clause (negative effects) and Deep Safety capabilities.

---

## Problem Statement

### Current State

The capabilities system has the following implementation gaps (as identified by the third-party review on 2026-03-22):

1. **Capability provision to callees**: When a `with Cap = impl in { body }` block is active, functions called within `body` that declare `uses Cap` should receive the provided implementation. This is partially working — some test paths show skipped provision.

2. **Stateful handlers**: The `handler(state: init) { op: (s) -> (s', val) }` pattern from the stateful-mock-testing proposal is not fully implemented in the evaluator or LLVM backend.

3. **LLVM/AOT capability support**: `uses` and `with...in` are handled in the evaluator but not in the LLVM codegen path. This means capability-tracked code cannot be compiled ahead-of-time.

4. **Marker capability discharge propagation**: `unsafe { }` and the `Suspend` marker exist syntactically but the type checker does not fully enforce the propagation rules (§20.9.2) — specifically, that calling a function with `uses Unsafe` without either declaring `uses Unsafe` or wrapping in `unsafe { }` should be error E1250.

### Why This Must Be Fixed Before Deep Safety

Every Deep Safety feature depends on correct capability propagation:
- `without` clause (negative effects) requires denied sets to propagate through call chains
- Low-level capabilities (`InterruptCtx`, `VolatileIO`) require callers to declare them
- Capability-gated FFI (`uses FFI("lib")`) requires parametric provision via `with...in`
- Contract verification on capability-gated functions requires correct capability context

Building Deep Safety on incomplete propagation infrastructure would produce silent correctness bugs — functions appearing to work without the required capability.

---

## Design

This proposal does not introduce new syntax or semantics. It defines **completion criteria** for the existing spec.

### Criterion 1: Positive Propagation

A function that declares `uses X` can only be called from a context where `X` is available. Availability means one of:
- The caller declares `uses X`
- A `with X = impl in { ... }` block is active in the caller's scope
- An imported `def impl X` is in scope

**Test**: A function `@f () -> void uses Http = ...` called from `@g () -> void = f()` without `Http` available must produce E1200.

### Criterion 2: `with...in` Provision

When `with X = impl in { body }`, all functions called within `body` that require `X` receive `impl` as their implementation. This must work for:
- Direct calls
- Indirect calls through function values
- Calls through trait method dispatch
- Nested `with...in` blocks (inner shadows outer)

**Test**: `with Http = MockHttp {} in { fetch_data() }` where `@fetch_data () -> str uses Http` must dispatch Http operations to `MockHttp`.

### Criterion 3: Marker Capability Enforcement

Marker capabilities (`Unsafe`, `Suspend`) shall:
- Propagate through call chains like any capability
- Cannot be bound via `with...in` (E1203)
- Be discharged by their specific mechanism (`unsafe { }` for `Unsafe`, runtime for `Suspend`)

**Test**: Calling `@f () -> void uses Unsafe` without `unsafe { }` or `uses Unsafe` → E1250.

### Criterion 4: Evaluator Completeness

The evaluator must:
- Track active capability bindings through `with...in` scopes
- Dispatch capability trait method calls to the provided implementation
- Handle stateful handlers (state threading through handler operations)
- Correctly scope capability bindings (inner `with...in` shadows outer)

### Criterion 5: LLVM/AOT Completeness

The LLVM codegen must:
- Propagate capability implementations as implicit parameters (or through a global context)
- Generate correct dispatch for capability trait method calls
- Handle `with...in` scope entry/exit in the generated code
- Support `unsafe { }` as a transparent wrapper (already a no-op in codegen)

---

## Implementation

### Phase 1: Audit Current Gaps (1-2 days)

Run the existing `tests/spec/expressions/with_expr.ori` and `tests/spec/capabilities/` test suites. Identify which tests are skipped, failing, or missing. Produce a gap list.

### Phase 2: Type Checker Fixes (1-2 weeks)

1. Fix capability propagation to callees — ensure E1200 fires when a required capability is unavailable
2. Fix marker capability enforcement — ensure E1250 fires for uncontained `uses Unsafe`
3. Fix E1203 enforcement — ensure marker capabilities cannot be bound via `with...in`
4. Add missing test coverage for propagation through indirect calls, trait dispatch, nested scopes

### Phase 3: Evaluator Fixes (1-2 weeks)

1. Complete `with...in` capability provision in the evaluator
2. Implement or complete stateful handler support
3. Verify correct scoping (inner shadows outer)
4. Add spec tests for all handler patterns from stateful-mock-testing proposal

### Phase 4: LLVM/AOT Support (2-4 weeks)

1. Design the runtime representation of capability bindings (implicit parameter? thread-local? global table?)
2. Generate `with...in` scope entry/exit code
3. Generate capability dispatch code for trait method calls
4. AOT tests for all capability patterns

---

## Spec Changes Required

None. The spec already defines the correct behavior (§20.4-20.7, §20.9). This proposal completes the implementation.

---

## Success Criteria

All of the following must pass:

```
cargo st tests/spec/capabilities/
cargo st tests/spec/expressions/with_expr.ori
cargo t -p ori_types  # capability propagation unit tests
cargo t -p ori_eval   # evaluator capability dispatch tests
cargo t -p ori_llvm   # AOT capability tests
```

No skipped tests in the capability test suites. All E1200, E1203, E1250 error codes fire correctly.
