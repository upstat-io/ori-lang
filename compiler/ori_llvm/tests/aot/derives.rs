//! AOT Derive Trait Codegen Tests
//!
//! End-to-end tests verifying that `#[derive(...)]` generates correct native code
//! through the LLVM backend. Each test compiles Ori source to a native binary,
//! runs it, and checks the exit code (0 = success).
//!
//! Covers roadmap Section 3.5: Derive Traits (Eq, Clone, Hashable, Printable).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use ori_ir::DerivedTrait;

use crate::util::assert_aot_success;

// --- Cross-crate sync enforcement (Section 05.1, Test 5) ---

#[test]
fn all_derived_traits_have_codegen() {
    // Known gaps — traits with documented reasons for missing LLVM codegen.
    // Adding a trait here requires a comment explaining why codegen is deferred.
    // Removing a trait means codegen was implemented — update the count below.
    let known_gaps: &[DerivedTrait] = &[
        DerivedTrait::Debug, // deferred: interpreter-only (trait_arch backlog)
    ];

    // Guard: pinned trait count forces this test to be reviewed when a new
    // DerivedTrait variant is added. Update this constant, then either
    // implement LLVM codegen or add the trait to known_gaps above.
    assert_eq!(
        DerivedTrait::COUNT,
        7,
        "DerivedTrait::COUNT changed! Update this test: either implement \
         LLVM codegen for the new trait or add it to known_gaps with a reason."
    );

    // Verify known_gaps entries are valid (no stale entries after variant removal)
    for gap in known_gaps {
        assert!(
            DerivedTrait::ALL.contains(gap),
            "Stale known_gap: {gap:?} is no longer in DerivedTrait::ALL"
        );
    }

    // Every trait in ALL must be either in known_gaps or expected to have codegen.
    // The pinned count above is the real guard; this documents intent.
    let should_have_codegen: Vec<_> = DerivedTrait::ALL
        .iter()
        .filter(|t| !known_gaps.contains(t))
        .collect();

    assert_eq!(
        should_have_codegen.len(),
        6,
        "Traits expected to have LLVM codegen changed: {should_have_codegen:?}. \
         Update this count after implementing codegen or adding to known_gaps."
    );
}

// 3.5.1: Derive Eq

#[test]
fn test_aot_derive_eq_basic() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Point = { x: int, y: int }

@main () -> int = {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 1, y: 2 };
    let c = Point { x: 3, y: 4 };
    if a.eq(other: b) && !a.eq(other: c) then 0 else 1
}
"#,
        "derive_eq_basic",
    );
}

#[test]
fn test_aot_derive_eq_with_strings() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Config = { name: str }

@main () -> int = {
    let a = Config { name: "hello" };
    let b = Config { name: "hello" };
    let c = Config { name: "world" };
    if a.eq(other: b) && !a.eq(other: c) then 0 else 1
}
"#,
        "derive_eq_with_strings",
    );
}

#[test]
fn test_aot_derive_eq_mixed_types() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Record = { id: int, active: bool, score: float }

@main () -> int = {
    let a = Record { id: 1, active: true, score: 3.14 };
    let b = Record { id: 1, active: true, score: 3.14 };
    let c = Record { id: 1, active: false, score: 3.14 };
    if a.eq(other: b) && !a.eq(other: c) then 0 else 1
}
"#,
        "derive_eq_mixed_types",
    );
}

#[test]
fn test_aot_derive_eq_single_field() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Wrapper = { value: int }

@main () -> int = {
    let a = Wrapper { value: 42 };
    let b = Wrapper { value: 42 };
    let c = Wrapper { value: 99 };
    if a.eq(other: b) && !a.eq(other: c) then 0 else 1
}
"#,
        "derive_eq_single_field",
    );
}

// 3.5.2: Derive Clone

#[test]
fn test_aot_derive_clone_basic() {
    assert_aot_success(
        r#"
#[derive(Eq, Clone)]
type Point = { x: int, y: int }

@main () -> int = {
    let a = Point { x: 10, y: 20 };
    let b = a.clone();
    if a.eq(other: b) then 0 else 1
}
"#,
        "derive_clone_basic",
    );
}

