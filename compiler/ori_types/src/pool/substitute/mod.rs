//! Type substitution for monomorphization.
//!
//! Provides [`substitute_in_pool`] which recursively replaces type variables
//! with concrete types. Used during monomorphization to build the
//! `body_type_map` (generic `Idx` → concrete `Idx`) that the ARC lowerer
//! uses to emit type-specific retain/release/drop.
//!
//! Follows the same structural recursion pattern as `UnifyEngine::substitute()`
//! but operates as a standalone function on `&mut Pool`, suitable for use
//! during mono instance recording in the type checker.

mod extract;

pub use extract::extract_var_from_types;

use rustc_hash::FxHashMap;

use ori_ir::Name;

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
    // Fast path: no variables OR bound vars to substitute. §08.3b widened the
    // gate to include HAS_BOUND_VAR — scheme bodies post-
    // migration contain `Tag::BoundVar` leaves with HAS_BOUND_VAR=true and
    // HAS_VAR=false; the old `!HAS_VAR` gate would skip them entirely.
    if !pool
        .flags(ty)
        .intersects(TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR)
    {
        return ty;
    }

    match pool.tag(ty) {
        Tag::Var => substitute_var(pool, ty, var_subst),
        Tag::BoundVar => substitute_bound_var(pool, ty, var_subst),

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
/// Post-§08.3b, scheme bodies themselves carry `Tag::BoundVar` leaves
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
/// `Tag::BoundVar.data` is the `var_id` declared by the enclosing scheme
/// . Substitution looks it up directly in `var_subst`;
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

/// Substitute in a single-child container (List, Option, Set, etc.).
fn substitute_single_child(
    pool: &mut Pool,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
    ctor: fn(&mut Pool, Idx) -> Idx,
) -> Idx {
    let child = Idx::from_raw(pool.data(ty));
    let new_child = substitute_in_pool(pool, child, var_subst);
    if new_child == child {
        ty
    } else {
        ctor(pool, new_child)
    }
}

/// Substitute in a Map type (key + value).
fn substitute_map(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let key = pool.map_key(ty);
    let value = pool.map_value(ty);
    let new_key = substitute_in_pool(pool, key, var_subst);
    let new_value = substitute_in_pool(pool, value, var_subst);
    if new_key == key && new_value == value {
        ty
    } else {
        pool.map(new_key, new_value)
    }
}

/// Substitute in a Result type (ok + err).
fn substitute_result(pool: &mut Pool, ty: Idx, var_subst: &FxHashMap<u32, Idx>) -> Idx {
    let ok = pool.result_ok(ty);
    let err = pool.result_err(ty);
    let new_ok = substitute_in_pool(pool, ok, var_subst);
    let new_err = substitute_in_pool(pool, err, var_subst);
    if new_ok == ok && new_err == err {
        ty
    } else {
        pool.result(new_ok, new_err)
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
/// `target`. Used by §10.1 bound-chain method dispatch to bind the trait
/// method's `Self` placeholders to the receiver's concrete `Tag::RigidVar`
/// (or other concrete type) so chained calls like `val.clone().to_str()`
/// see the receiver's type for the second dispatch instead of falling
/// back to a fresh unification var.
pub fn substitute_self_in_pool(pool: &mut Pool, ty: Idx, target: Idx) -> Idx {
    substitute_self_inner(pool, ty, target)
}

#[expect(
    clippy::too_many_lines,
    reason = "Tag-dispatch table — line count tracks variant count (16 tag arms), not algorithmic complexity. Per impl-hygiene.md §Algorithmic DRY, splitting into per-tag helpers would be cosmetic since each arm differs only in accessor/constructor method names."
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
        // ModuleNs — carry no `Tag::SelfType` children for §10.1's purpose.
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

        // Scheme: walk body, preserve scheme structure and var_ids.
        Tag::Scheme => substitute_named_scheme(pool, ty, subst),

        // Other tags (primitives, vars, struct/enum literals, projections,
        // self-type, infer, error) carry no Tag::Named children to substitute.
        _ => ty,
    }
}

fn substitute_named_map(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let key = pool.map_key(ty);
    let value = pool.map_value(ty);
    let new_key = substitute_named_inner(pool, key, subst);
    let new_value = substitute_named_inner(pool, value, subst);
    if new_key == key && new_value == value {
        ty
    } else {
        pool.map(new_key, new_value)
    }
}

fn substitute_named_result(pool: &mut Pool, ty: Idx, subst: &FxHashMap<Name, Idx>) -> Idx {
    let ok = pool.result_ok(ty);
    let err = pool.result_err(ty);
    let new_ok = substitute_named_inner(pool, ok, subst);
    let new_err = substitute_named_inner(pool, err, subst);
    if new_ok == ok && new_err == err {
        ty
    } else {
        pool.result(new_ok, new_err)
    }
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
    let new_child = substitute_named_inner(pool, child, subst);
    if new_child == child {
        ty
    } else {
        ctor(pool, new_child)
    }
}

/// Sink for [`build_mono_body_type_map`] entries — abstracts over the
/// accumulator shape used by each monomorphization call site. `Vec<(Idx, Idx)>`
/// callers (typeck-side: `maybe_record_mono_instance`, `build_mono_instance`)
/// post-process with `sort_by_key` + `dedup_by_key` to land a deterministic
/// Salsa-friendly `MonoInstance.body_type_map`. The `FxHashMap<Idx, Idx>`
/// caller (`oric::test::runner::llvm_backend::run_file_llvm` imported-mono
/// construction) keys insertions directly because it feeds the LLVM-side
/// `MonoFunction.body_type_map` which already requires hash lookup.
pub trait BodyTypeMapSink {
    /// Record a `(source_idx → substituted_idx)` entry.
    fn record(&mut self, key: Idx, value: Idx);
}

impl BodyTypeMapSink for Vec<(Idx, Idx)> {
    fn record(&mut self, key: Idx, value: Idx) {
        self.push((key, value));
    }
}

impl<S: std::hash::BuildHasher> BodyTypeMapSink for std::collections::HashMap<Idx, Idx, S> {
    fn record(&mut self, key: Idx, value: Idx) {
        self.insert(key, value);
    }
}

/// Build a monomorphization body-type map in `sink` for the mono instance
/// identified by `var_subst`.
///
/// Two-pass structure mirrors the three shipped call sites (typeck
/// `maybe_record_mono_instance`, typeck `build_mono_instance`, test-runner
/// imported-mono construction) — extracted as the SSOT per
/// after `/impl-hygiene-review`
/// 2026-04-20 Phase 3 F1 (Critical).
///
/// Pass 1 — existing-pool scan:
///   For every dynamic `Idx` (skip pre-interned primitives) whose
///   `TypeFlags::HAS_VAR | HAS_BOUND_VAR` is set, substitute via `var_subst`
///   and record when the result differs from the source. This covers
///   `sig`/`expr_types` positions carrying generic `Tag::Var` leaves from
///   pre-generalize inference AND post-§08.3b.1 normalization-rewritten
///   `Tag::BoundVar` leaves.
///
/// Pass 2 — scheme-var `BoundVar` pre-intern:
///   For every `(var_id, concrete_idx)` in `var_subst`, pre-intern
///   `Tag::BoundVar(var_id)` and record `(bound_idx → concrete_idx)` in
///   the sink. The end-of-body normalization pass
///   ([`crate::infer::InferEngine::normalize_body_generalized_to_bound_var`])
///   rewrites `Tag::Var(Generalized)` leaves to `Tag::BoundVar` per
///   AFTER mono instances are recorded; without the
///   pre-intern, post-normalization `sig`/`expr_types` referencing
///   `Tag::BoundVar` would not be substituted at ARC lowering, leading to
///   `Tag::BoundVar` reaching codegen's `TypeInfoStore` as a defensive ICE
///   signal per AIMS Invariant 2.
///
/// Callers are responsible for any per-site post-processing (sort+dedup
/// for `Vec` sinks to reach a Salsa-deterministic shape; Applied-type
/// resolution registration; etc.).
#[expect(
    clippy::implicit_hasher,
    reason = "var_subst is consistently FxHashMap<u32, Idx> across the whole ori_types \
              crate (matches `substitute_in_pool`'s signature); generalizing would force \
              BuildHasher plumbing through every caller for no measurable benefit."
)]
pub fn build_mono_body_type_map<Sink: BodyTypeMapSink>(
    pool: &mut Pool,
    var_subst: &FxHashMap<u32, Idx>,
    sink: &mut Sink,
) {
    let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
    let mask = TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR;
    for raw in Idx::FIRST_DYNAMIC..pool_len {
        let idx = Idx::from_raw(raw);
        if pool.flags(idx).intersects(mask) {
            let substituted = substitute_in_pool(pool, idx, var_subst);
            if substituted != idx {
                sink.record(idx, substituted);
            }
        }
    }
    for (&var_id, &concrete_idx) in var_subst {
        let bound_idx = pool.bound_var(var_id);
        if bound_idx != concrete_idx {
            sink.record(bound_idx, concrete_idx);
        }
    }
}

