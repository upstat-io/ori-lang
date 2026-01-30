//! Expression IDs and ranges for flat AST.
//!
//! Per design spec A-data-structuresmd:
//! - `ExprId(u32)` instead of `Box<Expr>` for 50% memory savings
//! - `ExprRange` for argument lists (6 bytes vs 24+ for Vec)
//! - All Salsa-required traits

use std::fmt;
use std::hash::{Hash, Hasher};

/// Index into expression arena.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
///
/// # Design
/// Per design: "No `Box<Expr>`, use `ExprId(u32)` indices"
/// - Memory: 4 bytes (vs 8 bytes for Box)
/// - Equality: O(1) integer compare
/// - Cache locality: indices into contiguous array
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct ExprId(u32);

impl ExprId {
    /// Invalid expression ID (sentinel value).
    pub const INVALID: ExprId = ExprId(u32::MAX);

    /// Create a new `ExprId`.
    #[inline]
    pub const fn new(index: u32) -> Self {
        ExprId(index)
    }

    /// Get the index into the arena.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Get the raw u32 value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Check if this is a valid ID.
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

impl Hash for ExprId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for ExprId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "ExprId({})", self.0)
        } else {
            write!(f, "ExprId::INVALID")
        }
    }
}

impl Default for ExprId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Range of expressions in flattened list.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
///
/// # Design
/// Per design spec: uses (start: u32, len: u16) = 6 bytes logical.
/// Rust aligns to 8 bytes, still 3x better than `Vec<ExprId>` at 24+ bytes.
/// - start: u32 (4 bytes) - start index in `expr_lists`
/// - len: u16 (2 bytes) - number of expressions
/// - padding: 2 bytes (Rust alignment)
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct ExprRange {
    pub start: u32,
    pub len: u16,
}

impl ExprRange {
    /// Empty range.
    pub const EMPTY: ExprRange = ExprRange { start: 0, len: 0 };

    /// Create a new range.
    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ExprRange { start, len }
    }

    /// Check if the range is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the number of expressions.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Iterator over indices in this range.
    #[inline]
    pub fn indices(&self) -> impl Iterator<Item = u32> {
        self.start..(self.start + u32::from(self.len))
    }
}

impl fmt::Debug for ExprRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExprRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

impl Default for ExprRange {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// Index into statement list.
///
/// Similar to `ExprId` but for statements.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct StmtId(u32);

impl StmtId {
    pub const INVALID: StmtId = StmtId(u32::MAX);

    #[inline]
    pub const fn new(index: u32) -> Self {
        StmtId(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

impl fmt::Debug for StmtId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "StmtId({})", self.0)
        } else {
            write!(f, "StmtId::INVALID")
        }
    }
}

impl Default for StmtId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Range of statements.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct StmtRange {
    pub start: u32,
    pub len: u16,
}

impl StmtRange {
    pub const EMPTY: StmtRange = StmtRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        StmtRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for StmtRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StmtRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Index into parsed type storage in arena.
///
/// Used to replace `Box<ParsedType>` with arena allocation for better
/// cache locality and reduced per-allocation overhead.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct ParsedTypeId(u32);

impl ParsedTypeId {
    /// Invalid parsed type ID (sentinel value).
    pub const INVALID: ParsedTypeId = ParsedTypeId(u32::MAX);

    /// Create a new `ParsedTypeId`.
    #[inline]
    pub const fn new(index: u32) -> Self {
        ParsedTypeId(index)
    }

    /// Get the index into the arena.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Get the raw u32 value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Check if this is a valid ID.
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

impl Hash for ParsedTypeId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for ParsedTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "ParsedTypeId({})", self.0)
        } else {
            write!(f, "ParsedTypeId::INVALID")
        }
    }
}

impl Default for ParsedTypeId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Range of parsed types in flattened list.
///
/// Used for type argument lists like `Result<T, E>` → range of 2 types.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct ParsedTypeRange {
    pub start: u32,
    pub len: u16,
}

impl ParsedTypeRange {
    /// Empty range.
    pub const EMPTY: ParsedTypeRange = ParsedTypeRange { start: 0, len: 0 };

    /// Create a new range.
    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ParsedTypeRange { start, len }
    }

    /// Check if the range is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the number of types.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for ParsedTypeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ParsedTypeRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Index into match pattern storage in arena.
///
/// Used to replace `Box<MatchPattern>` with arena allocation for better
/// cache locality and reduced per-allocation overhead.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct MatchPatternId(u32);

impl MatchPatternId {
    /// Invalid match pattern ID (sentinel value).
    pub const INVALID: MatchPatternId = MatchPatternId(u32::MAX);

    /// Create a new `MatchPatternId`.
    #[inline]
    pub const fn new(index: u32) -> Self {
        MatchPatternId(index)
    }

    /// Get the index into the arena.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Get the raw u32 value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Check if this is a valid ID.
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

impl Hash for MatchPatternId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for MatchPatternId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "MatchPatternId({})", self.0)
        } else {
            write!(f, "MatchPatternId::INVALID")
        }
    }
}

