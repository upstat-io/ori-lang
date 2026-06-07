//! AOT tests for recursive-type drop emission.
//!
//! These tests exercise the END-TO-END recursive-drop story: SCC-based
//! cycle detection (registry-side) + per-type compiled drop-glue (codegen
//! side) + `ori_rc_dec` invocation at refcount-zero (runtime side).
//!
//! `assert_aot_success`'s built-in `ORI_CHECK_LEAKS=1` oracle verifies the
//! recursive drop balances allocation/deallocation; the shared-reference pin
//! additionally inspects the `ORI_TRACE_RC=1` event stream to verify the
//! recursive drop body fires only at the final refcount-zero release.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_with_env};

/// Canonical recursive value-type repro: a 3-node `Node` linked list built in `@main`
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

/// Cross-case composition pin: a recursive `Node` type that
/// ALSO carries a user `@drop` impl. This composes §04.1 (SCC-based recursive
/// drop-glue: the `_ori_drop$Node` body traverses the self-referencing `next`
/// chain) with §04.3 (Drop AUGMENT: the user `@drop (self)` body runs FIRST,
/// then the compiler walks the owned fields). §04.1 and §04.3 each verify in
/// isolation; this pin verifies their composition on the SAME type.
///
/// The `ORI_CHECK_LEAKS=1` oracle (built into `assert_aot_success`) is the
/// composition check: a chain built then dropped at scope exit must reclaim
/// every node exactly once. A composition defect surfaces as either a leak
/// (`user_drop` ran but the recursive child-walk was skipped → exit code 2) or a
/// double-free abort (children traversed more than once). Clean exit proves
/// the AUGMENT `user_drop` + SCC `compiled_drop` compose: drop body runs once per
/// node, children traversed once, no double-free.
///
/// Regression: recursive-type × Drop-AUGMENT cross-case composition.
#[test]
fn recursive_drop_with_user_drop_composes_no_double_free() {
    let source = r#"
type Node = { value: int, next: Option<Node> }

impl Node: Drop {
    @drop (self) -> void = ();
}

@main () -> void = {
    let n3 = Node { value: 3, next: None };
    let n2 = Node { value: 2, next: Some(n3) };
    let n1 = Node { value: 1, next: Some(n2) };
    ()
}
"#;
    assert_aot_success(
        source,
        "recursive_drop_with_user_drop_composes_no_double_free",
    );
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
/// balance invariant once the recursive value-type defect is fixed.)
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

/// Shared-reference pin: the recursive compiled drop body fires ONLY at the
/// final refcount-zero release, never on an `rc > 1` decrement.
///
/// `n1_alias = n1` bumps the head node's rc to 2; `drop_early(value: n1)`
/// shortens `n1`'s lifetime, dropping the rc from 2 -> 1 WITHOUT invoking the
/// recursive `_ori_drop$Node` body (the chain stays alive). The `n1_alias`
/// scope-exit dec then reaches rc 0 and the recursive body traverses
/// `n1_alias` -> n2 -> n3.
///
/// Verifies both axes from the §04.1 success criterion:
///   1. Positive — the chain is fully reclaimed at final rc=0 (`ORI_CHECK_LEAKS`
///      zero-leak oracle + an observed `rc=0 FREE` event in the trace).
///   2. Negative — the `drop_early` dec lands at `rc=1` (non-zero) BEFORE any
///      `FREE`, proving the recursive drop body was skipped while shared.
///
/// Regression: shared-reference correctness — `rc > 1` release must decrement
/// without compiled-drop-body invocation.
/// See BUG-02-032 (`drop_early` prelude builtin) — this pin
/// is RED until that builtin is implemented (typeck signature + eval semantics
/// + AOT early `rc_dec` codegen); it goes green with no edit once it lands.
#[test]
#[ignore = "BUG-02-032: drop_early prelude builtin not yet implemented (E2003 unknown identifier) — blocks the rc-above-one drop-skip pin"]
fn recursive_drop_skips_body_when_rc_above_one() {
    // drop_early surface per spec/08-types.md §8.10.5 (`@drop_early<T> (value: T) -> void`).
    let source = r#"
use std.testing { assert_eq }
type Node = { value: int, next: Option<Node> }

@main () -> void = {
    let n3 = Node { value: 3, next: None };
    let n2 = Node { value: 2, next: Some(n3) };
    let n1 = Node { value: 1, next: Some(n2) };
    let n1_alias = n1;
    drop_early(value: n1);
    assert_eq(actual: n1_alias.value, expected: 1);
    ()
}
"#;

    let (exit_code, _stdout, stderr) = compile_and_run_with_env(source, &[("ORI_TRACE_RC", "1")]);

    // Compilation + run must succeed; exit code 2 = leak, 1 = panic (assert_eq),
    // -1 = compile failure. The ORI_CHECK_LEAKS oracle (always on) covers axis 1's
    // zero-leak half.
    assert_eq!(
        exit_code, 0,
        "recursive_drop_skips_body_when_rc_above_one did not run clean \
         (exit {exit_code}); leak/panic/compile-failure. stderr:\n{stderr}"
    );

    // RC dec events from ori_rt::rc::debug: `[RC] dec <ptr> → rc=N (...)` for a
    // surviving decrement, `[RC] dec <ptr> → rc=0 FREE (...)` at reclamation.
    let dec_events: Vec<&str> = stderr.lines().filter(|l| l.contains("[RC] dec")).collect();
    assert!(
        !dec_events.is_empty(),
        "expected ORI_TRACE_RC dec events; none captured. stderr:\n{stderr}"
    );

    // Axis 1 (positive): the chain reaches final rc=0 — at least one FREE event.
    let free_events = dec_events.iter().filter(|l| l.contains("FREE")).count();
    assert!(
        free_events >= 1,
        "axis 1: expected at least one `rc=0 FREE` dec (final reclamation of the \
         recursive chain), found none. dec events:\n{}",
        dec_events.join("\n")
    );

    // Axis 2 (negative): the FIRST dec event is a surviving `rc=N` (non-FREE)
    // decrement — that is the `drop_early(value: n1)` dropping rc 2 -> 1 while
    // n1_alias still holds the chain. The recursive `_ori_drop$Node` body must
    // NOT fire here; a FREE as the first dec would mean the shared node was
    // reclaimed while still aliased (double-free / use-after-free of the chain).
    let first_dec = dec_events[0];
    assert!(
        !first_dec.contains("FREE"),
        "axis 2: the first dec (drop_early on the shared head node) must NOT \
         reach rc=0 / invoke the recursive drop body while n1_alias is live; \
         a FREE as the first dec violates shared-reference correctness. \
         first dec: {first_dec}\nall dec events:\n{}",
        dec_events.join("\n")
    );
}
