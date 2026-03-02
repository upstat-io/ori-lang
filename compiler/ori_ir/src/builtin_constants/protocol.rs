//! Compiler-internal protocol functions emitted by ARC lowering.
//!
//! These names appear as `Apply` callees in ARC IR but are intercepted by the
//! LLVM emitter — they never become real function calls. Each variant carries
//! per-argument ownership semantics so borrow inference and RC insertion handle
//! them correctly without falling through to the "unknown callee → all Owned"
//! default.

/// Compiler-internal protocol functions emitted by ARC lowering / canonicalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProtocolBuiltin {
    /// `receiver[index]` — list/map indexing. Both args borrowed.
    Index,
    /// Iterator advancement. Iterator owned (consumed), type marker borrowed.
    IterNext,
    /// Set collection from iterator. Iterator owned (consumed).
    CollectSet,
}

/// Argument ownership for protocol builtin parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolArgOwnership {
    Owned,
    Borrowed,
}

impl ProtocolBuiltin {
    /// All protocol builtins. Exhaustive — add new variants here.
    pub const ALL: &[Self] = &[Self::Index, Self::IterNext, Self::CollectSet];

    /// The internal name emitted in ARC IR.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Index => "__index",
            Self::IterNext => "__iter_next",
            Self::CollectSet => "__collect_set",
        }
    }

    /// Reverse lookup from interned name string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "__index" => Some(Self::Index),
            "__iter_next" => Some(Self::IterNext),
            "__collect_set" => Some(Self::CollectSet),
            _ => None,
        }
    }

    /// Expected argument count.
    pub const fn arg_count(self) -> usize {
        match self {
            Self::Index | Self::IterNext => 2,
            Self::CollectSet => 1,
        }
    }

    /// Per-argument ownership semantics.
    ///
    /// This is the source of truth for how borrow inference should treat
    /// each argument position. Avoids the "unknown callee → all Owned"
    /// fallthrough that caused the `__index` RC leak.
    pub const fn arg_ownership(self) -> &'static [ProtocolArgOwnership] {
        match self {
            Self::Index => &[
                ProtocolArgOwnership::Borrowed,
                ProtocolArgOwnership::Borrowed,
            ],
            Self::IterNext => &[ProtocolArgOwnership::Owned, ProtocolArgOwnership::Borrowed],
            Self::CollectSet => &[ProtocolArgOwnership::Owned],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_covered() {
        assert_eq!(ProtocolBuiltin::ALL.len(), 3);
        for &pb in ProtocolBuiltin::ALL {
            assert!(!pb.name().is_empty());
            assert!(ProtocolBuiltin::from_name(pb.name()) == Some(pb));
            assert_eq!(pb.arg_ownership().len(), pb.arg_count());
        }
    }

    #[test]
    fn from_name_returns_none_for_unknown() {
        assert!(ProtocolBuiltin::from_name("unknown").is_none());
        assert!(ProtocolBuiltin::from_name("ori_print").is_none());
    }

    #[test]
    fn index_ownership_is_all_borrowed() {
        let ownership = ProtocolBuiltin::Index.arg_ownership();
        assert!(ownership
            .iter()
            .all(|o| *o == ProtocolArgOwnership::Borrowed));
    }

    #[test]
    fn iter_next_ownership_is_owned_borrowed() {
        let ownership = ProtocolBuiltin::IterNext.arg_ownership();
        assert_eq!(ownership[0], ProtocolArgOwnership::Owned);
        assert_eq!(ownership[1], ProtocolArgOwnership::Borrowed);
    }

    #[test]
    fn collect_set_ownership_is_owned() {
        let ownership = ProtocolBuiltin::CollectSet.arg_ownership();
        assert_eq!(ownership[0], ProtocolArgOwnership::Owned);
    }
}
