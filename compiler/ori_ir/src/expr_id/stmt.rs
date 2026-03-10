//! Statement ID and range types.

use std::fmt;

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
