//! AOT tests for recursive-type feature interactions.
//!
//! Root defect: recursive struct/enum value types fail AOT
//! LLVM codegen because the recursive back-edge is laid out by-value,
//! producing an infinitely-sized / zero-field LLVM struct. Surface symptom
//! on `type Node = { value: int, next: Option<Node> }`:
//! `build_struct: insert_value failed (index out of bounds?) index=0
//! num_fields=0` (struct path) or `extract_value on non-struct value ...
//! type resolution produced wrong layout` + `struct_gep on non-pointer
//! value` (enum/projection path), each followed by `error[E5001]: LLVM
//! module verification failed`. The fix is box-and-load codegen for the
//! recursive back-edge.
//!
//! These tests exercise recursive types where they INTERACT with other
//! language features — closures, `Result`, generics, mutual recursion,
//! collections, Option-returning accessors, and tuples. Each pin is
//! authored BEFORE the fix and currently aborts with the recursive-codegen
//! signature above; they pass only once the root defect is fixed, at which point
//! `assert_aot_success`'s built-in `ORI_CHECK_LEAKS=1` oracle also verifies
//! the recursive drop balances allocation/deallocation.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// A closure captures a recursive `Node` value and is invoked under `@main`.
/// The captured `Node` carries a `next: Option<Node>` back-edge, so the
/// closure environment construction routes through the recursive struct
/// codegen path. Pre-fix: `build_struct: insert_value failed ... num_fields=0`
/// + `error[E5001]`.
#[test]
fn recursive_feature_closure_captures_recursive_node() {
    let source = r#"
use std.testing { assert_eq }
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n2 = Node { value: 2, next: None };
    let n1 = Node { value: 1, next: Some(n2) };
    let f = (x: int) -> int = x + n1.value;
    let r = f(10);
    assert_eq(actual: r, expected: 11);
    ()
}
"#;
    assert_aot_success(source, "recursive_feature_closure_captures_recursive_node");
}

/// A recursive `Node` lives in the `Ok` arm of a `Result<Node, str>`.
/// Constructing `Ok(node)` and matching it back out routes the recursive
/// struct through the sum-type payload codegen path. Pre-fix:
/// `build_struct: insert_value failed ... num_fields=0` + `error[E5001]`.
#[test]
fn recursive_feature_result_ok_holds_recursive_node() {
    let source = r#"
use std.testing { assert_eq }
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n2 = Node { value: 2, next: None };
    let n1 = Node { value: 1, next: Some(n2) };
    let r: Result<Node, str> = Ok(n1);
    let v = match r {
        Ok(node) -> node.value,
        Err(_) -> 0,
    };
    assert_eq(actual: v, expected: 1);
    ()
}
"#;
    assert_aot_success(source, "recursive_feature_result_ok_holds_recursive_node");
}

/// Sum-type matrix sibling of `recursive_feature_result_ok_holds_recursive_node`:
/// a recursive `Node` lives in the `Some` arm of an `Option<Node>`. Matching
/// `Some(node)` take-projects the payload out of a consumed scrutinee whose own
/// drop is then sequenced after the projected binding's last use. The container
/// must not be credited with covering the moved-out payload's RC slot
/// (`04B.2-under-elim`: every concrete CFG path nets RC to 0). Pins the fix
/// across the `Result`/`Option` sum-type dimension.
#[test]
fn recursive_feature_option_some_holds_recursive_node() {
    let source = r#"
use std.testing { assert_eq }
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n2 = Node { value: 2, next: None };
    let n1 = Node { value: 1, next: Some(n2) };
    let o: Option<Node> = Some(n1);
    let v = match o {
        Some(node) -> node.value,
        None -> 0,
    };
    assert_eq(actual: v, expected: 1);
    ()
}
"#;
    assert_aot_success(source, "recursive_feature_option_some_holds_recursive_node");
}

/// A generic recursive `Box<T>` instantiated at BOTH `Box<int>` and
/// `Box<str>` in one program. Each monomorphization lays out a
/// `next: Option<Box<T>>` back-edge, so both specializations hit the
/// recursive struct codegen path. Pre-fix: `build_struct: insert_value
/// failed ... num_fields=0` + `error[E5001]`.
#[test]
fn recursive_feature_generic_box_two_instantiations() {
    let source = r#"
use std.testing { assert_eq }
type Box<T> = { val: T, next: Option<Box<T>> }

@main () -> void = {
    let bi2 = Box { val: 2, next: None };
    let bi1 = Box { val: 1, next: Some(bi2) };
    let bs2 = Box { val: "b", next: None };
    let bs1 = Box { val: "a", next: Some(bs2) };
    assert_eq(actual: bi1.val, expected: 1);
    assert_eq(actual: bs1.val, expected: "a");
    ()
}
"#;
    assert_aot_success(source, "recursive_feature_generic_box_two_instantiations");
}