#[test]
fn test_aot_derive_clone_large_struct() {
    assert_aot_success(
        r#"
#[derive(Eq, Clone)]
type Big = { a: int, b: int, c: int }

@main () -> int = {
    let x = Big { a: 1, b: 2, c: 3 };
    let y = x.clone();
    if x.eq(other: y) then 0 else 1
}
"#,
        "derive_clone_large_struct",
    );
}

// 3.5.3: Derive Hashable

#[test]
fn test_aot_derive_hash_equal_values() {
    assert_aot_success(
        r#"
#[derive(Eq, Hashable)]
type Point = { x: int, y: int }

@main () -> int = {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 1, y: 2 };
    if a.hash() == b.hash() then 0 else 1
}
"#,
        "derive_hash_equal_values",
    );
}

#[test]
fn test_aot_derive_hash_different_values() {
    assert_aot_success(
        r#"
#[derive(Eq, Hashable)]
type Point = { x: int, y: int }

@main () -> int = {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 3, y: 4 };
    if a.hash() != b.hash() then 0 else 1
}
"#,
        "derive_hash_different_values",
    );
}

// 3.5.4: Derive Printable

#[test]
fn test_aot_derive_printable_basic() {
    assert_aot_success(
        r#"
#[derive(Printable)]
type Point = { x: int, y: int }

@main () -> int = {
    let p = Point { x: 1, y: 2 };
    let s = p.to_str();
    if s.len() > 0 then 0 else 1
}
"#,
        "derive_printable_basic",
    );
}

// 3.5.5: Derive Default

#[test]
fn test_aot_derive_default_basic() {
    assert_aot_success(
        r#"
#[derive(Default)]
type Point = { x: int, y: int }

@main () -> int = {
    let p = Point.default();
    if p.x == 0 && p.y == 0 then 0 else 1
}
"#,
        "derive_default_basic",
    );
}

#[test]
fn test_aot_derive_default_mixed_types() {
    assert_aot_success(
        r#"
#[derive(Default)]
type Config = { count: int, enabled: bool, score: float }

@main () -> int = {
    let c = Config.default();
    if c.count == 0 && c.enabled == false && c.score == 0.0 then 0 else 1
}
"#,
        "derive_default_mixed_types",
    );
}

#[test]
fn test_aot_derive_default_eq_integration() {
    assert_aot_success(
        r#"
#[derive(Default, Eq)]
type Point = { x: int, y: int }

@main () -> int = {
    let a = Point.default();
    let b = Point.default();
    if a.eq(other: b) then 0 else 1
}
"#,
        "derive_default_eq_integration",
    );
}

#[test]
fn test_aot_derive_default_str_field() {
    assert_aot_success(
        r#"
#[derive(Default)]
type Record = { name: str, count: int }

@main () -> int = {
    let r = Record.default();
    if r.name == "" && r.count == 0 then 0 else 1
}
"#,
        "derive_default_str_field",
    );
}

#[test]
fn test_aot_derive_default_nested() {
    assert_aot_success(
        r#"
#[derive(Default)]
type Inner = { x: int, y: int }

#[derive(Default)]
type Outer = { inner: Inner, label: str }

@main () -> int = {
    let o = Outer.default();
    if o.inner.x == 0 && o.inner.y == 0 && o.label == "" then 0 else 1
}
"#,
        "derive_default_nested",
    );
}

// 3.7: Clone trait on primitives (built-in identity clone)

#[test]
fn test_aot_clone_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    let y = x.clone();
    if y == 42 then 0 else 1
}
"#,
        "clone_int",
    );
}

#[test]
fn test_aot_clone_float() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 3.14;
    let y = x.clone();
    if y == 3.14 then 0 else 1
}
"#,
        "clone_float",
    );
}

#[test]
fn test_aot_clone_bool() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = true.clone();
    let b = false.clone();
    if a && !b then 0 else 1
}
"#,
        "clone_bool",
    );
}

#[test]
fn test_aot_clone_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    let s2 = s.clone();
    if s2 == "hello" then 0 else 1
}
"#,
        "clone_str",
    );
}

// 3.7: Clone on collections

#[test]
fn test_aot_clone_list_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items = [1, 2, 3];
    let items2 = items.clone();
    if items2.len() == 3 then 0 else 1
}
"#,
        "clone_list_int",
    );
}

#[test]
fn test_aot_clone_list_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let items: [int] = [];
    let items2 = items.clone();
    if items2.len() == 0 then 0 else 1
}
"#,
        "clone_list_empty",
    );
}

