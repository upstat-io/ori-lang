---
plan: "empty-container-typeck-phase-contract"
title: "Empty-Container Typeck Phase-Contract Enforcement: Exhaustive Implementation Plan"
status: in-progress
supersedes:
  - "plans/bug-tracker/fix-BUG-04-074.md"
references:
  - "plans/bug-tracker/fix-BUG-04-074.md"
  - "docs/ori_lang/v2026/spec/14-expressions.md"
  - ".claude/rules/typeck.md"
  - ".claude/rules/codegen-rules.md"
  - ".claude/rules/impl-hygiene.md"
  - ".claude/rules/canon.md"
---

# Empty-Container Typeck Phase-Contract Enforcement: Exhaustive Implementation Plan

## Mission

Close the empty-container typeck phase-contract enforcement gap: empty list literals without type context must be rejected at the type checker with E2005 (per `14-expressions.md:1224-1228`) rather than silently passing unresolved `Tag::Var`s to codegen where they surface as "unresolved type variable at codegen" LLVM verification failures. Deliver this as ONE coherent system: (a) AST-based Value Restriction at the 3 let-generalization sites so monomorphic local bindings no longer polymorphically leak element vars; (b) a new `ori_types::check::validators` module enforcing the typeck PC-2 output contract (no `Tag::Var` in body `expr_types`) as the producer-side error-bearing path; (c) defense-in-depth `debug_assert!`s at codegen consumer sites for the Cross-Phase Invariant Contract; (d) spec-aligned diagnostics and a test-audit sweep. Supersedes `plans/bug-tracker/fix-BUG-04-074.md`, which was escalated here after 5 rounds of dual-source Plan TPR revealed architectural depth exceeding bug-tracker fix-section scope.

## Mission Success Criteria

- [ ] The original repro `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` REJECTS at type check with `E2005: cannot infer type for empty list — add a type annotation like 'let ages: [int] = []'`, NOT at codegen. Verified by a Rust unit test in `compiler/ori_types/src/check/validators/tests.rs` asserting the diagnostic code and span.
- [ ] With an explicit annotation `let ages: [int] = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` compiles via `ori build` and runs with exit code 0 through both the JIT (`ori run`) and AOT (`ori build`) pipelines. Verified by AOT integration tests in `compiler/ori_llvm/tests/aot/`.
- [ ] Interpreter and LLVM produce identical observable results (dual-execution parity) for every test program in the matrix. Verified by `diagnostics/dual-exec-verify.sh`.
- [ ] Let-polymorphism for non-capturing lambdas is preserved: `let id = x -> x; id(1); id("hello")` continues to compile and run correctly (Rust unit test `test_let_polymorphism_for_lambda` in `compiler/ori_types/src/infer/expr/tests.rs`). Verified that reverting the Value Restriction change fails this test.
- [ ] No `unresolved type variable at codegen` error path in `compiler/ori_llvm/src/codegen/type_info/store.rs:341-363` fires on any program in `tests/spec/` (positive attestation via a diagnostic script `diagnostics/detect-tag-var-at-codegen.sh` or equivalent). If the assertion fires in a debug build, the producer-side validator has a gap — the failing ArcFunction name identifies the regression.
- [ ] No regressions in `timeout 150 ./test-all.sh`, `timeout 150 ./clippy-all.sh`, or `./llvm-test.sh`. Matrix tests across empty-list × element-type × usage-pattern all pass in debug and release builds.
- [ ] `/tpr-review` on the full plan diff returns clean across both codex and gemini with no actionable findings.
- [ ] `/impl-hygiene-review` runs clean after `/tpr-review`.
- [ ] All section success criteria met (see per-section frontmatter).
- [ ] Plan annotations removed from production code at close-out (`docs/ori_lang/v2026/spec/` references are permanent; `EMPTY-CONTAINER-CONTRACT` / `§NN.M` annotations are ephemeral).

## Architecture

