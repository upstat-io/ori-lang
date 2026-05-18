//! Producer-side enforcement of.
//!
//! This module contains [`validate_body_types`] — the canonical validator
//! that walks a function body's `expr_types` and `FunctionSig` after
//! `InferEngine` body-checking completes, identifies any surviving unbound
//! [`Tag::Var`] per expression/signature position, and emits `E2005`
//! (`AmbiguousType`) once per [`ExprIndex`].
//!
//! # Why this exists
//!
//! (PC-2) mandates that no
//! [`Tag::Var`] survive in any type-bearing position of the typed IR.
//! Without producer-side enforcement, violations would leak through to
//! eval / ARC / canon / codegen — where downstream `debug_assert!`s
//! would fire only in debug builds, and release builds could silently
//! miscompile.
//!
//! # Design notes
//!
//! - Scope spans **both** body [`expr_types`](ExprIndex) and the body's
//!   [`FunctionSig`] (`param_types` + `return_type`) per:
//!   unannotated params / returns ship out of the signatures pass as fresh
//!   [`Tag::Var`]s and MUST resolve before Bodies-group exit.
//! - Walks compound types via the canonical `Pool::visit_children` helper
//!   (`compiler/ori_types/src/pool/descriptor.rs:303`) rather than cloning a
//!   parallel tag-dispatch ladder.
//! - Calls [`Pool::resolve_fully`] at the top of every recursive step to
//!   close the stale-flag window: flags are cached at intern time
//!   (TF-2), but [`VarState::Link`] may resolve later (`CHK:UN-7`
//!   union-find). The `HAS_VAR` fast-path gate is only sound after link
//!   resolution at each step.
//! - Emits one diagnostic per ([`ExprIndex`], diagnostic-code) site
//!   ; multiple unbound
//!   vars under one [`ExprIndex`] collapse to a single `E2005`.
//! - The `Tag::Scheme` path relies on `§02.0`'s `PROPAGATE_MASK` fix to
//!   `Pool::compute_flags` —.
//!
//! # Dependencies
//!
//! - `Pool::visit_children` must be visible to this module (`pub(crate)`).
//! - The `HAS_VAR` propagation fix for `Tag::Scheme` (`compute_flags` in
//!   `compiler/ori_types/src/pool/mod.rs`) must be in place; otherwise
//!   schemes wrapping unbound-var bodies would be skipped by the top-level
//!   gate (TF-5).

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::{ExprKind, Span};

use crate::output::FunctionSig;
use crate::pool::VarState;
use crate::tag::Tag;
use crate::{AmbiguousTypeSite, ExprIndex, Idx, Pool, TypeCheckError, TypeFlags};

/// Source-attribution closures the validator calls while walking body
/// expressions: look up the source [`Span`] of an [`ExprIndex`], look up
/// the source [`ExprKind`] (for [`AmbiguousTypeSite`] classification), and
/// look up a specific lambda parameter's token span (for
/// [`AmbiguousTypeSite::LambdaParam`] span narrowing per plan §06.1 item
/// 2). Bundled into one struct per `compiler.md §API` — the three closures
/// always travel together.
///
/// - `span` — callable that maps an [`ExprIndex`] to the source [`Span`]
///   for body diagnostic attribution.
/// - `kind` — callable that maps an [`ExprIndex`] to its source
///   [`ExprKind`] when one is available. Used to classify the error site
///   for specialized E2005 wording per plan §06.1 (empty lists get a
///   `[int]`-annotation hint; lambda params get a typed-lambda hint).
///   Callers that cannot look up an `ExprKind` (invalid index) return
///   `None`; the site defaults to `AmbiguousTypeSite::Expression`.
/// - `param_span` — callable that maps a `(Lambda ExprIndex, param_index)`
///   pair to the source [`Span`] of that parameter's token. Used to narrow
///   the E2005 diagnostic to the specific parameter that carries the
///   unresolved `Tag::Var` per plan §06.1 item 2 — the whole-lambda span
///   is too wide. Returns `None` when the `ExprIndex` does not point at a
///   `Lambda` or when `param_index` is out of range; the validator falls
///   back to the lambda's own span.
pub struct ValidatorContext<'a> {
    /// Callable: [`ExprIndex`] → source [`Span`] for body diagnostics.
    pub span: &'a dyn Fn(ExprIndex) -> Span,
    /// Callable: [`ExprIndex`] → [`ExprKind`] for site classification.
    pub kind: &'a dyn Fn(ExprIndex) -> Option<ExprKind>,
    /// Callable: `(Lambda ExprIndex, param_index)` → parameter-token span.
    pub param_span: &'a dyn Fn(ExprIndex, usize) -> Option<Span>,
}

