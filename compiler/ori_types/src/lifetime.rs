//! Lifetime identifiers for future borrowed reference types.
//!
//! Reserved for future use — all current Ori values are owned/ARC'd
//! (`'static` equivalent). When borrowed views or slices are added,
//! this type will constrain how long a reference can live.
//!
//! See `proposals/approved/low-level-future-proofing-proposal.md`.

/// Lifetime identifier for future borrowed reference types.
///
/// Currently unused — all values are owned/ARC'd (implicitly `STATIC`).
/// Reserved so that adding borrowed views later is an incremental change
/// rather than a type system redesign.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct LifetimeId(u32);

impl LifetimeId {
    /// The static lifetime — owned values, no borrowing.
    ///
    /// All current Ori types implicitly have this lifetime.
    pub const STATIC: Self = Self(0);

    /// Reserved for future: lifetime bound to current scope.
    ///
    /// A `SCOPED` reference cannot escape the block in which it was created.
    pub const SCOPED: Self = Self(1);

    /// Get the raw identifier value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Construct from a raw identifier value.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Check if this is the static (owned) lifetime.
    #[inline]
    pub const fn is_static(self) -> bool {
        self.0 == Self::STATIC.0
    }
}

impl std::fmt::Display for LifetimeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            0 => write!(f, "'static"),
            1 => write!(f, "'scoped"),
            n => write!(f, "'{n}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_lifetime_is_zero() {
        assert_eq!(LifetimeId::STATIC.raw(), 0);
    }

    #[test]
    fn scoped_lifetime_is_one() {
        assert_eq!(LifetimeId::SCOPED.raw(), 1);
    }

    #[test]
    fn is_static_works() {
        assert!(LifetimeId::STATIC.is_static());
        assert!(!LifetimeId::SCOPED.is_static());
        assert!(!LifetimeId::from_raw(42).is_static());
    }

    #[test]
    fn roundtrip_raw() {
        let lt = LifetimeId::from_raw(42);
        assert_eq!(lt.raw(), 42);
    }

    #[test]
    fn display_named_lifetimes() {
        assert_eq!(LifetimeId::STATIC.to_string(), "'static");
        assert_eq!(LifetimeId::SCOPED.to_string(), "'scoped");
        assert_eq!(LifetimeId::from_raw(5).to_string(), "'5");
    }

    #[test]
    fn equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(LifetimeId::STATIC);
        set.insert(LifetimeId::SCOPED);
        set.insert(LifetimeId::STATIC); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn size_is_4_bytes() {
        assert_eq!(std::mem::size_of::<LifetimeId>(), 4);
    }
}
