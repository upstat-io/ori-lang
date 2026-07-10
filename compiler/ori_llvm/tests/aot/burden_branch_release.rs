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
//! Cure surface: the Phase-5 RL-4 per-edge release
//! (`compute_branch_exclusive_edge_releases`, toggle
//! `ORI_DISABLE_BRANCH_EXCLUSIVE_EDGE_RELEASE=1`) cures the if/else
//! branch-exclusive cells (owned-push-arg, map-literal store, the primary
//! struct-store pin in `burden_store_dup.rs`) and supplies the dead-on-edge
//! birth release for fully-dead no-use arms; the Switch/match cells are
//! cured by the `lower_match` merge-param pruning + the construct-fed RL-5
//! ALT-CONSUMED mode (`burden_match_release.rs` toggle pins). GREEN clamps:
//! the post-merge borrow-read cells — reading the source after the merge
//! makes the store a genuine funded duplication; the edge release declines
//! there (the read keeps observing the birth reference). RESIDUAL
//! (BUG-04-180): the loop shape threads the lineage through loop
//! block-params into a dead loop-exit param whose feeding rep fractures
//! from the Construct root — that cell stays ignored-with-reason below.
//! Every cell is interpreter-verified. Spec: Annex E §AIMS RL-1 + RL-2 +
//! RL-4 + RL-5.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_cell_output;

/// Owned-call-arg if/else cell source — shared by the GREEN cell and the
/// `ORI_DISABLE_BRANCH_EXCLUSIVE_EDGE_RELEASE` toggle-parity pin.
const BRANCH_EXCLUSIVE_PUSH_SRC: &str = r#"
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
"#;

/// Owned-call-arg consume-kind cell: fresh local consumed by a COW `push`
/// (owned receiver) in exactly ONE branch, borrowed (`len`) on the other.
/// The non-consuming path owes the per-edge release of the pre-branch
/// funding inc. Interpreter prints a=4 b=13.
#[test]
fn test_branch_exclusive_owned_push_arg_no_leak() {
    assert_cell_output(
        BRANCH_EXCLUSIVE_PUSH_SRC,
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
/// at an owned store, so the per-path ledger balances with NO edge
/// release owed (the if/else both-consume sibling in `burden_dup_inc.rs`
/// is the Branch twin). The merge-param pruning removes the dead
/// block-param `base` used to ride into; the funded stores balance the
/// rest. Interpreter prints a=3 b=13.
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
/// never uses the local. Post-pruning the arms carry their own releases:
/// the storing arm's funded store, the borrow arm's last-use dec + edge
/// release, and the no-use arm's dead-on-edge pair (birth dec + edge
/// release). Interpreter prints a=3 b=13 c=42.
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
/// fresh local into a holder EVERY iteration (genuine per-iteration funded
/// duplication — the source survives into the next iteration), sibling
/// path borrows. Leaks on BOTH calls (2 leaks, exit 2): the `for` lowering
/// threads `%base` through the loop block-params and BOTH paths hand it
/// via `Jump` args into a DEAD merge block-param (`bb3(%46: int,
/// %47: [int])`, `%47` used nowhere) — the SAME dead-block-param root as
/// the Switch cells (proven by `ORI_DUMP_AFTER_BURDEN=1` dumps), not a
/// per-edge partition gap; the edge release correctly declines both edges
/// (Jump-arg consumes forward of each). Interpreter prints a=6 b=13.
#[test]
#[ignore = "BUG-04-180: loop lowering threads the loop-invariant heap local through loop block-params into a dead merge/exit block-param the RL-5 dead-param scans cannot resolve (rep fractures across the Jump-arg hop)"]
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

// ----- Switch/Branch lowering-boundary cell -----

/// User-defined 2-variant-enum match, branch-exclusive store: the exact
/// lowering boundary between the bool if/else pin (Branch — cured by the
/// per-edge release) and the 2-arm `match` cells (Switch — cured by the
/// merge-param pruning). ANY `match` — including a bool scrutinee and a
/// 2-variant enum — lowers to `Switch`; the boundary is `if/else` (Branch)
/// vs `match` (Switch), NOT bool vs enum. Interpreter prints a=3 b=13.
#[test]
fn test_enum_two_variant_match_exclusive_store_no_leak() {
    assert_cell_output(
        r#"
type Flag = On | Off;
type Holder = { kept: [int] }

@store_enum (f: Flag) -> int = {
    let base = [1, 2, 3];
    match f {
        On -> {
            let h = Holder { kept: base };

            h.kept.len()
        },
        Off -> base.len() + 10,
    }
}

@main () -> void = {
    let a = store_enum(f: On);
    let b = store_enum(f: Off);
    print(msg: `a={a} b={b}`)
}
"#,
        "enum_two_variant_match_exclusive_store",
        "a=3 b=13",
    );
}

// ----- Toggle-parity semantic pins (compile-time env; subprocess-isolated) -----
