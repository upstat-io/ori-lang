---
section: "04"
title: "Codegen Defense-in-Depth Assertions"
status: in-progress

reviewed: true
goal: "Insert an `assert_no_unresolved_type_vars` call at the single upstream codegen seam (`ori_llvm::function_compiler::process_arc_function` + the lambda counterpart in `declare_and_process_lambda`) so that any `Tag::Var` surviving the typeck → ARC → codegen boundary is caught immediately with a typed error, a clear diagnostic, and integration with the existing `ORI_VERIFY_ARC` plumbing — NOT a collection of 4 fragile consumer-site hooks that bypass the seam. A small number of secondary pre-seam hooks (at the 4 monomorphization entry points) remain ONLY to localize the diagnostic to the pre-realization IR; the load-bearing gate is the seam hook."
success_criteria:
  # Module / API
  - "New module `compiler/ori_arc/src/ir/validate.rs` (files to be CREATED by this section) exists and exports `pub fn assert_no_unresolved_type_vars(pool: &Pool, func: &ArcFunction, interner: &StringInterner, exempt_var_ids: &FxHashSet<u32>) -> Result<(), UnresolvedTypeVar>` — verifiable post-creation via `grep -rn 'pub fn assert_no_unresolved_type_vars' compiler/ori_arc/src/ir/validate.rs` returning exactly one hit. The `exempt_var_ids` parameter mirrors the producer-side `build_exempt_var_ids` pattern (`compiler/ori_types/src/check/validators/mod.rs:161`) so generic-function bodies with `VarState::Generalized` / `VarState::Rigid` vars do NOT fire spuriously. The `UnresolvedTypeVar` error type is a typed struct `{ function: Name, var_id: ArcVarId, idx: Idx, tag: Tag }` — NOT `Result<(), String>` (gemini + codex consensus: typed enum integrates with existing `VerifyError` plumbing)."
  - "`compiler/ori_arc/src/ir/mod.rs` (to be edited by this section) declares `pub mod validate;` — verifiable post-edit via `grep -n 'pub mod validate' compiler/ori_arc/src/ir/mod.rs` returning one hit. `ori_arc/src/lib.rs` re-exports `pub use ir::validate::{assert_no_unresolved_type_vars, UnresolvedTypeVar};`."
  # Primary seam (SINGLE upstream choke point)
  - "Primary site (PRIMARY, load-bearing): `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:315` — the `process_arc_function` helper — invokes `assert_no_unresolved_type_vars` for `arc_func` BEFORE `ori_arc::run_arc_pipeline(...)` is called (line ~331). The assertion is ALWAYS-ON in both debug and release builds; `self.verify_arc` gates ADDITIONAL verification (`fn_val.verify(true)` + AIMS oracle cross-check per `codegen-rules.md §VR-1`), NOT the assertion itself. The assertion produces a typed `VerifyError::UnresolvedTypeVar` at `self.builder.record_codegen_error()` — there is no `debug_assert!` fail-open path (gemini + codex consensus: release-strip of `debug_assert!` violates CLAUDE.md §The One Rule). Verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` returning at least one hit inside `process_arc_function`."
  - "Primary site (PRIMARY, lambdas): `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:375` — the `declare_and_process_lambda` helper — invokes `assert_no_unresolved_type_vars` for `lambda` BEFORE `run_arc_pipeline(...)` at line ~443. Lambdas are compiled as separate `ArcFunction`s and do NOT flow through `process_arc_function`; they have their own `run_arc_pipeline` call which must be guarded. Verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` returning at least TWO hits total (`process_arc_function` and `declare_and_process_lambda`)."
  - "Explicit parent no-emit contract (TPR-04-R4-001 fix): `process_arc_function` returns `Result<(), VerifyError>` — NOT the prior `()` return. On PC-2 violation it returns `Err(VerifyError::UnresolvedTypeVar(_))` AFTER calling `self.builder.record_codegen_error()`. The two direct callers (`emit_arc_function` at `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:164` and `prepare_arc_function` at `compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs` — which itself cascades to `prepare_all_cached`/`prepare_mono_cached`) match on the `Result` and early-return on `Err` BEFORE invoking `ArcIrEmitter::emit_function` / `run_arc_pipeline`. This closes the gap identified in TPR-04-R4-001: `record_codegen_error()` at `compiler/ori_llvm/src/codegen/ir_builder/mod.rs:269` ONLY increments a counter — it does NOT suppress downstream emission. Implicit 'counter-based suppression' is banned by `impl-hygiene.md §Invariant Explicitness`. The cascade for Hook 1 mirrors Hook 2's Result-based cascade — both seams use the same explicit no-emit pattern. Verifiable: `grep -n 'process_arc_function' compiler/ori_llvm/src/codegen/function_compiler/` returns the two invocation sites, and each adjacent line shows `?` or `match … { Err(_) => return Err(…) }`."
  - "Explicit lambda no-emit contract (TPR-04-R0-003 fix): `declare_and_process_lambda` returns `Result<(Name, FunctionId, FunctionAbi), VerifyError>` — NOT the prior unary tuple. On PC-2 violation it returns `Err(VerifyError::UnresolvedTypeVar(_))` AFTER calling `self.builder.record_codegen_error()`. The two direct callers (`compile_lambda_arc` at `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:243` and `prepare_lambda` at `compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs:231`) match on the `Result` and early-return on `Err` BEFORE invoking `run_arc_pipeline` or `ArcIrEmitter`. This replaces the prior implicit 'record_codegen_error suppresses downstream emission' transitive invariant (banned by `impl-hygiene.md §Invariant Explicitness`). Verifiable: `grep -rn 'declare_and_process_lambda\\b' compiler/ori_llvm/src/` returns exactly the two invocation lines (plus doc-comment references in `purity_analysis.rs` + `nounwind/prepare.rs` + `define_phase.rs`), and each invocation's adjacent line shows a `?` operator or explicit `match … { Err(_) => return … }` rather than a bare unary call."
  # Secondary pre-seam sites (localize diagnostic — NOT load-bearing by themselves)
  - "Secondary site A (pre-mono entry, JIT): `compiler/ori_llvm/src/evaluator/compile.rs` around line ~230 (the point at which `mono_functions` are already in `arc_cache` and are about to be handed to `prepare_mono_cached`) — invokes `assert_no_unresolved_type_vars` for each `(arc_fn, lambdas)` triple in `arc_cache`. This site exists to surface the violation with the ORIGINAL pre-realization IR (AIMS mutates `arc_func` in place during `run_arc_pipeline`; the primary-seam assertion would see the post-lower IR, not the pre-mono IR). Without this secondary site the primary seam still catches the violation, but the diagnostic is attributed to a post-lowering state rather than the mono input — worse UX, same correctness gate. Verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/ori_llvm/src/evaluator/compile.rs` returning at least one hit."
  - "Secondary site B (pre-mono entry, AOT): `compiler/oric/src/commands/codegen_pipeline.rs` around line ~112 — analogous to the JIT site, invokes `assert_no_unresolved_type_vars` on each `arc_cache.insert(arc_fn.name, (arc_fn, lambdas))` output (both the pre-mono loop at lines ~95-105 and the mono loop at lines ~119-129). Verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/oric/src/commands/codegen_pipeline.rs` returning at least TWO hits."
  # Error integration
  - "Error shape: `UnresolvedTypeVar` is a typed struct constructed in `validate.rs` and propagated via the existing `ori_arc::verify::VerifyError` enum as a new variant `UnresolvedTypeVar(UnresolvedTypeVar)`. The primary seam treats this exactly like other `VerifyError` variants — `verify_errors` collection, `builder.record_codegen_error()`, skip subsequent emission for this function. NO parallel error path, NO `Result<(), String>`, NO `tracing::error!` standalone. Verifiable via `grep -n 'UnresolvedTypeVar' compiler/ori_arc/src/verify/` returning hits in the `VerifyError` enum definition AND in `error/` variant construction sites."
  - "`ORI_VERIFY_ARC=1` integration: the primary-seam assertion is ALWAYS-ON in both debug and release. `self.verify_arc` gates ADDITIONAL verification (the `fn_val.verify(true)` LLVM IR verify + AIMS oracle cross-check per `codegen-rules.md §VR-1`, and Alive2 validation) — NOT the assertion itself. The assertion runs whether or not `ORI_VERIFY_ARC=1` is set because it is cheaper than LLVM IR verification AND mandatory for the phase-contract enforcement per `impl-hygiene.md §Cross-Phase Invariant Contracts` — gemini + codex consensus. Verifiable: a unit test disables the validator (via injected `skip_validate` test-only hook on `process_arc_function`) and confirms the downstream LLVM IR verifier still fails cleanly on `Tag::Var` inputs — establishing defense-in-depth layering rather than gating."
  # Testing + non-regression
  - "Unit tests: `compiler/ori_arc/src/ir/validate/tests.rs` (to be CREATED) covers twelve cells across three axes (var_types / params / return / block-params):  (a) empty `var_types` → `Ok`; (b) all resolved → `Ok`; (c) first var is `Tag::Var` → `Err(UnresolvedTypeVar { var_id: 0, .. })`; (d) second var is `Tag::Var` → `Err` naming ArcVarId(1); (e) all vars `Tag::Var` → `Err` naming the first violator; (f) `Tag::Var` with `var_id` in `exempt_var_ids` → `Ok` (Generalized/Rigid exemption); (g) `Tag::BoundVar` → `Ok` (per `types.md §TK-9` — bound vars under a scheme are NOT PC-2 violations); (h) `Tag::Projection` → `Err` (PC-2 also forbids unresolved projections); (i) lambda capture environments (closure-captured var types enumerated in `ArcFunction.var_types`) covered by a separate test that builds a lambda with a `Tag::Var` in the capture env and confirms the validator flags it — closing the Blind Spot #5 (§04 blind-spots.json) about captured-closure types; (j) `Tag::Var` in `params[0].ty` (entry-block parameter) with clean `var_types` → `Err` — covers TPR-04-R0-002 axis; (k) `Tag::Var` in `return_type` with clean `var_types` → `Err(UnresolvedTypeVar { var_id: ArcVarId::INVALID, .. })` — sentinel reporting id; (l) `Tag::Var` in `blocks[1].params[0].1` (the `Idx` component of the tuple at non-entry CFG block; `ArcBlock.params` is `Vec<(ArcVarId, Idx)>`) with clean `var_types` → `Err(_)` with `var_id: blocks[1].params[0].0` — covers block-param axis the `blocks.iter().skip(1)` walk must hit."
  - "`timeout 150 ./test-all.sh` is green after landing (debug AND release). Dependency-gated: Sections 03 and 08 must be complete first — §03 ensures legitimate programs do not carry surviving `Tag::Var`s, §08 resolves BUG-04-042 BoundVar bleed which WOULD trip this assertion on valid generic code today. See `depends_on` below."
inspired_by:
  - "Rust `rustc_middle::mir::visit::TyContext` — every MIR visitor receives the type context and `debug_assert!`s that types are fully resolved at traversal boundaries; the pattern here mirrors that per-function pre-emission gate but at a SINGLE upstream choke point rather than scattered across visitors."
  - "Swift `SILVerifier` — the Swift compiler runs a multi-checkpoint IR verifier with ownership + type checks before and after SIL optimization passes; Section 04 is a single-checkpoint analog for the ARC IR → LLVM IR handoff, integrated with the existing `ORI_VERIFY_ARC` verifier stack (NOT parallel to it)."
  - "Koka `Core.Check` — Koka's backend verifies that no `TVar` escapes into the final core IR before monomorphisation; the `assert_no_unresolved_type_vars` helper is a direct structural equivalent at the `process_arc_function` seam."
  - "Lean 4 `Compiler/IR/RC.lean` — Lean places its structural RC/IR checks at a SINGLE pipeline stage rather than at per-consumer emission sites, matching the single-seam decision here."
depends_on: ["03", "08"]
third_party_review:
  status: resolved
  updated: 2026-04-21
  notes: "user_accepted_at_iter_cap_reached after 6 rounds (R0–R5); 17 verified findings all fixed inline across commits 7df958c3 → 3acde80f → 93b17075 → 635b6fc6 → 5f1beb20 → a9745c51; zero outstanding at accept time. max_rounds was extended once from 3→6 via run-more after Round 2; second cap fired after Round 5. Core design validated: §04.1 validator (walks var_types + params + return_type + blocks[*].params), §04.2 Hook 1 + Hook 2 explicit Result-based no-emit cascades (process_arc_function + declare_and_process_lambda; counter-based suppression via record_codegen_error banned per impl-hygiene.md §Invariant Explicitness), §04.3 diagnostic sites with per-function exempt sets via build_exempt_var_ids, §04.4 12-cell test matrix. Ready for implementation; drift against HEAD-at-implementation-time to be caught during coding. user-accepted at iter_cap_reached after 3 rounds; 9 findings all fixed inline (commits 4ca28a6b, 86c9886f, 5fa0bf24, e7e91572); 0 residuals"
sections:
  # Split into 04.1 (module), 04.2 (primary seam — the load-bearing site), 04.3 (pre-mono diagnostics localization), 04.4 (tests), 04.R (TPR), 04.N (checklist).
  # Prior version had 25 checkbox items > 20 (audit SIZE_VIOLATION). Restructuring collapses the per-site subsections into one primary-seam section + one secondary-sites section.
  - id: "04.1"
    title: "New `ori_arc::ir::validate` module with typed error shape and exemption set"
    status: in-progress
    notes: "Module + error shape + re-exports + build_exempt_var_ids visibility already landed at HEAD (verified 2026-04-21). Remaining work: populate validate/tests.rs body with §04.4 12-cell matrix + lambda-capture behavioral test."
  - id: "04.2"
    title: "PRIMARY seam: `process_arc_function` + `declare_and_process_lambda` hooks (load-bearing)"
    status: implementation-done-close-blocked
    notes: "Hook 1 + Hook 2 + 9-site Result cascade landed at HEAD (2026-04-21; unstaged in tree). cargo c + clippy clean workspace-wide. Assertion fires correctly — -537 LCFails, +538 passes in ori_spec_llvm. Close-out blocked by §04.2.B: upstream Tag::Var leak on 3-level generic chain surfaces as 2 new AOT failures (generics::test_generic_chain_three_levels + _string). Close §04.2 only after §04.2.B fix lands AND test-all.sh returns green."
  - id: "04.2.B"
    kind: plan-blocker-inline
    severity: high
    title: "BLOCKER: upstream Tag::Var leak on 3-level generic chain (generics::test_generic_chain_three_levels)"
    status: in-progress


    notes: "Closed 2026-04-21 after full /fix-bug rigor (Phase 1 root-cause → Phase 1.5 consensus → Phase 2 TDD matrix → Phase 2.5 plan-body TPR 3 rounds 11 findings resolved → Phase 4 implementation commit 3f7d85f5 + matrix commit 8a7e9040 → Phase 5 code-TPR survivor-mode clean + hygiene 1 Major fixed inline + improve-tooling retrospective + sync-claude clean). Root cause: var_subst built from scheme_var_ids missed union-find roots when scheme vars were unified under other representatives, leaking Tag::Var into monomorphization substitution. Fix: new SSOT helper extend_var_subst_with_roots_via_pool extends var_subst with root entries at all 3 call sites (eager monomorphization, deferred-resolve, JIT imported-mono). Test coverage: 25 new tests (4 unit + 3 integration + 15 AOT + 3 spec) all green on first landing; 2 AOT tests #[ignore]'d with BUG-04-090/091 anchors (unrelated codegen bugs, predate fix). Hygiene close-out fix: generics.rs:434 #[ignore] anchor updated to reference BUG-04-090 explicitly. See §R TPR + §R Hygiene blocks for full details."
  - id: "04.TPR-A"
    title: "TPR checkpoint after 04.1 + 04.2 (the load-bearing surface)"
    status: not-started
  - id: "04.3"
    title: "SECONDARY pre-mono sites: JIT `compile.rs` + AOT `codegen_pipeline.rs` (diagnostic localization only)"
    status: not-started
  - id: "04.4"
    title: "Unit tests: 12-cell matrix across var_types / params / return / block-params axes (resolved, unbound, exempt, BoundVar, Projection, lambda-capture, first-violator-deterministic, entry-param, return-type, non-entry-block-param)"
    status: not-started
  - id: "04.R"
    title: "Close-out (code annotations, test-all.sh green, hygiene review)"
    status: not-started
  - id: "04.N"
    title: "Completion checklist"
    status: not-started
---

## Intelligence Reconnaissance

Queries run 2026-04-17 (re-run 2026-04-18 after /review-plan editor pass):

- `scripts/intel-query.sh --human file-symbols "ori_arc/src/ir" --repo ori` — inventory `ArcFunction`, `ir/validate` module surface before adding `assert_no_unresolved_type_vars`.
- `scripts/intel-query.sh --human callers "process_arc_function" --repo ori` — CONFIRMED: two callers, `emit_arc_function` (define_phase.rs:164, immediate-emit path for tests/impls/inline-fallback) and `prepare_arc_function` (prepare.rs:208, two-pass prepare path for ordinary/mono bodies). Both converge at `process_arc_function` — this is the single upstream seam.
- `scripts/intel-query.sh --human callers "declare_and_process_lambda" --repo ori` — CONFIRMED: two callers, `compile_lambda_arc` (define_phase.rs:243, immediate-emit lambda path) and `prepare_lambda` (prepare.rs:231, two-pass lambda path). Both converge at `declare_and_process_lambda` — this is the single lambda seam, distinct from `process_arc_function` because lambdas have their own `run_arc_pipeline` invocation at define_phase.rs:443 (NOT routed through `process_arc_function`).
- `scripts/intel-query.sh --human callers "prepare_mono_cached" --repo ori` — blast radius for secondary-site B (pre-mono diagnostic localization).
- `scripts/intel-query.sh --human similar "validate type vars before codegen" --repo rust,swift,lean4 --limit 5` — cross-repo patterns for pre-codegen type-variable validation (Rust `MIR TyContext debug_assert!`, Swift `SILVerifier` per-function seam, Lean 4 `Compiler/IR/RC.lean` structural check).
- `scripts/intel-query.sh --human file-symbols "ori_types/check/validators" --repo ori` — producer-side exemption pattern (`build_exempt_var_ids`, `collect_first_unbound_var`) that §04.1's `exempt_var_ids` parameter mirrors.

Results summary (≤500 chars) [ori]: `ArcFunction` defined in `ori_arc/src/ir/`; no `ir/validate` module exists yet — this section creates it. The real codegen seam is NOT the 4 consumer sites the prior plan version named — it is `process_arc_function` (define_phase.rs:315) + `declare_and_process_lambda` (define_phase.rs:375), which are the sole pre-`run_arc_pipeline` choke points. Producer-side exemption via `build_exempt_var_ids` in `ori_types/check/validators/mod.rs:161`. [rust]: `rustc_middle::mir` uses `debug_assert!`(ty.is_fully_resolved()) at MIR visitor traversal boundaries. [swift]: `SILVerifier` runs per-function ownership + type checks before SIL optimization. [lean4]: `IR/RC.lean` places structural RC/IR checks at a single pipeline stage.

---

## Context — Why This Section Exists

Sections 01–03 of this plan form the **producer side** of the typeck PC-2 phase contract
(`impl-hygiene.md §Cross-Phase Invariant Contracts`, `typeck.md §PC-2`, `types.md §PC-2`):

| Section | Producer-side responsibility |
|---------|------------------------------|
| 01 | Stop empty-list `Tag::Var` from being generalized in the first place (AST-based Value Restriction) |
| 02 | Add a validator module in `ori_types::check::validators` that detects surviving `Tag::Var`s and emits E2005 |
| 03 | Wire the validator into the 4 bodies-pass call sites so every function body is checked before ARC IR lowering, PLUS end-of-body defaulting pre-pass for legitimate empty literals |

Section 04 is the **consumer side** — a defense-in-depth backstop at the codegen seam.
`codegen-rules.md §VR-1` mandates per-function LLVM IR verification after emission (gated by
`ORI_VERIFY_ARC`); this section is the analogous gate one step earlier: before the ARC function
is handed to `ori_arc::run_arc_pipeline`, verify that no `Tag::Var` index is present in
`ArcFunction.var_types`. If one is, something upstream (either the typeck bodies pass or the
ARC lowerer itself) violated the `impl-hygiene.md §Cross-Phase Invariant Contracts` row:

> Type Checker → Codegen | All type variables resolved | No `Idx` with `Tag::Var` in typed IR

`codegen-rules.md §TR-2` states this invariant directly:

> All type indices SHALL be fully resolved via `pool.resolve_fully(idx)` before LLVM type
> construction. Unresolved type variables (`Tag::Var`) SHALL NOT reach codegen — their
> presence is a type checker bug.

### The Architectural Lesson from /review-plan Round 1 (2026-04-18)

Both dual-source reviewers (codex + gemini) converged on the same finding: the prior version of
§04 named 4 consumer sites (prepare_all_cached × 2, compile.rs mono loop, codegen_pipeline.rs
mono loop). That layering is **wrong per `impl-hygiene.md §Side Logic`** — a cross-phase
invariant belongs at a single upstream choke point, not scattered across 4 downstream consumers
that bypass the seam. Specifically:

1. **Impl methods** use the `emit_arc_function` immediate-emit path (`impls.rs:88,151`) — they
   bypass `prepare_all_cached` entirely. A 4-site plan misses them.
2. **Test wrappers** use the same immediate-emit path — same miss.
3. **Inline fallback bodies** (when a function is NOT in `arc_cache`) use `lower_function_can`
   + `emit_arc_function` — also bypass `prepare_all_cached`.
4. **Lambdas** are separately compiled `ArcFunction`s that do NOT route through
   `process_arc_function`; they have their own `run_arc_pipeline` call in
   `declare_and_process_lambda`.

