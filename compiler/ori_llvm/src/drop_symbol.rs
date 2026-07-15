//! LLVM symbol projection for per-type drop glue.

use ori_types::Idx;

/// Prefix for LLVM functions that implement one type's drop plan.
pub const DROP_GLUE_PREFIX: &str = "_ori_drop$";

/// Return the LLVM symbol for the drop function projected from `ty`'s plan.
#[must_use]
pub fn drop_glue_symbol(ty: Idx) -> String {
    format!("{DROP_GLUE_PREFIX}{}", ty.raw())
}

#[cfg(test)]
mod tests {
    use super::drop_glue_symbol;
    use ori_types::Idx;

    #[test]
    fn llvm_drop_glue_symbol_uses_type_identity() {
        assert_eq!(drop_glue_symbol(Idx::from_raw(141)), "_ori_drop$141");
        assert_eq!(
            drop_glue_symbol(Idx::STR),
            format!("_ori_drop${}", Idx::STR.raw())
        );
    }
}