```
Parser (ori_parse)
    │  AST with Tag::Infer for `let x = []`
    ▼
Type Checker (ori_types::check)  ── ENFORCEMENT PRODUCER
    │
    │  Bodies-pass per CK-1 (check_function / check_test / check_impl_method / check_def_impl_method)
    │       │
    │       │ (1) infer_block / infer_let / sequences.rs let-RHS typing
    │       │     ↓ Value Restriction (Section 01) — only non-capturing lambdas generalized
    │       │     ↓ Empty list's element Var stays Unbound/monomorphic
    │       │
    │       │ (2) AFTER body inference, BEFORE body exit:
    │       │     ↓ validate_body_types(pool, expr_types, sig, sig_span, span_of, errors)
    │       │       Section 02 — new `ori_types::check::validators` module
    │       │     ↓ Walks body_expr_types sorted by ExprIndex (deterministic)
    │       │     ↓ Skips types with HAS_ERROR (cascade suppression)
    │       │     ↓ HAS_VAR flag-based walk (no bound_vars set — BoundVar sets HAS_BOUND_VAR, not HAS_VAR)
    │       │     ↓ Emits E2005 via engine.push_error for each unresolved Tag::Var
    │       │
    │       └─ Integrated at 4 bodies-pass call sites (Section 03)
    │
    │  Typed IR with PC-2 guarantee (no Tag::Var in body expr_types)
    ▼
Canonicalizer (ori_canon) / ARC Pipeline (ori_arc) / AIMS / Realization
    │  Typed IR flows through canonicalization, ARC lowering, AIMS lattice
    ▼
LLVM Codegen (ori_llvm)  ── DEFENSE-IN-DEPTH CONSUMER
    │
    │  (3a) Pre-monomorphization input validation (Section 04a):
    │       ↓ debug_assert!(no Tag::Var in arc_func.var_types) for each cached ArcFunction
    │         BEFORE collect_mono_functions is called
    │         JIT site:  compiler/ori_llvm/src/evaluator/compile.rs:230
    │         AOT site:  compiler/oric/src/commands/codegen_pipeline.rs:112
    │
    │  (3b) Per-function validation (Section 04b):
    │       ↓ debug_assert!(no Tag::Var in arc_func.var_types) inside
    │         prepare_mono_cached / process_arc_function
    │
    │  Release builds surface ICE with the ArcFunction name (not TypeCheckError)
    │  per impl-hygiene.md §Cross-Phase Invariant Contracts
    ▼
LLVM IR + AOT binary (no unresolved Tag::Var path)
```

## Design Principles

1. **Enforce at the producer, assert at the consumer.** `typeck.md PC-2` declares "no `Tag::Var` in any type-bearing IR position." The producer (bodies-pass) owns the user-facing diagnostic (`E2005`); the consumer (codegen) owns defense-in-depth (`debug_assert!` + release ICE). This matches `impl-hygiene.md §Cross-Phase Invariant Contracts` — invariants crossing a phase boundary must be validatable at the consumer's entry point AND produced cleanly at the producer's exit point. Concrete bug: BUG-04-074's original failure mode was exactly this — the producer emitted unresolved `Tag::Var`, the consumer had no assertion, codegen surfaced a useless "unresolved type variable at codegen" error that blamed codegen for a typeck contract violation.

2. **Value Restriction over type-tag heuristics.** Gemini's Round 1 TPR finding (in the original fix-section) identified that `matches!(tag, Function | Scheme)` fails when the resolved type is still `Tag::Var` awaiting bi-directional unification (e.g., `let f = if cond then (x -> x) else (y -> y)`). AST-based Value Restriction — checking `ExprKind::Lambda` on the init's AST node — is the load-bearing detection mechanism. Concrete precedent: `blocks.rs:79-89` ALREADY uses AST-based Lambda detection for capturing closures; this plan extends the same pattern to the generalization decision.

3. **SSOT for the generalization policy.** The 3 let-generalization sites (`infer_block` L79-89, `infer_let` L167, `sequences.rs` L204-251) will all call a single `should_generalize(arena, init_expr_id) -> bool` helper. `impl-hygiene.md §Algorithmic DRY` threshold ("same fix at 3+ callsites = missing abstraction") applies directly.

4. **Narrow to lists per spec scope.** Spec `14-expressions.md:1224-1228` only mandates `[]` as a compile-time error. `{}` and `Set<T>()` are spec-neutral for this plan. The validator is type-agnostic (it walks any `Tag::Var`) so it naturally catches them, but test coverage and E2005 wording target lists only to avoid scope creep.

