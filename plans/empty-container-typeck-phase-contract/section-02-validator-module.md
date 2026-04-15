---
section: "02"
title: "Validator Module — validate_body_types()"
status: complete
reviewed: true
goal: >
  Introduce ori_types::check::validators — a new submodule of the type-checker
  crate — containing validate_body_types(), the producer-side enforcement of
  typeck.md §PC-2 ("no Tag::Var in any type-bearing IR position"). The
  validator walks body expr_types AND the body's FunctionSig (param_types +
  return_type) after InferEngine body-checking completes, identifies any
  surviving unbound Tag::Var per expression/signature, and emits E2005
  (AmbiguousType) once per ExprIndex. This section also fixes a latent pool
  bug (Tag::Scheme flags not propagating child flags, types.md §TF-3) that
  would otherwise defeat the HAS_VAR fast-path gate, and reuses the existing
  pool tag-child walker at `compiler/ori_types/src/pool/descriptor.rs:303`
  rather than cloning a parallel recursion skeleton (impl-hygiene.md
  §Algorithmic DRY). Section 03 threads the call into the bodies pass;
  Sections 01 and 02 remain independent.
success_criteria:
  - "Pool::compute_flags arm for Tag::Scheme ORs PROPAGATE_MASK bits from the scheme body into the parent flags (types.md §TF-3)."
  - "Regression test in compiler/ori_types/src/pool/tests.rs: `Tag::Scheme` wrapping a body containing an unbound `Tag::Var` has `HAS_VAR` set on the scheme Idx."
  - "compiler/ori_types/src/check/validators/mod.rs exists and declares validate_body_types() with the signature in §02.1."
  - "validate_body_types() accepts body expr_types AND the body's FunctionSig (param_types slice + return_type) — signature-validation path per CHK:CK-4 hand-off shape."
  - "lib.rs keeps `mod check;` private; a single narrow re-export `pub use check::validators::validate_body_types;` is added (no `pub mod check`)."
  - "check/mod.rs declares `pub(crate) mod validators;`."
  - "TF-5 fast-path: validator calls pool.resolve_fully(idx) before every HAS_VAR check at every walk step (not only the top level)."
  - "HAS_ERROR cascade suppression (types.md §TK-3): skip types with HAS_ERROR at the top-level gate."
  - "Tag-dispatch child recursion explicitly handles every compound variant (Applied, Scheme, Struct, Enum, Function, Tuple, List, Set, Map, Result, Range, Iterator, DoubleEndedIterator, Channel) — NO `_ => {}` catch-all silently dropping compound tags. Note: Tag::Projection is NOT included because typeck.md §PC-2 bullet 3 guarantees no Tag::Projection survives in the typed IR by the time the validator runs (all projections are normalized). The existing Pool::visit_children helper correctly treats Projection as a leaf."
  - "Scheme-aware walk is implemented WITHOUT a bound_vars parameter: HAS_VAR alone distinguishes free from bound vars (types.md §TF-1 — Tag::BoundVar sets HAS_BOUND_VAR, not HAS_VAR; §TF-3 propagation ensures outer HAS_VAR = true iff an unbound Var is reachable)."
  - "Algorithmic DRY: the walker either calls `Pool::visit_children` (compiler/ori_types/src/pool/descriptor.rs:303) or an extracted sibling, not a new parallel tag-dispatch ladder."
  - "One E2005 per ExprIndex: multiple unbound Vars under the same ExprIndex collapse to a single diagnostic (impl-hygiene.md §Deduplication by (Code, Span) with Follow-On Suppression)."
  - "Diagnostics emitted in ascending ExprIndex order (impl-hygiene.md §Pass determinism); ties within a signature are emitted before body diagnostics."
  - "Unit tests in check/validators/tests.rs cover all twelve matrix cells (T1–T12) in §02.4."
  - "`cargo test -p ori_types` passes; `cargo clippy -p ori_types` clean; `diagnostics/repo-hygiene.sh --check` clean."
inspired_by:
  - "typeck.md §CK-1 — pass ordering (validator runs at Bodies-group exit, after engine.take_errors())"
  - "typeck.md §CK-4 — signatures hand off with fresh Tag::Var for unannotated params/returns; validator catches surviving ones"
  - "typeck.md §PC-2 — output contract (no Tag::Var in any type position)"
  - "typeck.md §DI-1 — E2005 AmbiguousType"
  - "typeck.md §ER-4 — follow-on suppression (validator skips HAS_ERROR)"
  - "typeck.md §SL-1 — Salsa query purity (validator is pure: pool + expr_types + sig → errors)"
  - "types.md §TF-1 — HAS_VAR vs HAS_BOUND_VAR flag semantics"
  - "types.md §TF-3 — PROPAGATE_MASK propagation (Tag::Scheme must propagate body's flags — 02.0 fix)"
  - "types.md §TF-5 — Fast-Path Gates (HAS_VAR gate + resolve_fully at each step)"
  - "types.md §TK-3 — Tag::Error as poison (cascade suppression)"
  - "types.md §SC-1 — Scheme layout (body reached through pool.scheme_body())"
  - "impl-hygiene.md §SSOT — validator is the single canonical home for producer-side PC-2 enforcement"
  - "impl-hygiene.md §Side Logic — PC-2 enforcement MUST NOT leak into consumers' ad-hoc checks"
  - "impl-hygiene.md §Algorithmic DRY — reuse pool/descriptor.rs:303 Pool::visit_children rather than cloning"
  - "impl-hygiene.md §Cross-Phase Invariant Contracts — Type Checker → Codegen row; producer-side validator + consumer debug_assert! pair"
  - "compiler.md §File Organization — new validators/ submodule lands beside signatures/; keeps check/mod.rs at 408 lines (under 500 limit)"
  - "tests.md §Matrix Testing Rule — twelve cells cover negative (error path) × base/applied/signature/nested + positive (clean path) × resolved/linked/boundvar/generalized + cascade + determinism + dedup (conventional: Negative = emits diagnostic, Positive = clean)"
  - "compiler/ori_types/src/check/object_safety.rs — closest existing pub(crate) validator pattern (method signature + diagnostic accumulation)"
  - "compiler/ori_types/src/infer/context.rs — take_errors() idiom used by the Bodies pass"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-15
sections:
  - id: "02.0"
    title: "Pool scheme-flag propagation fix (prerequisite for §02.2 HAS_VAR gate)"
    status: complete
  - id: "02.1"
    title: "Validator signature, public contract, and narrow re-export"
    status: complete
  - id: "02.2"
    title: "Core algorithm: tag-dispatch child recursion (reusing Pool::visit_children)"
    status: complete
  - id: "02.3"
    title: "lib.rs and check/mod.rs wiring (no pub mod check)"
    status: complete
  - id: "02.4"
    title: "Unit test matrix (twelve cells)"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "02.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 02 — Validator Module: `validate_body_types()`

## Context

Section 01 (AST-based Value Restriction) and Section 02 are independent. Both
feed into Section 03, which threads the new producer-side enforcement into
the bodies pass.

The type checker's output contract (`typeck.md §PC-2`, mirrored in
`types.md §PC-2`) requires that no `Tag::Var` survive in any type position
of the typed IR. Today this contract is documented but **not enforced at the
producer boundary** — the only enforcement that exists is downstream
(consumers `debug_assert!` on entry, per `impl-hygiene.md §Cross-Phase
Invariant Contracts`). This section introduces the missing producer-side
validator so that violations are caught inside `ori_types` with a proper
`E2005` diagnostic, before the IR is handed off to eval, ARC, canon, and
codegen.

The validator lives in a new submodule
`compiler/ori_types/src/check/validators/` modelled on the
`pub(crate)` pattern of `compiler/ori_types/src/check/object_safety.rs`
(not a `pub` pattern — `object_safety.rs` exposes `pub(crate) fn`, not
`pub fn`). Section 03 will call it from
`compiler/ori_types/src/check/bodies/mod.rs` after `engine.take_errors()`
drains the inference accumulator.

### Scope expansion (from Phase 2 /tp-help + Phase 1 plan-audit)

The draft version of this section was architecturally incomplete. Four issues
surfaced under dual-source Plan TPR (Codex + Gemini) and six more via
`scripts/plan-audit.py`. The corrections are woven into §§02.0–02.N:

