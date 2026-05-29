//! AOT tests for recursive-type drop emission (BUG-04-043).
//!
//! These tests exercise the END-TO-END recursive-drop story: SCC-based
//! cycle detection (registry-side) + per-type compiled drop-glue (codegen
//! side) + `ori_rc_dec` invocation at refcount-zero (runtime side).
//!
//! Root defect (BUG-04-043): recursive struct/enum value types fail AOT
//! LLVM codegen because the recursive back-edge is laid out by-value,
//! producing an infinitely-sized / zero-field LLVM struct. Surface symptom
//! on `type Node = { value: int, next: Option<Node> }`:
//! `build_struct: insert_value failed (index out of bounds?) index=0
//! num_fields=0` followed by `error[E5001]: LLVM module verification
//! failed`. The fix is box-and-load codegen for `Construct`/`Project` of
//! the recursive back-edge.
//!
//! These tests are TDD pins authored BEFORE the fix — every one currently
//! aborts with the recursive-codegen signature above. They pass only once
//! BUG-04-043 closes (box-and-load for the recursive back-edge), at which
//! point `assert_aot_success`'s built-in `ORI_CHECK_LEAKS=1` oracle also
//! verifies the recursive drop balances allocation/deallocation.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// Canonical BUG-04-043 repro: a 3-node `Node` linked list built in `@main`
/// must compile, run, and drop the recursive chain without leaking at scope
/// exit. `assert_aot_success` enables `ORI_CHECK_LEAKS=1`, so a leaked node
/// surfaces as exit code 2 and fails the test.
#[test]
fn recursive_drop_node_chain_builds_runs_no_leak() {
    let source = r#"
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n3 = Node { value: 3, next: None };
    let n2 = Node { value: 2, next: Some(n3) };
    let n1 = Node { value: 1, next: Some(n2) };
    ()
}
"#;
    assert_aot_success(source, "recursive_drop_node_chain_builds_runs_no_leak");
}

/// Aliased recursive value: `let a = node; let b = a;` keeps two live
/// bindings to the same heap chain through scope exit, then reads
/// `b.value`. Pins shared-rc drop behavior — the final release (rc -> 0)
/// drops the recursive chain exactly once, with no double-free. The
/// `ORI_CHECK_LEAKS=1` oracle catches both a leak (exit 2) and a
/// double-free abort.
#[test]
fn recursive_drop_aliased_no_double_free() {
    let source = r#"
use std.testing { assert_eq }
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n3 = Node { value: 3, next: None };
    let n2 = Node { value: 2, next: Some(n3) };
    let n1 = Node { value: 1, next: Some(n2) };
    let a = n1;
    let b = a;
    assert_eq(actual: b.value, expected: 1);
    ()
}
"#;
    assert_aot_success(source, "recursive_drop_aliased_no_double_free");
}

/// Trace-balance pin: a 5-node recursive `Node` chain whose drop must
/// balance inc/dec across the whole chain. The `ORI_CHECK_LEAKS=1` oracle
/// built into `assert_aot_success` is the balance check — any unmatched
/// allocation leaks and surfaces as exit code 2. (An explicit
/// `ORI_TRACE_RC` capture is not possible while the program is RED, because
/// codegen aborts before a binary is linked; the leak oracle covers the
/// balance invariant once BUG-04-043 closes.)
#[test]
fn recursive_drop_chain_rc_balanced() {
    let source = r#"
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n5 = Node { value: 5, next: None };
    let n4 = Node { value: 4, next: Some(n5) };
    let n3 = Node { value: 3, next: Some(n4) };
    let n2 = Node { value: 2, next: Some(n3) };
    let n1 = Node { value: 1, next: Some(n2) };
    ()
}
"#;
    assert_aot_success(source, "recursive_drop_chain_rc_balanced");
}
