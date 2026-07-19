//! User-defined drop target binding.

use rustc_hash::FxHashMap;

use ori_repr::executable::{RealizationError, UserDropBinding};
use ori_types::{Pool, TypeRegistry};

use super::ProgramRealizationError;

pub(crate) fn collect_user_drop_bindings(
    registry: &TypeRegistry,
    typed_bindings: &[UserDropBinding],
    pool: &Pool,
) -> Result<Vec<UserDropBinding>, ProgramRealizationError> {
    let mut expected = FxHashMap::default();
    for entry in registry.iter() {
        let Some(logical) = registry
            .burden(entry.idx)
            .and_then(|burden| burden.user_drop)
        else {
            continue;
        };
        expected.insert(pool.resolve_fully(entry.idx), (entry.idx, logical));
    }

    let mut seen = FxHashMap::default();
    let mut bindings = Vec::with_capacity(typed_bindings.len());
    for &binding in typed_bindings {
        if !pool.is_valid_idx(binding.ty()) {
            return Err(RealizationError::InvalidUserDropType { ty: binding.ty() }.into());
        }
        let canonical = pool.resolve_fully(binding.ty());
        let Some(&(expected_ty, expected_logical)) = expected.get(&canonical) else {
            return Err(ProgramRealizationError::UnexpectedUserDropRole { ty: binding.ty() });
        };
        if binding.logical() != expected_logical {
            return Err(ProgramRealizationError::UserDropLogicalIdentityMismatch {
                ty: expected_ty,
                expected: expected_logical,
                found: binding.logical(),
            });
        }
        let count = seen.entry(canonical).or_insert(0usize);
        *count += 1;
        if *count > 1 {
            return Err(ProgramRealizationError::AmbiguousUserDropTarget {
                ty: expected_ty,
                targets: *count,
            });
        }
        bindings.push(UserDropBinding::new(
            expected_ty,
            expected_logical,
            binding.target(),
        ));
    }
    if let Some((_, &(ty, _))) = expected
        .iter()
        .find(|(canonical, _)| !seen.contains_key(canonical))
    {
        return Err(ProgramRealizationError::MissingUserDropTarget { ty });
    }
    bindings.sort_by_key(|binding| binding.ty().raw());
    Ok(bindings)
}
