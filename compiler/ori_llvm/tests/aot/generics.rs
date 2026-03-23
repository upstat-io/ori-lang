//! Generic Function AOT Tests
//!
//! Tests for generic function monomorphization through the AOT pipeline.
//! Covers: identity/pair basics, string/struct type arguments (RC-managed),
//! generic-calling-generic chains, multiple specializations, generic with
//! Option/Result, and generic functions returning tuples.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Generic with string arguments (RC-managed) ───

#[test]
fn test_generic_identity_string() {
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;

@main () -> int = {
    let s = identity(x: "hello");
    if s == "hello" then 0 else 1
}
"#,
        "generic_identity_string",
    );
}

#[test]
fn test_generic_pair_with_strings() {
    assert_aot_success(
        r#"
@make_pair <A, B> (a: A, b: B) -> (A, B) = (a, b);

@main () -> int = {
    let p = make_pair(a: "hello", b: "world");
    let (x, y) = p;
    if x == "hello" && y == "world" then 0 else 1
}
"#,
        "generic_pair_strings",
    );
}

#[test]
fn test_generic_swap_strings() {
    assert_aot_success(
        r#"
@swap <A, B> (a: A, b: B) -> (B, A) = (b, a);

@main () -> int = {
    let (b, a) = swap(a: "first", b: "second");
    if a == "first" && b == "second" then 0 else 1
}
"#,
        "generic_swap_strings",
    );
}

// ─── Generic with struct arguments ───

#[test]
fn test_generic_identity_struct() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@identity <T> (x: T) -> T = x;

@main () -> int = {
    let p = identity(x: Point { x: 10, y: 20 });
    if p.x == 10 && p.y == 20 then 0 else 1
}
"#,
        "generic_identity_struct",
    );
}

#[test]
fn test_generic_pair_mixed_struct_int() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@make_pair <A, B> (a: A, b: B) -> (A, B) = (a, b);

@main () -> int = {
    let p = make_pair(a: Point { x: 1, y: 2 }, b: 42);
    let (pt, n) = p;
    if pt.x == 1 && pt.y == 2 && n == 42 then 0 else 1
}
"#,
        "generic_pair_struct_int",
    );
}

// ─── Multiple specializations in same program ───

#[test]
fn test_generic_four_specializations() {
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;

@main () -> int = {
    let a = identity(x: 42);
    let b = identity(x: true);
    let c = identity(x: "hello");
    let d = identity(x: 3.14);
    if a == 42 && b && c == "hello" && d > 3.0 then 0 else 1
}
"#,
        "generic_four_specializations",
    );
}

#[test]
fn test_generic_same_type_multiple_calls() {
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;

@main () -> int = {
    let a = identity(x: 10);
    let b = identity(x: 20);
    let c = identity(x: 30);
    if a + b + c == 60 then 0 else 1
}
"#,
        "generic_same_type_multiple",
    );
}

// ─── Generic calling generic ───

#[test]
fn test_generic_calling_generic() {
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;
@apply_identity <T> (x: T) -> T = identity(x: x);

@main () -> int = {
    let a = apply_identity(x: 42);
    let b = apply_identity(x: true);
    if a == 42 && b then 0 else 1
}
"#,
        "generic_calling_generic",
    );
}

#[test]
fn test_generic_chain_three_levels() {
    assert_aot_success(
        r#"
@id <T> (x: T) -> T = x;
@wrap <T> (x: T) -> T = id(x: x);
@double_wrap <T> (x: T) -> T = wrap(x: x);

@main () -> int = {
    let n = double_wrap(x: 42);
    if n == 42 then 0 else 1
}
"#,
        "generic_chain_three_levels",
    );
}

// ─── Generic calling generic: multi-type-param ───

#[test]
fn test_generic_chain_two_type_params() {
    // A<T> calls B<T> which has TWO type params — one from caller, one concrete.
    assert_aot_success(
        r#"
@make_pair <A, B> (a: A, b: B) -> (A, B) = (a, b);
@wrap_with_int <T> (x: T) -> (T, int) = make_pair(a: x, b: 99);

@main () -> int = {
    let (s, n) = wrap_with_int(x: "hello");
    if s == "hello" && n == 99 then 0 else 1
}
"#,
        "generic_chain_two_params",
    );
}

