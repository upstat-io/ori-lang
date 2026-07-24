//! RL-5 release for a purely-dead loop-invariant fresh-collection local.
//!
//! `let root = [1]; let xs = []; for i in 0..N do { xs = xs.push(i) }; xs[k]`
//! constructs `root` (a fresh `[int]` buffer), threads it UNCHANGED through the
//! loop's block-params, and NEVER reads it. The loop back-edge fractures the
//! union-find lineage, so `compute_construct_fed_dead_param_lineage`'s
//! fresh-collection gate declines `root` and no RL-5 release is emitted -> the
//! buffer leaks (alloc +1, all burden ops are balanced keep-alive pairs).
//!
//! `compute_loop_invariant_dead_local_releases` recognizes the purely-dead
//! threaded-only Construct directly and emits ONE RL-5 dead-at-entry release at
//! the terminal dead block-param (governed by
//! `AimsProof.Realization::RL5_dead_at_entry_cleanup` and `RL5_cleanup_balanced`).
//! All cells run under `ORI_CHECK_LEAKS=1` and are interpreter-verified via
//! `assert_cell_output`. Spec: Annex E §AIMS RL-5.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_cell_output;

/// The cured shape: a dead loop-invariant `[int]` local (`root`) alongside a
/// loop-built `xs`. `root` is threaded through the loop block-params and never
/// read; without the RL-5 release it leaks one allocation. Interpreter prints
/// val=128.
const DEAD_ROOT_LOOP_SRC: &str = r#"
@main () -> void = {
    let root = [1];
    let xs: [int] = [];
    for i in 0..200 do { xs = xs.push(i) };
    let val = xs[128];
    print(msg: `val={val}`)
}
"#;

#[test]
fn test_dead_loop_invariant_int_list_local_no_leak() {
    assert_cell_output(
        DEAD_ROOT_LOOP_SRC,
        "dead_loop_invariant_int_list_local",
        "val=128",
    );
}

/// GREEN clamp: a loop-invariant local that IS read after the loop is NOT this
/// family (it has a real last-use the base walk releases). The cure's
/// purely-dead discriminator must DECLINE it (over-firing here would emit a
/// release the base walk also emits -> double-free). Interpreter prints
/// root0=7 val=128.
#[test]
fn test_read_loop_invariant_local_not_admitted_no_double_free() {
    assert_cell_output(
        r#"
@main () -> void = {
    let root = [7];
    let xs: [int] = [];
    for i in 0..200 do { xs = xs.push(i) };
    let val = xs[128];
    print(msg: `root0={root[0]} val={val}`)
}
"#,
        "read_loop_invariant_local_not_admitted",
        "root0=7 val=128",
    );
}
