//! Memory Safety Stress Tests (Section 11.3)
//!
//! Tests that push the ARC pipeline to its limits: high allocation counts,
//! complex ownership patterns, large collections, and deep recursion with
//! RC'd values. Every test runs with `ORI_CHECK_LEAKS=1` — any refcount
//! imbalance causes exit code 2 (leak detected).
//!
//! These tests complement `stress.rs` (which focuses on scale/throughput)
//! with tests specifically designed to catch ARC correctness bugs:
//! use-after-free, double-free, and leak scenarios.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── High allocation count (10,000+) ───

#[test]
fn test_mem_10k_struct_allocations() {
    assert_aot_success(
        r#"
type Item = { id: int, value: str }

@main () -> int = {
    let count = 0;
    for i in 0..10000 do {
        let item = Item { id: i, value: "data" };
        count = count + 1
    };
    if count == 10000 then 0 else 1
}
"#,
        "mem_10k_struct_allocs",
    );
}

#[test]
fn test_mem_10k_nested_struct_allocations() {
    assert_aot_success(
        r#"
type Inner = { val: int }
type Outer = { inner: Inner, name: str }

@main () -> int = {
    let sum = 0;
    for i in 0..10000 do {
        let o = Outer { inner: Inner { val: i }, name: "test" };
        sum = sum + o.inner.val
    };
    // sum(0..9999) = 9999 * 10000 / 2 = 49995000
    if sum == 49995000 then 0 else 1
}
"#,
        "mem_10k_nested_struct_allocs",
    );
}

#[test]
fn test_mem_10k_string_concat_and_discard() {
    // Allocates and discards 10,000 strings — tests that intermediate
    // string allocations from concat are properly freed each iteration.
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for i in 0..10000 do {
        let s = "prefix-" + i.to_str();
        count = count + 1
    };
    if count == 10000 then 0 else 1
}
"#,
        "mem_10k_string_discard",
    );
}

// ─── Large collections (10,000+ elements) ───

#[test]
fn test_mem_large_list_10k_elements() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 0..10000 yield i;
    let sum = xs.iter().fold(start: 0, f: (acc: int, x: int) -> int = acc + x);
    // sum(0..9999) = 9999 * 10000 / 2 = 49995000
    if sum == 49995000 then 0 else 1
}
"#,
        "mem_large_list_10k",
    );
}

#[test]
fn test_mem_large_list_10k_filter_collect() {
    // Build 10K list, filter to ~5K, collect. Two large allocations that
    // must both be freed. Tests iterator + collect allocation lifecycle.
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 0..10000 yield i;
    let evens = xs.iter()
        .filter(x -> x % 2 == 0)
        .count();
    // 0,2,4,...,9998 → 5000 items
    if evens == 5000 then 0 else 1
}
"#,
        "mem_large_list_10k_filter",
    );
}

#[test]
fn test_mem_large_list_map_chain() {
    // Tests iterator adapter allocation at scale: each adapter wraps
    // the previous one. At 10K iterations all must be properly dropped.
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 1..=10000 yield i;
    let result = xs.iter()
        .map(x -> x * 2)
        .filter(x -> x % 4 == 0)
        .map(x -> x + 1)
        .take(1000)
        .fold(start: 0, f: (acc: int, x: int) -> int = acc + x);
    // Items: 4+1, 8+1, 12+1, ..., first 1000 multiples of 4: (4k+1) for k=1..1000
    // = 4*sum(1..1000) + 1000 = 4*500500 + 1000 = 2003000
    if result == 2003000 then 0 else 1
}
"#,
        "mem_large_list_map_chain",
    );
}

// ─── Diamond sharing (multiple owners of same allocation) ───

#[test]
fn test_mem_diamond_sharing_struct() {
    // Classic diamond pattern: one allocation, multiple independent owners.
    // If RC is wrong, either use-after-free or leak.
    assert_aot_success(
        r#"
type Shared = { data: str, id: int }

@use_shared (s: Shared) -> int = s.id;

@main () -> int = {
    let shared = Shared { data: "diamond", id: 42 };
    // Create 5 independent references
    let a = shared;
    let b = shared;
    let c = shared;
    // Pass each through a function (tests RC across call boundary)
    let r1 = use_shared(s: a);
    let r2 = use_shared(s: b);
    let r3 = use_shared(s: c);
    // Also use the original
    let r4 = use_shared(s: shared);
    if r1 + r2 + r3 + r4 == 168 then 0 else 1
}
"#,
        "mem_diamond_sharing",
    );
}

#[test]
fn test_mem_diamond_sharing_in_loop() {
    // Diamond sharing created and destroyed 1000 times. Each iteration
    // creates multiple refs to one allocation, then all go out of scope.
    assert_aot_success(
        r#"
type Payload = { text: str, num: int }

@main () -> int = {
    let total = 0;
    for i in 0..1000 do {
        let p = Payload { text: "loop", num: i };
        let r1 = p;
        let r2 = p;
        let r3 = p;
        // All 4 refs (p, r1, r2, r3) must be properly freed each iteration
        total = total + r1.num + r2.num + r3.num
    };
    // total = sum(3*i for i in 0..1000) = 3 * 999*1000/2 = 1498500
    if total == 1498500 then 0 else 1
}
"#,
        "mem_diamond_in_loop",
    );
}

