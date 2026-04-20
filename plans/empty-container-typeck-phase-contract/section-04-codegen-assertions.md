---
section: "04"
title: "Codegen Defense-in-Depth Assertions"
status: in-progress

reviewed: false
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
  updated: 2026-04-20
  notes: "user_accepted_at_iter_cap_reached after 6 rounds (R0–R5); 17 verified findings all fixed inline across commits 7df958c3 → 3acde80f → 93b17075 → 635b6fc6 → 5f1beb20 → a9745c51; zero outstanding at accept time. max_rounds was extended once from 3→6 via run-more after Round 2; second cap fired after Round 5. Core design validated: §04.1 validator (walks var_types + params + return_type + blocks[*].params), §04.2 Hook 1 + Hook 2 explicit Result-based no-emit cascades (process_arc_function + declare_and_process_lambda; counter-based suppression via record_codegen_error banned per impl-hygiene.md §Invariant Explicitness), §04.3 diagnostic sites with per-function exempt sets via build_exempt_var_ids, §04.4 12-cell test matrix. Ready for implementation; drift against HEAD-at-implementation-time to be caught during coding."
sections:
  # Split into 04.1 (module), 04.2 (primary seam — the load-bearing site), 04.3 (pre-mono diagnostics localization), 04.4 (tests), 04.R (TPR), 04.N (checklist).
  # Prior version had 25 checkbox items > 20 (audit SIZE_VIOLATION). Restructuring collapses the per-site subsections into one primary-seam section + one secondary-sites section.
  - id: "04.1"
    title: "New `ori_arc::ir::validate` module with typed error shape and exemption set"
    status: not-started
  - id: "04.2"
    title: "PRIMARY seam: `process_arc_function` + `declare_and_process_lambda` hooks (load-bearing)"
    status: not-started
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

### Files to Create / Edit (files do NOT exist yet — `plan-audit.py` DEAD_PATH is expected pre-implementation)

| File | Action | Approx. LOC |
|------|--------|-------------|
| `compiler/ori_arc/src/ir/validate.rs` | CREATE | ~80 |
| `compiler/ori_arc/src/ir/validate/tests.rs` | CREATE | ~200 (12-cell matrix + lambda-capture + integration) |
| `compiler/ori_arc/src/ir/mod.rs` | EDIT — add `pub mod validate;` | +1 line |
| `compiler/ori_arc/src/lib.rs` | EDIT — add `pub use ir::validate::{assert_no_unresolved_type_vars, UnresolvedTypeVar};` | +1 line |
| `compiler/ori_arc/src/verify/mod.rs` | EDIT — add `UnresolvedTypeVar(UnresolvedTypeVar)` variant to `VerifyError` enum | +3 lines |
| `compiler/ori_types/src/check/validators/mod.rs` | EDIT — change `pub(crate) fn build_exempt_var_ids(pool: &Pool, scheme_var_ids: &[u32]) -> FxHashSet<u32>` (line 161) to `pub fn ...` so the §04.3 pre-mono sites can call it from outside `ori_types::check` | +0 net lines (visibility keyword change) |
| `compiler/ori_types/src/lib.rs` | EDIT — add `pub use check::validators::build_exempt_var_ids;` re-export so §04.3 callers write `ori_types::build_exempt_var_ids(pool, &sig.scheme_var_ids)` rather than the full path | +1 line |

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
///   For monomorphized functions this set is EMPTY. For non-monomorphized
///   function bodies (e.g., pre-mono JIT path) the caller populates it from
///   the owning `FunctionSig.scheme_var_ids`.
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
| 2b | `compile_tests` branches (impls.rs:88, impls.rs:151) | — | On `Err`, use `continue` to skip to the next test iteration WITHOUT altering `compile_tests`'s return signature. The per-test failure is already recorded via `record_codegen_error()` and the suite continues. This mirrors Gemini R5-001's recommendation: not every outer caller must change signature — some outer-loop callers can absorb `Err` via `continue` or `let _ =` when their loop semantic is "keep going past individual failures". |
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

