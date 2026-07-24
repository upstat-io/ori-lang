//! Exact semantic boundary roles for realized callable bodies.
//!
//! These facts are projections of frontend-owned semantic bindings. They tell
//! AIMS about language-prescribed callable boundaries without teaching the
//! calculus symbol spelling or any physical backend convention.

use ori_ir::Name;
use ori_registry::burden::FnSym;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::AccessClass;
use crate::ir::ArcFunction;

/// Semantic boundary facts keyed by exact realized callable identity.
#[derive(Debug, Default)]
pub struct CallableBoundaryFacts {
    user_drops: FxHashMap<Name, UserDropBoundary>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct UserDropBoundary {
    receiver: Idx,
    logical: FnSym,
}

impl CallableBoundaryFacts {
    /// Freeze exact user-defined `Drop` bodies and their receiver types.
    ///
    /// A user-drop body borrows `self`: the caller-owned drop glue invokes the
    /// body while every field is live, then performs the field walk and final
    /// storage release. The binding remains the authority; this table is the
    /// AIMS-facing projection of that fact.
    pub fn from_user_drop_targets(
        targets: impl IntoIterator<Item = (Name, Idx, FnSym)>,
    ) -> Result<Self, CallableBoundaryError> {
        let mut facts = Self::default();
        for (target, receiver, logical) in targets {
            let boundary = UserDropBoundary { receiver, logical };
            if let Some(previous) = facts.user_drops.insert(target, boundary) {
                if previous != boundary {
                    return Err(CallableBoundaryError::ConflictingUserDropBinding {
                        target,
                        first_receiver: previous.receiver,
                        first_logical: previous.logical,
                        second_receiver: receiver,
                        second_logical: logical,
                    });
                }
            }
        }
        Ok(facts)
    }

    pub(super) fn validate(
        &self,
        functions: &[ArcFunction],
        pool: &Pool,
    ) -> Result<ValidatedCallableBoundaryFacts<'_>, Vec<CallableBoundaryError>> {
        let mut by_name = FxHashMap::default();
        let mut duplicate_names = FxHashSet::default();
        for function in functions {
            if by_name.insert(function.name, function).is_some() {
                duplicate_names.insert(function.name);
            }
        }
        let mut targets: Vec<_> = self.user_drops.iter().collect();
        targets.sort_by_key(|(target, _)| target.raw());
        let mut errors = Vec::new();
        for (&target, &boundary) in targets {
            let receiver = boundary.receiver;
            if !pool.is_valid_idx(receiver) {
                errors.push(CallableBoundaryError::InvalidUserDropReceiver { target, receiver });
                continue;
            }
            if duplicate_names.contains(&target) {
                errors.push(CallableBoundaryError::DuplicateUserDropTarget { target });
                continue;
            }
            let Some(function) = by_name.get(&target) else {
                errors.push(CallableBoundaryError::MissingUserDropTarget { target });
                continue;
            };
            let parameter_type = function.params.first().map(|parameter| parameter.ty);
            let valid = function.params.len() == 1
                && parameter_type
                    .is_some_and(|ty| pool.resolve_fully(ty) == pool.resolve_fully(receiver))
                && pool.resolve_fully(function.return_type) == Idx::UNIT;
            if !valid {
                errors.push(CallableBoundaryError::InvalidUserDropSignature {
                    target,
                    expected_receiver: receiver,
                    parameters: function.params.len(),
                    parameter_type,
                    return_type: function.return_type,
                });
            }
        }
        if errors.is_empty() {
            Ok(ValidatedCallableBoundaryFacts {
                user_drops: Some(&self.user_drops),
            })
        } else {
            Err(errors)
        }
    }
}

/// Callable roles proven consistent with the exact closed ARC batch.
///
/// Construction is private to this module, so analysis cannot accidentally
/// consume an unvalidated nonempty role table.
#[derive(Debug)]
pub(crate) struct ValidatedCallableBoundaryFacts<'a> {
    user_drops: Option<&'a FxHashMap<Name, UserDropBoundary>>,
}

impl ValidatedCallableBoundaryFacts<'_> {
    /// Empty roles need no body validation and preserve the legacy standalone
    /// analysis entry points.
    pub(crate) const fn empty() -> Self {
        Self { user_drops: None }
    }

    /// Apply the language-prescribed restricted-domain projection after raw
    /// body-fact extraction.
    ///
    /// Other contract dimensions remain body-derived. Reapplying this before
    /// every SCC join and after TRMC refresh keeps all consumers on the same
    /// semantic boundary. Validation proves a user-drop contract has receiver
    /// position 0, and contract extraction preserves function arity.
    pub(crate) fn constrain_contract(&self, name: Name, contract: &mut MemoryContract) {
        if self
            .user_drops
            .is_some_and(|targets| targets.contains_key(&name))
        {
            contract.params[0].access = AccessClass::Borrowed;
        }
    }
}

