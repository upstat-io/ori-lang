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
//! The LLVM backend maintains a parallel `BuiltinTable` with `receiver_borrowed`
//! flags for codegen dispatch. A sync test in `ori_llvm` asserts that table's
//! effective borrowing set matches this canonical list.

use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashSet;

/// All builtin method names that borrow their receiver, sorted alphabetically.
///
/// Each method listed here borrows its receiver and produces a result that does
/// not reference the receiver's data (i.e., the result is independent).
///
/// When adding a new builtin method to the LLVM backend's `declare_builtins!`
/// with `borrow: true`, also add its name here (if not already present).
const BORROWING_METHOD_NAMES: &[&str] = &[
    "abs",
    "byte",
    "chars",
    "clone",
    "compare",
    "concat",
    "contains",
    "contains_key",
    "count",
    "difference",
    "ends_with",
    "equals",
    "f",
    "first",
    "get",
    "hash",
    "insert",
    "intersection",
    "into",
    "is_empty",
    "is_equal",
    "is_err",
    "is_greater",
    "is_greater_or_equal",
    "is_less",
    "is_less_or_equal",
    "is_none",
    "is_ok",
    "is_some",
    "keys",
    "last",
    "len",
    "length",
    "pop",
    "remove",
    "repeat",
    "replace",
    "reverse",
    "split",
    "starts_with",
    "to_float",
    "to_int",
    "to_list",
    "to_lowercase",
    "to_str",
    "to_uppercase",
    "trim",
    "union",
    "unwrap",
    "unwrap_err",
    "unwrap_or",
    "values",
];

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
    "push",        // list.push (COW push)
    "remove",      // list.remove (COW remove)
    "reverse",     // list.reverse (COW reverse)
    "sort",        // list.sort (COW sort, unstable)
    "sort_stable", // list.sort_stable (COW sort, stable/TimSort)
];

/// COW list methods that consume both receiver AND second argument (list2).
///
/// For these methods, the runtime takes ownership of list2's buffer and checks
/// uniqueness at runtime to skip RC increments when list2 is uniquely owned.
/// The ARC pipeline must mark arg[1] as `Owned` (no extra `RcDec`) in addition
/// to the receiver.
///
/// Sorted alphabetically.
const CONSUMING_SECOND_ARG_METHOD_NAMES: &[&str] = &[
    "add",    // list + list (COW concat)
    "concat", // list.concat(other)
];

/// Collect interned [`Name`]s for all builtin methods that borrow their receiver.
///
/// Returns the set of method names (not type-qualified) that borrow inference
/// and RC insertion should treat as borrowing the receiver. This allows
/// inline-compiled builtins to avoid unnecessary `rc_inc`/`rc_dec` pairs.
///
/// See [`BORROWING_METHOD_NAMES`] for the full list and exclusion rules.
pub fn borrowing_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    BORROWING_METHOD_NAMES
        .iter()
        .map(|name| interner.intern(name))
        .collect()
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

/// Pre-computed interned sets for ARC ownership annotation.
///
/// Groups the three builtin method name sets that
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
}

