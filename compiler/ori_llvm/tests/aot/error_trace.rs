//! AOT RC-balance pin for `?`-hop trace injection on a HEAP (non-SSO) `Error`
//! message (`error_trace_heap_msg.ori`'s Rust-level sibling).
//!
//! SSO messages (<=23 chars) never allocate a heap buffer, so they cannot
//! exercise the transferred `message` fat-pointer path (`_ori_inject_trace_entry`
//! passes `function`/`file` by pointer since a 24-byte `OriStr` exceeds the
//! `SysV` ABI's 16-byte direct-passing threshold); `assert_aot_success`'s
//! `ORI_CHECK_LEAKS=1` oracle flags a leaked/double-freed buffer as a non-zero exit.
//!
//! Reads the `Result` delegation receiver directly (never `match`-extracted,
//! never mixing one-hop/two-hop bindings in one fn) — a `match`-extracted
//! receiver and mixed one-hop/two-hop bindings each trip their own orthogonal
//! RC defect on this path, tracked separately.

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
/// (>23 char) `Error` messages, read back via `.trace_entries().len()`.
/// Each `two_hop()` call COW-pushes twice on the same `Error`, atop the
/// first-push allocation `one_hop()` covers.
///
/// One accessor call per binding: mixing `.trace_entries()` and `.trace()`
/// on sibling two-hop bindings reproduces a leak tracked separately; this
/// cell stays on the confirmed-clean same-accessor shape instead.
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
