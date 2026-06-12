//! RL-1 duplication funding on multi-fork same-allocation collection lineages
//! whose duplicates are consumed at OWNED call-arg positions (COW-method
//! receivers and consuming user fns).
//!
//! Mechanism under test: Phase 5 emits an alias-site `BurdenInc` per
//! duplication, but the terminator-dec loop emits a symmetric same-block
//! `BurdenDec` for every transferred-out inc'd var, so each owned-call-arg
//! duplication pair nets 0 — ONE net inc survives per lineage regardless of N
//! duplications. Per RL-1 `RL1_duplication_balanced` + RL-2
//! `RL2_transfer_kinds_no_dec`, each owned hand-off must be funded by a
//! SURVIVING per-site inc whose matched release is the consumer's. The
//! under-inc surfaces as a silent wrong value (rc hits 1 early, the COW
//! uniqueness check sees "unique" and mutates an aliased buffer in place)
//! and as a double-free / UAF once consumes drive rc past 0.
//!
//! Matrix: types [List [int], Map {str:int}, `Option<str>`-payload map] x
//! `fork_counts` [1, 2, 3] x consumer kinds [builtin COW receiver (owned
//! Invoke arg), consuming user fn (owned Apply/Invoke arg), aggregate-Store
//! regression pin, borrowed read arg NEGATIVE, iter-consuming for-loop
//! NEGATIVE] x control flow [straight-line, branch-exclusive decline pin,
//! loop-carried duplication, if/else merge]. Clamped: every axis covered,
//! every known-failure cell named, redundant interior cells skipped.
//! All cells run under `ORI_CHECK_LEAKS=1`; over-fire (extra surviving inc)
//! shows up as exit 2 on the negative pins. Subprocess-isolated.
//! Spec: Annex E §AIMS RL-1 + RL-2.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, assert_cell_output};

// ----- Straight-line, builtin COW receivers (owned Invoke args) -----

/// 3-fork list lineage, straight-line, builtin COW receivers
/// (push / push / remove), base read after all forks. The exact
/// silent-wrong-value shape: rc reaches 1 after the first fork, the second
/// fork's uniqueness check mutates the aliased buffer in place, and the
/// remove leg over-consumes (double-free).
#[test]
fn test_list_three_owned_invoke_forks_base_unmodified() {
    assert_cell_output(
        r#"
@multi_fork () -> ([int], [int], [int], [int]) = {
    let base = [1, 2, 3];
    let f1 = base.push(4);
    let f2 = base.push(5);
    let f3 = base.remove(0);
    (base, f1, f2, f3)
}

@main () -> void = {
    let (base, f1, f2, f3) = multi_fork();
    print(msg: `lens {base.len()} {f1.len()} {f2.len()} {f3.len()} base0={base[0]} f3_0={f3[0]}`)
}
"#,
        "list_three_owned_invoke_forks",
        "lens 3 4 4 2 base0=1 f3_0=2",
    );
}

/// Named wrong-value regression pin: diamond sharing with a `set` fork.
/// `base` must stay `[10, 20, 30]` and only `fork_c` carries the 99 —
/// the broken realization mutates base's buffer in place
/// (`base == [99, 20, 30]`).
#[test]
fn test_diamond_sharing_set_fork_does_not_mutate_base() {
    assert_cell_output(
        r#"
@diamond_sharing () -> ([int], [int], [int], [int]) = {
    let base = [10, 20, 30];
    let fork_a = base.push(40);
    let fork_b = base.push(50);
    let fork_c = base.set(0, 99);
    (base, fork_a, fork_b, fork_c)
}

@main () -> void = {
    let (base, a, b, c) = diamond_sharing();
    print(msg: `base={base[0]},{base[1]},{base[2]} a3={a[3]} b3={b[3]} c={c[0]},{c[1]},{c[2]}`)
}
"#,
        "diamond_sharing_set_fork",
        "base=10,20,30 a3=40 b3=50 c=99,20,30",
    );
}