// 3.7: Clone on Option

#[test]
fn test_aot_clone_option_some() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt = Some(42);
    let opt2 = opt.clone();
    if (opt2 ?? 0) == 42 then 0 else 1
}
"#,
        "clone_option_some",
    );
}

#[test]
fn test_aot_clone_option_none() {
    assert_aot_success(
        r#"
@main () -> int = {
    let opt: Option<int> = None;
    let opt2 = opt.clone();
    if (opt2 ?? -1) == -1 then 0 else 1
}
"#,
        "clone_option_none",
    );
}

// 3.7: Clone on Result

#[test]
fn test_aot_clone_result_ok() {
    assert_aot_success(
        r#"
@main () -> int = {
    let r: Result<int, str> = Ok(42);
    let r2 = r.clone();
    if r2.is_ok() then 0 else 1
}
"#,
        "clone_result_ok",
    );
}

#[test]
fn test_aot_clone_result_err() {
    assert_aot_success(
        r#"
@main () -> int = {
    let r: Result<int, str> = Err("fail");
    let r2 = r.clone();
    if r2.is_err() then 0 else 1
}
"#,
        "clone_result_err",
    );
}

// 3.7: Clone on tuples

#[test]
fn test_aot_clone_tuple_pair() {
    // Tuple destructuring not yet implemented in AOT codegen,
    // so we verify clone compiles and the value is usable.
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (42, 99);
    let _t2 = t.clone();
    0
}
"#,
        "clone_tuple_pair",
    );
}

#[test]
fn test_aot_clone_tuple_triple() {
    // Tuple destructuring not yet implemented in AOT codegen,
    // so we verify clone compiles and the value is usable.
    assert_aot_success(
        r#"
@main () -> int = {
    let t = (1, 2, 3);
    let _t2 = t.clone();
    0
}
"#,
        "clone_tuple_triple",
    );
}

// 3.14: Derive Comparable

#[test]
fn test_aot_derive_comparable_basic() {
    assert_aot_success(
        r#"
#[derive(Eq, Comparable)]
type Point = { x: int, y: int }

@main () -> int = {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 1, y: 3 };
    let c = Point { x: 1, y: 2 };
    let ab = a.compare(other: b);
    let ac = a.compare(other: c);
    if ab.is_less() && ac.is_equal() then 0 else 1
}
"#,
        "derive_comparable_basic",
    );
}

#[test]
fn test_aot_derive_comparable_first_field_wins() {
    assert_aot_success(
        r#"
#[derive(Eq, Comparable)]
type Pair = { x: int, y: int }

@main () -> int = {
    let a = Pair { x: 5, y: 1 };
    let b = Pair { x: 3, y: 999 };
    let cmp = a.compare(other: b);
    if cmp.is_greater() then 0 else 1
}
"#,
        "derive_comparable_first_field",
    );
}

#[test]
fn test_aot_derive_comparable_with_strings() {
    assert_aot_success(
        r#"
#[derive(Eq, Comparable)]
type Named = { name: str, id: int }

@main () -> int = {
    let a = Named { name: "alice", id: 1 };
    let b = Named { name: "bob", id: 1 };
    let c = Named { name: "alice", id: 2 };
    let ab = a.compare(other: b);
    let ac = a.compare(other: c);
    if ab.is_less() && ac.is_less() then 0 else 1
}
"#,
        "derive_comparable_strings",
    );
}

#[test]
fn test_aot_derive_comparable_single_field() {
    assert_aot_success(
        r#"
#[derive(Eq, Comparable)]
type Wrapper = { value: int }

@main () -> int = {
    let a = Wrapper { value: 10 };
    let b = Wrapper { value: 20 };
    let c = Wrapper { value: 10 };
    let ab = a.compare(other: b);
    let ac = a.compare(other: c);
    if ab.is_less() && ac.is_equal() then 0 else 1
}
"#,
        "derive_comparable_single_field",
    );
}

// 3.5.6: Multiple derives on one type

#[test]
fn test_aot_derive_multiple_traits() {
    assert_aot_success(
        r#"
#[derive(Eq, Clone)]
type Pair = { x: int, y: int }

@main () -> int = {
    let a = Pair { x: 5, y: 10 };
    let b = a.clone();
    if a.eq(other: b) then 0 else 1
}
"#,
        "derive_multiple_traits",
    );
}

