//! Stable user-drop callable identities for a closed executable.

use ori_ir::Name;
use ori_registry::burden::FnSym;
use ori_types::{Idx, Pool, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{FunctionId, RealizationError};

/// Upstream binding between a burden identity and its realized callable body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserDropBinding {
    ty: Idx,
    logical: FnSym,
    target: Name,
}

impl UserDropBinding {
    /// Construct a binding from type-checker burden facts and realization's
    /// stable qualified impl-body identity.
    #[must_use]
    pub const fn new(ty: Idx, logical: FnSym, target: Name) -> Self {
        Self {
            ty,
            logical,
            target,
        }
    }

    /// Type whose destruction selects this body.
    #[must_use]
    pub const fn ty(self) -> Idx {
        self.ty
    }

    /// Stable logical operation identity assigned by the burden producer.
    #[must_use]
    pub const fn logical(self) -> FnSym {
        self.logical
    }

    /// Exact realized callable body implementing the user operation.
    #[must_use]
    pub const fn target(self) -> Name {
        self.target
    }
}

/// One validated user-drop operation in the executable artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableUserDrop {
    ty: Idx,
    logical: FnSym,
    target: FunctionId,
}

impl ExecutableUserDrop {
    /// Type whose destruction runs this operation.
    #[must_use]
    pub const fn ty(self) -> Idx {
        self.ty
    }

    /// Stable logical operation identity from the burden calculus.
    #[must_use]
    pub const fn logical(self) -> FnSym {
        self.logical
    }

    /// Exact realized callable body implementing the operation.
    #[must_use]
    pub const fn target(self) -> FunctionId {
        self.target
    }
}

/// Complete validated user-drop plan for one executable unit.
#[derive(Default)]
pub struct ExecutableDropPlan {
    entries: Box<[ExecutableUserDrop]>,
    by_type: FxHashMap<Idx, usize>,
}

impl ExecutableDropPlan {
    /// Stable user-drop entries, ordered by artifact-local type identity.
    #[must_use]
    pub fn entries(&self) -> &[ExecutableUserDrop] {
        &self.entries
    }

    pub(super) fn get(&self, ty: Idx, pool: &Pool) -> Option<&ExecutableUserDrop> {
        self.by_type
            .get(&ty)
            .or_else(|| self.by_type.get(&pool.resolve_fully(ty)))
            .map(|&index| &self.entries[index])
    }
}

pub(super) fn freeze_user_drop_plan(
    bindings: Vec<UserDropBinding>,
    registry: &TypeRegistry,
    pool: &Pool,
    functions: &[ori_arc::ArcFunction],
    function_ids: &FxHashMap<Name, FunctionId>,
) -> Result<ExecutableDropPlan, RealizationError> {
    let mut expected = FxHashMap::default();
    for entry in registry.iter() {
        let Some(logical) = registry
            .burden(entry.idx)
            .and_then(|burden| burden.user_drop)
        else {
            continue;
        };
        let canonical = pool.resolve_fully(entry.idx);
        if expected.insert(canonical, (entry.idx, logical)).is_some() {
            return Err(RealizationError::UserDropTypeIdentityCollision {
                ty: entry.idx,
                canonical,
            });
        }
    }

    let mut seen = FxHashSet::default();
    let mut entries = Vec::with_capacity(bindings.len());
    for binding in bindings {
        if !pool.is_valid_idx(binding.ty) {
            return Err(RealizationError::InvalidUserDropType { ty: binding.ty });
        }
        let canonical = pool.resolve_fully(binding.ty);
        if !seen.insert(canonical) {
            return Err(RealizationError::DuplicateUserDropBinding { ty: binding.ty });
        }
        let Some(&(expected_ty, expected_logical)) = expected.get(&canonical) else {
            return Err(RealizationError::UnexpectedUserDropBinding { ty: binding.ty });
        };
        if binding.logical != expected_logical {
            return Err(RealizationError::UserDropLogicalIdentityMismatch {
                ty: expected_ty,
                expected: expected_logical,
                found: binding.logical,
            });
        }
        let target = function_ids.get(&binding.target).copied().ok_or(
            RealizationError::MissingUserDropTarget {
                ty: expected_ty,
                target: binding.target,
            },
        )?;
        let body = &functions[target.index()];
        let signature_valid = body.params.len() == 1
            && pool.resolve_fully(body.params[0].ty) == canonical
            && pool.resolve_fully(body.return_type) == Idx::UNIT;
        if !signature_valid {
            return Err(RealizationError::InvalidUserDropSignature {
                ty: expected_ty,
                target: binding.target,
                parameters: body.params.len(),
                parameter_type: body.params.first().map(|parameter| parameter.ty),
                return_type: body.return_type,
            });
        }
        entries.push(ExecutableUserDrop {
            ty: expected_ty,
            logical: expected_logical,
            target,
        });
    }

    if let Some((_, &(ty, _))) = expected
        .iter()
        .find(|(canonical, _)| !seen.contains(canonical))
    {
        return Err(RealizationError::MissingUserDropBinding { ty });
    }

    entries.sort_by_key(|entry| entry.ty.raw());
    let mut by_type = FxHashMap::default();
    for (index, entry) in entries.iter().enumerate() {
        by_type.insert(entry.ty, index);
        by_type.insert(pool.resolve_fully(entry.ty), index);
    }
    Ok(ExecutableDropPlan {
        entries: entries.into_boxed_slice(),
        by_type,
    })
}
