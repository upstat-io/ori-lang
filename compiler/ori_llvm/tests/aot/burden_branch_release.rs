//! RL-4 per-edge release matrix for branch-exclusive terminal-move
//! lineages: a FRESH local consumed at an owned position (struct-field
//! store, owned call-arg, map-literal value) on a strict subset of branch
//! paths, with a non-consuming (borrow / no-use) sibling path. Per RL-1
//! `RL1_duplication_balanced` + RL-2 `RL2_release_exactly_once` + RL-4
//! `RL4_edge_release_balanced`, the pre-branch fresh-site funding inc must
//! have a matched release on EVERY path — the non-consuming edge owes
//! exactly one release. Missing edge release surfaces as exit 2 under
//! `ORI_CHECK_LEAKS=1` (harness default).
//!
//! Matrix axes: consume-kind [struct store, owned call-arg, map-literal
//! value] x control flow [if/else, match 2-arm, match 3-arm,
//! loop-inside-branch] x post-merge source read [none, borrow-read].
//! Primary if/else store pin lives in `burden_store_dup.rs`
//! (`test_branch_exclusive_store_no_kept_inc_no_leak`); both-paths-consume
//! if/else green clamp lives in `burden_dup_inc.rs`
//! (`test_list_branch_exclusive_alias_no_kept_inc_no_leak`); the funded
//! store-dup branch-exclusive DECLINE unit pin
//! (`funded_store_dup_declines_branch_exclusive_aliases`) lives in
//! `ori_arc` `lower/burden_lower/tests.rs`.
//!
//! RED today (BUG-04-176): branch-exclusive owned-push-arg, map-literal
//! store, match 2-arm both-paths-store (leaks on BOTH calls — the match /
//! Switch lowering leaks even on storing arms, a wider facet than the
//! if/else shape), match 3-arm exclusive store (every call leaks), and
//! loop-inside-branch per-iteration store. GREEN clamps (must stay green
//! through the cure): the post-merge borrow-read cells — reading the
//! source after the merge makes the store a genuine duplication, funded
//! correctly today; the cure's edge release must not fire there (the read
//! must keep observing the birth reference). Every cell is
//! interpreter-verified. Spec: Annex E §AIMS RL-1 + RL-2 + RL-4.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_cell_output;

// ----- RED cells (fail today; GREEN after the cure, unmodified) -----

/// Owned-call-arg consume-kind cell: fresh local consumed by a COW `push`
/// (owned receiver) in exactly ONE branch, borrowed (`len`) on the other.
/// The non-consuming path leaks the pre-branch funding inc (1 leak,
/// exit 2 today). Interpreter prints a=4 b=13.
#[test]
fn test_branch_exclusive_owned_push_arg_no_leak() {
    assert_cell_output(
        r#"
@push_one (flag: bool) -> int = {
    let base = [1, 2, 3];
    if flag then {
        let f = base.push(4);

        f.len()
    } else {
        base.len() + 10
    }
}

@main () -> void = {
    let a = push_one(flag: true);
    let b = push_one(flag: false);
    print(msg: `a={a} b={b}`)
}
"#,
        "branch_exclusive_owned_push_arg",
        "a=4 b=13",
    );
}

/// Map-literal consume-kind cell: fresh local stored as a `{str: [int]}`
/// literal VALUE in exactly ONE branch, borrowed on the other. Same
/// per-edge under-release through the map Set.value consume position
/// (1 leak, exit 2 today). Interpreter prints a=3 b=13.
#[test]
fn test_branch_exclusive_map_literal_store_no_leak() {
    assert_cell_output(
        r#"
@map_one (flag: bool) -> int = {
    let base = [1, 2, 3];
    if flag then {
        let m = {"k": base};

        (m["k"] ?? []).len()
    } else {
        base.len() + 10
    }
}

@main () -> void = {
    let a = map_one(flag: true);
    let b = map_one(flag: false);
    print(msg: `a={a} b={b}`)
}
"#,
        "branch_exclusive_map_literal_store",
        "a=3 b=13",
    );
}

/// Match 2-arm BOTH-paths-store cell: every arm consumes the fresh local
/// at an owned store, so the per-path ledger should balance with NO edge
/// release owed (the if/else both-consume sibling in `burden_dup_inc.rs`
/// is green). LEAKS TODAY on BOTH calls (2 leaks, exit 2) — the match /
/// Switch lowering under-releases even on storing arms, pinning the
/// Switch facet independently of the branch-exclusive partition.
/// Interpreter prints a=3 b=13.
#[test]
fn test_match_two_arm_both_paths_store_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@store_both (flag: bool) -> int = {
    let base = [1, 2, 3];
    match flag {
        true -> {
            let h = Holder { kept: base };

            h.kept.len()
        },
        false -> {
            let h = Holder { kept: base };

            h.kept.len() + 10
        },
    }
}

