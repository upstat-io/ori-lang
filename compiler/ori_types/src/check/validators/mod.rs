//! Producer-side enforcement of typeck.md §PC-2.
//!
//! This module contains [`validate_body_types`] — the canonical validator
//! that walks a function body's `expr_types` and `FunctionSig` after
//! `InferEngine` body-checking completes, identifies any surviving unbound
//! [`Tag::Var`] per expression/signature position, and emits `E2005`
//! (`AmbiguousType`) once per [`ExprIndex`].
//!
//! # Why this exists
//!
//! `typeck.md §PC-2` (mirrored in `types.md §PC-2`) mandates that no
//! [`Tag::Var`] survive in any type-bearing position of the typed IR.
//! Without producer-side enforcement, violations would leak through to
//! eval / ARC / canon / codegen — where downstream `debug_assert!`s
//! would fire only in debug builds, and release builds could silently
//! miscompile.
//!
//! # Design notes
//!
//! - Scope spans **both** body [`expr_types`](ExprIndex) and the body's
//!   [`FunctionSig`] (`param_types` + `return_type`) per `typeck.md §CK-4`:
//!   unannotated params / returns ship out of the signatures pass as fresh
//!   [`Tag::Var`]s and MUST resolve before Bodies-group exit.
//! - Walks compound types via the canonical `Pool::visit_children` helper
//!   (`compiler/ori_types/src/pool/descriptor.rs:303`) rather than cloning a
//!   parallel tag-dispatch ladder (`impl-hygiene.md §Algorithmic DRY`).
//! - Calls [`Pool::resolve_fully`] at the top of every recursive step to
//!   close the stale-flag window: flags are cached at intern time
//!   (`types.md §TF-2`), but [`VarState::Link`] may resolve later (`CHK:UN-7`
//!   union-find). The `HAS_VAR` fast-path gate is only sound after link
//!   resolution at each step.
//! - Emits one diagnostic per ([`ExprIndex`], diagnostic-code) site
//!   (`impl-hygiene.md §Deduplication by (Code, Span)`); multiple unbound
//!   vars under one [`ExprIndex`] collapse to a single `E2005`.
//! - The `Tag::Scheme` path relies on `§02.0`'s `PROPAGATE_MASK` fix to
//!   `Pool::compute_flags` — see `types.md §TF-3`.
//!
//! # Dependencies
//!
//! - `Pool::visit_children` must be visible to this module (`pub(crate)`).
//! - The `HAS_VAR` propagation fix for `Tag::Scheme` (`compute_flags` in
//!   `compiler/ori_types/src/pool/mod.rs`) must be in place; otherwise
//!   schemes wrapping unbound-var bodies would be skipped by the top-level
//!   gate (`types.md §TF-5`).

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Span;

use crate::output::FunctionSig;
use crate::pool::VarState;
use crate::tag::Tag;
use crate::{ExprIndex, Idx, Pool, TypeCheckError, TypeFlags};