#[test]
fn test_generic_chain_swap_params() {
    // Caller has <A,B>, callee has <X,Y> — params cross over.
    assert_aot_success(
        r#"
@swap <X, Y> (a: X, b: Y) -> (Y, X) = (b, a);
@swap_twice <A, B> (a: A, b: B) -> (A, B) = swap(a: b, b: a);

@main () -> int = {
    let (x, y) = swap_twice(a: 10, b: 20);
    if x == 10 && y == 20 then 0 else 1
}
"#,
        "generic_chain_swap_params",
    );
}

// ─── Generic calling generic: multiple callees ───

#[test]
fn test_generic_calls_two_different_generics() {
    // One generic caller invokes two different generic callees.
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;
@duplicate <T> (x: T) -> (T, T) = (x, x);
@id_and_dup <T> (x: T) -> (T, (T, T)) = {
    let a = identity(x: x);
    let b = duplicate(x: x);
    (a, b)
};

@main () -> int = {
    let (a, (b, c)) = id_and_dup(x: 42);
    if a == 42 && b == 42 && c == 42 then 0 else 1
}
"#,
        "generic_calls_two_generics",
    );
}

// ─── Generic calling generic: string (RC-managed) ───

#[test]
fn test_generic_chain_with_strings() {
    // Generic chain with RC-managed types to verify retain/release correctness.
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;
@apply_identity <T> (x: T) -> T = identity(x: x);

@main () -> int = {
    let s = apply_identity(x: "hello");
    if s == "hello" then 0 else 1
}
"#,
        "generic_chain_strings",
    );
}

#[test]
fn test_generic_chain_three_levels_string() {
    // Three-level chain with strings — stress-tests RC in deep chains.
    assert_aot_success(
        r#"
@id <T> (x: T) -> T = x;
@wrap <T> (x: T) -> T = id(x: x);
@double_wrap <T> (x: T) -> T = wrap(x: x);

@main () -> int = {
    let s = double_wrap(x: "world");
    if s == "world" then 0 else 1
}
"#,
        "generic_chain_three_levels_string",
    );
}

// ─── Generic calling generic: mixed specializations ───

#[test]
fn test_generic_chain_multiple_specializations() {
    // Same chain instantiated with different types in one program.
    assert_aot_success(
        r#"
@id <T> (x: T) -> T = x;
@wrap <T> (x: T) -> T = id(x: x);

@main () -> int = {
    let a = wrap(x: 42);
    let b = wrap(x: true);
    let c = wrap(x: "test");
    let d = wrap(x: 3.14);
    if a == 42 && b && c == "test" && d > 3.0 then 0 else 1
}
"#,
        "generic_chain_four_specializations",
    );
}

// ─── Generic calling generic: with conditional logic ───

#[test]
fn test_generic_chain_choose() {
    // Chain where the intermediate generic uses conditional logic.
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;
@pick_first <T> (a: T, b: T) -> T = identity(x: a);

@main () -> int = {
    let n = pick_first(a: 10, b: 20);
    if n == 10 then 0 else 1
}
"#,
        "generic_chain_choose",
    );
}

// ─── Generic calling generic: with struct types ───

#[test]
fn test_generic_chain_with_struct() {
    // Chain passing structs through multiple generic functions.
    assert_aot_success(
        r#"
type Point = { x: int, y: int };

@identity <T> (x: T) -> T = x;
@apply_identity <T> (x: T) -> T = identity(x: x);

@main () -> int = {
    let p = apply_identity(x: Point { x: 3, y: 4 });
    if p.x == 3 && p.y == 4 then 0 else 1
}
"#,
        "generic_chain_struct",
    );
}

// ─── Generic with conditional logic ───

