//! Per-instantiation concrete-body enumeration + field/payload substitution for
//! deriving on generic composites. A generic struct/enum has one concrete
//! instantiation per distinct arg set; each instantiation's materialized body
//! lives in `Pool.resolutions`. These helpers enumerate those instantiations and
//! project the declared `FieldDef`/`VariantDef` lists onto each concrete body so
//! the strategy-driven derive codegen emits a layout-correct method per instance.

use ori_ir::Name;
use ori_types::{FieldDef, Idx, Tag, VariantDef, VariantFields};

use rustc_hash::FxHashSet;

use super::super::function_compiler::FunctionCompiler;

/// Enumerate every fully-resolved concrete instantiation of a generic composite
/// `type_name` in the codegen pool: `(applied_idx, concrete_body_idx)` pairs where
/// `applied_idx` is a `Tag::Applied` named `type_name` carrying no remaining type
/// variables and `concrete_body_idx = pool.resolve(applied_idx)` is its materialized
/// `Struct`/`Enum` body. Empty for non-generic types (no `Applied`).
pub(super) fn concrete_instantiations<'a>(
    fc: &FunctionCompiler<'_, 'a, 'a, '_>,
    type_name: Name,
) -> Vec<(Idx, Idx)> {
    let pool = fc.pool();
    let mut out = Vec::new();
    let mut seen_bodies = FxHashSet::default();
    for idx in pool.iter_indices() {
        if pool.tag(idx) != Tag::Applied || pool.applied_name(idx) != type_name {
            continue;
        }
        let Some(concrete) = pool.resolve(idx) else {
            continue;
        };
        // Gate on the MATERIALIZED BODY's concreteness, NOT the `Applied`
        // node's flag. An `Applied(Wrap, [Applied(Wrap, [int])])` carries
        // `HAS_VAR` on its type-argument list (the args were not flag-cleared
        // after registration) while its resolved struct/enum body is fully
        // concrete — that body is the layout-correct instantiation a derived
        // method must be emitted for. The `Applied`-flag gate that preceded
        // this rejected every nested instantiation (the starve).
        let body = pool.resolve_fully(concrete);
        if pool.flags(body).has_any_var_or_infer() {
            continue;
        }
        let tag = pool.tag(body);
        if (tag == Tag::Struct || tag == Tag::Enum) && seen_bodies.insert(body) {
            out.push((idx, body));
        }
    }
    out
}

/// Build the concrete `FieldDef` list for a generic struct instantiation by
/// substituting each declared field's `ty` with the materialized concrete field
/// type from `concrete_idx`'s pool body, preserving field names/spans/visibility.
/// Falls back to the declared field when arities disagree.
pub(super) fn substitute_struct_fields<'a>(
    fc: &FunctionCompiler<'_, 'a, 'a, '_>,
    generic_fields: &[FieldDef],
    concrete_idx: Idx,
) -> Vec<FieldDef> {
    let concrete = fc.pool().struct_fields(concrete_idx);
    generic_fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let mut out = field.clone();
            if let Some(&(_, ty)) = concrete.get(i) {
                out.ty = ty;
            }
            out
        })
        .collect()
}

/// Build the concrete `VariantDef` list for a generic enum instantiation by
/// substituting each variant payload type with the materialized concrete payload
/// from `concrete_idx`'s pool body, preserving the variant shape (Unit/Tuple/Record)
/// and field names. Falls back to the declared payload on arity skew.
pub(super) fn substitute_enum_variants<'a>(
    fc: &FunctionCompiler<'_, 'a, 'a, '_>,
    generic_variants: &[VariantDef],
    concrete_idx: Idx,
) -> Vec<VariantDef> {
    let concrete = fc.pool().enum_variants(concrete_idx);
    generic_variants
        .iter()
        .enumerate()
        .map(|(vi, variant)| {
            let payloads = concrete.get(vi).map(|(_, tys)| tys.as_slice());
            let fields = match &variant.fields {
                VariantFields::Unit => VariantFields::Unit,
                VariantFields::Tuple(tys) => {
                    let mut out = tys.clone();
                    if let Some(p) = payloads {
                        for (j, slot) in out.iter_mut().enumerate() {
                            if let Some(&ty) = p.get(j) {
                                *slot = ty;
                            }
                        }
                    }
                    VariantFields::Tuple(out)
                }
                VariantFields::Record(fields) => {
                    let out = fields
                        .iter()
                        .enumerate()
                        .map(|(j, f)| {
                            let mut nf = f.clone();
                            if let Some(&ty) = payloads.and_then(|p| p.get(j)) {
                                nf.ty = ty;
                            }
                            nf
                        })
                        .collect();
                    VariantFields::Record(out)
                }
            };
            VariantDef {
                fields,
                ..variant.clone()
            }
        })
        .collect()
}

