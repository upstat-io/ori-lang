//! Runtime ABI carrier types and discriminants.

// Why: The standalone runtime cannot depend on `ori_ir`; tests pin these duplicate discriminants.
pub(crate) const OPTION_TAG_SOME: i64 = 0;
pub(crate) const OPTION_TAG_NONE: i64 = 1;

/// Ori Option representation: `{ i8 tag, T value }`.
#[derive(Debug)]
#[repr(C)]
pub struct OriOption<T> {
    /// Active variant tag.
    pub tag: i8,
    /// Payload storage for the active variant.
    pub value: T,
}

/// Ori Result representation: `{ i8 tag, T value }`.
#[derive(Debug)]
#[repr(C)]
pub struct OriResult<T> {
    /// Active variant tag.
    pub tag: i8,
    /// Payload storage for the active variant.
    pub value: T,
}
