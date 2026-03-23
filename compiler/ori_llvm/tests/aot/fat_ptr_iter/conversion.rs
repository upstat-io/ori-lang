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

// Semantic pin: slice strings in tuples — TPR-04-003 fix.
// str.split() produces seamless slice strings (SLICE_FLAG in cap).
// When stored in [(str, int)], the per-field RC inc/dec in rc_value_traversal
// must use ori_str_rc_inc/ori_str_rc_dec (slice-aware), not ori_rc_inc/ori_rc_dec.
// Without the fix: misaligned pointer dereference crash on RC inc of interior ptr.

#[test]
fn test_str_split_in_tuple_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "this is a long string part one,and this is another very long part two";
    let parts = s.split(sep: ",");
    let pairs: [(str, int)] = for p in parts yield (p, p.len());
    let total = 0;
    for (text, n) in pairs do {
        total = total + n + text.len();
    };
    // parts: "this is a long string part one" (30), "and this is another very long part two" (38)
    // pairs: [("this...", 30), ("and...", 38)]
    // total = 30 + 30 + 38 + 38 = 136
    if total == 136 then 0 else 1
}
"#,
        "str_split_in_tuple_list",
    );
}

// Semantic pin: slice strings in Option — TPR-04-003 fix.
// Similar to tuple test but exercises Option variant dispatch path.

#[test]
fn test_str_split_in_option_list() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello this is a long enough string to exceed SSO,world and another long one";
    let parts = s.split(sep: ",");
    let opts: [Option<str>] = for p in parts yield Some(p);
    let total = 0;
    for opt in opts do {
        match opt {
            Some(text) -> { total = total + text.len(); },
            None -> ()
        };
    };
    // "hello this is a long enough string to exceed SSO" (48) + "world and another long one" (26) = 74
    if total == 74 then 0 else 1
}
"#,
        "str_split_in_option_list",
    );
}

// Semantic pin: top-level slice string passed to/from function — TPR-04-003 fix.
// str.split() result elements are slice strings. When a slice string is passed
// as a top-level str parameter (FatPointer RC strategy), emit_rc_inc_fat must
// use ori_str_rc_inc(data, cap) not ori_rc_inc(data).

#[test]
fn test_str_split_function_param() {
    assert_aot_success(
        r#"
@use_str (s: str) -> int = s.len();

@main () -> int = {
    let text = "this is a long string part that exceeds SSO limit easily,and so does this second part";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let n = use_str(s: p);
        total = total + n;
    };
    // "this is a long string part that exceeds SSO limit easily" (56) + "and so does this second part" (28) = 84
    if total == 84 then 0 else 1
}
"#,
        "str_split_function_param",
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

// Semantic pin: slice-string clone — TPR-04-004 fix.
// str.split() produces seamless slice strings. When clone() is called on a
// slice str, emit_rc_inc_clone → emit_slice_aware_rc_inc must use
// ori_str_rc_inc(data, cap), not ori_rc_inc(data).
// Without the fix: misaligned pointer dereference crash on ori_rc_inc of
// interior pointer.

#[test]
fn test_str_split_clone() {
    assert_aot_success(
        r#"
@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let copy = p.clone();
        total = total + copy.len();
    };
    // "this is a long string exceeding SSO by a large amount" (53) +
    // "and this second part also exceeds it" (36) = 89
    if total == 89 then 0 else 1
}
"#,
        "str_split_clone",
    );
}

// Semantic pin: slice-string auto-iteration — TPR-04-004 fix.
// When an iterator method (e.g., .map, .any, .count) is called on a str
// from str.split(), emit_auto_iter → emit_slice_aware_rc_inc must use
// ori_str_rc_inc(data, cap). Without the fix: crash on auto-iter RcInc
// of interior pointer.

#[test]
fn test_str_split_auto_iter() {
    assert_aot_success(
        r#"
@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let n = p.iter().count();
        total = total + n;
    };
    // "this is a long string exceeding SSO by a large amount" = 53 chars
    // "and this second part also exceeds it" = 36 chars
    // total = 89
    if total == 89 then 0 else 1
}
"#,
        "str_split_auto_iter",
    );
}

// F12: set.to_list() on Set<str> — exercises ori_set_to_list with elem_dec_fn.

#[test]
fn test_set_to_list_str() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ].iter().collect();
    let list = s.to_list();
    let total = 0;
    for item in list do {
        total = total + item.len();
    };
    // 53 + 56 = 109
    if total == 109 then 0 else 1
}
"#,
        "set_to_list_str",
    );
}

// Semantic pin: TPR-04-006 — splitting a substring (slice-backed string) must not crash.
// ori_str_split now receives str_cap to detect SLICE_FLAG and compute the original
// allocation base for RC inc. Without the fix: misaligned pointer dereference.