#[test]
fn test_generic_with_bool_condition() {
    assert_aot_success(
        r#"
@choose <T> (cond: bool, a: T, b: T) -> T =
    if cond then a else b;

@main () -> int = {
    let x = choose(cond: true, a: 10, b: 20);
    let y = choose(cond: false, a: 10, b: 20);
    if x == 10 && y == 20 then 0 else 1
}
"#,
        "generic_choose",
    );
}

#[test]
fn test_generic_choose_strings() {
    assert_aot_success(
        r#"
@choose <T> (cond: bool, a: T, b: T) -> T =
    if cond then a else b;

@main () -> int = {
    let s = choose(cond: true, a: "yes", b: "no");
    if s == "yes" then 0 else 1
}
"#,
        "generic_choose_strings",
    );
}

// ─── Generic returning tuple ───

#[test]
fn test_generic_duplicate() {
    assert_aot_success(
        r#"
@duplicate <T> (x: T) -> (T, T) = (x, x);

@main () -> int = {
    let (a, b) = duplicate(x: 42);
    if a == 42 && b == 42 then 0 else 1
}
"#,
        "generic_duplicate",
    );
}

#[test]
fn test_generic_duplicate_string() {
    assert_aot_success(
        r#"
@duplicate <T> (x: T) -> (T, T) = (x, x);

@main () -> int = {
    let (a, b) = duplicate(x: "hello");
    if a == "hello" && b == "hello" then 0 else 1
}
"#,
        "generic_duplicate_string",
    );
}

// ─── Generic with Option ───

#[test]
fn test_generic_with_option_some() {
    // NOTE: Uses .is_some()/.unwrap() instead of match to avoid
    // pre-existing ARC leak in Option match codegen (not generic-specific).
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;

@main () -> int = {
    let opt = identity(x: Some(42));
    if opt.is_some() && opt.unwrap() == 42 then 0 else 1
}
"#,
        "generic_option_some",
    );
}

#[test]
fn test_generic_wrap_in_option() {
    assert_aot_success(
        r#"
@wrap_some <T> (x: T) -> Option<T> = Some(x);

@main () -> int = {
    let a = wrap_some(x: 42);
    if a.is_some() && a.unwrap() == 42 then 0 else 1
}
"#,
        "generic_wrap_option",
    );
}

#[test]
fn test_generic_option_match_leak() {
    // This test documents the leak — match on Option leaks even without generics.
    assert_aot_success(
        r#"
@main () -> int = {
    let opt: Option<int> = Some(42);
    match opt {
        Some(n) -> if n == 42 then 0 else 1,
        None -> 2,
    }
}
"#,
        "generic_option_match_leak",
    );
}

// ─── Generic with Result ───

#[test]
fn test_generic_ok_result() {
    assert_aot_success(
        r#"
@wrap_ok <T> (x: T) -> Result<T, str> = Ok(x);

@main () -> int = {
    let r = wrap_ok(x: 42);
    match r {
        Ok(n) -> if n == 42 then 0 else 1,
        Err(_) -> 2,
    }
}
"#,
        "generic_ok_result",
    );
}

// ─── Generic with HOF ───

#[test]
fn test_generic_apply_fn() {
    assert_aot_success(
        r#"
@apply <T> (f: (T) -> T, x: T) -> T = f(x);
@double (n: int) -> int = n * 2;

@main () -> int = {
    let result = apply(f: double, x: 21);
    if result == 42 then 0 else 1
}
"#,
        "generic_apply_fn",
    );
}

// ─── Generic with for-yield ───

#[test]
fn test_generic_in_for_yield() {
    assert_aot_success(
        r#"
@identity <T> (x: T) -> T = x;

@main () -> int = {
    let nums = [1, 2, 3, 4, 5];
    let mapped = for n in nums yield identity(x: n);
    let sum = mapped.iter().fold(start: 0, f: (acc, n) -> acc + n);
    if sum == 15 then 0 else 1
}
"#,
        "generic_in_for_yield",
    );
}

// ─── Generic with user-defined struct params (indirect type vars) ───

