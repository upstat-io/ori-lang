---
section: "03"
title: "Type Solver Budget Infrastructure"
status: not-started
reviewed: false
goal: "Add fuel counters and depth limits to the type solver so pathological generics terminate predictably with structured diagnostics instead of stack overflow or nontermination"
success_criteria:
  - "Unification has a configurable fuel counter (default: 10_000 steps) that produces E2042 on exhaustion"
  - "Substitution has a depth limit (default: 256) that produces E2043 on overflow"
  - "Trait resolution has a recursion limit (default: 128) with a dedicated diagnostic"
  - "Pathological programs (recursive generics, deep instantiation chains) terminate with clear errors"
  - "Normal programs are unaffected — fuel limits are high enough for all existing tests"
  - "typeck.md contains the solver budget rules"
inspired_by:
  - "TypeScript checker.ts:1509-1514 (instantiationCount, instantiationDepth tracking)"
  - "Rust trait solver recursion limit (default 128, configurable via #![recursion_limit])"
  - "Koka src/Type/Infer.hs (occurs check + depth-bounded unification)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Unification Fuel Counter"
    status: not-started
  - id: "03.2"
    title: "Substitution Depth Limit"
    status: not-started
  - id: "03.3"
    title: "Trait Resolution Recursion Limit"
    status: not-started
  - id: "03.4"
    title: "Rule Documentation & Error Codes"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Type Solver Budget Infrastructure

**Status:** Not Started
**Goal:** Add explicit fuel/budget limits to the type solver so pathological generic programs terminate predictably with structured diagnostics. Currently the solver relies on the generic "recursion depth: 256" rule in `impl-hygiene.md:600` and specific local limits (rank saturation at `u16::MAX`, resolution depth at 16, collection walk depth at 32). Missing: unification fuel, substitution depth, and trait-resolution recursion limits.

**Success Criteria:**
- [ ] Unification has a fuel counter that produces `E2042` (solver budget exceeded) — satisfies mission criterion "solver terminates predictably"
- [ ] Substitution has a depth limit that produces `E2043` (substitution depth exceeded) — satisfies mission criterion "solver terminates predictably"
- [ ] Trait resolution has a recursion limit with a dedicated diagnostic — satisfies mission criterion "solver terminates predictably"
- [ ] All existing tests pass unchanged (fuel limits high enough for real programs)
- [ ] At least 2 pathological test cases demonstrate the fuel limits fire correctly (TDD matrix)

**Context:** Research found these existing guards:
- `Rank(u16)` saturation in `ori_types/src/unify/rank/mod.rs` (131 lines) — prevents rank overflow
- `MAX_DEPTH: u32 = 16` in `ori_types/src/pool/accessors.rs:370` — resolution chain limit
- Collection surface walk bounded at depth 32 (`pool/collection_surface/mod.rs:36`)
- Triviality cycle detection via `FxHashSet<Idx>` (`triviality/mod.rs`, 203 lines)
- LLVM type resolution `MAX_RESOLVE_DEPTH: u32 = 32` + cycle detection (`layout_resolver.rs:104`)

Missing: no limit on total unification steps, no recursion depth limit on `substitute()` or `substitute_in_pool()`, no global type-checking budget per module.

**Reference implementations:**
- **TypeScript** `src/compiler/checker.ts:1509-1514`: tracks `totalInstantiationCount`, `instantiationCount`, and `instantiationDepth`; reports `Type instantiation is excessively deep and possibly infinite` as a structured diagnostic
- **Rust** `rustc_trait_selection`: configurable `#![recursion_limit = "N"]` (default 128) for trait resolution; overflow produces `E0275` with the chain displayed

**Depends on:** Section 01 (policy language).

---

## 03.1 Unification Fuel Counter

**File(s):** `compiler/ori_types/src/unify/mod.rs`, `compiler/ori_types/src/infer/mod.rs`

Add a fuel counter to the unification engine that tracks total unification steps per module check. This catches pathological constraint generation (e.g., exponential blowup from deeply nested generics) without affecting normal programs.

- [ ] Write failing test: `.ori` program with deeply nested generic instantiation that should trigger E2042
  - Matrix: recursive type aliases, deeply chained trait bounds, repeated instantiation
  - Negative pin: program that currently type-checks must still pass

