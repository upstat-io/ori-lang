//! Type substitution for monomorphization.
//!
//! [`substitute_in_pool`] materializes concrete monomorphization body maps
//! directly in a `Pool`, preserving the type shapes ARC lowering consumes.

mod body_type_map;
mod extract;
mod materialize;
mod named_self;

pub use body_type_map::{
    build_finalized_body_type_map, build_mono_body_type_map, extend_var_subst_with_roots,
    BodyTypeMapSink,
};
pub use extract::extract_var_from_types;
pub use materialize::register_concrete_applied_resolutions;
pub(crate) use materialize::{has_unproven_named_leaf, materialize_applied_body};
pub use named_self::{substitute_named_in_pool, substitute_self_in_pool};

use rustc_hash::FxHashMap;

use crate::{Idx, Pool, Tag, TypeFlags, VarState};

/// Recursively substitute type variables in `ty` using `var_subst`.
///
/// The substitution map keys are `var_ids` (matching [`FunctionSig::scheme_var_ids`]).
/// Each mapped value is a concrete `Idx` (e.g., `Idx::INT` for `int`).
///
/// Returns the substituted type. If no variables in `ty` match the map,
/// returns `ty` unchanged (O(1) via the `HAS_VAR` flag fast path).
/// New composite types are interned in `pool` (deduplication is automatic).
#[expect(
    clippy::implicit_hasher,
    reason = "always called with FxHashMap internally"
)]
pub fn substitute_in_pool(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    // INVARIANT: every substitutable variable kind participates in the fast-path gate.
    if !pool
        .flags(ty)
        .intersects(TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR | TypeFlags::HAS_RIGID_VAR)
    {
        return ty;
    }

    match pool.tag(ty) {
        Tag::Var => substitute_var(pool, ty, var_subst),
        Tag::BoundVar => substitute_bound_var(pool, ty, var_subst),
        Tag::RigidVar => substitute_rigid_var(pool, ty, var_subst),

        // Single-child containers
        Tag::List => substitute_single_child(pool, ty, var_subst, Pool::list),
        Tag::Option => substitute_single_child(pool, ty, var_subst, Pool::option),
        Tag::Set => substitute_single_child(pool, ty, var_subst, Pool::set),
        Tag::Channel => substitute_single_child(pool, ty, var_subst, Pool::channel),
        Tag::Range => substitute_single_child(pool, ty, var_subst, Pool::range),
        Tag::Iterator => substitute_single_child(pool, ty, var_subst, Pool::iterator),
        Tag::DoubleEndedIterator => {
            substitute_single_child(pool, ty, var_subst, Pool::double_ended_iterator)
        }

        // Two-child containers
        Tag::Map => substitute_map(pool, ty, var_subst),
        Tag::Result => substitute_result(pool, ty, var_subst),

        // Borrowed reference
        Tag::Borrowed => substitute_borrowed(pool, ty, var_subst),

        // Variable-length types
        Tag::Function => substitute_function(pool, ty, var_subst),
        Tag::Tuple => substitute_tuple(pool, ty, var_subst),
        Tag::Applied => substitute_applied(pool, ty, var_subst),
        Tag::Struct => substitute_struct(pool, ty, var_subst),
        Tag::Enum => substitute_enum(pool, ty, var_subst),

        // Schemes have their own bound variables; primitives and other tags
        // don't contain variables.
        _ => ty,
    }
}

/// Substitute a type variable: check `var_id`, then follow links.
///
/// `Tag::Var` leaves whose `var_state` is `Generalized` or `Rigid` fall
/// through to the bottom and return `ty` unchanged — they are orphan
/// references to scheme-bound vars that the substitution map (keyed by
/// the callee's `var_id`s) does not target. The whole-pool walk in
/// `infer::expr::calls::monomorphization::maybe_record_mono_instance`
/// routinely hits such orphans and relies on this no-op fall-through.
///
/// Post-migration, scheme bodies themselves carry `Tag::BoundVar` leaves
/// (see `substitute_bound_var`); the only legitimate `Tag::Var(Generalized)`
/// pool entries are the orphan inference residues just described.
fn substitute_var(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let var_id = pool.data(ty);

    // Direct var_id match (scheme variable)
    if let Some(&replacement) = var_subst.get(&var_id) {
        return replacement;
    }

    // Follow link if present
    if let VarState::Link { target } = pool.var_state(var_id) {
        let target = *target;
        return substitute_in_pool(pool, target, var_subst);
    }

    ty
}