/// Classify the error site from the `ExprKind` at the validator's top-level
/// body-expression entry point. Signature positions (no `ExprKind` in scope)
/// pass `None` and default to `AmbiguousTypeSite::Expression`.
///
/// The classification drives specialized E2005 wording per plan §06.1:
/// empty list literals (both `List([])` and `ListWithSpread([])`) map to
/// `EmptyList`; `Lambda` expressions map to `LambdaParam`; everything else
/// (including empty maps/sets per plan §06.1) falls back to `Expression`.
fn site_from_expr_kind(kind: Option<ExprKind>) -> AmbiguousTypeSite {
    match kind {
        Some(ExprKind::List(r)) if r.is_empty() => AmbiguousTypeSite::EmptyList,
        Some(ExprKind::ListWithSpread(r)) if r.is_empty() => AmbiguousTypeSite::EmptyList,
        Some(ExprKind::Lambda { .. }) => AmbiguousTypeSite::LambdaParam,
        _ => AmbiguousTypeSite::Expression,
    }
}

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
/// - `ctx` — source-attribution closures bundled into
///   [`ValidatorContext`] (span, expr-kind, and param-span lookups).
///   Bundling keeps the parameter count within `compiler.md §API`'s
///   `>3 params → config struct` guidance and lets callers construct
///   the closure set once per body-checking pass.
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
    ctx: &ValidatorContext<'_>,
    errors: &mut Vec<TypeCheckError>,
) {
    // Build the exempt root set: scheme var ids + their union-find roots.
    // This allows the validator to distinguish "unbound because it's a
    // generic parameter" from "unbound because inference genuinely failed"
    // even when union-find made a fresh instantiation var the root
    let exempt = build_exempt_var_ids(pool, scheme_var_ids);

    // 1. Signature positions first (declaration order):
    //    param_types[0..N], then return_type. No ExprKind in scope at
    //    signature positions — the site is always `Expression` (generic
    //    fallback wording per plan §06.1 item 4).
    let sig_site = AmbiguousTypeSite::Expression;
    for param_ty in &sig.param_types {
        collect_first_unbound_var(pool, *param_ty, sig_span, sig_site, &exempt, errors);
    }
    collect_first_unbound_var(pool, sig.return_type, sig_span, sig_site, &exempt, errors);

    // 2. Body expressions in ascending ExprIndex order.
    //    FxHashMap iteration is non-deterministic; sort once
    //
    let mut entries: Vec<(ExprIndex, Idx)> = expr_types.iter().map(|(&k, &v)| (k, v)).collect();
    entries.sort_unstable_by_key(|(idx, _)| *idx);

    for (expr_idx, ty) in entries {
        let site = site_from_expr_kind((ctx.kind)(expr_idx));
        // Plan §06.1 item 2: for LambdaParam sites, narrow the primary
        // diagnostic span from the whole-lambda span to the specific
        // parameter-token span that carries the unresolved `Tag::Var`.
        // The whole-lambda span (e.g., `x -> x.method()`) is too wide for a
        // targeted "add a type annotation to this parameter" message. Fall
        // back to the lambda's own span if the Lambda's return type is the
        // unresolved position (no narrow param available) or if the type
        // shape is not a function (defensive).
        let primary_span = match site {
            AmbiguousTypeSite::LambdaParam => {
                narrow_to_lambda_param_span(pool, ty, expr_idx, ctx.param_span)
                    .unwrap_or_else(|| (ctx.span)(expr_idx))
            }
            _ => (ctx.span)(expr_idx),
        };
        collect_first_unbound_var(pool, ty, primary_span, site, &exempt, errors);
    }
}

