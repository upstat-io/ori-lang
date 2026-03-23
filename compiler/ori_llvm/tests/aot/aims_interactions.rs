#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

#[test]
fn test_h1_drop_hint_unique_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $xs = for i in 1..61 yield i;
    let sum = 0;
    for x in xs do sum = sum + x;
    if sum == 1830 then 0 else 1
}
"#,
        "h1_drop_hint_unique_list",
    );
}

#[test]
fn test_h3_precise_death_points_once_cardinality() {
    assert_aot_success(
        r#"
@main () -> int = {
    let $a = "alpha";
    let $b = "beta-beta";
    let $xs = [10, 20, 30];
    let $ys = [1, 2];

    let $first = len(collection: a);
    let $second = xs[0] + xs[1] + xs[2];
    let $third = len(collection: b);
    let $fourth = ys[0] + ys[1];

    if first == 5 && second == 60 && third == 9 && fourth == 3 then 0 else 1
}
"#,
        "h3_precise_death_points_once_cardinality",
    );
}

#[test]
fn test_h4_fip_certified_recursive_rebuild() {
    assert_aot_success(
        r#"
type Node = End | Cell(value: int, next: Node);

@build (n: int) -> Node = {
    if n <= 0 then End
    else Cell(value: n, next: build(n: n - 1))
};

@rewrite (node: Node) -> Node = match node {
    End -> End,
    Cell(v, next) -> Cell(value: v * 2 - 1, next: rewrite(node: next)),
};

@sum (node: Node) -> int = match node {
    End -> 0,
    Cell(v, next) -> v + sum(node: next),
};

@len (node: Node) -> int = match node {
    End -> 0,
    Cell(_, next) -> 1 + len(node: next),
};

@main () -> int = {
    let $input = build(n: 64);
    let $output = rewrite(node: input);
    let $sum_ok = sum(node: output) == 4096;
    let $len_ok = len(node: output) == 64;
    if sum_ok && len_ok then 0 else 1
}
"#,
        "h4_fip_certified_recursive_rebuild",
    );
}

#[test]
fn test_h6_callee_returns_unique_for_caller_reuse() {
    assert_aot_success(
        r#"
type Bundle = { values: [int], marker: int };

@build_bundle (seed: int) -> Bundle = {
    Bundle { values: [seed, seed + 1, seed + 2], marker: seed * 10 }
};

@main () -> int = {
    let $bundle = build_bundle(seed: 7);
    let $ok = bundle.marker == 70
        && bundle.values.len() == 3
        && bundle.values[0] == 7
        && bundle.values[1] == 8
        && bundle.values[2] == 9;
    if ok then 0 else 1
}
"#,
        "h6_callee_returns_unique_for_caller_reuse",
    );
}

#[test]
fn test_h7_pure_callee_then_static_unique_cow_mutation() {
    assert_aot_success(
        r#"
@inspect (xs: [int]) -> int = xs[0] + xs[1] + xs[2] + xs[3];

@main () -> int = {
    let xs = [1, 2, 3, 4];
    let $before = inspect(xs: xs);
    xs = xs.push(99);
    let $after_ok = xs.len() == 5 && xs[0] == 1 && xs[1] == 2 && xs[2] == 3 && xs[3] == 4 && xs[4] == 99;
    if before == 10 && after_ok then 0 else 1
}
"#,
        "h7_pure_callee_then_static_unique_cow_mutation",
    );
}

#[test]
fn test_h13_reuse_priority_over_drop_hint() {
    assert_aot_success(
        r#"
type Chain = Done | Link(a: int, b: int, next: Chain);

@build (n: int) -> Chain = {
    if n <= 0 then Done
    else Link(a: n, b: n * 3, next: build(n: n - 1))
};

@rewrite (c: Chain) -> Chain = match c {
    Done -> Done,
    Link(a, b, next) -> Link(a: a + 1, b: b - 1, next: rewrite(c: next)),
};

@score (c: Chain) -> int = match c {
    Done -> 0,
    Link(a, b, next) -> a + b + score(c: next),
};

@len (c: Chain) -> int = match c {
    Done -> 0,
    Link(_, _, next) -> 1 + len(c: next),
};

@main () -> int = {
    let $input = build(n: 80);
    let $before = score(c: input);
    let $output = rewrite(c: input);
    let $after = score(c: output);
    let $len_ok = len(c: output) == 80;
    if before == after && len_ok then 0 else 1
}
"#,
        "h13_reuse_priority_over_drop_hint",
    );
}