/// Substitute a scheme-bound type variable.
///
/// `Tag::BoundVar.data` is the `var_id` declared by the enclosing scheme.
/// Substitution looks it up directly in `var_subst`;
/// missing entries leave the leaf unchanged — non-substituted bound vars
/// are legitimate (e.g., when `default_unbound_vars_in_scope` walks an
/// expression tree carrying a fresh-instantiation handle whose underlying
/// scheme is unrelated to the substitution map).
fn substitute_bound_var(pool: &Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let var_id = pool.data(ty);
    if let Some(&replacement) = var_subst.get(&var_id) {
        return replacement;
    }
    ty
}

/// Substitute an impl-level rigid type variable (`@m (self) -> T` where `T` is
/// an `impl<T> Box<T>` binder). `Tag::RigidVar.data` is the `var_id` allocated
/// by `Pool::rigid_var`. Rigids carry no unification links, so a missing entry
/// leaves the leaf unchanged. `Var_ids` are globally unique across
/// `Tag::Var`/`Tag::RigidVar`, so a substitution map built for `Tag::Var`s never
/// targets a rigid leaf.
fn substitute_rigid_var(pool: &Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let var_id = pool.data(ty);
    if let Some(&replacement) = var_subst.get(&var_id) {
        return replacement;
    }
    ty
}

/// Build a `var_id -> concrete` substitution for impl-level rigid generics.
/// Scans every `VarState::Rigid { name }` in the pool; when `name` matches an
/// impl binder in `name_to_concrete`, maps that rigid's `var_id` to the concrete
/// type. SSOT for the impl-rigid scan consumed by `resolve_impl_signature`
/// (signature substitution feeding mono recording) and the mono body type map.
pub fn build_impl_rigid_var_subst(
    pool: &Pool,
    name_to_concrete: &FxHashMap<ori_ir::Name, Idx>,
) -> FxHashMap<u32, Idx> {
    let mut out: FxHashMap<u32, Idx> = FxHashMap::default();
    if name_to_concrete.is_empty() {
        return out;
    }
    for var_id in 0..pool.next_var_id() {
        if let Some(VarState::Rigid { name }) = pool.var_state_checked(var_id) {
            if let Some(&concrete) = name_to_concrete.get(name) {
                out.insert(var_id, concrete);
            }
        }
    }
    out
}

/// Build the concrete body-type map for a method owned by a generic impl.
///
/// Named bindings cover declaration-level type parameters while the derived
/// rigid substitution covers canonical body types. When the caller supplies a
/// generic receiver body, the helper materializes and registers its concrete
/// layout before ARC lowering resolves field projections.
pub fn build_impl_mono_body_type_map(
    pool: &mut Pool,
    named_bindings: &[(ori_ir::Name, Idx)],
    receiver: Idx,
    receiver_body: Option<Idx>,
    concrete_receiver: Option<Idx>,
) -> FxHashMap<Idx, Idx> {
    let named: FxHashMap<_, _> = named_bindings.iter().copied().collect();
    let rigid_subst = build_impl_rigid_var_subst(pool, &named);
    let generic_body = receiver_body.or_else(|| pool.resolve(receiver));
    let concrete_receiver_body =
        if let (Some(concrete_receiver), Some(generic_body)) = (concrete_receiver, generic_body) {
            let named_body = substitute_named_in_pool(pool, generic_body, &named);
            let concrete_body = substitute_in_pool(pool, named_body, &rigid_subst);
            pool.set_resolution(concrete_receiver, concrete_body);
            Some((generic_body, concrete_body))
        } else {
            None
        };
    let named_entries: Vec<_> = named
        .iter()
        .map(|(&name, &concrete)| (pool.named(name), concrete))
        .collect();
    let mut body_type_map: FxHashMap<_, _> =
        build_finalized_body_type_map(pool, &rigid_subst, &named_entries)
            .into_iter()
            .collect();
    if let Some(concrete_receiver) = concrete_receiver {
        body_type_map.insert(receiver, concrete_receiver);
    }
    if let Some((generic_body, concrete_body)) = concrete_receiver_body {
        body_type_map.insert(generic_body, concrete_body);
    }
    body_type_map
}

/// Substitute in a single-child container (List, Option, Set, etc.).
fn substitute_single_child(
    pool: &mut Pool,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
    ctor: fn(&mut Pool, Idx) -> Idx,
) -> Idx {
    let child = Idx::from_raw(pool.data(ty));
    substitute_child(pool, ty, child, var_subst, substitute_in_pool, ctor)
}

/// Substitute in a Map type (key + value).
fn substitute_map(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let key = pool.map_key(ty);
    let value = pool.map_value(ty);
    substitute_pair(
        pool,
        ty,
        key,
        value,
        var_subst,
        substitute_in_pool,
        Pool::map,
    )
}

/// Substitute in a Result type (ok + err).
fn substitute_result(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let ok = pool.result_ok(ty);
    let err = pool.result_err(ty);
    substitute_pair(
        pool,
        ty,
        ok,
        err,
        var_subst,
        substitute_in_pool,
        Pool::result,
    )
}

