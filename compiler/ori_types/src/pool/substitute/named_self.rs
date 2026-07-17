//! `Tag::Named` and `Tag::SelfType` substitution walkers.
//!
//! The compound-type recursion twins of `substitute_in_pool` (which keys on
//! `var_id`): [`substitute_named_in_pool`] rewrites `Tag::Named(name)` leaves
//! by name for impl-level binder substitution; [`substitute_self_in_pool`]
//! rewrites `Tag::SelfType` leaves to a concrete target for bound-chain method
//! dispatch.

use rustc_hash::FxHashMap;

use ori_ir::Name;

use crate::{Idx, Pool, Tag};

/// Recursively substitute `Tag::Named(name)` leaves in `ty` with the entries
/// in `name_subst`.
///
/// Used for impl-level binder substitution.
/// A registered method signature on `impl<U> Box<U> { @m<T> ... }` carries
/// `Tag::Named(U)` references for the impl-level binder `U`. After the
/// inference engine structurally matches a concrete receiver `Box<int>`
/// against `entry.self_type = Applied(Box, [Named(U)])` to produce the
/// substitution `{U: int}`, this walker rewrites the registered signature
/// so the method-level `Tag::Scheme` instantiation that follows sees a
/// fully impl-substituted body.
///
/// Distinct from `substitute_in_pool` which substitutes `Tag::Var` /
/// `Tag::BoundVar` by `var_id` for method-level / monomorphization paths.
/// Both walkers share the same compound-type recursion shape.
///
/// `Tag::Scheme` bodies ARE walked (impl-level Named references can live
/// inside a method-level scheme body when the impl AND the method both
/// declare type generics). The scheme's own `var_ids` are preserved unchanged.
pub fn substitute_named_in_pool(
    pool: &mut Pool,
    ty: Idx,
    name_subst: &FxHashMap<Name, Idx>,
) -> Idx {
    if name_subst.is_empty() {
        return ty;
    }
    substitute_named_inner(pool, ty, name_subst)
}

/// Substitute every `Tag::SelfType` occurrence reachable from `ty` with
/// `target`. Used by bound-chain method dispatch to bind the trait
/// method's `Self` placeholders to the receiver's concrete `Tag::RigidVar`
/// (or other concrete type) so chained calls like `val.clone().to_str()`
/// see the receiver's type for the second dispatch instead of falling
/// back to a fresh unification var.
pub fn substitute_self_in_pool(pool: &mut Pool, ty: Idx, target: Idx) -> Idx {
    substitute_self_inner(pool, ty, target)
}

