//! Function body type checking passes.
//!
//! This module implements Passes 2-5 of module type checking:
//! - Pass 2: Function bodies (`functions::check_function_bodies`)
//! - Pass 3: Test bodies (`functions::check_test_bodies`)
//! - Pass 4: Impl method bodies (`impls::check_impl_bodies`)
//! - Pass 5: Def impl (default implementation) method bodies (`def_impls::check_def_impl_bodies`)
//!
//! # Architecture
//!
//! Each function body is checked in a child environment that:
//! 1. Inherits from the frozen base environment (contains all function signatures)
//! 2. Has parameter bindings added
//! 3. Has function scope context set (`current_function`, capabilities)
//!
//! ```text
//! Base Environment (frozen after Pass 1)
//!   - child for function foo
//!     - param: x -> int
//!     - param: y -> str
//!   - child for function bar
//!     - param: n -> int
//! ```
//!
//! # File layout
//!
//! `bodies/mod.rs` is a thin dispatch hub. Body-checking logic lives in the
//! submodule that owns each pass — `functions.rs` (Passes 2–3), `impls.rs`
//! (Pass 4), and `def_impls.rs` (Pass 5); `method_sig.rs` carries the
//! method-binder + signature-construction helpers Passes 4–5 share. Each
//! public pass-dispatch function has exactly one canonical home; `mod.rs`
//! re-exports via `pub use` so `check::bodies::check_function_bodies`
//! and siblings continue to resolve without changing import paths.

mod contracts;
mod def_impls;
mod functions;
mod impls;
mod method_sig;

pub(super) use def_impls::check_def_impl_bodies;
pub(super) use functions::{check_function_bodies, check_test_bodies};
pub(super) use impls::check_impl_bodies;
pub(crate) use method_sig::allocate_rigid_var_map;

use ori_ir::{ExprId, ExprKind};
use rustc_hash::FxHashMap;

use crate::check::validators::{
    validate_body_types, validate_consumed_binding, validate_drop_partial_move,
    validate_partial_move, ValidatorContext,
};
use crate::check::ModuleChecker;
use crate::output::FunctionSig;
use crate::registry::burden::UserBurdenSpec;
use crate::{
    DeferredMonoCall, ExprIndex, Idx, MonoInstance, MonoInstanceId, PatternKey, PatternResolution,
    TypeCheckError, TypeCheckWarning,
};

/// Outputs drained from the `InferEngine` at end-of-body, plus the defaulted
/// `expr_types` map. Consumed by [`finalize_body_and_export`].
pub(super) struct BodyOutputs {
    pub expr_types: FxHashMap<ExprIndex, Idx>,
    pub errors: Vec<TypeCheckError>,
    pub warnings: Vec<TypeCheckWarning>,
    pub pat_resolutions: Vec<(PatternKey, PatternResolution)>,
    pub mono_instances: Vec<MonoInstance>,
    /// Pre-dedup `(call_expr_id, MonoInstanceId)` entries from this body's
    /// `InferEngine`. Indices are body-local positions into `mono_instances`;
    /// `accumulate_mono_session` re-anchors them into module-wide positions
    /// when the spine absorbs both vectors together.
    pub mono_dispatch_pre_dedup: Vec<(ExprId, MonoInstanceId)>,
    pub deferred_mono_calls: Vec<DeferredMonoCall>,
    /// Composed `UserBurdenSpec` entries produced by this body's
    /// monomorphization sites (one per first-instantiation of a
    /// fully-resolved generic-builtin `Idx`). Flushed into
    /// `TypeRegistry` via
    /// [`crate::check::ModuleChecker::flush_composed_burdens`] in
    /// [`finalize_body_and_export`]; downstream codegen reads the
    /// registered spec via `TypeRegistry::burden(idx)` without
    /// re-deriving.
    pub composed_burdens: Vec<(Idx, UserBurdenSpec)>,
    /// Pool `var_id`s of capability marker vars (`uses Cap`) bound in the body.
    /// A no-self `Cap.method(...)` call leaves its receiver var unconstrained by
    /// design (the concrete type is provided by the caller's `with...in`); these
    /// `var_id`s are merged into the `validate_body_types` exempt set so they do
    /// not surface a spurious E2005. Empty for bodies without `uses` capabilities.
    pub capability_exempt_var_ids: Vec<u32>,
    /// Type-directed desugar plans for `ExprKind::AssignTarget` chains in this
    /// body. Keys are module-wide AST `ExprId`s; `accumulate_assign_desugars`
    /// extends the checker's accumulator without re-anchoring. Consumed by
    /// `ori_canon` via [`crate::TypedModule::assign_desugar_map`].
    pub assign_desugars: Vec<(ExprId, crate::AssignDesugar)>,
    /// Module-alias qualified-call rewrite entries resolved in this body. Keys
    /// are module-wide AST `ExprId`s; `accumulate_module_alias_calls` extends
    /// the checker's accumulator. Consumed by `ori_canon` via
    /// [`crate::TypedModule::module_alias_call_map`].
    pub module_alias_calls: Vec<(ExprId, ori_ir::Name)>,
    /// Iterable->Iterator routed method calls resolved in this body.
    /// Keys are exact source call `ExprId`s; values carry the iterator type and
    /// optional eager-adapter type consumed by `ori_canon`.
    pub iter_route_desugars: Vec<(ExprId, crate::IterMethodRoute)>,
}

