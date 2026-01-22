//! Compact ID types for referencing AST nodes.

use std::fmt;

/// Index into expression arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ExprId(u32);

impl ExprId {
    pub const INVALID: ExprId = ExprId(u32::MAX);

    #[inline]
    pub const fn new(index: u32) -> Self {
        ExprId(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
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

/// Range of expressions in flattened list.
///
/// Layout: 6 bytes total
/// - start: u32 (4 bytes) - start index in expr_lists
/// - len: u16 (2 bytes) - number of expressions
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct ExprRange {
    pub start: u32,
    pub len: u16,
}

impl ExprRange {
    pub const EMPTY: ExprRange = ExprRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ExprRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn indices(&self) -> impl Iterator<Item = u32> {
        self.start..(self.start + self.len as u32)
    }
}

impl fmt::Debug for ExprRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExprRange({}..+{})", self.start, self.len)
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
}

impl fmt::Debug for StmtRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StmtRange({}..+{})", self.start, self.len)
    }
}

/// Range of match arms.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct ArmRange {
    pub start: u32,
    pub len: u16,
}

impl ArmRange {
    pub const EMPTY: ArmRange = ArmRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ArmRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Debug for ArmRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArmRange({}..+{})", self.start, self.len)
    }
}

/// Range of parameters.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct ParamRange {
    pub start: u32,
    pub len: u16,
}

impl ParamRange {
    pub const EMPTY: ParamRange = ParamRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ParamRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Debug for ParamRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ParamRange({}..+{})", self.start, self.len)
    }
}

/// Range of map entries.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct MapEntryRange {
    pub start: u32,
    pub len: u16,
}

impl MapEntryRange {
    pub const EMPTY: MapEntryRange = MapEntryRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        MapEntryRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Debug for MapEntryRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MapEntryRange({}..+{})", self.start, self.len)
    }
}

/// Range of field initializers.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct FieldInitRange {
    pub start: u32,
    pub len: u16,
}

impl FieldInitRange {
    pub const EMPTY: FieldInitRange = FieldInitRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        FieldInitRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Debug for FieldInitRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FieldInitRange({}..+{})", self.start, self.len)
    }
}

/// Index into pattern arguments array.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct PatternArgsId(u32);

impl PatternArgsId {
    pub const INVALID: PatternArgsId = PatternArgsId(u32::MAX);

    #[inline]
    pub const fn new(index: u32) -> Self {
        PatternArgsId(index)
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

impl fmt::Debug for PatternArgsId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "PatternArgsId({})", self.0)
        } else {
            write!(f, "PatternArgsId::INVALID")
        }
    }
}

/// Index into type expressions.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct TypeExprId(u32);

impl TypeExprId {
    pub const INVALID: TypeExprId = TypeExprId(u32::MAX);

    #[inline]
    pub const fn new(index: u32) -> Self {
        TypeExprId(index)
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

impl fmt::Debug for TypeExprId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "TypeExprId({})", self.0)
        } else {
            write!(f, "TypeExprId::INVALID")
        }
    }
}
