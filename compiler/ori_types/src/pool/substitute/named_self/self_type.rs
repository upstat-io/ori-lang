//! `Tag::SelfType` substitution for bound-chain method dispatch.

use crate::{Idx, Pool, Tag};

/// Substitute every `Tag::SelfType` occurrence reachable from `ty` with
/// `target`. Bound-chain method dispatch uses this to bind a trait method's
/// `Self` placeholders to the receiver's concrete type.
pub fn substitute_self_in_pool(pool: &mut Pool, ty: Idx, target: Idx) -> Idx {
    substitute_self_inner(pool, ty, target)
}

fn substitute_self_inner(pool: &mut Pool, ty: Idx, target: Idx) -> Idx {
    match pool.tag(ty) {
        Tag::SelfType => target,

        Tag::List => {
            let elem = pool.list_elem(ty);
            substitute_single(pool, ty, elem, target, Pool::list)
        }
        Tag::Option => {
            let inner = pool.option_inner(ty);
            substitute_single(pool, ty, inner, target, Pool::option)
        }
        Tag::Set => {
            let elem = pool.set_elem(ty);
            substitute_single(pool, ty, elem, target, Pool::set)
        }
        Tag::Range => {
            let elem = pool.range_elem(ty);
            substitute_single(pool, ty, elem, target, Pool::range)
        }
        Tag::Iterator => {
            let elem = pool.iterator_elem(ty);
            substitute_single(pool, ty, elem, target, Pool::iterator)
        }
        Tag::DoubleEndedIterator => {
            let elem = pool.iterator_elem(ty);
            substitute_single(pool, ty, elem, target, Pool::double_ended_iterator)
        }
        Tag::Map => substitute_self_map(pool, ty, target),
        Tag::Result => substitute_self_result(pool, ty, target),

        Tag::Function => substitute_self_function(pool, ty, target),
        Tag::Tuple => substitute_self_tuple(pool, ty, target),
        Tag::Applied => substitute_self_applied(pool, ty, target),

        _ => ty,
    }
}

/// Adapter matching the shared `substitute_pair` recurse shape
/// (`fn(&mut Pool, Idx, &C) -> Idx` with `C = Idx`).
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "the shared substitute_pair recurse fn-pointer contract passes context as &C"
)]
fn recurse_self(pool: &mut Pool, ty: Idx, target: &Idx) -> Idx {
    substitute_self_inner(pool, ty, *target)
}

fn substitute_self_map(pool: &mut Pool, ty: Idx, target: Idx) -> Idx {
    let key = pool.map_key(ty);
    let value = pool.map_value(ty);
    super::super::substitute_pair(pool, ty, key, value, &target, recurse_self, Pool::map)
}

fn substitute_self_result(pool: &mut Pool, ty: Idx, target: Idx) -> Idx {
    let ok = pool.result_ok(ty);
    let err = pool.result_err(ty);
    super::super::substitute_pair(pool, ty, ok, err, &target, recurse_self, Pool::result)
}

fn substitute_self_function(pool: &mut Pool, ty: Idx, target: Idx) -> Idx {
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

fn substitute_self_tuple(pool: &mut Pool, ty: Idx, target: Idx) -> Idx {
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

fn substitute_self_applied(pool: &mut Pool, ty: Idx, target: Idx) -> Idx {
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

fn substitute_single(
    pool: &mut Pool,
    ty: Idx,
    child: Idx,
    target: Idx,
    ctor: fn(&mut Pool, Idx) -> Idx,
) -> Idx {
    let new_child = substitute_self_inner(pool, child, target);
    if new_child == child {
        ty
    } else {
        ctor(pool, new_child)
    }
}
