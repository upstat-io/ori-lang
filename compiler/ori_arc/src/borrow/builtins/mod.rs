//! Semantic borrowing facts for builtin method receivers.
//!
//! Listed methods read their receiver and return an independent value. Iterator
//! transforms are excluded because they consume or retain hidden receiver data;
//! `.iter()` therefore keeps owned semantics so iterator destruction releases
//! its retained buffer. Registry sync tests pin this catalog to builtin method
//! ownership declarations.

mod cow_catalog;
#[cfg(test)]
mod tests;

pub use cow_catalog::{all_cow_method_names, copy_in_builtin_names};
pub(crate) use cow_catalog::{
    consuming_receiver_builtin_names, consuming_receiver_only_builtin_names,
    consuming_second_arg_builtin_names, consuming_third_arg_builtin_names,
    persistent_list_runtime_methods,
};
#[cfg(test)]
use cow_catalog::{
    CONSUMING_RECEIVER_METHOD_NAMES, CONSUMING_RECEIVER_ONLY_METHOD_NAMES,
    CONSUMING_SECOND_ARG_METHOD_NAMES, CONSUMING_THIRD_ARG_METHOD_NAMES,
};

use ori_ir::builtin_constants::protocol::{ProtocolArgOwnership, ProtocolBuiltin};
use ori_ir::{Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

/// Intern a static method-name table into a [`Name`] set.
fn intern_name_set(names: &[&str], interner: &StringInterner) -> FxHashSet<Name> {
    names.iter().map(|name| interner.intern(name)).collect()
}

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
///    args (`__index`, `__cast`). These are ARC pipeline internals, not
///    regular builtin methods, so [`ProtocolBuiltin`] supplies them directly.
pub fn borrowing_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    // Base set from registry (type method definitions)
    let mut names: FxHashSet<Name> = ori_registry::borrowing_method_names()
        .iter()
        .map(|name| interner.intern(name))
        .collect();

    // Append all-borrowed protocol builtins (ARC pipeline internals,
    // not in registry BUILTIN_TYPES): "__index", "__cast".
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

/// Method names that return values **sharing backing storage** with the receiver.
///
/// Unlike COW methods (which always return `Unique` results), these methods
/// create views into the receiver's data. The returned value shares the
/// receiver's logical storage identity, so its uniqueness is `MaybeShared`.
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
/// These methods return values that share the receiver's logical storage
/// identity, so their return uniqueness is `MaybeShared`.
///
/// See [`SHARING_METHOD_NAMES`] for the full list.
pub fn sharing_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    intern_name_set(SHARING_METHOD_NAMES, interner)
}

/// Accessor methods that EXTRACT an owned heap payload out of a wrapper /
/// collection and RETAIN it (codegen emits `inc_value_rc` on the extracted
/// element/payload — `option_result.rs:emit_option_method`/`emit_result_method`,
/// `list_builtins/helpers.rs:emit_list_first_or_last`, `list_builtins/mod.rs`
/// list index, `map_builtins.rs:emit_map_get`).
///
/// The retain makes the result a FRESH owned reference, NOT a buffer-sharing view
/// (contrast [`SHARING_METHOD_NAMES`] `slice`/`substring`, which return views over
/// the receiver's backing without a retain). The receiver passed at a borrowed
/// `Invoke` terminator arg therefore SURVIVES the call and its scope-exit release
/// belongs on the successor edges (Spec: Annex E §AIMS RL-2 / RL-4) — not inline
/// before the borrowed call, which would free the payload before the accessor
/// retains it.
///
/// Sorted alphabetically.
const ACCESSOR_RETAIN_METHOD_NAMES: &[&str] = &[
    "expect",     // Option/Result.expect — retains Some/Ok payload
    "expect_err", // Result.expect_err — retains Err payload
    "first",      // list.first — retains first element copy
    "get",        // list index / map.get — retains element/value copy
    "last",       // list.last — retains last element copy
    "unwrap",     // Option/Result.unwrap — retains Some/Ok payload
    "unwrap_err", // Result.unwrap_err — retains Err payload
];

/// Accessor methods that return a BORROW VIEW of an interior field without
/// minting a result credit or a shared-storage credit. The receiver's logical
/// lifetime and cleanup obligation govern the view. Contrast
/// [`ACCESSOR_RETAIN_METHOD_NAMES`] (the result receives its own credit) and
/// [`SHARING_METHOD_NAMES`] (the result receives a credit on shared storage).
/// Booking a call-result credit for one of these would double-discharge the
/// receiver's obligation.
///
/// Sorted alphabetically.
/// `trace` is NOT here: `_ori_format_error_trace` renders a FRESH owned str
/// (`OriStr::from_owned`), so its call result is an owned arrival.
const BORROW_VIEW_ACCESSOR_METHOD_NAMES: &[&str] = &[
    "trace_entries", // Error/Result.trace_entries — loads the interior trace list, no retain
];

