//! AOT RC-balance pin for `?`-hop trace injection on a HEAP (non-SSO) `Error`
//! message (TDD matrix M9, `error_trace_heap_msg.ori`'s Rust-level sibling).
//!
//! SSO messages (<=23 chars) never allocate a heap buffer, so they cannot
//! exercise the transferred `message` fat-pointer path Gap D's ABI fix
//! corrected (`_ori_inject_trace_entry`'s `function`/`file` args must pass by
//! pointer, not by value, per `codegen-rules.md` AB-1). `assert_aot_success`'s
//! built-in `ORI_CHECK_LEAKS=1` oracle is the RC-balance check: a leaked or
//! double-freed message/trace buffer surfaces as a non-zero exit.
//!
//! Accessor calls here read directly off the `Result` delegation receiver
//! (never a `match`-extracted `Err(e)`, and never mixing a one-hop binding
//! with a two-hop binding in one function) — see BUG-04-246 (RC leak) and
//! BUG-04-247 (LLVM verification failure) for the two orthogonal defects
//! those shapes trip, discovered authoring this cell and tracked separately.

use crate::util::assert_aot_success;

/// Two independent single-`?`-hop trace injections on heap-allocated
/// (>23 char) `Error` messages read back through `.trace_entries().len()`.
#[test]
fn error_trace_heap_message_repeated_one_hop_no_leak() {
    let source = r#"
@fail_heap () -> Result<int, Error> =
    Err(Error("a heap error message over 23 chars"));

@one_hop () -> Result<int, Error> = {
    let x = fail_heap()?;
    Ok(x)
}

@main () -> void = {
    let r1 = one_hop();
    let n1 = r1.trace_entries().len();
    let r2 = one_hop();
    let n2 = r2.trace_entries().len();
    print(msg: `{n1} {n2}`)
}
"#;
    assert_aot_success(source, "error_trace_heap_message_repeated_one_hop_no_leak");
}

/// Two independent chained 2-hop trace injections on heap-allocated
/// (>23 char) `Error` messages, read back through `.trace_entries().len()`.
/// Each `two_hop()` call exercises `_ori_inject_trace_entry`'s COW-push
/// twice on the same `Error` (growing an already-populated trace list), in
/// addition to the first-push allocation `one_hop()` covers.
///
/// Deliberately narrow to a single accessor call per binding: combining
/// `.trace_entries()` and `.trace()` reads on sibling two-hop bindings in
/// one function reproduces a leak tracked separately under BUG-04-246 — the
/// isolation matrix there documents the wider accessor-combination family;
/// this cell stays on the confirmed-clean shape to pin Gap D's own RC
/// concern without re-encoding that unrelated defect as a passing test.
#[test]
fn error_trace_heap_message_repeated_two_hop_no_leak() {
    let source = r#"
@fail_heap () -> Result<int, Error> =
    Err(Error("a heap error message over 23 chars"));

@one_hop () -> Result<int, Error> = {
    let x = fail_heap()?;
    Ok(x)
}

@two_hop () -> Result<int, Error> = {
    let x = one_hop()?;
    Ok(x)
}

@main () -> void = {
    let r1 = two_hop();
    let n1 = r1.trace_entries().len();
    let r2 = two_hop();
    let n2 = r2.trace_entries().len();
    print(msg: `{n1} {n2}`)
}
"#;
    assert_aot_success(source, "error_trace_heap_message_repeated_two_hop_no_leak");
}
