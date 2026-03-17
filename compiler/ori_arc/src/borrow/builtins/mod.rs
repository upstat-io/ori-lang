//! Borrowing-builtin method knowledge for ARC borrow inference.
//!
//! Defines which builtin methods borrow their receiver (read without consuming)
//! AND produce independent results (no hidden dependency on the receiver's data).
//!
//! This is a **language-semantic fact**, not a codegen implementation detail.
//! Borrow inference needs this knowledge to recognize calls to inline-compiled
//! builtins (e.g., `len`, `is_empty`, `compare`) as borrowing rather than
//! defaulting to all-Owned.
//!
//! # Exclusions
//!
//! - **Iterator methods**: These consume/transform the iterator or create
//!   derived values — the ARC pipeline can't model these hidden dependencies.
//! - **`.iter()`**: Creates an iterator that references the receiver's data.
//!   Uses Owned semantics (default). The runtime's `IterState::List` stores
//!   the data pointer, cap, and `elem_dec_fn`; `Drop for IterState` calls
//!   `ori_buffer_rc_dec` to release the list data when the iterator is consumed.
//!
//! # Sync
//!
//! The LLVM backend's `BuiltinTable` registration reads from
//! `ori_registry::BUILTIN_TYPES` and respects `Ownership::Borrow` annotations.
//! A sync test in `ori_llvm` asserts the effective borrowing set matches this
//! canonical list.

use ori_ir::builtin_constants::protocol::{ProtocolArgOwnership, ProtocolBuiltin};
use ori_ir::{Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

/// Method names with **consuming receiver** semantics for list types.
///
/// These are COW (Copy-on-Write) list methods that handle the old buffer's
/// RC lifecycle internally: the fast path reuses the buffer (unique owner),
/// the slow path allocates a new buffer and `ori_rc_dec`s the old one.
///
/// The ARC pipeline must NOT emit an additional `RcDec` for the receiver
/// argument when calling these methods — doing so causes double-free.
///
/// **Type-qualified**: `"add"` and `"concat"` are borrowing for strings but
/// consuming for lists. The type check happens at the call site in
/// [`annotate_arg_ownership`](crate::rc_insert::annotate_arg_ownership).
///
/// Sorted alphabetically.
const CONSUMING_RECEIVER_METHOD_NAMES: &[&str] = &[
    "add",         // list + list (COW concat)
    "concat",      // list.concat (COW concat)
    "insert",      // list.insert (COW insert)
    "iter",        // list.iter (iterator takes ownership of data buffer)
    "pop",         // list.pop (COW pop)
    "push",        // list.push (COW push)
    "remove",      // list.remove (COW remove)
    "reverse",     // list.reverse (COW reverse)
    "set",         // list.set (COW set at index)
    "sort",        // list.sort (COW sort, unstable)
    "sort_stable", // list.sort_stable (COW sort, stable/TimSort)
];

/// COW list methods that consume both receiver AND second argument.
///
/// For these methods, the runtime takes ownership of the second argument's data:
/// - `add`/`concat`: list2's buffer is consumed (uniqueness-checked at runtime)
/// - `push`: the element's bytes are copied into the list buffer, creating a
///   new reference to any RC-managed data (e.g., str data pointers)
///
/// The ARC pipeline must mark arg[1] as `Owned` (no extra `RcDec`) in addition
/// to the receiver. For `push`, this ensures AIMS emits `RcInc` on fat pointer
/// elements borrowed from iterators.
///
/// Sorted alphabetically.
const CONSUMING_SECOND_ARG_METHOD_NAMES: &[&str] = &[
    "add",    // list + list (COW concat)
    "concat", // list.concat(other)
    "push",   // list.push(value) — element stored in list buffer
];

/// COW methods that consume ONLY the receiver; non-receiver args are borrowed.
///
/// These are Map/Set COW methods where the runtime takes ownership of the
/// receiver's buffer but only reads other arguments (comparison keys, read-only
/// collections). Contrast with `CONSUMING_RECEIVER_METHOD_NAMES` where List
/// methods also transfer inserted elements.
///
/// In `compute_arg_ownership`, these methods produce `[Owned, Borrowed, ...]`
/// instead of the default all-Owned, preventing RC leaks on comparison keys
/// and read-only collection arguments.
///
/// Sorted alphabetically.
const CONSUMING_RECEIVER_ONLY_METHOD_NAMES: &[&str] = &[
    "difference",   // set.difference(other) — other is read-only
    "insert",       // map/set.insert(key, val) — key/val are copied, not consumed
    "intersection", // set.intersection(other) — other is read-only
    "remove",       // map/set.remove(key) — key is comparison-only
    "union",        // set.union(other) — other is read-only
];

/// Collect interned [`Name`]s for all builtin methods that borrow their receiver.
///
/// Returns the set of method names (not type-qualified) that borrow inference
/// and RC insertion should treat as borrowing the receiver. This allows
/// inline-compiled builtins to avoid unnecessary `rc_inc`/`rc_dec` pairs.
///
/// The set is derived from two sources:
/// 1. **Registry**: `ori_registry::borrowing_method_names()` — all builtin type
///    methods with `receiver: Ownership::Borrow`, excluding Iterator methods
///    and `.iter()`.
/// 2. **Protocol builtins**: `ProtocolBuiltin::ALL` entries with all-borrowed
///    args (currently only `__index`). These are ARC pipeline internals, not
///    regular builtin methods, so they live in `ori_ir` rather than the registry.
pub fn borrowing_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    // Base set from registry (type method definitions)
    let mut names: FxHashSet<Name> = ori_registry::borrowing_method_names()
        .iter()
        .map(|name| interner.intern(name))
        .collect();

    // Append all-borrowed protocol builtins (ARC pipeline internals,
    // not in registry BUILTIN_TYPES). Currently only "__index".
    for pb in ProtocolBuiltin::ALL {
        if pb
            .arg_ownership()
            .iter()
            .all(|o| *o == ProtocolArgOwnership::Borrowed)
        {
            names.insert(interner.intern(pb.name()));
        }
    }

    names
}

