//! Rank type for let-polymorphism.
//!
//! Ranks track the depth of let-bindings to determine which type variables
//! can be generalized. Variables created at rank N can only be generalized
//! when exiting rank N.
//!
//! # Rank System (from Elm/Roc)
//!
//! - **Rank 0 (TOP)**: Universally quantified, always generalizable
//! - **Rank 1 (IMPORT)**: Imported from other modules
//! - **Rank 2+ (FIRST+)**: Created in nested let-scopes
//!
//! When a variable at rank N is unified with a type containing variables
//! at rank M > N, those variables must be promoted to rank N to prevent
//! them from escaping their scope.

/// Rank tracks the scope depth for let-polymorphism.
///
/// Higher ranks indicate deeper scopes. Variables at higher ranks
/// can be generalized when exiting their scope.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
#[repr(transparent)]
pub struct Rank(u16);

impl Rank {
    /// Top-level rank (can always be generalized).
    ///
    /// Used for type schemes that have already been generalized.
    pub const TOP: Self = Self(0);

    /// Import rank (always generalizable, from other modules).
    ///
    /// Type schemes from imports start at this rank.
    pub const IMPORT: Self = Self(1);

    /// First user rank (top-level definitions within a module).
    ///
    /// This is where type checking starts for module-level definitions.
    pub const FIRST: Self = Self(2);

    /// Maximum rank (prevents overflow in deeply nested code).
    pub const MAX: Self = Self(u16::MAX - 1);

    /// Create a rank from a raw value.
    #[inline]
    pub const fn from_raw(value: u16) -> Self {
        Self(value)
    }

    /// Get the raw rank value.
    #[inline]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Get the next (deeper) rank.
    ///
    /// Saturates at `MAX` to prevent overflow.
    #[inline]
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1).min(Self::MAX.0))
    }

    /// Get the previous (shallower) rank.
    ///
    /// Saturates at `TOP` to prevent underflow.
    #[inline]
    #[must_use]
    pub fn prev(self) -> Self {
        Self(self.0.saturating_sub(1))
    }

    /// Check if this variable can be generalized at the given rank.
    ///
    /// A variable at rank N can be generalized when exiting rank N.
    /// Variables at higher (deeper) ranks escape with generalization.
    #[inline]
    pub fn can_generalize_at(self, generalization_rank: Self) -> bool {
        self >= generalization_rank
    }

    /// Check if this rank represents a generalized (quantified) variable.
    #[inline]
    pub fn is_generalized(self) -> bool {
        self == Self::TOP
    }

    /// Check if this rank is at or above the first user rank.
    #[inline]
    pub fn is_user_level(self) -> bool {
        self >= Self::FIRST
    }

    /// Return the maximum of two ranks.
    #[inline]
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        if self >= other {
            self
        } else {
            other
        }
    }

    /// Return the minimum of two ranks.
    #[inline]
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        if self <= other {
            self
        } else {
            other
        }
    }
}

impl std::fmt::Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::TOP => write!(f, "TOP"),
            Self::IMPORT => write!(f, "IMPORT"),
            Self::FIRST => write!(f, "FIRST"),
            Self::MAX => write!(f, "MAX"),
            Self(n) => write!(f, "R{n}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_constants() {
        assert_eq!(Rank::TOP.raw(), 0);
        assert_eq!(Rank::IMPORT.raw(), 1);
        assert_eq!(Rank::FIRST.raw(), 2);
    }

    #[test]
    fn rank_ordering() {
        assert!(Rank::TOP < Rank::IMPORT);
        assert!(Rank::IMPORT < Rank::FIRST);
        assert!(Rank::FIRST < Rank::MAX);
    }

    #[test]
    fn rank_next_prev() {
        let r = Rank::FIRST;
        assert_eq!(r.next().raw(), 3);
        assert_eq!(r.prev().raw(), 1);
        assert_eq!(r.prev().prev().raw(), 0);

        // Saturates at TOP
        assert_eq!(Rank::TOP.prev(), Rank::TOP);

        // Saturates at MAX
        assert_eq!(Rank::MAX.next(), Rank::MAX);
    }

    #[test]
    fn can_generalize_at() {
        let r3 = Rank::from_raw(3);
        let r5 = Rank::from_raw(5);

        // Variable at rank 5 can be generalized at rank 3, 4, or 5
        assert!(r5.can_generalize_at(Rank::from_raw(3)));
        assert!(r5.can_generalize_at(Rank::from_raw(4)));
        assert!(r5.can_generalize_at(Rank::from_raw(5)));

        // Variable at rank 3 cannot be generalized at rank 5
        assert!(!r3.can_generalize_at(Rank::from_raw(5)));
    }

    #[test]
    fn is_generalized() {
        assert!(Rank::TOP.is_generalized());
        assert!(!Rank::IMPORT.is_generalized());
        assert!(!Rank::FIRST.is_generalized());
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", Rank::TOP), "TOP");
        assert_eq!(format!("{}", Rank::IMPORT), "IMPORT");
        assert_eq!(format!("{}", Rank::FIRST), "FIRST");
        assert_eq!(format!("{}", Rank::from_raw(7)), "R7");
    }
}
