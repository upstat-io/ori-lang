//! Frozen primitive-operation facts carried by ARC IR artifacts.

use std::collections::BTreeMap;

use ori_registry::{OpStrategy, PrimitiveDescriptor};

use super::ArcVarId;

/// One validated primitive-operation fact.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PrimitiveFact {
    /// Typed semantic execution family or runtime identity.
    pub strategy: OpStrategy,
    /// Frozen ownership and allocation interface.
    pub descriptor: PrimitiveDescriptor,
}

impl PrimitiveFact {
    /// Construct a fact only when the strategy owns a coherent descriptor for
    /// the given arity.
    #[must_use]
    pub fn resolve(strategy: OpStrategy, arity: usize) -> Option<Self> {
        let descriptor = strategy.descriptor(arity)?;
        descriptor.is_valid_for(arity).then_some(Self {
            strategy,
            descriptor,
        })
    }

    /// Verify that the frozen descriptor still equals the registry authority.
    #[must_use]
    pub fn is_valid_for(self, arity: usize) -> bool {
        self.descriptor.is_valid_for(arity)
            && self.strategy.descriptor(arity) == Some(self.descriptor)
    }
}

/// Identity-bearing primitive facts keyed by the stable destination variable.
///
/// Unlike COW annotations, this table participates in `ArcFunction` equality
/// and hashing. Physical instruction indices may change during realization;
/// the SSA destination remains stable.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct PrimitiveFacts {
    facts: BTreeMap<ArcVarId, PrimitiveFact>,
}

impl PrimitiveFacts {
    /// Insert one resolved fact. Returns the replaced fact if the destination
    /// was already present, which callers treat as duplicate-definition error.
    pub(crate) fn insert(&mut self, dst: ArcVarId, fact: PrimitiveFact) -> Option<PrimitiveFact> {
        self.facts.insert(dst, fact)
    }

    /// Read one frozen fact.
    #[must_use]
    pub fn get(&self, dst: ArcVarId) -> Option<PrimitiveFact> {
        self.facts.get(&dst).copied()
    }

    /// Number of frozen sites.
    #[must_use]
    pub fn len(&self) -> usize {
        self.facts.len()
    }

    /// Whether no sites are frozen.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    /// Iterate in stable destination order for validation and fingerprinting.
    pub fn iter(&self) -> impl Iterator<Item = (ArcVarId, PrimitiveFact)> + '_ {
        self.facts.iter().map(|(&dst, &fact)| (dst, fact))
    }

    /// Retain facts whose defining instructions survived a structural rewrite.
    pub(crate) fn retain(&mut self, mut keep: impl FnMut(ArcVarId) -> bool) {
        self.facts.retain(|&dst, _| keep(dst));
    }
}
