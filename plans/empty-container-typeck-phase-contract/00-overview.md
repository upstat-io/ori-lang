---
plan: "empty-container-typeck-phase-contract"
title: "Empty-Container Typeck Phase-Contract Enforcement: Exhaustive Implementation Plan"
status: in-progress
supersedes:
  - "plans/bug-tracker/fix-BUG-04-074.md"
references:
  - "plans/bug-tracker/fix-BUG-04-074.md"
  - "plans/bug-tracker/fix-BUG-04-084.md"
  - "docs/ori_lang/v2026/spec/14-expressions.md"
  - ".claude/rules/typeck.md"
  - ".claude/rules/codegen-rules.md"
  - ".claude/rules/impl-hygiene.md"
  - ".claude/rules/canon.md"
---

# Empty-Container Typeck Phase-Contract Enforcement: Exhaustive Implementation Plan

## Mission

Close the empty-container typeck phase-contract enforcement gap end-to-end: empty list literals without type context must be rejected at the type checker with E2005 (per `14-expressions.md:1224-1228`) rather than silently passing unresolved `Tag::Var`s to codegen where they surface as "unresolved type variable at codegen" LLVM verification failures. Deliver this as ONE coherent system: (a) AST-based Value Restriction at the 3 let-generalization sites so monomorphic local bindings no longer polymorphically leak element vars; (b) a new `ori_types::check::validators` module enforcing the typeck PC-2 output contract (no `Tag::Var` in body `expr_types`) as the producer-side error-bearing path; (c) end-of-body defaulting pre-pass (`default_unbound_vars_from_empty_literals`) so legitimate unconstrained empty literals default to `Idx::NEVER` instead of spuriously firing E2005; (d) defense-in-depth `debug_assert!`s at codegen consumer sites for the Cross-Phase Invariant Contract; (e) resolve the poly-lambda `BoundVar` bleed (BUG-04-042) that currently blocks `test-all.sh` on monomorphized `assert_eq` for imported generics, because the plan cannot land clean commits without it; (f) investigate and resolve the `Tag::Scheme` `PROPAGATE_MASK` regression (BUG-04-085) that may have been caused by §02.0's fix and is surfacing as the LLVM-backend crash in the spec runner; (g) spec-aligned diagnostics and a test-audit sweep.

**Scope absorption (2026-04-17)**: this plan previously deferred two commit-pipeline blockers (BUG-04-042, BUG-04-085) as sibling bug-tracker entries. Per CLAUDE.md §Ownership & Deferral "Plan-blocker bugs belong IN the plan — NEVER sibling fix files", both are now absorbed into the plan's scope (Sections 08 and 07 respectively). The anti-pattern being corrected: when plan completion depends on a bug fix, creating a parallel `fix-BUG-XX-NNN.md` artifact produces a chain — plan → fix-A → fix-B → fix-C — where each link has its own blockers and TPR cycle, and nothing ever completes. The cure is to let the plan own resolving its own blockers directly.

Supersedes `plans/bug-tracker/fix-BUG-04-074.md` (original escalation); absorbs `fix-BUG-04-084.md` (work complete, pending commit); absorbs BUG-04-042 and BUG-04-085 (bug-tracker entries, no fix files created because the plan now owns them).

## Mission Success Criteria

