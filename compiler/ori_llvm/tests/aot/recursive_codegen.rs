//! AOT codegen tests for recursive struct/enum value types.
//!
//! Root defect: a recursive struct/enum value type whose
//! back-edge is laid out BY VALUE produces an infinitely-sized /
//! zero-field LLVM struct. The surface symptom on
//! `type Node = { value: int, next: Option<Node> }` is:
//! `build_struct: insert_value failed (index out of bounds?) index=0
//! num_fields=0` followed by `error[E5001]: LLVM module verification
//! failed`. The fix is box-and-load codegen for `Construct` / `Project`
//! of the recursive back-edge.
//!
//! These are TDD pins authored BEFORE the fix. The recursive-STRUCT cells
//! currently abort at LLVM codegen with the signature above; they pass
//! only once the root defect is fixed, at which point `assert_aot_success`'s
//! built-in `ORI_CHECK_LEAKS=1` oracle also verifies the recursive drop
//! balances allocation/deallocation.
//!
//! Scope-clamp finding (verified this session): the codegen abort
//! reproduces for recursive STRUCT value types with a self-referential
//! `next: Option<Self>` field, AND for mutual recursion whose cycle
//! passes through such a struct field. It does NOT reproduce for a
//! recursive ENUM whose recursive payload is a variant field
//! (`Cons(tail: IntList)`) — enum variant payloads are already boxed and
//! compile/run/leak-clean today. `recursive_codegen_shape_enum_*` is
//! therefore a positive boundary clamp (green today, must stay green
//! post-fix), not a red pin.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// shape-struct: canonical repro of the root defect. A recursive `Node` struct
/// whose `next: Option<Node>` field is the by-value back-edge. Building a
/// two-node chain in `@main` aborts at codegen pre-fix
/// (`build_struct ... num_fields=0` + `E5001`); post-fix it compiles,
/// runs, and drops the chain with no leak.
#[test]
fn recursive_codegen_struct_node_chain_builds() {
    let source = r#"
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n2 = Node { value: 2, next: None };
    let n1 = Node { value: 1, next: Some(n2) };
    ()
}
"#;
    assert_aot_success(source, "recursive_codegen_struct_node_chain_builds");
}

/// shape-enum (positive boundary clamp): a recursive ENUM
/// `IntList = Nil | Cons(head: int, tail: IntList)` whose recursive
/// back-edge is a variant payload field. Enum variant payloads are
/// already boxed, so this compiles, runs, and leak-checks clean TODAY —
/// it pins the boundary of the defect (the defect is struct-field
/// specific) and MUST remain green after the fix. The `match` read +
/// `assert_eq` keeps it a real positive test, not an orphan.
#[test]
fn recursive_codegen_enum_cons_list_reads_head() {
    let source = r#"
use std.testing { assert_eq }
type IntList = Nil | Cons(head: int, tail: IntList);

@main () -> void = {
    let lst = Cons(head: 1, tail: Cons(head: 2, tail: Nil));
    let n = match lst {
        Cons(head:, tail:) -> head,
        Nil -> 0,
    };
    assert_eq(actual: n, expected: 1);
    ()
}
"#;
    assert_aot_success(source, "recursive_codegen_enum_cons_list_reads_head");
}

/// shape-mutual: mutually-recursive `Tree` / `Forest`. `Tree` has a
/// `Branch(forest: Forest)` variant and `Forest` carries a recursive
/// struct field `rest: Option<Forest>` — the cycle Tree -> Forest -> Tree
/// plus the self-edge Forest -> Forest passes through a by-value
/// recursive struct field, which reproduces the codegen abort
/// pre-fix.
#[test]
fn recursive_codegen_mutual_tree_forest_builds() {
    let source = r#"
type Tree = Leaf(label: int) | Branch(forest: Forest);
type Forest = { head: Tree, rest: Option<Forest> }

@main () -> void = {
    let f = Forest { head: Leaf(label: 1), rest: None };
    let t = Branch(forest: f);
    ()
}
"#;
    assert_aot_success(source, "recursive_codegen_mutual_tree_forest_builds");
}

