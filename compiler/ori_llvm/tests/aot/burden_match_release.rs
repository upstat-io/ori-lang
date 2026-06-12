//! RL-5 dead-merge-param release matrix for Switch/loop-lowered `match`:
//! `lower_match` threads EVERY in-scope mutable binding into the merge
//! block-params unconditionally (no divergence pruning like `lower_if`'s
//! `merge_mutable_vars`), so an unchanged fresh local rides every arm's
//! `Jump` into a DEAD merge block-param. Per RL-4 the Jump arg transfers
//! the RC obligation to the param; per RL-5 a dead-at-entry owned
//! non-scalar block-param owes an immediate release — no emission surface
//! supplies it for collection `Construct` roots whose lineage consumes
//! are all FUNDED (BUG-04-179). Each missing release surfaces as exit 2
//! under `ORI_CHECK_LEAKS=1` (harness default).
//!
//! Matrix axes: arm-consume shape [every-arm-store, struct partial-move,
//! no-store unchanged, loop-in-arm store, diverging reassignment, nested
//! match inner-store] x post-merge source read [none, borrow-read]. The
//! primary 2-arm/3-arm/loop-in-branch/enum-boundary cells live in
//! `burden_branch_release.rs` (same root, ignored with the same reason).
//!
//! Cure surfaces this matrix clamps: (B) `lower_match` adopting the
//! `merge_mutable_vars` divergence check so UNCHANGED bindings are pruned
//! from the merge params — the diverging-bindings and post-merge-read
//! cells pin that pruning never fires on changed or still-read bindings;
//! (A) a construct-fed RL-5 alt-consumed mode releasing the dead param
//! when EVERY lineage consume is funded — the struct partial-move cell
//! clamps its all-fields-funded DECLINE gate (an over-fire double-frees).
//! Every cell is interpreter-verified. Spec: Annex E §AIMS RL-1 + RL-2 +
//! RL-4 + RL-5.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_cell_output;

/// Zero-leak positive: a fresh local stored at an owned position on EVERY
/// arm, each arm into its own holder type. The per-arm funded store incs
/// balance against the holder drops, so the only unmatched ref is the
/// birth ref the `Jump` args transfer into the dead merge block-param.
/// Leaks on BOTH calls today (2 leaks, exit 2). Interpreter prints
/// a=3 b=13.
#[test]
#[ignore = "BUG-04-179: Switch-lowered match threads the stored fresh local into a dead merge block-param via Jump args; no RL-5 release for collection Constructs"]
fn test_match_every_arm_stores_fresh_local_no_leak() {
    assert_cell_output(
        r#"
type HolderA = { kept: [int] }
type HolderB = { kept: [int] }

@store_every (flag: bool) -> int = {
    let base = [1, 2, 3];
    match flag {
        true -> {
            let h = HolderA { kept: base };

            h.kept.len()
        },
        false -> {
            let h = HolderB { kept: base };

            h.kept.len() + 10
        },
    }
}

@main () -> void = {
    let a = store_every(flag: true);
    let b = store_every(flag: false);
    print(msg: `a={a} b={b}`)
}
"#,
        "match_every_arm_stores_fresh_local",
        "a=3 b=13",
    );
}

/// Struct partial-move-through-match NEGATIVE clamp: a struct whose
/// fields are PARTIALLY consumed at funded positions through the arms —
/// `left` stored into a holder (funded), `right` only borrowed. The
/// RL-5 alt-consumed cure must DECLINE this lineage (only SOME fields
/// funded); firing anyway releases a ref the funded-store machinery
/// still owns and double-frees (crash / wrong output, NOT a leak).
/// Recorded honestly: today the unchanged `p` rides the Jump args into
/// the dead merge param and LEAKS on both calls (2 leaks, exit 2) — the
/// cure path for `p` is the divergence pruning, never the alt-consumed
/// release. Interpreter prints a=5 b=13.
#[test]
#[ignore = "BUG-04-179: Switch-lowered match threads the unchanged struct local into a dead merge block-param via Jump args; cure must prune the param, NOT fire the alt-consumed release (partial-funded fields)"]
fn test_match_struct_partial_move_no_double_free() {
    assert_cell_output(
        r#"
type Pair = { left: [int], right: [int] }
type Holder = { kept: [int] }

@partial_move (flag: bool) -> int = {
    let p = Pair { left: [1, 2, 3], right: [4, 5] };
    match flag {
        true -> {
            let h = Holder { kept: p.left };

            h.kept.len() + p.right.len()
        },
        false -> p.left.len() + 10,
    }
}

@main () -> void = {
    let a = partial_move(flag: true);
    let b = partial_move(flag: false);
    print(msg: `a={a} b={b}`)
}
"#,
        "match_struct_partial_move",
        "a=5 b=13",
    );
}

