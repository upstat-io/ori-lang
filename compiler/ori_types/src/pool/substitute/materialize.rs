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

/// Register concrete composite bodies referenced by a finalized mono body map.
///
/// A body map records both the generic pool handle and its concrete
/// substitution. Applied substitutions also need a concrete `Struct` or
/// `Enum` resolution before layout and ownership analysis can consume them.
/// This is the shared registration tail for eager, deferred, and realization-
/// discovered monomorphization instances.
#[expect(
    clippy::implicit_hasher,
    reason = "generic_type_params is consistently FxHashMap<Name, Vec<Name>> across the whole \
              ori_types crate; generalizing would force BuildHasher plumbing through every \
              caller for no measurable benefit."
)]
pub fn register_concrete_applied_resolutions(
    pool: &mut Pool,
    body_type_map: &[(Idx, Idx)],
    generic_type_params: &FxHashMap<Name, Vec<Name>>,
) {
    for &(_, concrete) in body_type_map {
        if pool.tag(concrete) == Tag::Applied {
            let mut in_progress = FxHashSet::default();
            materialize_applied_body(pool, concrete, generic_type_params, &mut in_progress);
        }
    }
}

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
    let name = pool.applied_name(applied);
    let Some(params) = type_params.get(&name) else {
        return;
    };
    // Revisit an existing resolution as well. Some registration paths attach
    // a concrete `Applied` handle to its generic composite body before the
    // binder substitutions are known; that resolution is not a materialized
    // specialization. Re-running is idempotent once the body is concrete and
    // lets this routine replace an earlier generic-body resolution.
    // Resolve each arg through its var-links: an inferred construct's type
    // (`GenPair{a:1,b:"s"}`) interns the `Applied` with a Var arg linked to the
    // concrete element (`B` → str-linked Var), so the raw `Applied` carries
    // `HAS_VAR` while it is genuinely a concrete instantiation.
    // Only a still-generic arg AFTER resolution (a `BoundVar`/`RigidVar` param)
    // means this is not a concrete instantiation — leave it for instantiation.
    let raw_args = pool.applied_args(applied);
    // `TypeFlags` cannot distinguish an unresolved `Named(T)` binder from a
    // nominal named type: both carry `IS_NAMED` without a variable bit. Limit
    // the collision check to THIS head's declared binders: a plan-wide union
    // would make an unrelated `Named(T)` nominal non-materializable merely
    // because some other generic declaration also uses the conventional `T`.
    // A resolved `S<Named(A)>` remains ambiguous when `A` is S's own binder:
    // pool interning gives the generic shell and `S<global A>` the same Idx,
    // so attaching a resolution would corrupt the shell. Fail closed there.
    if raw_args
        .iter()
        .any(|&arg| has_unproven_named_leaf(pool, arg, params))
    {
        return;
    }
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
            let chased = pool.chase_var_links(a);
            if pool.tag(chased) == Tag::Applied && !pool.flags(chased).has_any_var_or_infer() {
                return chased;
            }
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

    if params.len() != resolved_args.len() {
        in_progress.remove(&applied);
        return;
    }
    let name_subst: FxHashMap<Name, Idx> = params
        .iter()
        .copied()
        .zip(resolved_args.iter().copied())
        .collect();

    // Always instantiate from the registered generic declaration. An inferred
    // `Applied` carrier can retain an older concrete resolution after its
    // linked arguments become more precise. Treating that stale specialization
    // as the template nests the old payload into the new body and corrupts the
    // carrier's representation. The nominal declaration is the stable source
    // that still contains the named binders in every materialization pass.
    let generic_owner = pool.named(name);
    let generic = pool.resolve_fully(generic_owner);
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

/// Return whether `ty` contains a `Named`/`Alias` leaf that is not proven
/// concrete. An unresolved leaf is a generic binder. A resolved `Named` whose
/// spelling is one of the applied head's declared binders is ambiguous because
/// pool interning erases lexical origin; fail closed rather than attach a
/// concrete body to a generic shell. The walk is cycle-safe for registered
/// recursive types.
pub(crate) fn has_unproven_named_leaf(pool: &Pool, ty: Idx, head_params: &[Name]) -> bool {
    let mut visiting = FxHashSet::default();
    contains_unproven_named_leaf(pool, ty, head_params, &mut visiting)
}

fn contains_unproven_named_leaf(
    pool: &Pool,
    ty: Idx,
    head_params: &[Name],
    visiting: &mut FxHashSet<Idx>,
) -> bool {
    let current = pool.chase_var_links(ty);
    if !pool.is_valid_idx(current) || !visiting.insert(current) {
        return false;
    }

    let result = match pool.tag(current) {
        Tag::Named if head_params.contains(&pool.named_name(current)) => true,
        Tag::Named | Tag::Alias => match pool.resolve(current) {
            Some(resolved) => contains_unproven_named_leaf(pool, resolved, head_params, visiting),
            None => true,
        },
        Tag::Applied => pool
            .applied_args(current)
            .into_iter()
            .any(|arg| contains_unproven_named_leaf(pool, arg, head_params, visiting)),
        Tag::List
        | Tag::Option
        | Tag::Set
        | Tag::Range
        | Tag::Channel
        | Tag::Iterator
        | Tag::DoubleEndedIterator => contains_unproven_named_leaf(
            pool,
            Idx::from_raw(pool.data(current)),
            head_params,
            visiting,
        ),
        Tag::Map => {
            contains_unproven_named_leaf(pool, pool.map_key(current), head_params, visiting)
                || contains_unproven_named_leaf(
                    pool,
                    pool.map_value(current),
                    head_params,
                    visiting,
                )
        }
        Tag::Result => {
            contains_unproven_named_leaf(pool, pool.result_ok(current), head_params, visiting)
                || contains_unproven_named_leaf(
                    pool,
                    pool.result_err(current),
                    head_params,
                    visiting,
                )
        }
        Tag::Borrowed => {
            contains_unproven_named_leaf(pool, pool.borrowed_inner(current), head_params, visiting)
        }
        Tag::Function => {
            pool.function_params(current)
                .into_iter()
                .any(|param| contains_unproven_named_leaf(pool, param, head_params, visiting))
                || contains_unproven_named_leaf(
                    pool,
                    pool.function_return(current),
                    head_params,
                    visiting,
                )
        }
        Tag::Tuple => pool
            .tuple_elems(current)
            .into_iter()
            .any(|element| contains_unproven_named_leaf(pool, element, head_params, visiting)),
        Tag::Struct => pool
            .struct_fields(current)
            .into_iter()
            .any(|(_, field)| contains_unproven_named_leaf(pool, field, head_params, visiting)),
        Tag::Enum => pool.enum_variants(current).into_iter().any(|(_, fields)| {
            fields
                .into_iter()
                .any(|field| contains_unproven_named_leaf(pool, field, head_params, visiting))
        }),
        Tag::Scheme => {
            contains_unproven_named_leaf(pool, pool.scheme_body(current), head_params, visiting)
        }
        _ => false,
    };

    visiting.remove(&current);
    result
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
