//! Unit tests for `assert_no_unresolved_type_vars`.
//!
//! Full 12-cell matrix (§04.4 deliverable) lands in a subsequent commit.
//! This stub ensures the module links cleanly during §04.1.

use super::UnresolvedTypeVar;

#[test]
fn unresolved_type_var_is_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<UnresolvedTypeVar>();
}
