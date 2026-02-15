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
mod tests;