1. **Signature validation gap (CONSENSUS critical).** `typeck.md §PC-2` and
   `§CK-4` make it explicit that `FunctionSig.param_types` and
   `FunctionSig.return_type` may carry fresh `Tag::Var` for unannotated
   params/returns at signatures-pass exit; those vars MUST be resolved by
   Bodies-group exit. The validator MUST walk the signature in addition to
   `expr_types`, or unannotated `@f(x) = 42` leaks an unbound `Var` directly
   to codegen. Addressed by §02.1 and §02.2.
2. **Public-API exposure scope (CONSENSUS critical).** Promoting
   `mod check` to `pub mod check` would leak the entire type-checker
   internal layout (`bodies/`, `exports/`, `imports/`, `registration/`, …)
   as stable API. `object_safety.rs` — the cited precedent — is actually
   `pub(crate) fn`, NOT `pub`. Fix: keep `mod check;` private; add a
   single narrow re-export `pub use check::validators::validate_body_types;`
   at `lib.rs`. Addressed by §02.1 and §02.3.
3. **Redundant bound-var tracking (CONSENSUS critical).** The draft tracked
   `FxHashSet<u32>` of scheme-bound var ids. Per `types.md §TF-1`,
   `Tag::BoundVar` sets `HAS_BOUND_VAR`, NOT `HAS_VAR`. So `HAS_VAR` alone
   distinguishes "free unbound var" from "scheme-bound var". The parameter
   is redundant — the walker simply recurses into the scheme body and
   silently skips any `Tag::BoundVar` it meets. Addressed by §02.2.
4. **Latent pool bug: `Tag::Scheme` flag propagation (verified against
   `compiler/ori_types/src/pool/mod.rs:652`).** `types.md §TF-3` requires
   `PROPAGATE_MASK` flags to be OR'd from every child into the parent,
   explicitly including schemes. But the current `compute_flags` arm reads:
   ```rust
   Tag::Scheme => TypeFlags::IS_SCHEME,
   ```
   No child-flag propagation. Consequence: a scheme wrapping a body with
   `HAS_VAR = true` has `HAS_VAR = false` on the outer Idx — the HAS_VAR
   fast-path gate silently skips such schemes and misses legitimate PC-2
   violations. This is IN-SCOPE for §02: the validator depends on the gate
   being sound, the fix is ~10–15 LOC + a test, and splitting it off as a
   separate bug would fragment one coherent correction. Addressed by §02.0.
5. **plan-audit DEAD_PATH findings (6, major).** Bare paths (`lib.rs`,
   `check_error/mod.rs`, `infer/mod.rs`, `pool/accessors.rs`,
   `check/validators/mod.rs`, `check/validators/tests.rs`) must be fully
   qualified or annotated as forward references (the last two are created by
   this section). Addressed inline throughout §§02.0–02.N.
6. **plan-audit COMPLETION_TPL finding (1, major).** Section 02 lacked a
   `02.N — Completion Checklist`. Added in §02.N.
7. **plan-audit BLOAT_RISK (1, minor).** `compiler/ori_types/src/check/mod.rs`
   is 408 lines (under the 500-line limit per
   `compiler.md §File Organization`). New validator logic lands in
   `compiler/ori_types/src/check/validators/mod.rs`, a sibling submodule —
   `check/mod.rs` gains exactly one line (`pub(crate) mod validators;`). No
   refactor required.

Other high-value findings woven in:

- **HAS_VAR cache staleness after unification (DRIFT).** Flags are cached at
  intern time; `VarState::Link(target)` changes after. A Var with
  `HAS_VAR = true` may actually be resolved via link. Fix: call
  `pool.resolve_fully(idx)` before every HAS_VAR check at every walk step —
  not just the top level. Addressed by §02.2.
- **Catch-all `_ => {}` leaking compound tags (LEAK).** `let x: Option<_>`
  produces `Tag::Applied` containing an unbound Var. A catch-all would
  silently pass through. Fix: explicit handling of every compound tag; no
  silent drops. Addressed by §02.2.
- **Algorithmic duplication (LEAK).**
  `compiler/ori_types/src/pool/descriptor.rs:303` — `Pool::visit_children` —
  already performs tag-dispatched child recursion over every compound tag.
  Writing a second parallel ladder would be a second SSOT for "how to reach
  every child of a type" (`impl-hygiene.md §Algorithmic DRY`). Fix: reuse
  `Pool::visit_children` in the walker. Addressed by §02.2.
- **One E2005 per ExprIndex (LEAK dedup).** Multiple unbound Vars under one
  ExprIndex (e.g. `{}` produces key-var + value-var both unbound) must
  collapse to one diagnostic at that ExprIndex
  (`impl-hygiene.md §Deduplication by (Code, Span) with Follow-On
  Suppression`). Addressed by §02.2 and §02.4 T8.

---

## 02.0 Pool Scheme-Flag Propagation Fix (Prerequisite)

**Source of truth:** `types.md §TF-3` — `PROPAGATE_MASK` flags SHALL be OR'd
from every child into the parent for every compound tag, explicitly
including schemes.

**Current state (verified at `compiler/ori_types/src/pool/mod.rs:652`):**

```rust
// Scheme
Tag::Scheme => TypeFlags::IS_SCHEME,
```

The arm returns a bare `IS_SCHEME` bit with no child-flag propagation. This
is a spec violation (`types.md §TF-3`) and a latent bug that would defeat
the `HAS_VAR` fast-path gate (`types.md §TF-5`) that §02.2 depends on.

**BLOAT_RISK acknowledgement:** `compiler/ori_types/src/pool/mod.rs` is
currently 690 lines (over the 500-line limit). `pool/tests.rs` is 1244
lines. Both are touched by §02.0. The pool module's size is a pre-existing
condition and is NOT addressed by this section -- splitting `pool/mod.rs`
is a separate refactoring concern tracked by the plan-audit BLOAT_RISK
finding. The §02.0 changes add ~10 LOC to `pool/mod.rs` and ~30 LOC to
`pool/tests.rs`, which does not materially change the bloat status.
Similarly, `pool/descriptor.rs` (501 lines) gains only a visibility change
in §02.2.

### 02.0.1 — Fix

- [x] In `compiler/ori_types/src/pool/mod.rs` `compute_flags`, replace the
  `Tag::Scheme` arm with:
  ```rust
  // Scheme: propagate PROPAGATE_MASK from the body (types.md §TF-3)
  Tag::Scheme => {
      // Spec: types.md §SC-1 — Scheme extra layout is
      // [var_count, var_id_1, ..., var_id_N, body_idx].
      // The body Idx is the LAST u32 in extra.
      let body_idx = extra[extra.len() - 1] as usize;
      let mut flags = TypeFlags::IS_SCHEME;
      flags |= TypeFlags::propagate_from(self.flags[body_idx]);
      flags
  }
  ```
  Verify the body-index offset against `Pool::scheme_body` (in
  `compiler/ori_types/src/pool/accessors.rs`) — whatever the canonical
  accessor reads, `compute_flags` MUST read from the same layout slot.
  `compute_flags` runs during `Pool::intern` before the body Idx is
  re-wrapped by accessors, so it computes the offset directly from the
  extra slice rather than calling `scheme_body(idx)`.

- [x] Confirm that `propagate_from` exists and already masks to
  `PROPAGATE_MASK` per `types.md §TF-3`. If it does not (or its mask is
  stale), update it in the same change — flag-propagation asymmetries between
  Scheme and other compound tags are themselves `DRIFT` findings.

### 02.0.2 — Regression test

- [x] Add to `compiler/ori_types/src/pool/tests.rs`:
  ```rust
  #[test]
  fn scheme_wrapping_unbound_var_body_propagates_has_var() {
      // Spec: types.md §TF-3 — PROPAGATE_MASK must OR from scheme body.
      let mut pool = Pool::new();
      let var_idx = pool.fresh_var(/* rank */ 0);
      assert!(pool.flags(var_idx).contains(TypeFlags::HAS_VAR));
      let scheme_idx = pool.intern_scheme(&[], var_idx); // ∀. Var — body is the unbound var
      assert!(
          pool.flags(scheme_idx).contains(TypeFlags::HAS_VAR),
          "Tag::Scheme must propagate HAS_VAR from body per types.md §TF-3"
      );
      assert!(pool.flags(scheme_idx).contains(TypeFlags::IS_SCHEME));
  }
  ```
  Exact constructor names (`fresh_var`, `intern_scheme`) must be resolved
  against the real API in `compiler/ori_types/src/pool/construct/` at
  implementation time. If no public constructor exists for an empty-vars
  scheme, add one via `#[cfg(test)]` helper on `Pool` OR use the existing
  generalization path to build a scheme whose body is the unbound var.