## Section Dependency Graph

```
Section 01 (Value Restriction)
     │
     ├─► Section 06 (Diagnostics + Audit) — diagnostic wording depends on when E2005 fires
     │
Section 02 (Validator Module)
     │
     └─► Section 03 (Bodies-Pass Integration) — needs validator module to exist
               │
               ├─► Section 04 (Codegen Assertions) — depends on producer side being clean
               │
               └─► Section 06 (Diagnostics + Audit) — audit runs after producer works

Section 04 depends on Section 03 (clean producer output).
Section 05 (Test Matrix) is written FIRST (TDD) but verified LAST.
Section 07 (Close-out) requires all above.
```

- **Sections 01 and 02 are independent** and can be drafted in parallel (different code paths: generalization policy vs. validator module).
- **Section 03 depends on Sections 01 + 02** — it wires the validator into the 4 bodies-pass sites AFTER the generalization policy no longer bleeds unresolved vars for lambda-typed bindings.
- **Section 04 depends on Section 03** — defense-in-depth `debug_assert!`s at codegen rely on the producer being clean; enabling them before Section 03 would trigger on legitimate typed IR.
- **Section 06 depends on Sections 03 + 04** — the diagnostic audit (sweeping `tests/spec/` for `[].iter()` / `[].len()` patterns beyond just `let x = []` bindings per `TPR-04-005-codex`) runs after enforcement is live so we can detect what needs annotation.

**Cross-section interactions (must be co-implemented):**
- **Section 01 + Section 05**: The TDD matrix MUST include the let-polymorphism semantic pin (`test_let_polymorphism_for_lambda`) that only passes when Section 01's Value Restriction correctly identifies `ExprKind::Lambda` as the ONLY generalizable init. Missing this pin = silent regression of the `let id = x -> x` pattern.
- **Section 02 + Section 03**: The validator's error-API shape (`&mut dyn FnMut(TypeCheckError)` vs `Vec<TypeCheckError>` return) is the contract between the module definition and its 4 integration sites. Gemini Round-5 finding `[TPR-04-002-gemini]` recommended `Vec` return over closure. Plan adopts `Vec` per the existing `engine.take_errors()` idiom at `infer/context.rs:64`.

## Implementation Sequence

```
Phase 0 — TDD Stubs (Section 05 first wave)
  └─ 05: Write failing matrix tests + semantic pins + negative pins as STUBS
     Gate: Every test compiles but fails with the current codegen error (not the target E2005)

Phase 1 — Generalization Policy (Section 01)
  └─ 01.1: Extract `should_generalize(arena, init)` SSOT helper
  └─ 01.2: Migrate infer_block L79-89 call site
  └─ 01.3: Migrate infer_let L167 call site
  └─ 01.4: Migrate sequences.rs L204-251 call site
     Gate: `test_let_polymorphism_for_lambda` passes; empty-list binding no longer generalizes

Phase 2 — Validator Module (Section 02)
  └─ 02.0: Pool scheme-flag propagation fix (prerequisite for §02.2 HAS_VAR gate)
  └─ 02.1: Validator signature, public contract, and narrow re-export (NO pub mod check)
  └─ 02.2: Core algorithm: tag-dispatch child recursion (reusing Pool::visit_children)
  └─ 02.3: lib.rs and check/mod.rs wiring (no pub mod check)
  └─ 02.4: Unit test matrix (twelve cells)
     Gate: Unit tests pass; validator emits E2005 for every unresolved Tag::Var

Phase 3 — Bodies-Pass Integration (Section 03) — CRITICAL PATH
  └─ 03.1: Call validator from check_function after body inference
  └─ 03.2: Call validator from check_test
  └─ 03.3: Call validator from check_impl_method
  └─ 03.4: Call validator from check_def_impl_method
     Gate: Original BUG-04-074 repro now emits E2005 at typeck; all spec tests still compile OR are annotated

Phase 4 — Codegen Defense-in-Depth (Section 04)
  └─ 04.1: debug_assert! hook in prepare_mono_cached (per-function seam)
  └─ 04.2: debug_assert! hook at compile.rs:230 (JIT pre-mono)
  └─ 04.3: debug_assert! hook at codegen_pipeline.rs:112 (AOT pre-mono)
  └─ 04.4: Release-build ICE path with ArcFunction name in panic message
     Gate: If ANY Tag::Var reaches codegen in a debug build, the assertion fires with actionable output

Phase 5 — Diagnostics + Test Audit (Section 06)
  └─ 06.1: Refine E2005 message wording + suggestion format
  └─ 06.2: Audit tests/spec/ for [].iter(), [].len(), [].is_empty() patterns — annotate or document
  └─ 06.3: Verify spec test corpus compiles clean
     Gate: `cargo st` green; annotations documented in spec-test README where material

Phase 6 — Close-out (Section 07)
  └─ 07.1: Supersede bug-tracker entry for BUG-04-074
  └─ 07.2: Remove plan annotations from production code
  └─ 07.3: `/tpr-review` → `/impl-hygiene-review` → `/improve-tooling` sweep → `/sync-claude`
     Gate: All reviews clean; plan status: complete
```

