//! RL-1 store-family duplication funding on FRESH-local lineages whose
//! duplicates are consumed at owned Construct-field STORE positions
//! (`Holder { kept: base }` with `base` a fresh local, read after the store).
//!
//! Mechanism under test: the same symmetric-cancellation under-inc as the
//! owned-call-arg family (see `burden_dup_inc.rs`), through the aggregate
//! Store consume position instead of the Invoke/Apply arg position. Per RL-1
//! `RL1_duplication_balanced`, each kept store must be funded by a SURVIVING
//! per-site inc whose matched release is the holder's drop. The under-inc
//! surfaces as a SIGSEGV / double-free once the holders' drops drive rc past
//! 0; the over-fire polarity (extra surviving inc) surfaces as exit 2 under
//! `ORI_CHECK_LEAKS=1` on the negative pins.
//!
//! Matrix: store fork-counts [1, 2, 3] x post-store reads [single-use after
//! store, read-before-store, holder-field + base, COW-mutated base + holder]
//! x control flow [straight-line, branch-exclusive, loop-reassigned].
//! GREEN clamps (must stay green — the cure's over-fire boundary): the
//! single-use-read-after-store compensation cell and the COW `.set`
//! interaction cell. ALL other cells are RED today — including the
//! branch-exclusive-store and loop-reassigned-store cells the fix consensus
//! expected green: empirically they leak (exit 2) / SIGSEGV, so they share
//! the store-family under-funding root and are pinned at their correct
//! post-cure behavior. All cells run under `ORI_CHECK_LEAKS=1` (harness
//! default); every cell is interpreter-verified. Spec: Annex E §AIMS RL-1 +
//! RL-2.
//!
//! Note: the COW-interaction cell uses `base = base.set(0, 42)` — the
//! index-assignment sugar `base[0] = 42` is not yet shipped (E6080
//! interpreter / E4003 AOT); `.set` reassignment exercises the same dynamic
//! COW uniqueness-check path on the stored lineage.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_cell_output;

// ----- GREEN over-fire clamps (must stay GREEN through the cure) -----

/// Single-use-read compensation clamp — the latent-green shape the RCA
/// found: fresh local stored DIRECTLY once, base read exactly ONCE after the
/// store (`base.len()` only, single-use read alias). The decoupled
/// read-alias split IS this lineage's release today; the cure's funded
/// treatment must fully REPLACE that compensation, never stack on it (a
/// stacked inc leaks, exit 2).
#[test]
fn test_struct_store_single_use_read_after_store_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@main () -> void = {
    let base = [1, 2, 3];
    let h = Holder { kept: base };
    print(msg: `n={base.len()}`)
}
"#,
        "struct_store_single_use_read_after_store",
        "n=3",
    );
}

// ----- RED cells (fail today; GREEN after the cure, unmodified) -----

/// Read-BEFORE-store variant: fresh local alias-stored once, source read
/// ONCE before the store and never after — the store hand-off must be priced
/// by the lineage net so the fresh keep-alive elides. Shared by the GREEN
/// cell and the `ORI_DISABLE_STORE_FAMILY_FUNDING` toggle-parity pin below.
const READ_BEFORE_STORE_SRC: &str = r#"
type Holder = { kept: [int] }

@main () -> void = {
    let base = [1, 2, 3];
    let first = base[0];
    let a = base;
    let h = Holder { kept: a };
    print(msg: `first={first} kept0={h.kept[0]}`)
}
"#;

#[test]
fn test_local_alias_store_src_read_before_store_no_leak() {
    assert_cell_output(
        READ_BEFORE_STORE_SRC,
        "local_alias_store_src_read_before_store",
        "first=1 kept0=1",
    );
}

