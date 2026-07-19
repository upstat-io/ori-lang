//! Stable logical facts for callables defined outside this executable unit.

use ori_arc::{EffectSummary, MemoryContract};
use ori_ir::Name;
use ori_types::{Idx, Pool};

use super::external_identities::compute_identities;
use super::RealizationError;

type FrozenExternalCallables = (
    Box<[ExternalCallable]>,
    rustc_hash::FxHashMap<Name, ExternalFunctionId>,
);

/// Stable index of an external callable in one closed executable artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalFunctionId(u32);

impl ExternalFunctionId {
    /// Return the external callable's zero-based artifact index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    fn from_index(index: usize) -> Result<Self, RealizationError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| RealizationError::TooManyExternalFunctions { count: index })
    }
}

/// Whether an external callable may unwind across the Ori call boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalUnwind {
    /// The producer proves that the call cannot unwind.
    NoUnwind,
    /// The call may unwind into the caller.
    MayUnwind,
}

impl ExternalUnwind {
    /// Construct the unwind fact carried by a final AIMS effect summary.
    #[must_use]
    pub const fn from_effects(effects: EffectSummary) -> Self {
        if effects.may_throw {
            Self::MayUnwind
        } else {
            Self::NoUnwind
        }
    }

    /// Return whether the boundary may unwind.
    #[must_use]
    pub const fn may_unwind(self) -> bool {
        matches!(self, Self::MayUnwind)
    }
}

/// Content-stable compatibility identities for one external callable.
///
/// The identities exclude the importer's local alias and include the producer
/// link symbol. Type identity is based on Merkle hashes, not pool-local `Idx`
/// values, so the same facts survive cross-module type re-interning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalFactIdentities {
    signature: u64,
    ownership: u64,
    effects: u64,
    unwind: u64,
}

/// Producer-authored logical facts transported with a callable export.
///
/// The callable's symbol and semantic signature travel beside this carrier in
/// compiled-module metadata. Their stable identity is nevertheless included
/// here, so an importer cannot attach these ownership/effect facts to a
/// different symbol or a different re-interned signature without validation
/// rejecting the record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalCallableMetadata {
    contract: MemoryContract,
    unwind: ExternalUnwind,
    identities: ExternalFactIdentities,
}

impl ExternalCallableMetadata {
    pub(super) fn freeze(
        link_symbol: &str,
        parameter_types: &[Idx],
        return_type: Idx,
        contract: MemoryContract,
        unwind: ExternalUnwind,
        pool: &Pool,
    ) -> Self {
        let identities = compute_identities(
            link_symbol,
            parameter_types,
            return_type,
            &contract,
            unwind,
            pool,
        );
        Self {
            contract,
            unwind,
            identities,
        }
    }

    /// Reconstruct a complete metadata carrier read from compiled-module
    /// storage.
    ///
    /// This constructor does not bless the record. Importers must pair it with
    /// the transported symbol and signature through
    /// [`ExternalCallable::from_imported_metadata`], then run
    /// [`validate_external_callables`] before the facts can enter AIMS.
    #[must_use]
    pub fn from_imported_parts(
        contract: MemoryContract,
        unwind: ExternalUnwind,
        identities: ExternalFactIdentities,
    ) -> Self {
        Self {
            contract,
            unwind,
            identities,
        }
    }

    /// Final producer-owned AIMS contract.
    #[must_use]
    pub const fn contract(&self) -> &MemoryContract {
        &self.contract
    }

    /// Exact cross-boundary unwind fact.
    #[must_use]
    pub const fn unwind(&self) -> ExternalUnwind {
        self.unwind
    }

    /// Content-stable compatibility identities.
    #[must_use]
    pub const fn identities(&self) -> ExternalFactIdentities {
        self.identities
    }
}

impl ExternalFactIdentities {
    /// Construct identities read from a producer's compiled-module metadata.
    #[must_use]
    pub const fn from_raw(signature: u64, ownership: u64, effects: u64, unwind: u64) -> Self {
        Self {
            signature,
            ownership,
            effects,
            unwind,
        }
    }

    /// Stable signature-fact hash.
    #[must_use]
    pub const fn signature(self) -> u64 {
        self.signature
    }

    /// Stable ownership-contract hash.
    #[must_use]
    pub const fn ownership(self) -> u64 {
        self.ownership
    }

    /// Stable effect-summary hash.
    #[must_use]
    pub const fn effects(self) -> u64 {
        self.effects
    }

    /// Stable unwind-fact hash.
    #[must_use]
    pub const fn unwind(self) -> u64 {
        self.unwind
    }
}

/// Exact backend-neutral signature and AIMS facts for an external callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalCallable {
    name: Name,
    link_symbol: Box<str>,
    parameter_types: Box<[Idx]>,
    return_type: Idx,
    metadata: ExternalCallableMetadata,
}

impl ExternalCallable {
    /// Freeze producer facts and compute their cross-module identities.
    #[must_use]
    pub fn freeze(
        name: Name,
        link_symbol: impl Into<Box<str>>,
        parameter_types: Vec<Idx>,
        return_type: Idx,
        contract: MemoryContract,
        unwind: ExternalUnwind,
        pool: &Pool,
    ) -> Self {
        let link_symbol = link_symbol.into();
        let metadata = ExternalCallableMetadata::freeze(
            &link_symbol,
            &parameter_types,
            return_type,
            contract,
            unwind,
            pool,
        );
        Self {
            name,
            link_symbol,
            parameter_types: parameter_types.into_boxed_slice(),
            return_type,
            metadata,
        }
    }