- [ ] The original repro `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` REJECTS at type check with `E2005: cannot infer type for empty list — add a type annotation like 'let ages: [int] = []'`, NOT at codegen. Verified by a Rust unit test in `compiler/ori_types/src/check/validators/tests.rs` asserting the diagnostic code and span.
- [ ] With an explicit annotation `let ages: [int] = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` compiles via `ori build` and runs with exit code 0 through both the JIT (`ori run`) and AOT (`ori build`) pipelines. Verified by AOT integration tests in `compiler/ori_llvm/tests/aot/`.
- [ ] Interpreter and LLVM produce identical observable results (dual-execution parity) for every test program in the matrix. Verified by `diagnostics/dual-exec-verify.sh`.
- [ ] Let-polymorphism for non-capturing lambdas is preserved: `let id = x -> x; id(1); id("hello")` continues to compile and run correctly (Rust unit test `test_let_polymorphism_for_lambda` in `compiler/ori_types/src/infer/expr/tests.rs`). Verified that reverting the Value Restriction change fails this test.
- [ ] Empty-collection defaulting pre-pass (§03.BUG-FIXES): `for x in [1, 2, 3] do {};` compiles clean (both as statement and expression body positions); `let empty = []; empty.len()` compiles clean with `empty` typed as `[Never]`. Verified by the 18 positive spec tests + 3 negative pins in `tests/spec/types/empty_literals/`. No legitimate empty-literal program fires E2005.
- [ ] No `unresolved type variable at codegen` error path in `compiler/ori_llvm/src/codegen/type_info/store.rs:341-363` fires on any program in `tests/spec/` (positive attestation via a diagnostic script `diagnostics/detect-tag-var-at-codegen.sh` or equivalent). If the assertion fires in a debug build, the producer-side validator has a gap — the failing ArcFunction name identifies the regression.
- [ ] **Cross-module pool-merge var_id collision resolved (Section 08, absorbs BUG-04-042)**: `timeout 150 cargo run --bin ori -- test --backend=llvm tests/spec/expressions/lambda_mono.ori` runs clean (previously produced `Idx(241)` unresolved type variable + 17 LCFails). The test-runner pool-merge path at `compiler/oric/src/test/runner/llvm_backend.rs:167-251` preserves var_id disjointness between the host test-file pool and every imported module's pool, via a remap-aware re-intern abstraction lifted into `compiler/ori_types/src/pool/re_intern/`; `assert_eq<T: Eq + Debug>` monomorphizes correctly for imported generics regardless of whether the host module contains polymorphic lambda definitions. Verified by the spec test pass (JIT backend) and the §08.2 `pool/re_intern/tests.rs` matrix cells e1–e5 pinning leaf-var remap, scheme-binder remap, `FunctionSig.scheme_var_ids` coherence, `Tag::Scheme` binder list remap, and `VarState` clone-vs-blank-init semantics (each with positive + negative pairing). **AOT scope (2026-04-19 TPR-08-R4-01 resolution, Round 5)**: §08 is JIT-only — `ori build` does NOT share the `re_intern_*` code path. The AOT integration tests in `compiler/ori_llvm/tests/aot/poly_lambda_mono.rs` stay in-tree as failing TDD pins for the sibling bug BUG-04-AOT-MONO (AOT `collect_mono_functions` lacks imported generic monomorphization); they will green when that sibling bug lands, not as part of §08.
- [ ] **LLVM spec runner crash resolved (Section 07 close-out, absorbs BUG-04-085)**: `timeout 150 ./test-all.sh` reports no "Ori spec (LLVM backend) CRASHED" line; `assert_eq$m$int` no longer triggers `ArcIrEmitter: variable not yet defined` or the ensuing stack overflow. **§07.0 retests BUG-04-085 immediately after §08.3 lands** (the cross-module pool-merge var_id collision fix) — under the corrected 2026-04-19 diagnosis, BUG-04-085 is plausibly the same defect family as BUG-04-042 (corrupted imported `body_type_map` surfacing downstream in ArcIrEmitter). If §08.3 closes BUG-04-085, §07.0 closes it as same-family with a pointer to §08. If §08.3 does NOT close it, §07.0 reopens with a new root-cause hypothesis (NOT "§02.0's `Tag::Scheme PROPAGATE_MASK` regression" — that hypothesis is disconfirmed per codex Step 4 advice). Verified by a green LLVM-backend spec run.
- [ ] No regressions in `timeout 150 ./test-all.sh`, `timeout 150 ./clippy-all.sh`, or `./llvm-test.sh`. Matrix tests across empty-list × element-type × usage-pattern all pass in debug and release builds. **Zero failing tests from this plan's domain at close-out** — every spec test either passes or carries a concrete `#skip(...)` / `#compile_fail(...)` annotation with a pointer to a separate non-blocker bug.
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
    │  PRIMARY SEAM (Section 04.2, load-bearing):
    │       ↓ assert_no_unresolved_type_vars(pool, arc_func, interner, exempt)
    │         at the SINGLE upstream choke point — both paths below converge here.
    │       ↓ Per-function hook:
    │         compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:315
    │         (process_arc_function) — BEFORE ori_arc::run_arc_pipeline
    │       ↓ Per-lambda hook:
    │         compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:375
    │         (declare_and_process_lambda) — BEFORE its own run_arc_pipeline
    │
    │  SECONDARY pre-mono sites (Section 04.3, diagnostic localization only):
    │       ↓ JIT site:  compiler/ori_llvm/src/evaluator/compile.rs:230
    │       ↓ AOT site:  compiler/oric/src/commands/codegen_pipeline.rs:95,112
    │       (non-load-bearing — primary seam catches the violation regardless;
    │        secondary sites attribute to pre-mono input for better diagnostic)
    │
    │  Both debug AND release surface a typed VerifyError::UnresolvedTypeVar
    │  via the existing ori_arc::verify plumbing — NO debug_assert! fail-open.
    │  Integrates with ORI_VERIFY_ARC=1 layering per codegen-rules.md §VR-1
    │  (the assertion is ALWAYS-ON; verify_arc gates ADDITIONAL verification).
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
Section 01 (Value Restriction)         Section 02 (Validator Module)
     │                                       │
     └───────────────┬───────────────────────┘
                     │
                     ▼
               Section 03 (Bodies-Pass Integration)
                 + §03.BUG-FIXES (empty-literal defaulting — BUG-04-084)
                     │
        ┌────────────┼────────────────────────┐
        │            │                        │
        ▼            ▼                        ▼
  Section 04    Section 08                Section 06
  (Codegen      (Poly-Lambda              (Diagnostics
   Assertions)   BoundVar bleed —           + Audit)
                 BUG-04-042)
        │            │                        │
        └────────────┼────────────────────────┘
                     │
                     ▼
               Section 07 (Close-out)
                 + BUG-04-085 investigation
                   (did §02.0 PROPAGATE_MASK cause it?)

