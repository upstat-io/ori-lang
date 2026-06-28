//! Concrete-body materialization for generic-composite `Applied` types.
//!
//! Substitutes a fully-concrete `Applied(Generic, [concrete])`'s field/payload
//! `Tag::Named(param)` leaves with the concrete args, interns the concrete
//! `Struct`/`Enum` body, and records `set_resolution(applied → concrete)` so
//! downstream readers (codegen-direct derived-method paths, burden composition)
//! see concrete field/payload types. Drives the monomorphic-annotation path
//! (`@f (p: P3Pair<int,str>)`) that never flows through generic-call mono
//! recording.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::{Idx, Pool, Tag};

use super::substitute_named_in_pool;

/// Materialize the concrete `Struct`/`Enum` body for a fully-concrete
/// `Applied(Generic, [concrete])`: substitute each field/payload
/// `Tag::Named(param)` with the concrete arg, intern the concrete body, and
/// record `set_resolution(applied → concrete)` so downstream readers see
/// concrete field/payload types. `type_params` maps each generic
/// name to its declared param names; `in_progress` guards self-referential
/// composites (`Cons(T, List<T>)`) from infinite expansion. The materialized body
/// lives in `Pool.resolutions`; the nominal `applied` Idx is unchanged.
pub(crate) fn materialize_applied_body(
    pool: &mut Pool,
    applied: Idx,
    type_params: &FxHashMap<Name, Vec<Name>>,
    in_progress: &mut FxHashSet<Idx>,
) {
    if pool.tag(applied) != Tag::Applied {
        return;
    }
    // Already materialized (concrete resolution recorded) — done.
    if pool.resolve(applied).is_some() {
        return;
    }
    // Resolve each arg through its var-links: an inferred construct's type
    // (`GenPair{a:1,b:"s"}`) interns the `Applied` with a Var arg linked to the
    // concrete element (`B` → str-linked Var), so the raw `Applied` carries
    // `HAS_VAR` while it is genuinely a concrete instantiation.
    // Only a still-generic arg AFTER resolution (a `BoundVar`/`RigidVar` param)
    // means this is not a concrete instantiation — leave it for instantiation.
    let raw_args = pool.applied_args(applied);
    // Materialize any nested-`Applied` arg FIRST (bottom-up) so its concrete body
    // is recorded before the concreteness guard below resolves it. The pool-wide
    // sweep visits indices in interning order; without this, visiting the OUTER
    // `Applied(Wrap, [Wrap<int>])` before the inner `Wrap<int>` resolves the
    // inner arg to its still-`HAS_VAR`-stale `Applied` node, trips the
    // `has_any_var_or_infer` guard, and PERMANENTLY skips the outer (the sweep
    // never retries) — leaving the outer to resolve to the GENERIC body whose
    // field is the bare param. Pre-materializing the arg makes the walk
    // order-independent; the `in_progress` + `resolve(applied).is_some()` guards
    // keep it terminating + idempotent.
    for &arg in &raw_args {
        let resolved_arg = pool.resolve_fully(arg);
        if pool.tag(resolved_arg) == Tag::Applied {
            materialize_applied_body(pool, resolved_arg, type_params, in_progress);
        }
    }
    let resolved_args: Vec<Idx> = raw_args
        .iter()
        .map(|&a| {
            let r = pool.resolve_fully(a);
            // An unconstrained type param resolves to an unbound unification
            // `Var` (`Either<str, $t6>` when the second variant is never
            // constructed). Default it to the bottom type `Never` (the same
            // empty-collection defaulting used for unconstrained inferred elems): the
            // phantom variant payload is unconstructable, so `Never` is sound
            // and lets the instantiation materialize a concrete body.
            if pool.tag(r) == Tag::Var {
                Idx::NEVER
            } else {
                r
            }
        })
        .collect();
    // A genuinely-generic param (BoundVar/RigidVar) or a nested unresolved var
    // means this is not a concrete instantiation — leave it for instantiation.
    if resolved_args
        .iter()
        .any(|&a| pool.flags(a).has_any_var_or_infer())
    {
        return;
    }
    // Recursion guard: a mutually-recursive composite re-reaching `applied`
    // before its resolution is recorded returns the in-progress handle.
    if !in_progress.insert(applied) {
        return;
    }

    let name = pool.applied_name(applied);
    let Some(params) = type_params.get(&name) else {
        in_progress.remove(&applied);
        return;
    };
    if params.len() != resolved_args.len() {
        in_progress.remove(&applied);
        return;
    }
    let name_subst: FxHashMap<Name, Idx> = params
        .iter()
        .copied()
        .zip(resolved_args.iter().copied())
        .collect();

    // Resolve the GENERIC body via the Applied→Named matching-args fallback.
    let generic = pool.resolve_fully(applied);
    match pool.tag(generic) {
        Tag::Struct => {
            materialize_struct_resolution(
                pool,
                applied,
                generic,
                &name_subst,
                type_params,
                in_progress,
            );
        }
        Tag::Enum => {
            materialize_enum_resolution(
                pool,
                applied,
                generic,
                &name_subst,
                type_params,
                in_progress,
            );
        }
        // An `Applied` past the type_params + arity gates is expected to resolve
        // to a `Struct`/`Enum`; any other tag (generic newtype/alias) materializes
        // no concrete body — the `tracing::debug!` below records it, not silent.
        other => tracing::debug!(
            ?applied,
            ?other,
            "materialize_applied_body: non-composite body"
        ),
    }

    in_progress.remove(&applied);
}

