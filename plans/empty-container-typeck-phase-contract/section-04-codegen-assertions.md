---
section: "04"
title: "Codegen Defense-in-Depth Assertions"
status: not-started
reviewed: false
goal: "Insert an `assert_no_unresolved_type_vars` call at the single upstream codegen seam (`ori_llvm::function_compiler::process_arc_function` + the lambda counterpart in `declare_and_process_lambda`) so that any `Tag::Var` surviving the typeck → ARC → codegen boundary is caught immediately with a typed error, a clear diagnostic, and integration with the existing `ORI_VERIFY_ARC` plumbing — NOT a collection of 4 fragile consumer-site hooks that bypass the seam. A small number of secondary pre-seam hooks (at the 4 monomorphization entry points) remain ONLY to localize the diagnostic to the pre-realization IR; the load-bearing gate is the seam hook."
success_criteria:
  # Module / API
  - "New module `compiler/ori_arc/src/ir/validate.rs` (files to be CREATED by this section) exists and exports `pub fn assert_no_unresolved_type_vars(pool: &Pool, func: &ArcFunction, interner: &StringInterner, exempt_var_ids: &FxHashSet<u32>) -> Result<(), UnresolvedTypeVar>` — verifiable post-creation via `grep -rn 'pub fn assert_no_unresolved_type_vars' compiler/ori_arc/src/ir/validate.rs` returning exactly one hit. The `exempt_var_ids` parameter mirrors the producer-side `build_exempt_var_ids` pattern (`compiler/ori_types/src/check/validators/mod.rs:161`) so generic-function bodies with `VarState::Generalized` / `VarState::Rigid` vars do NOT fire spuriously. The `UnresolvedTypeVar` error type is a typed struct `{ function: Name, var_id: ArcVarId, idx: Idx, tag: Tag }` — NOT `Result<(), String>` (gemini + codex consensus: typed enum integrates with existing `VerifyError` plumbing)."
  - "`compiler/ori_arc/src/ir/mod.rs` (to be edited by this section) declares `pub mod validate;` — verifiable post-edit via `grep -n 'pub mod validate' compiler/ori_arc/src/ir/mod.rs` returning one hit. `ori_arc/src/lib.rs` re-exports `pub use ir::validate::{assert_no_unresolved_type_vars, UnresolvedTypeVar};`."
  # Primary seam (SINGLE upstream choke point)
  - "Primary site (PRIMARY, load-bearing): `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:315` — the `process_arc_function` helper — invokes `assert_no_unresolved_type_vars` for `arc_func` BEFORE `ori_arc::run_arc_pipeline(...)` is called (line ~331). The call is gated by `self.verify_arc` for AIMS-style opt-in, AND produces a typed `VerifyError::UnresolvedTypeVar` at `self.builder.record_codegen_error()` in BOTH debug and release builds — there is no `debug_assert!` fail-open path (gemini + codex consensus: release-strip of `debug_assert!` violates CLAUDE.md §The One Rule). Verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` returning at least one hit inside `process_arc_function`."
  - "Primary site (PRIMARY, lambdas): `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:375` — the `declare_and_process_lambda` helper — invokes `assert_no_unresolved_type_vars` for `lambda` BEFORE `run_arc_pipeline(...)` at line ~443. Lambdas are compiled as separate `ArcFunction`s and do NOT flow through `process_arc_function`; they have their own `run_arc_pipeline` call which must be guarded. Verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` returning at least TWO hits total (`process_arc_function` and `declare_and_process_lambda`)."
  # Secondary pre-seam sites (localize diagnostic — NOT load-bearing by themselves)
  - "Secondary site A (pre-mono entry, JIT): `compiler/ori_llvm/src/evaluator/compile.rs` around line ~230 (the point at which `mono_functions` are already in `arc_cache` and are about to be handed to `prepare_mono_cached`) — invokes `assert_no_unresolved_type_vars` for each `(arc_fn, lambdas)` triple in `arc_cache`. This site exists to surface the violation with the ORIGINAL pre-realization IR (AIMS mutates `arc_func` in place during `run_arc_pipeline`; the primary-seam assertion would see the post-lower IR, not the pre-mono IR). Without this secondary site the primary seam still catches the violation, but the diagnostic is attributed to a post-lowering state rather than the mono input — worse UX, same correctness gate. Verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/ori_llvm/src/evaluator/compile.rs` returning at least one hit."
  - "Secondary site B (pre-mono entry, AOT): `compiler/oric/src/commands/codegen_pipeline.rs` around line ~112 — analogous to the JIT site, invokes `assert_no_unresolved_type_vars` on each `arc_cache.insert(arc_fn.name, (arc_fn, lambdas))` output (both the pre-mono loop at lines ~95-105 and the mono loop at lines ~119-129). Verifiable via `grep -n 'assert_no_unresolved_type_vars' compiler/oric/src/commands/codegen_pipeline.rs` returning at least TWO hits."
  # Error integration
  - "Error shape: `UnresolvedTypeVar` is a typed struct constructed in `validate.rs` and propagated via the existing `ori_arc::verify::VerifyError` enum as a new variant `UnresolvedTypeVar(UnresolvedTypeVar)`. The primary seam treats this exactly like other `VerifyError` variants — `verify_errors` collection, `builder.record_codegen_error()`, skip subsequent emission for this function. NO parallel error path, NO `Result<(), String>`, NO `tracing::error!` standalone. Verifiable via `grep -n 'UnresolvedTypeVar' compiler/ori_arc/src/verify/` returning hits in the `VerifyError` enum definition AND in `error/` variant construction sites."
  - "`ORI_VERIFY_ARC=1` integration: the primary-seam assertion is gated by `self.verify_arc` (the existing flag that gates the full `fn_val.verify(true)` + AIMS oracle cross-check per `codegen-rules.md §VR-1`). When `ORI_VERIFY_ARC=1` is NOT set, the assertion still runs — it is cheaper than LLVM IR verification and is mandatory for the phase-contract enforcement per `impl-hygiene.md §Cross-Phase Invariant Contracts`. The FLAG gates ADDITIONAL verification (oracle cross-check, Alive2 validation); the assertion is ALWAYS-ON in both debug and release — gemini + codex consensus. Verifiable: a unit test disables the validator (via injected `skip_validate` test-only hook on `process_arc_function`) and confirms the downstream LLVM IR verifier still fails cleanly on `Tag::Var` inputs — establishing defense-in-depth layering rather than gating."
  # Testing + non-regression
  - "Unit tests: `compiler/ori_arc/src/ir/validate/tests.rs` (to be CREATED) covers: (a) empty `var_types` → `Ok`; (b) all resolved → `Ok`; (c) first var is `Tag::Var` → `Err(UnresolvedTypeVar { var_id: 0, .. })`; (d) second var is `Tag::Var` → `Err` naming ArcVarId(1); (e) all vars `Tag::Var` → `Err` naming the first violator; (f) `Tag::Var` with `var_id` in `exempt_var_ids` → `Ok` (Generalized/Rigid exemption); (g) `Tag::BoundVar` → `Ok` (per `types.md §TK-9` — bound vars under a scheme are NOT PC-2 violations); (h) `Tag::Projection` → `Err` (PC-2 also forbids unresolved projections); (i) lambda capture environments (closure-captured var types enumerated in `ArcFunction.var_types`) covered by a separate test that builds a lambda with a `Tag::Var` in the capture env and confirms the validator flags it — closing the Blind Spot #5 (§04 blind-spots.json) about captured-closure types."
  - "`timeout 150 ./test-all.sh` is green after landing (debug AND release). Dependency-gated: Sections 03 and 08 must be complete first — §03 ensures legitimate programs do not carry surviving `Tag::Var`s, §08 resolves BUG-04-042 BoundVar bleed which WOULD trip this assertion on valid generic code today. See `depends_on` below."