#[expect(
    clippy::too_many_lines,
    reason = "Tag-dispatch table — line count tracks variant count (16 tag arms), not algorithmic complexity. Splitting into per-tag helpers would be cosmetic since each arm differs only in accessor/constructor method names."
)]
fn substitute_self_inner(pool: &mut Pool, ty: Idx, target: Idx) -> Idx {
    match pool.tag(ty) {
        Tag::SelfType => target,

        Tag::List => {
            let elem = pool.list_elem(ty);
            let new_elem = substitute_self_inner(pool, elem, target);
            if new_elem == elem {
                ty
            } else {
                pool.list(new_elem)
            }
        }
        Tag::Option => {
            let inner = pool.option_inner(ty);
            let new_inner = substitute_self_inner(pool, inner, target);
            if new_inner == inner {
                ty
            } else {
                pool.option(new_inner)
            }
        }
        Tag::Set => {
            let elem = pool.set_elem(ty);
            let new_elem = substitute_self_inner(pool, elem, target);
            if new_elem == elem {
                ty
            } else {
                pool.set(new_elem)
            }
        }
        Tag::Range => {
            let elem = pool.range_elem(ty);
            let new_elem = substitute_self_inner(pool, elem, target);
            if new_elem == elem {
                ty
            } else {
                pool.range(new_elem)
            }
        }
        Tag::Iterator => {
            let elem = pool.iterator_elem(ty);
            let new_elem = substitute_self_inner(pool, elem, target);
            if new_elem == elem {
                ty
            } else {
                pool.iterator(new_elem)
            }
        }
        Tag::DoubleEndedIterator => {
            let elem = pool.iterator_elem(ty);
            let new_elem = substitute_self_inner(pool, elem, target);
            if new_elem == elem {
                ty
            } else {
                pool.double_ended_iterator(new_elem)
            }
        }
        Tag::Map => {
            let key = pool.map_key(ty);
            let value = pool.map_value(ty);
            let new_key = substitute_self_inner(pool, key, target);
            let new_value = substitute_self_inner(pool, value, target);
            if new_key == key && new_value == value {
                ty
            } else {
                pool.map(new_key, new_value)
            }
        }
        Tag::Result => {
            let ok = pool.result_ok(ty);
            let err = pool.result_err(ty);
            let new_ok = substitute_self_inner(pool, ok, target);
            let new_err = substitute_self_inner(pool, err, target);
            if new_ok == ok && new_err == err {
                ty
            } else {
                pool.result(new_ok, new_err)
            }
        }
        Tag::Function => {
            let params: Vec<Idx> = pool.function_params(ty);
            let ret = pool.function_return(ty);
            let new_params: Vec<Idx> = params
                .iter()
                .map(|&p| substitute_self_inner(pool, p, target))
                .collect();
            let new_ret = substitute_self_inner(pool, ret, target);
            if new_params == params && new_ret == ret {
                ty
            } else {
                pool.function(&new_params, new_ret)
            }
        }
        Tag::Tuple => {
            let elems: Vec<Idx> = pool.tuple_elems(ty);
            let new_elems: Vec<Idx> = elems
                .iter()
                .map(|&e| substitute_self_inner(pool, e, target))
                .collect();
            if new_elems == elems {
                ty
            } else {
                pool.tuple(&new_elems)
            }
        }
        Tag::Applied => {
            let name = pool.applied_name(ty);
            let args: Vec<Idx> = pool.applied_args(ty);
            let new_args: Vec<Idx> = args
                .iter()
                .map(|&a| substitute_self_inner(pool, a, target))
                .collect();
            if new_args == args {
                ty
            } else {
                pool.applied(name, &new_args)
            }
        }
        // Primitives, vars (Var/BoundVar/RigidVar), Struct/Enum literals,
        // Projection, Infer, Error, Named, Channel, Borrowed, Scheme, Alias,
        // ModuleNs — carry no `Tag::SelfType` children for this dispatch path.
        // Scheme is intentionally skipped: trait method signatures with Self
        // returns wrap as `Tag::Function`, not `Tag::Scheme` (method-level
        // generics are a separate axis from `Self`).
        _ => ty,
    }
}

fn substitute_named_inner(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    match pool.tag(ty) {
        Tag::Named => {
            let name = pool.named_name(ty);
            *subst.get(&name).unwrap_or(&ty)
        }

        // Single-child containers
        Tag::List => substitute_named_single(pool, ty, subst, Pool::list),
        Tag::Option => substitute_named_single(pool, ty, subst, Pool::option),
        Tag::Set => substitute_named_single(pool, ty, subst, Pool::set),
        Tag::Channel => substitute_named_single(pool, ty, subst, Pool::channel),
        Tag::Range => substitute_named_single(pool, ty, subst, Pool::range),
        Tag::Iterator => substitute_named_single(pool, ty, subst, Pool::iterator),
        Tag::DoubleEndedIterator => {
            substitute_named_single(pool, ty, subst, Pool::double_ended_iterator)
        }

        // Two-child containers
        Tag::Map => substitute_named_map(pool, ty, subst),
        Tag::Result => substitute_named_result(pool, ty, subst),
        Tag::Borrowed => substitute_named_borrowed(pool, ty, subst),

        // Variable-length compound types
        Tag::Function => substitute_named_function(pool, ty, subst),
        Tag::Tuple => substitute_named_tuple(pool, ty, subst),
        Tag::Applied => substitute_named_applied(pool, ty, subst),
        Tag::Struct => substitute_named_struct(pool, ty, subst),
        Tag::Enum => substitute_named_enum(pool, ty, subst),

        // Scheme: walk body, preserve scheme structure and var_ids.
        Tag::Scheme => substitute_named_scheme(pool, ty, subst),

        // Other tags (primitives, vars, projections, self-type, infer, error)
        // carry no Tag::Named children to substitute.
        _ => ty,
    }
}

