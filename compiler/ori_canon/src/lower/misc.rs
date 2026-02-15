//! Miscellaneous lowering helpers — cast target extraction, type name mapping.

use ori_ir::{Name, StringInterner, TypeId};

use super::Lowerer;

impl Lowerer<'_> {
    // Cast Target Name Extraction

    /// Extract the target type name from a `ParsedTypeId` for `Cast` expressions.
    ///
    /// The evaluator dispatches cast operations by type name (e.g. "int", "float",
    /// "str"), while the LLVM backend uses the resolved `TypeId` from `CanNode.ty`.
    pub(super) fn extract_cast_target_name(&self, ty_id: ori_ir::ParsedTypeId) -> Name {
        let parsed_ty = self.src.get_parsed_type(ty_id);
        match parsed_ty {
            ori_ir::ParsedType::Primitive(type_id) => {
                // Map well-known TypeIds to their interned names.
                type_id_to_name(*type_id, self.interner)
            }
            ori_ir::ParsedType::Named { name, .. } => *name,
            // For complex types, fall back to an empty name (error recovery).
            _ => Name::EMPTY,
        }
    }
}

/// Map a primitive `TypeId` to its interned name.
///
/// Cast expressions need the type name for evaluator dispatch (e.g. `as int`).
/// The LLVM backend ignores this and uses the resolved `TypeId` on `CanNode.ty`.
pub(crate) fn type_id_to_name(type_id: TypeId, interner: &StringInterner) -> Name {
    let s = match type_id {
        TypeId::INT => "int",
        TypeId::FLOAT => "float",
        TypeId::BOOL => "bool",
        TypeId::STR => "str",
        TypeId::CHAR => "char",
        TypeId::BYTE => "byte",
        TypeId::UNIT => "void",
        _ => return Name::EMPTY,
    };
    interner.intern(s)
}
