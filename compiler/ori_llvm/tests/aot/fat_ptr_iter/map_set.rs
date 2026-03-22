//! Map and set iteration tests — string keys/values with RC cleanup.

use crate::util::assert_aot_success;

// Map iteration with string keys/values

#[test]
fn test_map_str_key_iteration() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO threshold here": 10,
        "another very long key that also exceeds the SSO thresh": 20
    };
    let total = 0;
    for (k, v) in m do {
        total = total + v;
    };
    if total == 30 then 0 else 1
}
"#,
        "map_str_key_iteration",
    );
}

// Map iteration with RC-managed keys/values — key_dec_fn/val_dec_fn

#[test]
fn test_map_str_key_for_do_full() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO": 10,
        "another very long key that exceeds SSO too": 20,
        "third long key exceeding SSO threshold": 30
    };
    let total = 0;
    for (k, v) in m do {
        total = total + v;
    };
    if total == 60 then 0 else 1
}
"#,
        "map_str_key_for_do_full",
    );
}

/// `{str: int}` map for-do with early break — unconsumed entries' str keys
/// must be cleaned up by `key_dec_fn` in the iterator's Drop path.
#[test]
fn test_map_str_key_for_do_break() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO": 10,
        "another very long key that exceeds SSO too": 20,
        "third long key exceeding SSO threshold": 30
    };
    let total = 0;
    for (k, v) in m do {
        total = total + v;
        if total >= 10 then break;
    };
    if total >= 10 then 0 else 1
}
"#,
        "map_str_key_for_do_break",
    );
}

/// `{int: str}` map for-do — heap str values cleaned up via `val_dec_fn`.
#[test]
fn test_map_str_val_for_do() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        10: "this is a very long value exceeding SSO",
        20: "another very long value that exceeds SSO too",
        30: "third long value exceeding SSO threshold"
    };
    let total = 0;
    for (k, v) in m do {
        total = total + k;
    };
    if total == 60 then 0 else 1
}
"#,
        "map_str_val_for_do",
    );
}

/// `{str: str}` map for-do — both keys and values are heap strings,
/// both `key_dec_fn` and `val_dec_fn` must work.
#[test]
fn test_map_str_key_str_val_for_do() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO": "long value exceeding SSO threshold too",
        "another very long key that exceeds SSO": "another long value exceeding threshold"
    };
    let count = 0;
    for (k, v) in m do {
        count = count + 1;
    };
    if count == 2 then 0 else 1
}
"#,
        "map_str_key_str_val_for_do",
    );
}

// T8-F3: {str: int} map for-yield — derive values from map iteration.
// Exercises emit_set_iter conversion under for-yield control flow.

#[test]
fn test_map_str_key_for_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO": 10,
        "another very long key that exceeds SSO too": 20,
        "third long key exceeding SSO threshold": 30
    };
    let values = for (k, v) in m yield v;
    let total = 0;
    for v in values do {
        total = total + v;
    };
    if total == 60 then 0 else 1
}
"#,
        "map_str_key_for_yield",
    );
}

// T9: Set<str> — set with string elements

#[test]
fn test_set_str_iteration() {
    // Set<str> converts to contiguous list via ori_set_to_list before iterating.
    // The output list needs elem_dec_fn for proper string cleanup.
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ].iter().collect();
    let total = 0;
    for item in s do {
        total = total + item.len();
    };
    // 53 + 56 = 109
    if total == 109 then 0 else 1
}
"#,
        "set_str_iteration",
    );
}

// T9-F3: Set<str> for-yield — exercises emit_set_iter conversion to list under
// for-yield control flow. Both source set and derived list need cleanup.

#[test]
fn test_set_str_for_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ].iter().collect();
    let lengths = for item in s yield item.len();
    let total = 0;
    for n in lengths do {
        total = total + n;
    };
    // 53 + 56 = 109
    if total == 109 then 0 else 1
}
"#,
        "set_str_for_yield",
    );
}
