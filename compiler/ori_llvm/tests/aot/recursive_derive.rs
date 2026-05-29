//! AOT tests for derived traits on recursive struct/enum value types (BUG-04-043).
//!
//! Root defect (BUG-04-043): recursive struct/enum value types fail AOT LLVM
//! codegen because the recursive back-edge is laid out by-value, producing an
//! infinitely-sized / zero-field LLVM struct. Surface symptom on
//! `type Node = { value: int, next: Option<Node> }`:
//! `build_struct: insert_value failed (index out of bounds?) index=0
//! num_fields=0` followed by `error[E5001]: LLVM module verification failed`.
//! The fix is box-and-load codegen for `Construct`/`Project` of the recursive
//! back-edge.
//!
//! These tests pin the derive surface: `#derive(Clone)`, `#derive(Eq)`, and
//! `#derive(Debug)` on a recursive `Node`. The derived methods (`clone`,
//! `equals`/`==`, `debug`) construct and project the recursive type, so they
//! hit the same `build_struct` abort as the drop story in `recursive_drop.rs`.
//! They are TDD pins authored BEFORE the fix — every one currently aborts with
//! the recursive-codegen signature above and passes only once BUG-04-043 closes
//! (box-and-load for the recursive back-edge), at which point
//! `assert_aot_success`'s built-in `ORI_CHECK_LEAKS=1` oracle also verifies the
//! derived methods balance allocation/deallocation.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// `#derive(Clone)` on a recursive `Node`: clone a 2-node chain and assert the
/// clone is an independent deep copy by reading a field off the clone. The
/// derived `clone` constructs the recursive back-edge, hitting the `build_struct`
/// abort. Once BUG-04-043 closes, the `ORI_CHECK_LEAKS=1` oracle confirms the
/// clone allocates and the chain (original + clone) drops without leaking.
#[test]
fn recursive_derive_clone_chain_deep_copies_no_leak() {
    let source = r#"
use std.testing { assert_eq }

#derive(Clone)
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n2 = Node { value: 2, next: None };
    let n1 = Node { value: 1, next: Some(n2) };
    let c = n1.clone();
    assert_eq(actual: c.value, expected: 1);
    ()
}
"#;
    assert_aot_success(source, "recursive_derive_clone_chain_deep_copies_no_leak");
}

/// `#derive(Eq)` on a recursive `Node`: two structurally-equal chains compare
/// `== true`, a differing chain compares `== false`. The derived `equals`
/// projects the recursive back-edge to compare the `next` field, hitting the
/// `build_struct` abort. The positive/negative pair clamps the equality semantics
/// from both sides.
#[test]
fn recursive_derive_eq_equal_chains_true_differing_false() {
    let source = r#"
use std.testing { assert_eq }

#derive(Eq)
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let a2 = Node { value: 2, next: None };
    let a1 = Node { value: 1, next: Some(a2) };
    let b2 = Node { value: 2, next: None };
    let b1 = Node { value: 1, next: Some(b2) };
    let c2 = Node { value: 9, next: None };
    let c1 = Node { value: 1, next: Some(c2) };
    assert_eq(actual: a1 == b1, expected: true);
    assert_eq(actual: a1 == c1, expected: false);
    ()
}
"#;
    assert_aot_success(
        source,
        "recursive_derive_eq_equal_chains_true_differing_false",
    );
}

/// `#derive(Debug)` on a recursive `Node`: format a 2-node chain (bounded by the
/// actual `None`-terminated data, so no infinite loop) and assert the formatted
/// string is non-empty. The derived `debug` projects the recursive back-edge to
/// format the `next` field, hitting the `build_struct` abort.
#[test]
fn recursive_derive_debug_chain_formats_nonempty() {
    let source = r#"
use std.testing { assert_eq }

#derive(Debug)
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n2 = Node { value: 2, next: None };
    let n1 = Node { value: 1, next: Some(n2) };
    let s = n1.debug();
    assert_eq(actual: s.is_empty(), expected: false);
    ()
}
"#;
    assert_aot_success(source, "recursive_derive_debug_chain_formats_nonempty");
}