/// Narrow the diagnostic span for a `LambdaParam` site to the specific
/// parameter-token span that carries the unresolved `Tag::Var`.
///
/// Plan §06.1 item 2: the lambda expression's span covers the entire
/// `x -> body` form, but the E2005 wording "cannot infer the type of this
/// closure parameter; add a full typed-lambda annotation like `(x: int) ->
/// ReturnT = body`" is actionable only against the parameter token. This
/// helper resolves the lambda's inferred type, unwraps any enclosing
/// [`Tag::Scheme`] (polymorphic lambdas carry a scheme wrapper per
/// `types.md §SC-1`), and walks the function signature's parameter types
/// for the first one containing an unresolved `Tag::Var` (via
/// [`TypeFlags::HAS_VAR`] per `types.md §TF-5`).
///
/// Returns `None` when:
/// - The lambda's ty does not resolve to a `Tag::Function` (target shape is
///   `Function` or `Scheme(Function(...))`; anything else is defensive).
/// - No parameter carries `HAS_VAR` (the return-type slot may be the
///   unresolved position; callers fall back to the lambda's own span).
/// - `param_span_of(expr_idx, i)` returns `None` (the `ExprIndex` does
///   not point at a `Lambda` or `i` is out of range).
///
/// `HAS_VAR` is the canonical fast-path gate (`types.md §TF-3` propagation
/// and `§02.0` scheme-flag fix) — it is set iff an unbound `Tag::Var` is
/// reachable under the type. The helper reads it after `resolve_fully`
/// (staleness guard per `types.md §TF-2`).
fn narrow_to_lambda_param_span(
    pool: &Pool,
    ty: Idx,
    expr_idx: ExprIndex,
    param_span_of: &dyn Fn(ExprIndex, usize) -> Option<Span>,
) -> Option<Span> {
    // Resolve the lambda's ty; unwrap a `Tag::Scheme` wrapper (poly-lambda
    // shape per `types.md §SC-1`: `Scheme([var_ids], Function(...))`).
    let resolved = pool.resolve_fully(ty);
    let fn_ty = match pool.tag(resolved) {
        Tag::Function => resolved,
        Tag::Scheme => {
            let body = pool.resolve_fully(pool.scheme_body(resolved));
            if pool.tag(body) == Tag::Function {
                body
            } else {
                return None;
            }
        }
        _ => return None,
    };

    // Walk param types in declaration order; return the span of the first
    // one carrying `HAS_VAR`. Resolve-then-check closes the staleness
    // window (TF-2).
    let param_count = pool.function_param_count(fn_ty);
    for i in 0..param_count {
        let p = pool.resolve_fully(pool.function_param(fn_ty, i));
        if pool.flags(p).contains(TypeFlags::HAS_VAR) {
            return param_span_of(expr_idx, i);
        }
    }
    None
}

/// Build the set of `var_id`s that are legitimate scheme-parameter vars
/// or their union-find equivalence-class roots. Any `VarState::Unbound`
/// var whose `var_id` is in this set is a polymorphic type parameter, not
/// an inference failure.
///
/// Uses [`Pool::var_idx_for_id`] (the canonical `var_id` → Idx helper)
/// to avoid duplicating the linear-scan pattern from monomorphization
///
pub fn build_exempt_var_ids(pool: &Pool, scheme_var_ids: &[u32]) -> FxHashSet<u32> {
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
///
/// `site` is the site classification selected by the caller per `ExprKind`
/// at the error position (plan §06.1). It is threaded unchanged through
/// recursion — classification is a property of the top-level expression,
/// not of its type subchildren.
fn collect_first_unbound_var(
    pool: &Pool,
    ty: Idx,
    span: Span,
    site: AmbiguousTypeSite,
    exempt: &FxHashSet<u32>,
    errors: &mut Vec<TypeCheckError>,
) -> bool {
    // Gate order: resolve_fully → HAS_ERROR → HAS_VAR.
    //
    // (1) `resolve_fully` first, so flags are read on the resolved Idx.
    //     Flags are cached at intern time (TF-2) but
    //     `VarState::Link` may mutate later. Resolving before
    //     flag reads keeps the HAS_VAR / HAS_ERROR checks staleness-safe.
    //
    // (2) HAS_ERROR gate BEFORE the HAS_VAR fast-path. Cascade
    //     suppression must precede
    //     the walk gate so that any position already poisoned by a type
    //     error never emits follow-on E2005 diagnostics — even if some
    //     future refactor changes how HAS_VAR is cleared on error-typed
    //     compound nodes. The two orderings are behaviorally equivalent
    //     today (HAS_VAR is false on pure `Tag::Error` leaves), but
    //     matching the documented contract makes cascade suppression
    //     robust against future flag-propagation changes.
    //
    // (3) HAS_VAR gate last (TF-5). This is the fast-path
    //     that keeps the walk O(1) for types with no unresolved
    //     variables (`§TF-3` propagation + `§02.0` scheme fix ensures
    //     the flag is set iff an unbound Var is reachable).
    let ty = pool.resolve_fully(ty);
    let flags = pool.flags(ty);
    if flags.contains(TypeFlags::HAS_ERROR) {
        // Cascade suppression.
        return false;
    }
    if !flags.contains(TypeFlags::HAS_VAR) {
        // !HAS_VAR ⇒ no unbound var reachable
        // (TF-3).
        return false;
    }

    match pool.tag(ty) {
        Tag::Var => check_var_tag(pool, ty, span, site, exempt, errors),

        // Scheme-quantified — legitimate; HAS_VAR should not be set on
        // BoundVar (TF-1). If it is, that's a separate pool
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
                if collect_first_unbound_var(pool, child, span, site, exempt, errors) {
                    emitted = true;
                }
            });
            emitted
        }
    }
}