- [x] Add a negative companion to prove that a resolved body no longer sets
  `HAS_VAR` on the outer scheme:
  ```rust
  #[test]
  fn scheme_wrapping_resolved_body_does_not_set_has_var() {
      let pool = Pool::new();
      // Scheme whose body is `int` — fully resolved, HAS_VAR must not propagate up.
      let scheme_idx = pool.intern_scheme(&[], Idx::INT);
      assert!(!pool.flags(scheme_idx).contains(TypeFlags::HAS_VAR));
  }
  ```

### 02.0.3 — Completion criteria

- [x] `cargo test -p ori_types --lib pool::tests` passes with both new tests.
- [x] `ORI_DUMP_AFTER_TYPECK=1` on any program containing a non-generalized
  scheme with an unbound body shows `HAS_VAR = true` on the scheme
  (observable via the flags column in the dump).
- [x] No other `compute_flags` arm regressed — `types.md §TF-3` compliance
  is uniform across every compound tag.

---

## 02.1 Validator Signature, Public Contract, and Narrow Re-Export

### 02.1.1 — New files

- [x] `compiler/ori_types/src/check/validators/mod.rs`
  <!-- NEW — created by this section -->
- [x] `compiler/ori_types/src/check/validators/tests.rs`
  <!-- NEW — created by this section -->

### 02.1.2 — Canonical signature

The signature covers BOTH body `expr_types` AND the body's `FunctionSig`
(`typeck.md §CK-4` — fresh `Tag::Var` in unannotated params/returns is
legitimate at signatures-pass exit and MUST be resolved by Bodies-group
exit; §PC-2 makes the whole signature part of the output contract).

```rust
/// Validate that no unbound `Tag::Var` survives in `expr_types`, param
/// types, or the return type after body inference completes, enforcing
/// typeck.md §PC-2 ("no Tag::Var in any type-bearing IR position").
///
/// For every (ExprIndex, Idx) pair in `expr_types`, and for each param
/// type + the return type in `sig`, this function performs a recursive
/// walk of the type tree. The first unbound `Tag::Var` encountered under
/// a given ExprIndex emits one E2005 (impl-hygiene.md §Deduplication by
/// (Code, Span)). Signature positions (params + return) emit against the
/// function-declaration span supplied by the caller.
///
/// Diagnostics are emitted in this deterministic order (impl-hygiene.md
/// §Pass determinism):
///   1. Signature positions in declaration order (param_types[0..N],
///      then return_type).
///   2. Body expressions in ascending ExprIndex order.
///
/// Fast paths (types.md §TF-5):
///   * Before every HAS_VAR check at every walk step, the idx is
///     resolved via `pool.resolve_fully(idx)` — flag caching at intern
///     time predates `VarState::Link` mutation (impl-hygiene.md
///     §Salsa & Caching — flags are stable, vars are not).
///   * Types with `!HAS_VAR` short-circuit the walk.
///   * Types with `HAS_ERROR` short-circuit at the top-level gate
///     (types.md §TK-3, typeck.md §ER-4 — cascade suppression).
///
/// Tag::BoundVar (scheme-quantified) is silently skipped: per
/// types.md §TF-1, Tag::BoundVar sets HAS_BOUND_VAR, not HAS_VAR, so a
/// scheme whose only free variables are bound has HAS_VAR = false on
/// its body (given §02.0's fix to §TF-3 propagation). The walker
/// recurses into scheme bodies via the existing Pool::visit_children
/// helper (compiler/ori_types/src/pool/descriptor.rs:303) — no
/// parallel tag-dispatch ladder (impl-hygiene.md §Algorithmic DRY).
///
/// # Parameters
/// * `pool`        — the type pool for tag/data/flags/resolve queries
/// * `expr_types`  — map from expression index to resolved Idx (the
///                   InferOutput::expr_types field populated during
///                   body inference)
/// * `sig`         — the body's FunctionSig; `sig.param_types` and
///                   `sig.return_type` are walked for surviving vars
/// * `sig_span`    — the function declaration span used for signature
///                   diagnostics (caller-supplied because FunctionSig
///                   itself carries span-free Idx values)
/// * `span_of`     — function mapping an ExprIndex to the source Span
///                   for body diagnostic attribution
/// * `errors`      — mutable accumulator; new TypeCheckError values
///                   are appended (typeck.md §ER-1 — accumulate,
///                   don't bail)
pub fn validate_body_types(
    pool: &Pool,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    sig: &FunctionSig,
    sig_span: Span,
    span_of: &dyn Fn(ExprIndex) -> Span,
    errors: &mut Vec<TypeCheckError>,
)
```

Notes (load-bearing):

- The `&dyn Fn(ExprIndex) -> Span` keeps the validator free of any typed-IR
  handle types (`impl-hygiene.md §Phase Boundaries — Minimal boundary types`).
- `FunctionSig` is reused as-is from `compiler/ori_types/src/output/mod.rs`
  (the typed-IR output module, NOT `check/signatures/`) — the validator SHALL
  NOT duplicate its shape or flatten it into positional args
  (`impl-hygiene.md §SSOT`).
- The validator is pure: inputs are immutable refs + a closure; outputs are
  appended errors. No Salsa, no global state. This keeps it trivially
  callable from any Salsa-tracked query (`typeck.md §SL-1`).

**Section 03 integration note (Gemini finding, verified):**
`check_def_impl_method` in `compiler/ori_types/src/check/bodies/mod.rs`
constructs `param_types: Vec<Idx>` and `return_type: Idx` locally but does
NOT build a `FunctionSig`. The other three bodies-pass call sites
(`check_function_bodies`, `check_test_bodies`, `check_impl_method`) DO have
a `FunctionSig` available. Section 03 will need to either (a) construct a
minimal `FunctionSig` at the `check_def_impl_method` site using the local
param_types/return_type, or (b) accept that the sig validation at that site
uses a locally-constructed FunctionSig. Option (a) is correct: FunctionSig
is a plain data struct, constructing it inline costs nothing, and it keeps
the validator's interface uniform. This is a Section 03 implementation
decision, not a Section 02 concern, but is documented here to prevent the
integration site from being surprised by the mismatch.

### 02.1.3 — Imports (resolved at implementation time)

Whatever `compiler/ori_types/src/check/object_safety.rs` does for imports is
the pattern to follow; deviations are `impl-hygiene.md §Pattern consistency`
violations. The expected set:

```rust
use rustc_hash::FxHashMap;

use ori_ir::Span;

use crate::output::FunctionSig; // FunctionSig lives in output/mod.rs, not check/signatures/
use crate::{ExprIndex, Idx, Pool, TypeCheckError, TypeFlags};
```

### 02.1.4 — Narrow public re-export

- [x] In `compiler/ori_types/src/lib.rs`, keep line 16 as
  `mod check;` (private — no `pub` promotion). Add a narrow re-export:
  ```rust
  pub use check::validators::validate_body_types;
  ```
  Place it in the `pub use check::{...}` block (`lib.rs:32`) so all public
  check-module exports remain clustered. This exposes exactly one function;
  the rest of `check/` stays internal (`impl-hygiene.md §API Stability —
  minimize pub surface`).

### 02.1.5 — Completion criteria

- [x] Signature in `compiler/ori_types/src/check/validators/mod.rs` matches
  §02.1.2 verbatim (param names, param order, `pub` vs `pub(crate)`; the
  exposed function is `pub` because §02.1.4 re-exports it).
- [x] `compiler/ori_types/src/lib.rs` has `mod check;` (unchanged) and a new
  `pub use check::validators::validate_body_types;`. No `pub mod check`.
- [x] `cargo doc -p ori_types` renders the rustdoc block cleanly, including
  cross-references to `Pool`, `FunctionSig`, and `TypeCheckError`.

---

## 02.2 Core Algorithm: Tag-Dispatch Child Recursion (Reusing `Pool::visit_children`)

### 02.2.1 — Algorithmic DRY: reuse, don't clone

`compiler/ori_types/src/pool/descriptor.rs:303` already defines
`Pool::visit_children(&self, idx: Idx, mut f: impl FnMut(Idx))` — a
tag-dispatch ladder covering every compound tag: simple containers (via
`has_child_in_data`), Map, Result, Borrowed, Function, Tuple, Struct, Enum,
Applied, Scheme, plus fallthrough for Named/Alias/Projection/primitives/
variables.