inspired_by:
  - "Rust `rustc_middle::mir::visit::TyContext` — every MIR visitor receives the type context and `debug_assert!`s that types are fully resolved at traversal boundaries; the pattern here mirrors that per-function pre-emission gate but at a SINGLE upstream choke point rather than scattered across visitors."
  - "Swift `SILVerifier` — the Swift compiler runs a multi-checkpoint IR verifier with ownership + type checks before and after SIL optimization passes; Section 04 is a single-checkpoint analog for the ARC IR → LLVM IR handoff, integrated with the existing `ORI_VERIFY_ARC` verifier stack (NOT parallel to it)."
  - "Koka `Core.Check` — Koka's backend verifies that no `TVar` escapes into the final core IR before monomorphisation; the `assert_no_unresolved_type_vars` helper is a direct structural equivalent at the `process_arc_function` seam."
  - "Lean 4 `Compiler/IR/RC.lean` — Lean places its structural RC/IR checks at a SINGLE pipeline stage rather than at per-consumer emission sites, matching the single-seam decision here."
depends_on: ["03", "08"]
third_party_review:
  status: none
  updated: null
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
    title: "Unit tests: 9-cell matrix (resolved, unbound, exempt, BoundVar, Projection, lambda-capture, first-violator-deterministic)"
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
| `compiler/ori_arc/src/ir/validate/tests.rs` | CREATE | ~150 |
| `compiler/ori_arc/src/ir/mod.rs` | EDIT — add `pub mod validate;` | +1 line |
| `compiler/ori_arc/src/lib.rs` | EDIT — add `pub use ir::validate::{assert_no_unresolved_type_vars, UnresolvedTypeVar};` | +1 line |
| `compiler/ori_arc/src/verify/mod.rs` | EDIT — add `UnresolvedTypeVar(UnresolvedTypeVar)` variant to `VerifyError` enum | +3 lines |

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

