//! Scale & Stress AOT Tests
//!
//! Tests that push the AOT pipeline on **scale**: large collections, many fields,
//! deep nesting, and high allocation counts. These stress the ARC pipeline,
//! LLVM codegen, and runtime memory management at volumes that basic tests
//! don't reach.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Large collection construction via for-yield ───

#[test]
fn test_stress_large_list_100_elements() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 0..100 yield i;
    let sum = xs.iter().fold(start: 0, f: (acc: int, x: int) -> int = acc + x);
    // sum(0..99) = 99 * 100 / 2 = 4950
    if sum == 4950 then 0 else 1
}
"#,
        "stress_large_list_100",
    );
}

#[test]
fn test_stress_large_list_500_elements() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 0..500 yield i;
    let count = xs.iter().count();
    if count == 500 then 0 else 1
}
"#,
        "stress_large_list_500",
    );
}

#[test]
fn test_stress_large_list_filter_collect() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 0..200 yield i;
    let evens = xs.iter().filter(x -> x % 2 == 0).count();
    if evens == 100 then 0 else 1
}
"#,
        "stress_large_list_filter",
    );
}

// ─── Struct with many fields ───

#[test]
fn test_stress_struct_8_fields() {
    assert_aot_success(
        r#"
type Wide = { a: int, b: int, c: int, d: int, e: int, f: int, g: int, h: int }

@main () -> int = {
    let w = Wide { a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, h: 8 };
    let sum = w.a + w.b + w.c + w.d + w.e + w.f + w.g + w.h;
    if sum == 36 then 0 else 1
}
"#,
        "stress_struct_8_fields",
    );
}

#[test]
fn test_stress_struct_mixed_field_types() {
    assert_aot_success(
        r#"
type Mixed = { n: int, f: float, b: bool, s: str, c: char }

@main () -> int = {
    let m = Mixed { n: 42, f: 3.14, b: true, s: "hello", c: 'x' };
    let ok1 = m.n == 42;
    let ok2 = m.f > 3.0 && m.f < 4.0;
    let ok3 = m.b;
    let ok4 = m.s == "hello";
    let ok5 = m.c == 'x';
    if ok1 && ok2 && ok3 && ok4 && ok5 then 0 else 1
}
"#,
        "stress_struct_mixed_types",
    );
}

#[test]
fn test_stress_struct_update_many_fields() {
    assert_aot_success(
        r#"
type Wide = { a: int, b: int, c: int, d: int, e: int }

@main () -> int = {
    let w = Wide { a: 1, b: 2, c: 3, d: 4, e: 5 };
    let w2 = Wide { ...w, a: 10, c: 30, e: 50 };
    if w2.a == 10 && w2.b == 2 && w2.c == 30 && w2.d == 4 && w2.e == 50 then 0 else 1
}
"#,
        "stress_struct_update_many",
    );
}

// ─── Deep struct nesting ───

#[test]
fn test_stress_nested_structs_4_levels() {
    assert_aot_success(
        r#"
type L1 = { value: int }
type L2 = { inner: L1, tag: int }
type L3 = { inner: L2, label: str }
type L4 = { inner: L3, count: int }

@main () -> int = {
    let deep = L4 {
        inner: L3 {
            inner: L2 {
                inner: L1 { value: 42 },
                tag: 1,
            },
            label: "deep",
        },
        count: 99,
    };
    let ok1 = deep.inner.inner.inner.value == 42;
    let ok2 = deep.inner.inner.tag == 1;
    let ok3 = deep.inner.label == "deep";
    let ok4 = deep.count == 99;
    if ok1 && ok2 && ok3 && ok4 then 0 else 1
}
"#,
        "stress_nested_4_levels",
    );
}

#[test]
fn test_stress_nested_struct_with_strings_at_every_level() {
    assert_aot_success(
        r#"
type A = { name: str, val: int }
type B = { a: A, desc: str }
type C = { b: B, info: str }

@main () -> int = {
    let c = C {
        b: B {
            a: A { name: "inner", val: 7 },
            desc: "middle",
        },
        info: "outer",
    };
    let ok1 = c.b.a.name == "inner";
    let ok2 = c.b.a.val == 7;
    let ok3 = c.b.desc == "middle";
    let ok4 = c.info == "outer";
    if ok1 && ok2 && ok3 && ok4 then 0 else 1
}
"#,
        "stress_nested_strings_all_levels",
    );
}

// ─── Large tuples ───

#[test]
fn test_stress_tuple_5_elements() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (10, 20, 30, 40, 50);
    let (a, b, c, d, e) = t;
    if a + b + c + d + e == 150 then 0 else 1
}
"#,
        "stress_tuple_5",
    );
}

