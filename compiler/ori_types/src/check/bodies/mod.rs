//! Function, test, impl, and default-impl body checking.
//!
//! Each body runs in a child of the frozen signature environment with parameter
//! bindings and function context installed. Shared finalization exports inferred
//! types, diagnostics, monomorphization requests, and burden metadata.

mod accumulate;
mod contracts;
mod def_impls;
mod functions;
mod impls;
mod method_sig;

pub(super) use def_impls::check_def_impl_bodies;
pub(super) use functions::{check_function_bodies, check_test_bodies};
pub(super) use impls::{check_extension_bodies, check_impl_bodies};
pub(crate) use method_sig::{allocate_rigid_var_map, allocate_rigid_var_map_for_names};

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
#[derive(Debug)]
pub(super) struct BodyOutputs {
    pub expr_types: FxHashMap<ExprIndex, Idx>,
    pub errors: Vec<TypeCheckError>,
    pub warnings: Vec<TypeCheckWarning>,
    pub pat_resolutions: Vec<(PatternKey, PatternResolution)>,
    pub mono_instances: Vec<MonoInstance>,
    /// Pre-dedup `(call_expr_id, MonoInstanceId)` entries from this body's
    /// `InferEngine`. Indices are body-local positions into `mono_instances`;
    /// the finalization spine re-anchors both vectors together.
    pub mono_dispatch_pre_dedup: Vec<(ExprId, MonoInstanceId)>,
    pub deferred_mono_calls: Vec<DeferredMonoCall>,
    /// Composed `UserBurdenSpec` entries produced by this body's
    /// monomorphization sites (one per first-instantiation of a
    /// fully-resolved generic-builtin `Idx`). Registered with the
    /// `TypeRegistry` in [`finalize_body_and_export`]; codegen reads the
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
    /// body. Keys are module-wide AST `ExprId`s, so the checker can extend its
    /// accumulator without re-anchoring. Consumed by `ori_canon` via
    /// [`crate::TypedModule::assign_desugar_map`].
    pub assign_desugars: Vec<(ExprId, crate::AssignDesugar)>,
    /// Module-alias qualified-call rewrite entries resolved in this body. Keys
    /// are module-wide AST `ExprId`s. Consumed by `ori_canon` via
    /// [`crate::TypedModule::module_alias_call_map`].
    pub module_alias_calls: Vec<(ExprId, ori_ir::Name)>,
    /// Iterable->Iterator routed method calls resolved in this body.
    /// Keys are exact source call `ExprId`s; values carry the iterator type and
    /// optional eager-adapter type consumed by `ori_canon`.
    pub iter_route_desugars: Vec<(ExprId, crate::IterMethodRoute)>,
    /// Ordered capability-provider selections for free calls in this body.
    pub capability_calls: Vec<(ExprId, crate::CapabilityCallSite)>,
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
        capability_calls,
    } = outputs;

    // Validate PC-2 contract: no unbound Tag::Var in expr_types or sig positions.
    run_validator(
        checker,
        &expr_types,
        sig,
        sig_span,
        &capability_exempt_var_ids,
    );

    // INVARIANT: E2043 keeps path-sensitive partial releases out of ARC emission.
    run_partial_move_validator(checker, &expr_types, body_root);

    // INVARIANT: E2048 prevents `Drop` implementations from observing moved fields.
    run_drop_partial_move_validator(checker, &expr_types, body_root);

    // Spec: Clause 13.7 requires E2054 after `drop_early` consumes a binding.
    run_consumed_binding_validator(checker, body_root);

    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }

    for error in errors {
        checker.push_error(error);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }

    // Mono instances and their body-local dispatch IDs must be re-anchored
    // together. The remaining outputs already use module-wide coordinates.
    checker.pattern_resolutions.extend(pat_resolutions);
    checker.accumulate_mono_session(mono_instances, mono_dispatch_pre_dedup);
    checker.deferred_mono_calls.extend(deferred_mono_calls);
    checker.assign_desugars.extend(assign_desugars);
    checker.module_alias_calls.extend(module_alias_calls);
    checker.iter_route_desugars.extend(iter_route_desugars);
    checker.capability_calls.extend(capability_calls);

    // INVARIANT: registering burdens here exposes them to codegen and later-body dedup.
    for (idx, spec) in composed_burdens {
        checker.type_registry_mut().register_user_burden(idx, spec);
    }
}

/// Shared PC-2 contract enforcement for every body-checking pass.
///
/// After inference and the end-of-body defaulting pass both complete, walks
/// `expr_types` and `sig` (`param_types` + `return_type`) for any surviving
/// unbound [`Tag::Var`] and emits `E2005` per position. Errors are
/// accumulated via `checker.push_error` alongside normal inference errors.
///
/// `check_function`, `check_test`, `check_impl_method`, and
/// `check_def_impl_method` call this function so all four body passes share
/// the identical validation skeleton.
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
        // Why: `ValidatorContext` shares one arena borrow across attribution callbacks.
        let span_of = |expr_index| arena.get_expr(validator_expr_id(expr_index)).span;
        let expr_kind_of = |expr_index| Some(arena.get_expr(validator_expr_id(expr_index)).kind);
        // INVARIANT: `None` selects the validator's lambda-wide span fallback.
        let param_span_of = |expr_index, param_index: usize| {
            let expr = arena.get_expr(validator_expr_id(expr_index));
            if let ExprKind::Lambda { params, .. } = expr.kind {
                arena.get_params(params).get(param_index).map(|p| p.span)
            } else {
                None
            }
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

fn validator_expr_id(expr_index: ExprIndex) -> ExprId {
    let Ok(raw) = u32::try_from(expr_index) else {
        panic!(
            "type inference stored expression index {expr_index} outside ExprId range; keep \
             inference expression keys sourced from ExprId"
        );
    };
    ExprId::new(raw)
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
/// Resolves the `Drop` trait by interning its source name. If the registry has
/// no such trait, no type can satisfy the predicate and the walk emits no error.
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
