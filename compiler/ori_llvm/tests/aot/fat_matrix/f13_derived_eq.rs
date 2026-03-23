//! F13: Derived Eq — equality comparison on types containing fat pointer fields.
//!
//! Tests the `$eq` derived method codegen for structs and sum types whose fields
//! include strings, lists, and other fat pointer types.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4/T5: Eq on struct with str field
#[test]
fn test_fm_eq_struct_str() {
    assert_aot_success(
        r#"
#derive(Eq)
type Named = { name: str, id: int }

@main () -> int = {
    let a = Named { name: "alice", id: 1 };
    let b = Named { name: "alice", id: 1 };
    let c = Named { name: "bob", id: 1 };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_struct_str",
    );
}

// T6: Eq on struct with list of scalars
#[test]
fn test_fm_eq_struct_list_scalar() {
    assert_aot_success(
        r#"
#derive(Eq)
type Container = { items: [int] }

@main () -> int = {
    let a = Container { items: [1, 2, 3] };
    let b = Container { items: [1, 2, 3] };
    let c = Container { items: [1, 2, 4] };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_struct_list_scalar",
    );
}

// T7: Eq on struct with list of strings
#[test]
fn test_fm_eq_struct_list_fat() {
    assert_aot_success(
        r#"
#derive(Eq)
type Words = { items: [str] }

@main () -> int = {
    let a = Words { items: ["hello", "world"] };
    let b = Words { items: ["hello", "world"] };
    let c = Words { items: ["hello", "other"] };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_struct_list_fat",
    );
}

// T9: Eq on struct with nested fat fields
#[test]
fn test_fm_eq_nested_fat() {
    assert_aot_success(
        r#"
#derive(Eq)
type Inner = { name: str }
#derive(Eq)
type Outer = { inner: Inner, count: int }

@main () -> int = {
    let a = Outer { inner: Inner { name: "alice" }, count: 5 };
    let b = Outer { inner: Inner { name: "alice" }, count: 5 };
    let c = Outer { inner: Inner { name: "bob" }, count: 5 };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_nested_fat",
    );
}

// Sum type: Eq on Option<str>
#[test]
fn test_fm_eq_option_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = Some("hello");
    let b = Some("hello");
    let c = Some("world");
    let d: Option<str> = None;
    if a == b then {
        if a != c then {
            if a != d then 0 else 1
        } else 2
    } else 3
}
"#,
        "fm_eq_option_str",
    );
}

// Direct str comparison
#[test]
fn test_fm_eq_str_direct() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = "hello";
    let c = "world";
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_str_direct",
    );
}

