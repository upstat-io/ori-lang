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
//! - **`.iter()`**: Creates an iterator that references the receiver's data,
//!   so it must use Owned semantics.
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
    "add",     // list + list (COW concat)
    "concat",  // list.concat (COW concat)
    "push",    // list.push (COW push)
    "reverse", // list.reverse (COW reverse)
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
            "\"iter\" must not be in BORROWING_METHOD_NAMES — .iter() creates dependent values"
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
    fn cow_methods_in_consuming() {
        for &method in &["add", "concat", "push", "reverse"] {
            assert!(
                CONSUMING_RECEIVER_METHOD_NAMES.contains(&method),
                "\"{method}\" must be in CONSUMING_RECEIVER_METHOD_NAMES"
            );
        }
    }
}
