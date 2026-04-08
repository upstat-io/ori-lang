//! Compiler-internal protocol functions emitted by ARC lowering.
//!
//! These names appear as `Apply` callees in ARC IR. Most are intercepted by
//! the LLVM emitter (they never become real function calls), while `Iter` and
//! `IterDrop` go through normal function dispatch — see [`ProtocolBuiltin::is_intercepted()`].
//! Each variant carries per-argument ownership semantics so borrow inference
//! and RC insertion handle them correctly without falling through to the
//! "unknown callee → all Owned" default.

/// Compiler-internal protocol functions emitted by ARC lowering / canonicalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProtocolBuiltin {
    /// `receiver[index]` — list/map indexing. Both args borrowed.
    Index,
    /// Iterator creation from collection. Collection borrowed (not consumed).
    ///
    /// Emitted by `for...in` lowering as `Apply @iter(collection)`. The
    /// iterator holds a reference to the collection's buffer; the collection
    /// itself is not consumed. Without this entry, the "unknown callee →
    /// all Owned" fallthrough promotes the collection parameter to Owned,
    /// causing ABI mismatch for `@main(args: [str])`.
    Iter,
    /// Iterator advancement. Iterator owned (consumed), type marker borrowed.
    IterNext,
    /// Iterator cleanup. Iterator state borrowed (freed internally).
    IterDrop,
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
    pub const ALL: &[Self] = &[
        Self::Index,
        Self::Iter,
        Self::IterNext,
        Self::IterDrop,
        Self::CollectSet,
    ];

    /// The internal name emitted in ARC IR.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Index => "__index",
            Self::Iter => "iter",
            Self::IterNext => "__iter_next",
            Self::IterDrop => "ori_iter_drop",
            Self::CollectSet => "__collect_set",
        }
    }

    /// Reverse lookup from interned name string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "__index" => Some(Self::Index),
            "iter" => Some(Self::Iter),
            "__iter_next" => Some(Self::IterNext),
            "ori_iter_drop" => Some(Self::IterDrop),
            "__collect_set" => Some(Self::CollectSet),
            _ => None,
        }
    }

    /// Whether this protocol is intercepted by `try_emit_protocol()` in LLVM codegen.
    ///
    /// `Iter` and `IterDrop` are registered as `ProtocolBuiltin` for borrow
    /// inference only — they go through normal function dispatch (real runtime
    /// calls), not protocol intercept.
    pub const fn is_intercepted(self) -> bool {
        match self {
            Self::Iter | Self::IterDrop => false,
            Self::Index | Self::IterNext | Self::CollectSet => true,
        }
    }

    /// Expected argument count.
    pub const fn arg_count(self) -> usize {
        match self {
            Self::Index | Self::IterNext => 2,
            Self::Iter | Self::IterDrop | Self::CollectSet => 1,
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
            Self::Iter => &[ProtocolArgOwnership::Borrowed],
            // `ori_iter_drop` consumes the iterator handle
            // (frees the Box-allocated state), just like `__collect_set`
            // consumes the iterator it drains. Marking `Owned` tells
            // ARC's borrow inference that the call is a consumption
            // event so no subsequent scope-exit `RcDec` is inserted
            // for the same iterator variable (which would double-free
            // now that iterators are non-trivial).
            Self::IterDrop | Self::CollectSet => &[ProtocolArgOwnership::Owned],
            Self::IterNext => &[ProtocolArgOwnership::Owned, ProtocolArgOwnership::Borrowed],
        }
    }
}

#[cfg(test)]
mod tests;