Section 05 (Test Matrix) is written FIRST (TDD) but verified LAST.
```

- **Sections 01 and 02 are independent** and can be drafted in parallel (different code paths: generalization policy vs. validator module).
- **Section 03 depends on Sections 01 + 02** — wires the validator into the 4 bodies-pass sites AFTER the generalization policy no longer bleeds unresolved vars for lambda-typed bindings. §03.BUG-FIXES (BUG-04-084) added the end-of-body defaulting pre-pass so legitimate empty literals default to `[Never]` before the validator runs.
- **Section 04 depends on Section 03** — defense-in-depth `debug_assert!`s at codegen rely on the producer being clean; enabling them before Section 03 would trigger on legitimate typed IR.
- **Section 08 depends on Section 03** (for the same clean-producer invariant) but is otherwise independent — it fixes a different codegen bug (poly-lambda `BoundVar` bleed into the mono pipeline). Section 08 **must complete before `test-all.sh` passes**, which means **plan commits are blocked until Section 08 lands**. This is the new reality after absorbing BUG-04-042.
- **Section 06 depends on Sections 03 + 04** — the diagnostic audit runs after enforcement is live so we can detect what needs annotation.
- **Section 07 depends on all above** — in addition to the original close-out work, §07 now owns investigating BUG-04-085 (LLVM spec runner crash) and either fixing the consumer that depended on the old `Tag::Scheme` flag semantics or adjusting §02.0's propagation for compatibility.

**Cross-section interactions (must be co-implemented):**
- **Section 01 + Section 05**: The TDD matrix MUST include the let-polymorphism semantic pin (`test_let_polymorphism_for_lambda`) that only passes when Section 01's Value Restriction correctly identifies `ExprKind::Lambda` as the ONLY generalizable init. Missing this pin = silent regression of the `let id = x -> x` pattern.
- **Section 02 + Section 03**: The validator's error-API shape (`&mut dyn FnMut(TypeCheckError)` vs `Vec<TypeCheckError>` return) is the contract between the module definition and its 4 integration sites. Gemini Round-5 finding `[TPR-04-002-gemini]` recommended `Vec` return over closure. Plan adopts `Vec` per the existing `engine.take_errors()` idiom at `infer/context.rs:64`.

## Implementation Sequence

```
Phase 0 — TDD Stubs (Section 05 first wave)
  └─ 05: Write failing matrix tests + semantic pins + negative pins as STUBS

