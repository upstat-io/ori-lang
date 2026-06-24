//! Per-instantiation concrete-body enumeration + field/payload substitution for
//! deriving on generic composites. A generic struct/enum has one concrete
//! instantiation per distinct arg set; each instantiation's materialized body
//! lives in `Pool.resolutions`. These helpers enumerate those instantiations and
//! project the declared `FieldDef`/`VariantDef` lists onto each concrete body so
//! the strategy-driven derive codegen emits a layout-correct method per instance.

use ori_ir::Name;
use ori_types::{FieldDef, Idx, Tag, VariantDef, VariantFields};

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
    for idx in pool.iter_indices() {
        if pool.tag(idx) != Tag::Applied || pool.applied_name(idx) != type_name {
            continue;
        }
        if pool.flags(idx).has_any_var_or_infer() {
            continue;
        }
        let Some(concrete) = pool.resolve(idx) else {
            continue;
        };
        let tag = pool.tag(concrete);
        if tag == Tag::Struct || tag == Tag::Enum {
            out.push((idx, concrete));
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
