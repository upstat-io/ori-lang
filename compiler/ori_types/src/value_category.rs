//! Value category for types — determines memory representation and semantics.
//!
//! Reserved for future use — all current Ori compound types are `Boxed`
//! (heap-allocated with ARC). When inline types or borrowed views are added,
//! this enum will distinguish their memory semantics.
//!
//! See `proposals/approved/low-level-future-proofing-proposal.md`.

/// Value category for a type — determines memory representation and semantics.
///
/// Currently all compound types are `Boxed` (heap-allocated, ARC-managed).
/// The `Inline` and `View` variants are reserved for future low-level features.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, Default)]
pub enum ValueCategory {
    /// Heap-allocated with ARC (current default for all compound types).
    #[default]
    Boxed,

    /// Stack-allocated, copied on assignment (future: `inline type`).
    ///
    /// Reserved — not yet implemented. When active, values of this category
    /// will be passed by copy rather than by reference count.
    Inline,

    /// Borrowed view, cannot outlive source (future: `Slice<T>`).
    ///
    /// Reserved — not yet implemented. When active, values of this category
    /// will carry lifetime constraints preventing escape from their source scope.
    View,
}

impl ValueCategory {
    /// Check if this is the default boxed (ARC-managed) category.
    #[inline]
    pub const fn is_boxed(self) -> bool {
        matches!(self, Self::Boxed)
    }

    /// Check if this is an inline (stack-allocated) category.
    #[inline]
    pub const fn is_inline(self) -> bool {
        matches!(self, Self::Inline)
    }

    /// Check if this is a view (borrowed) category.
    #[inline]
    pub const fn is_view(self) -> bool {
        matches!(self, Self::View)
    }

    /// Get a human-readable name for this category.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Boxed => "boxed",
            Self::Inline => "inline",
            Self::View => "view",
        }
    }
}

impl std::fmt::Display for ValueCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_boxed() {
        assert_eq!(ValueCategory::default(), ValueCategory::Boxed);
    }

    #[test]
    fn predicates_work() {
        assert!(ValueCategory::Boxed.is_boxed());
        assert!(!ValueCategory::Boxed.is_inline());
        assert!(!ValueCategory::Boxed.is_view());

        assert!(!ValueCategory::Inline.is_boxed());
        assert!(ValueCategory::Inline.is_inline());
        assert!(!ValueCategory::Inline.is_view());

        assert!(!ValueCategory::View.is_boxed());
        assert!(!ValueCategory::View.is_inline());
        assert!(ValueCategory::View.is_view());
    }

    #[test]
    fn display_names() {
        assert_eq!(ValueCategory::Boxed.to_string(), "boxed");
        assert_eq!(ValueCategory::Inline.to_string(), "inline");
        assert_eq!(ValueCategory::View.to_string(), "view");
    }

    #[test]
    fn size_is_1_byte() {
        assert_eq!(std::mem::size_of::<ValueCategory>(), 1);
    }

    #[test]
    fn equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ValueCategory::Boxed);
        set.insert(ValueCategory::Inline);
        set.insert(ValueCategory::View);
        set.insert(ValueCategory::Boxed); // duplicate
        assert_eq!(set.len(), 3);
    }
}