// Eq on struct with multiple fat fields
#[test]
fn test_fm_eq_multi_fat_fields() {
    assert_aot_success(
        r#"
#derive(Eq)
type Person = { first: str, last: str }

@main () -> int = {
    let a = Person { first: "alice", last: "smith" };
    let b = Person { first: "alice", last: "smith" };
    let c = Person { first: "alice", last: "jones" };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_multi_fat_fields",
    );
}

// Eq with heap strings (>23 bytes)
#[test]
fn test_fm_eq_heap_str() {
    assert_aot_success(
        r#"
#derive(Eq)
type Doc = { content: str }

@main () -> int = {
    let a = Doc { content: "abcdefghijklmnopqrstuvwxyz1234" };
    let b = Doc { content: "abcdefghijklmnopqrstuvwxyz1234" };
    let c = Doc { content: "abcdefghijklmnopqrstuvwxyz9999" };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_heap_str",
    );
}

// BUG-04-03b: Derived Eq on struct with [str] using heap-backed strings
// Regression: ori_list_eq_scalar did byte-level memcmp which fails for heap
// strings (different data pointers but identical content).
#[test]
fn test_fm_eq_list_heap_str() {
    assert_aot_success(
        r#"
#derive(Eq)
type Words = { items: [str] }

@main () -> int = {
    let s1 = "abcdefghijklmnopqrstuvwxyz1234";
    let s2 = "abcdefghijklmnopqrstuvwxyz1234";
    let a = Words { items: [s1] };
    let b = Words { items: [s2] };
    if a == b then 0 else 1
}
"#,
        "fm_eq_list_heap_str",
    );
}

// BUG-04-03b: Multiple heap strings in a list
#[test]
fn test_fm_eq_list_multiple_heap_str() {
    assert_aot_success(
        r#"
#derive(Eq)
type Doc = { lines: [str] }

@main () -> int = {
    let a = Doc { lines: ["aaaaaaaaaaaaaaaaaaaaaaaaa1", "bbbbbbbbbbbbbbbbbbbbbbbbb2"] };
    let b = Doc { lines: ["aaaaaaaaaaaaaaaaaaaaaaaaa1", "bbbbbbbbbbbbbbbbbbbbbbbbb2"] };
    let c = Doc { lines: ["aaaaaaaaaaaaaaaaaaaaaaaaa1", "ccccccccccccccccccccccccc3"] };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_list_multi_heap_str",
    );
}

// BUG-04-03b: Empty list equality
#[test]
fn test_fm_eq_list_empty() {
    assert_aot_success(
        r#"
#derive(Eq)
type Container = { items: [str] }

@main () -> int = {
    let empty1 = Container { items: [] };
    let empty2 = Container { items: [] };
    let nonempty = Container { items: ["hello"] };
    if empty1 == empty2 then {
        if empty1 != nonempty then 0 else 1
    } else 2
}
"#,
        "fm_eq_list_empty",
    );
}

// BUG-04-03b: Mixed SSO and heap strings in a list
#[test]
fn test_fm_eq_list_mixed_str() {
    assert_aot_success(
        r#"
#derive(Eq)
type MixedDoc = { items: [str] }

@main () -> int = {
    let a = MixedDoc { items: ["hi", "a_very_long_heap_allocated_string_here"] };
    let b = MixedDoc { items: ["hi", "a_very_long_heap_allocated_string_here"] };
    if a == b then 0 else 1
}
"#,
        "fm_eq_list_mixed_str",
    );
}

// BUG-04-03c: Map equality with composite value type {str: [int]}
#[test]
fn test_fm_eq_map_composite_list_val() {
    assert_aot_success(
        r#"
#derive(Eq)
type Wrapper = { m: {str: [int]} }

@main () -> int = {
    let a = Wrapper { m: {"x": [1, 2, 3]} };
    let b = Wrapper { m: {"x": [1, 2, 3]} };
    let c = Wrapper { m: {"x": [1, 2, 4]} };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_map_composite_list_val",
    );
}

// BUG-04-03c: Map equality with str value type (non-primitive)
#[test]
fn test_fm_eq_map_str_val() {
    assert_aot_success(
        r#"
#derive(Eq)
type Config = { settings: {int: str} }

@main () -> int = {
    let a = Config { settings: {1: "hello_world_long_heap_string_here"} };
    let b = Config { settings: {1: "hello_world_long_heap_string_here"} };
    let c = Config { settings: {1: "different_heap_string_value_here!"} };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_map_str_val",
    );
}

// BUG-04-03c: Map equality — str keys with int values (base case, should still work)
#[test]
fn test_fm_eq_map_primitive_val() {
    assert_aot_success(
        r#"
#derive(Eq)
type Counts = { m: {str: int} }

@main () -> int = {
    let a = Counts { m: {"a": 1, "b": 2} };
    let b = Counts { m: {"a": 1, "b": 2} };
    let c = Counts { m: {"a": 1, "b": 3} };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_map_primitive_val",
    );
}

// TPR-04-003: Derived Eq on struct with [Option<str>] — wrapper elements
// require deep comparison but were missed by needs_deep_comparison().
#[test]
fn test_fm_eq_list_option_str() {
    assert_aot_success(
        r#"
#derive(Eq)
type Wrap = { items: [Option<str>] }

@main () -> int = {
    let a = Wrap { items: [Some("hello world long heap string!"), None, Some("another long str here!")] };
    let b = Wrap { items: [Some("hello world long heap string!"), None, Some("another long str here!")] };
    let c = Wrap { items: [Some("hello world long heap string!"), None, Some("DIFFERENT long string!")] };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_list_option_str",
    );
}

// TPR-04-003: Derived Eq on struct with [Option<str>] — both None
#[test]
fn test_fm_eq_list_option_str_both_none() {
    assert_aot_success(
        r#"
#derive(Eq)
type Wrap = { items: [Option<str>] }

@main () -> int = {
    let a = Wrap { items: [None, None] };
    let b = Wrap { items: [None, None] };
    if a == b then 0 else 1
}
"#,
        "fm_eq_list_option_none",
    );
}

// TPR-04-004: Derived Eq on struct with {str: Option<str>} — wrapper map values
#[test]
fn test_fm_eq_map_option_str_val() {
    assert_aot_success(
        r#"
#derive(Eq)
type Config = { m: {str: Option<str>} }

@main () -> int = {
    let a = Config { m: {"key": Some("a long heap allocated value!!!")} };
    let b = Config { m: {"key": Some("a long heap allocated value!!!")} };
    let c = Config { m: {"key": Some("different heap allocated value!")} };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_map_option_str_val",
    );
}

// TPR-04-003/004: Derived Eq with Option field directly (not nested in list/map)
#[test]
fn test_fm_eq_option_field_direct() {
    assert_aot_success(
        r#"
#derive(Eq)
type MaybeNamed = { name: Option<str>, id: int }

@main () -> int = {
    let a = MaybeNamed { name: Some("alice has a long name here!!!"), id: 1 };
    let b = MaybeNamed { name: Some("alice has a long name here!!!"), id: 1 };
    let c = MaybeNamed { name: None, id: 1 };
    let d = MaybeNamed { name: None, id: 1 };
    if a == b then {
        if a != c then {
            if c == d then 0 else 3
        } else 2
    } else 1
}
"#,
        "fm_eq_option_field_direct",
    );
}

// TPR-04-003: Derived Eq with Result<str, str> in list
// Uses SSO-length strings to avoid pre-existing [Result<str,str>] RC leak
// (heap strings in Result list elements leak — tracked in rc-integrity plan).
#[test]
fn test_fm_eq_list_result_str() {
    assert_aot_success(
        r#"
#derive(Eq)
type Outcomes = { results: [Result<str, str>] }

@main () -> int = {
    let a = Outcomes { results: [Ok("success"), Err("fail")] };
    let b = Outcomes { results: [Ok("success"), Err("fail")] };
    let c = Outcomes { results: [Ok("success"), Err("DIFF")] };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_list_result_str",
    );
}

// TPR-04-003: Derived Eq with (int, str) tuple in list
#[test]
fn test_fm_eq_list_tuple() {
    assert_aot_success(
        r#"
#derive(Eq)
type Pairs = { items: [(int, str)] }

@main () -> int = {
    let a = Pairs { items: [(1, "long heap allocated string here!")] };
    let b = Pairs { items: [(1, "long heap allocated string here!")] };
    let c = Pairs { items: [(1, "DIFFERENT heap allocated string!")] };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_list_tuple",
    );
}

// TPR-04-005: Direct [Named] list equality where Named has fat fields
#[test]
fn test_fm_eq_list_fat_struct() {
    assert_aot_success(
        r#"
#derive(Eq)
type Named = { name: str, id: int }

@main () -> int = {
    let a = [Named { name: "alice", id: 1 }, Named { name: "bob has a very long name here", id: 2 }];
    let b = [Named { name: "alice", id: 1 }, Named { name: "bob has a very long name here", id: 2 }];
    let c = [Named { name: "alice", id: 1 }, Named { name: "charlie", id: 3 }];
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_list_fat_struct",
    );
}

// TPR-04-005: Struct with [Named] field (compute_elem_size must use LLVM layout)
#[test]
fn test_fm_eq_struct_with_list_of_fat_struct() {
    assert_aot_success(
        r#"
#derive(Eq)
type Named = { name: str, id: int }

#derive(Eq)
type Container = { items: [Named] }

@main () -> int = {
    let a = Container { items: [Named { name: "heap string that is long enough!", id: 1 }] };
    let b = Container { items: [Named { name: "heap string that is long enough!", id: 1 }] };
    let c = Container { items: [Named { name: "different heap string content!!", id: 9 }] };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_struct_with_list_of_fat_struct",
    );
}

// TPR-04-005: Equality on empty [Named] list (edge case)
#[test]
fn test_fm_eq_list_fat_struct_empty() {
    assert_aot_success(
        r#"
#derive(Eq)
type Named = { name: str, id: int }

@main () -> int = {
    let a: [Named] = [];
    let b: [Named] = [];
    if a == b then 0 else 1
}
"#,
        "fm_eq_list_fat_struct_empty",
    );
}

// TPR-04-005: Map with fat-struct values
#[test]
fn test_fm_eq_map_fat_struct_value() {
    assert_aot_success(
        r#"
#derive(Eq)
type Named = { name: str, id: int }

#derive(Eq)
type Lookup = { data: {int: Named} }

@main () -> int = {
    let a = Lookup { data: {1: Named { name: "very long heap string here!!!", id: 10 }} };
    let b = Lookup { data: {1: Named { name: "very long heap string here!!!", id: 10 }} };
    let c = Lookup { data: {1: Named { name: "different string content!!!", id: 99 }} };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#,
        "fm_eq_map_fat_struct_value",
    );
}