impl Default for MatchPatternId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Range of match patterns in flattened list.
///
/// Used for pattern lists like `(a, b, c)` → range of 3 patterns.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct MatchPatternRange {
    pub start: u32,
    pub len: u16,
}

impl MatchPatternRange {
    /// Empty range.
    pub const EMPTY: MatchPatternRange = MatchPatternRange { start: 0, len: 0 };

    /// Create a new range.
    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        MatchPatternRange { start, len }
    }

    /// Check if the range is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the number of patterns.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for MatchPatternRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MatchPatternRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_id_valid() {
        let id = ExprId::new(42);
        assert!(id.is_valid());
        assert_eq!(id.index(), 42);
    }

    #[test]
    fn test_expr_id_invalid() {
        assert!(!ExprId::INVALID.is_valid());
        assert!(!ExprId::default().is_valid());
    }

    #[test]
    fn test_expr_range() {
        let range = ExprRange::new(10, 5);
        assert!(!range.is_empty());
        assert_eq!(range.len(), 5);
        let indices: Vec<_> = range.indices().collect();
        assert_eq!(indices, vec![10, 11, 12, 13, 14]);
    }

    #[test]
    fn test_expr_range_empty() {
        assert!(ExprRange::EMPTY.is_empty());
        assert!(ExprRange::default().is_empty());
    }

    #[test]
    fn test_expr_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ExprId::new(1));
        set.insert(ExprId::new(1)); // duplicate
        set.insert(ExprId::new(2));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_memory_size() {
        // ExprId: 4 bytes (u32)
        assert_eq!(std::mem::size_of::<ExprId>(), 4);

        // ExprRange: Design spec says 6 bytes (u32 + u16), but Rust aligns
        // to 8 bytes due to u32 alignment requirements. Still better than
        // Vec<ExprId> at 24+ bytes.
        assert_eq!(std::mem::size_of::<ExprRange>(), 8);
    }

    #[test]
    fn test_parsed_type_id_valid() {
        let id = ParsedTypeId::new(42);
        assert!(id.is_valid());
        assert_eq!(id.index(), 42);
        assert_eq!(id.raw(), 42);
    }

    #[test]
    fn test_parsed_type_id_invalid() {
        assert!(!ParsedTypeId::INVALID.is_valid());
        assert!(!ParsedTypeId::default().is_valid());
    }

    #[test]
    fn test_parsed_type_id_debug() {
        let valid = ParsedTypeId::new(10);
        assert_eq!(format!("{:?}", valid), "ParsedTypeId(10)");
        assert_eq!(
            format!("{:?}", ParsedTypeId::INVALID),
            "ParsedTypeId::INVALID"
        );
    }

    #[test]
    fn test_parsed_type_range() {
        let range = ParsedTypeRange::new(5, 3);
        assert!(!range.is_empty());
        assert_eq!(range.len(), 3);
    }

    #[test]
    fn test_parsed_type_range_empty() {
        assert!(ParsedTypeRange::EMPTY.is_empty());
        assert!(ParsedTypeRange::default().is_empty());
    }

    #[test]
    fn test_match_pattern_id_valid() {
        let id = MatchPatternId::new(100);
        assert!(id.is_valid());
        assert_eq!(id.index(), 100);
        assert_eq!(id.raw(), 100);
    }

    #[test]
    fn test_match_pattern_id_invalid() {
        assert!(!MatchPatternId::INVALID.is_valid());
        assert!(!MatchPatternId::default().is_valid());
    }

    #[test]
    fn test_match_pattern_id_debug() {
        let valid = MatchPatternId::new(20);
        assert_eq!(format!("{:?}", valid), "MatchPatternId(20)");
        assert_eq!(
            format!("{:?}", MatchPatternId::INVALID),
            "MatchPatternId::INVALID"
        );
    }

    #[test]
    fn test_match_pattern_range() {
        let range = MatchPatternRange::new(0, 5);
        assert!(!range.is_empty());
        assert_eq!(range.len(), 5);
    }

    #[test]
    fn test_match_pattern_range_empty() {
        assert!(MatchPatternRange::EMPTY.is_empty());
        assert!(MatchPatternRange::default().is_empty());
    }

    #[test]
    fn test_parsed_type_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ParsedTypeId::new(1));
        set.insert(ParsedTypeId::new(1)); // duplicate
        set.insert(ParsedTypeId::new(2));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_match_pattern_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(MatchPatternId::new(1));
        set.insert(MatchPatternId::new(1)); // duplicate
        set.insert(MatchPatternId::new(2));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_new_id_memory_sizes() {
        // ParsedTypeId and MatchPatternId: 4 bytes each
        assert_eq!(std::mem::size_of::<ParsedTypeId>(), 4);
        assert_eq!(std::mem::size_of::<MatchPatternId>(), 4);

        // Range types: 8 bytes each (with padding)
        assert_eq!(std::mem::size_of::<ParsedTypeRange>(), 8);
        assert_eq!(std::mem::size_of::<MatchPatternRange>(), 8);
    }
}
