//! F12: Sum Type Payload — fat pointer types as enum variant payloads.
//!
//! Tests tag + payload layout, extractvalue for payloads, and RC handling
//! when fat pointer values are wrapped in sum type variants.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// T4/T5: Sum type with str payload
#[test]
fn test_fm_sum_str_payload() {
    assert_aot_success(
        r#"
type Value = Text(content: str) | Empty;

@main () -> int = {
    let v = Text(content: "hello");
    match v {
        Text(content) ->if content.length() == 5 then 0 else 1,
        Empty -> 2,
    }
}
"#,
        "fm_sum_str_payload",
    );
}

// T5: Sum type with heap str payload
#[test]
fn test_fm_sum_str_heap_payload() {
    assert_aot_success(
        r#"
type Value = Text(content: str) | Empty;

@main () -> int = {
    let v = Text(content: "abcdefghijklmnopqrstuvwxyz1234");
    match v {
        Text(content) ->if content.length() == 30 then 0 else 1,
        Empty -> 2,
    }
}
"#,
        "fm_sum_str_heap_payload",
    );
}

// T6: Sum type with list of scalars payload
#[test]
fn test_fm_sum_list_scalar_payload() {
    assert_aot_success(
        r#"
type Data = Numbers(items: [int]) | Empty;

@main () -> int = {
    let d = Numbers(items: [10, 20, 30]);
    match d {
        Numbers(items) ->if items.length() == 3 then 0 else 1,
        Empty -> 2,
    }
}
"#,
        "fm_sum_list_scalar_payload",
    );
}

// T7: Sum type with list of fat pointers payload
#[test]
fn test_fm_sum_list_fat_payload() {
    assert_aot_success(
        r#"
type Data = Words(items: [str]) | Empty;

@main () -> int = {
    let d = Words(items: ["hello", "world"]);
    match d {
        Words(items) ->if items.length() == 2 then 0 else 1,
        Empty -> 2,
    }
}
"#,
        "fm_sum_list_fat_payload",
    );
}

// T9: Sum type with struct (fat fields) payload
#[test]
fn test_fm_sum_struct_fat_payload() {
    assert_aot_success(
        r#"
type Named = { name: str, id: int }
type Value = Person(info: Named) | Unknown;

@main () -> int = {
    let v = Person(info: Named { name: "alice", id: 1 });
    match v {
        Person(info) ->if info.name.length() + info.id == 6 then 0 else 1,
        Unknown -> 2,
    }
}
"#,
        "fm_sum_struct_fat_payload",
    );
}

// Sum type with multiple fat-payload variants
#[test]
fn test_fm_sum_multi_fat_variant() {
    assert_aot_success(
        r#"
type Content = Title(text: str) | Items(list: [int]) | Nothing;

@get_size (c: Content) -> int =
    match c {
        Title(text) ->text.length(),
        Items(list) ->list.length(),
        Nothing -> 0,
    };

@main () -> int = {
    let a = get_size(c: Title(text: "hello"));
    let b = get_size(c: Items(list: [1, 2, 3]));
    let c = get_size(c: Nothing);
    if a == 5 then {
        if b == 3 then {
            if c == 0 then 0 else 1
        } else 2
    } else 3
}
"#,
        "fm_sum_multi_fat_variant",
    );
}

// None variant matching (Option<str> with no payload)
#[test]
fn test_fm_sum_none_variant() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x: Option<str> = None;
    match x {
        Some(_) -> 1,
        None -> 0,
    }
}
"#,
        "fm_sum_none_variant",
    );
}

// Sum type payload passed to function
#[test]
fn test_fm_sum_payload_passed() {
    assert_aot_success(
        r#"
type Value = Text(content: str) | Empty;

@extract_len (v: Value) -> int =
    match v {
        Text(content) ->content.length(),
        Empty -> 0,
    };

@main () -> int = {
    let v = Text(content: "hello");
    if extract_len(v: v) == 5 then 0 else 1
}
"#,
        "fm_sum_payload_passed",
    );
}