#[test]
fn test_h15_reuse_survives_block_merge() {
    assert_aot_success(
        r#"
type Stream = End | Left(v: int, next: Stream) | Right(v: int, next: Stream);

@build (n: int) -> Stream = {
    if n <= 0 then End
    else {
        let $tail = build(n: n - 1);
        if n % 2 == 0 then Left(v: n, next: tail) else Right(v: n, next: tail)
    }
};

@rewrite (s: Stream, flip: bool) -> Stream = match s {
    End -> End,
    Left(v, next) -> {
        let $next_v = if flip then v + 1 else v + 1;
        Left(v: next_v, next: rewrite(s: next, flip: flip))
    },
    Right(v, next) -> {
        let $next_v = if flip then v + 1 else v + 1;
        Right(v: next_v, next: rewrite(s: next, flip: flip))
    },
};

@sum (s: Stream) -> int = match s {
    End -> 0,
    Left(v, next) -> v + sum(s: next),
    Right(v, next) -> v + sum(s: next),
};

@main () -> int = {
    let $input = build(n: 70);
    let $output = rewrite(s: input, flip: true);
    if sum(s: output) == 2555 then 0 else 1
}
"#,
        "h15_reuse_survives_block_merge",
    );
}

// H2: Lattice × TRMC — Canonicalize Rule 4 fires on context var in TRMC loop.
// Context var is StaticUnique for in-place Set (no Dynamic COW check).

#[test]
fn test_h2_lattice_trmc_context_var_unique() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@build (n: int) -> List = {
    if n <= 0 then Nil
    else Cons(head: n, tail: build(n: n - 1))
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@main () -> int = {
    let $list = build(n: 100);
    if list_sum(list: list) == 5050 then 0 else 1
}
"#,
        "h2_lattice_trmc_context_var_unique",
    );
}

// H5: Intra × TRMC — Analysis reconverges on TRMC-rewritten IR.
// Loop back-edge creates cycle in dataflow; context vars get Unique+FunctionLocal.

#[test]
fn test_h5_intra_trmc_reconvergence() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@map_add (list: List, offset: int) -> List = match list {
    Nil -> Nil,
    Cons(h, t) -> Cons(head: h + offset, tail: map_add(list: t, offset: offset)),
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@list_len (list: List) -> int = match list {
    Nil -> 0,
    Cons(_, t) -> 1 + list_len(list: t),
};

@main () -> int = {
    let $list = Cons(head: 1, tail: Cons(head: 2, tail: Cons(head: 3,
        tail: Cons(head: 4, tail: Cons(head: 5, tail: Nil)))));
    let $mapped = map_add(list: list, offset: 10);
    let $sum_ok = list_sum(list: mapped) == 65;
    let $len_ok = list_len(list: mapped) == 5;
    if sum_ok && len_ok then 0 else 1
}
"#,
        "h5_intra_trmc_reconvergence",
    );
}

// H8: Inter × TRMC — Callee is TRMC-rewritten; caller sees refreshed contract.
// Caller uses has_unbounded_stack=false from updated contract.

#[test]
fn test_h8_inter_trmc_refreshed_contract() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@map_inc (list: List) -> List = match list {
    Nil -> Nil,
    Cons(h, t) -> Cons(head: h + 1, tail: map_inc(list: t)),
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@caller () -> int = {
    let $list = Cons(head: 10, tail: Cons(head: 20, tail: Cons(head: 30, tail: Nil)));
    let $result = map_inc(list: list);
    list_sum(list: result)
};

@main () -> int = {
    if caller() == 63 then 0 else 1
}
"#,
        "h8_inter_trmc_refreshed_contract",
    );
}

// H9: RC × FIP — FIP Certified function has zero RcInc; all RcDec matched by reuse.

#[test]
fn test_h9_rc_fip_zero_net_rc() {
    assert_aot_success(
        r#"
type Node = End | Cell(value: int, next: Node);

@rebuild (node: Node) -> Node = match node {
    End -> End,
    Cell(v, next) -> Cell(value: v * 3, next: rebuild(node: next)),
};

@sum (node: Node) -> int = match node {
    End -> 0,
    Cell(v, next) -> v + sum(node: next),
};

@build (n: int) -> Node = {
    if n <= 0 then End
    else Cell(value: n, next: build(n: n - 1))
};

@main () -> int = {
    let $input = build(n: 50);
    let $output = rebuild(node: input);
    // 3 * (1+2+...+50) = 3 * 1275 = 3825
    if sum(node: output) == 3825 then 0 else 1
}
"#,
        "h9_rc_fip_zero_net_rc",
    );
}

// H10: RC × TRMC — RcInc/RcDec correct for context vars in TRMC loop body.
// Context root: no RcDec until base case; context hole_obj: Set only.

#[test]
fn test_h10_rc_trmc_context_var_lifecycle() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@map_square (list: List) -> List = match list {
    Nil -> Nil,
    Cons(h, t) -> Cons(head: h * h, tail: map_square(list: t)),
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@main () -> int = {
    let $list = Cons(head: 1, tail: Cons(head: 2, tail: Cons(head: 3,
        tail: Cons(head: 4, tail: Cons(head: 5, tail: Nil)))));
    let $squared = map_square(list: list);
    // 1+4+9+16+25 = 55
    if list_sum(list: squared) == 55 then 0 else 1
}
"#,
        "h10_rc_trmc_context_var_lifecycle",
    );
}