/// 3-fork map lineage, straight-line, builtin COW receivers
/// (insert / insert / remove), base read after all forks.
#[test]
fn test_map_three_owned_invoke_forks_base_unmodified() {
    assert_cell_output(
        r#"
@multi_fork () -> ({str: int}, {str: int}, {str: int}, {str: int}) = {
    let base = {"k": 42};
    let f1 = base.insert("a", 1);
    let f2 = base.insert("b", 2);
    let f3 = base.remove("k");
    (base, f1, f2, f3)
}

@main () -> void = {
    let (base, f1, f2, f3) = multi_fork();
    let kept = base["k"].unwrap_or(default: -1);
    print(msg: `lens {base.len()} {f1.len()} {f2.len()} {f3.len()} k={kept}`)
}
"#,
        "map_three_owned_invoke_forks",
        "lens 1 2 2 0 k=42",
    );
}

/// 3-fork map lineage with `Option<str>` payloads — nested heap elements
/// multiply the freed surface, so the over-consume lands as a double-free
/// on element strings, not just the buffer.
#[test]
fn test_option_payload_map_three_forks_no_double_free() {
    assert_cell_output(
        r#"
@multi_fork () -> ({str: Option<str>}, {str: Option<str>}, {str: Option<str>}, {str: Option<str>}) = {
    let base: {str: Option<str>} = {"k": Some("heap payload string one")};
    let f1 = base.insert("a", Some("heap payload string two"));
    let f2 = base.insert("b", None);
    let f3 = base.remove("k");
    (base, f1, f2, f3)
}

@main () -> void = {
    let (base, f1, f2, f3) = multi_fork();
    print(msg: `lens {base.len()} {f1.len()} {f2.len()} {f3.len()}`)
}
"#,
        "option_payload_map_three_forks",
        "lens 1 2 2 0",
    );
}

// ----- 2-fork boundary cells (net -1/0 boundary per the lineage ledger) -----

/// 2-fork list lineage source (push + remove), base read after both forks.
/// Shared by the GREEN boundary cell and the `ORI_DISABLE_OWNED_CALL_ARG_DUP_INC`
/// toggle-parity pin below.
const TWO_FORK_LIST_SRC: &str = r#"
@two_fork () -> ([int], [int], [int]) = {
    let base = [1, 2, 3];
    let f1 = base.push(4);
    let f2 = base.remove(0);
    (base, f1, f2)
}

@main () -> void = {
    let (base, f1, f2) = two_fork();
    print(msg: `lens {base.len()} {f1.len()} {f2.len()} base0={base[0]}`)
}
"#;

/// 2-fork list lineage (push + remove), base read after both forks.
/// The net -1 boundary: the second fork's uniqueness check already sees
/// rc=1 and mutates in place.
#[test]
fn test_list_two_owned_invoke_forks_base_unmodified() {
    assert_cell_output(
        TWO_FORK_LIST_SRC,
        "list_two_owned_invoke_forks",
        "lens 3 4 2 base0=1",
    );
}

/// 2-fork map lineage (insert + remove), base read after both forks.
#[test]
fn test_map_two_owned_invoke_forks_base_unmodified() {
    assert_cell_output(
        r#"
@two_fork () -> ({str: int}, {str: int}, {str: int}) = {
    let base = {"k": 42};
    let f1 = base.insert("a", 1);
    let f2 = base.remove("k");
    (base, f1, f2)
}

@main () -> void = {
    let (base, f1, f2) = two_fork();
    let kept = base["k"].unwrap_or(default: -1);
    print(msg: `lens {base.len()} {f1.len()} {f2.len()} k={kept}`)
}
"#,
        "map_two_owned_invoke_forks",
        "lens 1 2 0 k=42",
    );
}

// ----- Consuming user-fn owned args (Apply/Invoke owned-arg family) -----

