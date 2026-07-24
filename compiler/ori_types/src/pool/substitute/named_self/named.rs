//! `Tag::Named` substitution for impl-level binders.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::{Idx, Pool, Tag};

/// Recursively substitute `Tag::Named(name)` leaves in `ty` with the entries
/// in `name_subst`.
///
/// Used for impl-level binder substitution. A registered method signature on
/// `impl<U> Box<U> { @m<T> ... }` carries `Tag::Named(U)` references for the
/// impl-level binder `U`. After the inference engine structurally matches a
/// concrete receiver `Box<int>` against `entry.self_type = Applied(Box,
/// [Named(U)])`, this walker rewrites the registered signature so the
/// method-level `Tag::Scheme` instantiation sees a fully impl-substituted body.
///
/// Distinct from `substitute_in_pool`, which substitutes `Tag::Var` and
/// `Tag::BoundVar` by `var_id` for method-level and monomorphization paths.
/// Both walkers share the same compound-type recursion shape.
///
/// `Tag::Scheme` bodies are walked because impl-level Named references can live
/// inside a method-level scheme body. The scheme's own `var_ids` are preserved.
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

fn substitute_named_inner(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    match pool.tag(ty) {
        Tag::Named => {
            let name = pool.named_name(ty);
            *subst.get(&name).unwrap_or(&ty)
        }

        Tag::List => substitute_named_single(pool, ty, subst, Pool::list),
        Tag::Option => substitute_named_single(pool, ty, subst, Pool::option),
        Tag::Set => substitute_named_single(pool, ty, subst, Pool::set),
        Tag::Channel => substitute_named_single(pool, ty, subst, Pool::channel),
        Tag::Range => substitute_named_single(pool, ty, subst, Pool::range),
        Tag::Iterator => substitute_named_single(pool, ty, subst, Pool::iterator),
        Tag::DoubleEndedIterator => {
            substitute_named_single(pool, ty, subst, Pool::double_ended_iterator)
        }

        Tag::Map => substitute_named_map(pool, ty, subst),
        Tag::Result => substitute_named_result(pool, ty, subst),
        Tag::Borrowed => substitute_named_borrowed(pool, ty, subst),

        Tag::Function => substitute_named_function(pool, ty, subst),
        Tag::Tuple => substitute_named_tuple(pool, ty, subst),
        Tag::Applied => substitute_named_applied(pool, ty, subst),
        Tag::Struct => substitute_named_struct(pool, ty, subst),
        Tag::Enum => substitute_named_enum(pool, ty, subst),

        Tag::Scheme => substitute_named_scheme(pool, ty, subst),

        _ => ty,
    }
}

fn substitute_named_map(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let key = pool.map_key(ty);
    let value = pool.map_value(ty);
    super::super::substitute_pair(
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
    super::super::substitute_pair(
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
            changed |= new_p != p;
            new_p
        })
        .collect();
    let new_ret = substitute_named_inner(pool, ret, subst);
    changed |= new_ret != ret;
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
            changed |= new_e != e;
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
            changed |= new_a != a;
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
    super::super::substitute_child(pool, ty, child, subst, substitute_named_inner, ctor)
}