/// Validate that no unbound [`Tag::Var`] survives in `expr_types`, param
/// types, or the return type after body inference completes, enforcing
/// `typeck.md §PC-2` ("no `Tag::Var` in any type-bearing IR position").
///
/// For every `(ExprIndex, Idx)` pair in `expr_types`, and for each param
/// type + the return type in `sig`, this function performs a recursive
/// walk of the type tree. The first unbound [`Tag::Var`] encountered under
/// a given position emits one `E2005`
/// (`impl-hygiene.md §Deduplication by (Code, Span)`). Signature positions
/// (params + return) emit against the function-declaration span supplied
/// by the caller.
///
/// Diagnostics are emitted in this deterministic order
/// (`impl-hygiene.md §Pass determinism`):
///   1. Signature positions in declaration order
///      (`param_types[0..N]`, then `return_type`).
///   2. Body expressions in ascending [`ExprIndex`] order.
///
/// Fast paths (`types.md §TF-5`):
///   - Before every [`TypeFlags::HAS_VAR`] check at every walk step, the
///     idx is resolved via [`Pool::resolve_fully`] — flag caching at intern
///     time predates [`VarState::Link`] mutation (`impl-hygiene.md §Salsa &
///     Caching` — flags are stable, vars are not).
///   - Types with `!HAS_VAR` short-circuit the walk.
///   - Types with [`TypeFlags::HAS_ERROR`] short-circuit at the top-level
///     gate (`types.md §TK-3`, `typeck.md §ER-4` — cascade suppression).
///
/// [`Tag::BoundVar`] (scheme-quantified) is silently skipped: per
/// `types.md §TF-1`, `Tag::BoundVar` sets `HAS_BOUND_VAR`, not `HAS_VAR`,
/// so a scheme whose only free variables are bound has `HAS_VAR = false`
/// on its body (given `§02.0`'s fix to `§TF-3` propagation). The walker
/// recurses into scheme bodies via the canonical `Pool::visit_children`
/// helper (`compiler/ori_types/src/pool/descriptor.rs:303`) — no parallel
/// tag-dispatch ladder (`impl-hygiene.md §Algorithmic DRY`).
///
/// # Parameters
///
/// - `pool` — the type pool for tag/data/flags/resolve queries.
/// - `expr_types` — map from expression index to resolved [`Idx`] (the
///   `InferOutput::expr_types` field populated during body inference).
/// - `sig` — the body's [`FunctionSig`]; `sig.param_types` and
///   `sig.return_type` are walked for surviving vars.
/// - `sig_span` — the function-declaration span used for signature
///   diagnostics (caller-supplied because [`FunctionSig`] itself carries
///   span-free [`Idx`] values).
/// - `scheme_var_ids` — the caller function's scheme-quantified var ids
///   (from [`FunctionSig::scheme_var_ids`]). For generic functions these
///   are the `var_id`s that LEGITIMATELY remain `VarState::Unbound` after
///   body inference — they represent polymorphic type parameters, not
///   inference failures. The validator builds an **exempt root set** from
///   these ids: each scheme var's union-find root is resolved and added
///   to the set so that fresh instantiation vars (which may have become
///   roots due to rank-weighted union-find direction) are also exempted.
///   Pass `&[]` for monomorphic functions.
/// - `span_of` — function mapping an [`ExprIndex`] to the source [`Span`]
///   for body diagnostic attribution.
/// - `errors` — mutable accumulator; new [`TypeCheckError`] values are
///   appended (`typeck.md §ER-1` — accumulate, don't bail).
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the crate-wide choice for expr_types maps \
              produced by InferEngine; callers are internal and always \
              use FxHashMap. Exposing a generic BuildHasher parameter \
              would add callsite noise with no benefit."
)]
pub fn validate_body_types(
    pool: &Pool,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    sig: &FunctionSig,
    sig_span: Span,
    scheme_var_ids: &[u32],
    span_of: &dyn Fn(ExprIndex) -> Span,
    errors: &mut Vec<TypeCheckError>,
) {
    // Build the exempt root set: scheme var ids + their union-find roots.
    // This allows the validator to distinguish "unbound because it's a
    // generic parameter" from "unbound because inference genuinely failed"
    // even when union-find made a fresh instantiation var the root
    let exempt = build_exempt_var_ids(pool, scheme_var_ids);

    // 1. Signature positions first (declaration order):
    //    param_types[0..N], then return_type.
    for param_ty in &sig.param_types {
        collect_first_unbound_var(pool, *param_ty, sig_span, &exempt, errors);
    }
    collect_first_unbound_var(pool, sig.return_type, sig_span, &exempt, errors);

    // 2. Body expressions in ascending ExprIndex order.
    //    FxHashMap iteration is non-deterministic; sort once
    //    (impl-hygiene.md §Pass determinism).
    let mut entries: Vec<(ExprIndex, Idx)> = expr_types.iter().map(|(&k, &v)| (k, v)).collect();
    entries.sort_unstable_by_key(|(idx, _)| *idx);

    for (expr_idx, ty) in entries {
        collect_first_unbound_var(pool, ty, span_of(expr_idx), &exempt, errors);
    }
}

/// Build the set of `var_id`s that are legitimate scheme-parameter vars
/// or their union-find equivalence-class roots. Any `VarState::Unbound`
/// var whose `var_id` is in this set is a polymorphic type parameter, not
/// an inference failure.
///
/// Uses [`Pool::var_idx_for_id`] (the canonical `var_id` → Idx helper)
/// to avoid duplicating the linear-scan pattern from monomorphization
/// (`impl-hygiene.md §Algorithmic DRY`).
pub(crate) fn build_exempt_var_ids(pool: &Pool, scheme_var_ids: &[u32]) -> FxHashSet<u32> {
    let mut exempt = FxHashSet::default();
    exempt.extend(scheme_var_ids.iter().copied());
    for &sv_id in scheme_var_ids {
        if let Some(sv_idx) = pool.var_idx_for_id(sv_id) {
            let root = pool.resolve_fully(sv_idx);
            if pool.tag(root) == Tag::Var {
                exempt.insert(pool.data(root));
            }
        }
    }
    exempt
}

