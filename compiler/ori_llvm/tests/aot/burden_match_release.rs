//! RL-5 dead-merge-param release matrix for Switch/loop-lowered `match`
//! (the dual-cure matrix). Pre-cure, `lower_match` threaded
//! EVERY in-scope mutable binding into the merge block-params, so an
//! unchanged fresh local rode every arm's `Jump` into a DEAD merge
//! block-param with no RL-5 release. Each missing release surfaces as
//! exit 2 under `ORI_CHECK_LEAKS=1` (harness default).
//!
//! Matrix axes: arm-consume shape [every-arm-store, struct partial-move,
//! no-store unchanged, loop-in-arm store, diverging reassignment,
//! diverging struct partial-consume, nested match inner-store] x
//! post-merge source read [none, borrow-read]. The primary
//! 2-arm/3-arm/loop-in-branch/enum-boundary cells live in
//! `burden_branch_release.rs`.
//!
//! Cure surfaces this matrix clamps: (B) `lower_match` merge-param
//! divergence pruning (toggle `ORI_DISABLE_MATCH_PARAM_PRUNING=1`) — the
//! diverging-bindings and post-merge-read cells pin that pruning never
//! fires on changed or still-read bindings; (A) the construct-fed RL-5
//! ALT-CONSUMED mode releasing the dead param when EVERY lineage consume
//! is funded (toggle `ORI_DISABLE_ALT_CONSUMED_DEAD_PARAM_RELEASE=1`) —
//! the struct partial-move cells clamp its root-kind DECLINE fence (an
//! over-fire double-frees). Toggle-parity pins mutation-verify each cure
//! on two Switch cells. Residual cells stay ignored on their own roots:
//! BUG-04-180 (loop-param threading), BUG-04-181 (struct partial-move
//! over-release), BUG-04-183 (diverging multi-rep dead param). Every cell
//! is interpreter-verified. Spec: Annex E §AIMS RL-1 + RL-2 + RL-4 + RL-5.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_cell_output, compile_and_run_with_build_env};

/// Every-arm-store cell source — shared by the GREEN cell and the
/// dual-cure toggle-parity pins.
const MATCH_EVERY_ARM_STORE_SRC: &str = r#"
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
"#;

/// 3-arm exclusive-store cell source (store / borrow / no-use arms) —
/// shared by the GREEN cell and the dual-cure toggle-parity pins.
const MATCH_THREE_ARM_STORE_SRC: &str = r#"
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
"#;

/// Zero-leak positive: a fresh local stored at an owned position on EVERY
/// arm, each arm into its own holder type. The merge-param pruning removes
/// the dead block-param entirely (no arm reassigns `base`); the per-arm
/// funded stores + last-use decs balance every reference. Interpreter
/// prints a=3 b=13.
#[test]
fn test_match_every_arm_stores_fresh_local_no_leak() {
    assert_cell_output(
        MATCH_EVERY_ARM_STORE_SRC,
        "match_every_arm_stores_fresh_local",
        "a=3 b=13",
    );
}

/// Struct partial-move-through-match NEGATIVE clamp: a struct whose
/// fields are PARTIALLY consumed at funded positions through the arms —
/// `left` stored into a holder (funded), `right` only borrowed. The
/// RL-5 alt-consumed cure DECLINES this lineage (struct root fails the
/// gate-(a) fence) and the divergence pruning removes the dead merge
/// param — but the cell routes into the standalone struct partial-move
/// over-release (double-free at AOT even straight-line, interpreter
/// green). Recorded honestly: ignored on that distinct root.
/// Interpreter prints a=5 b=13.
#[test]
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
/// sibling arm borrows. The match merge-param pruning removes the MATCH
/// dead param, but loop lowering still threads `base` through the loop
/// block-params into a dead loop-exit param whose feeding rep FRACTURES
/// from the Construct root across the Jump-arg hop (unit pin:
/// `dead_param_single_feeding_rep_fractures_across_loop_back_edge`) —
/// the RL-5 scans cannot resolve it. Interpreter prints a=6 b=13.
#[test]
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
/// local crosses TWO Switch merge frontiers, and the outer no-use arm
/// (`false -> 42`) exercises the dead-on-edge birth release (the
/// fully-dead path owes TWO decs: birth + funding inc). Pins that the
/// cure composes across nested merge frontiers instead of fixing only
/// the innermost. Interpreter prints x=3 y=13 z=42.
#[test]
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