// H11: RC × TailCall — Tail-call rewrite removes self-call; no phantom RcInc.
// Args transferred via Jump, not call.

#[test]
fn test_h11_rc_tailcall_no_phantom_inc() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@sum_acc (list: List, acc: int) -> int = match list {
    Nil -> acc,
    Cons(h, t) -> sum_acc(list: t, acc: acc + h),
};

@build (n: int) -> List = {
    if n <= 0 then Nil
    else Cons(head: n, tail: build(n: n - 1))
};

@main () -> int = {
    let $list = build(n: 80);
    if sum_acc(list: list, acc: 0) == 3240 then 0 else 1
}
"#,
        "h11_rc_tailcall_no_phantom_inc",
    );
}

// H12: RC × Merge — Block merge doesn't invalidate RC operations.
// Trampoline blocks cleaned up correctly.

#[test]
fn test_h12_rc_merge_preserves_rc_ops() {
    assert_aot_success(
        r#"
type Wrapper = { name: str, value: int };

@process (w: Wrapper) -> int = {
    let $base = w.value;
    let $result = if base > 10 then {
        let $extra = w.name;
        len(collection: extra) + base
    } else {
        base * 2
    };
    result
};

@main () -> int = {
    let $w1 = Wrapper { name: "hello world", value: 20 };
    let $w2 = Wrapper { name: "test", value: 5 };
    let $r1 = process(w: w1);
    let $r2 = process(w: w2);
    // r1 = 11 + 20 = 31, r2 = 5 * 2 = 10
    if r1 == 31 && r2 == 10 then 0 else 1
}
"#,
        "h12_rc_merge_preserves_rc_ops",
    );
}

// H14: Reuse × TRMC — Pattern match in TRMC-rewritten loop: scrutinee
// death + same-type construct → reuse.

#[test]
fn test_h14_reuse_trmc_loop_reuse() {
    assert_aot_success(
        r#"
type Chain = Done | Link(a: int, b: int, next: Chain);

@transform (c: Chain) -> Chain = match c {
    Done -> Done,
    Link(a, b, next) -> Link(a: a * 2, b: b + 1, next: transform(c: next)),
};

@chain_sum (c: Chain) -> int = match c {
    Done -> 0,
    Link(a, b, next) -> a + b + chain_sum(c: next),
};

@build (n: int) -> Chain = {
    if n <= 0 then Done
    else Link(a: n, b: n * 10, next: build(n: n - 1))
};

@main () -> int = {
    let $input = build(n: 60);
    let $output = transform(c: input);
    // a_sum = 2*(1+..+60)=3660, b_sum = (10+20+..+600)+60=18360
    // total = 3660 + 18360 = 22020
    if chain_sum(c: output) == 22020 then 0 else 1
}
"#,
        "h14_reuse_trmc_loop_reuse",
    );
}

// H16: COW × FIP — FIP Certified function: all COW ops are StaticUnique (auto-FBIP).

#[test]
fn test_h16_cow_fip_auto_fbip() {
    assert_aot_success(
        r#"
type Node = End | Cell(value: int, next: Node);

@double_values (node: Node) -> Node = match node {
    End -> End,
    Cell(v, next) -> Cell(value: v * 2, next: double_values(node: next)),
};

@sum (node: Node) -> int = match node {
    End -> 0,
    Cell(v, next) -> v + sum(node: next),
};

@build (n: int) -> Node = {
    if n <= 0 then End
    else Cell(value: n, next: build(n: n - 1))
};

@main () -> int = {
    let $input = build(n: 40);
    let $output = double_values(node: input);
    // 2 * (1+2+...+40) = 2 * 820 = 1640
    if sum(node: output) == 1640 then 0 else 1
}
"#,
        "h16_cow_fip_auto_fbip",
    );
}

// H17: COW × TRMC — COW mutation in TRMC loop body on context var → StaticUnique.

#[test]
fn test_h17_cow_trmc_static_unique_context() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@map_negate (list: List) -> List = match list {
    Nil -> Nil,
    Cons(h, t) -> Cons(head: 0 - h, tail: map_negate(list: t)),
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@main () -> int = {
    let $list = Cons(head: 1, tail: Cons(head: 2, tail: Cons(head: 3,
        tail: Cons(head: 4, tail: Nil))));
    let $negated = map_negate(list: list);
    // -(1+2+3+4) = -10
    if list_sum(list: negated) == 0 - 10 then 0 else 1
}
"#,
        "h17_cow_trmc_static_unique_context",
    );
}

