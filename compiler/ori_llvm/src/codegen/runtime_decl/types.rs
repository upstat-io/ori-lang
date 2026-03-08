//! Type and attribute descriptors for runtime function declarations.

/// Primitive type descriptor for runtime function signatures.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Ty {
    I64,
    I32,
    I8,
    F64,
    Bool,
    Ptr,
    /// `{i64, i64, ptr}` — Ori string representation (len, cap, data).
    Str,
    /// `{i64, i64, ptr}` — Ori list/set representation.
    List,
    /// `{i64, i64, ptr}` — Ori map representation (len, cap, data).
    Map,
    /// `{i32, i64}` — char iteration result `{codepoint, next_offset}`.
    CharResult,
}

impl Ty {
    /// Whether this return type exceeds the x86-64 `SysV` ABI register return
    /// threshold (16 bytes) and must use `sret` convention.
    ///
    /// `Str` (24 bytes), `List` (24 bytes), and `Map` (24 bytes) all exceed it.
    /// `CharResult` (16 bytes) fits exactly and uses direct return.
    pub(crate) const fn needs_sret(self) -> bool {
        matches!(self, Self::Str | Self::List | Self::Map)
    }
}

/// Function attribute applied after declaration.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Attr {
    Nounwind,
    Noreturn,
    Cold,
    NoaliasReturn,
    MemArgmemRW,
}

/// Runtime function specification: name, signature, and attributes.
#[derive(Debug)]
pub(crate) struct RtFn {
    pub(crate) name: &'static str,
    pub(crate) params: &'static [Ty],
    pub(crate) ret: Option<Ty>,
    pub(crate) attrs: &'static [Attr],
    /// Whether this function is registered for JIT use via `LLVMAddSymbol`.
    ///
    /// `true` = available in both JIT and AOT. `false` = AOT-only.
    /// This is the single source of truth — `jit_symbol_mappings()` derives
    /// its list from entries where `jit_allowed == true`.
    pub(crate) jit_allowed: bool,
}