/// Collect interned [`Name`]s for COW list methods with consuming receiver semantics.
///
/// These methods handle the old buffer's RC internally. When the receiver is
/// a `List` type, the ARC pipeline must mark the receiver argument as `Owned`
/// (no extra `RcDec`) instead of the default `Borrowed` from the borrowing set.
///
/// See [`CONSUMING_RECEIVER_METHOD_NAMES`] for the full list and rationale.
pub fn consuming_receiver_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    CONSUMING_RECEIVER_METHOD_NAMES
        .iter()
        .map(|name| interner.intern(name))
        .collect()
}

/// Collect interned [`Name`]s for COW list methods that also consume their
/// second argument (list2).
///
/// When the receiver is a `List` type and `args.len() >= 2`, the ARC pipeline
/// marks `arg_ownership[1]` as `Owned` to prevent a duplicate `RcDec` — the
/// runtime takes ownership of list2 and handles its lifecycle internally.
///
/// See [`CONSUMING_SECOND_ARG_METHOD_NAMES`] for the full list.
pub fn consuming_second_arg_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    CONSUMING_SECOND_ARG_METHOD_NAMES
        .iter()
        .map(|name| interner.intern(name))
        .collect()
}

/// Collect interned [`Name`]s for COW methods that consume only the receiver.
///
/// Non-receiver arguments are borrowed (comparison keys, read-only collections).
/// Used by `compute_arg_ownership` to produce `[Owned, Borrowed, ...]` instead
/// of the default all-Owned.
///
/// See [`CONSUMING_RECEIVER_ONLY_METHOD_NAMES`] for the full list.
pub fn consuming_receiver_only_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    CONSUMING_RECEIVER_ONLY_METHOD_NAMES
        .iter()
        .map(|name| interner.intern(name))
        .collect()
}

/// Collect interned [`Name`]s for ALL COW methods (union of consuming-receiver
/// and consuming-receiver-only sets).
///
/// This is the single integration point for uniqueness analysis: both
/// [`consuming_receiver_builtin_names`] (list COW: `push`, `sort`, …) and
/// [`consuming_receiver_only_builtin_names`] (map/set COW: `remove`, `union`, …)
/// are COW operations whose results are always `Unique` (RC == 1).
///
/// Pass the result to [`crate::uniqueness::inter::build_cow_summaries`] as the
/// `cow_method_names` argument.
pub fn all_cow_method_names(interner: &StringInterner) -> FxHashSet<Name> {
    let mut names = consuming_receiver_builtin_names(interner);
    names.extend(consuming_receiver_only_builtin_names(interner));
    names
}