**Why this order:**
- Phase 0 is TDD discipline — tests frame the implementation per `CLAUDE.md §TDD for Bugs`. Tests fail with today's codegen error; they'll transition to passing with Phase 3's E2005 emission.
- Phase 1 (Section 01) must precede Phase 2 (Section 02) because if Section 02 runs first, the validator would reject legitimate lambda-polymorphism (`let id = x -> x`) — the generalization policy must land first so that only non-lambda initializers have Unbound element vars to catch.
- Phase 3 is the critical path — it's the moment E2005 starts firing on user programs. Phase 4 (debug_assert!) depends on Phase 3 being clean, otherwise the assertions fire on legitimately-typed IR.
- Phase 5 runs AFTER enforcement is live so the audit detects real annotation needs.

**Known failing tests (expected until plan completion):**

- **`test_empty_list_let_binding_emits_e2005`** (in `check/validators/tests.rs`) — fails in Phase 0 (no validator exists); passes in Phase 2/3. Root cause: validator module doesn't exist yet. Do NOT investigate as a separate bug — this is the target behavior.
- **`test_empty_list_with_push_and_len_compiles_with_annotation`** (AOT test in `ori_llvm/tests/aot/`) — fails in Phase 0 (codegen-time error); passes in Phase 3+ (clean typeck → clean codegen). Root cause: Tag::Var reaches codegen. Resolves when Section 03 lands.
- **`test_let_polymorphism_for_lambda`** (in `compiler/ori_types/src/infer/expr/tests.rs`) — PASSES in Phase 0 (current behavior); may transiently FAIL during Section 01 implementation if `should_generalize` is incorrectly narrowed. Semantic pin for Section 01.

Do NOT attempt to fix these tests individually until the section that owns them is active.

## Metrics (Current State)

Baseline measurements (pre-implementation) from the codebase as of plan creation. These are used to measure delta at close-out.

