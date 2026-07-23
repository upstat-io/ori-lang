//! General-enum payload matching and byte-offset projection.

use ori_types::{Idx, Pool, Tag};

use super::super::super::drop_enum::{compute_variant_field_offsets, variant_field_offset};
use super::super::super::ArcIrEmitter;

fn find_matching_enum_payload_fields(
    pool: &Pool,
    enum_ty: Idx,
    payload_field_idx: u32,
    field_type: Idx,
) -> Option<(Idx, Vec<Idx>)> {
    let resolved = pool.resolve_fully(enum_ty);
    if pool.tag(resolved) != Tag::Enum {
        return None;
    }
    let field_index = usize::try_from(payload_field_idx).ok()?;
    let resolved_field_type = pool.resolve_fully(field_type);
    pool.enum_variants(resolved)
        .into_iter()
        .find_map(|(_, fields)| {
            (fields
                .get(field_index)
                .map(|field| pool.resolve_fully(*field))
                == Some(resolved_field_type))
            .then_some((resolved, fields))
        })
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    fn compute_enum_payload_byte_offset(
        &self,
        enum_ty: Idx,
        payload_field_idx: u32,
        field_type: Idx,
    ) -> Option<u64> {
        let (resolved, fields) =
            find_matching_enum_payload_fields(self.pool, enum_ty, payload_field_idx, field_type)?;
        let field_index = usize::try_from(payload_field_idx).ok()?;
        let offsets = compute_variant_field_offsets(&fields, resolved, self);
        Some(variant_field_offset(&offsets, field_index))
    }

    /// Records a codegen error when enum metadata has no matching payload field.
    pub(super) fn compute_general_enum_payload_byte_offset(
        &mut self,
        enum_ty: Idx,
        field: u32,
        field_ty: Idx,
    ) -> Option<u64> {
        let offset = field.checked_sub(1).and_then(|payload_field| {
            self.compute_enum_payload_byte_offset(enum_ty, payload_field, field_ty)
        });

        if offset.is_none() {
            self.builder.record_codegen_error_with_msg(format!(
                "general-enum projection field {field} (type #{}) has no matching payload field in enum type #{}; report this compiler bug",
                field_ty.raw(),
                enum_ty.raw()
            ));
        }
        offset
    }
}

#[cfg(test)]
mod tests {
    use ori_ir::Name;
    use ori_types::{EnumVariant, Idx, Pool};

    use super::find_matching_enum_payload_fields;

    #[test]
    fn enum_payload_matching_rejects_unmatched_metadata() {
        let mut pool = Pool::new();
        let enum_ty = pool.enum_type(
            Name::from_raw(100),
            &[EnumVariant {
                name: Name::from_raw(101),
                field_types: vec![Idx::INT],
            }],
        );

        assert_eq!(
            find_matching_enum_payload_fields(&pool, enum_ty, 0, Idx::INT),
            Some((enum_ty, vec![Idx::INT]))
        );

        assert_eq!(
            find_matching_enum_payload_fields(&pool, enum_ty, 0, Idx::BOOL),
            None
        );

        assert_eq!(
            find_matching_enum_payload_fields(&pool, Idx::INT, 0, Idx::INT),
            None
        );
    }
}