/// Intern the concrete `Struct` body for `applied` (each field's
/// `Tag::Named(param)` substituted via `name_subst`), record the resolution, and
/// recurse into nested `Applied` field types. The `Tag::Struct` twin of
/// [`materialize_enum_resolution`].
fn materialize_struct_resolution(
    pool: &mut Pool,
    applied: Idx,
    generic: Idx,
    name_subst: &FxHashMap<Name, Idx>,
    type_params: &FxHashMap<Name, Vec<Name>>,
    in_progress: &mut FxHashSet<Idx>,
) {
    let struct_name = pool.struct_name(generic);
    let fields = pool.struct_fields(generic);
    let concrete_fields: Vec<(Name, Idx)> = fields
        .iter()
        .map(|&(field_name, field_ty)| {
            (
                field_name,
                substitute_named_in_pool(pool, field_ty, name_subst),
            )
        })
        .collect();
    let concrete = pool.struct_type(struct_name, &concrete_fields);
    // Record BEFORE recursing so a direct self-reference short-circuits via the
    // `pool.resolve(applied).is_some()` gate in `materialize_applied_body`.
    pool.set_resolution(applied, concrete);
    for &(_, field_ty) in &concrete_fields {
        if pool.tag(field_ty) == Tag::Applied {
            materialize_applied_body(pool, field_ty, type_params, in_progress);
        }
    }
}

/// Intern the concrete `Enum` body for `applied` (each variant payload's
/// `Tag::Named(param)` substituted via `name_subst`), record the resolution, and
/// recurse into nested `Applied` payload types. The `Tag::Enum` twin of
/// [`materialize_struct_resolution`].
fn materialize_enum_resolution(
    pool: &mut Pool,
    applied: Idx,
    generic: Idx,
    name_subst: &FxHashMap<Name, Idx>,
    type_params: &FxHashMap<Name, Vec<Name>>,
    in_progress: &mut FxHashSet<Idx>,
) {
    let enum_name = pool.enum_name(generic);
    let variants = pool.enum_variants(generic);
    let concrete_variants: Vec<crate::pool::EnumVariant> = variants
        .iter()
        .map(|(variant_name, payloads)| {
            let new_payloads: Vec<Idx> = payloads
                .iter()
                .map(|&payload_ty| substitute_named_in_pool(pool, payload_ty, name_subst))
                .collect();
            crate::pool::EnumVariant {
                name: *variant_name,
                field_types: new_payloads,
            }
        })
        .collect();
    let concrete = pool.enum_type(enum_name, &concrete_variants);
    pool.set_resolution(applied, concrete);
    for variant in &concrete_variants {
        for &payload_ty in &variant.field_types {
            if pool.tag(payload_ty) == Tag::Applied {
                materialize_applied_body(pool, payload_ty, type_params, in_progress);
            }
        }
    }
}