#[test]
fn test_str_split_on_substring() {
    assert_aot_success(
        r#"
@main () -> int = {
    let base = "AAAAAAAAAAAAAAAAAAAAAAAA,BBBBBBBBBBBBBBBBBBBBBBBB,CCCCCCCCCCCC";
    // Take a substring that contains a comma — it's a seamless slice
    let sub = base.substring(start: 0, end: 49);
    // Now split the slice-backed string — this is the TPR-04-006 regression case
    let parts = sub.split(sep: ",");
    let total = 0;
    for p in parts do {
        total = total + p.len();
    };
    // "AAAAAAAAAAAAAAAAAAAAAAAA" (24) + "BBBBBBBBBBBBBBBBBBBBBBBB" (24) = 48
    if total == 48 then 0 else 1
}
"#,
        "str_split_on_substring",
    );
}

// Semantic pin: derived Clone on struct with slice-string field — TPR-04-005 fix.
// emit_clone_rc_inc_str() must use ori_str_rc_inc(data, cap), not ori_rc_inc(data).
// Without the fix: misaligned pointer dereference on ori_rc_inc of interior pointer
// from str.split() slice strings.

#[test]
fn test_derive_clone_slice_str_struct() {
    assert_aot_success(
        r#"
#derive(Clone)
type Wrapper = { s: str, n: int }

@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let w = Wrapper { s: p, n: p.len() };
        let copy = w.clone();
        total = total + copy.n;
    };
    // 53 + 36 = 89
    if total == 89 then 0 else 1
}
"#,
        "derive_clone_slice_str_struct",
    );
}

// Semantic pin: derived Clone on struct with Option<str> field — TPR-04-005 fix.
// emit_clone_composite_rc_inc recursively processes Option fields, eventually
// calling emit_clone_rc_inc_str on the inner str. Slice strings must be
// handled with ori_str_rc_inc(data, cap).

#[test]
fn test_derive_clone_slice_str_option() {
    assert_aot_success(
        r#"
#derive(Clone)
type MaybeNamed = { name: Option<str>, id: int }

@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let w = MaybeNamed { name: Some(p), id: p.len() };
        let copy = w.clone();
        total = total + copy.id;
    };
    // 53 + 36 = 89
    if total == 89 then 0 else 1
}
"#,
        "derive_clone_slice_str_option",
    );
}

// Semantic pin: derived Clone on struct with (str, int) tuple field — TPR-04-005 fix.
// emit_clone_composite_rc_inc recursively processes Tuple fields, eventually
// calling emit_clone_rc_inc_str on the str element. Slice strings must be
// handled with ori_str_rc_inc(data, cap).

#[test]
fn test_derive_clone_slice_str_tuple() {
    assert_aot_success(
        r#"
#derive(Clone)
type Pair = { data: (str, int) }

@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let w = Pair { data: (p, p.len()) };
        let copy = w.clone();
        total = total + copy.data.1;
    };
    // 53 + 36 = 89
    if total == 89 then 0 else 1
}
"#,
        "derive_clone_slice_str_tuple",
    );
}

// Semantic pin: TPR-04-007 — derived Clone on struct with Result<str, str> field.
// emit_clone_field_rc_inc must handle Tag::Result by branching on the tag and
// RC-incrementing the correct variant's payload. Without the fix, Result payloads
// are never RC-incremented → double-free when both original and clone are dropped.

#[test]
fn test_derive_clone_result_str_str() {
    assert_aot_success(
        r#"
#derive(Clone)
type Holder = { payload: Result<str, str>, id: int }

@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let ok_holder = Holder { payload: Ok(p), id: p.len() };
        let ok_copy = ok_holder.clone();
        let err_holder = Holder { payload: Err(p), id: p.len() };
        let err_copy = err_holder.clone();
        total = total + ok_copy.id + err_copy.id;
    };
    // (53 + 53) + (36 + 36) = 178
    if total == 178 then 0 else 1
}
"#,
        "derive_clone_result_str_str",
    );
}

// Semantic pin: TPR-04-007 — derived Clone on struct with Result<str, int> field.
// Tests the heterogeneous case where Ok type needs RC but Err type is scalar.

#[test]
fn test_derive_clone_result_str_int() {
    assert_aot_success(
        r#"
#derive(Clone)
type MaybeError = { result: Result<str, int>, n: int }

@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let ok_val = MaybeError { result: Ok(p), n: p.len() };
        let ok_copy = ok_val.clone();
        let err_val = MaybeError { result: Err(42), n: 42 };
        let err_copy = err_val.clone();
        total = total + ok_copy.n + err_copy.n;
    };
    // (53 + 42) + (36 + 42) = 173
    if total == 173 then 0 else 1
}
"#,
        "derive_clone_result_str_int",
    );
}