| Crate | Production LOC | Test LOC | Total | Notes |
|-------|---------------|----------|-------|-------|
| `ori_types/src/check` | ~3,500 | ~1,200 | ~4,700 | Bodies-pass home; new `check/validators/` submodule lands here |
| `ori_types/src/infer/expr/blocks.rs` | ~250 | ~400 | ~650 | Value Restriction migration site |
| `ori_types/src/infer/expr/sequences.rs` | ~350 | ~500 | ~850 | Third generalization site |
| `ori_types/src/infer/expr/mod.rs` | ~270 | ~100 | ~370 | `infer_let` dispatch |
| `ori_llvm/src/evaluator/compile.rs` | ~450 | — | ~450 | JIT pre-mono hook |
| `ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs` | ~200 | ~80 | ~280 | Per-function seam |
| `oric/src/commands/codegen_pipeline.rs` | ~500 | ~100 | ~600 | AOT pre-mono hook |
| `tests/spec/types/collections.ori` | — | ~150 | ~150 | Matrix tests land here |
| **Total affected** | **~5,500** | **~2,500** | **~8,100** | |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Value Restriction | ~120 (40 helper + 3×20 migrations + 20 tests) | Medium | — |
|   ↳ 01.1 `should_generalize` helper | ~40 | Low | — |
|   ↳ 01.2–01.4 3 migration sites | ~60 | Medium | 01.1 |
| 02 Validator Module | ~280 (10 pool fix + 150 module + 120 tests) | Medium | — |
|   ↳ 02.0 Pool scheme-flag propagation fix | ~10 + ~30 tests | Low | — |
|   ↳ 02.1 Validator signature + narrow re-export | ~60 | Low | 02.0 |
|   ↳ 02.2 Core algorithm (reusing Pool::visit_children) | ~80 | Medium | 02.1 |
|   ↳ 02.3 lib.rs + check/mod.rs wiring | ~10 | Low | 02.1 |
|   ↳ 02.4 Unit test matrix (12 cells) | ~120 | Low | 02.2 |
| 03 Bodies-Pass Integration | ~80 (4×15 integration + 20 tests) | Low | 01, 02 |
|   ↳ 03.1–03.4 4 integration sites | ~60 | Low | 02 |
|   ↳ 03.5 End-to-end tests | ~20 | Low | 03.1–03.4 |
| 04 Codegen Assertions | ~60 (3×15 assertions + 15 ICE path + 15 tests) | Low | 03 |
|   ↳ 04.1–04.3 3 assertion sites | ~45 | Low | 03 |
|   ↳ 04.4 Release ICE path | ~15 | Low | 04.1–04.3 |
| 05 Test Matrix | ~200 (matrix + pins + cross-backend) | Medium | — (written first, TDD) |
| 06 Diagnostics + Audit | ~80 (30 message + 50 audit/annotations) | Low | 03 |
| 07 Close-out | ~40 (annotation removal + bug-tracker update) | Low | All |
| **Total new** | **~830** | | |
| **Total deleted** | **~20** (plan annotations stripped at close) | | |

## Known Bugs (Pre-existing)

Bugs surfaced during the BUG-04-074 investigation + 5 rounds of Plan TPR + Round 5 research verification. These feed directly into the sections below.

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| BUG-04-074: Empty list literal with `push()` leaves unresolved Tag::Var, causing LLVM verification failure at AOT | Unconditional generalization at 3 let-binding sites (infer_block L85/L88, infer_let L167, sequences L247) creates Generalized element vars that later instantiation at `.len()`-style non-constraining use sites doesn't resolve; codegen has no assertion, surfaces useless error | Sections 01 + 02 + 03 | Escalated to this plan |
| `ori_types::check` module exposure: new validator needs external access | `mod check;` (private) at `lib.rs:16`; fix: narrow re-export `pub use check::validators::validate_body_types;` — NO `pub mod check` promotion (Phase 2 /tp-help consensus) | Section 02.1 + 02.3 | Not Started |
| Spec violation: `14-expressions.md:1224-1228` declares `let y = []` a compile-time error; compiler silently passes to codegen | typeck.md PC-2 output contract not enforced at phase boundary | Sections 02 + 03 | Not Started |
| `TPR-04-005-codex` audit finding: `tests/spec/` uses `[].iter()`, `[].is_empty()` patterns beyond just `let x = []` bindings; these WILL trip E2005 once live | Existing spec test corpus not spec-compliant | Section 06.2 | Not Started |
| Informational: Round 4 supersession markers overclaim in the original fix-section — round-1 §R entries do not carry `(SUPERSEDED)` suffixes despite item 7 claiming they do | Documentation drift in `plans/bug-tracker/fix-BUG-04-074.md` | Section 07.1 — supersede the fix-section and preserve the TPR audit trail as a reference in `references:` frontmatter | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | AST-based Value Restriction | `section-01-value-restriction.md` | Complete |
| 02 | Validator Module (`ori_types::check::validators`) | `section-02-validator-module.md` | Not Started |
| 03 | Bodies-Pass Integration | `section-03-bodies-pass-integration.md` | Not Started |
| 04 | Codegen Defense-in-Depth Assertions | `section-04-codegen-assertions.md` | Not Started |
| 05 | Test Matrix + Semantic Pins | `section-05-test-matrix.md` | Not Started |
| 06 | Diagnostics + Spec-Test Audit | `section-06-diagnostics-audit.md` | Not Started |
| 07 | Close-out + Supersession | `section-07-closeout.md` | Not Started |