#[test]
fn test_stress_tuple_6_elements_field_access() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (1, 2, 3, 4, 5, 6);
    if t.0 == 1 && t.1 == 2 && t.2 == 3 && t.3 == 4 && t.4 == 5 && t.5 == 6 then 0 else 1
}
"#,
        "stress_tuple_6_access",
    );
}

#[test]
fn test_stress_tuple_mixed_types() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (42, true, "hello", 3.14);
    let (n, b, s, f) = t;
    if n == 42 && b && s == "hello" && f > 3.0 then 0 else 1
}
"#,
        "stress_tuple_mixed",
    );
}

#[test]
fn test_stress_tuple_from_function() {
    assert_aot_success(
        r#"
@make5 (x: int) -> (int, int, int, int, int) = (x, x * 2, x * 3, x * 4, x * 5);

@main () -> int = {
    let (a, b, c, d, e) = make5(x: 3);
    if a == 3 && b == 6 && c == 9 && d == 12 && e == 15 then 0 else 1
}
"#,
        "stress_tuple_5_from_fn",
    );
}

// ─── ARC allocation stress ───

#[test]
fn test_stress_arc_1000_struct_allocations() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@main () -> int = {
    let sum = 0;
    for i in 0..1000 do {
        let p = Point { x: i, y: i * 2 };
        sum = sum + p.x + p.y
    };
    // sum = Σ(i + 2i) for i=0..999 = 3 * Σi = 3 * 999*1000/2 = 1498500
    if sum == 1498500 then 0 else 1
}
"#,
        "stress_arc_1000_allocs",
    );
}

#[test]
fn test_stress_arc_1000_string_struct_allocations() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }

@main () -> int = {
    let count = 0;
    for i in 0..1000 do {
        let n = Named { name: "item", id: i };
        count = count + 1
    };
    if count == 1000 then 0 else 1
}
"#,
        "stress_arc_1000_string_allocs",
    );
}

#[test]
fn test_stress_arc_nested_struct_in_loop() {
    assert_aot_success(
        r#"
type Inner = { value: int }
type Outer = { inner: Inner, label: str }

@main () -> int = {
    let sum = 0;
    for i in 0..500 do {
        let o = Outer { inner: Inner { value: i }, label: "test" };
        sum = sum + o.inner.value
    };
    // sum(0..499) = 499*500/2 = 124750
    if sum == 124750 then 0 else 1
}
"#,
        "stress_arc_nested_in_loop",
    );
}

// ─── Deep recursion with struct parameters ───

#[test]
fn test_stress_deep_recursion_200_levels() {
    assert_aot_success(
        r#"
type Acc = { sum: int, depth: int }

@go_deep (a: Acc) -> int =
    if a.depth == 0 then a.sum
    else go_deep(a: Acc { sum: a.sum + a.depth, depth: a.depth - 1 });

@main () -> int = {
    let result = go_deep(a: Acc { sum: 0, depth: 200 });
    // sum(1..200) = 200*201/2 = 20100
    if result == 20100 then 0 else 1
}
"#,
        "stress_deep_recursion_200",
    );
}

#[test]
fn test_stress_deep_recursion_500_levels() {
    assert_aot_success(
        r#"
@sum_to (n: int, acc: int) -> int =
    if n == 0 then acc else sum_to(n: n - 1, acc: acc + n);

@main () -> int = {
    let result = sum_to(n: 500, acc: 0);
    // sum(1..500) = 500*501/2 = 125250
    if result == 125250 then 0 else 1
}
"#,
        "stress_deep_recursion_500",
    );
}

// ─── String concatenation stress ───

#[test]
fn test_stress_string_concat_100() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    for _ in 0..100 do s = s + "a";
    if s.length() == 100 then 0 else 1
}
"#,
        "stress_string_concat_100",
    );
}

#[test]
fn test_stress_string_concat_varied() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "start";
    for i in 0..50 do {
        s = s + "-" + "x"
    };
    // "start" + 50 * "-x" = 5 + 50*2 = 105
    if s.length() == 105 then 0 else 1
}
"#,
        "stress_string_concat_varied",
    );
}

// ─── List of structs ───

#[test]
fn test_stress_list_of_structs_iteration() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@make_point (i: int) -> Point = Point { x: i, y: i * 10 };

@main () -> int = {
    let points = for i in 0..50 yield make_point(i: i);
    let count = points.iter().count();
    if count == 50 then 0 else 1
}
"#,
        "stress_list_of_structs",
    );
}

#[test]
fn test_stress_list_of_strings() {
    assert_aot_success(
        r#"
@main () -> int = {
    let strings = ["alpha", "beta", "gamma", "delta", "epsilon",
                    "zeta", "eta", "theta", "iota", "kappa"];
    let total_len = 0;
    for s in strings do total_len = total_len + s.length();
    // 5+4+5+5+7+4+3+5+4+5 = 47
    if total_len == 47 then 0 else 1
}
"#,
        "stress_list_of_strings",
    );
}