// H18: Drop × TRMC — RcDec on context root at base-case return → drop hint
// if collection type. Context root is unique by construction.

#[test]
fn test_h18_drop_trmc_context_root_hint() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@build (n: int) -> List = {
    if n <= 0 then Nil
    else Cons(head: n, tail: build(n: n - 1))
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@list_len (list: List) -> int = match list {
    Nil -> 0,
    Cons(_, t) -> 1 + list_len(list: t),
};

@main () -> int = {
    // Build and consume — context root at base case gets drop hint.
    let $list = build(n: 50);
    let $sum_ok = list_sum(list: list) == 1275;
    let $len_ok = list_len(list: list) == 50;
    if sum_ok && len_ok then 0 else 1
}
"#,
        "h18_drop_trmc_context_root_hint",
    );
}

// H19: FIP × TRMC — Contract upgrades from Never to Certified post-rewrite.
// Before TRMC: unbounded stack (recursive calls). After: constant stack.

#[test]
fn test_h19_fip_trmc_contract_upgrade() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@map_triple (list: List) -> List = match list {
    Nil -> Nil,
    Cons(h, t) -> Cons(head: h * 3, tail: map_triple(list: t)),
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@build (n: int) -> List = {
    if n <= 0 then Nil
    else Cons(head: n, tail: build(n: n - 1))
};

@main () -> int = {
    let $input = build(n: 30);
    let $output = map_triple(list: input);
    // 3 * (1+2+...+30) = 3 * 465 = 1395
    if list_sum(list: output) == 1395 then 0 else 1
}
"#,
        "h19_fip_trmc_contract_upgrade",
    );
}

// H20: TRMC × TailCall — TRMC produces loop back-edge; tail-call pass
// runs after; must not double-loopify.

#[test]
fn test_h20_trmc_tailcall_no_double_loop() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@filter_even (list: List) -> List = match list {
    Nil -> Nil,
    Cons(h, t) -> {
        if h % 2 == 0 then Cons(head: h, tail: filter_even(list: t))
        else filter_even(list: t)
    },
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@list_len (list: List) -> int = match list {
    Nil -> 0,
    Cons(_, t) -> 1 + list_len(list: t),
};

@build (n: int) -> List = {
    if n <= 0 then Nil
    else Cons(head: n, tail: build(n: n - 1))
};

@main () -> int = {
    let $list = build(n: 20);
    let $evens = filter_even(list: list);
    // Even numbers 2,4,6,...,20: sum=110, count=10
    let $sum_ok = list_sum(list: evens) == 110;
    let $len_ok = list_len(list: evens) == 10;
    if sum_ok && len_ok then 0 else 1
}
"#,
        "h20_trmc_tailcall_no_double_loop",
    );
}

// H21: TRMC × Merge — TRMC prologue→header→helper topology survives merge.
// Prologue retained (or inlined correctly); context init preserved.

#[test]
fn test_h21_trmc_merge_topology_preserved() {
    assert_aot_success(
        r#"
type Tree = Leaf(value: int) | Branch(left: Tree, right: Tree);

@inc_leaves (t: Tree) -> Tree = match t {
    Leaf(v) -> Leaf(value: v + 1),
    Branch(l, r) -> Branch(left: inc_leaves(t: l), right: inc_leaves(t: r)),
};

@tree_sum (t: Tree) -> int = match t {
    Leaf(v) -> v,
    Branch(l, r) -> tree_sum(t: l) + tree_sum(t: r),
};

@main () -> int = {
    let $tree = Branch(
        left: Branch(left: Leaf(value: 1), right: Leaf(value: 2)),
        right: Leaf(value: 3),
    );
    let $result = inc_leaves(t: tree);
    // (1+1)+(2+1)+(3+1) = 2+3+4 = 9
    if tree_sum(t: result) == 9 then 0 else 1
}
"#,
        "h21_trmc_merge_topology_preserved",
    );
}

// H22: TRMC × Verify — Verify pass checks rewritten IR: no undefined vars,
// no unreachable blocks, RC balanced.

#[test]
fn test_h22_trmc_verify_clean_ir() {
    assert_aot_success(
        r#"
type List = Nil | Cons(head: int, tail: List);

@map_plus_ten (list: List) -> List = match list {
    Nil -> Nil,
    Cons(h, t) -> Cons(head: h + 10, tail: map_plus_ten(list: t)),
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@build (n: int) -> List = {
    if n <= 0 then Nil
    else Cons(head: n, tail: build(n: n - 1))
};

@main () -> int = {
    let $list = build(n: 25);
    let $mapped = map_plus_ten(list: list);
    // (1+2+...+25) + 25*10 = 325 + 250 = 575
    if list_sum(list: mapped) == 575 then 0 else 1
}
"#,
        "h22_trmc_verify_clean_ir",
    );
}
