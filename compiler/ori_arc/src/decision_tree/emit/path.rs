use ori_ir::Span;
use ori_types::{Idx, Tag};

use crate::ir::ArcVarId;
use crate::lower::collections::{emit_list_element, emit_list_rest_slice};

use super::super::PathInstruction;

/// Resolve a scrutinee path by emitting projections from the root value.
///
/// Recursive payload projections retain the enum type needed to dereference
/// RC-boxed fields.
pub(in crate::decision_tree) fn resolve_path(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    root: ArcVarId,
    root_ty: Idx,
    path: &[PathInstruction],
    span: Span,
    variant_stack: &[(Idx, u32)],
) -> ArcVarId {
    let pool = lowerer.pool;
    let mut current = root;
    let mut current_ty = root_ty;
    let mut tag_step_idx = 0;

    for step in path {
        let (field, output_ty) = match step {
            PathInstruction::TagPayload(field) => {
                let field_ty = if tag_step_idx < variant_stack.len() {
                    let (enum_ty, variant_idx) = variant_stack[tag_step_idx];
                    lookup_variant_field_type(pool, enum_ty, variant_idx, *field)
                } else {
                    tracing::warn!(
                        tag_step = tag_step_idx,
                        stack_len = variant_stack.len(),
                        "TagPayload step has no variant context; falling back to UNIT"
                    );
                    Idx::UNIT
                };
                tag_step_idx += 1;
                (field + 1, field_ty)
            }
            PathInstruction::TupleIndex(index) => {
                let resolved = pool.resolve_fully(current_ty);
                let element_ty = if pool.tag(resolved) == Tag::Tuple {
                    let count = pool.tuple_elem_count(resolved);
                    if (*index as usize) < count {
                        pool.tuple_elem(resolved, *index as usize)
                    } else {
                        Idx::UNIT
                    }
                } else {
                    Idx::UNIT
                };
                (*index, element_ty)
            }
            PathInstruction::StructField(index) => {
                let resolved = pool.resolve_fully(current_ty);
                let field_ty = if pool.tag(resolved) == Tag::Struct {
                    let count = pool.struct_field_count(resolved);
                    if (*index as usize) < count {
                        let (_, ty) = pool.struct_field(resolved, *index as usize);
                        ty
                    } else {
                        Idx::UNIT
                    }
                } else {
                    Idx::UNIT
                };
                (*index, field_ty)
            }
            PathInstruction::ListElement(index) => {
                let resolved = pool.resolve_fully(current_ty);
                let element_ty = if pool.tag(resolved) == Tag::List {
                    pool.list_elem(resolved)
                } else {
                    Idx::UNIT
                };
                current = emit_list_element(
                    lowerer.builder,
                    lowerer.interner,
                    current,
                    *index,
                    element_ty,
                    Some(span),
                );
                current_ty = element_ty;
                continue;
            }
            PathInstruction::ListRest(start_index) => {
                current = emit_list_rest_slice(
                    lowerer.builder,
                    lowerer.interner,
                    current,
                    *start_index,
                    current_ty,
                    Some(span),
                );
                continue;
            }
        };
        current = lowerer
            .builder
            .emit_project(output_ty, current, field, Some(span));
        current_ty = output_ty;
    }
    current
}

fn lookup_variant_field_type(
    pool: &ori_types::Pool,
    enum_type: Idx,
    variant_index: u32,
    field_index: u32,
) -> Idx {
    let resolved = pool.resolve_fully(enum_type);
    match pool.tag(resolved) {
        Tag::Enum => {
            let variants = pool.enum_variants(resolved);
            if let Some((_, fields)) = variants.get(variant_index as usize) {
                if let Some(&field_ty) = fields.get(field_index as usize) {
                    return field_ty;
                }
            }
        }
        Tag::Option => {
            if variant_index == 0 && field_index == 0 {
                return pool.option_inner(resolved);
            }
        }
        Tag::Result => {
            if field_index == 0 {
                return if variant_index == 0 {
                    pool.result_ok(resolved)
                } else {
                    pool.result_err(resolved)
                };
            }
        }
        _ => {}
    }
    Idx::UNIT
}