/// Method names that return values **sharing backing storage** with the receiver.
///
/// Unlike COW methods (which always return `Unique` results), these methods
/// create views into the receiver's data. The returned value shares the
/// receiver's refcounted backing buffer, so its uniqueness is `MaybeShared`.
///
/// Used by [`crate::uniqueness::inter::build_cow_summaries`] as the
/// `shared_method_names` argument.
///
/// Sorted alphabetically.
const SHARING_METHOD_NAMES: &[&str] = &[
    "slice",     // list.slice — shares list backing
    "substring", // str.substring — shares string backing
];

/// Collect interned [`Name`]s for methods that share backing with their receiver.
///
/// These methods return values that reference the receiver's heap data,
/// so their return uniqueness is `MaybeShared` (not `Unique` like COW methods).
///
/// See [`SHARING_METHOD_NAMES`] for the full list.
pub fn sharing_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    SHARING_METHOD_NAMES
        .iter()
        .map(|name| interner.intern(name))
        .collect()
}

/// Pre-computed interned sets for ARC ownership annotation.
///
/// Groups the builtin method name sets that
/// [`annotate_arg_ownership`](crate::rc_insert::annotate_arg_ownership)
/// needs. Constructing this once avoids redundant `intern()` work across
/// multiple function compilations.
pub struct BuiltinOwnershipSets {
    /// Methods that borrow their receiver (e.g., `len`, `is_empty`).
    pub borrowing: FxHashSet<Name>,
    /// COW list methods that consume their receiver (e.g., `push`, `reverse`).
    pub consuming_receiver: FxHashSet<Name>,
    /// COW list methods that also consume their second argument (e.g., `add`, `concat`).
    pub consuming_second_arg: FxHashSet<Name>,
    /// COW methods that consume only the receiver; other args are borrowed.
    ///
    /// For Map/Set operations like `remove(key)` and `union(other)`, the
    /// receiver is consumed (COW handles its RC) but the key/other-set is
    /// only read for comparison — its RC must be decremented by the caller.
    pub consuming_receiver_only: FxHashSet<Name>,
    /// Pre-interned protocol builtin lookup: `Name` → per-arg ownership.
    ///
    /// Used by borrow inference and RC annotation to handle protocol
    /// builtins (`__index`, `__iter_next`, `__collect_set`) without
    /// needing the `StringInterner` in the core loop.
    pub protocol: FxHashMap<Name, &'static [ProtocolArgOwnership]>,
}

impl BuiltinOwnershipSets {
    /// Intern all builtin method name sets from the given interner.
    pub fn new(interner: &StringInterner) -> Self {
        Self {
            borrowing: borrowing_builtin_names(interner),
            consuming_receiver: consuming_receiver_builtin_names(interner),
            consuming_second_arg: consuming_second_arg_builtin_names(interner),
            consuming_receiver_only: consuming_receiver_only_builtin_names(interner),
            protocol: ProtocolBuiltin::ALL
                .iter()
                .map(|pb| (interner.intern(pb.name()), pb.arg_ownership()))
                .collect(),
        }
    }

    /// Check if a name is a known builtin method (in any ownership set).
    pub fn is_builtin(&self, name: Name) -> bool {
        self.borrowing.contains(&name)
            || self.consuming_receiver.contains(&name)
            || self.consuming_receiver_only.contains(&name)
            || self.protocol.contains_key(&name)
    }

    /// Empty sets for unit tests that don't exercise builtin ownership.
    pub fn empty() -> Self {
        Self {
            borrowing: FxHashSet::default(),
            consuming_receiver: FxHashSet::default(),
            consuming_second_arg: FxHashSet::default(),
            consuming_receiver_only: FxHashSet::default(),
            protocol: FxHashMap::default(),
        }
    }
}

#[cfg(test)]
mod tests;