impl BuiltinOwnershipSets {
    /// Intern all builtin method name sets from the given interner.
    pub fn new(interner: &StringInterner) -> Self {
        Self {
            borrowing: borrowing_builtin_names(interner),
            consuming_receiver: consuming_receiver_builtin_names(interner),
            consuming_second_arg: consuming_second_arg_builtin_names(interner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowing_method_names_sorted() {
        for window in BORROWING_METHOD_NAMES.windows(2) {
            assert!(
                window[0] < window[1],
                "BORROWING_METHOD_NAMES not sorted: {:?} >= {:?}",
                window[0],
                window[1],
            );
        }
    }

    #[test]
    fn borrowing_method_names_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for &name in BORROWING_METHOD_NAMES {
            assert!(
                seen.insert(name),
                "duplicate in BORROWING_METHOD_NAMES: {name:?}",
            );
        }
    }

    #[test]
    fn borrowing_builtin_names_returns_correct_count() {
        let interner = StringInterner::default();
        let names = borrowing_builtin_names(&interner);
        assert_eq!(
            names.len(),
            BORROWING_METHOD_NAMES.len(),
            "interned set should have same count as const array (no duplicates)"
        );
    }

    #[test]
    fn iter_excluded() {
        assert!(
            !BORROWING_METHOD_NAMES.contains(&"iter"),
            "\"iter\" must not be in BORROWING_METHOD_NAMES — .iter() creates dependent values \
             (the iterator references the receiver's data). With borrowing, the ARC pipeline \
             would dec the list data before the iterator is consumed → use-after-free. \
             Instead, iter uses Owned semantics (default) and the runtime handles cleanup: \
             IterState::List stores the data pointer and drops it via ori_buffer_rc_dec."
        );
    }

    #[test]
    fn consuming_receiver_method_names_sorted() {
        for window in CONSUMING_RECEIVER_METHOD_NAMES.windows(2) {
            assert!(
                window[0] < window[1],
                "CONSUMING_RECEIVER_METHOD_NAMES not sorted: {:?} >= {:?}",
                window[0],
                window[1],
            );
        }
    }

    #[test]
    fn consuming_receiver_method_names_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for &name in CONSUMING_RECEIVER_METHOD_NAMES {
            assert!(
                seen.insert(name),
                "duplicate in CONSUMING_RECEIVER_METHOD_NAMES: {name:?}",
            );
        }
    }

    #[test]
    fn consuming_receiver_builtin_names_returns_correct_count() {
        let interner = StringInterner::default();
        let names = consuming_receiver_builtin_names(&interner);
        assert_eq!(
            names.len(),
            CONSUMING_RECEIVER_METHOD_NAMES.len(),
            "interned set should have same count as const array (no duplicates)"
        );
    }

    #[test]
    fn push_not_in_borrowing() {
        assert!(
            !BORROWING_METHOD_NAMES.contains(&"push"),
            "\"push\" must not be in BORROWING — it's list-only and COW consuming"
        );
    }

    #[test]
    fn add_not_in_borrowing() {
        assert!(
            !BORROWING_METHOD_NAMES.contains(&"add"),
            "\"add\" must not be in BORROWING — it's list-only and COW consuming"
        );
    }

    #[test]
    fn reverse_in_both_borrowing_and_consuming() {
        // "reverse" is borrowing for Ordering (Ordering.reverse() is a pure read)
        // but consuming for List (COW semantics). The consuming-receiver override
        // in annotate_arg_ownership handles the list case.
        assert!(
            BORROWING_METHOD_NAMES.contains(&"reverse"),
            "\"reverse\" must be in BORROWING — Ordering.reverse() borrows"
        );
        assert!(
            CONSUMING_RECEIVER_METHOD_NAMES.contains(&"reverse"),
            "\"reverse\" must be in CONSUMING — list.reverse() is COW consuming"
        );
    }

    #[test]
    fn insert_in_both_borrowing_and_consuming() {
        // "insert" is borrowing for Map/Set but consuming for List (COW).
        assert!(
            BORROWING_METHOD_NAMES.contains(&"insert"),
            "\"insert\" must be in BORROWING — map/set.insert() borrows"
        );
        assert!(
            CONSUMING_RECEIVER_METHOD_NAMES.contains(&"insert"),
            "\"insert\" must be in CONSUMING — list.insert() is COW consuming"
        );
    }

    #[test]
    fn remove_in_both_borrowing_and_consuming() {
        // "remove" is borrowing for Map/Set but consuming for List (COW).
        assert!(
            BORROWING_METHOD_NAMES.contains(&"remove"),
            "\"remove\" must be in BORROWING — map/set.remove() borrows"
        );
        assert!(
            CONSUMING_RECEIVER_METHOD_NAMES.contains(&"remove"),
            "\"remove\" must be in CONSUMING — list.remove() is COW consuming"
        );
    }

    #[test]
    fn cow_methods_in_consuming() {
        for &method in &[
            "add",
            "concat",
            "insert",
            "push",
            "remove",
            "reverse",
            "sort",
            "sort_stable",
        ] {
            assert!(
                CONSUMING_RECEIVER_METHOD_NAMES.contains(&method),
                "\"{method}\" must be in CONSUMING_RECEIVER_METHOD_NAMES"
            );
        }
    }

    #[test]
    fn consuming_second_arg_method_names_sorted() {
        for window in CONSUMING_SECOND_ARG_METHOD_NAMES.windows(2) {
            assert!(
                window[0] < window[1],
                "CONSUMING_SECOND_ARG_METHOD_NAMES not sorted: {:?} >= {:?}",
                window[0],
                window[1],
            );
        }
    }

    #[test]
    fn consuming_second_arg_method_names_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for &name in CONSUMING_SECOND_ARG_METHOD_NAMES {
            assert!(
                seen.insert(name),
                "duplicate in CONSUMING_SECOND_ARG_METHOD_NAMES: {name:?}",
            );
        }
    }

    #[test]
    fn consuming_second_arg_builtin_names_returns_correct_count() {
        let interner = StringInterner::default();
        let names = consuming_second_arg_builtin_names(&interner);
        assert_eq!(
            names.len(),
            CONSUMING_SECOND_ARG_METHOD_NAMES.len(),
            "interned set should have same count as const array (no duplicates)"
        );
    }

    #[test]
    fn consuming_second_arg_subset_of_consuming_receiver() {
        for &method in CONSUMING_SECOND_ARG_METHOD_NAMES {
            assert!(
                CONSUMING_RECEIVER_METHOD_NAMES.contains(&method),
                "\"{method}\" is in CONSUMING_SECOND_ARG but not CONSUMING_RECEIVER — \
                 a method can't consume arg[1] without also consuming the receiver"
            );
        }
    }
}