/// Invalid or contradictory callable-boundary facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallableBoundaryError {
    /// One exact body was assigned as the destructor for incompatible types.
    ConflictingUserDropBinding {
        target: Name,
        first_receiver: Idx,
        first_logical: FnSym,
        second_receiver: Idx,
        second_logical: FnSym,
    },
    /// The semantic binding names no body in the closed ARC batch.
    MissingUserDropTarget { target: Name },
    /// More than one closed ARC body carries the exact target identity.
    DuplicateUserDropTarget { target: Name },
    /// The semantic binding carries a receiver outside the closed type pool.
    InvalidUserDropReceiver { target: Name, receiver: Idx },
    /// A user-drop body does not have the required `(self: T) -> void` shape.
    InvalidUserDropSignature {
        target: Name,
        expected_receiver: Idx,
        parameters: usize,
        parameter_type: Option<Idx>,
        return_type: Idx,
    },
}

impl std::fmt::Display for CallableBoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConflictingUserDropBinding {
                target,
                first_receiver,
                first_logical,
                second_receiver,
                second_logical,
            } => write!(
                f,
                "callable name_id={} is bound to incompatible user-drop operations ({first_receiver:?}, {first_logical:?}) and ({second_receiver:?}, {second_logical:?})",
                target.raw(),
            ),
            Self::MissingUserDropTarget { target } => write!(
                f,
                "user-drop boundary names missing callable name_id={}",
                target.raw(),
            ),
            Self::DuplicateUserDropTarget { target } => write!(
                f,
                "user-drop boundary target name_id={} is carried by more than one ARC body",
                target.raw(),
            ),
            Self::InvalidUserDropReceiver { target, receiver } => write!(
                f,
                "user-drop callable name_id={} carries invalid receiver type {receiver:?}",
                target.raw(),
            ),
            Self::InvalidUserDropSignature {
                target,
                expected_receiver,
                parameters,
                parameter_type,
                return_type,
            } => write!(
                f,
                "user-drop callable name_id={} must have signature ({expected_receiver:?}) -> void; found {parameters} parameter(s), receiver {parameter_type:?}, return {return_type:?}",
                target.raw(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use super::*;
    use crate::aims::contract::{FipContract, MemoryContract};

    #[test]
    fn exact_user_drop_target_constrains_only_receiver_access() {
        let target = Name::from_raw(7);
        let logical = FnSym::new(NonZeroU32::MIN);
        let facts =
            match CallableBoundaryFacts::from_user_drop_targets([(target, Idx::STR, logical)]) {
                Ok(facts) => facts,
                Err(error) => panic!("one exact user-drop binding must be valid: {error:?}"),
            };
        let mut contract = MemoryContract::conservative(1);
        let prior_consumption = contract.params[0].consumption;

        let function = ArcFunction {
            name: target,
            params: vec![crate::ArcParam {
                var: crate::ArcVarId::new(0),
                ty: Idx::STR,
                ownership: crate::Ownership::Owned,
            }],
            return_type: Idx::UNIT,
            ..ArcFunction::default()
        };
        let pool = Pool::new();
        let validated = match facts.validate(&[function], &pool) {
            Ok(validated) => validated,
            Err(errors) => {
                panic!("test binding must have a valid closed-body signature: {errors:?}")
            }
        };
        validated.constrain_contract(target, &mut contract);

        assert_eq!(contract.params[0].access, AccessClass::Borrowed);
        assert_eq!(contract.params[0].consumption, prior_consumption);

        let mut ordinary = MemoryContract::all_borrowed(1, FipContract::Never);
        ordinary.params[0].access = AccessClass::Owned;
        validated.constrain_contract(Name::from_raw(8), &mut ordinary);
        assert_eq!(ordinary.params[0].access, AccessClass::Owned);
    }

    #[test]
    fn conflicting_receiver_bindings_fail_closed() {
        let target = Name::from_raw(7);
        let one = FnSym::new(NonZeroU32::MIN);
        let two = FnSym::new(NonZeroU32::new(2).unwrap_or(NonZeroU32::MIN));
        let Err(error) = CallableBoundaryFacts::from_user_drop_targets([
            (target, Idx::STR, one),
            (target, Idx::INT, two),
        ]) else {
            panic!("one body accepted incompatible user-drop receiver types")
        };
        assert!(matches!(
            error,
            CallableBoundaryError::ConflictingUserDropBinding { .. }
        ));
    }

    #[test]
    fn invalid_receiver_fails_validation_without_resolving_pool_data() {
        let target = Name::from_raw(7);
        let invalid = Idx::from_raw(u32::MAX);
        let logical = FnSym::new(NonZeroU32::MIN);
        let facts =
            match CallableBoundaryFacts::from_user_drop_targets([(target, invalid, logical)]) {
                Ok(facts) => facts,
                Err(error) => panic!(
                    "user-drop binding shape must be accepted before pool validation: {error:?}"
                ),
            };
        let Err(error) = facts.validate(&[], &Pool::new()) else {
            panic!("an out-of-pool user-drop receiver did not fail closed")
        };
        assert_eq!(
            error,
            [CallableBoundaryError::InvalidUserDropReceiver {
                target,
                receiver: invalid,
            }]
        );
    }
}