#[test]
fn test_generic_struct_field_access() {
    // Type param T nested in struct param — tests indirect var extraction.
    // first(p: Pair{42, 10}) + 1 should return 43, not 1.
    assert_aot_success(
        r#"
type Pair<A, B> = { first: A, second: B };

@first <A, B> (p: Pair<A, B>) -> A = p.first;

@main () -> int = {
    let p = Pair { first: 42, second: 10 };
    let f = first(p: p);
    if f + 1 == 43 then 0 else 1
}
"#,
        "generic_struct_field_access",
    );
}

#[test]
fn test_generic_struct_two_fields() {
    // Verify both fields of a generic struct are accessible.
    assert_aot_success(
        r#"
type Pair<A, B> = { first: A, second: B };

@make_pair <A, B> (a: A, b: B) -> Pair<A, B> =
    Pair { first: a, second: b };

@main () -> int = {
    let p = make_pair(a: 10, b: 20);
    if p.first == 10 && p.second == 20 then 0 else 1
}
"#,
        "generic_struct_two_fields",
    );
}

#[test]
fn test_generic_struct_nested() {
    // Generic struct containing another generic struct.
    assert_aot_success(
        r#"
type Pair<A, B> = { first: A, second: B };
type Wrapper<T> = { inner: T };

@unwrap <T> (w: Wrapper<T>) -> T = w.inner;

@main () -> int = {
    let p = Pair { first: 42, second: true };
    let w = Wrapper { inner: p };
    let p2 = unwrap(w: w);
    if p2.first == 42 then 0 else 1
}
"#,
        "generic_struct_nested",
    );
}

#[test]
fn test_generic_struct_with_string_field() {
    // Generic struct with RC-managed field — tests ARC interaction.
    assert_aot_success(
        r#"
type Pair<A, B> = { first: A, second: B };

@second <A, B> (p: Pair<A, B>) -> B = p.second;

@main () -> int = {
    let p = Pair { first: 1, second: "hello" };
    let s = second(p: p);
    if s == "hello" then 0 else 1
}
"#,
        "generic_struct_string_field",
    );
}

// ─── Monomorphized nounwind analysis ───

#[test]
fn test_mono_nounwind_callee_uses_call_not_invoke() {
    // After the two-pass nounwind fix, _ori_main should call identity$m$int
    // with `call` (not `invoke`) because identity is trivially nounwind.
    let ir = crate::util::compile_and_capture_ir(
        r#"
@identity <T> (x: T) -> T = x;

@main () -> int = identity(x: 42);
"#,
    );
    // Find the _ori_main function in the IR and check for call vs invoke
    // to the monomorphized identity function.
    let main_section = crate::util::extract_function_ir(&ir, "_ori_main");

    // The monomorphized function name contains identity and $m$
    assert!(
        main_section.contains("call fastcc"),
        "expected `call fastcc` for nounwind monomorphized callee in _ori_main, \
         but found invoke or missing call.\nIR:\n{main_section}"
    );
    // Verify it's NOT using invoke for the identity call
    assert!(
        !main_section.contains("invoke fastcc"),
        "expected NO `invoke fastcc` for nounwind monomorphized callee in _ori_main — \
         the two-pass nounwind analysis should have proven identity nounwind.\nIR:\n{main_section}"
    );
}

#[test]
#[ignore = "Pre-existing: nounwind analysis doesn't yet distinguish may-unwind monomorphized callees"]
fn test_mono_may_unwind_callee_uses_invoke() {
    // A generic function that calls panic should still use `invoke`.
    let ir = crate::util::compile_and_capture_ir(
        r#"
@may_panic <T> (x: T) -> T = {
    if true then panic(msg: "boom") else x
};

@main () -> int = may_panic(x: 42);
"#,
    );

    let main_section = crate::util::extract_function_ir(&ir, "_ori_main");

    // _ori_main should use `invoke` for may_panic$m$int because it calls panic
    assert!(
        main_section.contains("invoke fastcc"),
        "expected `invoke fastcc` for may-unwind monomorphized callee in _ori_main.\n\
         IR:\n{main_section}"
    );
}