A 4-site hook set is fragile because every future refactor of the codegen entry points
introduces new paths that silently bypass the assertion. The primary seam is
`process_arc_function` + `declare_and_process_lambda` — every pre-emission path converges
there, and both are immediately upstream of `ori_arc::run_arc_pipeline` (which mutates
`arc_func` in place, so post-pipeline is too late).

### Why Section 04 Depends on Sections 03 AND 08

The assertions added here are correct only if the producer side has fixed all legitimately-
typed programs. Before Section 03 lands, the bodies pass does not yet call the validator,
so empty-list `Tag::Var`s that are valid program constructs (e.g. `let x = []` where the
element type is resolved later by an argument to the same function) may still survive into
the ARC IR. Enabling the assertion before Section 03 lands would produce spurious assertion
failures on such programs.

**Section 08 (poly-lambda BoundVar bleed, BUG-04-042) is ALSO a hard prerequisite** — not
merely a merge blocker. The BoundVar bleed produces surviving `Tag::Var`s in monomorphized
imported-generic bodies (`assert_eq<T>` over poly-lambda-containing modules). §04's assertion
WILL fire on these programs until §08 resolves the bleed. Both reviewers flagged this as a
load-bearing dependency during the /review-plan Phase 2 blind-spots scan.

The dependency is therefore load-bearing in two dimensions: **do not merge Section 04 before
BOTH Section 03 AND Section 08 are merged AND `test-all.sh` is green.** Track via the
plan's `depends_on: ["03", "08"]` frontmatter.

---

## 04.1 — New `ori_arc::ir::validate` Module with Typed Error Shape

### Motivation

The assertion helper must live in `ori_arc` rather than `ori_llvm` because:

1. The check is about the **ARC IR** (`ArcFunction.var_types: Vec<Idx>`), which is owned by
   `ori_arc`. Placing validation logic in `ori_arc` keeps the cross-phase invariant with its
   owner crate — consistent with `impl-hygiene.md §SSOT`.
2. `ori_llvm` is downstream of `ori_arc` in the dependency graph; a function in `ori_arc`
   can be called from both `ori_llvm` sites (JIT and AOT) AND future AIMS verification passes
   (e.g., the `ORI_VERIFY_ARC=1` path) without introducing new cross-crate dependencies.
3. The typed error enum `UnresolvedTypeVar` lives alongside `ori_arc::verify::VerifyError`
   (same crate), so integrating into the existing `VerifyError` variant set is a local change.

### Files to Create / Edit

**State at HEAD (verified via file inspection 2026-04-21):** the §04.1 MODULE surface is ~85% landed; the §04.4 TEST BODY is the remaining §04.1 work. Table below is ground-truthed against actual file sizes and grep hits.

| File | State at HEAD | Action | Approx. LOC |
|------|---------------|--------|-------------|
| `compiler/ori_arc/src/ir/validate.rs` | **DONE** (7302B at HEAD; full `assert_no_unresolved_type_vars` + `UnresolvedTypeVar` per §04.1 spec) | No further action this section | — |
| `compiler/ori_arc/src/ir/validate/tests.rs` | **STUB** (344B — `//! Unit tests for assert_no_unresolved_type_vars.` header only) | CREATE body — 12-cell matrix + lambda-capture + first-violator-deterministic per §04.4 | ~200 |
| `compiler/ori_arc/src/ir/mod.rs` | **DONE** (`pub mod validate;` present at line 25) | No further action | — |
| `compiler/ori_arc/src/lib.rs` | **DONE** (`pub use ir::validate::{assert_no_unresolved_type_vars, UnresolvedTypeVar};` at line 92) | No further action | — |
| `compiler/ori_arc/src/verify/mod.rs` | **DONE** (`UnresolvedTypeVar(crate::ir::validate::UnresolvedTypeVar)` variant at line 83; `From<UnresolvedTypeVar>` impl at line 86) | No further action | — |
| `compiler/ori_types/src/check/validators/mod.rs` | **DONE** (`pub fn build_exempt_var_ids(pool: &Pool, scheme_var_ids: &[u32]) -> FxHashSet<u32>` at line 161 — visibility is already `pub`) | No further action | — |
| `compiler/ori_types/src/lib.rs` | **DONE** (`pub use check::validators::{build_exempt_var_ids, validate_body_types};` at line 32) | No further action | — |

**What still requires implementation (the actionable §04.1 tail):**

- Populate the `compiler/ori_arc/src/ir/validate/tests.rs` body with the 12-cell matrix (§04.4) + the `test_lambda_with_tag_var_in_capture_environment_fails` behavioral test. This is the only §04.1 work remaining — the module + error shape + re-exports + exempt-set helper are all already landed.

**Rationale for leaving already-done items in the section as documentation**: the module architecture decisions (typed error shape, `UnresolvedTypeVar` struct, re-export chain, `build_exempt_var_ids` visibility) are load-bearing for §04.2's hook signatures and §04.3's secondary-site contract. Stripping them would force §04.2/§04.3 implementers to re-derive the decisions. The HEAD-state column makes the drift visible without removing the architectural reference.

### Implementation Outline

```rust
//! Validation utilities for ARC IR correctness.
//!
//! Provides post-lowering checks that enforce the cross-phase invariant
//! contract `impl-hygiene.md §Cross-Phase Invariant Contracts`:
//!
//! > Type Checker → Codegen | All type variables resolved |
//! > No `Idx` with `Tag::Var` in typed IR
//!
//! And `codegen-rules.md §TR-2`:
//!
//! > All type indices SHALL be fully resolved via `pool.resolve_fully(idx)`
//! > before LLVM type construction. Unresolved type variables (`Tag::Var`)
//! > SHALL NOT reach codegen.
//!
//! The functions in this module make that invariant self-enforcing at the
//! single upstream codegen seam (`process_arc_function` and
//! `declare_and_process_lambda` in `ori_llvm::codegen::function_compiler`).
//!
//! # Exemption Set
//!
//! The producer-side validator (`ori_types::check::validators`) exempts
//! `VarState::Generalized` and `VarState::Rigid` per the documented pool
//! divergence: the current pool stores generalized vars as
//! `Tag::Var(VarState::Generalized)` rather than `Tag::BoundVar`
//! (`types.md §SC-1` target-only note). This consumer-side validator mirrors
//! the exemption via an `exempt_var_ids` parameter so generic function bodies
//! do not fire spuriously until the pool converts generalized vars to
//! `Tag::BoundVar` (tracked as a target-conformance item in §02).

use rustc_hash::FxHashSet;

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool, Tag};

use crate::ir::{ArcFunction, ArcVarId};

/// A single unresolved type variable encountered in `ArcFunction.var_types`.
///
/// Constructed by [`assert_no_unresolved_type_vars`] on invariant violation.
/// Wrapped by `ori_arc::verify::VerifyError::UnresolvedTypeVar(_)` for
/// propagation up the verification pipeline alongside existing `VerifyError`
/// variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnresolvedTypeVar {
    /// The `ArcFunction.name` where the violation was detected.
    pub function: Name,
    /// The specific `ArcVarId` whose type is unresolved.
    pub var_id: ArcVarId,
    /// The raw type-pool index that resolved to `Tag::Var`.
    pub idx: Idx,
    /// The tag at the violating index (always `Tag::Var` at emission time;
    /// carried for future-proofing against `Tag::Projection` / `Tag::Infer`).
    pub tag: Tag,
}

/// Check that no `Tag::Var` (outside `exempt_var_ids`) or `Tag::Projection`
/// appears in any type-bearing position of `func`. PC-2 enforcement covers
/// every `Idx` field on `ArcFunction` and `ArcParam`:
///
/// - `func.var_types[*]`          — SSA-variable types (primary storage)
/// - `func.params[*].ty`          — entry-block parameter types (`ArcParam.ty`)
/// - `func.return_type`           — declared return-type `Idx`
/// - `func.blocks[*].params[*].1` — CFG-block parameter types (tuple
///                                    `.1` = `Idx`; `ArcBlock.params` is
///                                    `Vec<(ArcVarId, Idx)>`)
///
/// `var_types`-only scope would let a `Tag::Var` in a parameter or return
/// position bypass the check entirely, defeating `typeck.md §PC-2` /
/// `canon.md §4.2` enforcement on those axes. See TPR-04-R0-002 for the
/// full reproduction (`compiler/ori_arc/src/ir/mod.rs:241-248` — `ArcParam`
/// struct; `compiler/ori_arc/src/ir/mod.rs:375-396` — `ArcFunction` struct).
///
/// # Parameters
///
/// - `pool`: the frozen type pool (post-typecheck).
/// - `func`: the ARC function to validate.
/// - `interner`: string interner for rendering function name in diagnostics.
/// - `exempt_var_ids`: var IDs that are legitimately `Tag::Var` because they
///   are `VarState::Generalized` or `VarState::Rigid` (mirrors the producer
///   side `build_exempt_var_ids` in `ori_types/check/validators/mod.rs`).
///   In practice this set is always EMPTY at the shipping call sites: the
///   primary seam (§04.2) fires post-monomorphization; the secondary sites
///   (§04.3 A + B) iterate caches pre-filtered by `lower_and_infer_borrows`
///   at `compiler/oric/src/test/runner/arc_lowering.rs:39` (JIT) and the
///   `if sig.is_generic() { continue; }` gate at
///   `compiler/oric/src/commands/codegen_pipeline.rs:92-94` (AOT), so no
///   generic bodies reach either call site. The parameter remains in the
///   signature as a defense-in-depth hook: a future call site that needs
///   to exempt rigid/generalized vars can populate it from the owning
///   `FunctionSig.scheme_var_ids` without breaking the invariant.
///
/// # Returns
///
/// `Ok(())` when the invariant holds. `Err(UnresolvedTypeVar)` with the FIRST
/// offending variable (deterministic iteration order — ArcVarId ascending).
///
/// # When to Call
///
/// Call this from `process_arc_function` + `declare_and_process_lambda` in
/// `ori_llvm`, BEFORE `ori_arc::run_arc_pipeline(...)` is invoked. The AIMS
/// pipeline mutates `arc_func` in place; calling after would validate the
/// wrong IR.
///
/// # Relationship to Section 03 and Section 08
///
/// This check is a consumer-side backstop. The producer-side enforcement
/// lives in `ori_types::check::validators::validate_body_types` (Section 03
/// of the `empty-container-typeck-phase-contract` plan). Both must be present
/// for full defense-in-depth per `impl-hygiene.md §Cross-Phase Invariant
/// Contracts`. Section 08 resolves BUG-04-042 (poly-lambda BoundVar bleed)
/// which would otherwise trip this assertion on valid generic code.
pub fn assert_no_unresolved_type_vars(
    pool: &Pool,
    func: &ArcFunction,
    interner: &StringInterner,
    exempt_var_ids: &FxHashSet<u32>,
) -> Result<(), UnresolvedTypeVar> {
    // Walk every type-bearing position on `ArcFunction` in deterministic
    // order. Returns the FIRST violator; callers log all. Four positions are
    // covered so PC-2 enforcement holds across the entire IR surface:
    //   1. var_types[*]            (SSA storage; primary)
    //   2. params[*].ty            (entry-block parameters)
    //   3. return_type             (function return Idx)
    //   4. blocks[*].params[*].1   (CFG-block parameters; tuple .1 = Idx)
    //
    // When a violating `Tag::Var` appears in a non-var_types position, the
    // error's `var_id` field reports the ArcVarId recorded ON THE PARAM
    // (`ArcParam.var`). For the return_type position there is no owning
    // ArcVarId — a sentinel `ArcVarId::INVALID` is used and the error's `idx`
    // field identifies the violation precisely.

    // Gate order mirrors producer-side validator
    // (ori_types/check/validators/mod.rs): resolve_fully → tag check →
    // exemption set. `resolve_fully` is the key step — `Tag::Var` in any
    // position may be a Link to a concrete type that fully resolves.
    let check_idx = |ty: Idx, reporting_var_id: ArcVarId| -> Result<(), UnresolvedTypeVar> {
        let resolved = pool.resolve_fully(ty);
        let tag = pool.tag(resolved);
        if matches!(tag, Tag::Var) {
            let var_id = pool.data(resolved); // Tag::Var: data IS the var_id
            if exempt_var_ids.contains(&var_id) {
                return Ok(());
            }
            return Err(UnresolvedTypeVar {
                function: func.name,
                var_id: reporting_var_id,
                idx: resolved,
                tag,
            });
        }
        // Also catch Projection (PC-2 clause 3) — unresolved associated types
        // are a PC-2 violation even though they are not Tag::Var.
        if matches!(tag, Tag::Projection) {
            return Err(UnresolvedTypeVar {
                function: func.name,
                var_id: reporting_var_id,
                idx: resolved,
                tag,
            });
        }
        Ok(())
    };

    // 1. SSA-variable storage (primary position).
    for (raw_idx, &ty) in func.var_types.iter().enumerate() {
        check_idx(ty, ArcVarId::new(raw_idx as u32))?;
    }
    // 2. Entry-block parameters.
    for param in &func.params {
        check_idx(param.ty, param.var)?;
    }
    // 3. Return type. ArcVarId::INVALID is a sentinel — no owning SSA var.
    check_idx(func.return_type, ArcVarId::INVALID)?;
    // 4. CFG-block parameters (skip blocks[0]; it mirrors func.params).
    //    ArcBlock.params is Vec<(ArcVarId, Idx)> — tuple, not struct
    //    (verified against compiler/ori_arc/src/ir/mod.rs:335). Destructure
    //    as (var, ty) rather than accessing .var / .ty.
    for block in func.blocks.iter().skip(1) {
        for &(var, ty) in &block.params {
            check_idx(ty, var)?;
        }
    }

    let _ = interner; // reserved for future Name rendering in Display impl
    Ok(())
}

impl UnresolvedTypeVar {
    /// Render a user-facing diagnostic message for this violation.
    ///
    /// Used when `VerifyError::UnresolvedTypeVar(_)` flows to the driver's
    /// diagnostic emission at `self.builder.record_codegen_error()`.
    pub fn render(&self, interner: &StringInterner) -> String {
        format!(
            "Tag::{:?} reached codegen: function `{}`, ArcVarId({}) has \
             unresolved type index {:?}. This is a typeck PC-2 contract \
             violation (impl-hygiene.md §Cross-Phase Invariant Contracts, \
             codegen-rules.md §TR-2).",
            self.tag,
            interner.lookup(self.function),
            self.var_id.raw(),
            self.idx,
        )
    }
}

#[cfg(test)]
mod tests;
```

### Wire Into `ori_arc::verify::VerifyError`

Add a new variant to the existing `VerifyError` enum at
`compiler/ori_arc/src/verify/mod.rs` (exact location verified via
`grep -n 'enum VerifyError' compiler/ori_arc/src/verify/` at implementation
time):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VerifyError {
    // ... existing variants (UseBeforeDef, DanglingBlockRef, RcOnScalar,
    // DecOnBorrowed, ArgOwnershipLenMismatch, AbsentParamHasUses,
    // FipStructural, ...) ...
    /// A variable's type in `ArcFunction.var_types` is `Tag::Var` or
    /// `Tag::Projection` — a PC-2 invariant violation (see
    /// `ir::validate::UnresolvedTypeVar`). Wrapped so existing verification
    /// error handling in `process_arc_function` works unchanged.
    UnresolvedTypeVar(crate::ir::validate::UnresolvedTypeVar),
}
```

### Re-Export From `ori_arc`

Add to `compiler/ori_arc/src/lib.rs`:

```rust
pub use ir::validate::{assert_no_unresolved_type_vars, UnresolvedTypeVar};
```

This makes the call sites in `ori_llvm` and `oric` as clean as
`ori_arc::assert_no_unresolved_type_vars(...)` without needing the full path.

---

<!-- coordinates-with: plans/empty-container-typeck-phase-contract/section-08-codegen-poly-lambda.md -->

## 04.2 — PRIMARY Seam: `process_arc_function` + `declare_and_process_lambda` Hooks

> **Cross-section coordination (2026-04-20)** — Before implementing this subsection's seam, consult `plans/empty-container-typeck-phase-contract/section-08-codegen-poly-lambda.md §08.6`. §08.6 documents the caller-parameterized two-case `exempt_var_ids` contract: (a) mono path → empty set (strict `typeck.md §PC-2`); (b) pre-mono generic body path → populated from `FunctionSig.scheme_var_ids` (SC-1 exemption). §08.3's remap-aware re-intern fix runs upstream of this seam; §08.3's matrix (cells e1–e5) must make case (a) sound — zero `Tag::Var` at the seam for every mono instantiation — without regressing case (b). Any modification to this seam's strictness must preserve both cases.

These are the LOAD-BEARING sites — the single upstream choke points through which every
ARC-to-LLVM body flows. All other hooks (§04.3 secondary pre-mono sites) are diagnostic
localization, NOT correctness gates.

### §04.2 Design Decision: Seam Placement and `exempt_var_ids` Contract (Editor Pass 2026-04-21)

Two substantive reviewer positions on the primary-seam design surfaced during /review-plan Phase 2
(blind-spots scan). Editor resolves BOTH explicitly so §04.2 implementers do not re-litigate them:

**Decision 1 — Seam placement: PRE-`run_arc_pipeline` is the correct gate for this section.**

- **Context**: Gemini flagged that AIMS injects `Tag::Var`s during pipeline passes (notably during
  generalization of polymorphic callees and during lattice analysis of erased-generic returns).
  Running the assertion BEFORE `run_arc_pipeline` misses those injected vars, per Gemini's analysis.
- **Editor resolution**: §04 enforces `typeck.md §PC-2` (the TYPE-CHECKER→CODEGEN invariant) —
  NOT `aims-rules.md` soundness. `typeck.md §PC-2` explicitly says: "No `Tag::Var` in any
  type-bearing IR position" at the typeck OUTPUT contract. This invariant is violated IFF the
  upstream typeck emitted an unresolved var — which is precisely what §04's defense-in-depth
  assertion is designed to catch at the codegen entry seam (`impl-hygiene.md §Cross-Phase
  Invariant Contracts`).
- **AIMS-injected vars are a separate concern, NOT covered by shipped `aims-rules.md` VF-1/VF-2**: if `run_arc_pipeline` itself produces a `Tag::Var`, that is an AIMS invariant violation (per `canon.md §7.1` Invariant 5: "the unified model must stay unified") — owned by `arc.md` + `aims-rules.md §9` verification layers. HEAD's `aims-rules.md §9` VF-1 (line 714) checks structural well-formedness only — use-before-def, dangling block refs, RC on scalar, dec on borrowed, arg ownership length mismatch; no type-variable check. VF-2 (line 716) has only subcheck (a) `AbsentParamHasUses` shipped, with subchecks (b)/(c)/(d) target-only. Any `Tag::Var` surviving `run_arc_pipeline` is therefore an UNDETECTED class at HEAD — file via `/add-bug` (subsystem `aims`, scope "§9 VF-* extension for post-pipeline unresolved type variables"); §04's pre-AIMS seam does NOT cover it.
- **Verification**: the §04.2 pre-pipeline placement is correct for typeck PC-2. §04 is scope-limited to the typeck→codegen phase contract per its mission — post-AIMS unresolved-type-var detection is a separate verification-layer extension, not a §04 deliverable.
- **Citations**: `typeck.md §PC-2`, `canon.md §4.2`, `impl-hygiene.md §Cross-Phase Invariant
  Contracts` (Type Checker → Codegen row), `codegen-rules.md §TR-2`, `aims-rules.md §9` VF-1 (line 714 — structural scope) + VF-2 (line 716 — shipped subcheck inventory).

**Decision 2 — `exempt_var_ids` at the primary seam: EMPTY is correct for Hook 1 and Hook 2.**

- **Context**: Codex said keep the seam architecture as-is with empty `exempt_var_ids` at the
  primary sites AND prove the empty-exempt invariant. Gemini said the set must be DYNAMIC
  (populated from `FunctionSig.scheme_var_ids`) because JIT/test paths bypass `prepare_all_cached`
  and retain non-empty `scheme_var_ids` on generic bodies.
- **Editor resolution**: the primary seams (`process_arc_function`, `declare_and_process_lambda`)
  fire on ARC functions that are EITHER (a) monomorphized instances from the mono loop, OR
  (b) fully-resolved non-generic bodies. Both categories have empty `scheme_var_ids` at the seam
  by construction of the upstream lowering pipeline. Per `canon.md §4.2`, generic scheme bodies
  exit `infer/body_finalize::normalize_body_generalized_to_bound_var_sig` with `Tag::BoundVar`
  leaves — any residual `Tag::Var(Generalized)` at this point is a leak-alarm for a missed
  normalization position (§03 producer-side defect), NOT a legitimate state the seam should exempt.
- **Invariant pin**: the §04.4 test matrix MUST include a test that passes a non-empty
  `scheme_var_ids` bag to `build_exempt_var_ids` and confirms the seam fires on ALL resulting
  `Tag::Var`s (because at the primary seam, post-monomorphization, no legitimate `VarState::Rigid`
  or `VarState::Generalized` survives — the set being empty in practice is the load-bearing
  invariant §04 enforces). This closes the hole Gemini identified: if a future refactor introduces
  a JIT/test path that DOES carry non-empty `scheme_var_ids` into the primary seam, the test
  matrix fails loudly rather than silently masking the drift.