/// Branch-exclusive store: base stored in exactly ONE branch with NO use of
/// base after the store — a terminal move per path, NOT a duplication, so
/// the correct realization keeps no inc (the store-family funding DECLINES
/// this shape by design; the per-path ledger agrees at net 0 with the store
/// debit). The residual leak is a DISTINCT root: the holder's
/// terminal-move-per-path drop under-releases the kept reference on the
/// storing path. Interpreter prints a=3 b=13.
#[test]
fn test_branch_exclusive_store_no_kept_inc_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@store_one (flag: bool) -> int = {
    let base = [1, 2, 3];
    if flag then {
        let h = Holder { kept: base };

        h.kept.len()
    } else {
        base.len() + 10
    }
}

@main () -> void = {
    let a = store_one(flag: true);
    let b = store_one(flag: false);
    print(msg: `a={a} b={b}`)
}
"#,
        "branch_exclusive_store_no_kept_inc",
        "a=3 b=13",
    );
}

/// Loop-store: a local REASSIGNED per iteration (fresh construct, store into
/// a holder, then a COW fork rebinding the local). Each iteration's lineage
/// is fresh; correct realization neither over-funds (leak) nor under-funds.
/// SIGSEGVs TODAY (exit -139) — the store-family under-funding root on the
/// loop-carried reassignment shape. Interpreter prints total=12 slen=3.
#[test]
fn test_loop_reassigned_local_store_per_iteration_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@main () -> void = {
    let s = [0];
    let total = 0;
    for i in 0..3 do {
        s = [i, i + 1];
        let h = Holder { kept: s };
        s = s.push(7);
        total = total + h.kept[0] + s.len();
    };
    print(msg: `total={total} slen={s.len()}`)
}
"#,
        "loop_reassigned_local_store_per_iteration",
        "total=12 slen=3",
    );
}

/// Map-typed ({str: int}) two-store cell — the type-dimension clamp: a fresh
/// map stored into TWO holders, read through both holder projections + base.
/// Same funded store-family treatment as the `[int]` cells through the map
/// fat-pointer repr. Interpreter prints h1a=1 h2b=2 n=2. Shared by the GREEN
/// cell and the `ORI_DISABLE_STORE_FAMILY_FUNDING` toggle-parity pin below.
const MAP_STORE_PAIR_SRC: &str = r#"
type Holder = { kept: {str: int} }

@main () -> void = {
    let base = {"a": 1, "b": 2};
    let h1 = Holder { kept: base };
    let h2 = Holder { kept: base };
    print(msg: `h1a={h1.kept["a"] ?? 0} h2b={h2.kept["b"] ?? 0} n={base.len()}`)
}
"#;

#[test]
fn test_map_store_pair_fresh_local_base_read_no_crash() {
    assert_cell_output(
        MAP_STORE_PAIR_SRC,
        "map_store_pair_fresh_local_base_read",
        "h1a=1 h2b=2 n=2",
    );
}

/// Loop-OUTSIDE/store-INSIDE symmetric subshape: a fresh local defined BEFORE
/// the loop, stored into a holder EVERY iteration (each iteration's store is
/// a genuine duplication of the loop-invariant source — the source survives
/// into the next iteration). Interpreter prints total=6 blen=3.
#[test]
fn test_loop_invariant_local_stored_per_iteration_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@main () -> void = {
    let base = [1, 2, 3];
    let total = 0;
    for i in 0..3 do {
        let h = Holder { kept: base };
        total = total + h.kept[i];
    };
    print(msg: `total={total} blen={base.len()}`)
}
"#,
        "loop_invariant_local_stored_per_iteration",
        "total=6 blen=3",
    );
}

