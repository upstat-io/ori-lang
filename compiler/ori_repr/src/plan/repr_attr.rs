//! `#repr` attribute types for user-specified layout control.
//!
//! These attributes gate optimization passes: `#repr("c")` prevents
//! field reordering, `#repr("packed")` disables padding, etc.
//! Consumed by struct layout and enum repr passes.

/// User-specified representation attribute (`#repr(...)`).
///
/// Stored in `ReprPlan::repr_attrs` per struct/enum type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReprAttribute {
    /// Default layout — all optimizations permitted.
    Default,
    /// C-compatible field ordering (no reordering).
    C,
    /// No padding between fields.
    Packed,
    /// Same layout as single field (zero-cost newtype).
    Transparent,
    /// Minimum alignment (power of two).
    Aligned(u32),
    /// C-compatible with minimum alignment.
    CAligned(u32),
}