/// DIVERGING struct partial-consume cell: the struct binding is REASSIGNED
/// in the consuming arm (a real phi — the merge param survives pruning,
/// per the `match_assigned_mutable_binding_keeps_merge_param` unit pin)
/// while its fields are partially consumed at funded positions. The
/// alt-consumed release correctly DECLINES (struct root fails the gate-(a)
/// fence; the dead param is fed by TWO reps), so the dead multi-rep merge
/// param leaks — the LEAK signature is the clamp: an alt-consumed
/// over-fire here would double-free (crash), never leak. Interpreter
/// prints a=5 b=13.
#[test]
fn test_match_diverging_struct_partial_consume_no_double_free() {
    assert_cell_output(
        r#"
type Pair = { left: [int], right: [int] }
type Holder = { kept: [int] }

@diverge_partial (n: int) -> int = {
    let p = Pair { left: [1, 2, 3], right: [4, 5] };
    match n {
        0 -> {
            let h = Holder { kept: p.left };
            p = Pair { left: [9], right: [8, 7] };
            h.kept.len() + p.right.len()
        },
        _ -> p.left.len() + 10,
    }
}

@main () -> void = {
    let a = diverge_partial(n: 0);
    let b = diverge_partial(n: 1);
    print(msg: `a={a} b={b}`)
}
"#,
        "match_diverging_struct_partial_consume",
        "a=5 b=13",
    );
}

// ----- Dual-cure toggle-parity pins (compile-time env; subprocess-isolated) -----

/// Backstop pin: pruning disabled, ALT-CONSUMED release active — cure A
/// alone supplies the dead param's RL-5 release on the every-arm-store
/// shape (exit 0).
#[test]
fn test_every_arm_store_with_pruning_disabled_stays_clean() {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(
        MATCH_EVERY_ARM_STORE_SRC,
        &[("ORI_DISABLE_MATCH_PARAM_PRUNING", "1")],
    );
    assert_eq!(
        exit, 0,
        "the alt-consumed release alone must cure the dead merge param\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "a=3 b=13");
}

/// Backstop pin: pruning disabled, ALT-CONSUMED release active — 3-arm
/// shape stays clean (exit 0).
#[test]
fn test_three_arm_store_with_pruning_disabled_stays_clean() {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(
        MATCH_THREE_ARM_STORE_SRC,
        &[("ORI_DISABLE_MATCH_PARAM_PRUNING", "1")],
    );
    assert_eq!(
        exit, 0,
        "the alt-consumed release alone must cure the 3-arm dead merge param\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "a=3 b=13 c=42");
}

/// Structural pin: ALT-CONSUMED release disabled, pruning active — cure B
/// alone dissolves the dead param on the every-arm-store shape (exit 0).
#[test]
fn test_every_arm_store_with_alt_consumed_release_disabled_stays_clean() {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(
        MATCH_EVERY_ARM_STORE_SRC,
        &[("ORI_DISABLE_ALT_CONSUMED_DEAD_PARAM_RELEASE", "1")],
    );
    assert_eq!(
        exit, 0,
        "the merge-param pruning alone must dissolve the dead merge param\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "a=3 b=13");
}

/// Structural pin: ALT-CONSUMED release disabled, pruning active — 3-arm
/// shape stays clean (exit 0).
#[test]
fn test_three_arm_store_with_alt_consumed_release_disabled_stays_clean() {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(
        MATCH_THREE_ARM_STORE_SRC,
        &[("ORI_DISABLE_ALT_CONSUMED_DEAD_PARAM_RELEASE", "1")],
    );
    assert_eq!(
        exit, 0,
        "the merge-param pruning alone must dissolve the 3-arm dead merge param\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "a=3 b=13 c=42");
}