/// Shared post-inference spine for every body-checking pass.
///
/// Runs the PC-2 validator, stores expression types into the checker, pushes
/// accumulated errors / warnings, and extends pattern-resolution / mono /
/// deferred-call vectors. Covers only the post-body common work — each
/// caller still owns its own parameter binding, inference closure, and
/// signature construction / export.
pub(super) fn finalize_body_and_export(
    checker: &mut ModuleChecker<'_>,
    sig: &FunctionSig,
    sig_span: ori_ir::Span,
    body_root: ExprId,
    outputs: BodyOutputs,
) {
    let BodyOutputs {
        expr_types,
        errors,
        warnings,
        pat_resolutions,
        mono_instances,
        mono_dispatch_pre_dedup,
        deferred_mono_calls,
        composed_burdens,
        capability_exempt_var_ids,
        assign_desugars,
        module_alias_calls,
        iter_route_desugars,
    } = outputs;

    // Validate PC-2 contract: no unbound Tag::Var in expr_types or sig positions.
    run_validator(
        checker,
        &expr_types,
        sig,
        sig_span,
        &capability_exempt_var_ids,
    );

    // Validate conditional-partial-move rejection — emits E2043 for
    // asymmetric field projections of non-Drop owned aggregates across
    // branches of `if`/`match`. Producer-side guard so class-ledger Step-4b
    // emission never receives a CFG shape requiring path-sensitive partial-
    // release reconciliation.
    run_partial_move_validator(checker, &expr_types, body_root);

    // Validate Drop-partial-move rejection — emits E2048 for `let $f =
    // v.field` bindings where `v`'s type implements `Drop`. Per
    // `drop-trait-proposal.md §Execution Timing`, partial moves on Drop
    // types are forbidden regardless of CFG path (axis disjoint from
    // E2043). Producer-side guard so the compiler-walked field drop
    // after the user `@drop` body never observes absent fields.
    run_drop_partial_move_validator(checker, &expr_types, body_root);

    // Validate use-after-`drop_early` rejection — emits E2054 for a use of a
    // binding after `drop_early(value: x)` consumed it (Spec Clause 13 §13.7).
    // Producer-side guard so a consumed binding never reaches the burden path
    // as a live read of reclaimed memory (use-after-free).
    run_consumed_binding_validator(checker, body_root);

    // Store expression types.
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }

    // Store errors and warnings.
    for error in errors {
        checker.push_error(error);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }

    // Accumulate pattern resolutions, mono outputs, and deferred calls.
    // `accumulate_mono_session` extends both `mono_instances` and
    // `mono_dispatch_pre_dedup` together so dispatch entries are
    // re-anchored from body-local to module-wide positions in lockstep.
    checker.pattern_resolutions.extend(pat_resolutions);
    checker.accumulate_mono_session(mono_instances, mono_dispatch_pre_dedup);
    checker.accumulate_deferred_mono_calls(deferred_mono_calls);
    checker.accumulate_assign_desugars(assign_desugars);
    checker.accumulate_module_alias_calls(module_alias_calls);
    checker.accumulate_iter_routes(iter_route_desugars);

    // Flush composed burdens into the TypeRegistry. Each entry was
    // accumulated by the body's `InferEngine::record_composed_burden`
    // calls; flushing here makes the registered specs visible to
    // downstream codegen via `TypeRegistry::burden(idx)`. Late-stage
    // monomorphization sites in subsequent bodies see prior entries
    // through the same registry surface and dedup against them at the
    // signature-reverse-index gate.
    checker.flush_composed_burdens(composed_burdens);
}