// =========================================================================
// 3.14: Derive hash edge cases (hygiene fixes)
// =========================================================================

// Derive Hashable with float fields: ±0.0 must produce same hash

#[test]
fn test_aot_derive_hash_float_neg_zero() {
    assert_aot_success(
        r#"
#[derive(Eq, Hashable)]
type Wrapper = { value: float }

@main () -> int = {
    let a = Wrapper { value: 0.0 };
    let b = Wrapper { value: -0.0 };
    // 0.0 and -0.0 are equal, so their hashes must match
    if a.hash() == b.hash() then 0 else 1
}
"#,
        "derive_hash_float_neg_zero",
    );
}

// Derive Hashable with str fields: different strings must hash differently

#[test]
fn test_aot_derive_hash_str_content() {
    assert_aot_success(
        r#"
#[derive(Eq, Hashable)]
type Named = { name: str }

@main () -> int = {
    let a = Named { name: "abc" };
    let b = Named { name: "abc" };
    let c = Named { name: "xyz" };
    // Same string → same hash
    let r1 = a.hash() == b.hash();
    // Different string (same length) → different hash
    let r2 = a.hash() != c.hash();
    if r1 && r2 then 0 else 1
}
"#,
        "derive_hash_str_content",
    );
}

// Derive Hashable with byte field: values ≥ 128 must use unsigned extension

#[test]
fn test_aot_derive_hash_byte_field() {
    assert_aot_success(
        r#"
#[derive(Eq, Hashable)]
type ByteBox = { b: byte }

@main () -> int = {
    let a = ByteBox { b: byte(200) };
    let b = ByteBox { b: byte(200) };
    let c = ByteBox { b: byte(100) };
    // Same byte → same hash
    let r1 = a.hash() == b.hash();
    // Different byte → different hash
    let r2 = a.hash() != c.hash();
    if r1 && r2 then 0 else 1
}
"#,
        "derive_hash_byte_field",
    );
}

// =========================================================================
// C3: Derive Eq on payload sum types (code journey 11)
// =========================================================================

#[test]
fn test_aot_derive_eq_payload_sum_type() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Shape = Circle(radius: int) | Rect(w: int, h: int);

@main () -> int = {
    let s1 = Circle(radius: 10);
    let s2 = Circle(radius: 10);
    let s3 = Rect(w: 5, h: 8);
    let same = s1 == s2;
    let diff = s1 != s3;
    if same && diff then 0 else 1
}
"#,
        "derive_eq_payload_sum",
    );
}

#[test]
fn test_aot_derive_eq_mixed_variant_comparison() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Shape = Circle(radius: int) | Rect(w: int, h: int);

@main () -> int = {
    let c1 = Circle(radius: 10);
    let c2 = Circle(radius: 10);
    let c3 = Circle(radius: 20);
    let r1 = Rect(w: 5, h: 8);
    let r2 = Rect(w: 5, h: 8);
    let same_circle = c1 == c2;
    let diff_circle = c1 != c3;
    let cross_type = c1 != r1;
    let same_rect = r1 == r2;
    if same_circle && diff_circle && cross_type && same_rect then 0 else 1
}
"#,
        "derive_eq_mixed_variant",
    );
}

#[test]
fn test_aot_derive_eq_single_payload_variant() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Wrapper = Val(x: int) | Empty;

@main () -> int = {
    let v1 = Val(x: 42);
    let v2 = Val(x: 42);
    let v3 = Val(x: 99);
    let e = Empty;
    let same_val = v1 == v2;
    let diff_val = v1 != v3;
    let val_vs_empty = v1 != e;
    if same_val && diff_val && val_vs_empty then 0 else 1
}
"#,
        "derive_eq_single_payload",
    );
}

