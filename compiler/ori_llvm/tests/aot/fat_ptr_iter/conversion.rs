//! Collection conversion tests — F12: `map.keys()`, `map.values()`, `set.to_list()`, `str.split()`.

use crate::util::assert_aot_success;

// map.keys() on {str: int} → [str]

#[test]
fn test_map_keys_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO threshold here": 10,
        "another very long key that also exceeds the SSO thresh": 20
    };
    let keys = m.keys();
    let total = 0;
    for k in keys do {
        total = total + k.len();
    };
    // 53 + 53 = 106
    if total == 106 then 0 else 1
}
"#,
        "map_keys_str",
    );
}

// map.values() on {int: str} → [str]

#[test]
fn test_map_values_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        1: "this is a very long value exceeding SSO threshold here",
        2: "another very long value that also exceeds the SSO thresh"
    };
    let vals = m.values();
    let total = 0;
    for v in vals do {
        total = total + v.len();
    };
    // 54 + 56 = 110
    if total == 110 then 0 else 1
}
"#,
        "map_values_str",
    );
}

// str.split() → [str]

#[test]
fn test_str_split() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "this is a very long string that exceeds SSO threshold,another very long string also exceeds";
    let parts = s.split(sep: ",");
    let total = 0;
    for p in parts do {
        total = total + p.len();
    };
    // 53 + 37 = 90
    if total == 90 then 0 else 1
}
"#,
        "str_split",
    );
}

// map.keys() then use original map — verify key_inc_fn prevents dangling

#[test]
fn test_map_keys_then_use_map() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO threshold here": 10,
        "another very long key that also exceeds the SSO thresh": 20
    };
    let keys = m.keys();
    let val_total = 0;
    for (_, v) in m do {
        val_total = val_total + v;
    };
    let key_total = 0;
    for k in keys do {
        key_total = key_total + k.len();
    };
    if val_total == 30 && key_total == 106 then 0 else 1
}
"#,
        "map_keys_then_use_map",
    );
}
