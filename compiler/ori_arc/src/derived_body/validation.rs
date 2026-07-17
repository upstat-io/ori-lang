//! Shared validation for concrete compiler-derived method signatures.

use ori_types::{Idx, Pool, Tag, TypeFlags};

pub(super) const SELF_PARAMETER: &str = "self parameter";
pub(super) const RETURN_TYPE: &str = "return type";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConcreteTypeError {
    InvalidTypeIndex { position: &'static str, ty: Idx },
    NonConcreteType { position: &'static str, ty: Idx },
}

pub(super) fn validate_concrete_type(
    pool: &Pool,
    position: &'static str,
    ty: Idx,
) -> Result<(), ConcreteTypeError> {
    if !pool.is_valid_idx(ty) {
        return Err(ConcreteTypeError::InvalidTypeIndex { position, ty });
    }
    let resolved = pool.resolve_fully(ty);
    if !pool.is_valid_idx(resolved) {
        return Err(ConcreteTypeError::InvalidTypeIndex {
            position,
            ty: resolved,
        });
    }

    let flags = pool.flags(resolved);
    let unresolved = TypeFlags::HAS_SELF | TypeFlags::HAS_PROJECTION;
    if !flags.is_recordable()
        || flags.intersects(unresolved)
        || matches!(pool.tag(resolved), Tag::Scheme | Tag::ModuleNs)
    {
        return Err(ConcreteTypeError::NonConcreteType { position, ty });
    }
    Ok(())
}