Per `impl-hygiene.md §Algorithmic DRY` (and specifically the
"cross-crate / cross-module" heading + Remediation Hierarchy step 2
"higher-order function"), the validator SHALL call
`Pool::visit_children` instead of building a parallel tag ladder. Concrete
plan:

- [x] Promote `Pool::visit_children` from its current `fn` (private to the
  `pool::descriptor` module) to `pub(crate) fn` in
  `compiler/ori_types/src/pool/descriptor.rs`, so `check::validators` can
  call it. If the shape must change to cover `Tag::Projection` receiver +
  `Tag::Applied` args uniformly (it already does), no change is needed
  beyond visibility.
- [x] If additional compound tags are reachable from typed IR that
  `visit_children` does NOT cover today (e.g., `Tag::Option`, `Tag::Set`,
  `Tag::Range`, `Tag::Iterator`, `Tag::DoubleEndedIterator`, `Tag::Channel`
  — which are `has_child_in_data()` simple containers — or `Tag::List` /
  `Tag::Map` which are already handled), verify coverage by reading the
  helper. If any compound tag reachable from post-inference IR is missing,
  EXTEND the helper (the fix lands in `pool/descriptor.rs`, not in the
  validator). Adding per-tag arms only in `validators/` would re-introduce
  the algorithmic duplication this section is removing.

### 02.2.2 — Walker

```rust
/// Recursively walk the type tree rooted at `ty`, emitting one E2005 at
/// `span` for the first unbound Tag::Var encountered under this walk.
/// Returns `true` iff a diagnostic was emitted on this call. Callers
/// that want "one diagnostic per ExprIndex" short-circuit further
/// recursion at the same ExprIndex once this returns true.
fn collect_first_unbound_var(
    pool: &Pool,
    ty: Idx,
    span: Span,
    errors: &mut Vec<TypeCheckError>,
) -> bool {
    // TF-5 fast-path, staleness-safe: resolve links first, then check.
    let ty = pool.resolve_fully(ty);
    let flags = pool.flags(ty);
    if !flags.contains(TypeFlags::HAS_VAR) {
        return false; // !HAS_VAR ⇒ no unbound var reachable (types.md §TF-3 + §02.0)
    }
    if flags.contains(TypeFlags::HAS_ERROR) {
        return false; // Cascade suppression (types.md §TK-3, typeck.md §ER-4)
    }

    match pool.tag(ty) {
        Tag::Var => {
            // Consult the var's state — only Unbound is a PC-2 violation.
            let var_id = pool.data(ty); // Tag::Var: data IS the var_id
            match pool.var_state(var_id) {
                VarState::Unbound { .. } => {
                    errors.push(TypeCheckError::ambiguous_type(
                        span,
                        var_id,
                        "expression".to_string(),
                    ));
                    true
                }
                // Resolved via link — resolve_fully above should have removed these,
                // but guard defensively.
                VarState::Link { target } => {
                    collect_first_unbound_var(pool, *target, span, errors)
                }
                // Implementation note: the current pool stores generalized
                // vars as Tag::Var(VarState::Generalized), NOT as Tag::BoundVar
                // (diverges from types.md SC-1 which says BoundVar). Because of
                // this, HAS_VAR is set on scheme bodies containing generalized
                // vars — the HAS_VAR gate alone cannot distinguish "free unbound"
                // from "generalized" at the outer scheme level.
                //
                // The validator MUST exempt VarState::Generalized here:
                // rejecting it would fire E2005 on every polymorphic let-binding,
                // breaking let-polymorphism entirely. This is correct behavior for
                // the current implementation.
                //
                // If a future SC-1 conformance fix changes generalized vars to
                // Tag::BoundVar, this arm becomes unreachable and can be removed.
                VarState::Generalized { .. } | VarState::Rigid { .. } => false,
            }
        }

        Tag::BoundVar => {
            // Scheme-quantified — legitimate; HAS_VAR should not be set
            // on BoundVar (types.md §TF-1). If it is, that's a separate
            // pool bug. Silently skip.
            false
        }

        // Every compound tag — recurse via the canonical child walker.
        _ => {
            let mut emitted = false;
            pool.visit_children(ty, |child| {
                if emitted { return; } // dedup: one diagnostic per top-level span
                if collect_first_unbound_var(pool, child, span, errors) {
                    emitted = true;
                }
            });
            emitted
        }
    }
}
```

Key properties:

- **No `bound_vars` parameter.** `Tag::BoundVar` sets `HAS_BOUND_VAR`, not
  `HAS_VAR` (`types.md §TF-1`); the HAS_VAR gate at the top of each call
  naturally admits only types reachable-to-an-unbound-Var, and the
  `Tag::BoundVar` arm is a trivial no-op. The scheme body is reached via
  `visit_children`, which yields `pool.scheme_body(idx)` per
  `pool/descriptor.rs:353`.
- **`resolve_fully` at every step.** Flags are cached at intern time
  (`types.md §TF-2`), but `VarState::Link { target }` may resolve later
  (`types.md §SC-3` substitution, `CHK:UN-7` union-find). The staleness
  window is closed by calling `pool.resolve_fully(idx)` at the top of every
  recursive call, not only at the outermost entry.
- **No `_ => {}` dropping compound tags.** The default arm recurses via
  `visit_children`; if a future tag lands in the pool it is handled by the
  canonical walker, not accidentally silently dropped here (a leak-hardening
  property).
- **`Tag::Projection` is safely treated as a leaf.** `Pool::visit_children`
  does NOT recurse into `Tag::Projection`'s receiver child (line 356 of
  `descriptor.rs` treats it alongside Named/Alias as having no children).
  This is safe for the validator because `typeck.md §PC-2` bullet 3
  guarantees all projections are normalized before bodies-pass exit --
  `Tag::Projection` cannot appear in the validator's input. If a future
  change breaks this guarantee, a `debug_assert!` at the validator entry
  or a `Tag::Projection` arm in `visit_children` would catch it. The
  correct fix would be extending `visit_children` to yield the receiver
  child (since `Projection` IS a compound tag per `types.md §TK-8`),
  NOT adding a special case in the validator.
- **Short-circuit after first diagnostic.** The returned `bool` lets callers
  collapse N unbound Vars under one `ExprIndex` into one E2005
  (`impl-hygiene.md §Deduplication`). Pool-level tree walks remain O(nodes)
  in the worst case for verification, but each `ExprIndex` contributes at
  most one error.

### 02.2.3 — Entry point

```rust
pub fn validate_body_types(
    pool: &Pool,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    sig: &FunctionSig,
    sig_span: Span,
    span_of: &dyn Fn(ExprIndex) -> Span,
    errors: &mut Vec<TypeCheckError>,
) {
    // 1. Signature positions first (declaration order).
    for param_ty in &sig.param_types {
        collect_first_unbound_var(pool, *param_ty, sig_span, errors);
    }
    collect_first_unbound_var(pool, sig.return_type, sig_span, errors);

    // 2. Body expressions in ascending ExprIndex order.
    // FxHashMap iteration is non-deterministic; sort once (impl-hygiene.md
    // §Pass determinism).
    let mut entries: Vec<(ExprIndex, Idx)> = expr_types
        .iter()
        .map(|(&k, &v)| (k, v))
        .collect();
    entries.sort_unstable_by_key(|(idx, _)| *idx);

    for (expr_idx, ty) in entries {
        collect_first_unbound_var(pool, ty, span_of(expr_idx), errors);
    }
}
```

### 02.2.4 — Completion criteria

- [x] `collect_first_unbound_var` delegates compound-tag recursion to
  `pool.visit_children` — grep shows no duplicated per-tag arms for `List`,
  `Map`, `Tuple`, `Struct`, `Enum`, `Applied`, `Function`, `Scheme` inside
  `validators/` beyond the `Tag::Var` / `Tag::BoundVar` discrimination above.
- [x] `pool.resolve_fully(idx)` is called at the top of every recursive
  step (enforce via a rustdoc assertion + a test that inserts a `Link` chain
  and verifies no false positive).
- [x] No `_ => {}` arm in the `match pool.tag(ty)` block that drops a
  compound variant (the catch-all explicitly recurses).

---

## 02.3 `lib.rs` and `check/mod.rs` Wiring (No `pub mod check`)

### 02.3.1 — `check/mod.rs`

**File:** `compiler/ori_types/src/check/mod.rs` (currently 408 lines;
`compiler.md §File Organization` 500-line limit is respected — new
logic lives in `validators/`, not in `check/mod.rs`).

