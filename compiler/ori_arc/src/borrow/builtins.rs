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
    "add",
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
    "push",
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
}