- [ ] Add `unify_fuel: u32` field to `UnifyEngine` initialized to `DEFAULT_UNIFY_FUEL = 10_000`
  - Decrement on each `unify()` call
  - When fuel reaches 0, return `Err(TypeError::SolverBudgetExceeded)` with E2042
  - Note: the fuel counter tracks top-level `unify()` invocations, not recursive sub-calls — this makes the limit correspond to "number of constraints generated" rather than "depth of a single constraint resolution"

- [ ] Add `E2042` error code to `ori_types/src/type_error/check_error/` — "Type solver budget exceeded: the checker generated more than N constraints. This usually indicates recursive or deeply nested generic instantiation."

- [ ] Verify all existing tests pass with the default limit (if any test fails, the limit is too low — raise it, don't weaken tests)

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting 03.2:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 03.2 Substitution Depth Limit

**File(s):** `compiler/ori_types/src/pool/substitute/mod.rs`, `compiler/ori_types/src/unify/substitute.rs`

Add a recursion depth limit to `substitute_in_pool()` and `UnifyEngine::substitute()`. Both currently recurse unboundedly through the type tree.

- [ ] Write failing test: `.ori` program with recursive type alias that causes unbounded substitution depth

- [ ] Add `depth: u32` parameter (or a `SubstitutionContext` with a depth field) to `substitute_in_pool()` and propagate through recursive calls
  - Default limit: 256 (matches the generic `impl-hygiene.md:600` depth limit)
  - On overflow: return `Idx::ERROR` and accumulate `E2043` diagnostic
  - Apply the same limit to `UnifyEngine::substitute()` in `unify/substitute.rs`

- [ ] Add `E2043` error code — "Substitution depth exceeded: type substitution recursed more than N levels. This usually indicates a recursive type alias or deeply nested generic instantiation."

- [ ] Verify all existing tests pass (substitution depth of 256 should be far beyond any normal program)

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 03.3 Trait Resolution Recursion Limit

**File(s):** `compiler/ori_types/src/registry/traits/mod.rs`, `compiler/ori_types/src/check/registration/impls.rs`

Add a recursion limit for trait resolution — specifically for the supertrait chain traversal and impl-matching recursion that can diverge on circular trait hierarchies.

- [ ] Write failing test: `.ori` program with circular trait bounds or deeply chained supertraits

- [ ] Add resolution depth tracking to `TraitRegistry::lookup_method()` and bound-satisfaction checks
  - Default limit: 128 (matches Rust's default)
  - On overflow: produce a diagnostic (reuse E2042 with a "trait resolution" qualifier, or add a dedicated code)

- [ ] Verify all existing tests pass

- [ ] **Subsection close-out (03.3)** — MANDATORY before starting 03.4:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 03.4 Rule Documentation & Error Codes

**File(s):** `.claude/rules/typeck.md`, `.claude/rules/impl-hygiene.md`

Document the solver budget rules in the canonical rule files.

- [ ] Add rules to `typeck.md` — new section or addition to §3 Unification:
  - Every recursive solver loop SHALL carry explicit fuel/depth budget
  - Cycle detection is mandatory for trait resolution and type normalization
  - Overflow MUST produce structured diagnostics (E2042/E2043), not panics
  - Cross-reference: TypeScript instantiation tracking, Rust recursion_limit

- [ ] Update `impl-hygiene.md` §Panic & Assertion line 600 — replace the generic "recursion depth: 256" with a reference to the solver-specific limits defined in `typeck.md`, noting that solver recursion now has domain-specific limits with domain-specific error codes

- [ ] Register E2042 and E2043 in `typeck.md` §DI-1 Error Code Table

- [ ] **Subsection close-out (03.4)** — MANDATORY before completing section:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] All subsections (03.1-03.4) complete
- [ ] E2042 and E2043 error codes registered and tested
- [ ] At least 2 pathological test programs trigger the fuel limits (positive pins)
- [ ] All existing tests pass unchanged (negative pins — limits don't affect normal code)
- [ ] Debug AND release builds pass
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] **`/tpr-review`** — independent dual-source review clean
- [ ] **`/impl-hygiene-review`** — implementation hygiene clean
- [ ] **`/improve-tooling`** — section-close sweep
- [ ] **`/sync-claude`** — section-close doc sync
