//! Shared typed struct-field resolution for canonical desugars.

use ori_ir::{Name, TypeId};
use ori_types::{Idx, Tag};

use crate::lower::Lowerer;

impl Lowerer<'_> {
    /// Resolve declaration-order field names and concrete logical types.
    ///
    /// The receiver type is authoritative: its resolved struct body contains
    /// substitutions for an applied generic such as `Box<int>`. The registry
    /// fallback preserves error recovery for incomplete synthetic test state,
    /// while still resolving aliases recorded in declaration field types.
    pub(crate) fn resolve_struct_fields(
        &self,
        name: Name,
        receiver_ty: TypeId,
    ) -> Option<Vec<(Name, TypeId)>> {
        let receiver_idx = Idx::from_raw(receiver_ty.raw());
        let resolved = self.pool.resolve_fully(receiver_idx);
        if self.pool.is_valid_idx(resolved)
            && self.pool.tag(resolved) == Tag::Struct
            && self.pool.struct_name(resolved) == name
        {
            return Some(
                self.pool
                    .struct_fields(resolved)
                    .into_iter()
                    .map(|(field, ty)| (field, self.resolved_type_id(ty)))
                    .collect(),
            );
        }

        let type_entry = self.typed.type_def(name)?;
        match &type_entry.kind {
            ori_types::TypeKind::Struct(def) => Some(
                def.fields
                    .iter()
                    .map(|field| (field.name, self.resolved_type_id(field.ty)))
                    .collect(),
            ),
            _ => None,
        }
    }

    fn resolved_type_id(&self, ty: Idx) -> TypeId {
        TypeId::from_raw(self.pool.resolve_fully(ty).raw())
    }
}