Existing submodule declarations (private): `accessors`, `api`, `bodies`,
`exports`, `imports`, `object_safety`, `registration`, `scope`, `signatures`,
`well_known`, plus `#[cfg(test)] mod integration_tests;` and
`#[cfg(test)] mod tests;`.

- [x] Add, in alphabetical order immediately after `mod signatures;`:
  ```rust
  pub(crate) mod validators;
  ```
  The `pub` here is load-bearing: the re-export at `lib.rs` is
  `pub use check::validators::validate_body_types;`, which requires at
  least `pub(crate)` visibility on `validators`. Using `pub(crate) mod validators;`
  keeps the submodule callable from outside `ori_types` when the single
  re-exported function is accessed via its canonical path for testing.
  Verified at `compiler/ori_types/src/check/mod.rs:77`.

### 02.3.2 — `lib.rs`

**File:** `compiler/ori_types/src/lib.rs`

Current state (verified):

```rust
// Line 16:
mod check;
// Line 25:
pub mod reporting;
// Lines 32–35:
pub use check::{
    check_module, check_module_with_imports, check_module_with_pool,
    check_module_with_registries, ModuleChecker,
};
```

- [x] KEEP `mod check;` (line 16) — do NOT promote to `pub mod check`.
  (The earlier draft of this section proposed promotion mirroring
  `pub mod reporting;`; Phase 2 /tp-help rejected that because the
  check-module hierarchy is strictly internal compiler plumbing — bodies,
  exports, imports, registration, scope, signatures, etc. — and
  `object_safety.rs`'s `pub(crate)` is the real precedent, not
  `reporting`'s `pub`.) Verified at `compiler/ori_types/src/lib.rs:16`.
- [x] Extend the existing `pub use check::{ ... };` block to include
  `validators::validate_body_types`:
  ```rust
  pub use check::{
      check_module, check_module_with_imports, check_module_with_pool,
      check_module_with_registries, ModuleChecker,
  };
  pub use check::validators::validate_body_types;
  ```
  The two `pub use` lines stay separated: the first re-exports names
  directly from `check`; the second reaches into a submodule. Keeping them
  on distinct `pub use` statements matches the crate's existing
  re-export style and makes the narrow scope of the validator export
  grep-visible. Verified at `compiler/ori_types/src/lib.rs:32-36`.

### 02.3.3 — `validators/mod.rs` test module declaration

At the bottom of `compiler/ori_types/src/check/validators/mod.rs`:

- [x] Add:
  ```rust
  #[cfg(test)]
  mod tests;
  ```
  Body in `compiler/ori_types/src/check/validators/tests.rs`
  (`compiler.md §File Organization` — sibling `tests.rs`, not inline).
  Verified at `compiler/ori_types/src/check/validators/mod.rs:231-232`;
  scaffold `tests.rs` exists from §02.1 and §02.4 will populate it.

### 02.3.4 — Completion criteria

- [x] `grep -n "^pub mod check" compiler/ori_types/src/lib.rs` returns
  zero lines (the internal layout is not leaked). Verified 2026-04-15.
- [x] `grep -n "^pub use check::validators::validate_body_types;"
  compiler/ori_types/src/lib.rs` returns exactly one line.
  Verified 2026-04-15 — matches `lib.rs:32`.
- [x] External consumers compile against `ori_types::validate_body_types`
  via the re-export; internal consumers use `crate::check::validators::
  validate_body_types` as usual. Verified — `cargo check -p ori_types
  --tests --lib` succeeds on the current wiring.

---

## 02.4 Unit Test Matrix (Twelve Cells)

Tests land in `compiler/ori_types/src/check/validators/tests.rs`
<!-- NEW — created by this section --> and cover the full positive/negative,
base/scheme/signature/linked/generalized, cascade, determinism, nested
compound depth, and dedup axes per `tests.md §Matrix Testing Rule`. Use the behavioral
naming shape `<subject>_<scenario>_<expected>` (`impl-hygiene.md §Test
Function Naming`).

| Cell | Scenario | Expected |
|------|----------|----------|
| T1 | Negative (Base): `let x = []` — body `expr_types` contains a fresh unbound `Tag::Var` | exactly one `E2005` emitted against the expression's span |
| T2 | Negative (Applied): a user-defined generic type `Applied(MyGeneric, [Var])` — `Tag::Applied` with an unbound `Tag::Var` as a type argument, constructed directly in the pool | exactly one `E2005` — exercises `Tag::Applied` compound-tag walk via `Pool::visit_children` (Note: `Option<_>` is `Tag::Option`, not `Tag::Applied`; `Result<_, _>` is `Tag::Result`; `Tag::Applied` is for registered user generic types only — those dedicated tags are tested via T11 and other cells) |
| T3 | Negative (Signature): `@f(x) = 42` — unannotated param `x` leaves a fresh `Tag::Var` in `FunctionSig.param_types[0]` | exactly one `E2005` emitted against `sig_span` (signature-validation path) |
| T4 | Positive (Resolved Int): `let x: int = 42` | no diagnostic — `HAS_VAR` fast-path gate fires |
| T5 | Positive (Linked): `Tag::Var` with `VarState::Link(int)` in `expr_types` | no diagnostic — `pool.resolve_fully` walks the link before HAS_VAR check |
| T6 | Positive (Scheme BoundVar): body of `let id = x -> x` registered as a `Tag::Scheme` whose body references the scheme's bound var id | no diagnostic — `Tag::BoundVar` sets `HAS_BOUND_VAR`, not `HAS_VAR` |
| T7 | Positive (Generalized): `Tag::Var` with `VarState::Generalized` | no diagnostic — Generalized is not an ambiguous-type violation (the implementation stores generalized vars as `Tag::Var(VarState::Generalized)`, not `Tag::BoundVar`; exempting this arm is correct for let-polymorphism — see algorithm sketch rationale) |
| T8 | Cascade: body with both `HAS_ERROR` AND unbound `Tag::Var` in the same tree | no diagnostic — `HAS_ERROR` short-circuits before the walk reaches the Var (`types.md §TK-3`) |
| T9 | Determinism: three unresolved Vars — one originates from `FunctionSig.param_types[0]` (signature) and two from `expr_types` at `ExprIndex 2` and `ExprIndex 1` respectively | three `E2005` diagnostics emitted in deterministic order: signature diagnostic first (sig_span), then body diagnostics in ascending ExprIndex order (1, then 2) — verifies both ExprIndex ordering AND that signature diagnostics precede body diagnostics |
| T10 | Semantic pin for §02.0: `Tag::Scheme` whose body contains an unbound `Tag::Var`, passed DIRECTLY as an `expr_types` entry (not generalized) | exactly one `E2005` — REQUIRES the §02.0 flag-propagation fix to set `HAS_VAR` on the scheme Idx, otherwise the top-level gate skips and the bug is invisible |
| T11 | Nested compound: `Tag::Option` wrapping `Tag::List` wrapping unbound `Tag::Var` (2-level nesting via dedicated tags) | exactly one `E2005` — exercises `Pool::visit_children` recursive descent through multiple compound levels using dedicated `Tag::Option` and `Tag::List` tags (not `Tag::Applied`) |
| T12 | Dedup: two unbound `Tag::Var` nodes at the same `ExprIndex` (e.g., a `{K: V}` empty map producing key-var + value-var, both unbound, registered under one `ExprIndex`) | exactly one `E2005`, not two — verifies the short-circuit after first emission per `impl-hygiene.md §Deduplication by (Code, Span)` |

### 02.4.1 — Scaffolding pattern

Tests construct a minimal `Pool`, allocate vars and types via the real pool
construction API (not `#[cfg(test)]` helpers added to `Pool` itself — keep
the test surface local per `impl-hygiene.md §Visibility`), build a synthetic
`FxHashMap<ExprIndex, Idx>` and a synthetic `FunctionSig`, invoke
`validate_body_types`, and assert on the accumulated `errors` vec.
`span_of` returns `Span::new(0, 1)` for every `ExprIndex` in unit tests;
`sig_span` uses a distinguishable `Span::new(100, 101)` so T3's assertion
can distinguish signature-origin vs body-origin diagnostics.

Sketch of T1 (the others follow the same mould):

```rust
// In compiler/ori_types/src/check/validators/tests.rs:

use rustc_hash::FxHashMap;

use ori_ir::Span;

use crate::check::validators::validate_body_types;
use crate::output::FunctionSig;
use crate::{ExprIndex, Idx, Pool, TypeErrorKind, TypeFlags};

/// Spec: typeck.md §PC-2 — unbound Tag::Var in expr_types must emit E2005.
#[test]
fn validate_body_types_with_unbound_var_in_expr_types_emits_e2005() {
    let mut pool = Pool::new();
    let var_idx = pool.fresh_var(/* rank */ 0);

    let mut expr_types: FxHashMap<ExprIndex, Idx> = FxHashMap::default();
    expr_types.insert(0usize, var_idx);

    let sig = FunctionSig {
        name: Name::from("test"),
        type_params: vec![],
        const_params: vec![],
        param_names: vec![],
        param_types: vec![],
        return_type: Idx::INT,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params: 0,
        param_defaults: vec![],
        param_hashes: vec![],
        return_hash: 0,
    };
    let mut errors = Vec::new();
    validate_body_types(
        &pool,
        &expr_types,
        &sig,
        Span::new(100, 101),
        &|_| Span::new(0, 1),
        &mut errors,
    );

    assert_eq!(errors.len(), 1, "one E2005 per ExprIndex");
    assert!(matches!(
        errors[0].kind(),
        TypeErrorKind::AmbiguousType { .. }
    ));
    assert_eq!(errors[0].span(), Span::new(0, 1));
}
```

**`FunctionSig` construction for tests:** `FunctionSig` is a plain
`pub struct` with all-pub fields (verified at
`compiler/ori_types/src/output/mod.rs:373`), so direct construction is
possible without a test fixture method. The test can construct it inline:
```rust
let sig = FunctionSig {
    name: Name::from("test"),  // or a suitable Name constant
    type_params: vec![],
    const_params: vec![],
    param_names: vec![],
    param_types: vec![],
    return_type: Idx::INT,
    capabilities: vec![],
    is_public: false,
    is_test: false,
    is_main: false,
    is_fbip: false,
    type_param_bounds: vec![],
    where_clauses: vec![],
    generic_param_mapping: vec![],
    scheme_var_ids: vec![],
    required_params: 0,
    param_defaults: vec![],
    param_hashes: vec![],
    return_hash: 0,
};
```
If field count growth makes this unwieldy, add a `#[cfg(test)]` helper
in `output/tests.rs` (the sibling test file for `output/mod.rs`), NOT
in `validators/tests.rs` (keeps FunctionSig construction local to its
module, `impl-hygiene.md §SSOT`).

### 02.4.2 — Completion criteria

- [x] All twelve cells present in
  `compiler/ori_types/src/check/validators/tests.rs`, each with a
  behavioral name + a `///` doc comment citing the spec clause and/or
  rule anchor it pins.
- [x] T10 would FAIL if §02.0 is reverted (semantic pin — documented in its
  `///` block). Temporarily revert §02.0 locally, confirm T10 fails,
  restore §02.0, confirm T10 passes. Verified 2026-04-15 — reverting the
  `Tag::Scheme` propagation block in `pool/mod.rs:655-661` to return only
  `TypeFlags::IS_SCHEME` caused `scheme_wrapping_unbound_var_body_emits_one_e2005`
  to fail with the defensive `HAS_VAR` flag assertion; restoring the
  propagation block made it pass.
- [x] `cargo test -p ori_types --lib check::validators::tests` passes
  (12/12 on 2026-04-15).
- [x] No cell uses an ephemeral identifier (plan name, section number,
  bug ID) in its test function name (`impl-hygiene.md §Test Function
  Naming`). Behavioral `<subject>_<scenario>_<expected>` shape throughout;
  provenance in `///` doc comments only.

---

## 02.R Third Party Review Findings

- [x] **[TPR-02-001-codex][high] VarState::Generalized exemption needs design rationale.**
  The algorithm sketch exempts `VarState::Generalized` from E2005 without explaining
  why. The implementation stores generalized vars as `Tag::Var(VarState::Generalized)`,
  not `Tag::BoundVar` (types.md SC-1 divergence), so `HAS_VAR` is set on scheme bodies
  containing generalized vars. Rejecting them would break let-polymorphism.
  **Resolution:** Added a comment block in the `VarState::Generalized` arm of
  `collect_first_unbound_var` explaining the SC-1 divergence, the correct behavior,
  and the future clean-up path. Updated T7's description to reference this rationale.

- [x] **[TPR-02-002-codex][medium] T2 and T11 use `Option<_>` which hits `Tag::Option`, not `Tag::Applied`.**
  The matrix claimed `Tag::Applied` coverage but neither cell actually exercised it.
  `Option<_>` is a dedicated `Tag::Option` tag; `Tag::Applied` is for user-defined
  generic types only.
  **Resolution:** Changed T2 to use a `Tag::Applied` type constructed directly in
  the pool (user-defined generic). Changed T11 to use `Tag::Option` wrapping
  `Tag::List` (dedicated tag path). Added a note to T2 clarifying the tag taxonomy.

- [x] **[TPR-02-003-codex][medium] §03 and §05 reference stale §02 API.**
  The old 4-param signature and `pub mod check` design referenced by §03 and §05
  are superseded by the current 6-param signature and private `mod check` design.
  **Resolution:** Added a checklist item in §02.N requiring §03 and §05 to be
  updated to reference the final 6-parameter API, private `mod check` design, and
  twelve-cell validator matrix before §02 close-out.

- [x] **[TPR-02-004-codex][low] Test scaffolding uses wrong API.**
  The T1 sketch used `FunctionSig::test_fixture_with_return` (non-existent) and
  `required_params: vec![]` (wrong type — the real field is `usize`).
  **Resolution:** Replaced the `FunctionSig::test_fixture_with_return` call in
  the T1 sketch with the inline construction pattern already shown in §02.4.1.
  Changed `required_params: vec![]` to `required_params: 0` in both the T1 sketch
  and the inline construction example.

- [x] **[TPR-02-001-gemini][medium] Missing dedup test cell.**
  No test cell explicitly verified that multiple unbound `Tag::Var` nodes under
  one `ExprIndex` produce exactly one E2005, not two. T8 tests `HAS_ERROR`
  cascade suppression — a different dedup path.
  **Resolution:** Added T12 (Dedup): "Two unbound `Tag::Var` nodes at the same
  `ExprIndex` produce exactly one E2005." Updated all "eleven cells" references
  to "twelve cells" throughout the file (frontmatter success_criteria, section
  title, section header, completion criteria item).

- [x] **[TPR-02-002-gemini][medium] T9 tests ExprIndex ordering but not sig-before-body ordering.**
  The determinism cell did not verify that signature diagnostics precede body
  diagnostics, leaving the `sig → body` ordering guarantee untested.
  **Resolution:** Updated T9 to include one diagnostic from `FunctionSig.param_types[0]`
  (signature) and two from `expr_types` at ascending indices, verifying that all
  three emit in order: sig first, then body in ascending ExprIndex order.

- [x] **[TPR-02-003-gemini][low] Positive/Negative label convention inverted.**
  T1 (emits E2005 = error path) was labeled "Positive" and T4 (clean validation)
  was labeled "Negative". By convention, Negative = error path (rejects bad input),
  Positive = clean path (accepts good input).
  **Resolution:** Swapped labels throughout the matrix: T1, T2, T3 (emit E2005)
  → "Negative"; T4, T5, T6, T7 (produce no diagnostic) → "Positive". Updated
  the frontmatter success_criteria note to spell out the convention explicitly.

**TPR Round 3 findings (iteration 2):**

- [x] **[TPR-02-R3-001-codex][high] Sync Section 05 to twelve-cell matrix.**
  §05 still references the old matrix structure.
  **Resolution:** Tracked by §02.N sync checklist item — §05 will be updated
  when §02 implementation completes and the final matrix is confirmed.

- [x] **[TPR-02-R3-002-codex][medium] Resync overview to final validator contract.**
  Overview architecture diagram showed old 4-arg call shape.
  **Resolution:** Fixed in 00-overview.md — updated to 6-arg signature and
  "twelve cells" on 2026-04-15.

- [x] **[TPR-02-R3-003-codex][high] Rewrite Section 03 to current API.**
  §03 still references old 4-param signature and `pub mod check`.
  **Resolution:** Tracked by §02.N sync checklist item — §03 will be updated
  when §02 implementation completes. Single-section review scope; §03 edits
  deferred to §02.N sync gate with concrete `- [ ]` anchor.

- [x] **[TPR-02-R3-004-codex][medium] Replace placeholder span recipe in §03.**
  §03 uses `Span::new(0, 1)` placeholder instead of real ExprIndex mapping.
  **Resolution:** Tracked by §02.N sync checklist item — §03 span recipe will
  be updated with the concrete `ExprId::from_raw(expr_index as u32)` path
  when §02 implementation confirms the mapping contract.

**TPR Round 4 findings (§02.N close-out review):**

- [x] **[TPR-02-R4-001-codex][medium] Gate order drift: `collect_first_unbound_var` checks `!HAS_VAR` BEFORE `HAS_ERROR`, contradicting the documented `resolve_fully → HAS_ERROR → HAS_VAR` contract.**
  `DRIFT [ER-4, TF-5]`: `compiler/ori_types/src/check/validators/mod.rs:161`
  The fast-path gate at lines 161-164 returned on `!HAS_VAR` BEFORE
  consulting `HAS_ERROR` at lines 166-168. The plan's §02.4 T8
  description (line 736) and the Rules Brief both specified the
  order `resolve_fully → HAS_ERROR → HAS_VAR` (cascade suppression
  precedes the walk gate per `types.md §TK-3` + `typeck.md §ER-4`).
  The T8 test cell (`Tuple(Var, Error)` — both flags set) passed
  under either order because the two gates are semantically
  equivalent when both flags are propagated — so the matrix could
  not distinguish the implemented order from the contract. No type
  exists that would emit under one order but not the other (any
  leaf with HAS_ERROR has no reachable Var to emit about), so this
  is DRIFT, not a correctness bug. But matching the contract is
  required to keep cascade suppression robust against future
  flag-propagation changes (e.g., if `HAS_VAR` stopped propagating
  through error-typed compound nodes, the pre-fix order would
  incorrectly walk into them before suppressing).
  **Resolution:** Fixed on 2026-04-15 in `check/validators/mod.rs:156-180`.
  Swapped the two gates so `HAS_ERROR` fires first. Expanded the
  surrounding comment block into three numbered steps explaining
  the contract (1: `resolve_fully` before flag reads for staleness
  safety; 2: `HAS_ERROR` cascade suppression before walk; 3: `HAS_VAR`
  fast-path). All 12 cells of the §02.4 matrix pass unchanged
  (`cargo test -p ori_types --lib check::validators::tests` — 12/12
  ok). Reviewer: codex (attempt 3+4 envelopes — gemini capacity
  errors prevented dual-source confirmation on this round, but the
  finding was independently verified against the actual code at
  line 161 before being acted on, per `CLAUDE.md §Reviewer grounding`).

---

## 02.N Completion Checklist

At section completion (mirrors 01.N / 03.N shape), before flipping
`status: complete`:

- [x] All of §§02.0, 02.1, 02.2, 02.3, 02.4 checkboxes ticked.
- [x] `timeout 150 ./test-all.sh` passes (debug build). Verified 2026-04-15:
  15,341 tests across Rust unit / runtime / AOT / interpreter spec pass.
  LLVM spec backend crash filed as `BUG-04-085` (unrelated subsystem —
  ArcIrEmitter + monomorphization of imported `assert_eq`); gate treats
  as non-failing per existing BUG-04-030 weakening (note: BUG-04-030 is
  resolved; retargeting the gate's reference to BUG-04-085 is a follow-up
  tracked in the BUG-04-085 filing).
- [x] `timeout 150 cargo test --release -p ori_types` passes (release build).
  Verified 2026-04-15: 820/820 lib tests pass.
- [x] `timeout 150 ./clippy-all.sh` clean — no new warnings.
- [x] `cargo doc -p ori_types` renders the new `validate_body_types` rustdoc
  block without broken intra-doc links. Verified 2026-04-15: no warnings
  mentioning `validators` or `validate_body_types`; 11 pre-existing warnings
  in unrelated files (`type_error/problem/mod.rs` etc.) predate this section.
- [x] `diagnostics/repo-hygiene.sh --check` clean — no untracked temp files.
- [x] `/tpr-review` run on this section's diff; findings accepted into
  `02.R` and resolved per `CLAUDE.md §NEVER reason out of TPR findings`.
  Run on 2026-04-15, scratch `/tmp/ori-tpr-CTYOnQbk`. **Codex-only best-effort
  clean** (user-accepted): gemini-3.1-pro-preview hit `gemini_api_capacity`
  on 5/5 retry attempts (~45min total, persistent upstream capacity
  pressure, not a prompt issue). Codex completed 4 full runs with deep
  investigation (7 rules consulted, 25 files read, 2 tests rerun incl.
  §02.0 semantic pin, basis=fresh_verification, confidence=high). One
  finding surfaced — TPR-02-R4-001-codex [medium] DRIFT [ER-4, TF-5] in
  gate order at `validators/mod.rs:161` — independently verified against
  the code per `CLAUDE.md §Reviewer grounding`, fixed in commit
  `342731aa`, and recorded in §02.R above. Prior §02 review-plan cycles
  (R1 + R3) were full dual-source; 11 findings resolved across those
  rounds. User accepted codex-only on 2026-04-15 as the best-effort
  close-out path given the upstream infra block — gemini retry to reach
  full dual-source consensus remains open as a backlog item if the user
  wants to re-verify once capacity recovers.
- [x] `/impl-hygiene-review` run AFTER `/tpr-review` is clean — no new
  `LEAK` / `DRIFT` / `GAP` findings introduced by this section.
  Run on 2026-04-15, auto-scoped to the §02 work arc. Four passes
  (LEAK/SSOT, Algorithmic DRY, Boundary/Flow, Surface Hygiene) + Phase 0
  auto-lint. Phase 4 `/tp-help` cross-check skipped — codex-only for the
  same gemini capacity reason as Phase B1; the earlier codex TPR already
  served as the third-party review for this work and cited
  `impl-hygiene.md` rule anchors. Outcome: **§02-scope code is clean**
  — no NEW LEAK/DRIFT/GAP introduced. Two pre-existing findings surfaced
  in adjacent code (not §02's responsibility but tracked per CLAUDE.md
  "ownership irrelevant" rule):
  (1) `[BUG-02-007][high]` LEAK:algorithmic-duplication in
      `unify/generalization.rs::collect_free_vars_inner` — clones
      `Pool::visit_children`'s tag-dispatch ladder. Extraction path
      documented in the bug entry.
  (2) `[BUG-02-006][minor]` BLOAT in `ori_types/src/pool/` — `mod.rs`
      699 LOC, `descriptor.rs` 510 LOC, three over-100-line functions.
      Submodule-extraction plan documented in the bug entry.
  Eight self-introduced BLOAT banners in `validators/tests.rs` were
  auto-fixed by `hygiene-lint.py --fix --apply` (no commit ref on that
  alone — bundled into the same commit as the bug-tracker entries and
  this checklist update).
- [x] `/improve-tooling` retrospective sweep — any pain points encountered
  while implementing §§02.0–02.4 captured as concrete tracked items.
  Run on 2026-04-15 as the Phase B3 section-close safety net. Verified
  per-subsection retrospectives ran: §§02.0–02.3 closed out in prior
  session commits with their own retrospectives (per the plan's standard
  close-out pattern); §02.4 retrospective documented "no tooling gaps
  — pool construction API, `FunctionSig::simple`, and cargo test path
  selectors worked fluently." Cross-subsection pattern analysis
  surfaced ONE concrete tooling gap invisible at per-subsection scope:
  (1) `[BUG-07-012][minor]` dual-tpr transport discards codex's
      successful envelopes when gemini fails persistently — no
      `codex.final.envelope.json` preserved on infra failure. Filed
      with full repro + proposed fix. Blast radius across all
      dual-source consumer skills (tpr-review, review-work, tp-help,
      review-plan), so `/fix-bug BUG-07-012` will run full TPR +
      hygiene review on the transport change.
  Not implemented inline because the fix touches the shared dual-tpr
  transport which is used by multiple skills and deserves its own
  dedicated review cycle. Two other observations did NOT warrant
  filing: intel-graph underuse in Pass 2 DRY scan is a process
  improvement (documentation, not tooling); T10 semantic-pin
  revert/restore is one-off enough that automating it would be
  premature abstraction.
- [x] `/sync-claude` — `.claude/rules/typeck.md`, `types.md`, and
  `CLAUDE.md` audited for drift introduced by the new public symbol and
  the §02.0 flag-propagation fix; updates committed if needed.
  Run on 2026-04-15 as the Phase C1 doc sync. Drift audit found two
  gaps:
  (1) `typeck.md §PC-2` (output contract) did not mention
      `validate_body_types` as the producer-side enforcement point.
      Added a paragraph documenting the gate order, `visit_children`
      delegation, and the consumer-only `debug_assert!` risk without it.
  (2) `typeck.md §17 Key Files` did not list
      `compiler/ori_types/src/check/validators/mod.rs` or its sibling
      test file. Added two rows after `check/object_safety.rs` (the
      pattern precedent).
  (3) `canon.md §4.2` (type-checker output invariants) likewise did not
      point at the producer-side enforcement. Added a paragraph.
  `types.md §TF-3` already specified the correct `PROPAGATE_MASK`
  behavior for `Tag::Scheme` — the §02.0 fix brought code into compliance
  with existing docs, no doc update needed. `CLAUDE.md §Key Paths` is a
  top-level directory index; adding a validator submodule would bloat
  it. No CLAUDE.md changes.
- [x] Sections 03, 05, `00-overview.md` synced to final §02 API
  (TPR-02-R3-001, R3-003, R3-004 resolutions). Completed as Phase C2
  alongside `/sync-claude`: §03's dependency block rewritten to the
  shipped six-parameter validator signature with narrow re-export
  (resolves TPR-02-R3-003); §03's code-example span placeholder replaced
  with the concrete `arena.get_expr(ExprId::from_raw(expr_index as u32)).span`
  recipe (resolves TPR-02-R3-004); §05's test matrix table synced to
  the twelve shipped test names with behavioral naming + T10 semantic-pin
  note (resolves TPR-02-R3-001); `00-overview.md` effort table updated
  from "11 cells" to "12 cells".
- [x] `/commit-push` — single commit or ordered commit sequence per
  `CLAUDE.md §Stabilization Discipline` (multi-commit ordering).
  Completed as an ordered commit sequence across the §02 close-out:
  `6e47956a` (§02.0 pool fix), `19f68757` (§02.1-02.3 validator
  skeleton + wiring), `c41c2bcd` (§02.4 twelve-cell matrix), `342731aa`
  (TPR-02-R4-001 gate-order fix), `3a9b24fb` (§02 hygiene review —
  banner removal + bug filings), `dd267552` (§02 tooling section-close
  sweep — BUG-07-012 filing), `0d5dd41f` (§02 docs sync — rules +
  downstream plan sections to final API). This final commit closes
  out the section.
- [x] Frontmatter: this section's `status` → `complete`; all subsection
  `status` → `complete`.
  All §§02.0–02.4 subsection statuses: complete. §02.R: complete (15/15
  findings resolved across R1, R3, R4). §02.N: complete (this checklist).
  Section frontmatter status flipped from `in-progress` to `complete` in
  this commit.
- [x] `plans/empty-container-typeck-phase-contract/00-overview.md`
  "Quick Reference" table: Section 02 row → `Complete`.
  Updated in this commit.
- [x] `plans/empty-container-typeck-phase-contract/index.md` updated
  (status, Section 02 link text) if the corpus convention requires it.
  `index.md` Quick Reference uses an ID/Title/File layout (no Status
  column), so no update needed per the corpus convention. The plan's
  `index.md` frontmatter is for the reroute registry and isn't
  section-specific.
- [x] `plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md`
  frontmatter `depends_on: ["01", "02"]` — confirm no new external blocker
  was introduced.
  Verified on 2026-04-15: `depends_on: ["01", "02"]` at
  `section-03-bodies-pass-integration.md:22`. Both §01 and §02 are now
  `status: complete`, so §03 has zero unresolved external blockers and
  can pick up next when `/continue-roadmap plans/empty-container-typeck-phase-contract`
  is resumed.
- [x] Sections 03, 05, `00-overview.md`, and `index.md` updated to reference
  the final 6-parameter `validate_body_types` API (pool, expr_types, sig,
  sig_span, span_of, errors), the private `mod check;` design (no
  `pub mod check`), and the
  twelve-cell validator matrix (T1–T12).
  Completed as Phase C2 (commit `0d5dd41f`) — see earlier `/sync-claude`
  + Phase C2 line-item for details. Cross-reference consolidated.

---

## Cross-Section File Contention

The following files are touched by §02 AND by other sections. The ordering
dependency is noted to prevent merge conflicts and partial-state commits
(`CLAUDE.md §Stabilization Discipline`):

| File | §02 scope | Other section(s) | Ordering note |
|------|-----------|-------------------|---------------|
| `check/validators/mod.rs` | Created | §05 (test matrix references) | §02 creates; §05 reads |
| `check/validators/tests.rs` | Created | §05 (adds matrix cells) | §02 creates with T1-T12; §05 may add additional end-to-end cells |
| `pool/mod.rs` | §02.0 (compute_flags fix) | None in this plan | No contention |
| `pool/descriptor.rs` | §02.2 (visibility promotion) | None in this plan | No contention |
| `check/mod.rs` | §02.3 (`pub(crate) mod validators;`) | §03 (calls from bodies/) | §02 adds module declaration; §03 adds call sites in sibling bodies/ module |
| `lib.rs` | §02.3 (narrow re-export) | None in this plan | No contention |

---

## API Divergences from User Instructions

The following divergences from earlier drafts / the Phase 1 /tp-help briefing
were identified by reading the actual source files and the /tp-help
consensus. The plan above uses the verified real API throughout:

1. **`pool.children(ty)` does not exist.** The earlier draft listed this as
   the child-recursion API. The real pool has tag-specific accessors only.
   The canonical tag-dispatch walker is `Pool::visit_children` at
   `compiler/ori_types/src/pool/descriptor.rs:303` — §02.2 reuses it
   (`impl-hygiene.md §Algorithmic DRY`).

2. **`pool.var_id(ty)` does not exist.** The earlier draft listed
   `pool.var_id(ty)` as the accessor for the var_id of a `Tag::Var`. The
   real API is `pool.data(ty)` which returns the `u32` var_id for a
   `Tag::Var` per `types.md` Appendix B ("data = var_id" for the type-
   variable range).

3. **`pool.scheme_vars(idx)` returns `&[u32]`, not `Vec<u32>`.** Not a
   concern for this section anymore — the scheme-aware walk no longer
   consults `scheme_vars` at all. Scheme bodies are reached via
   `Pool::visit_children → pool.scheme_body(idx)`, and `HAS_VAR` on the
   outer scheme (given §02.0's fix) is sufficient to decide whether to
   recurse.

4. **`TypeCheckError::ambiguous_type` is confirmed.** Real at
   `compiler/ori_types/src/type_error/check_error/constructors.rs` (the
   earlier bare path `check_error/mod.rs` was imprecise — the
   `TypeCheckError::ambiguous_type` constructor lives in the sibling
   `constructors.rs` module inside `check_error/`; the plan-audit
   `DEAD_PATH` finding is addressed by citing the qualified directory
   `compiler/ori_types/src/type_error/check_error/` and deferring the exact
   file to implementation time).

5. **`lib.rs:16` has `mod check;` (private).** The earlier draft promoted
   this to `pub mod check;` mirroring the `pub mod reporting;` pattern at
   line 25. Phase 2 /tp-help CONSENSUSED against the promotion — it would
   leak the whole internal `check/` hierarchy as stable API. This section
   §02.3.2 instead adds a single narrow re-export
   `pub use check::validators::validate_body_types;`, matching the
   `pub(crate)` / single-function-exposure pattern of
   `compiler/ori_types/src/check/object_safety.rs`.

6. **`ExprIndex` is a type alias `usize`**, not a newtype (verified at
   `compiler/ori_types/src/infer/mod.rs`). The `FxHashMap<ExprIndex, Idx>`
   uses `usize` keys; sorting by `*idx` works directly.

7. **`bound_vars: &[u32]` parameter removed.** The earlier draft threaded a
   scheme-bound-var-id set through every recursive call. Phase 2 /tp-help
   CONSENSUSED that `HAS_VAR` alone distinguishes free from bound vars per
   `types.md §TF-1` (`Tag::BoundVar` sets `HAS_BOUND_VAR`, not `HAS_VAR`),
   so the set is redundant. Signature dropped.

8. **`Tag::Scheme` compute_flags arm is a bug, not a constraint.** The
   earlier draft treated the pool's `HAS_VAR = false` on schemes as fixed.
   §02.0 fixes it per `types.md §TF-3`; T10 in §02.4 is the semantic pin.