/// Check that every variable in `func.var_types` resolves to a concrete type
/// (no `Tag::Var` outside the `exempt_var_ids` set).
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
    // Iterate in ArcVarId order (raw_idx ascending) — deterministic per
    // impl-hygiene.md §SSOT. Return the FIRST violator; callers log all.
    for (raw_idx, &ty) in func.var_types.iter().enumerate() {
        // Gate order mirrors producer-side validator
        // (ori_types/check/validators/mod.rs): resolve_fully → tag check →
        // exemption set. `resolve_fully` is the key step — `Tag::Var` in
        // `var_types` may be a Link to a concrete type that fully resolves.
        let resolved = pool.resolve_fully(ty);
        let tag = pool.tag(resolved);
        if matches!(tag, Tag::Var) {
            let var_id = pool.data(resolved); // Tag::Var: data IS the var_id
            if exempt_var_ids.contains(&var_id) {
                continue;
            }
            return Err(UnresolvedTypeVar {
                function: func.name,
                var_id: ArcVarId::new(raw_idx as u32),
                idx: resolved,
                tag,
            });
        }
        // Also catch Projection (PC-2 clause 3) — unresolved associated types
        // are a PC-2 violation even though they are not Tag::Var.
        if matches!(tag, Tag::Projection) {
            return Err(UnresolvedTypeVar {
                function: func.name,
                var_id: ArcVarId::new(raw_idx as u32),
                idx: resolved,
                tag,
            });
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

## 04.2 — PRIMARY Seam: `process_arc_function` + `declare_and_process_lambda` Hooks

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
pub(super) fn process_arc_function(&mut self, name: Name, arc_func: &mut ori_arc::ArcFunction) {
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
        // Return early — do not invoke run_arc_pipeline on contract-violating
        // IR. The error is already recorded; downstream emission for this
        // function will be skipped by the existing record_codegen_error flow.
        return;
    }

    // ... existing body: AIMS param ownership, run_arc_pipeline, etc. ...
}
```

The early `return` is load-bearing: continuing into `run_arc_pipeline` on a contract-
violating input risks panics inside AIMS analysis (it assumes resolved types). Skipping
emission is the correct failure mode — the user sees a single clear PC-2 diagnostic rather
than a cryptic LLVM verification failure or AIMS panic.

### Hook 2: `declare_and_process_lambda` (line ~375)

Insert the assertion at the TOP of `declare_and_process_lambda` — analogous placement to
Hook 1, before `run_arc_pipeline` at line ~443. Lambdas do NOT route through
`process_arc_function`; they are a distinct seam.

```rust
pub(super) fn declare_and_process_lambda(
    &mut self,
    lambda: &mut ori_arc::ArcFunction,
) -> (Name, FunctionId, FunctionAbi) {
    // PC-2 contract check for lambdas — same pattern as process_arc_function.
    // Lambdas have their own run_arc_pipeline call (line ~443) and do NOT
    // route through process_arc_function.
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
        // Fall through with a placeholder return — existing code already
        // handles post-record_codegen_error unwinding. Compute_arc_function_abi
        // is infallible on pre-pipeline state and returns a valid abi that is
        // never emitted (record_codegen_error suppresses emit).
    }

    // ... existing body: apply AIMS contracts, declare LLVM function, etc. ...
}
```

Note: unlike `process_arc_function`, we cannot `return` early from
`declare_and_process_lambda` because the function must produce a `(Name, FunctionId,
FunctionAbi)` triple for the caller's lambda-rename bookkeeping. The `record_codegen_error`
call suppresses downstream emission; the returned values are never consumed for LLVM
emission after error recording.

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
call (line 238). For each `(arc_fn, lambdas)` in `arc_cache`, invoke the assertion. Uses the
same empty `exempt_var_ids` (mono functions are fully substituted):

```rust
// PC-2 contract check — early diagnostic localization. Non-load-bearing
// (the process_arc_function seam is the correctness gate); this site exists
// only to attribute the diagnostic to the pre-mono input.
let exempt: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
for (arc_fn, lambdas) in arc_cache.values() {
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

Analogous insertion after each `arc_cache.insert(arc_fn.name, (arc_fn, lambdas))` (both the
pre-mono loop at lines ~95-105 and the mono loop at lines ~119-129). Uses the bare `pool`
parameter (no `self`). Same diagnostic-only pattern.

---

## 04.4 — Unit Tests for `assert_no_unresolved_type_vars`

### File to Create

`compiler/ori_arc/src/ir/validate/tests.rs` — sibling of `validate.rs`, declared as
`#[cfg(test)] mod tests;` at the bottom of `validate.rs`.

### 9-Cell Test Matrix (up from the prior 5-cell matrix per reviewer request)

The matrix dimensions: `var_types state × exempt set × expected outcome`. Each row must be
realized as a named test function with a behavioral name per `impl-hygiene.md §Test Function
Naming`:

| # | Var types state | Exempt set | Expected | Test name |
|---|-----------------|------------|----------|-----------|
| 1 | empty `var_types` | empty | `Ok(())` | `test_empty_var_types_passes` |
| 2 | all fully resolved primitives | empty | `Ok(())` | `test_all_resolved_primitives_pass` |
| 3 | first var is `Tag::Var`, var_id 0 | empty | `Err(UnresolvedTypeVar { var_id: 0, .. })` | `test_first_var_unresolved_returns_error_with_var_id_zero` |
| 4 | second var is `Tag::Var`, var_id 7; first resolved | empty | `Err(UnresolvedTypeVar { var_id: 1, .. })` | `test_second_var_unresolved_names_that_arcvarid` |
| 5 | all vars `Tag::Var`, increasing var_ids | empty | `Err(_)` with the first (lowest ArcVarId) | `test_all_vars_unresolved_returns_first_violator_deterministic` |
| 6 | `Tag::Var` with var_id 42; exempt set = `{42}` | `{42}` | `Ok(())` | `test_tag_var_with_exempt_var_id_passes` |
| 7 | `Tag::Var` with var_id 42; exempt set = `{7}` | `{7}` | `Err(_)` with var_id 42 | `test_tag_var_outside_exempt_set_fails` |
| 8 | resolved via `VarState::Link` to concrete type | empty | `Ok(())` | `test_linked_var_resolves_via_pool_resolve_fully` |
| 9 | `Tag::Projection` (unresolved associated type) | empty | `Err(_)` with `tag: Tag::Projection` | `test_unresolved_projection_returns_error` |

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

## 04.N — Completion Checklist

- [ ] `ori_arc::ir::validate` module exists with `assert_no_unresolved_type_vars` and `UnresolvedTypeVar`
- [ ] `ori_arc::verify::VerifyError::UnresolvedTypeVar(_)` variant exists
- [ ] `ori_arc` re-exports both symbols from `lib.rs`
- [ ] `process_arc_function` calls the validator BEFORE `run_arc_pipeline`, with empty `exempt_var_ids`
- [ ] `declare_and_process_lambda` calls the validator BEFORE its own `run_arc_pipeline`, with empty `exempt_var_ids`
- [ ] (Optional, per §04.3 caveat) JIT pre-mono loop calls the validator for diagnostic localization
- [ ] (Optional, per §04.3 caveat) AOT pre-mono loop calls the validator for diagnostic localization
- [ ] Unit tests in `validate/tests.rs` cover the 9-cell matrix above + lambda-capture behavioral test
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