Phase 1 — Generalization Policy (Section 01) — COMPLETE
  └─ 01.1–01.4: should_generalize helper + 3 migration sites — COMPLETE
  └─ 01.R-HYGIENE: body_captures_outer soundness (F1+F7 from §03.N hygiene sweep)
  └─ 01.R-DRY: InferEngine constructor + maybe_generalize DRY (F4+F5)
  └─ 01.R-SIDE-LOGIC: dispatch-module side logic (F2+F3)
  └─ 01.R-TEST-HYGIENE: test helper DRY + naming + import hygiene (F6+F8+F13+F14)

Phase 2 — Validator Module (Section 02) — COMPLETE
  └─ 02.0: Pool scheme-flag propagation fix (prerequisite for §02.2 HAS_VAR gate)
           ⚠ Investigate in §07: possible cause of BUG-04-085 LLVM runner crash
  └─ 02.1–02.4: validator module + unit test matrix

Phase 3 — Bodies-Pass Integration (Section 03) — COMPLETE
  └─ 03.0: Split bodies/mod.rs (BLOAT gate) — COMPLETE
  └─ 03.1: Wire validator into check_function — COMPLETE
  └─ 03.2: Wire validator into check_test — COMPLETE
  └─ 03.BUG-FIXES: Absorbed BUG-04-084 (end-of-body empty-literal defaulting) — COMPLETE
  └─ 03.3: Wire validator into check_impl_method — COMPLETE
  └─ 03.4: Wire validator into check_def_impl_method — COMPLETE
  └─ 03.5: End-to-end regression suite and dual-execution parity — COMPLETE
  └─ 03.R: Third Party Review Findings (Rounds 0, 1, 2) — COMPLETE
  └─ 03.N: Completion Checklist — COMPLETE

Phase 4 — Codegen Defense-in-Depth (Section 04, restructured post /review-plan 2026-04-18)
  └─ 04.1: New ori_arc::ir::validate module + typed UnresolvedTypeVar error
  └─ 04.2: PRIMARY seam hooks — process_arc_function + declare_and_process_lambda
           at define_phase.rs:315 and define_phase.rs:375 (load-bearing)
  └─ 04.TPR-A: TPR checkpoint after primary seam + module
  └─ 04.3: SECONDARY pre-mono sites — JIT compile.rs:230 + AOT codegen_pipeline.rs:95,112
           (diagnostic localization only; non-load-bearing; may be dropped per reviewer caveat)
  └─ 04.4: Unit tests — 9-cell matrix (resolved, unbound, exempt, BoundVar, Projection,
           lambda-capture, first-violator-deterministic)

Phase 5 — Poly-Lambda Mono Fix (Section 08, NEW — absorbs BUG-04-042) — COMMIT BLOCKER
  └─ 08.1: Investigation — reproduce lambda_mono.ori failure; diagnosis CORRECTED 2026-04-19
           (cross-module pool-merge var_id collision at test-runner pool-merge boundary)
  └─ 08.1.5: Decide fix shape — remap-aware re-intern in pool/re_intern/ (reset to not-started
             2026-04-19; prior "sibling pass" decision reverted at HEAD=3dd4ded6)
  └─ 08.2: TDD matrix — spec tests + AOT tests + pool-crate re_intern tests covering
           poly-lambda + imported generic + var_id remap semantics (4 new cells added 2026-04-19)
  └─ 08.3: Implementation — lift remap-aware re-intern into pool/re_intern/ with
           test-runner llvm_backend.rs:320-360 as first consumer; 7 steps covering leaf
           remap, scheme binder remap, scheme_var_ids coherence, VarState clone
  └─ 08.4: Coordination with roadmap §21A if still active in that area
  └─ 08.5: Verification — lambda_mono.ori + integer_safety.ori both pass via --backend=llvm
  └─ 08.6: Confirm §04.2 seam order is correct under corrected fix (no rework needed)
  └─ 08.R: TPR review
  └─ 08.N: Completion checklist
     Gate: test-all.sh is GREEN on Ori spec (LLVM backend); commits unblock

Phase 6 — Diagnostics + Test Audit (Section 06)
  └─ 06.1: Refine E2005 message wording + suggestion format
  └─ 06.2: Audit tests/spec/ for [].iter(), [].len(), [].is_empty() patterns
  └─ 06.3: Verify spec test corpus compiles clean