/// Enumerate the nested derive-bearing `Applied` instantiations directly
/// reachable through the field/payload types of a concrete instantiation body
/// `concrete_idx`. For each field/variant-payload type that (fully resolved to
/// its `Applied` head) names a type in `derive_bearing`, returns
/// `(applied_idx, concrete_body_idx, inner_name)` — the same `(Idx, Idx)` shape
/// [`concrete_instantiations`] produces for a top-level instantiation, plus the
/// inner type `Name` so the caller can recurse.
///
/// One hop only: the transitive closure is driven by the caller's work-list,
/// which re-invokes this on each emitted inner body. Fully-resolved-Applied head
/// detection covers nesting wrapped in `Option`/`Result`/`Tuple`/`List`/`Set`
/// (e.g. `Wrap<Option<Wrap<int>>>`) — those tags carry their child `Applied`,
/// surfaced by walking the aggregate children below.
pub(super) fn nested_derive_instantiations<'a>(
    fc: &FunctionCompiler<'_, 'a, 'a, '_>,
    concrete_idx: Idx,
    derive_bearing: &FxHashSet<Name>,
) -> Vec<(Idx, Idx, Name)> {
    let pool = fc.pool();
    let mut out = Vec::new();
    for child in nested_field_payload_types(pool, concrete_idx) {
        // Find the `Applied`-headed type reachable through transparent wrappers
        // (the child itself, or an inner of Option/Result/Tuple/List/Set).
        for applied in applied_heads(pool, child) {
            let name = pool.applied_name(applied);
            if !derive_bearing.contains(&name) {
                continue;
            }
            let Some(concrete) = pool.resolve(applied) else {
                continue;
            };
            // Gate on the materialized BODY's concreteness, not the `Applied`
            // node's flag (which carries HAS_VAR on its arg list — see
            // `concrete_instantiations`).
            let body = pool.resolve_fully(concrete);
            if pool.flags(body).has_any_var_or_infer() {
                continue;
            }
            let tag = pool.tag(body);
            if tag == Tag::Struct || tag == Tag::Enum {
                out.push((applied, body, name));
            }
        }
    }
    out
}

/// The directly-nested field (Struct) / variant-payload (Enum) type indices of
/// a concrete instantiation body. Empty for non-aggregate tags.
fn nested_field_payload_types(pool: &ori_types::Pool, concrete_idx: Idx) -> Vec<Idx> {
    match pool.tag(concrete_idx) {
        Tag::Struct => pool
            .struct_fields(concrete_idx)
            .into_iter()
            .map(|(_, t)| t)
            .collect(),
        Tag::Enum => pool
            .enum_variants(concrete_idx)
            .into_iter()
            .flat_map(|(_, payloads)| payloads)
            .collect(),
        _ => Vec::new(),
    }
}

/// Collect every `Applied`-headed type reachable from `ty` through transparent
/// generic wrappers (`Option`/`Result`/`Tuple`/`List`/`Set`). Returns `ty`
/// itself when it resolves to an `Applied`. Cycle-safe via a visited set.
fn applied_heads(pool: &ori_types::Pool, ty: Idx) -> Vec<Idx> {
    let mut out = Vec::new();
    let mut visiting = FxHashSet::default();
    collect_applied_heads(pool, ty, &mut visiting, &mut out);
    out
}

fn collect_applied_heads(
    pool: &ori_types::Pool,
    ty: Idx,
    visiting: &mut FxHashSet<Idx>,
    out: &mut Vec<Idx>,
) {
    let resolved = pool.resolve_fully(ty);
    if !visiting.insert(resolved) {
        return;
    }
    match pool.tag(resolved) {
        Tag::Applied => out.push(resolved),
        Tag::Option => collect_applied_heads(pool, pool.option_inner(resolved), visiting, out),
        Tag::Result => {
            collect_applied_heads(pool, pool.result_ok(resolved), visiting, out);
            collect_applied_heads(pool, pool.result_err(resolved), visiting, out);
        }
        Tag::Tuple => {
            for elem in pool.tuple_elems(resolved) {
                collect_applied_heads(pool, elem, visiting, out);
            }
        }
        Tag::List => collect_applied_heads(pool, pool.list_elem(resolved), visiting, out),
        Tag::Set => collect_applied_heads(pool, pool.set_elem(resolved), visiting, out),
        _ => {}
    }
    visiting.remove(&resolved);
}
