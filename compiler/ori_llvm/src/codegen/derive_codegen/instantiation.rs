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
        // Why: gate on the MATERIALIZED body's concreteness, not the `Applied`
        // node's flag — a nested `Applied` carries stale `HAS_VAR` on its arg
        // list while its resolved body is fully concrete (the emit target).
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
/// reachable through the field/payload types of a concrete body `concrete_idx`.
///
/// # Returns
///
/// For each field/payload whose fully-resolved `Applied` head names a
/// `derive_bearing` type, `(applied_idx, concrete_body_idx, inner_name)` — the
/// [`concrete_instantiations`] shape plus the inner `Name` for the caller to recurse.
///
/// # Scope
///
/// One hop only; the caller's work-list drives the transitive closure. Head
/// detection covers nesting wrapped in `Option`/`Result`/`Tuple`/`List`/`Set`
/// (`Wrap<Option<Wrap<int>>>`) via the aggregate-children walk below.
pub(super) fn nested_derive_instantiations<'a>(
    fc: &FunctionCompiler<'_, 'a, 'a, '_>,
    concrete_idx: Idx,
    derive_bearing: &FxHashSet<Name>,
) -> Vec<(Idx, Idx, Name)> {
    let pool = fc.pool();
    let mut out = Vec::new();
    for child in nested_field_payload_types(pool, concrete_idx) {
        // Find each derive-instantiation head reachable through transparent
        // wrappers (the child itself, an inner of Option/Result/Tuple/List/Set,
        // OR a directly-nested already-resolved generic Struct/Enum body).
        for head in derive_instantiation_heads(pool, child) {
            // The head is either a `Tag::Applied` node (resolve to its body) or
            // an already-materialized concrete `Tag::Struct`/`Tag::Enum` body
            // (a directly-nested generic instantiation field, e.g. the `Wrap<int>`
            // field of `Wrap<Wrap<int>>` is stored as the resolved struct body,
            // never an `Applied` node — these must be enqueued under the SAME idx
            // the field-dispatch `get_derived_method_for_type` looks up).
            let (applied, body, name) = match pool.tag(head) {
                Tag::Applied => {
                    let name = pool.applied_name(head);
                    let Some(concrete) = pool.resolve(head) else {
                        continue;
                    };
                    (head, pool.resolve_fully(concrete), name)
                }
                Tag::Struct => (head, head, pool.struct_name(head)),
                Tag::Enum => (head, head, pool.enum_name(head)),
                _ => continue,
            };
            // `derive_bearing` is the set of GENERIC derive-bearing type names
            // (built in `compile_derives`), so this gate admits only generic
            // instantiations — a non-generic derive-bearing struct/enum field
            // dispatches via the type-name path, never the per-instantiation
            // mono map.
            if !derive_bearing.contains(&name) {
                continue;
            }
            // Gate on the materialized BODY's concreteness, not the `Applied`
            // node's flag (which carries HAS_VAR on its arg list — see
            // `concrete_instantiations`).
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

/// Collect every derive-instantiation head reachable from `ty` through
/// transparent generic wrappers (`Option`/`Result`/`Tuple`/`List`/`Set`). A head
/// is either a `Tag::Applied` node OR an already-resolved concrete
/// `Tag::Struct`/`Tag::Enum` body (a directly-nested generic instantiation field,
/// which the type pool stores as the resolved body, not an `Applied`). Returns
/// `ty` itself when it resolves to such a head. One hop only — a Struct/Enum head
/// is NOT descended into (the caller's work-list drives transitivity). Cycle-safe
/// via a visited set.
fn derive_instantiation_heads(pool: &ori_types::Pool, ty: Idx) -> Vec<Idx> {
    let mut out = Vec::new();
    let mut visiting = FxHashSet::default();
    collect_derive_instantiation_heads(pool, ty, &mut visiting, &mut out);
    out
}

fn collect_derive_instantiation_heads(
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
        // A nested generic instantiation head — pushed as a one-hop head, NOT
        // descended into (the caller's work-list closes the transitive set).
        Tag::Applied | Tag::Struct | Tag::Enum => out.push(resolved),
        // Transparent wrappers — descend to the inner head(s).
        Tag::Option => {
            collect_derive_instantiation_heads(pool, pool.option_inner(resolved), visiting, out);
        }
        Tag::Result => {
            collect_derive_instantiation_heads(pool, pool.result_ok(resolved), visiting, out);
            collect_derive_instantiation_heads(pool, pool.result_err(resolved), visiting, out);
        }
        Tag::Tuple => {
            for elem in pool.tuple_elems(resolved) {
                collect_derive_instantiation_heads(pool, elem, visiting, out);
            }
        }
        Tag::List => {
            collect_derive_instantiation_heads(pool, pool.list_elem(resolved), visiting, out);
        }
        Tag::Set => {
            collect_derive_instantiation_heads(pool, pool.set_elem(resolved), visiting, out);
        }
        _ => {}
    }
    visiting.remove(&resolved);
}