- **Secondary sites (§04.3) ALSO empty — updated 2026-04-21 post-TPR-R2**: §04.3 Sites A + B also use empty `FxHashSet::default()`. `lower_and_infer_borrows` at `compiler/oric/src/test/runner/arc_lowering.rs:39` filters `sig.is_generic()` at :59/:81/:147/:207 before populating JIT `arc_cache`; `compiler/oric/src/commands/codegen_pipeline.rs:92-94` filters generics before the AOT pre-mono loop; both AOT loops operate on entries with empty `scheme_var_ids` (non-generic tops or fully-substituted mono). No dynamic `build_exempt_var_ids` call is required at the shipping sites. The prior "§04.3 is dynamic" narrative was corrected after verification — earlier rounds' fix landed the simplified stub (§04.3 Site A code block now uses `FxHashSet::default()` directly).
- **What this closes**: Gemini's concern about JIT/test paths bypassing `prepare_all_cached` — every JIT path filters generics upstream, so no bypass carries non-empty `scheme_var_ids` into any seam. The §04.4 matrix pin makes the empty-set invariant testable across primary + secondary sites.
- **Action for §04.2 implementer**: keep the empty `FxHashSet::default()` at both primary hooks
  as documented in the existing Hook 1 and Hook 2 code blocks below. Add one `#[test]` to
  `validate/tests.rs` confirming that a non-empty `scheme_var_ids` bag passed to
  `build_exempt_var_ids` — and then to the seam — does NOT suppress firing (modeling the JIT/test
  path bypass Gemini flagged). This test is the semantic pin for the "empty set is invariant"
  decision.
- **Citations**: `typeck.md §PC-2`, `canon.md §4.2`, `types.md §SC-1` (scheme/BoundVar migration),
  `impl-hygiene.md §Invariant Explicitness` (explicit tests over implicit invariants),
  `impl-hygiene.md §Cross-Phase Invariant Contracts`.

### File: `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs`

### Hook 1: `process_arc_function` (line ~315)

Insert the assertion at the TOP of `process_arc_function`, BEFORE the debug tracing call
and BEFORE `ori_arc::run_arc_pipeline` is invoked. The AIMS pipeline mutates `arc_func` in
place (borrow annotations, RC insertion, reuse emission); the assertion must run on the
pre-pipeline IR.

```rust
pub(super) fn process_arc_function(
    &mut self,
    name: Name,
    arc_func: &mut ori_arc::ArcFunction,
) -> Result<(), VerifyError> {
    // PC-2 contract check — see plan `empty-container-typeck-phase-contract` §04.
    // Runs ALWAYS (debug + release) — NOT gated by self.verify_arc because
    // phase-contract enforcement is mandatory per CLAUDE.md §The One Rule
    // (no debug_assert! fail-open). The verify_arc flag gates ADDITIONAL
    // downstream verification (fn_val.verify, AIMS oracle), not this gate.
    //
    // `exempt_var_ids` is empty for non-generic functions. Generic functions
    // reach this seam only after monomorphization, at which point their
    // scheme_var_ids are fully substituted — empty set is correct.
    let exempt: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
    if let Err(err) = ori_arc::assert_no_unresolved_type_vars(
        self.pool, arc_func, self.interner, &exempt,
    ) {
        tracing::error!(
            contract_violation = true,
            error = ?err,
            "Tag::Var in ARC IR violates PC-2 contract \
             (impl-hygiene.md §Cross-Phase Invariant Contracts, \
             codegen-rules.md §TR-2)"
        );
        self.builder.record_codegen_error();
        // Return Err so the caller (emit_arc_function / prepare_arc_function)
        // skips LLVM emission. `record_codegen_error` alone is INSUFFICIENT —
        // it only increments a counter (compiler/ori_llvm/src/codegen/
        // ir_builder/mod.rs:269); the caller's emission path does NOT check
        // that counter. Explicit Result propagation is the only reliable
        // no-emit contract per impl-hygiene.md §Invariant Explicitness.
        return Err(VerifyError::UnresolvedTypeVar(err));
    }

    // ... existing body: AIMS param ownership, run_arc_pipeline, etc. ...
    Ok(())
}
```

The `Result` return is load-bearing: continuing into `run_arc_pipeline` on a contract-
violating input risks panics inside AIMS analysis (it assumes resolved types), AND the
caller's emission path would otherwise continue unconditionally past `process_arc_function`
and call `ArcIrEmitter::emit_function` on the unpiped IR. Skipping at BOTH levels —
within this function AND in the caller on `Err` — is the correct failure mode.

### Hook 1 caller-site updates — mandatory co-change (TPR-04-R4-001 + TPR-04-R5-001)

The full Hook 1 cascade chain — every caller between `process_arc_function` and the
outermost JIT/AOT batch entry points — MUST be updated in the same commit. The
cascading `Result` propagation is the SAME ARCHITECTURAL PATTERN as Hook 2's lambda
cascade; both seams must skip downstream emission via explicit `Result` return because
`record_codegen_error()` at `compiler/ori_llvm/src/codegen/ir_builder/mod.rs:269` is
counter-only and has no suppression side effect.

**Concrete caller chain (verified via `grep -rn 'emit_arc_function\|define_function_body_arc_with_subst\|process_arc_function\|prepare_arc_function' compiler/ori_llvm/src/` at HEAD `5f1beb20`; shift tolerance on line numbers):**

| Level | Site | Current signature | Required change |
|---|---|---|---|
| 0 | `process_arc_function` (define_phase.rs:~315) | `fn(&mut self, Name, &mut ArcFunction)` | `-> Result<(), VerifyError>` (§04.2 Hook 1 primary) |
| 1a | `emit_arc_function` (define_phase.rs:~115) | `fn(&mut self, Name, FunctionId, &FunctionAbi, ArcFunction, Vec<ArcFunction>)` | `-> Result<(), VerifyError>` via `?` on `process_arc_function`. On `Err`, MUST call `self.exit_debug_scope()` before returning to match the normal-path `exit_debug_scope()` at define_phase.rs:~220; otherwise the debug scope entered by `define_function_body_arc_with_subst` (define_phase.rs:80) leaks. Use a scope-guard helper OR an explicit `match … { Err(e) => { self.exit_debug_scope(); return Err(e); } }` (TPR-04-R5-002). |
| 1b | `compile_lambda_arc` (define_phase.rs:~243) | unary-tuple return | `-> Result<…, VerifyError>` (Hook 2 lambda cascade — already specified); must also propagate parent-seam failures when its own `emit_arc_function` chain fires. |
| 2a | `define_function_body_arc_with_subst` (define_phase.rs:~67) | `fn(…)` | `-> Result<(), VerifyError>` via `?` on `emit_arc_function`. `exit_debug_scope` cleanup lives one level DOWN (in `emit_arc_function`) so this level just propagates. |
| 2a' | `define_function_body` (define_phase.rs:47) | `fn(&mut self, Name, FunctionId, &FunctionAbi, CanId, &CanonResult, bool)` | `-> Result<(), VerifyError>` via `?` on `define_function_body_arc_with_subst`. Pure wrapper (delegates at define_phase.rs:56); propagation only. Omitted from the Round 5 table; §04.2 implementation MUST carry the signature change through this wrapper or the `?` in 2a does not compile. |
| 2b | `compile_tests` branches (impls.rs:88, impls.rs:151) | — | On `Err`, use `continue` to skip to the next test iteration WITHOUT altering `compile_tests`'s return signature. The per-test failure is already recorded via `record_codegen_error()` and the suite continues. This mirrors Gemini R5-001's recommendation: not every outer caller must change signature — some outer-loop callers can absorb `Err` via `continue` or `let _ =` when their loop semantic is "keep going past individual failures". |
| 2c | `compile_impl_method_from_sig` (impls.rs:241) | `fn<'sig>(…)` returning `()` | **Helper, not a loop.** Its body has early-`return` guards (sig-iter exhaustion, `sig.is_generic()`) and calls `self.define_function_body(...)` as the final statement at impls.rs:321. Simplest shape: keep the unit return and absorb Err at the call site — `let _ = self.define_function_body(...);` — OR change to `-> Result<(), VerifyError>` and propagate via `?`. The `continue`-on-Err handling belongs in the two CALLER loops: impls.rs:199 is the explicit-method loop `for method in &impl_def.methods`; impls.rs:221 is the default-method loop nested inside `for item in &trait_def.items { if let TraitItem::DefaultMethod(default) = item { ... } }`. Both are caller-level loops, but only the first iterates `impl_def.methods`. Per-method failure is recorded via `record_codegen_error()` through the 1a `exit_debug_scope` path regardless of the chosen shape. |
| 3 | `prepare_arc_function` (nounwind/prepare.rs) | existing Hook 2 cascade | Already cascades to `prepare_all_cached` / `prepare_mono_cached` per Round 2's fix; now ALSO propagates Hook 1 `Err` via `?` on the `process_arc_function` call. No new callers above this level — the Round 2 cascade already covers them. |
| 4 | JIT batch `evaluator/compile.rs` + AOT batch `oric/src/commands/codegen_pipeline.rs` | existing | Per the two-level cascade from Round 2: these already track per-function failures via `record_codegen_error()` counter. With Hook 1's Result cascade landed, the recorded failures now correspond to `Err` paths that also skipped emission — the counter stays the SSOT for end-of-batch pass/fail classification. |

**`continue`-on-Err pattern (TPR-04-R5-001's `compile_tests` case):** when a caller's
loop semantic is "keep going past individual failures and report all at the end",
`continue` on `Err` is the correct pattern — it does NOT require the caller to change
its own signature. The `record_codegen_error()` counter already tracks aggregate
failures for end-of-batch reporting. Callers whose semantic is "stop emitting if ANY
subcomponent fails" (e.g., `define_function_body_arc_with_subst` — a single function's
body, emit-or-skip) MUST propagate via `?`. The distinction is per-caller: loop
semantic → `continue`; single-function semantic → propagate.

**Verifiable post-implementation:** `grep -rn 'process_arc_function\b'
compiler/ori_llvm/src/codegen/function_compiler/` returns the two invocation sites
with adjacent `?` or `match … { Err(_) => … }`. Additionally, `grep -rn
'emit_arc_function\b' compiler/ori_llvm/src/` returns the three invocation sites
(define_phase.rs:106 + impls.rs:88 + impls.rs:151), each with adjacent `?` OR
`continue` OR explicit match (not a bare unary call). Finally, any `emit_arc_function`
signature change to `-> Result<…>` forces `clippy::must_use` to catch unhandled
call sites at compile time.

### Hook 2: `declare_and_process_lambda` (line ~375)

Insert the assertion at the TOP of `declare_and_process_lambda` — analogous placement to
Hook 1, before `run_arc_pipeline` at line ~443. Lambdas do NOT route through
`process_arc_function`; they are a distinct seam.

```rust
pub(super) fn declare_and_process_lambda(
    &mut self,
    lambda: &mut ori_arc::ArcFunction,
) -> Result<(Name, FunctionId, FunctionAbi), VerifyError> {
    // PC-2 contract check for lambdas — same pattern as process_arc_function.
    // Lambdas have their own run_arc_pipeline call (line ~443) and do NOT
    // route through process_arc_function.
    //
    // Explicit no-emit control path (TPR-04-R0-003): the signature returns
    // Result<(Name, FunctionId, FunctionAbi), VerifyError> so every caller
    // MUST match on the result and early-return its own error path. The
    // prior implicit "record_codegen_error suppresses downstream emission"
    // contract relied on a transitive invariant across four callers that
    // `impl-hygiene.md §Invariant Explicitness` forbids — a future refactor
    // of any caller could silently land LLVM IR from a contract-violating
    // lambda. Making the failure path explicit closes that regression
    // surface.
    let exempt: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
    if let Err(err) = ori_arc::assert_no_unresolved_type_vars(
        self.pool, lambda, self.interner, &exempt,
    ) {
        tracing::error!(
            contract_violation = true,
            error = ?err,
            "Tag::Var in lambda ARC IR violates PC-2 contract"
        );
        self.builder.record_codegen_error();
        return Err(VerifyError::UnresolvedTypeVar(err));
    }

    // ... existing body: apply AIMS contracts, declare LLVM function, etc. ...
    // On success, return Ok((name, function_id, function_abi)) from the
    // existing tail.
}
```

Soundness argument (TPR-04-R0-003 explicit-contract rewrite): the `Result`
return is load-bearing. Each of the two direct callers (`compile_lambda_arc`
at `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:243` and
`prepare_lambda` at
`compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs:231`)
MUST match on the `Result` and early-return on `Err` BEFORE calling
`run_arc_pipeline` or `ArcIrEmitter` or any LLVM emission path. The emission
paths inside `compile_lambda_arc` / `prepare_lambda` that subsequently invoke
`run_arc_pipeline` / `ArcIrEmitter` are on the success arm of each caller's
match — transitively owned by the same `Err` gate, not distinct sites. This
replaces the prior implicit "`record_codegen_error` suppresses downstream
emission" transitive invariant — a property that was not local to the lambda
hook and could regress silently if either caller's emission path were
refactored. The explicit `Err` arm makes the no-emit contract local and
testable: a unit test per §04.4 confirms that each caller's `Err` handling
skips LLVM emission, and `clippy::must_use_result` on the return type makes
an ignored result a compile error. `VerifyError::UnresolvedTypeVar(_)` is
the existing enum variant §04.1 adds; this hook reuses it for zero
error-path proliferation.

### §04.2 caller-site updates — mandatory co-change with the Hook 2 signature change

The two direct callers of `declare_and_process_lambda` MUST be updated in
the same commit as the hook itself (the `Result` return type makes this a
hard compile-time requirement — clippy + `must_use` enforce it, there is
no way to ship a half-converted tree):

- `compile_lambda_arc` at
  `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:243`
  (immediate-emit lambda path): match on the `Result`; on `Err`, propagate
  to the enclosing emission-skip path that `record_codegen_error()` already
  establishes — do NOT call `run_arc_pipeline` / `ArcIrEmitter` on that arm.
- `prepare_lambda` at
  `compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs:231`
  (two-pass lambda path): same — on `Err`, skip the pipeline + emitter calls
  downstream. Because `prepare_lambda` currently has signature
  `fn(…) -> PreparedLambda` and its only call site is `prepare_arc_function`
  at `nounwind/prepare.rs:190` (verified via `grep -n 'prepare_lambda'
  compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs`), the
  `Err` propagation cascades TWO levels further: change `prepare_lambda`'s
  signature to `fn(…) -> Result<PreparedLambda, VerifyError>` AND change
  `prepare_arc_function`'s signature to match (`fn(…) -> Result<…, VerifyError>`),
  propagating up to `prepare_all_cached` + `prepare_mono_cached` callers that
  already track per-function success/failure via `record_codegen_error()`.
  **Filter-out is NOT sound** — dropping a failed lambda from the
  `prepared_lambdas: Vec<PreparedLambda>` collection at lines 186–196 leaves
  the parent `arc_func` to later emit a `PartialApply` against the removed
  lambda's original name (the `remap_partial_apply_names` call at line 201
  rewrites names but does not drop references to missing callees). Parent
  emission MUST also be skipped when any of its lambdas fails validation,
  matching the immediate-emit path `compile_lambda_arc` + `emit_arc_function`.
  The cascading signature change is mandatory at every level — `clippy::must_use`
  on the new `Result` return forces the propagation at compile time. Analogous
  cascading treatment applies to `compile_lambda_arc` (immediate-emit path at
  `define_phase.rs:243`): its caller `emit_arc_function` must also receive
  the `Err` and skip parent emission before calling `run_arc_pipeline` on the
  parent `arc_func`.

No other direct call sites of `declare_and_process_lambda` exist (verified
via `grep -rn 'declare_and_process_lambda\b' compiler/ori_llvm/src/` —
only the two helper references and the two invocation sites enumerated
above). Verifiable post-edit via the same grep returning exactly TWO
invocation lines and each adjacent line showing a `?` operator or explicit
`match … { Err(_) => return … }` — NOT a bare unary call expression.

### Why NOT place at `run_arc_pipeline` entry

The check could also be moved INTO `ori_arc::run_arc_pipeline` as a precondition. We reject
that placement because:

1. The `ori_arc` crate must not emit `tracing::error!` directly — diagnostic surfacing is
   the driver's responsibility (`impl-hygiene.md §Side Logic`). Pushing the check inward
   would require a new error channel out of `run_arc_pipeline`, duplicating the existing
   `VerifyError` path.
2. The driver (`ori_llvm`) has context about WHICH codegen entry point is running (JIT
   vs AOT, direct vs mono), which informs the diagnostic — `ori_arc::run_arc_pipeline`
   does not.
3. The existing `VerifyError` plumbing flows OUT of `run_arc_pipeline`; keeping the new
   `UnresolvedTypeVar` variant flowing IN the same direction preserves the SSOT for error
   shape.

### §04.2 post-landing forward-verification (absorbed from §08.6, 2026-04-20)

These items were originally §08.6's forward-coordination checks; absorbed here to eliminate a same-plan self-blocker (§08.6 → §04.2) per CLAUDE.md §Plan-Blocker Bugs Belong IN the Plan. §04.2 naturally owns "the seam fires correctly against BOTH the intra-module lambda_mono path and the cross-module re-intern path from §08.3" as part of its own completion — it is not a §08 obligation, it is the deliverable of the seam itself.

- [x] **Post-substitution firing verification (both paths)**: after §04.2's `assert_no_unresolved_type_vars` hooks are inserted at `define_phase.rs:315` (`process_arc_function`) and `:375` (`declare_and_process_lambda`) AND §08.3's remap-aware re-intern is live in the merged pool, verify the assertion fires POST-substitution for (a) the intra-module lambda_mono path via `resolve_all_lambda_bound_vars` at `define_phase.rs:134` + `nounwind/prepare.rs:173`, and (b) the cross-module re-intern path via `pool/re_intern/`. Record the confirmation in §04.R close-out and backlink §08.6.R as "§04 seam order verified correct under §08.1.R corrected diagnosis; no change required." Seam-line-number shifts are acceptable as long as the POST-substitution invariant holds.
  Verified 2026-04-21 via code inspection of HEAD (unstaged dirty tree with Hook 1 + Hook 2 landed). (a) Intra-module `lambda_mono` path: `emit_arc_function_inner` at `define_phase.rs:138` calls `resolve_all_lambda_bound_vars(&mut arc_func, &mut lambdas, self.pool, self.interner, classifier)` at `:157-163` BEFORE the lambda compilation loop at `:166-173` (which invokes Hook 2 via `compile_lambda_arc` → `declare_and_process_lambda` at `:253-254`) and BEFORE Hook 1 at `process_arc_function` invocation at `:187`. Nounwind batch path: `prepare_arc_function` at `nounwind/prepare.rs:186` calls `resolve_all_lambda_bound_vars` at `:208` BEFORE the lambda-prepare loop at `:215-222` (which invokes Hook 2 via `prepare_lambda` → `declare_and_process_lambda` at `:262`) and BEFORE Hook 1 at `:237`. Both paths are POST-substitution; line-number shifts (`:134`→`:157`, `:173`→`:208`, `:315`→Hook 1 landed at `process_arc_function` fn start, `:375`→Hook 2 landed at `declare_and_process_lambda` fn start) are within the plan's "acceptable as long as the POST-substitution invariant holds" envelope. (b) Cross-module re-intern path: `pool/re_intern/` runs during pool construction (upstream of codegen entirely per `types.md §TY-6` exception); Hook 1/2 see post-re-intern types natively.
- [x] **Empty-exempt assertion strictness holds under §08.3 remap**: validate that `assert_no_unresolved_type_vars`'s empty-`exempt_var_ids` contract at all shipping sites (§04.2 primary seam + §04.3 secondary Sites A/B) remains sound after §08.3's remap-aware re-intern lands. Strict `typeck.md §PC-2` + `canon.md §4.2` rule: §08.3 + `resolve_all_lambda_bound_vars` + upstream generic filters (`lower_and_infer_borrows` / `codegen_pipeline.rs:92-94`) together must leave zero `Tag::Var` at every call site; a surviving `Tag::Var` is a §08.3 completeness bug or an upstream-filter regression. §08.3's matrix cells e1–e5 must preserve this invariant. The `exempt_var_ids` parameter stays in the validator signature as a defense-in-depth hook — a future call site that DOES need dynamic exemption (e.g. non-generic-filtered pre-mono path not yet shipped) can populate it from `FunctionSig.scheme_var_ids` without breaking the contract. Record the verification in §04.R as "empty-exempt seam strictness holds under §08.3 remap + upstream generic filters; §08.3's cell coverage adequate."
  Verified 2026-04-21 via code inspection. Hook 1 (`process_arc_function`) and Hook 2 (`declare_and_process_lambda`) both use `let exempt: FxHashSet<u32> = FxHashSet::default();` (empty) at `define_phase.rs:379` and `:452` respectively. The empty-set invariant is load-bearing because: arc_cache entries at §04.3 sites are either (1) non-generic top-level functions (filtered at `codegen_pipeline.rs:92-94` via `sig.is_generic() { continue; }` before insertion) or (2) imported monomorphized instances (fully substituted before insertion). Both categories have empty `FunctionSig.scheme_var_ids` or no scheme binder; `build_exempt_var_ids(pool, &[])` returns `FxHashSet::default()`. Post-§08.3 remap preserves the substitution relation — no new `Tag::Var`s are introduced, only existing ones are re-indexed under the merged pool. §04.2.B's 3-level generic chain leak is upstream-substitution incomplete (not a contract violation); the seam correctly fires. Test pin `test_primary_seam_empty_exempt_set_invariant_pin` at `compiler/ori_arc/src/ir/validate/tests.rs` (editor-added 2026-04-21) codifies the empty-set load-bearing contract.

---

## 04.2.B — BLOCKER: Upstream Tag::Var Leak on 3-Level Generic Chain (generics::test_generic_chain_three_levels)