@main () -> void = {
    let a = store_both(flag: true);
    let b = store_both(flag: false);
    print(msg: `a={a} b={b}`)
}
"#,
        "match_two_arm_both_paths_store",
        "a=3 b=13",
    );
}

/// Match 3-arm exclusive-store cell: arm 0 stores, arm 1 borrows, arm 2
/// never uses the local. EVERY call leaks today (3 leaks, exit 2) —
/// including the STORING arm, which the if/else shape balances; the
/// storing-arm leak is the Switch-lowering facet beyond the per-edge
/// partition. Interpreter prints a=3 b=13 c=42.
#[test]
fn test_match_three_arm_exclusive_store_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@store_arm (n: int) -> int = {
    let base = [1, 2, 3];
    match n {
        0 -> {
            let h = Holder { kept: base };

            h.kept.len()
        },
        1 -> base.len() + 10,
        _ -> 42,
    }
}

@main () -> void = {
    let a = store_arm(n: 0);
    let b = store_arm(n: 1);
    let c = store_arm(n: 2);
    print(msg: `a={a} b={b} c={c}`)
}
"#,
        "match_three_arm_exclusive_store",
        "a=3 b=13 c=42",
    );
}

/// Loop-inside-branch cell: the consuming branch stores the loop-invariant
/// fresh local into a holder EVERY iteration (genuine per-iteration
/// duplication — the source survives into the next iteration), sibling
/// path borrows. Leaks today (2 leaks, exit 2). The cure's per-path
/// terminality discriminator must price the loop-carried consumes
/// correctly — neither a spurious edge release on the looping path nor a
/// retained surplus on the borrow path. Interpreter prints a=6 b=13.
#[test]
fn test_loop_inside_branch_per_iteration_store_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@loop_in_branch (flag: bool) -> int = {
    let base = [1, 2, 3];
    if flag then {
        let total = 0;
        for i in 0..3 do {
            let h = Holder { kept: base };
            total = total + h.kept[i];
        };

        total
    } else {
        base.len() + 10
    }
}

@main () -> void = {
    let a = loop_in_branch(flag: true);
    let b = loop_in_branch(flag: false);
    print(msg: `a={a} b={b}`)
}
"#,
        "loop_inside_branch_per_iteration_store",
        "a=6 b=13",
    );
}

// ----- GREEN post-merge borrow-read clamps (must stay GREEN) -----

/// If/else store + POST-MERGE borrow-read: reading the source after the
/// merge makes the branch store a genuine duplication (not a terminal
/// move), funded correctly today — GREEN. The cure's edge release must
/// NOT fire here: it would release the birth reference the post-merge
/// read needs (a wrong-direction fix shows as freed-memory read, wrong
/// value, or SIGSEGV). Interpreter prints a=6 b=16.
#[test]
fn test_branch_store_post_merge_read_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@store_then_read (flag: bool) -> int = {
    let base = [1, 2, 3];
    let r = if flag then {
        let h = Holder { kept: base };

        h.kept.len()
    } else {
        base.len() + 10
    };

    r + base.len()
}

@main () -> void = {
    let a = store_then_read(flag: true);
    let b = store_then_read(flag: false);
    print(msg: `a={a} b={b}`)
}
"#,
        "branch_store_post_merge_read",
        "a=6 b=16",
    );
}

/// Match 3-arm store + POST-MERGE borrow-read: the Switch sibling of the
/// clamp above — GREEN today even though the read-free 3-arm cell leaks
/// on every arm; the post-merge read flips the lineage to a funded
/// genuine duplication. Guards the duplicate-vs-birth-reference placement
/// on the Switch facet. Interpreter prints a=6 b=16 c=45.
#[test]
fn test_match_three_arm_store_post_merge_read_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@store_arm_read (n: int) -> int = {
    let base = [1, 2, 3];
    let r = match n {
        0 -> {
            let h = Holder { kept: base };

            h.kept.len()
        },
        1 -> base.len() + 10,
        _ -> 42,
    };

    r + base.len()
}

@main () -> void = {
    let a = store_arm_read(n: 0);
    let b = store_arm_read(n: 1);
    let c = store_arm_read(n: 2);
    print(msg: `a={a} b={b} c={c}`)
}
"#,
        "match_three_arm_store_post_merge_read",
        "a=6 b=16 c=45",
    );
}