// ─── Multiple block scopes with allocations ───

#[test]
fn test_stress_many_block_scopes() {
    assert_aot_success(
        r#"
type Box = { val: int }

@main () -> int = {
    let sum = 0;
    sum = sum + { let b = Box { val: 1 }; b.val };
    sum = sum + { let b = Box { val: 2 }; b.val };
    sum = sum + { let b = Box { val: 3 }; b.val };
    sum = sum + { let b = Box { val: 4 }; b.val };
    sum = sum + { let b = Box { val: 5 }; b.val };
    sum = sum + { let b = Box { val: 6 }; b.val };
    sum = sum + { let b = Box { val: 7 }; b.val };
    sum = sum + { let b = Box { val: 8 }; b.val };
    sum = sum + { let b = Box { val: 9 }; b.val };
    sum = sum + { let b = Box { val: 10 }; b.val };
    if sum == 55 then 0 else 1
}
"#,
        "stress_many_block_scopes",
    );
}

// ─── Multiple function calls in loop ───

#[test]
fn test_stress_many_function_calls() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

@make_point (x: int, y: int) -> Point = Point { x: x, y: y };
@sum_point (p: Point) -> int = p.x + p.y;

@main () -> int = {
    let total = 0;
    for i in 0..200 do {
        let p = make_point(x: i, y: i + 1);
        total = total + sum_point(p: p)
    };
    // sum((i + i+1) for i=0..199) = sum(2i+1) = 2*Σi + 200 = 2*199*200/2 + 200 = 39800 + 200 = 40000
    if total == 40000 then 0 else 1
}
"#,
        "stress_many_function_calls",
    );
}

// ─── Shared ownership stress ───

#[test]
fn test_stress_shared_struct_many_refs() {
    assert_aot_success(
        r#"
type Data = { value: int, label: str }

@main () -> int = {
    let d = Data { value: 42, label: "shared" };
    let a = d;
    let b = d;
    let c = d;
    let e = d;
    let f = d;
    if a.value + b.value + c.value + e.value + f.value == 210 then 0 else 1
}
"#,
        "stress_shared_many_refs",
    );
}

// ─── Iterator pipeline stress ───

#[test]
fn test_stress_iterator_long_chain() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 0..100 yield i;
    let result = xs.iter()
        .filter(x -> x % 2 == 0)
        .map(x -> x * 3)
        .filter(x -> x % 4 == 0)
        .take(10)
        .count();
    if result == 10 then 0 else 1
}
"#,
        "stress_iterator_long_chain",
    );
}

#[test]
fn test_stress_iterator_fold_large() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 1..=100 yield i;
    let sum = xs.iter().fold(start: 0, f: (acc: int, x: int) -> int = acc + x);
    // sum(1..100) = 100*101/2 = 5050
    if sum == 5050 then 0 else 1
}
"#,
        "stress_iterator_fold_large",
    );
}

// ─── Struct passed through function chain ───

#[test]
fn test_stress_struct_function_chain() {
    assert_aot_success(
        r#"
type Pt = { x: int, y: int }

@translate (p: Pt, dx: int, dy: int) -> Pt = Pt { x: p.x + dx, y: p.y + dy };
@scale (p: Pt, factor: int) -> Pt = Pt { x: p.x * factor, y: p.y * factor };

@main () -> int = {
    let p = Pt { x: 1, y: 2 };
    let p2 = translate(p: p, dx: 3, dy: 4);
    let p3 = scale(p: p2, factor: 2);
    let p4 = translate(p: p3, dx: -1, dy: -1);
    // p2 = (4, 6), p3 = (8, 12), p4 = (7, 11)
    if p4.x == 7 && p4.y == 11 then 0 else 1
}
"#,
        "stress_struct_fn_chain",
    );
}

// ─── Combined: structs + closures + iteration ───

#[test]
#[ignore = "AOT gap: .map(r -> r.score) closure accessing struct field produces invalid LLVM IR"]
fn test_stress_combined_struct_closure_iteration() {
    assert_aot_success(
        r#"
type Record = { id: int, score: int }

@make_record (i: int) -> Record = Record { id: i, score: i * 10 };

@main () -> int = {
    let records = for i in 1..=20 yield make_record(i: i);
    let total = records.iter()
        .map(r -> r.score)
        .filter(s -> s > 50)
        .fold(start: 0, f: (acc: int, s: int) -> int = acc + s);
    // scores: 10,20,30,40,50,60,70,80,90,100,110,120,130,140,150,160,170,180,190,200
    // filtered (>50): 60+70+80+90+100+110+120+130+140+150+160+170+180+190+200 = 1950
    if total == 1950 then 0 else 1
}
"#,
        "stress_combined_struct_closure_iter",
    );
}