fn substitute_named_map(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let key = pool.map_key(ty);
    let value = pool.map_value(ty);
    super::substitute_pair(
        pool,
        ty,
        key,
        value,
        subst,
        substitute_named_inner,
        Pool::map,
    )
}

fn substitute_named_result(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let ok = pool.result_ok(ty);
    let err = pool.result_err(ty);
    super::substitute_pair(
        pool,
        ty,
        ok,
        err,
        subst,
        substitute_named_inner,
        Pool::result,
    )
}

fn substitute_named_borrowed(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let inner = pool.borrowed_inner(ty);
    let lt = pool.borrowed_lifetime(ty);
    let new_inner = substitute_named_inner(pool, inner, subst);
    if new_inner == inner {
        ty
    } else {
        pool.borrowed(new_inner, lt)
    }
}

fn substitute_named_function(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let params = pool.function_params(ty);
    let ret = pool.function_return(ty);
    let mut changed = false;
    let new_params: Vec<Idx> = params
        .iter()
        .map(|&p| {
            let new_p = substitute_named_inner(pool, p, subst);
            if new_p != p {
                changed = true;
            }
            new_p
        })
        .collect();
    let new_ret = substitute_named_inner(pool, ret, subst);
    if new_ret != ret {
        changed = true;
    }
    if changed {
        pool.function(&new_params, new_ret)
    } else {
        ty
    }
}

fn substitute_named_tuple(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let elems = pool.tuple_elems(ty);
    let mut changed = false;
    let new_elems: Vec<Idx> = elems
        .iter()
        .map(|&e| {
            let new_e = substitute_named_inner(pool, e, subst);
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

fn substitute_named_struct(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let name = pool.struct_name(ty);
    let fields = pool.struct_fields(ty);
    let mut changed = false;
    let new_fields: Vec<(Name, Idx)> = fields
        .iter()
        .map(|&(field_name, field_ty)| {
            let new_ty = substitute_named_inner(pool, field_ty, subst);
            changed |= new_ty != field_ty;
            (field_name, new_ty)
        })
        .collect();
    if changed {
        pool.struct_type(name, &new_fields)
    } else {
        ty
    }
}

fn substitute_named_enum(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let name = pool.enum_name(ty);
    let variants = pool.enum_variants(ty);
    let mut changed = false;
    let new_variants: Vec<crate::pool::EnumVariant> = variants
        .iter()
        .map(|(variant_name, payloads)| {
            let field_types = payloads
                .iter()
                .map(|&payload_ty| {
                    let new_ty = substitute_named_inner(pool, payload_ty, subst);
                    changed |= new_ty != payload_ty;
                    new_ty
                })
                .collect();
            crate::pool::EnumVariant {
                name: *variant_name,
                field_types,
            }
        })
        .collect();
    if changed {
        pool.enum_type(name, &new_variants)
    } else {
        ty
    }
}

fn substitute_named_applied(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let name = pool.applied_name(ty);
    let args = pool.applied_args(ty);
    let mut changed = false;
    let new_args: Vec<Idx> = args
        .iter()
        .map(|&a| {
            let new_a = substitute_named_inner(pool, a, subst);
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

fn substitute_named_scheme(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let vars = pool.scheme_vars(ty).to_vec();
    let body = pool.scheme_body(ty);
    let new_body = substitute_named_inner(pool, body, subst);
    if new_body == body {
        ty
    } else {
        pool.scheme(&vars, new_body)
    }
}

fn substitute_named_single(
    pool: &mut Pool,
    ty: Idx,
    subst: &FxHashMap<Name, Idx>,
    ctor: fn(&mut Pool, Idx) -> Idx,
) -> Idx {
    let child = Idx::from_raw(pool.data(ty));
    super::substitute_child(pool, ty, child, subst, substitute_named_inner, ctor)
}