/// Mutual recursion among ENUMS only: `A = Stop | ToB(b: B)` and
/// `B = ToA(a: A)`. Constructing a small `A -> B -> A` cycle routes the
/// mutually-recursive enum payloads through codegen. Unlike a
/// direct-self-recursive enum payload (already heap-boxed today), the
/// mutual cycle reproduces the root defect: pre-fix `extract_value on non-struct
/// value ... type resolution produced wrong layout` + `struct_gep on
/// non-pointer value` + `error[E5001]`.
#[test]
fn recursive_feature_mutual_enum_cycle() {
    let source = r#"
use std.testing { assert_eq }
type A = Stop | ToB(b: B);
type B = ToA(a: A);

@main () -> void = {
    let a = Stop;
    let b = ToA(a: a);
    let a2 = ToB(b: b);
    let depth = 3;
    assert_eq(actual: depth, expected: 3);
    ()
}
"#;
    assert_aot_success(source, "recursive_feature_mutual_enum_cycle");
}

/// Mutual recursion crossing a STRUCT/ENUM boundary: `S = { e: E }` and
/// `E = Leaf | ToS(s: S)`. Building `S -> E -> S` routes the struct field
/// and the enum payload through the recursive codegen path. Pre-fix:
/// `extract_value on non-struct value ... wrong layout` + `struct_gep on
/// non-pointer value` + `error[E5001]`.
#[test]
fn recursive_feature_mutual_struct_enum_cycle() {
    let source = r#"
use std.testing { assert_eq }
type S = { e: E };
type E = Leaf | ToS(s: S);

@main () -> void = {
    let s1 = S { e: Leaf };
    let e1 = ToS(s: s1);
    let s2 = S { e: e1 };
    let depth = 2;
    assert_eq(actual: depth, expected: 2);
    ()
}
"#;
    assert_aot_success(source, "recursive_feature_mutual_struct_enum_cycle");
}

/// A list of recursive elements: `[Node]` constructed and queried under
/// `@main`. The `Node` struct itself carries a `next: Option<Node>`
/// back-edge, so constructing each element routes through the recursive
/// struct codegen path before the list wraps it. Pre-fix: `build_struct:
/// insert_value failed ... num_fields=0` + `error[E5001]`.
#[test]
fn recursive_feature_list_of_recursive_nodes() {
    let source = r#"
use std.testing { assert_eq }
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n2 = Node { value: 2, next: None };
    let n1 = Node { value: 1, next: Some(n2) };
    let nodes = [n1];
    assert_eq(actual: nodes.length(), expected: 1);
    ()
}
"#;
    assert_aot_success(source, "recursive_feature_list_of_recursive_nodes");
}

/// A recursive element read back out of a list via an Option-returning
/// accessor: `[Node].first()` -> `Option<Node>`, then access the recursive
/// element's `.value`. Exercises construction, the Option-returning
/// accessor, and projection of the recursive struct. Pre-fix:
/// `build_struct: insert_value failed ... num_fields=0` + `error[E5001]`.
#[test]
fn recursive_feature_list_first_readback() {
    let source = r#"
use std.testing { assert_eq }
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n2 = Node { value: 2, next: None };
    let n1 = Node { value: 1, next: Some(n2) };
    let nodes = [n1];
    let first = nodes.first();
    let v = match first {
        Some(node) -> node.value,
        None -> 0,
    };
    assert_eq(actual: v, expected: 1);
    ()
}
"#;
    assert_aot_success(source, "recursive_feature_list_first_readback");
}

/// A recursive field nested inside a tuple inside an `Option`: the `paired`
/// field is typed `Option<(Node, int)>`, so the recursion cycle passes
/// through a tuple. Constructing `Some((leaf, 42))` routes the recursive
/// struct through the tuple-payload codegen path. Pre-fix: `build_struct:
/// insert_value failed ... num_fields=0` + `error[E5001]`.
#[test]
fn recursive_feature_tuple_nested_recursive_field() {
    let source = r#"
use std.testing { assert_eq }
type Node = { value: int, paired: Option<(Node, int)> }

@main () -> void = {
    let leaf = Node { value: 9, paired: None };
    let root = Node { value: 1, paired: Some((leaf, 42)) };
    assert_eq(actual: root.value, expected: 1);
    ()
}
"#;
    assert_aot_success(source, "recursive_feature_tuple_nested_recursive_field");
}
