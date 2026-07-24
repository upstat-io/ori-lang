//! `IndexSet.updated` AOT Integration Tests
//!
//! Mirrors the COW coverage of `cow_map_set.rs` for the `updated` method:
//! - List/fixed-list: COW replacement (unique in-place, shared copy), OOB panic
//!   matching `list[key]` semantics, ARC-managed element move (no leak,
//!   no double-free — `ORI_CHECK_LEAKS=1` enforced by the harness).
//! - Map: insert-or-replace with moved value, never panics.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, assert_panic_exit, compile_and_run_capture};

// List: unique receiver (in-place fast path)

#[test]
fn test_updated_list_unique_replaces_element() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.updated(key: 1, value: 9);
    if ys[0] == 1 && ys[1] == 9 && ys[2] == 3 then 0 else 1
}
"#,
        "updated_list_unique_replaces_element",
    );
}

// List: shared receiver (COW copy path) - alias unchanged, no leak

#[test]
fn test_updated_list_shared_preserves_alias() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [1, 2, 3];
    let b = a;
    let c = a.updated(key: 0, value: 99);
    if b[0] == 1 && c[0] == 99 then 0 else 1
}
"#,
        "updated_list_shared_preserves_alias",
    );
}

// Dead alias of a consumed receiver — the canonical phantom-obligation leak
// shape (the alias is never read; the COW transfer consumes one of the two
// class references; the dead alias's dec is the balance).
#[test]
fn test_updated_list_dead_alias_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [1, 2, 3];
    let b = a;
    let c = a.updated(key: 0, value: 99);
    if c[0] == 99 then 0 else 1
}
"#,
        "updated_list_dead_alias_no_leak",
    );
}

// List: ARC-managed (str) elements - moved value, released replacee

#[test]
fn test_updated_list_str_elements_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = ["alpha-string-beyond-sso-width", "b", "c"];
    let ys = xs.updated(key: 1, value: "inserted-string-beyond-sso-width");
    if ys[1] == "inserted-string-beyond-sso-width" && ys[0] == xs[0] then 0 else 1
}
"#,
        "updated_list_str_elements_no_leak",
    );
}

#[test]
fn test_updated_list_str_shared_copy_no_double_free() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = ["alpha-string-beyond-sso-width", "b"];
    let b = a;
    let c = a.updated(key: 0, value: "replacement-beyond-sso-width");
    if b[0] == "alpha-string-beyond-sso-width" && c[0] == "replacement-beyond-sso-width" then 0
    else 1
}
"#,
        "updated_list_str_shared_copy_no_double_free",
    );
}

// List: OOB key panics (matching `list[key]` index semantics)

#[test]
fn test_updated_list_oob_panics() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.updated(key: 5, value: 0);
    ys[0]
}
"#,
    );
    assert_panic_exit(exit_code, "updated_list_oob_panics", &stderr);
    assert!(
        stderr.contains("index out of bounds"),
        "OOB panic message should match list[key] semantics, got: {stderr}"
    );
}

#[test]
fn test_updated_list_negative_key_panics() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.updated(key: 0 - 1, value: 0);
    ys[0]
}
"#,
    );
    assert_panic_exit(exit_code, "updated_list_negative_key_panics", &stderr);
}

// Fixed-capacity list (erases to the List tag)

#[test]
fn test_updated_fixed_list_replaces_element() {
    assert_aot_success(
        r#"
@main () -> int = {
    let fixed: [int, max 3] = [1, 2, 3];
    let ys = fixed.updated(key: 2, value: 9);
    if ys[2] == 9 then 0 else 1
}
"#,
        "updated_fixed_list_replaces_element",
    );
}

#[test]
fn test_updated_fixed_list_oob_panics() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let fixed: [int, max 3] = [1, 2, 3];
    let ys = fixed.updated(key: 5, value: 0);
    ys[0]
}
"#,
    );
    assert_panic_exit(exit_code, "updated_fixed_list_oob_panics", &stderr);
}

// Map: insert-or-replace, moved ARC-managed value, never panics

#[test]
fn test_updated_map_replaces_existing_key() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = { "a": 1, "b": 2 };
    let n = m.updated(key: "b", value: 9);
    if n["b"] == Some(9) && n["a"] == Some(1) then 0 else 1
}
"#,
        "updated_map_replaces_existing_key",
    );
}

#[test]
fn test_updated_map_inserts_new_key() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = { "a": 1 };
    let n = m.updated(key: "z", value: 7);
    if n["z"] == Some(7) && n.len() == 2 then 0 else 1
}
"#,
        "updated_map_inserts_new_key",
    );
}

#[test]
fn test_updated_map_arc_value_moved_no_leak() {
    assert_aot_success(
        r#"
@main () -> int = {
    let m = { "a": [1], "b": [2] };
    let n = m.updated(key: "b", value: [7, 8, 9]);
    if n["b"] == Some([7, 8, 9]) then 0 else 1
}
"#,
        "updated_map_arc_value_moved_no_leak",
    );
}

#[test]
fn test_updated_map_shared_receiver_preserves_alias() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = { "k": 1 };
    let b = a;
    let c = a.updated(key: "k", value: 99);
    if b["k"] == Some(1) && c["k"] == Some(99) then 0 else 1
}
"#,
        "updated_map_shared_receiver_preserves_alias",
    );
}