/// Shared PC-2 contract enforcement for every body-checking pass.
///
/// After inference and the end-of-body defaulting pass both complete, walks
/// `expr_types` and `sig` (`param_types` + `return_type`) for any surviving
/// unbound [`Tag::Var`] and emits `E2005` per position. Errors are
/// accumulated via `checker.push_error` alongside normal inference errors.
///
/// `ExprIndex` → `u32` conversion is lossless by construction (see
/// `infer/mod.rs` `store_type` call sites, which cap `ExprIndex` at
/// `u32::MAX`). Falls back to `Span::DUMMY` on the impossible overflow path
/// rather than indexing the arena with `ExprId::INVALID`.
///
/// Consumers: `check_function` / `check_test` in `functions.rs` and
/// `check_impl_method` / `check_def_impl_method` in `impls.rs`. Lives in
/// `mod.rs` so both submodules can call it — the four body passes share the
/// identical validation skeleton.
pub(super) fn run_validator(
    checker: &mut ModuleChecker<'_>,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    sig: &FunctionSig,
    sig_span: ori_ir::Span,
    capability_exempt_var_ids: &[u32],
) {
    let validation_errors: Vec<TypeCheckError> = {
        // Scope the immutable borrows (pool, arena) so the subsequent
        // mutable borrow (push_error) can proceed without conflict.
        let arena = checker.arena();
        let pool = checker.pool();
        let mut errs: Vec<TypeCheckError> = Vec::new();
        // Bundle source-attribution closures (>3 params → config struct).
        // All three closures need the same `arena` borrow;
        // `ValidatorContext` bundles them into a single argument of
        // `validate_body_types`.
        let span_of = |expr_index| {
            u32::try_from(expr_index)
                .map(|raw| arena.get_expr(ExprId::new(raw)).span)
                .unwrap_or(ori_ir::Span::DUMMY)
        };
        let expr_kind_of = |expr_index| {
            u32::try_from(expr_index)
                .ok()
                .map(|raw| arena.get_expr(ExprId::new(raw)).kind)
        };
        // Resolve the param-token span for a Lambda ExprIndex + 0-based
        // param index. Returns `None` when the
        // ExprIndex does not point at a `Lambda` (validator falls back
        // to the lambda-wide span) or when `param_index` is out of
        // range. Spans flow from the parser-allocated `Param.span` at
        // parse time; the arena (`ori_ir::ExprArena::get_params`) owns
        // the range.
        let param_span_of = |expr_index, param_index: usize| {
            u32::try_from(expr_index).ok().and_then(|raw| {
                let expr = arena.get_expr(ExprId::new(raw));
                if let ExprKind::Lambda { params, .. } = expr.kind {
                    arena.get_params(params).get(param_index).map(|p| p.span)
                } else {
                    None
                }
            })
        };
        let ctx = ValidatorContext {
            span: &span_of,
            kind: &expr_kind_of,
            param_span: &param_span_of,
        };
        // Exempt the function's quantified type-var ids AND any capability
        // marker var_ids (a no-self `Cap.method(...)` leaves its receiver var
        // unconstrained by design) from the E2005 unbound-var check.
        let mut exempt_var_ids = sig.scheme_var_ids.clone();
        exempt_var_ids.extend_from_slice(capability_exempt_var_ids);
        validate_body_types(
            pool,
            expr_types,
            sig,
            sig_span,
            &exempt_var_ids,
            &ctx,
            &mut errs,
        );
        errs
    };
    for err in validation_errors {
        checker.push_error(err);
    }
}