/// Extend `var_subst` with `{union_find_root_var_id → concrete}` entries for
/// every scheme var whose equivalence-class root differs from the scheme
/// var's own `var_id`.
///
/// Rank-weighted union-find can make a fresh instantiation
/// var the root of a scheme var's equivalence class. In that case,
/// `substitute_var` (see lines 90-105) finds the scheme var's `var_id` via
/// direct map lookup, but walking a body type that carries the ROOT's
/// `Tag::Var(root_var_id)` leaf falls through to `VarState::Unbound` (the
/// root has no `Link` to follow) and returns unchanged. Adding the root's
/// `var_id` to the map ensures pool-walk visits of the root `Tag::Var`
/// entry find the concrete type through a direct hit.
///
/// **Semantics: preserve-existing** — mirrors the idempotent-set pattern of
/// `build_exempt_var_ids` at `check/validators/mod.rs:161-173`. Caller-supplied
/// map entries (declared scheme-var → concrete) are authoritative and MUST
/// NOT be overwritten; the helper only ADDS root-var entries that were not
/// already keys.
///
/// **Invoked by every monomorphization path** to maintain the SSOT invariant
/// per and `§SSOT`:
/// - Eager typeck: `infer::expr::calls::monomorphization::maybe_record_mono_instance`
/// - Deferred typeck: `check::exports::resolve_deferred_mono_calls`
/// - JIT imported-mono: `oric::test::runner::imported_mono`
///
/// Idempotent and side-effect-free on `pool` (read-only queries).
#[expect(
    clippy::implicit_hasher,
    reason = "var_subst is consistently FxHashMap<u32, Idx> across the whole ori_types crate \
              (matches substitute_in_pool's signature); generalizing would force BuildHasher \
              plumbing through every caller for no measurable benefit."
)]
pub fn extend_var_subst_with_roots(
    pool: &Pool,
    scheme_var_ids: &[u32],
    var_subst: &mut FxHashMap<u32, Idx>,
) {
    let mut extensions: Vec<(u32, Idx)> = Vec::new();
    for &sv_id in scheme_var_ids {
        let Some(concrete) = var_subst.get(&sv_id).copied() else {
            continue;
        };
        let Some(sv_idx) = pool.var_idx_for_id(sv_id) else {
            continue;
        };
        let root = pool.resolve_fully(sv_idx);
        if pool.tag(root) == Tag::Var {
            let root_vid = pool.data(root);
            if root_vid != sv_id {
                extensions.push((root_vid, concrete));
            }
        }
    }
    for (vid, concrete) in extensions {
        var_subst.entry(vid).or_insert(concrete);
    }
}

#[cfg(test)]
mod tests;
