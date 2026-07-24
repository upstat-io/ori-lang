//! Exact aggregate-reconstruction ownership transfer carried through SCC
//! convergence.

#![expect(
    clippy::disallowed_types,
    reason = "Contract proofs are immutable, thread-safe compiler metadata; \
        structural Arc sharing is the chosen direct carrier and is unrelated \
        to runtime Ori value storage."
)]

use std::sync::Arc;

/// How one projected field reaches its matching reconstruction position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExactFieldTransferKind {
    /// The projection enters the reconstruction directly.
    DirectMove,
    /// One effectively-owned call consumes the projection and returns its
    /// replacement owner.
    EffectiveOwnedRelay,
}

/// One top-level semantic projection from the aggregate root.
///
/// The PV-6 grain deliberately excludes arbitrary-depth paths. Nested
/// aggregates publish a transfer for the owned top-level member rather than
/// extending this caller-boundary coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExactFieldPath(u32);

impl ExactFieldPath {
    /// Construct exactly one top-level projection hop.
    #[must_use]
    pub fn new(path: impl IntoIterator<Item = u32>) -> Option<Self> {
        let mut path = path.into_iter();
        let field = path.next()?;
        path.next().is_none().then_some(Self(field))
    }

    /// Construct a one-hop projection path.
    #[must_use]
    pub fn single(field: u32) -> Self {
        Self(field)
    }

    /// Top-level field index.
    #[must_use]
    pub fn index(self) -> u32 {
        self.0
    }
}

/// One field and the ownership-preserving route into its reconstruction.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExactFieldTransfer {
    /// Typed semantic path of the transferred field.
    pub path: ExactFieldPath,
    /// Transfer route admitted by the canonical recognizer.
    pub kind: ExactFieldTransferKind,
}

/// Residual aggregate disposition after moving the listed fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResidualDisposition {
    /// Every owned field is represented exactly once in the reconstruction.
    FullyReconstructed,
}

/// Cleanup authority required before exact transfer can be published.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CleanupAuthority {
    /// Neither the container nor a transferred member has unsupported drop
    /// behavior.
    OrdinaryCleanupProven,
}

/// Immutable structural proof projected into the caller-visible contract.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExactAggregateTransfer {
    fields: Box<[ExactFieldTransfer]>,
    /// Disposition of the source aggregate after field transfer.
    pub residual: ResidualDisposition,
    /// Destructor authority checked by the producer.
    pub cleanup: CleanupAuthority,
}

impl ExactAggregateTransfer {
    /// Build a canonical transfer proof.
    ///
    /// Sorting makes field order irrelevant. Duplicate paths are rejected
    /// because one ownership credit cannot fund two reconstruction positions.
    #[must_use]
    pub fn new(
        mut fields: Vec<ExactFieldTransfer>,
        residual: ResidualDisposition,
        cleanup: CleanupAuthority,
    ) -> Option<Self> {
        fields.sort_unstable();
        if fields.is_empty() || fields.windows(2).any(|pair| pair[0].path == pair[1].path) {
            return None;
        }
        Some(Self {
            fields: fields.into_boxed_slice(),
            residual,
            cleanup,
        })
    }

    /// Canonically ordered field transfers.
    #[must_use]
    pub fn fields(&self) -> &[ExactFieldTransfer] {
        &self.fields
    }
}

/// Flat finite-height lattice for exact aggregate transfer.
///
/// `Optimistic` is bottom, each distinct `Exact` proof is an incomparable
/// atom, and `Unproven` is top. Therefore each parameter has at most two
/// strict rises regardless of the number of fields in a proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExactTransferState {
    /// SCC initialization before any body-derived fact is observed.
    Optimistic,
    /// One structurally exact reconstruction proof.
    Exact(Arc<ExactAggregateTransfer>),
    /// Conservative result after absence, disagreement, or unsafe cleanup.
    Unproven,
}

impl ExactTransferState {
    /// Wrap one immutable exact proof as a lattice atom.
    #[must_use]
    pub fn exact(transfer: ExactAggregateTransfer) -> Self {
        Self::Exact(Arc::new(transfer))
    }

    /// Least upper bound in the flat exact-transfer lattice.
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Optimistic, state) | (state, Self::Optimistic) => state.clone(),
            (Self::Exact(left), Self::Exact(right)) if left == right => {
                Self::Exact(Arc::clone(left))
            }
            (Self::Unproven, _) | (_, Self::Unproven) | (Self::Exact(_), Self::Exact(_)) => {
                Self::Unproven
            }
        }
    }
}