/// Shared conditional-partial-move enforcement for every body-checking
/// pass.
///
/// Walks `body_root`'s AST top-down and emits `E2043`
/// (`EBURDEN_CONDITIONAL_PARTIAL_MOVE`) for any non-Drop owned aggregate
/// whose field is projected asymmetrically across the arms of an `if` or
/// `match`. Producer-side guard whose ordering invariant is "typeck
/// rejects BEFORE Phase 5 emits". Without producer enforcement, Phase 5
/// ARC lowering would see patterns that violate its
/// "`moved_out_fields[v]` statically computable per-CFG-path" invariant
/// and would have to either fall back to fixpoint dataflow (out of
/// trivial-emission scope) or silently miscompile.
///
/// Errors append to `checker.push_error` alongside normal inference and
/// PC-2 validation errors.
pub(super) fn run_partial_move_validator(
    checker: &mut ModuleChecker<'_>,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    body_root: ExprId,
) {
    let validation_errors: Vec<TypeCheckError> = {
        let arena = checker.arena();
        let pool = checker.pool();
        let mut errs: Vec<TypeCheckError> = Vec::new();
        validate_partial_move(pool, arena, expr_types, body_root, &mut errs);
        errs
    };
    for err in validation_errors {
        checker.push_error(err);
    }
}

/// Shared E2048 Drop-partial-move enforcement for every body-checking
/// pass.
///
/// Walks `body_root`'s AST top-down and emits `E2048`
/// (`EDROP_PARTIAL_MOVE`) for any `let $f = v.field` binding whose
/// receiver type implements `Drop`. Producer-side guard so the
/// compiler-walked field drop in the AUGMENT path (`drop-trait-proposal.md
/// §Execution Timing`) never observes absent fields.
///
/// Disjoint from `run_partial_move_validator` (E2043, conditional
/// non-Drop case): the E2048 axis covers EVERY partial move on a Drop
/// type, regardless of CFG path.
///
/// Resolves the `Drop` trait by interning its source name; when the
/// trait is not yet wired into the prelude / registry, the validator
/// silently no-ops (pre-deployment shape).
pub(super) fn run_drop_partial_move_validator(
    checker: &mut ModuleChecker<'_>,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    body_root: ExprId,
) {
    let validation_errors: Vec<TypeCheckError> = {
        let arena = checker.arena();
        let pool = checker.pool();
        let trait_registry = checker.trait_registry();
        let drop_trait_name = checker.interner().intern("Drop");
        let mut errs: Vec<TypeCheckError> = Vec::new();
        validate_drop_partial_move(
            pool,
            arena,
            expr_types,
            trait_registry,
            drop_trait_name,
            body_root,
            &mut errs,
        );
        errs
    };
    for err in validation_errors {
        checker.push_error(err);
    }
}

/// Shared E2054 use-after-`drop_early` enforcement for every body-checking
/// pass.
///
/// Walks `body_root`'s AST in execution order and emits `E2054`
/// (`EUSE_AFTER_DROP_EARLY`) for any use of a binding after
/// `drop_early(value: x)` consumed it (Spec Clause 13 §13.7). Producer-side
/// guard so a consumed binding never reaches the burden path as a live read
/// of reclaimed memory.
///
/// Recognises `drop_early` by interning its prelude name; when the name never
/// appears in a call the walk is a no-op.
pub(super) fn run_consumed_binding_validator(checker: &mut ModuleChecker<'_>, body_root: ExprId) {
    let validation_errors: Vec<TypeCheckError> = {
        let arena = checker.arena();
        let drop_early_name = checker.interner().intern("drop_early");
        let mut errs: Vec<TypeCheckError> = Vec::new();
        validate_consumed_binding(arena, drop_early_name, body_root, &mut errs);
        errs
    };
    for err in validation_errors {
        checker.push_error(err);
    }
}

#[cfg(test)]
mod tests;