#[test]
fn test_aot_journey_11_derived_eq() {
    assert_aot_success(
        r#"
#[derive(Eq)]
type Point = { x: int, y: int }

#[derive(Eq)]
type Color = Red | Green | Blue;

#[derive(Eq)]
type Shape = Circle(radius: int) | Rect(w: int, h: int);

@check_struct_eq () -> int = {
    let p1 = Point { x: 10, y: 20 };
    let p2 = Point { x: 10, y: 20 };
    let p3 = Point { x: 10, y: 30 };
    let same = if p1 == p2 then 3 else 0;
    let diff = if p1 != p3 then 4 else 0;
    same + diff
}

@check_sum_eq () -> int = {
    let c1 = Red;
    let c2 = Red;
    let c3 = Blue;
    let unit_same = if c1 == c2 then 5 else 0;
    let unit_diff = if c1 != c3 then 6 else 0;
    unit_same + unit_diff
}

@check_nested () -> int = {
    let s1 = Circle(radius: 10);
    let s2 = Circle(radius: 10);
    let s3 = Rect(w: 5, h: 8);
    let payload_same = if s1 == s2 then 7 else 0;
    let payload_diff = if s1 != s3 then 8 else 0;
    payload_same + payload_diff
}

@main () -> int = {
    let a = check_struct_eq();
    let b = check_sum_eq();
    let c = check_nested();
    if a + b + c == 33 then 0 else 1
}
"#,
        "journey_11_derived_eq",
    );
}

// ---- TPR-04-021: Debug derive str field leak fix ----
//
// The `emit_field_to_string` Debug/Str path creates `"\"" + val + "\""`
// via two concats. The inner concat result (`quoted`) must be RC-decremented
// after being consumed by the outer concat. Additionally, the `emit_str_rc_dec`
// helper must pass `ori_str_drop_buffer` (not null) as the drop function, so
// `ori_rc_dec` can free the buffer when RC reaches 0.
//
// These tests form a matrix: {short (SSO) str, long (heap) str} x
// {single str field, multiple str fields, mixed str+int fields}.

/// Semantic pin: long str field (heap-backed intermediate) — the core
/// regression case. Without the fix, `ori_str_concat`'s in-place append
/// reuses the buffer, and the intermediate's RC reaches 0 inside the
/// derive function with no drop function to free it.
#[test]
fn test_aot_derive_debug_str_field_no_leak() {
    assert_aot_success(
        r#"
#[derive(Debug)]
type Wrapper = { msg: str }

@main () -> int = {
    // 28 chars — exceeds 23-byte SSO threshold, forces heap allocation
    let w = Wrapper { msg: "this is a long string value!" };
    let s = w.debug();
    if s.contains(substr: "this is a long string value!") then 0 else 1
}
"#,
        "derive_debug_str_field_no_leak",
    );
}

/// Short str field (SSO) — the intermediate stays SSO, so no heap
/// allocation to leak. Verifies the fix doesn't break the SSO path.
#[test]
fn test_aot_derive_debug_short_str_no_leak() {
    assert_aot_success(
        r#"
#[derive(Debug)]
type Tag = { label: str }

@main () -> int = {
    let t = Tag { label: "ok" };
    let s = t.debug();
    if s.contains(substr: "ok") then 0 else 1
}
"#,
        "derive_debug_short_str_no_leak",
    );
}

/// Multiple str fields — each field independently creates a quoted
/// intermediate. All must be properly freed.
#[test]
fn test_aot_derive_debug_multi_str_no_leak() {
    assert_aot_success(
        r#"
#[derive(Debug)]
type Pair = { first: str, second: str }

@main () -> int = {
    let p = Pair { first: "aaaaaaaaaaaaaaaaaaaaaaaaa", second: "bbbbbbbbbbbbbbbbbbbbbbbbb" };
    let s = p.debug();
    let ok = s.contains(substr: "aaaaaaaaaaaaaaaaaaaaaaaaa")
        && s.contains(substr: "bbbbbbbbbbbbbbbbbbbbbbbbb");
    if ok then 0 else 1
}
"#,
        "derive_debug_multi_str_no_leak",
    );
}

/// Mixed str + int fields — the str field goes through the Debug quoting
/// path while the int field goes through the sext/format path. Both must
/// clean up properly without interfering with each other.
#[test]
fn test_aot_derive_debug_mixed_str_int_no_leak() {
    assert_aot_success(
        r#"
#[derive(Debug)]
type Record = { name: str, count: int }

@main () -> int = {
    let r = Record { name: "a]long]record]name]value!!", count: 42 };
    let s = r.debug();
    let ok = s.contains(substr: "a]long]record]name]value!!")
        && s.contains(substr: "42");
    if ok then 0 else 1
}
"#,
        "derive_debug_mixed_str_int_no_leak",
    );
}