    /// Reconstruct facts transported from a producer metadata record.
    ///
    /// Executable-program validation recomputes every identity against the
    /// importing pool and rejects stale or signature-mismatched metadata.
    #[must_use]
    pub fn from_imported_parts(
        name: Name,
        link_symbol: impl Into<Box<str>>,
        parameter_types: Vec<Idx>,
        return_type: Idx,
        contract: MemoryContract,
        unwind: ExternalUnwind,
        identities: ExternalFactIdentities,
    ) -> Self {
        Self::from_imported_metadata(
            name,
            link_symbol,
            parameter_types,
            return_type,
            ExternalCallableMetadata::from_imported_parts(contract, unwind, identities),
        )
    }

    /// Reconstruct a callable from one required compiled-module metadata
    /// carrier.
    #[must_use]
    pub fn from_imported_metadata(
        name: Name,
        link_symbol: impl Into<Box<str>>,
        parameter_types: Vec<Idx>,
        return_type: Idx,
        metadata: ExternalCallableMetadata,
    ) -> Self {
        Self {
            name,
            link_symbol: link_symbol.into(),
            parameter_types: parameter_types.into_boxed_slice(),
            return_type,
            metadata,
        }
    }

    /// Import-local callable name used by ARC direct-call sites.
    #[must_use]
    pub const fn name(&self) -> Name {
        self.name
    }

    /// Exact symbol supplied to the physical linker or JIT.
    #[must_use]
    pub fn link_symbol(&self) -> &str {
        &self.link_symbol
    }

    /// Semantic parameter types in call order.
    #[must_use]
    pub fn parameter_types(&self) -> &[Idx] {
        &self.parameter_types
    }

    /// Semantic result type.
    #[must_use]
    pub const fn return_type(&self) -> Idx {
        self.return_type
    }

    /// Final producer-owned AIMS contract.
    #[must_use]
    pub const fn contract(&self) -> &MemoryContract {
        self.metadata.contract()
    }

    /// Exact cross-boundary unwind fact.
    #[must_use]
    pub const fn unwind(&self) -> ExternalUnwind {
        self.metadata.unwind()
    }

    /// Producer compatibility identities.
    #[must_use]
    pub const fn identities(&self) -> ExternalFactIdentities {
        self.metadata.identities()
    }

    /// Required producer metadata carried by this declaration.
    #[must_use]
    pub const fn metadata(&self) -> &ExternalCallableMetadata {
        &self.metadata
    }

    pub(super) fn validate_identities(&self, pool: &Pool) -> bool {
        self.identities()
            == compute_identities(
                &self.link_symbol,
                &self.parameter_types,
                self.return_type,
                self.metadata.contract(),
                self.metadata.unwind(),
                pool,
            )
    }
}

/// Validate producer-authored external facts before they enter AIMS.
///
/// Validation is intentionally independent of executable closure: callers
/// run it on the pre-AIMS batch, while [`freeze_external_callables`] repeats
/// it when assigning stable artifact identities after AIMS.
pub fn validate_external_callables(
    callables: &[ExternalCallable],
    pool: &Pool,
) -> Result<(), RealizationError> {
    let mut by_name = rustc_hash::FxHashSet::default();
    for callable in callables {
        if callable.link_symbol.is_empty() {
            return Err(RealizationError::EmptyExternalLinkSymbol {
                name: callable.name,
            });
        }
        if callable.parameter_types.len() != callable.contract().params.len() {
            return Err(RealizationError::ExternalContractArity {
                name: callable.name,
                parameters: callable.parameter_types.len(),
                contract_parameters: callable.contract().params.len(),
            });
        }
        if callable.contract().effects.may_throw != callable.unwind().may_unwind() {
            return Err(RealizationError::ExternalUnwindMismatch {
                name: callable.name,
            });
        }
        if !callable
            .parameter_types
            .iter()
            .all(|&ty| pool.is_valid_idx(ty))
            || !pool.is_valid_idx(callable.return_type)
        {
            return Err(RealizationError::InvalidExternalSignature {
                name: callable.name,
            });
        }
        if !callable.validate_identities(pool) {
            return Err(RealizationError::StaleExternalFacts {
                name: callable.name,
            });
        }
        if !by_name.insert(callable.name) {
            return Err(RealizationError::DuplicateExternalFunction {
                name: callable.name,
            });
        }
    }

    for (index, left) in callables.iter().enumerate() {
        for right in &callables[index + 1..] {
            if left.link_symbol == right.link_symbol
                && (left.parameter_types != right.parameter_types
                    || left.return_type != right.return_type
                    || left.metadata != right.metadata)
            {
                return Err(RealizationError::ExternalAliasFactsMismatch {
                    first: left.name,
                    second: right.name,
                });
            }
        }
    }

    Ok(())
}

pub(super) fn freeze_external_callables(
    mut callables: Vec<ExternalCallable>,
    pool: &Pool,
) -> Result<FrozenExternalCallables, RealizationError> {
    callables.sort_by(|left, right| {
        left.name
            .raw()
            .cmp(&right.name.raw())
            .then_with(|| left.link_symbol.cmp(&right.link_symbol))
    });
    validate_external_callables(&callables, pool)?;
    let mut ids = rustc_hash::FxHashMap::default();
    for (index, callable) in callables.iter().enumerate() {
        let id = ExternalFunctionId::from_index(index)?;
        let previous = ids.insert(callable.name, id);
        debug_assert!(previous.is_none(), "validated external names are unique");
    }

    Ok((callables.into_boxed_slice(), ids))
}
