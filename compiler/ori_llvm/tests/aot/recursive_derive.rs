//! AOT tests for derived traits on recursive struct/enum value types.
//!
//! Root defect: recursive struct/enum value types fail AOT LLVM
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
//! the recursive-codegen signature above and passes only once the root defect is fixed
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
/// abort. Once the root defect is fixed, the `ORI_CHECK_LEAKS=1` oracle confirms the
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

/// `#derive(Eq)` on a recursive `Node` whose back-edge is the `Ok` arm of a
/// `Result<Node, int>` field. The derived `equals` compares the `next` field
/// through the `Result` payload-comparison path; the `Ok` payload type is the
/// recursive `Node`, so its slot holds an RC box pointer, not an inline `Node`
/// struct. Equality MUST load through the box before comparing — loading the
/// box pointer as an inline struct compares pointer bits, not node contents, so
/// two structurally-equal chains built from distinct allocations would compare
/// unequal. The positive/negative pair clamps the semantics: equal chains
/// compare true, a chain differing one level deep compares false. Reverting the
/// box-load (comparing the raw box pointer) flips the equal-chains case to
/// false, failing this pin.
#[test]
fn recursive_derive_eq_result_boxed_ok_compares_through_box() {
    let source = r#"
use std.testing { assert_eq }

#derive(Eq)
type Node = { value: int, next: Result<Node, int> }

@main () -> void = {
    let a2 = Node { value: 2, next: Err(0) };
    let a1 = Node { value: 1, next: Ok(a2) };
    let b2 = Node { value: 2, next: Err(0) };
    let b1 = Node { value: 1, next: Ok(b2) };
    let c2 = Node { value: 9, next: Err(0) };
    let c1 = Node { value: 1, next: Ok(c2) };
    assert_eq(actual: a1 == b1, expected: true);
    assert_eq(actual: a1 == c1, expected: false);
    ()
}
"#;
    assert_aot_success(
        source,
        "recursive_derive_eq_result_boxed_ok_compares_through_box",
    );
}

/// `#derive(Clone)` on the same recursive-`Result`-payload `Node`. The derived
/// `clone` RC-increments the `next` field through the `Result` clone path; the
/// `Ok` payload is the recursive `Node`, whose slot holds an RC box pointer.
/// Clone MUST RC-inc the box POINTER (sharing the box, COW value semantics) —
/// loading and RC-incrementing the full inline `Node` struct treats the box
/// pointer's bytes as a struct and increments a wrong/garbage pointer, leaking
/// the real box and corrupting the chain. `assert_aot_success`'s built-in
/// `ORI_CHECK_LEAKS=1` oracle verifies clone + drop of both chains balance
/// allocation/deallocation; a wrong RC-inc target leaks (exit code 2).
#[test]
fn recursive_derive_clone_result_boxed_ok_incs_box_no_leak() {
    let source = r#"
use std.testing { assert_eq }

#derive(Clone)
type Node = { value: int, next: Result<Node, int> }

@main () -> void = {
    let n2 = Node { value: 2, next: Err(0) };
    let n1 = Node { value: 1, next: Ok(n2) };
    let c = n1.clone();
    assert_eq(actual: c.value, expected: 1);
    ()
}
"#;
    assert_aot_success(
        source,
        "recursive_derive_clone_result_boxed_ok_incs_box_no_leak",
    );
}

/// `#derive(Eq)` on `Outer` whose `items` is a `[Result<Inner, int>]` — a LIST
/// of `Result`, where `Inner` is recursive (`next: Option<Inner>`). A list of
/// non-scalar elements routes element equality through `ori_list_eq_deep` with a
/// generated per-element eq THUNK; the `Result` element's thunk GEPs the offset-8
/// payload slot and hands the slot pointer to its Ok/Err arm eq thunks. The Ok
/// payload `Inner` is a boxed recursive back-edge, so its slot holds an RC box
/// pointer, not an inline `Inner`. The Ok-arm thunk MUST load through the box
/// before comparing — comparing the box-pointer-slot bytes as an `Inner` reads a
/// pointer (and adjacent bytes past the 16-byte `Result`) instead of node
/// contents, so two structurally-equal chains from distinct allocations compare
/// unequal. This pin covers the THUNK comparison path, distinct from the inline
/// `emit_result_eq` path exercised by
/// `recursive_derive_eq_result_boxed_ok_compares_through_box` (where the `Result`
/// is a direct struct field, not a list element). The positive/negative pair
/// clamps the semantics: equal chains compare true, a chain differing one level
/// deep compares false.
///
/// Reachability + fail-on-revert: the list-of-`Result` field is what routes the
/// `Result` element eq through the thunk (verified in emitted IR:
/// `ori_list_eq_deep(..., @_ori_eq_result_*)`). Reverting the cure (handing the
/// raw offset-8 slot pointer to the Ok arm thunk instead of loading through the
/// box) flips the equal-chains case to false under AOT codegen — the AOT binary
/// panics `assertion failed: false != true`. The JIT path can mask the flip via
/// `FastISel` alloca reuse, so this pin runs through `assert_aot_success`'s
/// AOT-compile-and-execute path, which is the `FastISel`-divergent surface.
#[test]
fn recursive_derive_eq_result_boxed_ok_in_list_compares_through_box() {
    let source = r#"
use std.testing { assert_eq }

#derive(Eq)
type Inner = { value: int, next: Option<Inner> }

#derive(Eq)
type Outer = { items: [Result<Inner, int>] }

@main () -> void = {
    let a_in = Inner { value: 1, next: Some(Inner { value: 2, next: None }) };
    let a = Outer { items: [Ok(a_in)] };
    let b_in = Inner { value: 1, next: Some(Inner { value: 2, next: None }) };
    let b = Outer { items: [Ok(b_in)] };
    let c_in = Inner { value: 1, next: Some(Inner { value: 9, next: None }) };
    let c = Outer { items: [Ok(c_in)] };
    assert_eq(actual: a == b, expected: true);
    assert_eq(actual: a == c, expected: false);
    ()
}
"#;
    assert_aot_success(
        source,
        "recursive_derive_eq_result_boxed_ok_in_list_compares_through_box",
    );
}
