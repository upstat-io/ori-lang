//! AOT tests for recursive-type drop emission (§04.1 of
//! `plans/aims-burden-tracking/section-04-recursive-closures-drop-value.md`).
//!
//! These tests exercise the END-TO-END recursive-drop story: SCC-based
//! cycle detection (registry-side) + per-type compiled drop-glue (codegen
//! side) + `ori_rc_dec` invocation at refcount-zero (runtime side).
//!
//! Pre-existing blocker: `BUG-04-043` — "Recursive tagged-pointer enums
//! need box-and-load codegen for `Construct`/`Project` (§07.3.A future
//! work)" prevents recursive struct/enum types from passing through LLVM
//! codegen. Surface symptom on `type Node = { value: int, next:
//! Option<Node> }`: `build_struct: insert_value failed (index out of
//! bounds?) index=0 num_fields=0`. Both AOT tests below cite
//! `BUG-04-043` in their `#[ignore]` reason per
//! `.claude/rules/test-disposition.md`.
//!
//! Until `BUG-04-043` ships, the §04.1 algorithmic deliverables (SCC
//! detection + `compiled_drop` population + cache-before-body cycle
//! safety) are pinned by:
//! - `ori_types::registry::burden_compose::scc::tests` — 15-test matrix
//!   over self-loops, mutually-recursive pairs/triples, non-recursive
//!   baselines, and decision-rule clauses.
//! - `ori_llvm::codegen::arc_emitter::tests::{recursive_node_drop_fn_emits_self_referencing_rc_dec,
//!   mutually_recursive_tree_forest_drop_fns_cross_reference,
//!   drop_fn_cache_prevents_infinite_generation}` — codegen-IR-level
//!   verification of the cache cycle-safety pattern at `drop_gen.rs:69`.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// Regression: §04.1 deliverable for recursive-type drop emission.
/// Verifies a 3-node linked list traverses recursive drop without leak
/// at scope exit. `ORI_CHECK_LEAKS=1` reports zero leaks; `ORI_TRACE_RC=1`
/// shows three matching alloc/dec pairs.
///
/// Blocked by `BUG-04-043` — recursive struct types require box-and-load
/// codegen for `Construct`/`Project` at LLVM level. Pin the test now;
/// re-enable in the same commit that closes `BUG-04-043`.
#[test]
#[ignore = "BUG-04-043: recursive struct types not yet supported by LLVM codegen (Construct/Project box-and-load)"]
fn test_recursive_node_drop_chain() {
    let source = r#"
type Node = { value: int, next: Option<Node> }

@t tests _ () -> void = {
    let n3 = Node { value: 3, next: None };
    let n2 = Node { value: 2, next: Some(n3) };
    let n1 = Node { value: 1, next: Some(n2) };
    ()
}
"#;
    assert_aot_success(source, "recursive_node_drop_chain");
}

/// Regression: §04.1 shared-reference pin per `success_criterion` 2.
/// Verifies the refcount-zero branch of `ori_rc_dec` MUST NOT fire on
/// `n1`'s heap node while `n1_alias` still holds rc = 1. Recursive
/// compiled drop body is invoked only at the FINAL release (rc -> 0).
///
/// Blocked by `BUG-04-043` — same root cause as the chain test above.
#[test]
#[ignore = "BUG-04-043: recursive struct types not yet supported by LLVM codegen (Construct/Project box-and-load)"]
fn recursive_drop_skips_body_when_rc_above_one() {
    let source = r#"
use std.testing { assert_eq }
type Node = { value: int, next: Option<Node> }

@t tests _ () -> void = {
    let n3 = Node { value: 3, next: None };
    let n2 = Node { value: 2, next: Some(n3) };
    let n1 = Node { value: 1, next: Some(n2) };
    let n1_alias = n1;
    drop_early(value: n1);
    assert_eq(actual: n1_alias.value, expected: 1);
}
"#;
    assert_aot_success(source, "recursive_drop_skips_body_when_rc_above_one");
}