Phase 7 — Close-out + BUG-04-085 Investigation (Section 07, expanded)
  └─ 07.0: Investigate BUG-04-085 — isolate whether §02.0's PROPAGATE_MASK fix caused
           "ArcIrEmitter: variable not yet defined" in the mono pipeline. If YES, fix
           the consumer that depended on old flag semantics. If NO, file a new bug
           scoped to the true root cause.
  └─ 07.1: Close bug-tracker entries for BUG-04-074, BUG-04-084, BUG-04-042, BUG-04-085
           with pointers to their owning plan sections (no sibling fix files)
  └─ 07.2: Remove plan annotations from production code
  └─ 07.3: /tpr-review → /impl-hygiene-review → /improve-tooling sweep → /sync-claude
     Gate: All reviews clean; plan status: complete; test-all.sh fully green
```

**Why this order:**
- Phase 0 is TDD discipline — tests frame the implementation per `CLAUDE.md §TDD for Bugs`.
- Phase 1 (Section 01) precedes Phase 2 (Section 02) because if Section 02 runs first, the validator would reject legitimate lambda-polymorphism — the generalization policy must land first.
- Phase 3 is the critical typeck-side path — it's where E2005 starts firing on user programs. Phase 4 (debug_assert!) depends on Phase 3 being clean.
- **Phase 5 (Section 08) is the new commit-unblocker** — absorbed from BUG-04-042. The plan cannot land atomic commits until this section completes, because `test-all.sh` fails on the LLVM backend spec run due to the poly-lambda `BoundVar` bleed. §08 is an independent codegen fix that runs in parallel with §04/§06 but must finish before any commit attempt for sections landing after §03.
- Phase 6 runs AFTER enforcement is live so the audit detects real annotation needs.
- Phase 7 (Section 07) adds the BUG-04-085 investigation: §02.0's `Tag::Scheme PROPAGATE_MASK` fix may be the root cause of the `ArcIrEmitter: variable not yet defined` crash in the mono pipeline. Either fix the downstream consumer or adjust the propagation.

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

## Known Bugs (Absorbed into This Plan)

Bugs absorbed into the plan's scope per CLAUDE.md §Ownership & Deferral "Plan-blocker bugs belong IN the plan". Each was a blocker to plan completion and is now owned by a specific plan section rather than a sibling `fix-BUG-XX-NNN.md` file. The bug-tracker entries for these are closed with pointers back to their owning section.

| Bug | Root Cause | Owning Section | Status |
|-----|-----------|---------------|--------|
| BUG-04-074: Empty list literal with `push()` leaves unresolved Tag::Var, causing LLVM verification failure at AOT | Unconditional generalization at 3 let-binding sites creates Generalized element vars that non-constraining use sites (`.len()`) don't resolve | Sections 01 + 02 + 03 | Section 01 Complete, Section 02 Complete, Section 03 In-Progress (§03.1 + §03.2 done) |
| BUG-04-084: `for x in coll do {}` body causes unresolved `Tag::Var` at codegen; all four body-group passes needed defaulting before validator fires | Empty-collection literals at body-exit positions had no constraint channel, so unconstrained element vars survived inference and spuriously tripped E2005 | Section 03.BUG-FIXES (end-of-body defaulting pre-pass) | **Implementation Complete 2026-04-17** — defaulting wired in all 4 body-group passes; 18 spec tests + 3 negative pins pass; bug-tracker entry closed |
| BUG-04-042: LLVM codegen — polymorphic lambda presence causes unresolved type variable for imported generics (`assert_eq`) | **Cross-module pool-merge var_id collision** in the test runner at `compiler/oric/src/test/runner/llvm_backend.rs:320-360` — imported-module types re-interned at `compiler/ori_types/src/pool/re_intern/mod.rs:185-193` preserve source var_ids unchanged, while the merged pool's `var_states` vector was cloned from the test-file pool; `ensure_var_capacity(max_id + 1)` at `llvm_backend.rs:341-343` only APPENDS fresh `Unbound` slots and never SHIFTS imported var_ids, so an imported `Tag::Var(var_id = N)` reads slot N of `merged_pool.var_states` which may hold a host-file poly-lambda's `VarState::Generalized` state. The observed `Idx(241)` error is the downstream surface of this aliasing, NOT a typeck producer leak (the prior "polymorphic-lambda BoundVar types in the shared Pool" hypothesis was superseded 2026-04-19; sibling-pass attempts to default poly-lambda returns in typeck were tried and reverted at HEAD=3dd4ded6). | **Section 08 (new)** — absorbed 2026-04-17 from bug-tracker; diagnosis corrected 2026-04-19 per `/tp-help` consensus + codex Step 4 blind-spots | Not Started — §08.3 (remap-aware re-intern in `pool/re_intern/`) is the fix; **blocks plan commits** until resolved |
| BUG-04-085: LLVM spec runner crashes with stack overflow + `ArcIrEmitter: variable not yet defined` on monomorphized `assert_eq` | **Plausibly same defect family as BUG-04-042** — corrupted imported `body_type_map` produced by the cross-module pool-merge var_id collision surfaces downstream in ArcIrEmitter at `compiler/ori_arc/src/ir_emitter/emitter_utils.rs:182-194` as "variable not yet defined". The prior hypothesis ("regression from §02.0's `Tag::Scheme` `PROPAGATE_MASK` fix") is DISCONFIRMED per codex Step 4 advice — the PROPAGATE_MASK fix operates on typeck's output pool, not on the test-runner's merged pool where the real collision lives. **§07.0 retests BUG-04-085 immediately after §08.3 lands**: if it disappears, close as same-family with pointer to §08; if it remains, reopen §07 with a NEW root cause (not Tag::Scheme PROPAGATE_MASK). | **Section 07 (expanded)** — absorbed 2026-04-17 into close-out scope; hypothesis corrected 2026-04-19 per §08.1.R cross-cutting implications | Not Started — retest after §08.3 lands; open §07.0 may close trivially under the same-family hypothesis, or reopen with a new scope under a different root cause |
| `ori_types::check` module exposure: new validator needs external access | `mod check;` (private); fix: narrow re-export `pub use check::validators::validate_body_types;` | Section 02.1 + 02.3 | Complete (2026-04-15) |
| Spec violation: `14-expressions.md:1224-1228` declares `let y = []` a compile-time error; compiler silently passes to codegen | typeck.md PC-2 output contract not enforced at phase boundary | Sections 02 + 03 | Section 02 Complete; Section 03 In-Progress |
| `TPR-04-005-codex` audit finding: `tests/spec/` uses `[].iter()`, `[].is_empty()` patterns beyond just `let x = []` bindings | Existing spec test corpus not spec-compliant | Section 06.2 | Not Started |
| Informational: Round 4 supersession markers overclaim in the original fix-section | Documentation drift in `plans/bug-tracker/fix-BUG-04-074.md` | Section 07.1 — close the bug-tracker entry and preserve TPR audit trail in `references:` | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | AST-based Value Restriction | `section-01-value-restriction.md` | Complete |
| 02 | Validator Module (`ori_types::check::validators`) | `section-02-validator-module.md` | Complete |
| 03 | Bodies-Pass Integration (absorbs BUG-04-074, BUG-04-084) | `section-03-bodies-pass-integration.md` | Complete |
| 04 | Codegen Defense-in-Depth Assertions | `section-04-codegen-assertions.md` | Not Started |
| 05 | Test Matrix + Semantic Pins | `section-05-test-matrix.md` | Not Started |
| 06 | Diagnostics + Spec-Test Audit | `section-06-diagnostics-audit.md` | Not Started |
| 07 | Close-out + Supersession (absorbs BUG-04-085 investigation) | `section-07-closeout.md` | Not Started |
| 08 | **Codegen Poly-Lambda Monomorphization (absorbs BUG-04-042, JIT-only scope per 2026-04-19 TPR-08-R4-01 resolution)** | `section-08-codegen-poly-lambda.md` | In Progress — §08.1 complete; §08.2 in-progress (9/10 main cells + 5 re-intern cells e1–e5; cell d deferred to §08.5 broadened parity audit); §08.1.5 / §08.3 / §08.4 / §08.5 / §08.6 / §08.N not-started; blocks commits until §08.3 lands. AOT integration tests (BUG-04-AOT-MONO) are a sibling scope. |