/// 3-fork list lineage where every duplicate is consumed by a USER fn
/// taking the collection owned (the callee's push consumes its param).
/// Same under-inc as the builtin-receiver cells, through the
/// Apply/Invoke owned-arg position.
#[test]
fn test_list_three_user_fn_owned_arg_forks_base_unmodified() {
    assert_cell_output(
        r#"
@grow (xs: [int], v: int) -> [int] = xs.push(v);

@main () -> void = {
    let base = [1, 2, 3];
    let f1 = grow(xs: base, v: 4);
    let f2 = grow(xs: base, v: 5);
    let f3 = grow(xs: base, v: 6);
    print(msg: `lens {base.len()} {f1.len()} {f2.len()} {f3.len()} base0={base[0]} f3_3={f3[3]}`)
}
"#,
        "list_three_user_fn_owned_arg_forks",
        "lens 3 4 4 4 base0=1 f3_3=6",
    );
}

/// 2-fork map lineage through a consuming user fn (owned arg position).
#[test]
fn test_map_two_user_fn_owned_arg_forks_base_unmodified() {
    assert_cell_output(
        r#"
@stash (m: {str: int}, k: str, v: int) -> {str: int} = m.insert(k, v);

@main () -> void = {
    let base = {"k": 42};
    let f1 = stash(m: base, k: "a", v: 1);
    let f2 = stash(m: base, k: "b", v: 2);
    let kept = base["k"].unwrap_or(default: -1);
    print(msg: `lens {base.len()} {f1.len()} {f2.len()} k={kept}`)
}
"#,
        "map_two_user_fn_owned_arg_forks",
        "lens 1 2 2 k=42",
    );
}

// ----- Control-flow cells -----

/// Loop-carried duplication: `base.set(0, i)` per iteration duplicates the
/// same lineage on every back-edge re-entry; base is forward-reachable via
/// the loop. Each iteration's fork must COW-copy, never mutate base.
#[test]
fn test_list_loop_carried_set_forks_base_unmodified() {
    assert_cell_output(
        r#"
@main () -> void = {
    let base = [1, 2, 3];
    let total = 0;
    for i in 0..5 do {
        let f = base.set(0, i);
        total = total + f[0];
    };
    print(msg: `base0={base[0]} total={total}`)
}
"#,
        "list_loop_carried_set_forks",
        "base0=1 total=10",
    );
}

/// If/else merge: one duplication per branch, base read after the merge —
/// each executed path is a genuine duplication (src forward-reachable past
/// the alias on every path).
#[test]
fn test_list_fork_in_both_branches_merge_base_unmodified() {
    assert_cell_output(
        r#"
@fork_one (flag: bool) -> int = {
    let base = [1, 2, 3];
    let f = if flag then base.push(4) else base.push(5);

    base[0] * 100 + f[3]
}

@main () -> void = {
    let t = fork_one(flag: true);
    let e = fork_one(flag: false);
    print(msg: `then={t} else={e}`)
}
"#,
        "list_fork_in_both_branches_merge",
        "then=104 else=105",
    );
}

/// NEGATIVE decline pin: alias consumed in exactly ONE branch with NO use
/// of src after the branch — a terminal move per path, NOT a duplication.
/// The fix must NOT keep an inc here (a kept inc with no later release is
/// a leak; `ORI_CHECK_LEAKS=1` exits 2).
#[test]
fn test_list_branch_exclusive_alias_no_kept_inc_no_leak() {
    assert_aot_success(
        r#"
@consume_one (flag: bool) -> int = {
    let base = [1, 2, 3];
    if flag then {
        let f = base.push(4);

        f.len()
    } else {
        let g = base.push(5);

        g[3]
    }
}

@main () -> int = {
    let a = consume_one(flag: true);
    let b = consume_one(flag: false);
    if a == 4 && b == 5 then 0 else 1
}
"#,
        "list_branch_exclusive_alias_no_kept_inc_no_leak",
    );
}

// ----- Single-fork + non-admission pins (must stay GREEN) -----

/// 1-fork pin: a single duplication consumed owned with base read after.
/// Funded correctly today by the lineage keep-alive inc; clamps the
/// fork-count axis from below.
#[test]
fn test_list_single_fork_then_read_base_unmodified() {
    assert_cell_output(
        r#"
@main () -> void = {
    let base = [1, 2, 3];
    let f = base.push(4);
    print(msg: `base0={base[0]} flen={f.len()}`)
}
"#,
        "list_single_fork_then_read",
        "base0=1 flen=4",
    );
}