#[test]
fn test_mem_diamond_struct_in_struct() {
    // Shared allocation referenced from two different struct fields.
    // Tests that struct drop decrements the shared inner correctly.
    assert_aot_success(
        r#"
type Inner = { data: str }
type Container = { a: Inner, b: Inner }

@main () -> int = {
    let shared = Inner { data: "shared-inner" };
    let c = Container { a: shared, b: shared };
    let ok = c.a.data == "shared-inner" && c.b.data == "shared-inner";
    if ok then 0 else 1
}
"#,
        "mem_diamond_in_struct",
    );
}

#[test]
fn test_mem_diamond_closure_capture() {
    // Two closures capture the same RC'd value. Both must inc on capture,
    // both must dec when the closure is dropped.
    assert_aot_success(
        r#"
type Info = { msg: str, code: int }

@main () -> int = {
    let info = Info { msg: "captured", code: 7 };
    let get_msg = () -> str = info.msg;
    let get_code = () -> int = info.code;
    let m = get_msg();
    let c = get_code();
    if m == "captured" && c == 7 then 0 else 1
}
"#,
        "mem_diamond_closure",
    );
}

// ─── Deep recursion with RC'd values ───

#[test]
fn test_mem_deep_recursion_200_with_strings() {
    // Each recursive call creates a new string-containing struct.
    // Tests that all 200 allocations are properly freed as the stack unwinds.
    assert_aot_success(
        r#"
type State = { label: str, depth: int, acc: int }

@descend (s: State) -> int =
    if s.depth == 0 then s.acc
    else descend(s: State { label: "level", depth: s.depth - 1, acc: s.acc + s.depth });

@main () -> int = {
    let result = descend(s: State { label: "start", depth: 200, acc: 0 });
    // sum(1..200) = 200*201/2 = 20100
    if result == 20100 then 0 else 1
}
"#,
        "mem_deep_recursion_strings",
    );
}

#[test]
fn test_mem_deep_recursion_100_shared_param() {
    // Passes a shared (RC'd) struct through 100 levels of recursion.
    // Each call borrows the struct — tests that RC is maintained correctly
    // across deep call stacks.
    assert_aot_success(
        r#"
type Config = { base: int, name: str }

@recursive_sum (cfg: Config, n: int, acc: int) -> int =
    if n == 0 then acc + cfg.base
    else recursive_sum(cfg: cfg, n: n - 1, acc: acc + n);

@main () -> int = {
    let cfg = Config { base: 1000, name: "config" };
    let result = recursive_sum(cfg: cfg, n: 100, acc: 0);
    // 1000 + sum(1..100) = 1000 + 5050 = 6050
    if result == 6050 then 0 else 1
}
"#,
        "mem_deep_recursion_shared",
    );
}

// ─── Multi-function RC passing chains ───

#[test]
fn test_mem_function_chain_10_deep() {
    // RC'd struct passed through 10 functions. Each function receives,
    // reads a field, and passes to the next. Tests transitive RC tracking.
    assert_aot_success(
        r#"
type Ctx = { value: int, tag: str }

@step1 (c: Ctx) -> int = step2(c: c);
@step2 (c: Ctx) -> int = step3(c: c);
@step3 (c: Ctx) -> int = step4(c: c);
@step4 (c: Ctx) -> int = step5(c: c);
@step5 (c: Ctx) -> int = step6(c: c);
@step6 (c: Ctx) -> int = step7(c: c);
@step7 (c: Ctx) -> int = step8(c: c);
@step8 (c: Ctx) -> int = step9(c: c);
@step9 (c: Ctx) -> int = step10(c: c);
@step10 (c: Ctx) -> int = c.value;

@main () -> int = {
    let c = Ctx { value: 99, tag: "chain" };
    let result = step1(c: c);
    if result == 99 then 0 else 1
}
"#,
        "mem_chain_10_deep",
    );
}

#[test]
fn test_mem_function_chain_with_transform() {
    // Each function transforms the struct (creating new allocation) and
    // passes it on. Tests that intermediate allocations are freed.
    assert_aot_success(
        r#"
type Data = { x: int, label: str }

@add_one (d: Data) -> Data = Data { x: d.x + 1, label: d.label };
@add_two (d: Data) -> Data = Data { x: d.x + 2, label: d.label };
@add_three (d: Data) -> Data = Data { x: d.x + 3, label: d.label };

@pipeline (d: Data) -> int = {
    let d2 = add_one(d: d);
    let d3 = add_two(d: d2);
    let d4 = add_three(d: d3);
    d4.x
}

@main () -> int = {
    let total = 0;
    for i in 0..500 do {
        let d = Data { x: i, label: "pipe" };
        total = total + pipeline(d: d)
    };
    // sum((i + 1 + 2 + 3) for i in 0..500) = sum(i+6) = sum(i) + 3000
    // = 499*500/2 + 3000 = 124750 + 3000 = 127750
    if total == 127750 then 0 else 1
}
"#,
        "mem_chain_transform_500",
    );
}