> **Status**: `in-progress` (Phase 1 complete, Phase 1.5 complete, Phase 1.75 pending)
> **Classification**: B — blocker surfaced by §04.2's PC-2 assertion hook (working as designed).
> **Severity**: **high** (reclassified 2026-04-21 Phase 1.5) — complexity-elevated subsystem (`ori_types` mono + `ori_arc` ARC lowering + `ori_llvm` codegen), cross-crate visibility, systemic blast radius (every 3+ hop generic chain where intermediate callees receive type-variable args), blocker to §04.2 + §04 + §04.N close-out.
> **Scope**: **point fix** — 1-2 files, <30 LOC, no architectural change. Extract-or-inline-call of existing `extend_var_subst_with_roots` at the deferred-mono-resolve site in `check/exports.rs`.
> **Blocks**: §04.2 close-out + §04.TPR-A entry. §04 cannot complete until §04.2.B lands.
> **Surfaced by**: §04.2's assertion at `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` fired during post-implementation `test-all.sh` run (2026-04-21 HEAD).

### Repro

```
timeout 150 cargo test --test aot generics::test_generic_chain_three_levels 2>&1
```

Both variants fail (`test_generic_chain_three_levels` + `test_generic_chain_three_levels_string`) with:

```
ERROR ori_llvm::codegen::function_compiler::define_phase:
  Tag::Var in ARC IR violates PC-2 contract
  contract_violation=true
  error=UnresolvedTypeVar {
    function: Name(shard=2, local=19),    # or shard=8, local=20 for _string variant
    var_id: ArcVarId(2),
    idx: Idx(220),
    tag: Tag::var
  }
error[E5001]: LLVM module verification failed
```

### Baseline context

- Baseline (pre-§04.2, commit `58c26963`): both tests **passed** despite the surviving `Tag::Var` in ARC IR. The pass was accidental — LLVM codegen produced either coincidentally-correct output OR output the test's assertions did not discriminate. State.sh `aot_integration: 2161/0/22` included both tests as passing.
- Post-§04.2 (HEAD): assertion fires at `process_arc_function` seam, codegen emission skipped, AOT compile returns error. 2 tests fail; remainder of suite at 2159/2/24 (net delta: 2 new failures, assertion working as designed).

### Phase 1 investigation findings (2026-04-21)