/// Single-use move-alias non-admission pin: `let b = a;` where `a` is never
/// used again is a MOVE, not a duplication — no alias-site inc may survive
/// (leak otherwise).
#[test]
fn test_list_single_use_move_alias_no_dup_inc_no_leak() {
    assert_cell_output(
        r#"
@main () -> void = {
    let a = [1, 2, 3];
    let b = a;
    let c = b.push(4);
    print(msg: `clen={c.len()} c3={c[3]}`)
}
"#,
        "list_single_use_move_alias",
        "clen=4 c3=4",
    );
}

// ----- Regression + over-fire NEGATIVE pins (must stay GREEN) -----

/// Aggregate-STORE regression pin: the param-rooted alias-store shape
/// (`let a = xs; Holder { kept: a }` with `xs` live after) is the family
/// `compute_genuine_dup_move_aliases` already protects — the pair-kept inc
/// funds the store. Must stay green when the family widens to owned call
/// args.
#[test]
fn test_param_alias_store_dup_src_returned_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@stash_and_return (xs: [int]) -> [int] = {
    let a = xs;
    let h = Holder { kept: a };
    let $n = h.kept.len() + xs.len();

    if n > 0 then xs else xs
}

@main () -> void = {
    let base = [5, 6, 7];
    let out = stash_and_return(xs: base);
    print(msg: `len={out.len()} last={out[2]}`)
}
"#,
        "param_alias_store_dup_src_returned",
        "len=3 last=7",
    );
}

/// Discovered store-family cell: a fresh LOCAL base stored DIRECTLY into
/// TWO struct literals (no Let-Var alias), base read after both stores —
/// same-allocation multi-consume through owned Construct field args.
/// Crashes today (SIGSEGV); interpreter prints sum=3.
#[test]
fn test_struct_store_pair_fresh_local_base_read_no_crash() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@main () -> void = {
    let base = [1, 2, 3];
    let h1 = Holder { kept: base };
    let h2 = Holder { kept: base };
    print(msg: `sum={h1.kept[0] + h2.kept[0] + base[0]}`)
}
"#,
        "struct_store_pair_fresh_local_base_read",
        "sum=3",
    );
}

/// Discovered store-family cell, leak polarity: a fresh LOCAL base
/// alias-stored ONCE (`let a = base; Holder { kept: a }`) with base read
/// after — the kept duplication inc plus the fresh keep-alive over-supply
/// the lineage net (the fix's lineage-net accounting must count kept dup
/// incs exactly once). Leaks today (exit 2); interpreter prints sum=2.
#[test]
fn test_local_alias_store_dup_base_read_no_leak() {
    assert_cell_output(
        r#"
type Holder = { kept: [int] }

@main () -> void = {
    let base = [1, 2, 3];
    let a = base;
    let h = Holder { kept: a };
    print(msg: `sum={h.kept[0] + base[0]}`)
}
"#,
        "local_alias_store_dup_base_read",
        "sum=2",
    );
}

/// Borrowed-arg NEGATIVE over-fire pin: a read-only user fn borrows its
/// param (caller retains the release per RL-2
/// `RL2_borrowed_param_emits_caller_dec`); admitting borrowed args to the
/// duplication family would double-fund and leak.
#[test]
fn test_borrowed_read_args_no_extra_inc_no_leak() {
    assert_cell_output(
        r#"
@total (xs: [int]) -> int = xs.fold(initial: 0, op: (acc, x) -> acc + x);

@main () -> void = {
    let base = [1, 2, 3];
    let t1 = total(xs: base);
    let t2 = total(xs: base);
    let t3 = total(xs: base);
    print(msg: `sum={t1 + t2 + t3} base0={base[0]}`)
}
"#,
        "borrowed_read_args_no_extra_inc",
        "sum=18 base0=1",
    );
}