// ─── Complex ownership: multiple structs sharing inner allocations ───

#[test]
fn test_mem_shared_inner_multiple_containers() {
    // One inner struct shared across multiple outer structs.
    // When outers are dropped, inner must survive until last ref dies.
    assert_aot_success(
        r#"
type Core = { data: str }
type Wrapper = { core: Core, id: int }

@main () -> int = {
    let sum = 0;
    for i in 0..500 do {
        let c = Core { data: "shared" };
        let w1 = Wrapper { core: c, id: i };
        let w2 = Wrapper { core: c, id: i + 1 };
        sum = sum + w1.id + w2.id
    };
    // sum((i + i+1) for i in 0..500) = sum(2i+1) = 2*sum(i) + 500
    // = 2 * 124750 + 500 = 249500 + 500 = 250000
    if sum == 250000 then 0 else 1
}
"#,
        "mem_shared_inner_containers",
    );
}

#[test]
fn test_mem_reassignment_frees_old_value() {
    // Reassigning a variable holding an RC'd struct must free the old value.
    // 10,000 reassignments = 10,000 allocations that must be freed.
    assert_aot_success(
        r#"
type Holder = { val: int, name: str }

@main () -> int = {
    let h = Holder { val: 0, name: "init" };
    for i in 1..=10000 do {
        h = Holder { val: i, name: "updated" }
    };
    if h.val == 10000 then 0 else 1
}
"#,
        "mem_reassign_frees_old",
    );
}

// ─── Closure lifecycle stress ───

#[test]
fn test_mem_closure_capture_in_loop() {
    // Creates and destroys 1000 closures, each capturing an RC'd struct.
    // Tests that closure env + captured values are properly freed.
    assert_aot_success(
        r#"
type Env = { factor: int, label: str }

@main () -> int = {
    let total = 0;
    for i in 0..1000 do {
        let env = Env { factor: i, label: "closure" };
        let f = (x: int) -> int = x * env.factor;
        total = total + f(x: 2)
    };
    // total = sum(2*i for i in 0..1000) = 2 * 999*1000/2 = 999000
    if total == 999000 then 0 else 1
}
"#,
        "mem_closure_in_loop",
    );
}

#[test]
fn test_mem_closure_escapes_scope() {
    // Closure outlives the scope where its captures were created.
    // The captured value must be kept alive by the closure's RC.
    assert_aot_success(
        r#"
type Config = { multiplier: int, name: str }

@make_multiplier (m: int) -> (int) -> int = {
    let cfg = Config { multiplier: m, name: "mul" };
    (x: int) -> int = x * cfg.multiplier
}

@main () -> int = {
    let total = 0;
    for i in 1..=100 do {
        let mul = make_multiplier(m: i);
        total = total + mul(x: 3)
    };
    // total = sum(3*i for i in 1..100) = 3 * 100*101/2 = 15150
    if total == 15150 then 0 else 1
}
"#,
        "mem_closure_escapes",
    );
}

// ─── Mixed stress: combines multiple ownership patterns ───

#[test]
fn test_mem_combined_allocation_storm() {
    // Creates structs, closures, lists, strings, and shared references
    // all in one tight loop. Maximum allocation pressure.
    assert_aot_success(
        r#"
type Record = { id: int, name: str }

@process (r: Record) -> int = r.id + r.name.length();

@main () -> int = {
    let total = 0;
    for i in 0..2000 do {
        // Struct allocation
        let r = Record { id: i, name: "rec" };
        // Shared reference
        let alias = r;
        // Closure capturing struct
        let get_id = () -> int = r.id;
        // String concat
        let desc = "item-" + i.to_str();
        // Use everything
        total = total + process(r: alias) + get_id() + desc.length()
    };
    // process: i + 3 (len("rec")), get_id: i, desc.length: 5 + digits(i)
    // Rough check: just verify it completed without leak
    if total > 0 then 0 else 1
}
"#,
        "mem_combined_storm",
    );
}

#[test]
fn test_mem_interleaved_alloc_free() {
    // Alternating allocations and frees test that the allocator handles
    // fragmented free lists correctly.
    assert_aot_success(
        r#"
type Temp = { x: int, s: str }

@compute (n: int) -> int = {
    let t = Temp { x: n, s: "temporary" };
    t.x * 2
}

@main () -> int = {
    let sum = 0;
    for i in 0..5000 do {
        // Each call allocates and frees a Temp
        sum = sum + compute(n: i)
    };
    // sum(2i for i in 0..5000) = 2 * 4999*5000/2 = 24995000
    if sum == 24995000 then 0 else 1
}
"#,
        "mem_interleaved_alloc_free",
    );
}