/// Handle the `Tag::Var` arm of `collect_first_unbound_var`.
///
/// Dispatches on `VarState`: `Unbound` / `Generalized` (non-exempt) emit `E2005`;
/// `Link` recurses; `Rigid` and exempt scheme-vars are legitimate (return `false`).
///
/// # Why this is a helper
///
/// migration (§08.3b.1) adds the `Generalized → E2005` leak-alarm
/// path mirroring the `Unbound` path. Extracting this arm keeps the parent's
/// nesting depth ≤ 4 and its length < 100.
fn check_var_tag(
    pool: &Pool,
    ty: Idx,
    span: Span,
    site: AmbiguousTypeSite,
    exempt: &FxHashSet<u32>,
    errors: &mut Vec<TypeCheckError>,
) -> bool {
    let var_id = pool.data(ty); // Tag::Var: data IS the var_id
    match pool.var_state(var_id) {
        // Unbound: inference failure — PC-2 violation unless exempt.
        //
        // Generalized: post-§08.3b.1 leak alarm — after the end-of-body
        // normalization pass in `check::bodies` rewrites every generalized
        // leaf to `Tag::BoundVar` per, a surviving
        // `Generalized` here means the pass missed a position. Treat
        // identically to Unbound (same exempt-scheme-var semantics, same
        // `E2005` emission).
        //
        // Scheme-var exemption applies to both: if `var_id` is in the
        // exempt root set — either a scheme var itself or a fresh
        // instantiation var that became the union-find root of a scheme
        // var's equivalence class — it is a legitimate polymorphic
        // parameter, not an inference failure.
        VarState::Unbound { .. } | VarState::Generalized { .. } => {
            emit_ambiguous_if_not_exempt(var_id, span, site, exempt, errors)
        }
        // Resolved via link — resolve_fully above should have removed these,
        // but guard defensively by recursing on the target.
        VarState::Link { target } => {
            collect_first_unbound_var(pool, *target, span, site, exempt, errors)
        }
        // Rigid remains exempt — user-annotated parametric type parameters
        // are legitimate at PC-2 exit (`§UN-6`).
        VarState::Rigid { .. } => false,
    }
}

/// Emit `E2005` for `var_id` at `span` with the caller-supplied `site`
/// classification, unless the var is in the exempt set.
///
/// Returns `true` iff a diagnostic was pushed.
fn emit_ambiguous_if_not_exempt(
    var_id: u32,
    span: Span,
    site: AmbiguousTypeSite,
    exempt: &FxHashSet<u32>,
    errors: &mut Vec<TypeCheckError>,
) -> bool {
    if exempt.contains(&var_id) {
        return false;
    }
    errors.push(TypeCheckError::ambiguous_type(span, var_id, site));
    true
}

pub mod partial_move;

pub use partial_move::{validate_drop_partial_move, validate_partial_move};

#[cfg(test)]
mod tests;