/// Iter-consume exclusion pin: for-loops over the collection are
/// iter-consume positions, excluded from the duplication family; admitting
/// them double-funds and leaks (exit 2 catches over-fire post-fix). The
/// owned push fork after two iter consumes raises the lineage to
/// `use_count >= 3` — the under-inc double-frees TODAY, so this cell is also
/// a failing clamp of the same lineage ledger.
#[test]
fn test_iter_consume_then_fork_no_extra_inc_no_leak() {
    assert_cell_output(
        r#"
@main () -> void = {
    let base = [1, 2, 3];
    let s1 = 0;
    for x in base do {
        s1 = s1 + x;
    };
    let s2 = 0;
    for x in base do {
        s2 = s2 + x * 10;
    };
    let f = base.push(4);
    print(msg: `s1={s1} s2={s2} flen={f.len()} base0={base[0]}`)
}
"#,
        "iter_consume_then_fork_no_extra_inc",
        "s1=6 s2=60 flen=4 base0=1",
    );
}

// ----- Call-result-aggregate element FINAL-READ release cells -----

/// Multi-read returned-tuple element source: `let (a, b) = make()` with `a`
/// read TWICE (`a.len()` then `a[0]`) — each read goes through a fresh
/// `Let { Var }` alias of the element projection, so the lineage's
/// execution-final read alias must carry the element's single release
/// (RL-2 `RL2_release_exactly_once`). Shared by the GREEN cell and the
/// `ORI_DISABLE_RESULT_ELEM_FINAL_READ_RELEASE` toggle-parity pin below.
const RESULT_ELEM_MULTI_READ_SRC: &str = r#"
@make () -> ([int], [int]) = ([1, 2, 3], [4, 5]);

@main () -> void = {
    let (a, b) = make();
    let n = a.len();
    let first = a[0];
    print(msg: `n={n} first={first} blen={b.len()}`)
}
"#;

/// Multi-read returned-tuple element gets exactly one release at its final
/// read — no leaked element, no double-free.
#[test]
fn test_result_tuple_element_multi_read_final_release_no_leak() {
    assert_cell_output(
        RESULT_ELEM_MULTI_READ_SRC,
        "result_tuple_element_multi_read_final_release",
        "n=3 first=1 blen=2",
    );
}

// ----- Toggle-parity semantic pins (compile-time env; subprocess-isolated) -----
//
// Each toggle is passed through the COMPILE step via
// `compile_and_run_with_build_env` (never `std::env::set_var` — parallel-safe).

/// Semantic pin: `ORI_DISABLE_OWNED_CALL_ARG_DUP_INC=1` restores the
/// symmetric-cancellation under-inc — the `two_fork` boundary cell double-frees
/// again (rc hits 1 early; the remove leg over-consumes), proving the kept
/// owned-call-arg duplication inc is the cure surface.
#[test]
fn test_two_fork_with_owned_call_arg_dup_inc_disabled_double_frees_again() {
    use crate::util::compile_and_run_with_build_env;
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(
        TWO_FORK_LIST_SRC,
        &[("ORI_DISABLE_OWNED_CALL_ARG_DUP_INC", "1")],
    );
    assert_ne!(
        exit, 0,
        "with the owned-call-arg duplication inc disabled, the two_fork cell \
         must regress (double-free abort, exit != 0)\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("double-free"),
        "the regression is the pre-cure double-free shape\nexit={exit}\nstderr:\n{stderr}"
    );
}

/// Semantic pin: `ORI_DISABLE_RESULT_ELEM_FINAL_READ_RELEASE=1` restores the
/// keep-alive-pair arrangement — the multi-read tuple element's caller-owned
/// reference is never released, leaking one element per lineage (exit 2 under
/// `ORI_CHECK_LEAKS=1`). Proves the designated FINAL-READ release is the cure
/// surface.
#[test]
fn test_result_tuple_element_with_final_read_release_disabled_leaks_again() {
    use crate::util::compile_and_run_with_build_env;
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(
        RESULT_ELEM_MULTI_READ_SRC,
        &[("ORI_DISABLE_RESULT_ELEM_FINAL_READ_RELEASE", "1")],
    );
    assert_eq!(
        exit, 2,
        "with the final-read release disabled, the multi-read element must leak \
         (exit 2)\nstderr:\n{stderr}"
    );
}