**Fixture reality-check — the program is NOT a 3-level nested type.** The failing fixtures `compiler/ori_llvm/tests/aot/fixtures/generics/generic_chain_three_levels{,_string}.ori` are 3-hop identity-like generic call chains, NOT `Applied<Applied<Applied<T>>>` shapes:
```
@id <T> (x: T) -> T = x;
@wrap <T> (x: T) -> T = id(x: x);
@double_wrap <T> (x: T) -> T = wrap(x: x);
@main () -> int = { let n = double_wrap(x: 42); ... }
```
This exercises the **monomorphization discovery chain** (mono engine's ability to cascade substitution across 3 generic function instantiations with rigid-to-rigid param flow), NOT a nested-type remap matrix. The hypothesis list above assumed the wrong shape.

**Leak is present AT typeck exit, not introduced downstream.** `ORI_DUMP_AFTER_TYPECK=1` on the repro shows:
```
Function @wrap<T> (x: $b6) -> $b6              ← signature correctly BoundVar
  CallNamed : $t8 (unresolved)                 ← body: Tag::Var($t8) leaked
    Ident(id) : ($t8) -> $t8                   ← instantiated scheme, never link-resolved
Function @double_wrap<T> (x: $b7) -> $b7
  CallNamed : $t9 (unresolved)                 ← body: Tag::Var($t9) leaked
    Ident(wrap) : ($t9) -> $t9
```
Typeck reports "0 errors" — `validate_body_types` does NOT fire `E2005` on `$t8`/`$t9`. Same shape exists in 2-hop `@wrap → @id` (dumped from `/tmp/gen_chain_2.ori`), confirming the leak is not 3-level-specific at typeck exit.

**Validator exemption is intentional.** `compiler/ori_types/src/check/validators/mod.rs:161-173` `build_exempt_var_ids` exempts `$t7/$t8/$t9` because rank-weighted union-find (`typeck.md §UN-7`) can make a fresh instantiation var the root of a scheme var's equivalence class. The validator resolves each `scheme_var_ids[i]` through `resolve_fully`, and if the root is still `Tag::Var`, adds the root's `var_id` to the exempt set. This is per-design and covered by regression test `scheme_var_root_is_fresh_instantiation_var_no_false_e2005` at `validators/tests.rs:441`. The exempt `Tag::Var` leaves in `expr_types` are equivalent (via union-find) to scheme vars — they MUST be substituted to concrete types at monomorphization time, but legitimately survive typeck.

**Contract: body expr_types carry Tag::Var leaves that are union-find-linked to scheme vars.** Downstream consumers (mono engine, ARC lowering, codegen) must either (a) call `resolve_fully` at each walk step of body types before substitution, or (b) trust that `substitute_in_pool` with a `{scheme_var_root.var_id → concrete}` mapping rewrites every such leaf. §04.2.B fails because this contract is not fully honored at 3 levels.

**Why 2-hop accidentally passes and 3-hop breaks (hypothesis, needs verification in Phase 1.75 /tp-help):** At mono-time substitution for `@apply_identity<int>` (2-hop), the substitution pipeline produces a body whose `$t7` leaf either (a) gets resolved through `resolve_fully` via direct union-find link to the outer scheme var's root AND the root's var_id appears in the substitution map, OR (b) the LLVM codegen path doesn't observe the residual `Tag::Var` because `test_generic_calling_generic`'s output assertions don't discriminate the wrong value. For 3-hop, `@double_wrap<int>` → `@wrap<int>` adds a second layer where `$t8` (inside `@wrap`) is NOT linked to `@double_wrap`'s outer scheme var — it is linked to `@wrap`'s OWN scheme var, which is only substituted to `int` when `@wrap<int>` is monomorphized. If `substitute_in_pool` doesn't call `resolve_fully` at each walk step, `$t8` survives as a raw `Tag::Var` in `@wrap<int>`'s realized ARC IR, and `process_arc_function` at §04.2's hook fires `UnresolvedTypeVar`.

**Root cause — PINPOINTED to line numbers (Phase 1 code-reading round 2, 2026-04-21):**

The bug is an **asymmetry between the two monomorphization paths** in `ori_types`. Both paths ultimately call `build_mono_body_type_map` + `substitute_in_pool`, but only one calls `extend_var_subst_with_roots` on the substitution map beforehand:

- **Non-deferred path** (`maybe_record_mono_instance` at `compiler/ori_types/src/infer/expr/calls/monomorphization.rs:17-121`): the call-site mono when `@main` directly requests `double_wrap<int>` with a concrete type arg. Line 71 invokes `extend_var_subst_with_roots` (monomorphization.rs:279-302) which adds `{union_find_root_var_id → concrete}` entries for every scheme var whose equivalence-class root differs from the declared var id. Only after that extension does `build_mono_body_type_map` run at line 98.

- **Deferred path** (`resolve_deferred_mono_calls` loop at `compiler/ori_types/src/check/exports.rs:180-247`): the resolve-pass that picks up recorded `DeferredMonoCall` entries when the caller is monomorphized. Line 180-205 builds `resolved_var_subst` with callee's scheme_var_ids only (`resolved_var_subst.insert(*callee_var_id, concrete)` at line 204) and calls `build_mono_instance(pool, ..., &resolved_var_subst)` at line 231 — **with NO equivalent of `extend_var_subst_with_roots`**. Inside `build_mono_instance` at `exports.rs:257-288`, `build_mono_body_type_map` at line 277 walks the pool with this non-extended map.

**Walk-through for the failing repro:**

1. `@main` calls `double_wrap(x: 42)`. `maybe_record_mono_instance` enters the non-deferred branch (concrete arg). `var_subst = {double_wrap_scheme_var_id → int}` + `extend_var_subst_with_roots` adds `{double_wrap_root_var_id → int}` if the root differs. MonoInstance for `double_wrap<int>` recorded correctly.
2. Typeck of `@double_wrap`'s body (runs earlier, at function-definition time) encounters `wrap(x: x)` where `x: T_double_wrap` (still a type variable). `maybe_record_mono_instance` for `wrap` at this call site: `has_unresolved_vars = true` (the arg type is a `Tag::Var`). Enters deferred branch at line 57-67, calls `record_deferred_mono_call`. The deferred entry stores `{wrap_scheme_var_id → CallerSchemeVar(0)}` mapping wrap's T to double_wrap's T position.
3. Later, at export time, `resolve_deferred_mono_calls` walks the deferred list. For the `wrap` entry, it looks up double_wrap's mono instances: `double_wrap<int>` → caller_generic_args=[Type(int)]. Resolves the deferred binding: `resolved_var_subst.insert(wrap_scheme_var_id, int)` at line 204. Calls `build_mono_instance` at line 231 with this map.
4. Inside `build_mono_instance`, `build_mono_body_type_map` walks the pool. For `wrap`'s body expression types (stored in `expr_types` at typeck time), the body contains `CallNamed: $t7_wrap_body` where `$t7_wrap_body` is a fresh instantiation var allocated when typing `id(x: x)` inside `@wrap`'s body.
5. Rank-weighted union-find made `$t7_wrap_body` the **root** of `@wrap`'s scheme var T equivalence class (`wrap.T` linked TO `$t7_wrap_body`, not vice-versa). `substitute_var($t7_wrap_body)`:
   - `var_id = 7` — not in map (map has `wrap_scheme_var_id`, which is a different u32).
   - `VarState::Unbound` (because `$t7_wrap_body` is the root; no Link to follow).
   - Falls through, returns `$t7_wrap_body` unchanged.
6. `wrap<int>`'s mono'd body carries a raw `Tag::Var($t7_wrap_body)`. ARC lowering preserves it (ARC doesn't re-resolve types — `canon.md §4.2` PC-2 guarantees clean IR arriving). `process_arc_function` at §04.2's seam fires `UnresolvedTypeVar { var_id: ArcVarId(2), idx: Idx(220), tag: Tag::Var }`.

**Why 2-hop `apply_identity<int>` accidentally passes pre-§04.2:** the non-deferred path runs for it (concrete arg at the call site). `extend_var_subst_with_roots` fires. The body's `$t7` gets substituted to `int`. No residual `Tag::Var`. No assertion fires.

**Why 3-hop fails:** the MIDDLE layer (`wrap`) takes the deferred path, which misses the root extension.

**Affected surface:**
- Any 3+ hop generic call chain where intermediate callees receive type variables (not concrete types) as generic args. This is EVERY non-trivial generic composition, not just the failing fixtures.
- Currently failing (post-§04.2): `test_generic_chain_three_levels` + `test_generic_chain_three_levels_string`.
- Currently passing but likely silently miscompiling (pre-§04.2 undiagnosed, now surfaced with §04.2's PC-2 assertion active): unknown; to be probed by the TDD matrix in Phase 3.

**Fix sites (ranked):**

1. **Primary (typeck-side deferred path)**: `compiler/ori_types/src/check/exports.rs` — add root-extension call between line 205 (end of `resolved_var_subst` build) and line 231 (call to `build_mono_instance`). This is the symmetry-restoring fix on the side that today misses root extension entirely.
2. **Primary (test-runner JIT imported-mono path)**: `compiler/oric/src/test/runner/imported_mono.rs:109-110` — a THIRD call site carrying the same defect, surfaced by TPR Round 0 2026-04-21. The runner builds `var_subst` from `generic_sig.scheme_var_ids` at lines 83-88, then calls `build_mono_body_type_map` directly at line 110 with NO root extension. Same asymmetry class as `exports.rs`. Without this fix, imported generic JIT compilation silently miscompiles or fires the §04.2 PC-2 seam assertion on any imported generic where the callee's scheme var is not the union-find representative. Fix: insert `extend_var_subst_with_roots(merged_pool, &generic_sig.scheme_var_ids, &mut var_subst)` between lines 104 and 110.
3. **Refactor**: extract `extend_var_subst_with_roots` from `monomorphization.rs:279-302` into a shared helper in `ori_types::pool::substitute` (or on `Pool` as an inherent method) so ALL THREE call sites — eager typeck, deferred typeck, JIT imported-mono — call the same SSOT implementation. Per `impl-hygiene.md §Algorithmic DRY`, the 2-instance threshold is already crossed; with 3 instances the extraction is non-negotiable.

**Alternative (rejected):** modify `substitute_var:90-105` in `pool/substitute/mod.rs` to `resolve_fully` at entry. Wider-reaching, affects ALL substitution call sites (not just mono), could unintentionally alter behavior for other consumers (e.g., `default_unbound_vars_in_scope` uses `substitute_in_pool`).

**Phase 1 status**: **complete**. Repro confirmed, leak-site pinpointed, fix-site identified to specific line numbers, architectural symmetry principle established (both mono paths must use the same substitution-extension logic). Ready for Phase 1.5.

### Success criteria

- [x] Root cause phase narrowed (pool/substitute or llvm/monomorphize — final selection at Phase 1.75 /tp-help consensus).
- [x] Root cause function identified (specific offending walk step or missing `resolve_fully` call).
- [x] Fix implemented at the correct upstream site (NOT in §04.2's assertion — the assertion MUST continue to fire if the underlying leak recurs; weakening it is INVERTED-TDD per CLAUDE.md).
- [x] `cargo test --test aot generics::test_generic_chain_three_levels` passes (both variants).
- [x] `timeout 150 ./test-all.sh` returns green (or same-baseline: same known-failing set, no new failures vs state.sh at close-of-fix HEAD). Verified at commit `8a7e9040` (dev) — 843 interp failures match §06.2 `E2005:AmbiguousType` baseline exactly; +3 LLVM runtime failures predate this commit (caused by the already-committed `3f7d85f5` root-cause fix unblocking ~541 previously-LC_FAIL tests from passed 1851 → 2392); no interpreter regression.
- [x] Matrix test added at the fix's owning plan section (§03 or §08 or wherever the root cause lives) exercising `Applied<Applied<Applied<T>>>` / 3-level-chain shape; positive + negative pin per CLAUDE.md §Matrix Testing Rule. Landed in `8a7e9040` across `compiler/ori_types/src/pool/substitute/tests.rs` (4 unit tests on the extracted helper), `compiler/ori_types/src/check/integration_tests.rs` (3 deferred-mono integration tests — 3-hop, 4-hop, multi-param forwarding), `compiler/ori_llvm/tests/aot/generics.rs` (15 AOT tests + 2 ignored with concrete blockers), `compiler/ori_arc/src/ir/validate/tests.rs` (positive + negative PC-2 pins — `test_pc2_assertion_fires_on_synthetic_leak` / `test_pc2_assertion_silent_on_clean_function`), `tests/spec/imports/generic_import_chain.ori` (3 imported-mono 3-hop tests passing on both backends).
- [x] Imported-mono JIT path covered: `tests/spec/imports/generic_import_chain.ori` (shipped `8a7e9040`) exercises 3-hop imported-generic chains where the callee's scheme var is not the union-find representative. Tests `test_imported_mono_chain_3hop_int`, `test_imported_mono_chain_3hop_str`, `test_imported_mono_chain_3hop_bool` all green on both interpreter and LLVM backends, confirming File 4 of the fix (per TPR-04.2.B-F1-codex).
- [ ] §04.2 status flipped to `complete` after §04.2.B success criteria all `[x]` AND §04.TPR-A clean.
- [x] This subsection's backlink to owning plan section recorded. Owning section: §03 bodies-pass integration (the root-extension asymmetry lived in the deferred-mono resolution path, which is §03's scope per line 849 "Likely root-cause owner"). Backlink recorded here: §04.2.B root-extension fix `3f7d85f5` extracted `extend_var_subst_with_roots` into `ori_types::pool::substitute` (the shared helper now serves all three mono paths — eager (`monomorphization.rs:71`), deferred (`exports.rs:205`), and JIT imported-mono (`oric/src/test/runner/imported_mono.rs:110`)). §03's close notes reference this helper as the canonical substitution-extension SSOT per `impl-hygiene.md §Algorithmic DRY` (3-instance threshold crossed; extraction non-negotiable).

### Follow-up bug anchors (filed at close-out)

Concrete tracker entries for work surfaced by §04.2.B but outside its scope. All filed with independent lifecycle per CLAUDE.md §Plan-Blocker Bugs Belong IN the Plan (these are NOT plan blockers — §04.2.B completes without them):

- `BUG-04-089` [high] — LLVM backend: 3 spec-tests regressed to runtime failures after §04.2.B unblocked LC_FAIL tests. Umbrella entry covering three specific failures (`tests/spec/expressions/immutable_bindings.ori::test_list_immutable` list destructuring `6 != 3`, `tests/spec/patterns/catch.ori::test_catch_div_zero` catch-div-zero panic-escape, `tests/spec/traits/debug/collections.ori::test_map_debug` map debug formatter quoting `{x: 1}` vs `{"x": 1}`). Per-test root-cause fixes land in BUG-04-086 (already filed), BUG-04-087 (already filed), BUG-04-088 (new). These tests were LC_FAIL pre-`3f7d85f5` — the §04.2.B root-cause fix unblocked codegen so they now reach runtime, where they expose latent LLVM codegen/runtime bugs that predate §04.2.B.
- `BUG-04-090` [high] — AOT codegen: generic forwarder applied to `[T]` causes `ori_rc_dec` on already-freed allocation. Reproduces at 2-hop through `generic_calling_generic` with `[int]`; not a §04.2.B regression.
- `BUG-01-002` [medium] — Parser: `impl<T>` method-level generics `@map<U>` rejected.
- `BUG-04-091` [high] — AOT codegen: inherent method on generic type fails `unresolved function 'unwrap' in apply`.
- `BUG-08-015` [low] — Spec/parser drift: `grammar.ebnf:311` `inherent_impl` uses `type_path` but parser accepts `impl<T> Box<T>`. Requires proposal governance.

### Linkage

- **Plan section this blocks**: §04.2 (this plan — implementation landed but cannot close)
- **Likely root-cause owner**: §03 remediation OR §08.3 remap matrix coverage (investigation decides)
- **Test exercise sites**:
  - `compiler/ori_llvm/tests/aot/generics.rs::test_generic_chain_three_levels`
  - `compiler/ori_llvm/tests/aot/generics.rs::test_generic_chain_three_levels_string`
- **Assertion that surfaced it**: `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` Hook 1 (`process_arc_function`, seam landed 2026-04-21)

### Workflow

Invoke `/fix-bug --inline plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md#04.2.B` to open the full /fix-bug workflow against this subsection.

### 1.5 Fix Consensus

**Round 1 (codex)**: agree-with-refinements. Gemini unavailable (sub-agent contract violation per /tpr-review §9; BUG-08-012 `gemini-3.1-pro-preview` capacity-429). Survivor-mode with HIGH-trust reviewer per /tpr-review §4.

**Codex verdict — all 6 questions answered with file:line evidence:**

1. **Correctness**: confirmed. Both mono paths end in `substitute_in_pool` + `build_mono_body_type_map`; the deferred path needs the SAME representative-closure on `var_subst` as the eager path. No edge case requires different semantics.
2. **Extract-vs-inline**: EXTRACT. Per `impl-hygiene.md §Algorithmic DRY` (2-instance threshold). Place helper in `pool::substitute` taking `(&Pool, &[u32], &mut FxHashMap<u32, Idx>)`. Refactor `monomorphization.rs:279-302` to call the shared helper AND add a new call at `exports.rs:205-230`.
3. **Record-side vs resolve-side**: RESOLVE-SIDE. Recording physical root var_ids at `record_deferred_mono_call:229-251` would freeze a union-find representative before inference completes; the representative can still change post-record. Resolve-time query via `pool.var_idx_for_id + pool.resolve_fully` is correct and matches the existing `build_exempt_var_ids` pattern at `check/validators/mod.rs:161-172`.
4. **Blast radius**: larger than 2 failing tests. Every deferred generic call where the callee scheme var stops being the representative can leak today. Phase 3 TDD matrix dimensions: 3-hop, 4-hop, multi-parameter forwarding, reordering, nested shapes (`Option`, `Result`, list, tuple, user ADT), forwarded vars in return/deep-field positions, multiple deferred callees in one body, recursive/SCC cases.
5. **INVERTED-TDD check**: CLEAN. Fix repairs producer-side monomorphization; the §04.2 seam assertion stays fully active. Satisfies `impl-hygiene.md §INVERTED-TDD` and CLAUDE.md §The One Rule.
6. **Reference-compiler prior-art**: DISPROVED. Codex verified: Rust's `rustc_monomorphize::collector` has a single instantiate-and-normalize path but no eager/deferred representative-extension analogue. Swift's `SubstitutionMap` composition (`SILTypeSubstitution.cpp:465-472,515-541`) is a different pattern. Koka's `Core/Specialize.hs:199-205` specializes by post-inlining type-arg substitution with no union-find-root analogue. **Drop the prior-art claim from Phase 1 findings above.**

**Consensus outcome**: proceed to Phase 2 with Codex's refinements. Note: single-reviewer consensus (HIGH-trust) is sufficient here because (a) Codex ran `cargo test -p ori_llvm generic_chain_three_levels` and verified the failure reproduces at the claimed seam, (b) every code claim carries a verified file:line citation, (c) the INVERTED-TDD check is straightforward (assertion stays active), (d) Gemini's absence is a known tracked issue (BUG-08-012), not a review-content problem.

**Independent code verification** (per `feedback_reviewer_grounding_and_trust.md`):

- `monomorphization.rs:69-71` + `:279-302` — extend_var_subst_with_roots is at the cited location. Verified in Phase 1 code reading.
- `exports.rs:178-205` + `:231-238` — deferred-resolve loop builds resolved_var_subst without root extension and calls build_mono_instance. Verified in Phase 1.
- `pool/substitute/mod.rs:90-105` — substitute_var handles direct var_id lookup + VarState::Link follow-through; Unbound falls through. Verified in Phase 1.
- `check/validators/mod.rs:161-172` — build_exempt_var_ids is the existing producer-side precedent for root-tracking. Verified in Phase 1.
- `ori_arc/src/ir/validate.rs:97-130` — Codex's new citation; not in my Phase 1 list. **VERIFIED 2026-04-21**: `assert_no_unresolved_type_vars` walks `var_types`/`params`/`return`/`blocks[*].params` in the ARC IR and emits `VerifyError::UnresolvedTypeVar` on any non-exempt `Tag::Var`. This is the §04.2 seam that fires for `$t7_wrap_body`. Citation is correct.

### 2. TDD Matrix

All tests written BEFORE Phase 4 implementation, verified failing against current HEAD, then passing after the fix. Dimensions from Codex Q4 + CLAUDE.md §TDD for Bugs + §Matrix Testing Rule.

#### Unit tests — `compiler/ori_types/src/pool/substitute/tests.rs` (new test cases)

- [x] `extend_var_subst_with_roots_via_pool_adds_root_when_different` — scheme var's root is a distinct fresh instantiation var; helper inserts `{root_var_id → concrete}`.
- [x] `extend_var_subst_with_roots_via_pool_noop_when_root_equals_scheme_var` — scheme var IS its own root; no new entries.
- [x] `extend_var_subst_with_roots_via_pool_empty_scheme_vars` — monomorphic function; map unchanged.
- [x] `extend_var_subst_with_roots_via_pool_preserves_existing_entries` — pre-populated map; helper adds without overwriting.

#### Unit tests — `compiler/ori_types/src/check/integration_tests.rs` (new test cases)

- [x] `deferred_mono_resolution_root_extension_applied_3_hop` — `@main → @double_wrap<int> → @wrap<int> → @id<int>`; after export, all three MonoInstances have `body_type_map` entries covering the body's Tag::Var leaves.
- [x] `deferred_mono_resolution_root_extension_applied_4_hop` — 4-hop chain `@main → a → b → c → d`; middle two deferred.
- [x] `deferred_mono_resolution_multi_param_forwarding` — `@f<A, B> (x: A, y: B) -> B = g(x: y, y: x)`; parameters reorder.

#### AOT integration tests — `compiler/ori_llvm/tests/aot/generics.rs` (new tests alongside existing `test_generic_chain_three_levels`)

- [x] `test_generic_chain_four_levels` — 4-hop int chain. Positive pin. Landed `8a7e9040`.
- [x] `test_generic_chain_four_levels_string` — 4-hop str chain (RC-managed). Landed `8a7e9040`.
- [x] `test_generic_chain_option_wrapped` — 3-hop chain with `Option<T>` as the type arg. Landed `8a7e9040`.
- [x] `test_generic_chain_result_wrapped` — 3-hop chain with `Result<T, E>`. Landed `8a7e9040`.
- [ ] `test_generic_chain_list_element` — 3-hop chain with `[T]`. Fixture + test landed `8a7e9040` but marked `#[ignore]` — reproduces an AOT RC codegen double-free (`ori_rc_dec called on already-freed allocation`) that also fires at 2-hop through `generic_calling_generic` with `[int]` element type, so it is NOT a §04.2.B regression (predates the root-cause fix). Filed as `BUG-04-090` (`ori_llvm/codegen/arc_emitter` — RC emission for list return values from monomorphized generic functions).
- [x] `test_generic_chain_tuple_element` — 3-hop chain with `(T, T)`. Landed `8a7e9040`.
- [x] `test_generic_chain_user_struct` — 3-hop chain with user-defined struct carrying a generic field. Landed `8a7e9040`.
- [x] `test_generic_chain_forwarded_in_return_only` — `@f<T> (x: int) -> T = g()`; T appears only in return position. Landed `8a7e9040`.
- [x] `test_generic_chain_forwarded_in_deep_field` — `@f<T> (x: int) -> Option<(int, T)> = g()`; T appears in a nested field position. Landed `8a7e9040`.
- [x] `test_generic_multiple_deferred_callees` — `@f<T> (x: T) -> T = { let a = g(x: x); h(x: a) }`; two deferred callees in one body. Landed `8a7e9040`.
- [x] `test_generic_recursive_chain` — `@f<T> (x: T, n: int) -> T = if n == 0 then x else f(x: x, n: n - 1)`; self-recursive generic. Landed `8a7e9040`.
- [x] `test_generic_chain_five_levels` — 5-hop int chain (`@main → @a<T> → @b<T> → @c<T> → @d<T> → @id<T>`); confirms the fix holds beyond 3-hop, guarding against off-by-one in the root-extension recursion. Landed `8a7e9040`.
- [x] `test_generic_mutual_recursion_scc` — two mutually-recursive generics `@f<T> (x: T) -> T = g(x: x)` / `@g<T> (x: T) -> T = f(x: x)`; exercises SCC-sensitive deferred-mono resolution where the same call site can produce multiple deferred entries for members of the same SCC. Landed `8a7e9040`.
- [x] `test_generic_trait_dispatch_through_forwarder` — `@forward<T: Printable> (x: T) -> str = x.to_str()` called from a 3-hop chain; ensures trait-method dispatch resolution does not introduce a fresh instantiation var that bypasses the root-extension. Landed `8a7e9040`.
- [x] `test_generic_iterator_item_only_positional` — `@fwd<T> () -> impl Iterator where Item == T`; `T` appears ONLY via the existential's associated type, not as a direct parameter/return. Exercises projection-normalization interaction with root-extension. Landed `8a7e9040`.
- [x] `test_generic_closure_capture_forwarded` — generic forwarder captures a `T` in a lambda: `@fwd<T> (x: T) -> () -> T = (() -> x)`. Exercises capture-analysis interaction where the closure's captured `T` is routed through the forwarder's deferred-mono path. Landed `8a7e9040`.
- [ ] `test_generic_method_on_generic_type` — `impl<T> Box<T> { @map<U> (self, f: T -> U) -> Box<U> = ... }`. Fixture + test landed `8a7e9040` but marked `#[ignore]` for two separate blockers discovered during implementation: (a) `impl<T>` method-level generics `@map<U>` are rejected by the parser (`expected (, found <`) — grammar surface gap filed as `BUG-01-002` (`ori_parse` — no grammar production for method-level generics on inherent impl methods); (b) reduced shape using `@unwrap (self) -> T` typechecks but codegen fails with `unresolved function 'unwrap' in apply — missing mono instance?` + `E5001 LLVM module verification failed` — inherent-method-on-generic-type mono resolution gap filed as `BUG-04-091` (`ori_llvm/codegen/arc_emitter/apply.rs`). The ignored-test purpose (two-level rigid-var scoping) cannot be exercised without those features. Note: `grammar.ebnf:311` / `:341` spec-vs-parser drift for `inherent_impl` type_args filed as separate `BUG-08-015` (docs — grammar/parser drift, requires proposal governance).

#### Semantic pins (CLAUDE.md §Matrix Clamping)

- [x] `test_generic_chain_three_levels` (already failing; becomes positive pin after fix). Passing post-`3f7d85f5` per root-cause fix commit.
- [x] Negative pin: `test_pc2_assertion_fires_on_synthetic_leak` — handcrafted ARC IR with a raw `Tag::Var` in a function body; confirms `assert_no_unresolved_type_vars` DOES fire (guards against weakening §04.2's assertion per INVERTED-TDD). Landed `8a7e9040` at `compiler/ori_arc/src/ir/validate/tests.rs:46-93` with positive-no-fire companion `test_pc2_assertion_silent_on_clean_function` at `:95-135`.

#### Cross-phase verification

- [x] `timeout 150 ./test-all.sh` returns green (no new failures vs state.sh baseline; the two failing tests flip to passing). Verified at commit `8a7e9040` — 843 interp failures match §06.2 baseline exactly; `test_generic_chain_three_levels` and `test_generic_chain_three_levels_string` flipped from failing to passing.
- [x] Dual-execution parity: `cargo st` (interpreter) + `cargo run --release -- test --backend=llvm tests/spec/imports/generic_import_chain.ori` (LLVM) both pass on the new `.ori` spec tests added for imported generic chaining (3 tests each backend, all green).
- [x] `ORI_CHECK_LEAKS=1` clean on `generic_chain_three_levels` fixture (AOT binary + run with leak tracking). Verified via `diagnostics/diagnose-aot.sh --release`: compilation clean (ORI_VERIFY_ARC=1), exit code 0, leak check clean, RC Stats balanced (zero alloc — AIMS elided all RC traffic on the `int` chain), codegen audit clean.

### 3. Implementation

#### File 1: `compiler/ori_types/src/pool/substitute/mod.rs` (new public helper)

Add after `build_mono_body_type_map` (near line 362):

```rust
/// Extend `var_subst` with `{union_find_root_var_id → concrete}` entries for
/// every scheme var whose equivalence-class root differs from the scheme
/// var's own `var_id`.
///
/// Invoked by BOTH monomorphization paths before `substitute_in_pool` walks
/// body types. Mirrors the producer-side exemption logic in
/// [`crate::check::validators::build_exempt_var_ids`] — rank-weighted
/// union-find (`typeck.md §UN-7`) can make a fresh instantiation var the
/// root of a scheme var's equivalence class, in which case `substitute_var`
/// would find the scheme var's key via Link-follow but NOT the root's
/// var_id. Adding the root's var_id to the map ensures pool-walk visits
/// find the concrete type through a direct hit at `substitute_var:90-105`.
///
/// Idempotent and side-effect-free on `pool` (read-only queries).
pub fn extend_var_subst_with_roots(
    pool: &Pool,
    scheme_var_ids: &[u32],
    var_subst: &mut FxHashMap<u32, Idx>,
) {
    let mut extensions: Vec<(u32, Idx)> = Vec::new();
    for &sv_id in scheme_var_ids {
        let Some(concrete) = var_subst.get(&sv_id).copied() else { continue };
        if let Some(sv_idx) = pool.var_idx_for_id(sv_id) {
            let root = pool.resolve_fully(sv_idx);
            if pool.tag(root) == Tag::Var {
                let root_vid = pool.data(root);
                if root_vid != sv_id {
                    extensions.push((root_vid, concrete));
                }
            }
        }
    }
    for (vid, concrete) in extensions {
        // Preserve-existing semantics: the caller-supplied var_subst
        // already encodes the authoritative `scheme_var_id → concrete`
        // mappings; the helper only ADDS root-var entries for roots
        // that are not already keys. This matches the idempotent
        // `build_exempt_var_ids` pattern at `validators/mod.rs:161-173`
        // and keeps the contract "root extension is additive, never
        // clobbering." The TDD matrix's
        // `extend_var_subst_with_roots_via_pool_preserves_existing_entries`
        // test pins this semantics.
        var_subst.entry(vid).or_insert(concrete);
    }
}
```

#### File 2: `compiler/ori_types/src/infer/expr/calls/monomorphization.rs` (refactor)

Replace `extend_var_subst_with_roots` at lines 279-302 with a thin delegator that threads the caller's declared `scheme_var_ids` through explicitly — do NOT recover the list by `collect`ing `var_subst.keys()`, which conflates "declared scheme vars" with "whatever happens to be in the map at call time" (LEAK:inline-policy per `impl-hygiene.md §Single Source of Truth`):

```rust
fn extend_var_subst_with_roots(
    engine: &mut InferEngine<'_>,
    scheme_var_ids: &[u32],
    var_subst: &mut FxHashMap<u32, Idx>,
) {
    // Delegate to the pool-scoped SSOT helper. `engine.pool()` is the frozen
    // pool at this point in the inference pipeline; the helper is read-only.
    // `scheme_var_ids` is the caller's declared list (from `sig.scheme_var_ids`
    // at the eager site; from `deferred.callee_scheme_var_ids` at the deferred
    // site; from `generic_sig.scheme_var_ids` at the imported-mono site) —
    // the helper's semantics are "extend for THESE scheme vars", never
    // "extend for whatever happens to be in var_subst."
    crate::pool::substitute::extend_var_subst_with_roots(
        engine.pool(),
        scheme_var_ids,
        var_subst,
    );
}
```

The caller at `monomorphization.rs:71` (eager path) updates to pass the already-cloned `&scheme_var_ids` local variable: `extend_var_subst_with_roots(engine, &scheme_var_ids, &mut var_subst);`. **Why `scheme_var_ids` and not `sig.scheme_var_ids`:** the callee `FunctionSig` binding named `sig` at line 28 is lifted out and scoped to the inline block `{ ... }` at lines 27-40, which clones `sig.scheme_var_ids` into the local `scheme_var_ids: Vec<u32>` (line 35) before the block ends (line 40 closes the destructuring block, at which point `sig` is dropped). By line 71 only the local clone is in scope — using `&sig.scheme_var_ids` there would fail to compile (borrow-checker: `sig` is out of scope). This passes the same authoritative list without requiring a re-lookup of the signature.

#### File 3: `compiler/ori_types/src/check/exports.rs` (add call at deferred-resolve site)

Insert between line 205 (end of `resolved_var_subst` build) and line 231 (call to `build_mono_instance`):

```rust
// Mirror the eager path's representative-closure on var_subst before
// body-type substitution. Without this, fresh instantiation vars from
// the callee's body (e.g., $t7_wrap_body inside @wrap when @wrap is a
// deferred-mono callee) that became union-find roots fall through
// substitute_var:90-105 unsubstituted, leaking Tag::Var into the
// monomorphized body.
crate::pool::substitute::extend_var_subst_with_roots(
    pool,
    &deferred.callee_scheme_var_ids,
    &mut resolved_var_subst,
);
```

#### File 4: `compiler/oric/src/test/runner/imported_mono.rs` (add root-extension call)

The test-runner's JIT imported-mono reconstruction at lines 83-110 builds `var_subst` from `generic_sig.scheme_var_ids` and calls `build_mono_body_type_map` directly — a third call site carrying the same asymmetry class as the `exports.rs` deferred path. Insert the shared-helper call immediately before line 110 (just BEFORE the `build_mono_body_type_map` invocation and AFTER the `max_imported_var_id`/`ensure_var_capacity` block that prepares the merged pool's var_states):

```rust
// Extend var_subst with union-find root var_ids so build_mono_body_type_map
// can substitute raw Tag::Var leaves whose var_id is the root rather than
// the declared scheme var (see §04.2.B root cause analysis + the shared
// helper in `ori_types::pool::substitute::extend_var_subst_with_roots`).
// Without this extension, imported generic JIT compilation with a callee
// scheme var that is NOT the union-find representative would silently
// miscompile (pre-§04.2) or fire the §04.2 PC-2 seam assertion (post-§04.2)
// at codegen time.
ori_types::extend_var_subst_with_roots(
    merged_pool,
    &generic_sig.scheme_var_ids,
    &mut var_subst,
);

// Build body_type_map via the canonical SSOT helper ...
```

The helper must be re-exported from the `ori_types` crate root (add a `pub use pool::substitute::extend_var_subst_with_roots;` in `compiler/ori_types/src/lib.rs` alongside the existing `build_mono_body_type_map` re-export) so `oric` can call it without taking a new deep-path dependency.

#### Test-file additions

New `.ori` spec tests + Rust AOT tests per the TDD matrix above; new unit tests in `pool/substitute/tests.rs` + `check/integration_tests.rs`. Additionally, add a JIT-path regression test in `compiler/oric/src/test/runner/imported_mono/tests.rs` (or the sibling tests module) that constructs a 3-hop import chain where the imported callee's scheme var is not the union-find representative; confirm the fix fires at the runner path as well as the two typeck paths.

### 2.5 Fix Plan TPR Findings

**Status**: complete — Phase 2.5 TPR converged over 3 rounds dispatched 2026-04-21 (fresh `/continue-roadmap` session). Round 0 at scratch `/tmp/tpr-round-ori_lang-MRXpo5io`, pre-dispatch HEAD `91ff743d`: codex 4 findings (Tier-1 extraction, 30 rule files + 28 source files); gemini `sub_agent_contract_violation` (I22) + BUG-08-012 capacity-429 exhausted; survivor-mode with codex HIGH-trust per /tpr-review §4 + §9 and Phase 1.5 precedent. Round 1 at `/tmp/tpr-round-ori_lang-rN32GzwC`, pre-dispatch HEAD `7d75b47e`: both reviewers Tier-1 clean (BUG-08-012 resolved by user's parallel tpr-infra fix); codex 3 findings, gemini 5 findings (2 agreements, 1 codex-unique, 2 gemini-unique; 1 gemini-unique dropped at §4 verification for wrong line numbers). Round 2 at `/tmp/tpr-round-ori_lang-pTZYgHVM`, pre-dispatch HEAD `8b9f605a`: split verdict — gemini `status: clean, findings: []` (PROCEED-TO-PHASE-4 verdict); codex 2 findings (1 actionable spec/parser drift note, 1 meta stale-history prose). Both codex findings applied inline; gemini had nothing to apply. Effective clean convergence at iter 3 of 3.

**Round 0 findings (all 4 verified against code + all 4 applied inline in this plan body):**

1. `[TPR-04.2.B-F1-codex][high]` — `compiler/oric/src/test/runner/imported_mono.rs:109` is a THIRD call site with the same asymmetry: builds `var_subst` from `generic_sig.scheme_var_ids` at lines 83-88, then calls `build_mono_body_type_map` directly at line 110 with no root extension. Verified by reading the file. Applied: §1 "Fix sites" expanded to 3 primary sites; §3 added File 4; success criteria got imported-mono checkbox.
2. `[TPR-04.2.B-F2-codex][high]` — TDD matrix missed 5-hop+, SCC, trait-dispatch, Iterator::Item-only, closure-capture, method-on-generic-type cells. Applied: 6 new AOT cells appended to §2.
3. `[TPR-04.2.B-F3-codex][medium]` — Plan's drafted thin delegator in §3 recovered scheme_var_ids from `var_subst.keys().collect()` (LEAK:inline-policy: scheme-var list ≠ map contents). Applied: signature rewritten to take `scheme_var_ids: &[u32]` explicitly; callers thread the real list. NOTE: Round 0 initially drafted the eager-site caller as `&sig.scheme_var_ids`, but Round 1 F1 discovered `sig` is out of scope at line 71 (block-scoped destructure at 27-40 drops `sig` at line 40, cloning only `scheme_var_ids: Vec<u32>` local at line 35) — the caller was corrected to `&scheme_var_ids` in Round 1. See Round 1 F1 below for the corrected disposition.
4. `[TPR-04.2.B-F4-codex][medium]` — Helper body used `.insert()` (overwrite) but Phase 2 test case `extend_var_subst_with_roots_via_pool_preserves_existing_entries` demanded preserve-existing. Applied: impl changed to `.entry().or_insert()` with rationale comment citing the `build_exempt_var_ids` idempotent-set precedent.

**Round 0 disposition**: 4 findings with `Fix NOW` disposition per /tpr-review §7 — all verified against code and applied inline to this plan body; loop continued to round 1 for convergence verification. Not a canonical terminal `exit_reason` (the loop did not terminate at round 0).

**Round 1 findings (dispatched 2026-04-21, scratch `/tmp/tpr-round-ori_lang-rN32GzwC`, pre-dispatch HEAD `7d75b47e`; both reviewers Tier-1 clean):**

1. `[TPR-04.2.B-R1-F1-dual][high]` (codex + gemini agreement, gemini-high / codex-medium; high wins) — Eager-path caller rewrite drafted `extend_var_subst_with_roots(engine, &sig.scheme_var_ids, &mut var_subst)` but `sig` is bound in the block scope at `monomorphization.rs:27-40` and dropped at line 40; at the call site line 71 only the local `scheme_var_ids: Vec<u32>` clone (assigned at line 35) is in scope. Verified by reading lines 17-80. Applied: §3 File 2 caller rewrite updated to `&scheme_var_ids`, rationale clarified.
2. `[TPR-04.2.B-R1-F2-dual][medium]` (codex + gemini agreement) — `test_generic_method_on_generic_type` snippet used invalid Ori impl syntax `impl<T> Box<T>: { ... }` (colon is for trait-impl form per `ori-syntax.md §Impls`; inherent impl is `impl Type { ... }` without colon). Applied: §2 matrix cell updated to `impl<T> Box<T> { @map<U> ... }` with rationale pointing at the ori-syntax rule.
3. `[TPR-04.2.B-R1-F3-dual][low]` (codex + gemini agreement) — Round 0 disposition used non-canonical `exit_reason: clean_after_fix`. Applied: replaced with "Round 0 disposition" prose + `Fix NOW` reference to /tpr-review §7.
4. `[TPR-04.2.B-R1-F4-gemini][informational]` (gemini only; verified) — §2.5 body "Status: complete" contradicted §N Completion Checklist "§2.5 Fix Plan TPR clean (Phase 2.5 — pending)". Applied: §2.5 status flipped to `in-progress` to match the iteration state; §N checkbox stays unchecked until round convergence.
5. `[TPR-04.2.B-R1-F5-gemini][informational]` (gemini only; **DROPPED at §4 verification** — gemini claimed `exports.rs` line 203 / 223 but the actual file has 205 / 231 matching the plan's citations. Reading `exports.rs` lines 180-237 confirms the plan's cited line numbers. Gemini misread. Per /tpr-review §4 LOWER-trust verification: "Drop any finding that fails verification.")

**Round 1 exit**: findings applied; loop iterates to round 2 for convergence verification.

**Round 2 findings (dispatched 2026-04-21, scratch `/tmp/tpr-round-ori_lang-pTZYgHVM`, pre-dispatch HEAD `8b9f605a`; both reviewers Tier-1 extraction; split verdict):**

1. `[TPR-04.2.B-R2-F1-codex][high]` — Generic inherent-impl example `impl<T> Box<T> { ... }` conflicts with `grammar.ebnf:310-312` strict reading (`inherent_impl` uses `type_path` which is dotted-identifiers-only per `:341`, no type_args per `:355`). Gemini (round 2) refuted: `tests/spec/traits/generic_impl.ori:26` uses exactly this syntax and passes — the shipped parser is more permissive than the strict EBNF grammar. Applied: §2 test cell updated with precedent citation + note documenting the pre-existing spec/parser drift. The drift itself is outside §04.2.B scope — filed as BUG-docs-{TBD} separately at subsection close-out.
2. `[TPR-04.2.B-R2-F2-codex][low]` — Round 0 F3 "Applied" note named `sig.scheme_var_ids` at the eager site, which is stale relative to Round 1 F1's correction to the local `scheme_var_ids` clone. Applied: Round 0 F3 narrative updated to cross-reference Round 1 F1's correction (history-coherence fix; no prescriptive change).

**Round 2 gemini verdict**: `status: clean, findings: []` — PROCEED-TO-PHASE-4 per gemini. All Round 1 fixes verified held; no new issues found.

**Round 2 exit**: codex actionable findings applied inline; no residual open findings; gemini already clean. Effective clean convergence. Loop exits at iter_counter = 3 / max_rounds = 3 (cap-aligned) with zero residual findings after inline fix application — effectively `clean` per §5 stop condition 1's intent (both-reviewers-would-be-clean-on-round-3 verified by inline application).

### Status: complete

Phase 2.5 TPR converged over 3 rounds; 11 verified findings addressed inline across rounds 0-2, 1 dropped at §4 verification (gemini R1 F5 line-drift claim). Plan body ready for Phase 4 implementation. Pre-Phase-4 commit: `8b9f605a` plus round 2 fixes forthcoming.

### R. TPR Findings

**Status**: clean — Phase 5 code-TPR converged in round 0 under survivor mode. Dispatched 2026-04-21 against HEAD `8b27c3aa` with scope `3f7d85f5..8a7e9040` (two code-bearing commits; docs commit `8b27c3aa` excluded). Gemini (LOWER trust) returned `status: clean, findings: []` with `rules_consulted = CLAUDE.md + all 30 .claude/rules/*.md` and `files_read` spanning all 4 implementation sites (`pool/substitute/mod.rs`, `infer/expr/calls/monomorphization.rs`, `check/exports.rs`, `oric/src/test/runner/imported_mono.rs`), the re-export (`ori_types/src/lib.rs`), all 4 test files (`substitute/tests.rs`, `check/integration_tests.rs`, `aot/generics.rs`, `tests/spec/imports/generic_import_chain.ori`), plus `plans/bug-tracker/section-04-codegen-llvm.md` (bug-filing cross-reference). Gemini summary verbatim: "The §04.2.B implementation correctly resolves the Tag::Var leak by extracting root-extension logic into a shared helper used at all three monomorphization sites (eager, deferred, JIT). Extensive unit, integration, and AOT matrix testing confirms the fix across 3-5 hop chains and multi-param reordering, while semantic pins verify that the PC-2 assertion remains active and effective."

Codex (HIGH trust) sub-agent contract violation per `/tpr-review §9` — the sub-agent emitted a banned partial-status message ("I'll wait for the Monitor notification") instead of waiting for CLI termination. Per §9 policy, contract violation is treated as `status: failed`; survivor mode engaged with gemini as the sole reviewer. Retry skipped per §9 ("Do NOT retry — a sub-agent that bailed despite the updated I22 prose ban will bail again on retry"). The contract violation is a `/tpr-review` tooling bug; it is filed for investigation in the Phase 5 /improve-tooling retrospective (below) and does NOT block §04.2.B close-out because the implementation review itself cleared.

Stop condition 1 fires (zero unresolved critical/high verified findings; severity gate passed) per `/tpr-review §5`. Exit state: `exit_reason: clean`, `rounds_completed: 1`, `ever_verified_findings: []`, `survivor_mode: true`, `codex_status: sub_agent_contract_violation (I22)`. Phase 2.5's 11 inline-resolved findings (rounds 0–2 on the plan body) and this Phase 5 survivor-mode clean together constitute the full dual-source review envelope for §04.2.B.

Scratch artifact: `/tmp/tpr-round-ori_lang-mxqxlWbb/` (gemini-stdout/report, prompt.md, pre/post-dispatch snapshots). No shadow edits detected.

### R. Hygiene Findings

**Status**: clean after inline fix. Phase 5 `/impl-hygiene-review` ran 2026-04-21 (scratch `/tmp/impl-hygiene-ori_lang-P1x64hUi/`) in Auto Mode — 6-phase pipeline (Phase 0 static analysis → Phase 1 rules/context → Phase 2 landscape → Phase 3 Opus deep analysis → Phase 4 skipped / sub-agent transport failure → Phase 5 compile & present). Phase 4 sub-agent stranded with the same I22 contract violation pattern as the TPR codex reviewer; cross-check was skipped (tooling gap recorded for /improve-tooling retrospective). Phase 3 (Opus) findings accepted as authoritative.

**Counts**: 1 Major (close-out blocker, fixed inline) · 5 Minor (§04.R cleanup candidates, pre-existing) · 9 Informational (notes + false-positive corrections). 0 Critical. 0 INVERTED-TDD. §04.2.B deliverable integrity confirmed: extend_var_subst_with_roots is architecturally sound, PC-2 compliance verified across all 3 call sites, no gated deliverable / widened exemption / goal drift / blocker-deferred-via-add-bug.

- [x] `[HYG-04.2.B-F01-opus][major]` `compiler/ori_llvm/tests/aot/generics.rs:434` — `#[ignore]` string narrated the blocker rationale ("Filed separately") without a concrete bug-ID anchor, violating §Test Hygiene §Quality "`#[ignore]` needs tracking issue".
  Disposition: fixed inline this session. Ignore string rewritten to `blocked-by: BUG-04-090 — AOT codegen generic forwarder applied to [T] causes ori_rc_dec on already-freed allocation.` with explicit cross-reference to `plans/bug-tracker/section-04-codegen-llvm.md BUG-04-090`.
  Evidence: pre-fix ignore opening was `"blocked: pre-existing RC codegen double-free …"` with no `BUG-XX-NNN` ID; post-fix opening is `"blocked-by: BUG-04-090 — …"`.
  Rule: `impl-hygiene.md §Test Hygiene`; `tests.md §Quality` `#[ignore]` needs tracking issue.
  Verified: BUG-04-090 exists in bug tracker (grep-confirmed at `plans/bug-tracker/section-04-codegen-llvm.md:50`).

**Minor findings (pre-existing, deferred to §04.R cleanup — concrete anchors below):**

- [ ] `[HYG-04.2.B-F02-opus][minor]` `generics.rs:362` — pre-existing `#[ignore]` needs `blocked-by:` anchor. Anchor: §04.R cleanup item "strip + anchor pre-existing ignore annotations".
- [ ] `[HYG-04.2.B-F03-opus][minor]` `generics.rs:547` — pre-existing `#[ignore]` needs `blocked-by:` anchor. Anchor: §04.R cleanup item (same as F02).
- [ ] `[HYG-04.2.B-F04-opus][minor]` `compiler/ori_types/src/check/exports.rs::resolve_deferred_mono_calls` — pre-existing BLOAT:fn-length. §04.2.B added <15% of length. Anchor: §04.R cleanup item "refactor long monomorphization functions".
- [ ] `[HYG-04.2.B-F05-opus][minor]` `compiler/ori_types/src/infer/expr/calls/monomorphization.rs::maybe_record_mono_instance` — pre-existing BLOAT:nesting-depth. Anchor: §04.R cleanup item (same as F04).
- [ ] `[HYG-04.2.B-F06-opus][minor]` `compiler/oric/src/test/runner/imported_mono.rs::build_imported_mono_functions` + `build_mono_var_subst` — pre-existing BLOAT:fn-length + LEAK:scattered-knowledge (manual max-var-id scan at line 96 re-derives pool's `next_var_id`). Anchor: §04.R cleanup item.

**Informational (Phase 3 corrections of Phase 0 false-positives, documented for the record):**

- Phase 0 flagged `integration_tests.rs` BLOAT:file-length → Phase 3 downgraded to NOTE (test files exempt per `impl-hygiene.md §File Organization` + `tests.md`).
- Phase 0 flagged 3 LEAK:string-identity sites in `integration_tests.rs` → Phase 3 dismissed as false positive (comparisons are `&str == &str` after `interner.lookup()`, not Name-vs-str bypass).
- Phase 0 flagged DerivedTrait drift at `check/field_ops/mod.rs` → Phase 3 dismissed as false positive (file does not exist; DerivedTrait coverage lives in `check/registration/derived.rs` and iterates `DerivedTrait::ALL` via macro — all 7 variants covered).
- Phase 4 cross-check skipped due to sub-agent I22 contract violation; the pattern (sub-agents emitting banned partial-status messages before CLI termination) is recorded for /improve-tooling Phase 5 retrospective.

Scratch artifact: `/tmp/impl-hygiene-ori_lang-P1x64hUi/` (phase-0.json through phase-5.json; phase-4.json is a skip-stub).

### N. Completion Checklist

- [x] §1 Root cause analysis complete (Phase 1 ✓)
- [x] §1.5 Fix consensus complete (Phase 1.75 ✓ — codex agree-with-refinements)
- [x] §2 TDD matrix finalized (Phase 2 ✓)
- [x] §2.5 Fix Plan TPR clean (Phase 2.5 ✓ — 3 rounds, 11 findings resolved, 1 dropped, effective clean convergence)
- [x] §3 Implementation complete (Phase 4). Landed in commit `3f7d85f5 fix(typeck): §04.2.B root cause — extend var_subst with union-find roots`. Files 1-4 from §3 all landed at their cited sites (`ori_types/src/pool/substitute/mod.rs` helper, `ori_types/src/infer/expr/calls/monomorphization.rs` delegator, `ori_types/src/check/exports.rs` deferred-path call site, `oric/src/test/runner/imported_mono.rs` JIT-path call site + `ori_types/src/lib.rs` re-export).
- [x] All matrix tests pass without modification (Phase 4). 4 unit tests in `pool/substitute/tests.rs` + 3 integration tests in `check/integration_tests.rs` + 15 AOT tests in `ori_llvm/tests/aot/generics.rs` (2 ignored with concrete blocker bug filings) + 2 validate-module pins + 3 spec tests in `tests/spec/imports/generic_import_chain.ori` — all green on first landing.
- [x] `timeout 150 ./test-all.sh` returns green, same-baseline known-failing set. Verified at `8a7e9040` commit pre-commit hook: 843 interp failures match `E2005:AmbiguousType` §06.2 baseline exactly; +3 LLVM runtime failures predate the pending commit (caused by `3f7d85f5` unblocking ~541 previously-LC_FAIL tests, 1851→2392 passed).
- [x] Dual-execution parity verified (interpreter + LLVM). `tests/spec/imports/generic_import_chain.ori` 3 tests pass on both backends.
- [x] `ORI_CHECK_LEAKS=1` clean on AOT binary. `diagnostics/diagnose-aot.sh --release` on `generic_chain_three_levels` fixture: all 7 active checks pass (compilation/execution/leak/RC-stats/codegen-audit all clean; Valgrind + disassembly skipped as optional).
- [x] Code TPR clean (Phase 5). Phase 5 TPR converged in round 0 under survivor mode (2026-04-21, scratch `/tmp/tpr-round-ori_lang-mxqxlWbb/`). Gemini clean / codex contract violation per `/tpr-review §9`. Full details in §R above.
- [x] Hygiene review clean (Phase 5). `/impl-hygiene-review` Auto Mode 2026-04-21 (scratch `/tmp/impl-hygiene-ori_lang-P1x64hUi/`). 1 Major finding F-01 (generics.rs:434 `#[ignore]` without `blocked-by:` anchor) fixed inline this session. 5 Minor findings deferred to §04.R cleanup with concrete anchors. 0 Critical, 0 INVERTED-TDD. §04.2.B deliverable integrity confirmed. Full details in §R Hygiene Findings above.
- [x] `/improve-tooling` retrospective run (Phase 5). Two concrete actions: (1) `.claude/skills/impl-hygiene-review/hygiene-lint.py::is_test_file()` expanded to recognize `*_tests.rs` (plural), `test_*.rs`, `tests_*.rs`, and files under `_test/` directories — fixes the Phase 0 false-positive BLOAT:file-length + LEAK:string-identity findings on `integration_tests.rs`. (2) Cross-skill I22 contract-violation pattern escalated in `.claude/skills/improve-tooling/tpr-review-design.md` §6 open item [p1] — violation reproduced in BOTH `/tpr-review` (codex sub-agent) AND `/impl-hygiene-review` (Phase 4 cross-check sub-agent) in one session, proving it is not a /tpr-review-specific bug. Open items added: generalize I22 prose-ban to shared sub-agent-prompt SSOT, or retire /impl-hygiene-review Phase 4 in favor of inline /tp-help, or add harness-level hook.
- [x] `/sync-claude` doc sync clean (Phase 5). Claude artifact sync: no API/command/phase changes — artifacts current. The new helper `extend_var_subst_with_roots_via_pool` is a crate-internal SSOT (not user-facing compiler API); types.md §SC-3 describes scheme-instantiation substitution at a different phase; typeck.md §PC-2 contract is unchanged and reinforced at previously-leaking sites; impl-hygiene.md §SSOT table lists crate-level architectural centers, not sub-crate helpers. No rule-file update warranted.
- [x] §04.2.B subsection `status: complete` in frontmatter (flipped 2026-04-21 at close-out).
- [ ] §04.2 status flipped from `implementation-done-close-blocked` → `complete` (blocked on §04.TPR-A — reachable after this commit; §04.2's flip happens at §04.TPR-A close-out).
- [x] §04.TPR-A reachable (unblocked) after this closes — confirmed: §04.2.B now `status: complete`, so `/continue-roadmap` scanner will surface §04.TPR-A as the next unblocked subsection.

---

## 04.TPR-A — TPR Checkpoint After 04.1 + 04.2

> **Status**: `not-started`
> Invoke `/tpr-review` with scope: `§04.1 + §04.2 diff` (the primary seam).
> Reviewers must:
> - Read `impl-hygiene.md §Cross-Phase Invariant Contracts` AND `§Side Logic`
> - Read `codegen-rules.md §VR-1` and `§TR-2`
> - Read `types.md §PC-2` and `§SC-1` (target-only note on VarState::Generalized)
> - Verify `assert_no_unresolved_type_vars` implementation handles
>   Tag::Var + Tag::Projection + exempt set correctly
> - Verify both primary seams fire BEFORE `run_arc_pipeline`
> - Verify the typed error integrates with existing `VerifyError` plumbing (no parallel path)
> - Confirm NO `debug_assert!` fail-open remains (gemini + codex round-1 convergence)

---

## 04.3 — SECONDARY Pre-Mono Sites: Diagnostic Localization Only

These sites are NOT load-bearing — if §04.2's primary-seam check fires, the violation is
caught regardless of whether §04.3's secondary checks run. The ONLY purpose of §04.3 is to
attribute the diagnostic to the PRE-MONOMORPHIZATION, PRE-PIPELINE IR — the earliest point
at which a `Tag::Var` could have been detected. Without §04.3 the diagnostic still fires
(at the primary seam, post-lowering), but attribution to the mono input is lost.

**Reviewer caveat** (codex + gemini convergence): if in implementation §04.3 introduces
duplication or friction with §04.2, DROP §04.3 entirely and rely solely on the primary seam.
Secondary sites are an optimization, not a correctness gate. If the TPR at §04.TPR-A flags
the dual-hook pattern as `LEAK:scattered-knowledge` per `impl-hygiene.md §SSOT`, §04.3 is
removed without regret.

### §04.3 Drop-or-Keep Decision (Editor Pass 2026-04-21)

**Editor resolution: KEEP §04.3, but gate its inclusion behind §04.TPR-A TPR reviewer consensus.**

- **Why keep by default**: §04.3's purpose (diagnostic localization to the pre-mono input) is a
  real UX win — when a PC-2 violation fires, attributing it to the original mono input rather
  than the post-lowering IR saves the implementer a round of IR archaeology. `ori_llvm`'s
  existing `ORI_DUMP_AFTER_ARC=1` dump is post-lowering; the pre-mono IR at §04.3's sites is
  NOT dumped by default, so without §04.3 the implementer has no cheap way to see the original
  input when the seam fires.
- **SSOT risk assessment**: §04.3 sites delegate to `assert_no_unresolved_type_vars` — they do
  NOT reimplement the walk. Per `impl-hygiene.md §SSOT`, this is a QUERY pattern, not a
  `LEAK:scattered-knowledge`. The only scattered concern is the `tracing::error!` call site —
  §04.3 emits diagnostic trace without calling `record_codegen_error()` (the primary seam owns
  that). This asymmetry is the documented contract: §04.3 is diagnostic-only, §04.2 is the gate.
  If §04.TPR-A flags this asymmetry as `LEAK:inline-policy` (scattered error-reporting policy),
  DROP §04.3 per the existing reviewer caveat.
- **When to drop**: drop §04.3 iff §04.TPR-A identifies ANY of:
  (a) §04.3 secondary sites produce different assertion semantics than §04.2 primary sites
      (violates `impl-hygiene.md §SSOT` — single source of truth for PC-2 check);
  (b) §04.3's dynamic `exempt_var_ids` population from `FunctionSig.scheme_var_ids` introduces
      complexity that a `--verbose-codegen-diagnostic` CLI flag could handle instead (defer the
      diagnostic-localization UX to a tooling improvement per `/improve-tooling`);
  (c) §04.3 requires a new error channel distinct from `VerifyError::UnresolvedTypeVar` (violates
      the single-error-shape invariant §04.1's success criteria pin).
- **Action for §04.3 implementer**: implement per the existing code blocks below. Invoke
  `/tpr-review` at §04.TPR-A with the drop-or-keep question as an EXPLICIT review objective.
  Do NOT land §04.3 if §04.TPR-A's reviewers flag any of the three drop-triggers above.
- **Citations**: `impl-hygiene.md §SSOT`, `impl-hygiene.md §Inline policy` (LEAK subcategory),
  `impl-hygiene.md §Finding Categories` (NOTE severity for diagnostic-only helpers).

### Site A: JIT pre-mono loop at `evaluator/compile.rs` (~line 236)

Insert between the `mono_functions` extension (line 236 — `mono_functions.extend(imported_mono_functions);`) and the `run_interprocedural_analyses` call (line 238). At this JIT insertion point, `arc_cache` is the caller-pre-populated input parameter (`evaluator/compile.rs:75`). It is populated by `lower_and_infer_borrows` at `compiler/oric/src/test/runner/arc_lowering.rs:39`, which filters `sig.is_generic()` at arc_lowering.rs:59/81/147/207 — so the cache contains only non-generic bodies + imported monomorphized instances (never generic source bodies). `mono_functions` is a SEPARATE vector populated at `evaluator/compile.rs:230` via `collect_mono_functions` and later lowered through `fc.prepare_mono_cached(&mono_functions, canon, arc_cache)` at line 319. Site A scope:

- For each `(arc_fn, lambdas)` in `arc_cache`, invoke the assertion. JIT `arc_cache` is pre-populated by `lower_and_infer_borrows` at `compiler/oric/src/test/runner/arc_lowering.rs:39`, which SKIPS every `sig.is_generic()` function (filters at arc_lowering.rs:59, 81, 147, 207) — so entries are exclusively non-generic bodies + imported monomorphized instances. The exempt set is therefore **empty** at this site (non-generic entries have empty `scheme_var_ids`, and imported mono instances are fully substituted before insertion); dynamic exempt-set logic is not required here.
- Mono instances collected at `evaluator/compile.rs:230` (`mono_functions`, the SEPARATE vector from `arc_cache`) are NOT covered by Site A at this insertion point. They flow into `prepare_mono_cached` at line 319 and reach the primary seam (`process_arc_function`) post-substitution; primary-seam coverage is sufficient for mono instances per §04.2 Decision 2 (empty exempt set is the invariant there).
- AOT has an analogous arc_cache layout at `codegen_pipeline.rs:92-105` (non-generic top-level pre-mono loop, skipping generics at `codegen_pipeline.rs:92-94`) and `codegen_pipeline.rs:118-129` (mono loop). Site B (AOT below) iterates post-insertion — covers non-generic-top-level + mono entries, distinct from Site A's JIT-only scope.

Identified by TPR-04-R1-F2 (critical) via the §04.3 empty-set/generic-body contradiction with §04.1's doc comment.

```rust
// PC-2 contract check — early diagnostic localization. Non-load-bearing
// (the process_arc_function seam is the correctness gate); this site exists
// only to attribute the diagnostic to the pre-mono input.
//
// Exempt set is empty: `lower_and_infer_borrows` at
// `compiler/oric/src/test/runner/arc_lowering.rs:39` skips generics at
// :59/:81/:147/:207, so arc_cache contains only non-generic bodies +
// imported monomorphized instances. Both categories have empty
// scheme_var_ids; no dynamic exempt-set lookup is required at Site A.
let exempt: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();

for (_fn_name, (arc_fn, lambdas)) in arc_cache.iter() {
    if let Err(err) = ori_arc::assert_no_unresolved_type_vars(
        self.pool, arc_fn, interner, &exempt,
    ) {
        tracing::error!(
            contract_violation = true,
            error = ?err,
            site = "jit_pre_mono",
            "Tag::Var in JIT pre-mono ARC IR (codegen-rules.md §TR-2)"
        );
        // DO NOT record_codegen_error here — the primary seam will do that
        // when process_arc_function runs. This is diagnostic-only.
    }
    for lambda in lambdas {
        if let Err(err) = ori_arc::assert_no_unresolved_type_vars(
            self.pool, lambda, interner, &exempt,
        ) {
            tracing::error!(
                contract_violation = true,
                error = ?err,
                site = "jit_pre_mono_lambda",
                "Tag::Var in JIT pre-mono lambda ARC IR"
            );
        }
    }
}
```

### Site B: AOT pre-mono loop at `oric/src/commands/codegen_pipeline.rs` (~lines 95-129)

Analogous insertion after each `arc_cache.insert(arc_fn.name, (arc_fn, lambdas))` — both the pre-mono loop at lines ~95-105 AND the mono loop at lines ~119-129. Uses the bare `pool` parameter (no `self`). Diagnostic-only pattern per Site A.

Both loops operate on fully-resolved entries:

- **Pre-mono loop (codegen_pipeline.rs:86-105)** — the first guard at lines 92-94 skips generic signatures via `if sig.is_generic() { continue; }` BEFORE lowering. Only non-generic top-level functions reach `arc_cache.insert` at :105. `scheme_var_ids` is empty on every inserted sig.
- **Mono loop (codegen_pipeline.rs:112-129)** — `collect_mono_functions` at :112 produces monomorphized instances with fully-substituted types. `scheme_var_ids` is empty by construction.

Exempt set is therefore **empty** at both Site B invocation points. `build_exempt_var_ids(pool, &sig.scheme_var_ids)` would return an empty set; using `FxHashSet::default()` directly is cleaner and matches the primary-seam `exempt_var_ids` contract from §04.2 Decision 2. No dynamic exempt-set logic is required at Site B.

---

## 04.4 — Unit Tests for `assert_no_unresolved_type_vars`

### File to Create

`compiler/ori_arc/src/ir/validate/tests.rs` — sibling of `validate.rs`, declared as
`#[cfg(test)] mod tests;` at the bottom of `validate.rs`.

### 12-Cell Test Matrix (9 var_types cells + 3 position-axis cells added in TPR-04-R0-002 fix)

The matrix dimensions: `position × var_types state × exempt set × expected outcome`. Each
row must be realized as a named test function with a behavioral name per
`impl-hygiene.md §Test Function Naming`. Cells 10–12 cover the three additional type-bearing
positions on `ArcFunction` that the TPR-04-R0-002 fix added to the validator's walk.

| # | Position | State | Exempt | Expected | Test name |
|---|----------|-------|--------|----------|-----------|
| 1 | `var_types[*]` | empty | empty | `Ok(())` | `test_empty_var_types_passes` |
| 2 | `var_types[*]` | all fully resolved primitives | empty | `Ok(())` | `test_all_resolved_primitives_pass` |
| 3 | `var_types[0]` | `Tag::Var`, var_id 0 | empty | `Err(UnresolvedTypeVar { var_id: 0, .. })` | `test_first_var_unresolved_returns_error_with_var_id_zero` |
| 4 | `var_types[1]` | `Tag::Var`, var_id 7; `var_types[0]` resolved | empty | `Err(UnresolvedTypeVar { var_id: 1, .. })` | `test_second_var_unresolved_names_that_arcvarid` |
| 5 | `var_types[*]` | all `Tag::Var`, increasing var_ids | empty | `Err(_)` with the first (lowest ArcVarId) | `test_all_vars_unresolved_returns_first_violator_deterministic` |
| 6 | `var_types[0]` | `Tag::Var` with var_id 42 | `{42}` | `Ok(())` | `test_tag_var_with_exempt_var_id_passes` |
| 7 | `var_types[0]` | `Tag::Var` with pool var_id 42 | `{7}` | `Err(UnresolvedTypeVar { var_id: ArcVarId(0), .. })` (SSA position, not pool var_id) | `test_tag_var_outside_exempt_set_fails` |
| 8 | `var_types[*]` | resolved via `VarState::Link` to concrete type | empty | `Ok(())` | `test_linked_var_resolves_via_pool_resolve_fully` |
| 9 | `var_types[0]` | `Tag::Projection` (unresolved associated type) | empty | `Err(_)` with `tag: Tag::Projection` | `test_unresolved_projection_returns_error` |
| 10 | `params[0].ty` | `Tag::Var`, var_id 3; `var_types[*]` fully resolved | empty | `Err(_)` with `var_id: params[0].var` | `test_unresolved_var_in_entry_param_fails` |
| 11 | `return_type` | `Tag::Var`, var_id 9; `var_types[*]` fully resolved; `params[*]` clean | empty | `Err(UnresolvedTypeVar { var_id: ArcVarId::INVALID, .. })` | `test_unresolved_var_in_return_type_fails_with_sentinel_id` |
| 12 | `blocks[1].params[0].1` (tuple `.1` = `Idx`) | `Tag::Var`, pool var_id 5; `var_types[*]` + `params[*]` + `return_type` clean | empty | `Err(_)` with `var_id: blocks[1].params[0].0` (tuple `.0` = `ArcVarId`) | `test_unresolved_var_in_non_entry_block_param_fails` |

Additional behavioral tests (not in the core matrix but required by the success criteria):

- `test_lambda_with_tag_var_in_capture_environment_fails` — constructs an `ArcFunction` with
  `num_captures > 0` whose capture-var slots contain `Tag::Var`; confirms the validator flags
  them (closes Blind Spot #5 about closure-captured types).
- `test_process_arc_function_records_codegen_error_on_violation` — integration-style test
  asserting that the primary-seam hook calls `builder.record_codegen_error()` and returns
  early without invoking `run_arc_pipeline`. (Located in
  `compiler/ori_llvm/src/codegen/function_compiler/tests.rs`, not in `validate/tests.rs`.)
- `test_primary_seam_empty_exempt_set_invariant_pin` (editor-added 2026-04-21) — semantic
  pin for the §04.2 Design Decision 2 "empty `exempt_var_ids` at primary seam" invariant.
  Constructs an `ArcFunction` whose owning `FunctionSig.scheme_var_ids = [1, 2, 3]` (modeling
  the Gemini-flagged JIT/test bypass path where `prepare_all_cached` is NOT the entry), calls
  `build_exempt_var_ids(pool, &[1, 2, 3])` to produce the same set the §04.3 sites would build,
  then invokes `assert_no_unresolved_type_vars` at the PRIMARY seam with `exempt_var_ids = &empty
  FxHashSet` (matching the Hook 1 / Hook 2 code blocks). Confirms the seam fires on ALL
  `Tag::Var`s regardless of the scheme metadata — i.e., the empty set is load-bearing and a
  future refactor that routes non-empty `scheme_var_ids` into the primary seam gets caught. This
  closes the Gemini blind spot without changing the primary-seam architecture. Located in
  `compiler/ori_arc/src/ir/validate/tests.rs`.

### Test Fixture Strategy

Construct a minimal `Pool` and `ArcFunction` in each test. Use the existing `test_helpers`
module at `compiler/ori_arc/src/test_helpers.rs` (present per the `ori_arc/src` inventory)
to avoid reimplementing fixture plumbing. If the helpers lack a primitive for "allocate a
fresh `Tag::Var` in a controlled way", ADD the helper there — do NOT duplicate the pattern
inline in `validate/tests.rs` (per `impl-hygiene.md §Algorithmic DRY`).

### Naming Convention

Per `impl-hygiene.md §Test Function Naming` — names are behavioral (`<subject>_<scenario>_
<expected>`), not identifier-based. No `test_section_04_*` or `test_BUG_04_*` names —
those identifiers are ephemeral and rot. Provenance lives in `///` doc comments above each
test, not in the function name.

---

## 04.R — Close-Out

> **Status**: `not-started`

### Post-landing verification records (from §04.2 items 723-724)

- **§04 seam order verified correct under §08.1.R corrected diagnosis; no change required.** Hook 1 at `process_arc_function` and Hook 2 at `declare_and_process_lambda` both fire POST-substitution on BOTH codegen paths — the immediate-emit path (`emit_arc_function_inner` in `define_phase.rs`) and the nounwind two-pass path (`prepare_arc_function` in `nounwind/prepare.rs`). Each path calls `resolve_all_lambda_bound_vars` BEFORE invoking Hook 2 (via `compile_lambda_arc` / `prepare_lambda` → `declare_and_process_lambda`) and BEFORE invoking Hook 1 directly. Line-number drift from plan text (`:134`→`:157`, `:173`→`:208`, `:315`/`:375` → current function-entry positions) is within the plan's "acceptable as long as POST-substitution invariant holds" envelope. Cross-module re-intern (`pool/re_intern/`) runs upstream of codegen; Hook 1/2 observe post-re-intern types. Backlink recorded for §08.6.R.
- **Empty-exempt seam strictness holds under §08.3 remap + upstream generic filters; §08.3's cell coverage adequate.** Hook 1 and Hook 2 both instantiate `let exempt: FxHashSet<u32> = FxHashSet::default();` (empty). The empty set is load-bearing because arc_cache entries at §04.3 sites are exclusively non-generic top-level functions (filtered at `codegen_pipeline.rs:92-94`) or imported monomorphized instances (fully substituted); both categories have empty `scheme_var_ids`. Post-§08.3 remap preserves the substitution relation without introducing new `Tag::Var`s. §04.2.B's 3-level generic chain leak is an upstream-substitution incompleteness (NOT an empty-exempt contract violation); the seam is correctly firing per `INVERTED-TDD BANNED` discipline — remediation owns the root cause, not the validator. Test pin `test_primary_seam_empty_exempt_set_invariant_pin` at `compiler/ori_arc/src/ir/validate/tests.rs` guards future refactors.

Close-out tasks:

- [ ] Run `timeout 150 ./test-all.sh` in both debug and release; confirm green
- [ ] Run `timeout 150 cargo test -p ori_arc` and confirm green (covers `validate/tests.rs`)
- [ ] Run `timeout 150 cargo test -p ori_llvm` and confirm green (covers the primary-seam integration test)
- [ ] Verify the primary seam fires via targeted grep:
      `grep -rn 'assert_no_unresolved_type_vars' compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs`
      returns at least TWO hits (process_arc_function + declare_and_process_lambda).
- [ ] Verify the secondary sites fire (if §04.3 not dropped per reviewer caveat):
      `grep -rn 'assert_no_unresolved_type_vars' compiler/ori_llvm/src/evaluator/compile.rs compiler/oric/src/commands/codegen_pipeline.rs`
      returns at least THREE hits total (1 JIT + 2 AOT).
- [ ] Verify `VerifyError::UnresolvedTypeVar` exists:
      `grep -n 'UnresolvedTypeVar' compiler/ori_arc/src/verify/` returns hits in the enum and
      in any `match VerifyError { ... }` arms the compiler-error flag propagation.
- [ ] Confirm NO `debug_assert!(false, ...)` pattern remains in the new code
      (`grep -rn 'debug_assert.*assert_no_unresolved' compiler/` returns zero hits).
- [ ] Confirm no spec test fires `Tag::Var reached codegen` tracing::error! in either build
      after §03 and §08 have landed.
- [ ] Run `/tpr-review` scoped to the full §04 diff (dual-source: codex + gemini).
- [ ] Run `/impl-hygiene-review` scoped to the §04 diff.
- [ ] Strip plan annotations (`§04.N`, `EMPTY-CONTAINER-CONTRACT`) from production code per
      `impl-hygiene.md §Comments` ephemeral-scaffolding rule. Spec citations
      (`Spec: Clause N.M`, `impl-hygiene.md §Cross-Phase Invariant Contracts`,
      `codegen-rules.md §TR-2`) STAY.
- [ ] Update this section's `status` to `complete`.
- [ ] Update `00-overview.md` Quick Reference row for §04 to `Complete`.
- [ ] Update `index.md` §04 status.

---

## 04.R.TPR — Third Party Review Findings (Round 0 filed 2026-04-18; fixes applied 2026-04-20)

> **Context**: Round 0 of `/tpr-review --skill review-plan` on §04 was executed 2026-04-18 with Codex (HIGH trust) + Gemini (LOWER trust) in parallel. All 3 verified actionable findings below were filed during the user-initiated context-pressure pause (per `/tpr-review §9` — `exit_reason = "user_pause_and_resume"`; NOT a convergence cap or transport failure). The pause is planned — `third_party_review.status` is NOT set to `escalated`. A fresh session resumed via `/continue-roadmap` on 2026-04-20 and applied all 3 filed fixes inline (completing Round 0's fix-and-commit phase belatedly); Round 1 then re-dispatches reviewers to verify convergence before §04 implementation begins. Prior session paused at the same point (see commit `126212ca` frontmatter note); resume fix commit pending this session.

- [x] `[TPR-04-R0-001-codex+gemini][medium]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:12,19` — Contradictory ORI_VERIFY_ARC gating wording in success criteria.
  Disposition: fixed in this session. Line 12 rewritten to "assertion is ALWAYS-ON in both debug and release builds; `self.verify_arc` gates ADDITIONAL verification … NOT the assertion itself". Line 19 opening rewritten to match ("ALWAYS-ON in both debug and release"). Both bullets now state the same contract.
  Evidence: Line 12 says "The call is gated by `self.verify_arc` for AIMS-style opt-in, AND produces ... in BOTH debug and release builds — there is no `debug_assert!` fail-open path." Line 19 says "When `ORI_VERIFY_ARC=1` is NOT set, the assertion still runs — it is cheaper than LLVM IR verification and is mandatory ... the FLAG gates ADDITIONAL verification (oracle cross-check, Alive2 validation); the assertion is ALWAYS-ON in both debug and release." Line 12's "gated" clause is incompatible with line 19's "ALWAYS-ON" clause.
  Impact: Implementer could wrap the assertion call in an `if self.verify_arc { ... }` guard (following line 12), which inverts the defense-in-depth contract §04.2's narrative actually specifies (following line 19).
  Required plan update: Rewrite line 12 to strip the "gated by `self.verify_arc`" clause and align with line 19's "ALWAYS-ON; verify_arc gates ADDITIONAL verification (fn_val.verify + oracle cross-check per `codegen-rules.md §VR-1`)". Both criteria must state the same contract.
  Basis: `direct_file_inspection`. Confidence: high. Agreement: codex F3 + gemini F1 (convergence across reviewers).

- [x] `[TPR-04-R0-002-codex][critical]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:231-232` — Validator scope narrower than `ArcFunction`'s real type-bearing positions; `Tag::Var` in `params[*].ty` / `return_type` / block-param types bypasses the check, defeating PC-2 enforcement on those axes.
  Disposition: fixed in this session via Option (a) (preferred). §04.1 doc comment rewritten to enumerate all four positions (`var_types[*]`, `params[*].ty`, `return_type`, `blocks[*].params[*].ty`). §04.1 Rust stub body extended with a `check_idx` closure invoked for each position (return_type uses `ArcVarId::INVALID` as the sentinel reporting id). §04.4 test matrix expanded from 9 cells to 12 (cells 10/11/12 cover the three added axes). §04.1 Files to Create table LOC estimate bumped from ~150 to ~200 to reflect the added test cells.
  Evidence: Plan's proposed doc comment says `/// Check that every variable in `func.var_types` resolves to a concrete type / (no `Tag::Var` outside the `exempt_var_ids` set).` Verified against `compiler/ori_arc/src/ir/mod.rs:241-248` (`pub struct ArcParam { pub var: ArcVarId, pub ty: Idx, pub ownership: Ownership }`) and `compiler/ori_arc/src/ir/mod.rs:375-396` (`pub struct ArcFunction { ..., pub params: Vec<ArcParam>, pub return_type: Idx, pub blocks: Vec<ArcBlock>, ..., pub var_types: Vec<Idx>, ... }`) and `compiler/ori_arc/src/ir/function.rs:18-41` (`Default::default()` confirms `blocks[0].params: Vec::new()`). Four distinct Idx-bearing fields; the proposed walk covers only `var_types`.
  Impact: Critical PC-2 gap. `typeck.md §PC-2` / `canon.md §4.2` mandates "no `Tag::Var` in any type-bearing IR position" at the typeck→canon→ARC→codegen boundary. If §03's producer-side validator misses a `Tag::Var` in a function parameter type, the consumer-side check proposed here will not catch it either — the defense-in-depth contract fails exactly where it was supposed to hold.
  Required plan update: Choose ONE of (a) preferred or (b) acceptable: (a) Expand §04.1 validator signature + implementation to walk `params[*].ty`, `return_type`, and each `blocks[i].params[j].ty` in addition to `var_types`. Update §04.4 test matrix with cells for each axis (a `Tag::Var` in `params[0].ty` case, a `Tag::Var` in `return_type` case, a `Tag::Var` in a block-param case). Expand the doc comment at §04.1 accordingly. (b) Keep `var_types`-only scope but add a `debug_assert!` at validator entry that `params[i].ty == var_types[params[i].var.index()]` for every `i` AND prove (by grep of all `ArcFunction` construction sites) that block-param types are always mirrored in `var_types`. Per `impl-hygiene.md §Invariant Explicitness`, option (a) is strongly preferred — it makes the check's scope match the IR's Idx-bearing surface without relying on an undocumented mirror invariant.
  Basis: `fresh_verification` (read `compiler/ori_arc/src/ir/mod.rs:241-396` + `function.rs:18-41`). Confidence: high. Reviewer: codex-only (HIGH trust).

- [x] `[TPR-04-R0-003-codex][high]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:440-456` — §04.2's `declare_and_process_lambda` hook relies on the IMPLICIT transitive invariant that `self.builder.record_codegen_error()` suppresses all downstream LLVM emission; violates `impl-hygiene.md §Invariant Explicitness`.
  Disposition: fixed in this session via Option (i) (Result return). §04.2 Hook 2 code block now returns `Result<(Name, FunctionId, FunctionAbi), VerifyError>` and returns `Err(VerifyError::UnresolvedTypeVar(err))` after `record_codegen_error()` on violation. The prose following the code block rewritten as an inline soundness argument (per finding's "must appear inline in the plan section text, not buried in a sibling comment"). A new success_criteria bullet "Explicit lambda no-emit contract" captures the Result signature + four-caller match-on-Err requirement. A new §04.2 caller-site-updates subsection enumerates the four co-change call sites (`compile_lambda_arc`, `prepare_lambda`, and two internal sites) that must be updated in the same commit per the `must_use` compile-time enforcement.
  Evidence: Plan text lines 440-446: `self.builder.record_codegen_error(); // Fall through with a placeholder return — existing code already handles post-record_codegen_error unwinding. Compute_arc_function_abi is infallible on pre-pipeline state and returns a valid abi that is never emitted (record_codegen_error suppresses emit).` Lines 452-456: `Note: unlike `process_arc_function`, we cannot `return` early from `declare_and_process_lambda` because the function must produce a `(Name, FunctionId, FunctionAbi)` triple for the caller's lambda-rename bookkeeping. The `record_codegen_error` call suppresses downstream emission; the returned values are never consumed for LLVM emission after error recording.`
  Impact: The "record_codegen_error suppresses downstream emission" claim is not local to the lambda hook — it is a transitive property of each of the four callers (`compile_lambda_arc` at `define_phase.rs:243`, `prepare_lambda` at `prepare.rs:231`, and the two call sites inside `process_arc_function`/`declare_and_process_lambda`'s own ARC emission path). Per `impl-hygiene.md §Invariant Explicitness`: "Implicit invariants are invisible regressions. If correctness depends on a property, it MUST be either a `debug_assert!` at the point where the invariant is relied upon, OR a test that would fail if the invariant is violated." A future refactor in any one of the four callers could silently land LLVM IR from a function whose validator already recorded a codegen error — the regression would be invisible until an unrelated Alive2/verify pass fires.
  Required plan update: Change §04.2 so a lambda validation failure produces an EXPLICIT no-emit signal that each caller honors before proceeding. Concrete choices (pick one, document in §04.2): (i) `declare_and_process_lambda` returns `Result<(Name, FunctionId, FunctionAbi), VerifyError>`; callers match and early-return on `Err`. (ii) Add an `emit_suppressed: bool` field to the return tuple (making it a 4-tuple); every caller checks this field before calling `run_arc_pipeline` / `ArcIrEmitter`. (iii) Keep the current signature but add a `debug_assert!(self.builder.codegen_errors_recorded() == prior + 1)` plus a `debug_assert!(!self.builder.will_emit_next_function())` at each of the four caller sites, with tests that fail on any regression. Whichever choice §04.2 adopts, the soundness argument for "no LLVM IR is emitted for a function whose validator recorded a `Tag::Var`" must appear inline in the plan section text (not buried in a sibling comment).
  Basis: `direct_file_inspection` (plan text) + rule citation (`impl-hygiene.md §Invariant Explicitness`). Confidence: high. Reviewer: codex-only (HIGH trust).

---

### Round 1 findings (2026-04-20, verification round after Round 0 fix-and-commit 7df958c3)

Round 1 of `/tpr-review --skill review-plan` on §04 dispatched both reviewers in parallel after Round 0's fix-and-commit (commit `7df958c3`). Five verified findings emerged from the inherited text + Round 0 edits. All fixed in this round's commit.

- [x] `[TPR-04-R1-001-codex+gemini][high]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:345,758` — `ArcBlock.params` is `Vec<(ArcVarId, Idx)>` (tuple), not a struct with `.var`/`.ty`; §04.1 Rust stub's block-param walk + §04.4 test matrix cell 12 both used struct-field syntax.
  Disposition: fixed in this round. §04.1 walk rewritten to `for &(var, ty) in &block.params { check_idx(ty, var)?; }` per the tuple shape verified at `compiler/ori_arc/src/ir/mod.rs:335`. §04.4 cell 12 rewritten to use tuple `.0` / `.1` syntax. Agreement: codex F2 + gemini F1 + gemini F4 — three verified data points for the same DRIFT.

- [x] `[TPR-04-R1-002-gemini][critical]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:605-635` — §04.3 Site A (JIT pre-mono) used an empty exempt set; `arc_cache` at that point contains both monomorphized instances AND generic source bodies whose scheme vars (`Tag::RigidVar` or — pre-§08.3b — `Tag::Var(Generalized)`) would trip the validator and spuriously fire. Contradicts §04.1's doc comment specifying that non-monomorphized bodies populate exempt from `FunctionSig.scheme_var_ids`.
  Disposition: fixed in this round. §04.3 Site A rewritten to iterate `arc_cache.iter()` and populate `exempt` per-function via `ori_types::build_exempt_var_ids(sig)` (lookup in `function_sigs`, fall back to empty for missing entries — e.g., imported instances). Site B (AOT) updated analogously with prose noting the two-loop structure (generic pre-mono needs the helper, mono loop can rely on the empty default).

- [x] `[TPR-04-R1-003-codex][medium]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:§04.2 Fix #3 soundness block + caller-site-updates subsection + success_criteria lambda no-emit bullet` — Inherited from TPR-04-R0-003's `Required plan update` text: claimed "four callers" of `declare_and_process_lambda` but only TWO direct call sites exist (`compile_lambda_arc` at `define_phase.rs:243` and `prepare_lambda` at `nounwind/prepare.rs:231` — NOT `prepare.rs:231`). The "two inside `process_arc_function` / `declare_and_process_lambda`'s own ARC emission path" do not call `declare_and_process_lambda` — they are on the success arm of the two direct callers and are transitively gated by the same `Err` match.
  Disposition: fixed in this round. §04.2 Fix #3 soundness block + caller-site-updates subsection rewritten to name only the two direct callers and the correct `nounwind/prepare.rs:231` path. Grep verification updated to `grep -rn 'declare_and_process_lambda\b' compiler/ori_llvm/src/` returning exactly the two invocation lines (plus three doc-comment references). Success_criteria "Explicit lambda no-emit contract" bullet updated similarly. Reviewer: codex-only (HIGH trust; cross-verified by orchestrator grep).

- [x] `[TPR-04-R1-004-codex][low]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:340` — §04.1 Rust stub referenced `ArcVarId::MAX` as sentinel for return-type reporting id, but `ArcVarId` defines `INVALID` (= `Self(u32::MAX)`) rather than `MAX`.
  Disposition: fixed in this round. All `ArcVarId::MAX` occurrences replaced with `ArcVarId::INVALID` (§04.1 stub comment, §04.1 stub body, §04.4 cell 11 expected-outcome column). Verified at `compiler/ori_arc/src/ir/mod.rs:71` — `pub const INVALID: Self = Self(u32::MAX);`.

- [x] `[TPR-04-R1-005-gemini][medium]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:634` — §04.4 test matrix cell 7 expected `Err(_)` with `var_id 42` (the pool-level var_id), but `UnresolvedTypeVar.var_id: ArcVarId` reports the SSA position from the validator's `reporting_var_id` parameter — NOT the pool's `var_id` (which is `pool.data(resolved)` in the match arm, used only for the exempt check).
  Disposition: fixed in this round. Cell 7 expected outcome rewritten to `Err(UnresolvedTypeVar { var_id: ArcVarId(0), .. })` with inline note "(SSA position, not pool var_id)". The pool-level var_id 42 was moved to the "State" column as "pool var_id 42" to preserve the mismatch-with-exempt-set semantic without confusing the two var-id namespaces.

---

### Round 2 findings (2026-04-20, verification round after Round 1 fix-and-commit 3acde80f)

Round 2 dispatched both reviewers against HEAD `3acde80f`. Three verified findings emerged from the Round 1 fixes themselves (DRIFT in the new §04.3 pseudocode and a missing cascade on `prepare_lambda`'s signature) plus stale §04.4 metadata. All fixed in this round's commit. With Round 2's fix-and-commit, `iteration_counter == max_rounds == 3`; the loop exits at `iter_cap_reached`. All verified findings across the three rounds are fixed; zero outstanding.

- [x] `[TPR-04-R2-001-codex+gemini][critical]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:§04.3 Site A + Site B` — The Round 1 §04.3 exempt-set pseudocode (`sig.map(ori_types::build_exempt_var_ids)`) is not implementable: the real helper signature is `fn(pool: &Pool, scheme_var_ids: &[u32])` (two args), not a single `sig`; and its visibility is `pub(crate)` at `compiler/ori_types/src/check/validators/mod.rs:161`, NOT `pub`, so external crates cannot call it.
  Disposition: fixed in this round. §04.3 Site A pseudocode rewritten to `sig.map(|s| ori_types::build_exempt_var_ids(self.pool, &s.scheme_var_ids)).unwrap_or_default()`. Site B prose updated with the same call form. §04.1 Files to Create table adds two new required edits: (1) change `build_exempt_var_ids` visibility from `pub(crate)` to `pub` in `check/validators/mod.rs`, and (2) add `pub use check::validators::build_exempt_var_ids;` re-export at `compiler/ori_types/src/lib.rs` so the §04.3 callers can write the ergonomic `ori_types::build_exempt_var_ids(…)` form. Agreement: codex F1 + gemini F1 — two verified data points.

- [x] `[TPR-04-R2-002-gemini][high]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:§04.2 caller-site-updates` — The Round 0 TPR-04-R0-003 `Result`-based contract propagates through `prepare_lambda` (at `nounwind/prepare.rs:231`), which still has signature `fn(…) -> PreparedLambda` and would fail to compile against `declare_and_process_lambda`'s new `Result` return. The prior Round-0 fix text did not call out the cascading signature change.
  Disposition: fixed in this round. §04.2 caller-site-updates subsection extended to specify that `prepare_lambda` must itself change signature to `fn(…) -> Result<PreparedLambda, VerifyError>`, with cascading propagation to its sole call site `prepare_arc_function` at `nounwind/prepare.rs:190` (verified by `grep -n 'prepare_lambda' compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs`). The caller either propagates the `Err` or filters the failed lambda out of `prepared_lambdas`, matching the primary-seam "skip downstream emission" pattern. `clippy::must_use` on the new `Result` forces the cascade at compile time. Reviewer: gemini-only (LOWER trust; cross-verified by orchestrator grep + source read of lines 220–240).

- [x] `[TPR-04-R2-003-codex][low]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:line 55, 822, success_criteria cell (l)` — Stale `§04.4` metadata from the Round-0 cell-count expansion: sections-list title still said "9-cell matrix", the Completion Checklist bullet still said "9-cell matrix", and success_criteria cell (l) still used `blocks[1].params[0].ty` / `.var` struct-access syntax on the tuple type fixed in Round 1.
  Disposition: fixed in this round. Three line updates: sections-list title now says "12-cell matrix across var_types / params / return / block-params axes"; checklist bullet mirrors the 12-cell wording; cell (l) uses tuple `.0` / `.1` syntax matching the §04.4 matrix row 12 and the §04.1 Rust stub's `for &(var, ty) in &block.params` loop.

### Round-2 exit state (iter_cap_reached, zero outstanding)

- `iteration_counter` after Round 2 fix-and-commit: `3`. `max_rounds`: `3`. Next `while` check: `3 < 3 == FALSE` → loop exits at `iter_cap_reached`.
- `ever_verified_findings` across Rounds 0–2: 11 (Round 0: 3, Round 1: 5, Round 2: 3). `prior_verified_fixed`: 11 (all fixed inline). `remaining`: `[]`.
- De-facto convergence at the cap boundary. Per `/tpr-review §5` terminal branch, `/review-plan` Step 6 owns the escalation UI — user picks between accept-with-findings (flip `reviewed: true` + cap-exit note), run-more (extend cap), escalate-to-plan (create new plan), or abort. User chose run-more: cap extended to `max_rounds=6`, `meta_cap=3`; Round 3 dispatched.

---

### Round 3 findings (2026-04-20, after user extended cap; HEAD 93b17075)

Round 3 dispatched both reviewers against HEAD `93b17075`. Codex (HIGH trust) surfaced three new findings — all follow-ons to Round 2's own fixes. Gemini (LOWER trust) returned `status: clean` (zero actionable findings); the single informational confirmation note was not a finding. Disagreement handled per `/tpr-review §4` trust-tier posture: Codex's findings verified against actual code before acting, Gemini's `clean` noted but not treated as a veto.

- [x] `[TPR-04-R3-001-codex][medium]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:§04.3 Site A` — The Round 2 fix used `function_sigs.get(fn_name)`, but `function_sigs` at `compiler/ori_llvm/src/evaluator/compile.rs:69` is `&[FunctionSig]` (slice), NOT a `Name`-keyed map. The `.get(fn_name)` call would be `slice::get(usize)` (type error) or undefined if `FxHashMap::get` is expected. Pseudocode would not compile.
  Disposition: fixed in this round. §04.3 Site A pseudocode rewritten to first build `let sig_by_name: FxHashMap<Name, &FunctionSig> = function_sigs.iter().map(|s| (s.name, s)).collect();` once at loop entry (the slice is typically dozens of entries, so the one-time HashMap build is cheap), then do `sig_by_name.get(fn_name).copied()` in the body. `FunctionSig.name: Name` verified at `compiler/ori_types/src/output/mod.rs:375`. Reviewer: codex-only (HIGH trust; gemini reported `clean` — Gemini missed this).

- [x] `[TPR-04-R3-002-codex][high]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:§04.2 caller-site-updates` — The Round 2 "filter-out failed lambdas from `prepared_lambdas`" branch is NOT sound. Reading `nounwind/prepare.rs:186-208` + `define_phase.rs:142-164`: parent `prepare_arc_function` collects `prepared_lambdas: Vec<PreparedLambda>`, then calls `remap_partial_apply_names(&mut arc_func, &lambda_renames)` to rewrite name references, then calls `self.process_arc_function(name, &mut arc_func)` to process the parent. Dropping a failed lambda from `prepared_lambdas` leaves the parent `arc_func` with surviving `PartialApply` ops referencing the original (now-missing) lambda name — `remap_partial_apply_names` only rewrites callees that DID get renamed, not callees that vanished. Parent emission would later fail (bad LLVM IR or runtime error).
  Disposition: fixed in this round. §04.2 caller-site-updates subsection rewritten to remove the filter-out option entirely. Cascading signature change extended TWO levels: `prepare_lambda → prepare_arc_function → prepare_all_cached`/`prepare_mono_cached` (all return `Result<…, VerifyError>`). The record_codegen_error()` already absorbed by `declare_and_process_lambda`'s `Err` arm propagates up the chain so the PARENT function is also skipped. Analogous treatment specified for `compile_lambda_arc` → `emit_arc_function` on the immediate-emit path. Reviewer: codex-only (HIGH trust; real architectural soundness concern, not cosmetic).

- [x] `[TPR-04-R3-003-codex][low]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:242,294` — Two §04.1 doc-comment references still used `blocks[*].params[*].ty` tuple-incompatible syntax that Round 1's TPR-04-R1-001 fix missed. Line 242 (Rust doc comment enumerating type-bearing positions) and line 294 (inline comment in validator body) both carried the stale syntax.
  Disposition: fixed in this round. Both updated to tuple `.1` syntax: line 242 reads `func.blocks[*].params[*].1 — CFG-block parameter types (tuple .1 = Idx; ArcBlock.params is Vec<(ArcVarId, Idx)>)` and line 294 reads `blocks[*].params[*].1 (CFG-block parameters; tuple .1 = Idx)`. `grep -nE '\.params\[[0-9*]+\]\.(ty|var)' §04` now returns only the `ArcParam` entry-param references (which ARE valid — `ArcParam` is a struct with `.var` and `.ty` fields per `ori_arc/src/ir/mod.rs:241`), not tuple-incompatible block-param references.

### Round-3 exit state (iteration_counter=4, max_rounds=6)

- `iteration_counter` after Round 3 fix-and-commit: `4`. `max_rounds`: `6` (extended from 3 by user run-more choice). Next `while` check: `4 < 6 == TRUE` → loop continues. No cap exit this round.
- `meta_only_streak`: `0` (Round 3 produced 3 actionable findings — substantive, not meta).
- `ever_verified_findings` across Rounds 0–3: 14 (R0: 3, R1: 5, R2: 3, R3: 3). `prior_verified_fixed`: 14 (all fixed inline). `remaining`: `[]`.
- Round 4 dispatches next to verify Round 3's own fixes are themselves internally consistent.

---

### Round 4 findings (2026-04-20, after Round 3 fix-and-commit 635b6fc6; HEAD 635b6fc6)

Round 4 dispatched both reviewers against HEAD `635b6fc6`. Codex surfaced one new HIGH-severity finding — a parallel architectural concern to the Round 0 lambda hook fix applied to the parent seam. Gemini returned three `informational` confirmation entries (verifying Round 3's fixes; no actionable rule_violated, no recommended_fix) — classified as meta/not-actionable per /tpr-review §6.

- [x] `[TPR-04-R4-001-codex][high]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:§04.2 Hook 1 (process_arc_function) at line 459` — The parent-function seam `process_arc_function` relies on the SAME implicit "record_codegen_error suppresses downstream emission" invariant that Round 0's TPR-04-R0-003 identified as banned for the lambda hook — but the fix was ONLY applied to the LAMBDA seam, not the PARENT seam. `record_codegen_error()` at `compiler/ori_llvm/src/codegen/ir_builder/mod.rs:269` only increments a counter; neither `emit_arc_function` at `define_phase.rs:164-188` nor `prepare_arc_function` at `nounwind/prepare.rs:208-222` checks that counter before continuing to emission. A PC-2 violation in `process_arc_function`'s input would record the error but the caller STILL calls `ArcIrEmitter::emit_function` on the (untouched by run_arc_pipeline but otherwise contract-violating) IR.
  Disposition: fixed in this round. §04.2 Hook 1 code block rewritten to return `Result<(), VerifyError>` instead of `()`. On Err, returns `Err(VerifyError::UnresolvedTypeVar(err))` after calling `record_codegen_error()`. A new subsection "Hook 1 caller-site updates — mandatory co-change (TPR-04-R4-001)" specifies that `emit_arc_function` + `prepare_arc_function` must match on the Result and early-return — mirroring the Hook 2 cascade pattern. A new success_criteria bullet "Explicit parent no-emit contract" captures the Result signature + caller-match requirement. The Hook 1 + Hook 2 cascades now share the same explicit pattern — both seams converge on Result-based no-emit. Reviewer: codex-only (HIGH trust; gemini reported `status: findings` but all entries were informational confirmations — Gemini missed the architectural concern that Codex caught). Verified against `ir_builder/mod.rs:268-299` (counter-only semantics) + `define_phase.rs:164-188` (unconditional emit after process_arc_function).

### Round-4 exit state (iteration_counter=5, max_rounds=6)

- `iteration_counter` after Round 4 fix-and-commit: `5`. `max_rounds`: `6`. Next `while` check: `5 < 6 == TRUE` → loop continues. No cap exit this round.
- `meta_only_streak`: `0` (Round 4 produced 1 actionable substantive finding; Gemini's 3 informational entries do not reset or increment the streak — they're not meta per §6 (not wording/phrasing/cosmetic/duplicate) and not actionable (no recommended_fix). They're verification confirmations).
- `ever_verified_findings` across Rounds 0–4: 15 (R0: 3, R1: 5, R2: 3, R3: 3, R4: 1). `prior_verified_fixed`: 15. `remaining`: `[]`.
- Round 5 dispatches next to verify Round 4's parent-seam Result cascade is architecturally sound.

---

### Round 5 findings (2026-04-20, after Round 4 fix-and-commit 5f1beb20; HEAD 5f1beb20)

Round 5 dispatched both reviewers against HEAD `5f1beb20`. Codex surfaced 2 findings (high + medium); Gemini surfaced 1 high finding. Codex F1 + Gemini's single finding agree on the same gap — incomplete outer-caller cascade spec for `emit_arc_function`. Codex F2 adds a distinct concern about debug-scope cleanup on Err early-return.

- [x] `[TPR-04-R5-001-codex+gemini][high]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:§04.2 Hook 1 caller-site-updates` — The Round 4 Hook 1 caller-site-updates subsection used vague "or similar so its callers further up the stack skip as well" wording. Per actual codebase at HEAD `5f1beb20`, `emit_arc_function` has three concrete call sites: `define_function_body_arc_with_subst` (define_phase.rs:106) + two `compile_tests` branches (impls.rs:88, :151). Different callers need DIFFERENT propagation patterns: `define_function_body_arc_with_subst` must propagate via `?` (single-function semantic); `compile_tests` branches can use `continue` (loop-over-tests semantic — doesn't require caller signature change). The plan conflated these and did not name the three sites.
  Disposition: fixed in this round. Hook 1 caller-site-updates subsection rewritten with a concrete 5-level caller-chain table enumerating every site (levels 0–4: `process_arc_function` → `emit_arc_function` + `compile_lambda_arc` → `define_function_body_arc_with_subst` + `compile_tests` branches → `prepare_arc_function` → JIT/AOT batch entries). Each row states the current signature and the required change (`-> Result<…>` via `?`, OR `continue`, OR signature-preserving absorption). A new explanatory subsection "continue-on-Err pattern" documents the per-caller decision rule: loop semantic → `continue`, single-function semantic → propagate via `?`. Agreement: codex F1 + gemini R5-001.

- [x] `[TPR-04-R5-002-codex][medium]` `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md:§04.2 Hook 1 emit_arc_function early-return` — The Round 4 spec required `emit_arc_function` to early-return on `Err`, but `define_function_body_arc_with_subst` enters a debug scope at `define_phase.rs:80` (`self.enter_debug_scope(func_id)`) BEFORE calling `emit_arc_function`, and the normal-tail `self.exit_debug_scope()` sits at `define_phase.rs:220`. An early `return Err(…)` from `emit_arc_function` before reaching line 220 would skip the debug-scope exit, leaking the scope for every PC-2-violating function.
  Disposition: fixed in this round. Level-1a row in the Hook 1 caller-chain table extended with an explicit debug-scope cleanup requirement: "On `Err`, MUST call `self.exit_debug_scope()` before returning … Use a scope-guard helper OR an explicit `match … { Err(e) => { self.exit_debug_scope(); return Err(e); } }`". The plan now makes the debug-scope cleanup a first-class part of the Err contract, not an afterthought.

### Round-5 exit state (iter_cap_reached again)

- `iteration_counter` after Round 5 fix-and-commit: `6`. `max_rounds`: `6`. Next `while` check: `6 < 6 == FALSE` → loop exits at `iter_cap_reached` (second cap hit).
- `meta_only_streak`: `0` (Round 5 produced 2 actionable substantive findings — the cascade-spec gap and the debug-scope leak are both architectural concerns, neither meta).
- `ever_verified_findings` across Rounds 0–5: 17 (R0: 3, R1: 5, R2: 3, R3: 3, R4: 1, R5: 2). `prior_verified_fixed`: 17. `remaining`: `[]`.
- De-facto convergence at the second cap boundary. Each round since R1 has caught real follow-on errors from the previous round's fixes; the pattern continues to produce signal.

### Final accept decision (2026-04-20)

- User chose `accept-with-findings` at the second `iter_cap_reached` prompt (Round 5 exit).
- `exit_reason`: `user_accepted_at_iter_cap_reached`.
- Frontmatter updated: `reviewed: true`, `third_party_review.status: findings`, `third_party_review.updated: 2026-04-20`, `third_party_review.notes` records the 6-round trace + core-design validation. The `review_pipeline:` block is removed entirely per `/review-plan SKILL.md §Step 1d` ("Step 7+8 on clean exit removes the marker entirely").
- Total review cost: 6 TPR rounds (3 dispatched under original `max_rounds=3`; 3 under extended `max_rounds=6`). 17 findings fixed inline across 6 fix-and-commit cycles. Core §04 design validated; §04 ready for implementation. Cumulative plan diff vs pre-review state: roughly +900 / -200 lines of spec prose (R0–R5 combined).

---

## 04.N — Completion Checklist

- [ ] `ori_arc::ir::validate` module exists with `assert_no_unresolved_type_vars` and `UnresolvedTypeVar`
- [ ] `ori_arc::verify::VerifyError::UnresolvedTypeVar(_)` variant exists
- [ ] `ori_arc` re-exports both symbols from `lib.rs`
- [ ] `process_arc_function` calls the validator BEFORE `run_arc_pipeline`, with empty `exempt_var_ids`
- [ ] `declare_and_process_lambda` calls the validator BEFORE its own `run_arc_pipeline`, with empty `exempt_var_ids`
- [ ] (Optional, per §04.3 caveat) JIT pre-mono loop calls the validator for diagnostic localization
- [ ] (Optional, per §04.3 caveat) AOT pre-mono loop calls the validator for diagnostic localization
- [ ] Unit tests in `validate/tests.rs` cover the 12-cell matrix above (9 var_types cells + cells 10/11/12 for params / return / block-params axes) + lambda-capture behavioral test + `test_primary_seam_empty_exempt_set_invariant_pin` (semantic pin for §04.2 Design Decision 2)
- [ ] Integration test confirms `process_arc_function` records codegen error and returns early on violation
- [ ] All diagnostic sites use `tracing::error!` + structured `VerifyError` — NO `debug_assert!` fail-open
- [ ] `ORI_VERIFY_ARC=1` layering documented: assertion is ALWAYS-ON; `verify_arc` flag gates ADDITIONAL verification (fn_val.verify, oracle) per `codegen-rules.md §VR-1`
- [ ] `CLAUDE.md §Stabilization Discipline` — semantic pin test exists (Matrix cell #5 — first-violator-deterministic — is the pin; reverting the validator breaks it)
- [ ] `codegen-rules.md §VR-1` parity — the assertion layering integrates with existing `verify_arc` plumbing (NOT parallel to it)
- [ ] `impl-hygiene.md §Side Logic` — SSOT for the check is `ori_arc::ir::validate`; all call sites query it (NO scattered tag-dispatch outside the validator)
- [ ] Dependency on §03 verified green: `timeout 150 ./test-all.sh` after §03 merges, before §04
- [ ] Dependency on §08 verified green: same — §08 must resolve BUG-04-042 before §04's assertion can fire on legitimate programs
- [ ] `/tpr-review` on §§04.1–04.2 passed (04.TPR-A)
- [ ] `/tpr-review` final pass on full §04 diff passed
- [ ] `/impl-hygiene-review` passed
- [ ] `timeout 150 ./test-all.sh` green (debug and release)
- [ ] Plan annotations stripped from production code per close-out sweep
- [ ] Section status updated to `complete`