/// Collect interned [`Name`]s for retain-less borrow-view accessor methods.
pub fn borrow_view_accessor_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    intern_name_set(BORROW_VIEW_ACCESSOR_METHOD_NAMES, interner)
}

/// Collect interned [`Name`]s for accessor methods that retain their extracted
/// payload.
///
/// See [`ACCESSOR_RETAIN_METHOD_NAMES`] for the full list and rationale.
pub fn accessor_retain_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    intern_name_set(ACCESSOR_RETAIN_METHOD_NAMES, interner)
}

/// Pre-computed interned sets for ARC ownership annotation.
///
/// Groups the builtin method name sets that
/// [`annotate_arg_ownership`](crate::rc_insert::annotate_arg_ownership)
/// needs. Constructing this once avoids redundant `intern()` work across
/// multiple function compilations.
#[derive(Debug)]
pub struct BuiltinOwnershipSets {
    /// Methods that borrow their receiver (e.g., `len`, `is_empty`).
    pub borrowing: FxHashSet<Name>,
    /// COW list methods that consume their receiver (e.g., `push`, `reverse`).
    pub consuming_receiver: FxHashSet<Name>,
    /// COW list methods that also consume their second argument (e.g., `add`, `concat`).
    pub consuming_second_arg: FxHashSet<Name>,
    /// COW methods that also consume their third argument (`updated` — the
    /// inserted value is moved into the collection).
    pub consuming_third_arg: FxHashSet<Name>,
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
    zip: Name,
    chain: Name,
    pop: Name,
    insert: Name,
}

impl BuiltinOwnershipSets {
    /// Intern all builtin method name sets from the given interner.
    pub fn new(interner: &StringInterner) -> Self {
        Self {
            borrowing: borrowing_builtin_names(interner),
            consuming_receiver: consuming_receiver_builtin_names(interner),
            consuming_second_arg: consuming_second_arg_builtin_names(interner),
            consuming_third_arg: consuming_third_arg_builtin_names(interner),
            consuming_receiver_only: consuming_receiver_only_builtin_names(interner),
            protocol: ProtocolBuiltin::ALL
                .iter()
                .map(|pb| (interner.intern(pb.name()), pb.arg_ownership()))
                .collect(),
            zip: interner.intern("zip"),
            chain: interner.intern("chain"),
            pop: interner.intern("pop"),
            insert: interner.intern("insert"),
        }
    }

    /// Return the type-qualified builtin positions that consume their argument.
    ///
    /// Collection results are the complete ownership-transfer override for the
    /// call. Iterator results supplement the registry contract with the typed
    /// receiver and `zip`/`chain` iterator-operand positions that bare method
    /// names cannot disambiguate.
    pub(crate) fn type_qualified_consuming_positions(
        &self,
        callee: Name,
        arg_tags: &[Option<ori_registry::TypeTag>],
    ) -> SmallVec<[usize; 3]> {
        use ori_registry::TypeTag;

        let mut positions = SmallVec::new();
        let Some(Some(receiver_tag)) = arg_tags.first().copied() else {
            return positions;
        };

        if matches!(
            receiver_tag,
            TypeTag::Iterator | TypeTag::DoubleEndedIterator
        ) {
            positions.push(0);
            if (callee == self.zip || callee == self.chain)
                && arg_tags.get(1).copied().flatten().is_some_and(|tag| {
                    matches!(tag, TypeTag::Iterator | TypeTag::DoubleEndedIterator)
                })
            {
                positions.push(1);
            }
            return positions;
        }

        if !matches!(receiver_tag, TypeTag::List | TypeTag::Map | TypeTag::Set)
            || callee == self.pop
            || !(self.consuming_receiver.contains(&callee)
                || self.consuming_receiver_only.contains(&callee))
        {
            return positions;
        }

        positions.push(0);
        if callee == self.insert && matches!(receiver_tag, TypeTag::Map | TypeTag::Set) {
            return positions;
        }
        if arg_tags.len() >= 2 && self.consuming_second_arg.contains(&callee) {
            positions.push(1);
        }
        if arg_tags.len() >= 3 && self.consuming_third_arg.contains(&callee) {
            positions.push(2);
        }
        positions
    }

    /// Check if a name is a known builtin method in any ownership set.
    pub fn contains(&self, name: Name) -> bool {
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
            consuming_third_arg: FxHashSet::default(),
            consuming_receiver_only: FxHashSet::default(),
            protocol: FxHashMap::default(),
            zip: Name::EMPTY,
            chain: Name::EMPTY,
            pop: Name::EMPTY,
            insert: Name::EMPTY,
        }
    }
}
