//! Function-level effect summaries and FIP certification.

/// Effect summary for a complete function body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "The flags are independent effect dimensions."
)]
pub struct EffectSummary {
    /// May the function allocate on any code path?
    pub may_allocate: bool,
    /// Allocations occur only on slow paths guarded by uniqueness checks.
    pub alloc_only_on_slow_path: bool,
    /// May the function deallocate on any code path?
    pub may_deallocate: bool,
    /// May the function create shared references?
    pub may_share: bool,
    /// May the function throw exceptions or panic?
    pub may_throw: bool,
    /// Does the function have unbounded stack growth?
    pub has_unbounded_stack: bool,
}

impl EffectSummary {
    /// Conservative summary for an unknown function.
    pub const CONSERVATIVE: Self = Self {
        may_allocate: true,
        alloc_only_on_slow_path: false,
        may_deallocate: true,
        may_share: true,
        may_throw: true,
        has_unbounded_stack: true,
    };

    /// Most-optimistic summary for fixed-point initialization.
    pub const OPTIMISTIC: Self = Self {
        may_allocate: false,
        alloc_only_on_slow_path: false,
        may_deallocate: false,
        may_share: false,
        may_throw: false,
        has_unbounded_stack: false,
    };

    /// Join two summaries componentwise.
    #[must_use]
    pub(crate) fn join(self, other: Self) -> Self {
        Self {
            may_allocate: self.may_allocate || other.may_allocate,
            alloc_only_on_slow_path: self.alloc_only_on_slow_path && other.alloc_only_on_slow_path,
            may_deallocate: self.may_deallocate || other.may_deallocate,
            may_share: self.may_share || other.may_share,
            may_throw: self.may_throw || other.may_throw,
            has_unbounded_stack: self.has_unbounded_stack || other.has_unbounded_stack,
        }
    }
}

/// Backend-neutral whole-function memory-access class derived by AIMS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryAccessClass {
    /// The function may read any memory but does not write memory.
    ReadOnly,
    /// The function may write memory or lacks a no-write proof.
    ReadWrite,
}

/// Proof that a function returns its own fresh allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreshSelfAllocationFacts {
    returns_fresh_self_alloc: bool,
}

impl FreshSelfAllocationFacts {
    /// Construct a proof carrier from the converged return contract.
    pub(super) const fn new(returns_fresh_self_alloc: bool) -> Self {
        Self {
            returns_fresh_self_alloc,
        }
    }

    /// Return whether every path yields fresh storage with no input alias.
    #[must_use]
    pub const fn is_proven(self) -> bool {
        self.returns_fresh_self_alloc
    }
}

/// Final backend-neutral effect facts for one realized function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionEffectFacts {
    effects: EffectSummary,
    may_write_inaccessible: bool,
    memory_access: MemoryAccessClass,
}

impl FunctionEffectFacts {
    /// Construct the final effect carrier from converged AIMS facts.
    pub(super) const fn new(
        effects: EffectSummary,
        may_write_inaccessible: bool,
        memory_access: MemoryAccessClass,
    ) -> Self {
        Self {
            effects,
            may_write_inaccessible,
            memory_access,
        }
    }

    /// Return the final IC-5 effect summary.
    #[must_use]
    pub const fn effects(self) -> EffectSummary {
        self.effects
    }

    /// Return whether an untyped operation may write non-argument memory.
    #[must_use]
    pub const fn may_write_inaccessible(self) -> bool {
        self.may_write_inaccessible
    }

    /// Return the final RL-30 memory-access classification.
    #[must_use]
    pub const fn memory_access(self) -> MemoryAccessClass {
        self.memory_access
    }
}

/// FIP certification status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FipContract {
    /// Function cannot be certified FIP.
    Never,
    /// Function is FIP when the specified parameters are unique.
    Conditional {
        /// Parameters that must be unique for certification.
        requires_unique_params: Vec<bool>,
    },
    /// Function is unconditionally FIP.
    Certified,
    /// Function allocates at most the given constructors beyond its reuses.
    Bounded(u16),
}

impl FipContract {
    /// Join two certifications, weakening toward [`Self::Never`].
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Never, _) | (_, Self::Never) => Self::Never,
            (
                Self::Conditional {
                    requires_unique_params: left,
                },
                Self::Conditional {
                    requires_unique_params: right,
                },
            ) => {
                assert_eq!(
                    left.len(),
                    right.len(),
                    "FipContract param counts must match"
                );
                Self::Conditional {
                    requires_unique_params: left
                        .iter()
                        .zip(right.iter())
                        .map(|(left, right)| *left || *right)
                        .collect(),
                }
            }
            (Self::Conditional { .. }, Self::Certified | Self::Bounded(_))
            | (Self::Bounded(_), Self::Certified) => self.clone(),
            (Self::Certified | Self::Bounded(_), Self::Conditional { .. })
            | (Self::Certified, Self::Bounded(_)) => other.clone(),
            (Self::Bounded(left), Self::Bounded(right)) => Self::Bounded((*left).max(*right)),
            (Self::Certified, Self::Certified) => Self::Certified,
        }
    }
}
