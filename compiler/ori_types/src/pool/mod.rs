//! Interned structural type storage.
//!
//! [`Idx`] values address deduplicated [`Item`] entries, parallel flags and
//! hashes, and variable-length extra data. Primitive indices are fixed.

mod accessors;
mod bootstrap;
mod collection_surface;
mod construct;
pub mod descriptor;
mod flags_compute;
mod format;
mod hashing;
mod interning;
pub mod prelude;
mod queries;
pub mod re_intern;
mod structural_eq;
pub mod substitute;

use rustc_hash::FxHashMap;

use crate::{Idx, Item, Rank, TypeFlags};

pub use prelude::*;

/// Deduplicated type storage addressed by [`Idx`].
/// Parallel metadata supports constant-time equality and property queries.
// Why: Cross-module re-interning needs an owned merged pool despite O(n) cloning.
#[derive(Clone, Debug)]
pub struct Pool {
    /// All type items (tag + data).
    items: Vec<Item>,
    /// Pre-computed flags for each item (flags[i] corresponds to items[i]).
    flags: Vec<TypeFlags>,
    /// Stable hashes for each item (hashes[i] corresponds to items[i]).
    hashes: Vec<u64>,

    /// Variable-length data for complex types.
    /// Layout depends on tag (see documentation on each type).
    extra: Vec<u32>,

    /// Hash -> Idx mapping for deduplication.
    intern_map: FxHashMap<u64, Idx>,

    /// Maps Named/Applied Idx -> concrete Struct/Enum Idx.
    ///
    /// Populated during type registration to bridge the gap between
    /// named type references (created by the parser) and their concrete
    /// Pool definitions (Struct/Enum with full field data).
    resolutions: FxHashMap<Idx, Idx>,

    /// Symbolic C-ABI kind for each nominal `c_*` type.
    /// Scalar resolution preserves source ergonomics; this table retains
    /// target-dependent width identity for FFI boundaries.
    cabi_kinds: FxHashMap<Idx, ori_ir::CAbiKind>,

    /// Underlying type for each nominal, layout-transparent newtype name.
    /// Name keys let lowering recognize constructors before an `Idx` lookup.
    /// Spec: Clause 8.6.3.
    newtype_ctors: FxHashMap<ori_ir::Name, Idx>,

    /// Registered user-facing `Error` struct, distinct from the poison sentinel.
    /// Consumers use it to select the `TypeTag::Error` behavior table.
    error_struct_idx: Option<Idx>,

    /// State for each type variable.
    var_states: Vec<VarState>,
    /// Counter for generating fresh variable IDs.
    next_var_id: u32,
}

/// State carried by an unbound type variable.
#[derive(Clone, Debug)]
pub struct UnboundVarState {
    pub(crate) id: u32,
    pub(crate) rank: Rank,
    pub(crate) name: Option<ori_ir::Name>,
}

/// State carried by a generalized type variable.
#[derive(Clone, Debug)]
pub struct GeneralizedVarState {
    pub(crate) id: u32,
    pub(crate) name: Option<ori_ir::Name>,
}

/// State of a type variable.
#[derive(Clone, Debug)]
pub enum VarState {
    /// Unbound variable - waiting to be unified.
    Unbound(UnboundVarState),

    /// Linked to another type - follow the link.
    Link {
        /// The type this variable is unified with.
        target: Idx,
    },

    /// Rigid variable from annotation - cannot unify with concrete types.
    Rigid {
        /// The name from the annotation.
        name: ori_ir::Name,
    },

    /// Generalized variable - must be instantiated before use.
    Generalized(GeneralizedVarState),
}

#[cfg(test)]
mod tests;