/// Recursively walk the type tree rooted at `ty`, emitting one `E2005` at
/// `span` for the first unbound [`Tag::Var`] encountered under this walk.
///
/// Returns `true` iff a diagnostic was emitted on this call. Callers that
/// want "one diagnostic per position" short-circuit further recursion once
/// this returns `true` (handled by the `emitted` flag inside the
/// `visit_children` closure below).
///
/// The walk is guarded by the [`TypeFlags::HAS_VAR`] /
/// [`TypeFlags::HAS_ERROR`] fast paths and delegates compound-tag
/// traversal to `Pool::visit_children`. There is no `_ => {}` arm in the
/// `match pool.tag(ty)` below that silently drops a compound variant —
/// the catch-all explicitly recurses via `visit_children`.
fn collect_first_unbound_var(
    pool: &Pool,
    ty: Idx,
    span: Span,
    exempt: &FxHashSet<u32>,
    errors: &mut Vec<TypeCheckError>,
) -> bool {
    // Gate order: resolve_fully → HAS_ERROR → HAS_VAR.
    //
    // (1) `resolve_fully` first, so flags are read on the resolved Idx.
    //     Flags are cached at intern time (`types.md §TF-2`) but
    //     `VarState::Link` may mutate later (`types.md §SC-3`
    //     substitution, `typeck.md §UN-7` union-find). Resolving before
    //     flag reads keeps the HAS_VAR / HAS_ERROR checks staleness-safe.
    //
    // (2) HAS_ERROR gate BEFORE the HAS_VAR fast-path. Cascade
    //     suppression (`types.md §TK-3`, `typeck.md §ER-4`) must precede
    //     the walk gate so that any position already poisoned by a type
    //     error never emits follow-on E2005 diagnostics — even if some
    //     future refactor changes how HAS_VAR is cleared on error-typed
    //     compound nodes. The two orderings are behaviorally equivalent
    //     today (HAS_VAR is false on pure `Tag::Error` leaves), but
    //     matching the documented contract makes cascade suppression
    //     robust against future flag-propagation changes.
    //
    // (3) HAS_VAR gate last (`types.md §TF-5`). This is the fast-path
    //     that keeps the walk O(1) for types with no unresolved
    //     variables (`§TF-3` propagation + `§02.0` scheme fix ensures
    //     the flag is set iff an unbound Var is reachable).
    let ty = pool.resolve_fully(ty);
    let flags = pool.flags(ty);
    if flags.contains(TypeFlags::HAS_ERROR) {
        // Cascade suppression (types.md §TK-3, typeck.md §ER-4).
        return false;
    }
    if !flags.contains(TypeFlags::HAS_VAR) {
        // !HAS_VAR ⇒ no unbound var reachable
        // (types.md §TF-3 propagation + §02.0 scheme fix).
        return false;
    }

    match pool.tag(ty) {
        Tag::Var => {
            // Consult the var's state — only Unbound is a PC-2 violation.
            let var_id = pool.data(ty); // Tag::Var: data IS the var_id
            match pool.var_state(var_id) {
                VarState::Unbound { .. } => {
                    // Scheme-var exemption: if this unbound
                    // `var_id` is in the exempt root set — either a scheme
                    // var itself or a fresh instantiation var that became
                    // the union-find root of a scheme var's equivalence
                    // class — it is a legitimate polymorphic parameter,
                    // not an inference failure.
                    if exempt.contains(&var_id) {
                        return false;
                    }
                    errors.push(TypeCheckError::ambiguous_type(
                        span,
                        var_id,
                        "expression".to_string(),
                    ));
                    true
                }
                // Resolved via link — resolve_fully above should have
                // removed these, but guard defensively.
                VarState::Link { target } => {
                    collect_first_unbound_var(pool, *target, span, exempt, errors)
                }
                // §08.3b — `Tag::Var(VarState::Generalized)` is a partial-
                // migration residue. Scheme BODIES are now rewritten to
                // `Tag::BoundVar` per `types.md §SC-1`
                // (`unify::generalization::rewrite_generalized_to_bound_var`),
                // but `expr_types` and `FunctionSig.param_types` for
                // let-polymorphic lambdas still hold the pre-generalize
                // `Tag::Var` leaves whose `var_state` was mutated to
                // `Generalized` in place. Until a follow-up subsection
                // ports those positions to `Tag::BoundVar`, the validator
                // continues to exempt `Generalized` as a defensive net —
                // stripping this arm without porting expr_types would
                // fire `E2005` on every polymorphic let-binding
                // (`CLAUDE.md §INVERTED-TDD`).
                //
                // `Rigid` is exempted because user-annotated parametric
                // type parameters are legitimate at PC-2 exit (`§UN-6`).
                VarState::Generalized { .. } | VarState::Rigid { .. } => false,
            }
        }

        // Scheme-quantified — legitimate; HAS_VAR should not be set on
        // BoundVar (types.md §TF-1). If it is, that's a separate pool
        // bug. Silently skip.
        Tag::BoundVar => false,

        // Every compound tag — recurse via the canonical child walker.
        // No _ => {} arm silently drops a compound variant; visit_children
        // itself handles Named/Alias/Projection as leaves (descriptor.rs:356).
        _ => {
            let mut emitted = false;
            pool.visit_children(ty, |child| {
                if emitted {
                    // Dedup: one diagnostic per top-level span.
                    return;
                }
                if collect_first_unbound_var(pool, child, span, exempt, errors) {
                    emitted = true;
                }
            });
            emitted
        }
    }
}

#[cfg(test)]
mod tests;
