# Proposal: Formalize Test Terminology

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-01-30
**Approved:** 2026-01-30

---

## Summary

Standardize test terminology across the specification and documentation:

| Current (inconsistent) | Proposed (standard) |
|------------------------|---------------------|
| "targeted test", "bound test" | **attached test** |
| "free-floating test" | **floating test** |

---

## Motivation

The current documentation uses inconsistent terminology:

- **Specification** (`13-testing.md`): "targeted tests" and "free-floating tests"
- **Dependency-Aware Testing Proposal**: "bound tests" and "free-floating tests"
- **Test Execution Model Proposal**: "targeted tests" and "free-floating tests"

This inconsistency causes confusion. The terms should be:
1. **Concise** — shorter is better for frequent use
2. **Clear** — the meaning should be obvious
3. **Consistent** — one term everywhere

---

## Design

### Attached Test

An _attached test_ declares one or more functions it tests using `tests @target`:

```ori
@test_add tests @add () -> void = {
    assert_eq(actual: add(a: 2, b: 3), expected: 5)
}
```

The test is "attached" to `@add`. When `@add` or its callers change, the attached test runs.

**Properties:**
- Satisfies test coverage requirement for its targets
- Runs during `ori check` when affected by changes
- Part of the dependency graph

### Floating Test

A _floating test_ uses `_` as its target, indicating no attachment:

```ori
@test_integration tests _ () -> void = {
    let result = full_pipeline(input: "program")
    assert_ok(result: result)
}
```

The test "floats" — it has no anchor to any specific function.

**Properties:**
- Does not satisfy coverage requirements
- Does not run during `ori check`
- Runs only via explicit `ori test`
- Not part of the dependency graph

### Why These Terms

| Term | Rationale |
|------|-----------|
| **attached** | The test is attached to specific functions. Clear, active, visual metaphor. |
| **floating** | The test floats without attachment. Natural opposite of "attached". Short form of "free-floating". |

Alternative considered:
- "bound" / "unbound" — Too abstract, "unbound" sounds like an error
- "targeted" / "untargeted" — "Untargeted" implies aimless, which is wrong
- "unit" / "integration" — Wrong framing; floating tests can be unit tests

---

## Changes Required

### Specification Updates

Update `docs/ori_lang/0.1-alpha/spec/13-testing.md`:

```diff
-### Targeted Tests
+### Attached Tests

-A _targeted test_ declares one or more functions it tests:
+An _attached test_ declares one or more functions it tests:

-### Free-Floating Tests
+### Floating Tests

-A _free-floating test_ uses `_` as its target:
+A _floating test_ uses `_` as its target:

-Free-floating tests:
+Floating tests:
```

Update all instances throughout the specification.

### Proposal Updates

Update approved proposals:
- `dependency-aware-testing-proposal.md`
- `test-execution-model-proposal.md`
- `incremental-test-execution-proposal.md`

### CLAUDE.md Updates

```diff
-**Tests**: `@t tests @fn () -> void` | `tests _` free-floating | ...
+**Tests**: `@t tests @fn () -> void` | `tests _` floating | ...
```

---

## Summary

| Aspect | Attached Test | Floating Test |
|--------|---------------|---------------|
| Syntax | `tests @target` | `tests _` |
| Coverage | Satisfies requirement | Does not satisfy |
| When runs | `ori check` (if affected) | `ori test` only |
| Dependency graph | Included | Excluded |
| Use case | Unit tests | Integration tests, benchmarks |

---

## Implementation

This proposal is documentation-only. No compiler modifications are required.

### Scope

**In scope (this proposal):**
- Specification updates (`13-testing.md`)
- Approved proposal updates
- `CLAUDE.md` updates

**Out of scope (future work):**
- CLI output messages (tracked in Phase 14: Testing)
- Compiler error messages (tracked in Phase 14: Testing)
- Diagnostic help text

### Files to Update

1. `docs/ori_lang/0.1-alpha/spec/13-testing.md`
2. `docs/ori_lang/proposals/approved/dependency-aware-testing-proposal.md`
3. `docs/ori_lang/proposals/approved/test-execution-model-proposal.md`
4. `docs/ori_lang/proposals/approved/incremental-test-execution-proposal.md`
5. `/CLAUDE.md`