/// Two stores + post-store FIELD read: fresh local stored into TWO holders,
/// then read through holder1's field and through base. Double-frees TODAY
/// (SIGABRT, `ori_rc_dec` on freed allocation) — same root as the direct
/// two-store pin in `burden_dup_inc.rs`, with the post-store read going
/// through the holder projection instead of base alone. Interpreter prints
/// h1k=1 base0=1 h2n=3.
#[test]
fn test_struct_store_pair_field_read_through_holder_no_crash() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@main () -> void = {
    let base = [1, 2, 3];
    let h1 = Holder { kept: base };
    let h2 = Holder { kept: base };
    print(msg: `h1k={h1.kept[0]} base0={base[0]} h2n={h2.kept.len()}`)
}
"#,
        "struct_store_pair_field_read_through_holder",
        "h1k=1 base0=1 h2n=3",
    );
}

/// Three-store variant (fork-count 3) of the direct-store shape: fresh local
/// stored into THREE holders, base read after all three. Double-frees TODAY
/// (SIGABRT) — fork-count axis clamp above the two-store pin. Interpreter
/// prints sum=4.
#[test]
fn test_struct_store_triple_fresh_local_base_read_no_crash() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@main () -> void = {
    let base = [1, 2, 3];
    let h1 = Holder { kept: base };
    let h2 = Holder { kept: base };
    let h3 = Holder { kept: base };
    print(msg: `sum={h1.kept[0] + h2.kept[0] + h3.kept[0] + base[0]}`)
}
"#,
        "struct_store_triple_fresh_local_base_read",
        "sum=4",
    );
}

// ----- GREEN COW-interaction clamp (must stay GREEN through the cure) -----

/// COW-interaction: store base into a holder, then dynamic-COW mutate base
/// (`base = base.set(0, 42)`), then read BOTH. GREEN today — the `.set`
/// COW receiver path copies correctly on this lineage; the cure's funding
/// re-arrangement must keep the holder's view unmodified (h0=1) and stay
/// leak-clean (a wrong-direction fix shows as h0=42, freed-memory read, or
/// exit 2).
#[test]
fn test_store_then_cow_set_base_holder_view_unmodified() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@main () -> void = {
    let base = [1, 2, 3];
    let h = Holder { kept: base };
    base = base.set(0, 42);
    print(msg: `h0={h.kept[0]} base0={base[0]}`)
}
"#,
        "store_then_cow_set_base_holder_view",
        "h0=1 base0=42",
    );
}

// ----- Toggle-parity semantic pins (compile-time env; subprocess-isolated) -----

/// Semantic pin: `ORI_DISABLE_STORE_FAMILY_FUNDING=1` restores the pre-cure
/// arrangement — the map two-store cell double-frees again (`ori_rc_dec` on
/// an already-freed allocation), proving the funded store-family treatment is
/// the cure surface.
#[test]
fn test_map_store_pair_with_store_family_funding_disabled_double_frees_again() {
    use crate::util::compile_and_run_with_build_env;
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(
        MAP_STORE_PAIR_SRC,
        &[("ORI_DISABLE_STORE_FAMILY_FUNDING", "1")],
    );
    assert_ne!(
        exit, 0,
        "with the store-family funding disabled, the map two-store cell must \
         regress (double-free abort, exit != 0)\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("double-free"),
        "the regression is the pre-cure double-free shape\nexit={exit}\nstderr:\n{stderr}"
    );
}

/// Semantic pin: `ORI_DISABLE_STORE_FAMILY_FUNDING=1` restores the undebited
/// lineage net on the read-before-store shape — the fresh keep-alive inc is
/// kept again and the lineage leaks one allocation (exit 2 under
/// `ORI_CHECK_LEAKS=1`), proving the hand-off-debited net is the cure surface
/// for the leak polarity.
#[test]
fn test_read_before_store_with_store_family_funding_disabled_leaks_again() {
    use crate::util::compile_and_run_with_build_env;
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(
        READ_BEFORE_STORE_SRC,
        &[("ORI_DISABLE_STORE_FAMILY_FUNDING", "1")],
    );
    assert_eq!(
        exit, 2,
        "with the store-family funding disabled, the read-before-store cell \
         must leak (exit 2)\nstderr:\n{stderr}"
    );
}
