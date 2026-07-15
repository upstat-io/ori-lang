//! Validated parent/lambda topology for executable function bodies.

use ori_arc::ArcFunction;
use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{FunctionId, RealizationError};

/// Names-only family topology supplied by whole-program realization.
///
/// The parent and every lambda name are resolved to stable [`FunctionId`]
/// values only after the executable body table has been sorted and closed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionFamilyTopology {
    parent: Name,
    lambdas: Box<[Name]>,
}

impl FunctionFamilyTopology {
    /// Construct one parent family while preserving producer lambda order.
    #[must_use]
    pub fn new(parent: Name, lambdas: Vec<Name>) -> Self {
        Self {
            parent,
            lambdas: lambdas.into_boxed_slice(),
        }
    }

    /// Return the parent callable identity.
    #[must_use]
    pub const fn parent(&self) -> Name {
        self.parent
    }

    /// Return the ordered lambda identities owned by the parent.
    #[must_use]
    pub fn lambdas(&self) -> &[Name] {
        &self.lambdas
    }
}

pub(super) struct FrozenFunctionFamilies {
    pub(super) lambdas_by_parent: FxHashMap<FunctionId, Box<[FunctionId]>>,
    lambda_functions: FxHashSet<FunctionId>,
}

impl FrozenFunctionFamilies {
    pub(super) fn is_lambda(&self, function: FunctionId) -> bool {
        self.lambda_functions.contains(&function)
    }
}

pub(super) fn freeze_function_families(
    topologies: Vec<FunctionFamilyTopology>,
    functions: &[ArcFunction],
    function_ids: &FxHashMap<Name, FunctionId>,
) -> Result<FrozenFunctionFamilies, RealizationError> {
    let mut claimed_functions = FxHashSet::default();
    let mut lambdas_by_parent = FxHashMap::default();
    let mut lambda_functions = FxHashSet::default();

    for topology in topologies {
        let parent = topology.parent();
        let parent_id = function_ids
            .get(&parent)
            .copied()
            .ok_or(RealizationError::UnknownFunctionFamilyParent { parent })?;
        claim_family_member(parent, parent_id, &mut claimed_functions)?;

        let mut lambda_ids = Vec::with_capacity(topology.lambdas().len());
        for &lambda in topology.lambdas() {
            let lambda_id = function_ids
                .get(&lambda)
                .copied()
                .ok_or(RealizationError::UnknownFunctionFamilyLambda { parent, lambda })?;
            claim_family_member(lambda, lambda_id, &mut claimed_functions)?;
            lambda_functions.insert(lambda_id);
            lambda_ids.push(lambda_id);
        }
        lambdas_by_parent.insert(parent_id, lambda_ids.into_boxed_slice());
    }

    for function in functions {
        let function_id = function_ids.get(&function.name).copied().ok_or(
            RealizationError::MissingFunctionIdentity {
                name: function.name,
            },
        )?;
        if !claimed_functions.contains(&function_id) {
            return Err(RealizationError::MissingFunctionFamily {
                function: function.name,
            });
        }
    }

    Ok(FrozenFunctionFamilies {
        lambdas_by_parent,
        lambda_functions,
    })
}

fn claim_family_member(
    member: Name,
    function: FunctionId,
    claimed_functions: &mut FxHashSet<FunctionId>,
) -> Result<(), RealizationError> {
    if claimed_functions.insert(function) {
        Ok(())
    } else {
        Err(RealizationError::DuplicateFunctionFamilyMember { member })
    }
}