/// Unchanged-variable GREEN clamp: a mutable local NOT modified in any
/// arm and NOT consumed by any arm, read after the merge. The threaded
/// merge param is LIVE (the post-merge read consumes it), so no RL-5
/// obligation is stranded — GREEN today (exit 0). This is the IR-bloat
/// shape the divergence pruning eliminates; the pin guards the pruning's
/// rewiring: after the param is pruned, the post-merge read must still
/// observe the original binding (wrong-direction pruning shows as a
/// freed-memory read, wrong value, or SIGSEGV). Interpreter prints
/// a=4 b=5.
#[test]
fn test_match_unchanged_local_post_merge_read_no_leak() {
    assert_cell_output(
        r#"
@unchanged_read (flag: bool) -> int = {
    let base = [1, 2, 3];
    let r = match flag {
        true -> 1,
        false -> 2,
    };

    r + base.len()
}

@main () -> void = {
    let a = unchanged_read(flag: true);
    let b = unchanged_read(flag: false);
    print(msg: `a={a} b={b}`)
}
"#,
        "match_unchanged_local_post_merge_read",
        "a=4 b=5",
    );
}

/// Loop-inside-match RC-balance cell: a `for` loop inside one arm stores
/// the loop-invariant fresh local into a holder EVERY iteration (genuine
/// per-iteration funded duplication) while mutating arm-local state; the
/// sibling arm borrows. Leaks on BOTH calls today (2 leaks, exit 2) —
/// the loop params thread `base` back around AND the arm's `Jump` hands
/// it to the dead match-merge param, composing the loop-threading shape
/// with the Switch shape in one cell. Interpreter prints a=6 b=13.
#[test]
#[ignore = "BUG-04-179: loop-in-arm lowering threads the stored fresh local through loop params and the arm Jump into a dead merge block-param; same dead-param root as the Switch cells"]
fn test_match_arm_loop_per_iteration_store_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@loop_in_match (n: int) -> int = {
    let base = [1, 2, 3];
    match n {
        0 -> {
            let total = 0;
            for i in 0..3 do {
                let h = Holder { kept: base };
                total = total + h.kept[i];
            };

            total
        },
        _ -> base.len() + 10,
    }
}

@main () -> void = {
    let a = loop_in_match(n: 0);
    let b = loop_in_match(n: 1);
    print(msg: `a={a} b={b}`)
}
"#,
        "match_arm_loop_per_iteration_store",
        "a=6 b=13",
    );
}

/// Genuinely-DIVERGING bindings GREEN clamp: two arms REASSIGN the
/// mutable binding to different fresh lists, the third leaves it alone;
/// the merged value is read after the match. The merge param is the real
/// phi for a changed binding — exactly what `merge_mutable_vars` keeps
/// on the if/else shape — so the divergence pruning must NOT remove it.
/// GREEN today (exit 0): reassignment releases the prior value and the
/// post-merge read consumes the live param. A cure that over-prunes
/// shows as a wrong post-merge value or a leak of the replaced list.
/// Interpreter prints a=3 b=6 c=1.
#[test]
fn test_match_diverging_bindings_post_merge_read_no_leak() {
    assert_cell_output(
        r#"
@diverge (n: int) -> int = {
    let acc = [0];
    match n {
        0 -> {
            acc = [1, 2];
        },
        1 -> {
            acc = [3, 4, 5];
        },
        _ -> (),
    };

    acc.len() + acc[0]
}

@main () -> void = {
    let a = diverge(n: 0);
    let b = diverge(n: 1);
    let c = diverge(n: 2);
    print(msg: `a={a} b={b} c={c}`)
}
"#,
        "match_diverging_bindings_post_merge_read",
        "a=3 b=6 c=1",
    );
}

/// Nested match-in-match with a store in the INNER match: the fresh
/// local crosses TWO Switch merges — the inner match's dead param and
/// the outer match's dead param. Leaks on EVERY call today (3 leaks,
/// exit 2), including the outer no-use arm (`false -> 42`), pinning that
/// the cure composes across nested merge frontiers instead of fixing
/// only the innermost. Interpreter prints x=3 y=13 z=42.
#[test]
#[ignore = "BUG-04-179: nested Switch-lowered matches thread the stored fresh local through BOTH dead merge block-params via Jump args; same root as the single-match cells"]
fn test_nested_match_inner_store_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@nested (a: bool, b: bool) -> int = {
    let base = [1, 2, 3];
    match a {
        true -> match b {
            true -> {
                let h = Holder { kept: base };

                h.kept.len()
            },
            false -> base.len() + 10,
        },
        false -> 42,
    }
}

@main () -> void = {
    let x = nested(a: true, b: true);
    let y = nested(a: true, b: false);
    let z = nested(a: false, b: false);
    print(msg: `x={x} y={y} z={z}`)
}
"#,
        "nested_match_inner_store",
        "x=3 y=13 z=42",
    );
}