- [ ] **Post-substitution firing verification (both paths)**: after §04.2's `assert_no_unresolved_type_vars` hooks are inserted at `define_phase.rs:315` (`process_arc_function`) and `:375` (`declare_and_process_lambda`) AND §08.3's remap-aware re-intern is live in the merged pool, verify the assertion fires POST-substitution for (a) the intra-module lambda_mono path via `resolve_all_lambda_bound_vars` at `define_phase.rs:134` + `nounwind/prepare.rs:173`, and (b) the cross-module re-intern path via `pool/re_intern/`. Record the confirmation in §04.R close-out and backlink §08.6.R as "§04 seam order verified correct under §08.1.R corrected diagnosis; no change required." Seam-line-number shifts are acceptable as long as the POST-substitution invariant holds.
- [ ] **Two-case assertion strictness holds under §08.3 remap**: validate that `assert_no_unresolved_type_vars`'s caller-parameterized two-case design (per `§04.2` `exempt_var_ids` contract, lines 239-244) remains sound after §08.3's remap-aware re-intern lands. Case (a) — monomorphized functions, `exempt_var_ids` EMPTY, strict `typeck.md §PC-2` + `canon.md §4.2` rule: §08.3 + `resolve_all_lambda_bound_vars` must leave zero `Tag::Var` at the seam; a surviving `Tag::Var` is a §08.3 completeness bug. Case (b) — non-monomorphized generic function bodies (pre-mono JIT path), `exempt_var_ids` populated from `FunctionSig.scheme_var_ids`: `Tag::Var(Generalized)` / `Rigid` vars legitimately survive per `types.md §SC-1` divergence; the exemption mirrors `ori_types::check::validators::build_exempt_var_ids`. §08.3's matrix cells e1–e5 must make case (a) sound WITHOUT regressing case (b). Record the verification in §04.R as "two-case seam strictness holds under §08.3 remap; §08.3's cell coverage adequate."

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

### Site A: JIT pre-mono loop at `evaluator/compile.rs` (~line 230)

Insert between the `mono_functions` extension (line 236) and the `run_interprocedural_analyses`
call (line 238). For each `(arc_fn, lambdas)` in `arc_cache`, invoke the assertion. Because
`arc_cache` at this point contains BOTH monomorphized instances AND generic source bodies
(per the caller's pre-lowered layout verified at `compiler/ori_llvm/src/evaluator/compile.rs`),
the exempt set MUST be populated per-function from `FunctionSig.scheme_var_ids`; using an
empty set would spuriously fire on legitimate `Tag::RigidVar`s (and, pre-§08.3b, on
`Tag::Var(Generalized)`) that survive in generic bodies. Identified by TPR-04-R1-F2
(critical) via the §04.3 empty-set/generic-body contradiction with §04.1's doc comment.

```rust
// PC-2 contract check — early diagnostic localization. Non-load-bearing
// (the process_arc_function seam is the correctness gate); this site exists
// only to attribute the diagnostic to the pre-mono input.
//
// `build_exempt_var_ids` is the producer-side helper at
// `compiler/ori_types/src/check/validators/mod.rs`; it returns the set of
// rigid + generalized scheme var_ids for a given FunctionSig. For
// monomorphized functions the helper returns an empty set. For generic
// source bodies it returns the scheme_var_ids set. This mirrors §04.1's
// doc-comment contract.
// `function_sigs: &[FunctionSig]` is a slice (verified at
// `compiler/ori_llvm/src/evaluator/compile.rs:69`), NOT a `Name`-keyed
// map. Build a name→&sig lookup once at loop entry so the inner
// exempt-set construction is O(1) per function; the slice itself is
// typically dozens of entries, so the one-time HashMap build is cheap
// and keeps the per-function scheme_var_ids query correct.
let sig_by_name: rustc_hash::FxHashMap<ori_ir::Name, &ori_types::FunctionSig> =
    function_sigs.iter().map(|s| (s.name, s)).collect();

for (fn_name, (arc_fn, lambdas)) in arc_cache.iter() {
    // Look up the generic scheme for this function; imported mono
    // instances may not be in `function_sigs`, in which case the helper
    // default (empty set) is correct — they are fully substituted.
    let sig = sig_by_name.get(fn_name).copied();
    let exempt: rustc_hash::FxHashSet<u32> = sig
        .map(|s| ori_types::build_exempt_var_ids(self.pool, &s.scheme_var_ids))
        .unwrap_or_default();
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
        // Lambdas inherit their parent function's scheme; reuse the same
        // `exempt` set rather than re-querying function_sigs.
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

Analogous insertion after each `arc_cache.insert(arc_fn.name, (arc_fn, lambdas))` (both the
pre-mono loop at lines ~95-105 and the mono loop at lines ~119-129). Uses the bare `pool`
parameter (no `self`). Same diagnostic-only pattern as Site A — including the same
per-function exempt-set requirement: the pre-mono loop operates on generic source bodies
whose `FunctionSig.scheme_var_ids` must exempt rigid/generalized vars from spurious
firing, while the mono loop operates on monomorphized instances where the exempt set is
empty. Both loops use `ori_types::build_exempt_var_ids(pool, &sig.scheme_var_ids)` with the
corresponding `FunctionSig` rather than the empty default. The mono loop is the only one
where the exempt set is empty by design (mono functions have empty `scheme_var_ids`);
the pre-mono loop passes the generic source body's populated `scheme_var_ids`.

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
- [ ] Unit tests in `validate/tests.rs` cover the 12-cell matrix above (9 var_types cells + cells 10/11/12 for params / return / block-params axes) + lambda-capture behavioral test
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