fn substitute_child<C>(
    pool: &mut Pool,
    ty: Idx,
    child: Idx,
    context: &C,
    recurse: fn(&mut Pool, Idx, &C) -> Idx,
    ctor: fn(&mut Pool, Idx) -> Idx,
) -> Idx {
    let new_child = recurse(pool, child, context);
    if new_child == child {
        ty
    } else {
        ctor(pool, new_child)
    }
}

fn substitute_pair<C>(
    pool: &mut Pool,
    ty: Idx,
    first: Idx,
    second: Idx,
    context: &C,
    recurse: fn(&mut Pool, Idx, &C) -> Idx,
    ctor: fn(&mut Pool, Idx, Idx) -> Idx,
) -> Idx {
    let new_first = recurse(pool, first, context);
    let new_second = recurse(pool, second, context);
    if new_first == first && new_second == second {
        ty
    } else {
        ctor(pool, new_first, new_second)
    }
}

/// Substitute in a Borrowed reference (inner + lifetime preserved).
fn substitute_borrowed(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let inner = pool.borrowed_inner(ty);
    let lt = pool.borrowed_lifetime(ty);
    let new_inner = substitute_in_pool(pool, inner, var_subst);
    if new_inner == inner {
        ty
    } else {
        pool.borrowed(new_inner, lt)
    }
}

/// Substitute in a Function type (params + return).
fn substitute_function(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let params = pool.function_params(ty);
    let ret = pool.function_return(ty);

    let mut changed = false;
    let new_params: Vec<Idx> = params
        .iter()
        .map(|&p| {
            let new_p = substitute_in_pool(pool, p, var_subst);
            if new_p != p {
                changed = true;
            }
            new_p
        })
        .collect();

    let new_ret = substitute_in_pool(pool, ret, var_subst);
    if new_ret != ret {
        changed = true;
    }

    if changed {
        pool.function(&new_params, new_ret)
    } else {
        ty
    }
}

/// Substitute in a Tuple type (element list).
fn substitute_tuple(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let elems = pool.tuple_elems(ty);

    let mut changed = false;
    let new_elems: Vec<Idx> = elems
        .iter()
        .map(|&e| {
            let new_e = substitute_in_pool(pool, e, var_subst);
            if new_e != e {
                changed = true;
            }
            new_e
        })
        .collect();

    if changed {
        pool.tuple(&new_elems)
    } else {
        ty
    }
}

/// Substitute in an Applied type (name + type args).
fn substitute_applied(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let name = pool.applied_name(ty);
    let args = pool.applied_args(ty);

    let mut changed = false;
    let new_args: Vec<Idx> = args
        .iter()
        .map(|&a| {
            let new_a = substitute_in_pool(pool, a, var_subst);
            if new_a != a {
                changed = true;
            }
            new_a
        })
        .collect();

    if changed {
        pool.applied(name, &new_args)
    } else {
        ty
    }
}

/// Substitute in a Struct type (field types, preserving field names).
fn substitute_struct(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let name = pool.struct_name(ty);
    let fields = pool.struct_fields(ty);

    let mut changed = false;
    let new_fields: Vec<(ori_ir::Name, Idx)> = fields
        .iter()
        .map(|&(field_name, field_ty)| {
            let new_ty = substitute_in_pool(pool, field_ty, var_subst);
            if new_ty != field_ty {
                changed = true;
            }
            (field_name, new_ty)
        })
        .collect();

    if changed {
        pool.struct_type(name, &new_fields)
    } else {
        ty
    }
}

/// Substitute in an Enum type (variant payload types, preserving variant names).
///
/// The `Tag::Enum` twin of [`substitute_struct`]: substitutes each variant
/// payload `Idx` via `var_subst`, payload-name-agnostic (the Pool enum
/// representation carries no field names).
fn substitute_enum(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let name = pool.enum_name(ty);
    let variants = pool.enum_variants(ty);

    let mut changed = false;
    let new_variants: Vec<crate::pool::EnumVariant> = variants
        .iter()
        .map(|(variant_name, payloads)| {
            let new_payloads: Vec<Idx> = payloads
                .iter()
                .map(|&payload_ty| {
                    let new_ty = substitute_in_pool(pool, payload_ty, var_subst);
                    if new_ty != payload_ty {
                        changed = true;
                    }
                    new_ty
                })
                .collect();
            crate::pool::EnumVariant {
                name: *variant_name,
                field_types: new_payloads,
            }
        })
        .collect();

    if changed {
        pool.enum_type(name, &new_variants)
    } else {
        ty
    }
}

#[cfg(test)]
mod tests;