/// payload-str: recursive struct field alongside a `str` payload. Pins
/// that the box-and-load fix preserves the adjacent heap-typed `str`
/// field (its RC + drop) when the recursive back-edge is boxed. Aborts
/// at codegen pre-fix.
#[test]
fn recursive_codegen_struct_with_str_payload_builds() {
    let source = r#"
type SNode = { label: str, next: Option<SNode> }

@main () -> void = {
    let n2 = SNode { label: "b", next: None };
    let n1 = SNode { label: "a", next: Some(n2) };
    ()
}
"#;
    assert_aot_success(source, "recursive_codegen_struct_with_str_payload_builds");
}

/// payload-list: recursive struct field alongside a `[int]` payload. Pins
/// that the fix preserves the adjacent collection field (its buffer RC +
/// element cleanup) when the recursive back-edge is boxed. Aborts at
/// codegen pre-fix.
#[test]
fn recursive_codegen_struct_with_list_payload_builds() {
    let source = r#"
type LNode = { data: [int], next: Option<LNode> }

@main () -> void = {
    let n2 = LNode { data: [3, 4], next: None };
    let n1 = LNode { data: [1, 2], next: Some(n2) };
    ()
}
"#;
    assert_aot_success(source, "recursive_codegen_struct_with_list_payload_builds");
}

/// payload-nested-option: the recursive `Node` back-edge nested behind an
/// extra `Option` layer (`Option<Option<Node>>`). Pins that the fix
/// handles the recursive type even when an outer binding wraps it in a
/// second optional layer. Aborts at codegen pre-fix.
#[test]
fn recursive_codegen_nested_option_node_builds() {
    let source = r#"
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n2 = Node { value: 2, next: None };
    let n1 = Node { value: 1, next: Some(n2) };
    let nested: Option<Option<Node>> = Some(Some(n1));
    ()
}
"#;
    assert_aot_success(source, "recursive_codegen_nested_option_node_builds");
}

/// deep-nesting: an 8-node `Node` chain. Exercises the box-and-load fix
/// across a deep recursive back-edge plus the matching deep drop
/// traversal at scope exit; the `ORI_CHECK_LEAKS=1` oracle confirms every
/// node in the chain is released post-fix. Aborts at codegen pre-fix.
#[test]
fn recursive_codegen_deep_chain_eight_levels_builds() {
    let source = r#"
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n8 = Node { value: 8, next: None };
    let n7 = Node { value: 7, next: Some(n8) };
    let n6 = Node { value: 6, next: Some(n7) };
    let n5 = Node { value: 5, next: Some(n6) };
    let n4 = Node { value: 4, next: Some(n5) };
    let n3 = Node { value: 3, next: Some(n4) };
    let n2 = Node { value: 2, next: Some(n3) };
    let n1 = Node { value: 1, next: Some(n2) };
    ()
}
"#;
    assert_aot_success(source, "recursive_codegen_deep_chain_eight_levels_builds");
}

/// semantic-pin: build a chain, then READ a field through the boxed
/// back-edge (`n1.next.unwrap().value`) and `assert_eq` the value. This
/// ONLY passes once the root defect is fixed: pre-fix it aborts at codegen
/// (red), and the value-bearing assertion makes it the permanent
/// regression guard that the box-and-load fix preserves the projected
/// value through the recursive back-edge.
#[test]
fn recursive_codegen_read_through_back_edge_returns_value() {
    let source = r#"
use std.testing { assert_eq }
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n2 = Node { value: 2, next: None };
    let n1 = Node { value: 1, next: Some(n2) };
    let inner = n1.next.unwrap();
    assert_eq(actual: inner.value, expected: 2);
    ()
}
"#;
    assert_aot_success(
        source,
        "recursive_codegen_read_through_back_edge_returns_value",
    );
}
